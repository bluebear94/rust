use std::marker::PhantomData;

use smallvec::smallvec;

use crate::data_structures::HashSet;
use crate::inherent::*;
use crate::lang_items::TraitSolverLangItem;
use crate::outlives::{Component, push_outlives_components};
use crate::{self as ty, Interner, Upcast as _};

/// "Elaboration" is the process of identifying all the predicates that
/// are implied by a source predicate. Currently, this basically means
/// walking the "supertraits" and other similar assumptions. For example,
/// if we know that `T: Ord`, the elaborator would deduce that `T: PartialOrd`
/// holds as well. Similarly, if we have `trait Foo: 'static`, and we know that
/// `T: Foo`, then we know that `T: 'static`.
pub struct Elaborator<I: Interner, O> {
    cx: I,
    stack: Vec<O>,
    visited: HashSet<ty::Binder<I, ty::PredicateKind<I>>>,
    mode: Filter,
}

enum Filter {
    All,
    OnlySelf,
}

/// Describes how to elaborate an obligation into a sub-obligation.
pub trait Elaboratable<I: Interner> {
    fn predicate(&self) -> I::Predicate;

    // Makes a new `Self` but with a different clause that comes from elaboration.
    fn child(&self, clause: I::Clause) -> Self;

    // Makes a new `Self` but with a different clause and a different cause
    // code (if `Self` has one, such as [`PredicateObligation`]).
    fn child_with_derived_cause(
        &self,
        clause: I::Clause,
        span: I::Span,
        parent_trait_pred: ty::Binder<I, ty::TraitPredicate<I>>,
        index: usize,
    ) -> Self;
}

pub fn elaborate<I: Interner, O: Elaboratable<I>>(
    cx: I,
    obligations: impl IntoIterator<Item = O>,
) -> Elaborator<I, O> {
    let mut elaborator =
        Elaborator { cx, stack: Vec::new(), visited: HashSet::default(), mode: Filter::All };
    elaborator.extend_deduped(obligations);
    elaborator
}

impl<I: Interner, O: Elaboratable<I>> Elaborator<I, O> {
    fn extend_deduped(&mut self, obligations: impl IntoIterator<Item = O>) {
        // Only keep those bounds that we haven't already seen.
        // This is necessary to prevent infinite recursion in some
        // cases. One common case is when people define
        // `trait Sized: Sized { }` rather than `trait Sized { }`.
        self.stack.extend(
            obligations.into_iter().filter(|o| {
                self.visited.insert(self.cx.anonymize_bound_vars(o.predicate().kind()))
            }),
        );
    }

    /// Filter to only the supertraits of trait predicates, i.e. only the predicates
    /// that have `Self` as their self type, instead of all implied predicates.
    pub fn filter_only_self(mut self) -> Self {
        self.mode = Filter::OnlySelf;
        self
    }

    fn elaborate(&mut self, elaboratable: &O) {
        let cx = self.cx;

        // We only elaborate clauses.
        let Some(clause) = elaboratable.predicate().as_clause() else {
            return;
        };

        let bound_clause = clause.kind();
        match bound_clause.skip_binder() {
            ty::ClauseKind::Trait(data) => {
                // Negative trait bounds do not imply any supertrait bounds
                if data.polarity != ty::PredicatePolarity::Positive {
                    return;
                }

                // HACK(effects): The following code is required to get implied bounds for effects associated
                // types to work with super traits.
                //
                // Suppose `data` is a trait predicate with the form `<T as Tr>::Fx: EffectsCompat<somebool>`
                // and we know that `trait Tr: ~const SuperTr`, we need to elaborate this predicate into
                // `<T as SuperTr>::Fx: EffectsCompat<somebool>`.
                //
                // Since the semantics for elaborating bounds about effects is equivalent to elaborating
                // bounds about super traits (elaborate `T: Tr` into `T: SuperTr`), we place effects elaboration
                // next to super trait elaboration.
                if cx.is_lang_item(data.def_id(), TraitSolverLangItem::EffectsCompat)
                    && matches!(self.mode, Filter::All)
                {
                    // first, ensure that the predicate we've got looks like a `<T as Tr>::Fx: EffectsCompat<somebool>`.
                    if let ty::Alias(ty::AliasTyKind::Projection, alias_ty) = data.self_ty().kind()
                    {
                        // look for effects-level bounds that look like `<Self as Tr>::Fx: TyCompat<<Self as SuperTr>::Fx>`
                        // on the trait, which is proof to us that `Tr: ~const SuperTr`. We're looking for bounds on the
                        // associated trait, so we use `explicit_implied_predicates_of` since it gives us more than just
                        // `Self: SuperTr` bounds.
                        let bounds = cx.explicit_implied_predicates_of(cx.parent(alias_ty.def_id));

                        // instantiate the implied bounds, so we get `<T as Tr>::Fx` and not `<Self as Tr>::Fx`.
                        let elaborated = bounds.iter_instantiated(cx, alias_ty.args).filter_map(
                            |(clause, _)| {
                                let ty::ClauseKind::Trait(tycompat_bound) =
                                    clause.kind().skip_binder()
                                else {
                                    return None;
                                };
                                if !cx.is_lang_item(
                                    tycompat_bound.def_id(),
                                    TraitSolverLangItem::EffectsTyCompat,
                                ) {
                                    return None;
                                }

                                // extract `<T as SuperTr>::Fx` from the `TyCompat` bound.
                                let supertrait_effects_ty =
                                    tycompat_bound.trait_ref.args.type_at(1);
                                let ty::Alias(ty::AliasTyKind::Projection, supertrait_alias_ty) =
                                    supertrait_effects_ty.kind()
                                else {
                                    return None;
                                };

                                // The self types (`T`) must be equal for `<T as Tr>::Fx` and `<T as SuperTr>::Fx`.
                                if supertrait_alias_ty.self_ty() != alias_ty.self_ty() {
                                    return None;
                                };

                                // replace the self type in the original bound `<T as Tr>::Fx: EffectsCompat<somebool>`
                                // to the effects type of the super trait. (`<T as SuperTr>::Fx`)
                                let elaborated_bound = data.with_self_ty(cx, supertrait_effects_ty);
                                Some(
                                    elaboratable
                                        .child(bound_clause.rebind(elaborated_bound).upcast(cx)),
                                )
                            },
                        );
                        self.extend_deduped(elaborated);
                    }
                }

                let map_to_child_clause =
                    |(index, (clause, span)): (usize, (I::Clause, I::Span))| {
                        elaboratable.child_with_derived_cause(
                            clause.instantiate_supertrait(cx, bound_clause.rebind(data.trait_ref)),
                            span,
                            bound_clause.rebind(data),
                            index,
                        )
                    };

                // Get predicates implied by the trait, or only super predicates if we only care about self predicates.
                match self.mode {
                    Filter::All => self.extend_deduped(
                        cx.explicit_implied_predicates_of(data.def_id())
                            .iter_identity()
                            .enumerate()
                            .map(map_to_child_clause),
                    ),
                    Filter::OnlySelf => self.extend_deduped(
                        cx.explicit_super_predicates_of(data.def_id())
                            .iter_identity()
                            .enumerate()
                            .map(map_to_child_clause),
                    ),
                };
            }
            ty::ClauseKind::TypeOutlives(ty::OutlivesPredicate(ty_max, r_min)) => {
                // We know that `T: 'a` for some type `T`. We can
                // often elaborate this. For example, if we know that
                // `[U]: 'a`, that implies that `U: 'a`. Similarly, if
                // we know `&'a U: 'b`, then we know that `'a: 'b` and
                // `U: 'b`.
                //
                // We can basically ignore bound regions here. So for
                // example `for<'c> Foo<'a,'c>: 'b` can be elaborated to
                // `'a: 'b`.

                // Ignore `for<'a> T: 'a` -- we might in the future
                // consider this as evidence that `T: 'static`, but
                // I'm a bit wary of such constructions and so for now
                // I want to be conservative. --nmatsakis
                if r_min.is_bound() {
                    return;
                }

                let mut components = smallvec![];
                push_outlives_components(cx, ty_max, &mut components);
                self.extend_deduped(
                    components
                        .into_iter()
                        .filter_map(|component| elaborate_component_to_clause(cx, component, r_min))
                        .map(|clause| elaboratable.child(bound_clause.rebind(clause).upcast(cx))),
                );
            }
            ty::ClauseKind::RegionOutlives(..) => {
                // Nothing to elaborate from `'a: 'b`.
            }
            ty::ClauseKind::WellFormed(..) => {
                // Currently, we do not elaborate WF predicates,
                // although we easily could.
            }
            ty::ClauseKind::Projection(..) => {
                // Nothing to elaborate in a projection predicate.
            }
            ty::ClauseKind::ConstEvaluatable(..) => {
                // Currently, we do not elaborate const-evaluatable
                // predicates.
            }
            ty::ClauseKind::ConstArgHasType(..) => {
                // Nothing to elaborate
            }
        }
    }
}

fn elaborate_component_to_clause<I: Interner>(
    cx: I,
    component: Component<I>,
    outlives_region: I::Region,
) -> Option<ty::ClauseKind<I>> {
    match component {
        Component::Region(r) => {
            if r.is_bound() {
                None
            } else {
                Some(ty::ClauseKind::RegionOutlives(ty::OutlivesPredicate(r, outlives_region)))
            }
        }

        Component::Param(p) => {
            let ty = Ty::new_param(cx, p);
            Some(ty::ClauseKind::TypeOutlives(ty::OutlivesPredicate(ty, outlives_region)))
        }

        Component::Placeholder(p) => {
            let ty = Ty::new_placeholder(cx, p);
            Some(ty::ClauseKind::TypeOutlives(ty::OutlivesPredicate(ty, outlives_region)))
        }

        Component::UnresolvedInferenceVariable(_) => None,

        Component::Alias(alias_ty) => {
            // We might end up here if we have `Foo<<Bar as Baz>::Assoc>: 'a`.
            // With this, we can deduce that `<Bar as Baz>::Assoc: 'a`.
            Some(ty::ClauseKind::TypeOutlives(ty::OutlivesPredicate(
                alias_ty.to_ty(cx),
                outlives_region,
            )))
        }

        Component::EscapingAlias(_) => {
            // We might be able to do more here, but we don't
            // want to deal with escaping vars right now.
            None
        }
    }
}

impl<I: Interner, O: Elaboratable<I>> Iterator for Elaborator<I, O> {
    type Item = O;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.stack.len(), None)
    }

    fn next(&mut self) -> Option<Self::Item> {
        // Extract next item from top-most stack frame, if any.
        if let Some(obligation) = self.stack.pop() {
            self.elaborate(&obligation);
            Some(obligation)
        } else {
            None
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// Supertrait iterator
///////////////////////////////////////////////////////////////////////////

/// Computes the def-ids of the transitive supertraits of `trait_def_id`. This (intentionally)
/// does not compute the full elaborated super-predicates but just the set of def-ids. It is used
/// to identify which traits may define a given associated type to help avoid cycle errors,
/// and to make size estimates for vtable layout computation.
pub fn supertrait_def_ids<I: Interner>(
    cx: I,
    trait_def_id: I::DefId,
) -> impl Iterator<Item = I::DefId> {
    let mut set = HashSet::default();
    let mut stack = vec![trait_def_id];

    set.insert(trait_def_id);

    std::iter::from_fn(move || {
        let trait_def_id = stack.pop()?;

        for (predicate, _) in cx.explicit_super_predicates_of(trait_def_id).iter_identity() {
            if let ty::ClauseKind::Trait(data) = predicate.kind().skip_binder() {
                if set.insert(data.def_id()) {
                    stack.push(data.def_id());
                }
            }
        }

        Some(trait_def_id)
    })
}

pub fn supertraits<I: Interner>(
    cx: I,
    trait_ref: ty::Binder<I, ty::TraitRef<I>>,
) -> FilterToTraits<I, Elaborator<I, I::Clause>> {
    elaborate(cx, [trait_ref.upcast(cx)]).filter_only_self().filter_to_traits()
}

impl<I: Interner> Elaborator<I, I::Clause> {
    fn filter_to_traits(self) -> FilterToTraits<I, Self> {
        FilterToTraits { _cx: PhantomData, base_iterator: self }
    }
}

/// A filter around an iterator of predicates that makes it yield up
/// just trait references.
pub struct FilterToTraits<I: Interner, It: Iterator<Item = I::Clause>> {
    _cx: PhantomData<I>,
    base_iterator: It,
}

impl<I: Interner, It: Iterator<Item = I::Clause>> Iterator for FilterToTraits<I, It> {
    type Item = ty::Binder<I, ty::TraitRef<I>>;

    fn next(&mut self) -> Option<ty::Binder<I, ty::TraitRef<I>>> {
        while let Some(pred) = self.base_iterator.next() {
            if let Some(data) = pred.as_trait_clause() {
                return Some(data.map_bound(|t| t.trait_ref));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.base_iterator.size_hint();
        (0, upper)
    }
}
