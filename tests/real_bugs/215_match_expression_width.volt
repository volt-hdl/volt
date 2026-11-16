//~ PASS
// #215 (open): a match expression is emitted at a narrower width than the
// type checker gives it. type-inference.md §3.9 (ADR-0083): "a match
// expression is a multi-arm if expression (§3.7): its width behaviour does
// not diverge". `a + b` and `a - b` of two u8 are 9 bits wide (ADR-0025),
// so `m` is 9 bits like `n`; the generated SV declares `logic [7:0] m` and
// cuts the sum: y = 44 for a = 200, b = 100. The `if` form `n` is right.
// The metamorphic rewrites `match-to-if` and `if-to-match` exchange the
// two forms. Known failure until #215 is fixed.
module M {
    in  a : u8
    in  b : u8
    in  c : bool
    out y : u9
    out z : u9
    let m = match c { true => a + b, _ => a - b }
    let n = if c { a + b } else { a - b }
    y = m
    z = n
}

test "a match expression keeps the width of its arms" {
    let dut = M { };
    // c = true: both forms take the first arm, a + b = 200 + 100 = 300,
    // which fits the 9 bits of the sum and of y and z.
    dut.a = 200;
    dut.b = 100;
    dut.c = true;
    step(1);
    assert_eq(dut.y, 300);
    assert_eq(dut.z, 300);
    // c = false: the `_` arm, a - b = 200 - 100 = 100.
    dut.c = false;
    step(1);
    assert_eq(dut.y, 100);
    assert_eq(dut.z, 100);
}
