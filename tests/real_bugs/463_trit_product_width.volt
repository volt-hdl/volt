//~ PASS
// #463: the SV emitter's width range of an expression (volt-sv-emit
// let_width.rs `int_range`) treats a Trit as a two-bit integer: `-t` gets
// the range of i3 and a Trit product gets a wrong range or none. Two
// silent-wrong results follow (found by the differential test, #458):
// 1. the untyped `let s = t * x * u` (x : i14) is declared
//    `wire signed [3:0]`: the value is cut to four bits;
// 2. `-(t * y)` (y : i17) is computed at 17 bits instead of 18.
// Right answer (type-inference.md sec. 3.3, 3.4, 3.5): `t * x` (t : Trit,
// x : iN) is iN, so `t * x * u` is i14 and the `let` carries the value;
// `-z` (z : i17) is i18 and `z[h:l]` selects bits of that value. Before
// the fix `volt test` stopped at Verilator's WIDTHEXPAND and WIDTHTRUNC
// on the four-bit wire. Fixed in the pull request that closes #463: the
// emitter reads the width of `-t`, of a Trit product and of `t + u` from
// the type checker (ADR-0145).

module TritProductWidth {
    in  tp : bool
    in  tn : bool
    in  up : bool
    in  un : bool
    in  x  : i14
    in  y  : i17
    out o1 : i14
    out o2 : bits<17>

    let t : Trit = if tp { 1 } else if tn { -1 } else { 0 }
    let u : Trit = if up { 1 } else if un { -1 } else { 0 }

    let s = t * x * u
    o1 = s
    o2 = (-(t * y))[17:1]
}

// Values by hand. t = +1 and u = +1 select x (sec. 3.3, ADR-0062: the
// ternary MAC is x, -x or 0); an `out` port reads back its bit pattern.
test "a Trit product keeps its width in a let and under a minus" {
    let dut = TritProductWidth { };
    dut.tp = true;
    dut.tn = false;
    dut.up = true;
    dut.un = false;
    dut.x = 8192;
    dut.y = 1;
    step(1);
    // 1. t * x * u = (+1) * 8192 * (+1) = 8192 = 0x2000, an i14 value.
    //    The four-bit wire keeps the low four bits, 0.
    assert_eq(dut.o1, 8192);
    // 2. t * y = 1 (i17); -(t * y) = -1 in i18, the pattern 0x3ffff;
    //    bits 17..1 are 0x1ffff = 131071. Negated at 17 bits, -1 is
    //    0x1ffff and the select reads 0x0ffff = 65535.
    assert_eq(dut.o2, 131071);
}

test "a zero Trit gives zero" {
    let dut = TritProductWidth { };
    dut.tp = false;
    dut.tn = false;
    dut.up = true;
    dut.un = false;
    dut.x = 8192;
    dut.y = 1;
    step(1);
    // t = 0: t * x * u = 0 and -(t * y) = 0 (sec. 3.3, 3.4).
    assert_eq(dut.o1, 0);
    assert_eq(dut.o2, 0);
}
