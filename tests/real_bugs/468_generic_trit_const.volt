//~ PASS
// #468: the parser turns a generic `const S : Trit` parameter into its
// literal (ADR-0041) and the type checker did not see that S is a Trit.
// `S * x` (x : iN) was typed by the rule of an integer product (literal
// times iN, i(2N)), not by the Trit rule, and its value left the spec in
// the A15 corner: in `Weighted<1>` with z : i2 = -2, `(-S * z) as i12` gave
// +2 where a Trit signal t = -1 gives -2.
// Right answer (type-inference.md sec. 3.3, ADR-0145 Decision 8): `t * x`
// (t : Trit, x : iN; `-t * x` included) is iN. As the operand of an
// operation (as, + and -, a comparison) it wraps in iN: t = -1, x = iN::MIN
// gives iN::MIN. At the top of a wider target it is pushed to the target
// (ADR-0041). The maintainer's decision on #468: a generic `const S :
// Trit` stays a Trit in the type checker, so every expression with S gets
// the type and the value a Trit signal would get in its place. Fixed in
// the pull request that closes #468 (with #471, which makes `Weighted<-1>`
// writable).

module Weighted<const S : Trit> {
    in  x : i8
    in  z : i2
    out a : i12
    out b : i12
    out c : i12
    out d : i12
    out e : i9
    out f : bool
    out g : i16
    out h : i12

    a = (S * x) as i12
    b = (-S * x) as i12
    c = (S * z) as i12
    d = (-S * z) as i12
    e = -S * x + x
    f = (-S * x) < 0
    let q = -S * x
    g = q
    h = -S * x
}

module Top {
    in  x  : i8
    in  z  : i2
    out pa : i12
    out pb : i12
    out pc : i12
    out pd : i12
    out pe : i9
    out pf : bool
    out pg : i16
    out ph : i12
    out na : i12
    out nb : i12
    out nc : i12
    out nd : i12
    out ne : i9
    out nf : bool
    out ng : i16
    out nh : i12

    let p = Weighted<1> { x: x, z: z }
    let n = Weighted<-1> { x: x, z: z }
    pa = p.a
    pb = p.b
    pc = p.c
    pd = p.d
    pe = p.e
    pf = p.f
    pg = p.g
    ph = p.h
    na = n.a
    nb = n.b
    nc = n.c
    nd = n.d
    ne = n.e
    nf = n.f
    ng = n.g
    nh = n.h
}

// Values by hand from sec. 3.3 (ternary MAC: +1 -> x, -1 -> -x, 0 -> 0).
// An `in` port takes and an `out` port reads back the bit pattern: x =
// -128 is 128, z = -2 is 2; -128 in i12 is 3968, -2 is 4094, in i16 -128
// is 65408, -256 in i9 is 256.
test "the A15 corner: a generic Trit product wraps like a Trit signal" {
    let dut = Top { };
    dut.x = 128;
    dut.z = 2;
    step(1);
    // S = +1. S * x = x = -128.
    assert_eq(dut.pa, 3968);
    // -S = -1; -x = +128 does not fit in i8 and wraps to -128 (the
    // operand of `as`). The integer rule gave +128.
    assert_eq(dut.pb, 3968);
    // S * z = z = -2.
    assert_eq(dut.pc, 4094);
    // -z = +2 does not fit in i2 and wraps to -2. The integer rule gave
    // +2, the value measured in #468.
    assert_eq(dut.pd, 4094);
    // The operand of `+` wraps: -128 + (-128) = -256 in i9.
    assert_eq(dut.pe, 256);
    // The operand of `<` wraps: -128 < 0.
    assert_true(dut.pf);
    // An untyped let holds the i8 product, -128, widened to i16.
    assert_eq(dut.pg, 65408);
    // At the top of the i12 target the product is pushed: +128.
    assert_eq(dut.ph, 128);
    // S = -1. S * x = -x wraps to -128.
    assert_eq(dut.na, 3968);
    // -S = +1: x = -128.
    assert_eq(dut.nb, 3968);
    // S * z = -z wraps to -2.
    assert_eq(dut.nc, 4094);
    // -S * z = z = -2.
    assert_eq(dut.nd, 4094);
    // -128 + (-128) = -256.
    assert_eq(dut.ne, 256);
    assert_true(dut.nf);
    assert_eq(dut.ng, 65408);
    // -S * x = x = -128, nothing to push.
    assert_eq(dut.nh, 3968);
}

test "away from the corner the sign follows S" {
    let dut = Top { };
    dut.x = 5;
    dut.z = 1;
    step(1);
    // S = +1: S * x = 5, -S * x = -5 (4091 in i12), S * z = 1, -S * z = -1
    // (4095), -5 + 5 = 0, -5 < 0, -5 in i16 is 65531, -5 in i12.
    assert_eq(dut.pa, 5);
    assert_eq(dut.pb, 4091);
    assert_eq(dut.pc, 1);
    assert_eq(dut.pd, 4095);
    assert_eq(dut.pe, 0);
    assert_true(dut.pf);
    assert_eq(dut.pg, 65531);
    assert_eq(dut.ph, 4091);
    // S = -1: S * x = -5, -S * x = 5, S * z = -1, -S * z = 1, 5 + 5 = 10,
    // 5 < 0 is false.
    assert_eq(dut.na, 4091);
    assert_eq(dut.nb, 5);
    assert_eq(dut.nc, 4095);
    assert_eq(dut.nd, 1);
    assert_eq(dut.ne, 10);
    assert_false(dut.nf);
    assert_eq(dut.ng, 5);
    assert_eq(dut.nh, 5);
}
