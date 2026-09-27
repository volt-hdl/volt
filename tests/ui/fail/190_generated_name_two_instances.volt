//~ E1003
// ADR-0090: two Volt-made names meet — 'a_b' + 'c' and 'a' + 'b_c' both
// become 'a_b_c'; the message names both instance outputs.

module A {
    out c : bool
    c = true
}

module B {
    out b_c : bool
    b_c = false
}

module Pair {
    out o : bool

    let a_b = A { }
    let a = B { }
//~^ ERROR 'a_b_c' is both output 'c' of instance 'a_b' and output 'b_c' of instance 'a'
    o = a_b.c ^ a.b_c
}
