//~ W2014
// #216: in a statement match, an arm after the wildcard was emitted after
// `default` and won, because SystemVerilog takes `default` only when no
// other item matches. The first matching arm wins (ADR-0032, ADR-0083: the
// statement and the expression form follow the same rules); arms after the
// first wildcard are not emitted and get W2014 (sv-mapping.md §19.1). The
// expression form `y = match s { 0 => 1, _ => 7, 1 => 9 }` gave 7 for
// s = 1, this statement form 9, and no form gave W2014. Fixed by #326.
module M {
    in  s : u2
    out y : u8
    comb {
        match s {
            0 => { y = 1 }
            _ => { y = 7 }
            1 => { y = 9 }
        }
    }
}

test "the first matching arm wins" {
    let dut = M { };
    // s = 0: the arm `0` is the first that matches, y = 1.
    dut.s = 0;
    step(1);
    assert_eq(dut.y, 1);
    // s = 1: `0` does not match, `_` matches every value and comes first,
    // y = 7 (the arm `1` after it is unreachable).
    dut.s = 1;
    step(1);
    assert_eq(dut.y, 7);
}
