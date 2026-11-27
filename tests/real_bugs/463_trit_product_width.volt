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
//
// The same family, found by the large differential run on the pull request
// (#458's generator): a Trit product as the operand of an operation was
// computed at the operation's width. ADR-0145 Decision 8: `t * x` is iN;
// as the operand of an operation (as, + and -, a bitwise operation, a
// comparison, a selection) it wraps in iN, so t = -1, x = iN::MIN gives
// iN::MIN. At the top of a wider target it is pushed to the target and
// computed at the target width (ADR-0041, ADR-0062 Decision 2).

module TritProductWidth {
    in  tp : bool
    in  tn : bool
    in  up : bool
    in  un : bool
    in  x  : i14
    in  y  : i17
    in  w  : i8
    in  v  : i13
    in  sel : bool
    in  z  : i2
    out o1 : i14
    out o2 : bits<17>
    out o3 : i12
    out o4 : i12
    out o5 : i14
    out o6 : i12
    out o7 : i12

    let t : Trit = if tp { 1 } else if tn { -1 } else { 0 }
    let u : Trit = if up { 1 } else if un { -1 } else { 0 }

    let s = t * x * u
    o1 = s
    o2 = (-(t * y))[17:1]
    o3 = t * w
    o4 = (t * w) as i12
    o5 = -u * v + v
    o6 = if sel { t * w } else { 0 }
    o7 = (t * z) as i12
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

// ADR-0145 Decision 8, both sides of the line. t = -1, w = -128 (pattern
// 128), u = +1, v = -4096 (pattern 4096, i13::MIN), z = -2 (pattern 2,
// i2::MIN). On main, o7 fails: the emitter isolated a product whose two
// sides differ in width (o4, o6), and the two sides of an i2 product do
// not. o5 failed while the emitter read the product's width from the type
// checker's entry for the product: as the operand of a pushed `+`, that
// entry is the target type (ADR-0041); the width now comes from the iN
// side.
test "a Trit product wraps in its own width as an operand" {
    let dut = TritProductWidth { };
    dut.tp = false;
    dut.tn = true;
    dut.up = true;
    dut.un = false;
    dut.w = 128;
    dut.v = 4096;
    dut.sel = true;
    step(1);
    // 3. At the top of the i12 target the product is pushed and computed
    //    at 12 bits: -1 * -128 = +128.
    assert_eq(dut.o3, 128);
    // 4. As the operand of `as` it is computed in i8: -1 * -128 wraps to
    //    -128; widened by its sign, 0xf80 = 3968.
    assert_eq(dut.o4, 3968);
    // 5. -u = -1; -u * v wraps in i13 to -4096; -4096 + -4096 = -8192 in
    //    i14, the pattern 0x2000 = 8192. Computed at 14 bits it would be
    //    +4096 - 4096 = 0.
    assert_eq(dut.o5, 8192);
    // 6. As a selected arm it is computed in i8: -128, 0xf80 = 3968.
    assert_eq(dut.o6, 3968);
}

test "an i2 Trit product wraps in i2 under a cast" {
    let dut = TritProductWidth { };
    dut.tp = false;
    dut.tn = true;
    dut.z = 2;
    step(1);
    // 7. -1 * -2 = +2 does not fit i2 and wraps to -2; widened by its
    //    sign, 0xffe = 4094. On main the product is computed at 12 bits:
    //    +2.
    assert_eq(dut.o7, 4094);
}
