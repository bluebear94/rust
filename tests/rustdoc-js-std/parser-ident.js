const PARSED = [
    {
        query: "R<!>",
        elems: [{
            name: "R",
            fullPath: ["r"],
            pathWithoutLast: [],
            pathLast: "r",
            generics: [
                {
                    name: "never",
                    fullPath: ["never"],
                    pathWithoutLast: [],
                    pathLast: "never",
                    generics: [],
                    typeFilter: 1,
                },
            ],
            typeFilter: -1,
        }],
        foundElems: 1,
        userQuery: "R<!>",
        returned: [],
        error: null,
    },
    {
        query: "!",
        elems: [{
            name: "never",
            fullPath: ["never"],
            pathWithoutLast: [],
            pathLast: "never",
            generics: [],
            typeFilter: 1,
        }],
        foundElems: 1,
        userQuery: "!",
        returned: [],
        error: null,
    },
    {
        query: "a!",
        elems: [{
            name: "a",
            fullPath: ["a"],
            pathWithoutLast: [],
            pathLast: "a",
            generics: [],
            typeFilter: 16,
        }],
        foundElems: 1,
        userQuery: "a!",
        returned: [],
        error: null,
    },
    {
        query: "a!::b",
        elems: [],
        foundElems: 0,
        userQuery: "a!::b",
        returned: [],
        error: "Cannot have associated items in macros",
    },
    {
        query: "!<T>",
        elems: [],
        foundElems: 0,
        userQuery: "!<T>",
        returned: [],
        error: "Never type `!` does not accept generic parameters",
    },
    {
        query: "!::b",
        elems: [{
            name: "!::b",
            fullPath: ["never", "b"],
            pathWithoutLast: ["never"],
            pathLast: "b",
            generics: [],
            typeFilter: -1,
        }],
        foundElems: 1,
        userQuery: "!::b",
        returned: [],
        error: null,
    },
    {
        query: "b::!",
        elems: [],
        foundElems: 0,
        userQuery: "b::!",
        returned: [],
        error: "Never type `!` is not associated item",
    },
    {
        query: "!::!",
        elems: [],
        foundElems: 0,
        userQuery: "!::!",
        returned: [],
        error: "Never type `!` is not associated item",
    },
    {
        query: "b::!::c",
        elems: [],
        foundElems: 0,
        userQuery: "b::!::c",
        returned: [],
        error: "Never type `!` is not associated item",
    },
    {
        query: "!::b<T>",
        elems: [{
            name: "!::b",
            fullPath: ["never", "b"],
            pathWithoutLast: ["never"],
            pathLast: "b",
            generics: [
                {
                    name: "T",
                    fullPath: ["t"],
                    pathWithoutLast: [],
                    pathLast: "t",
                    generics: [],
                    typeFilter: -1,
                }
            ],
            typeFilter: -1,
        }],
        foundElems: 1,
        userQuery: "!::b<T>",
        returned: [],
        error: null,
    },
    {
        query: "a!::b!",
        elems: [],
        foundElems: 0,
        userQuery: "a!::b!",
        returned: [],
        error: "Cannot have associated items in macros",
    },
];
