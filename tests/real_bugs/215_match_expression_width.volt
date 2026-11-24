//~ PASS
// #215: a match expression was emitted at a narrower width than the type
// checker gives it. type-inference.md §3.9 (ADR-0083): "a match
// expression is a multi-arm if expression (§3.7): its width behaviour does
// not diverge". `a + b` and `a - b` of two u8 are 9 bits wide (ADR-0025),
// so `m` is 9 bits like `n`; the generated SV declared `logic [7:0] m` and
// cut the sum: y = 44 for a = 200, b = 100. The `if` form `n` was right.
// The scrutinee had the same cut: `match a + b` was a `case` at 8 bits
// with the pattern 300 printed as `8'h2C`, the label of 44. The type
// checker synthesizes the scrutinee and checks the patterns against its
// natural width (u9: 300 fits, 600 is E2010), so the 300 arm is the one
// taken; the `if` form `j` compares at 9 bits (ADR-0106). The metamorphic
// rewrites `match-to-if` and `if-to-match` exchange the two forms. Fixed
// by the pull request that closes #215: the emitter reads the arms of a
// match expression with the rule of `if` (`value_arms`) and the
// scrutinee at its natural width.
module M {
    in  a : u8
    in  b : u8
    in  c : bool
    out y : u9
    out z : u9
    out k : u2
    out j : u2
    let m = match c { true => a + b, _ => a - b }
    let n = if c { a + b } else { a - b }
    y = m
    z = n
    k = match a + b { 44 => 1, 300 => 2, _ => 3 }
    j = if a + b == 44 { 1 } else { if a + b == 300 { 2 } else { 3 } }
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

test "a match scrutinee keeps its natural width" {
    let dut = M { };
    // a + b = 300 at 9 bits: the arm `300` (k = 2), not `44` (300 mod 256).
    dut.a = 200;
    dut.b = 100;
    step(1);
    assert_eq(dut.k, 2);
    assert_eq(dut.j, 2);
    // a + b = 44: the arm `44` (k = 1).
    dut.a = 22;
    dut.b = 22;
    step(1);
    assert_eq(dut.k, 1);
    assert_eq(dut.j, 1);
    // a + b = 0: neither literal, the `_` arm (k = 3).
    dut.a = 0;
    dut.b = 0;
    step(1);
    assert_eq(dut.k, 3);
    assert_eq(dut.j, 3);
}
