//~ E0018
// ADR-0080: 'a ^ a ^ ...' is a tree one level deeper per operator; 300 links
// exceed the 256-level limit. Every later compiler stage walks the tree
// recursively, so the parser cuts it here instead of the process aborting.

module TooDeep {
    in  a : u8
    out y : u8
    y = a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a ^ a
//~^ ERROR nesting is too deep
}
