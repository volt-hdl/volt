//~ W1002
// #130: monomorphization (ADR-0041) replaced every single-segment path `N`
// in the body of `G` with the generic argument's literal, also the `N` that
// a block `let` declares again: `G<3>` wrote `y = 8'd3` and the let was
// reported unused (W1001). A block scope sits inside the module scope and an
// inner declaration shadows the outer one (name-resolution.md §2, §6): `y = N`
// reads the local `N`, which is `s`; the shadowing is W1002. Fixed by #142
// (the parser's substitution goes through the scope chain, ADR-0081
// appendix).
module G<const N: u32> {
    in  s : u8
    out y : u8
    comb {
        let N = s
        y = N
    }
}

module Top {
    in  s : u8
    out y : u8
    let g = G<3> { s: s }
    y = g.y
}

test "the block let shadows the generic parameter" {
    let dut = Top { };
    dut.s = 42;
    step(1);
    // y = N, N the block let = s = 42 (the bug gave the argument 3).
    assert_eq(dut.y, 42);
    dut.s = 7;
    step(1);
    // y = s = 7.
    assert_eq(dut.y, 7);
}
