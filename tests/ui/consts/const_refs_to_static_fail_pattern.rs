// normalize-stderr-test "(the raw bytes of the constant) \(size: [0-9]*, align: [0-9]*\)" -> "$1 (size: $$SIZE, align: $$ALIGN)"
// normalize-stderr-test "([0-9a-f][0-9a-f] |╾─*ALLOC[0-9]+(\+[a-z0-9]+)?(<imm>)?─*╼ )+ *│.*" -> "HEX_DUMP"
#![feature(const_refs_to_static)]

static mut S_MUT: i32 = 0;

const C: &i32 = unsafe { &S_MUT };
//~^ERROR: undefined behavior
//~| encountered reference to mutable memory

fn main() {
    // This *must not build*, the constant we are matching against
    // could change its value!
    match &42 {
        C => {}, //~ERROR: could not evaluate constant pattern
        _ => {},
    }
}
