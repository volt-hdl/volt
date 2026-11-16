//~ PASS
// #144: arithmetic of literals only was written with each literal cut to
// the width of the context and the operation left to the SystemVerilog:
// in a u2 context `7 / 3` became `2'h3 / 2'd3` = 1 and `5 % 3` became
// `2'h1 % 2'd3` = 1. type-inference.md §3.3 (two literals give a literal)
// and §4 (an unsuffixed literal takes the expected type, E2010 if it does
// not fit), const-eval.md §1 (exact i128 value, fit checked afterwards)
// and §4 (`checked_div`, `checked_rem`, toward zero): the value is the
// exact one. Fixed in PR #151.
module M {
    out q : u2
    out r : u2
    q = 7 / 3
    r = 5 % 3
}

test "literal division and remainder keep their exact value" {
    let dut = M { };
    step(1);
    // 7 / 3 = 2 (toward zero), fits u2 (0..=3). Before the fix: 3 / 3 = 1.
    assert_eq(dut.q, 2);
    // 5 % 3 = 2, fits u2. Before the fix: 1 % 3 = 1.
    assert_eq(dut.r, 2);
}
