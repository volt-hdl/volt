//~ PASS
// #471: a generic `const S : Trit` parameter could not be given -1, one of
// the three values of a Trit (ADR-0003). `Scaled<-1>`, `Scaled<(-1)>` and
// `Scaled<0 - 1>` got E0010 ("comparison operators cannot be chained"):
// the parser's look-ahead for a generic instance accepted names, literals
// and commas between `<` and `>`, so a minus or a parenthesis sent the
// statement to the expression parser, which read `Scaled < -1 > { ... }`
// as a chain of comparisons. A named constant (`Scaled<NEG>`) got E2008:
// the monomorphization accepted only a literal argument. The same held for
// a signed parameter (`P<-1>` with `const K : i8`).
// Right answer (grammar-full.ebnf: `GenericArg = Type | Expr`; ADR-0041:
// the argument is a compile-time constant): each of the four forms gives
// S = -1, so they name one module. Fixed in the pull request that closes
// #471.

const NEG : Trit = -1

module Scaled<const S : Trit> {
    in  x : i8
    out y : i8
    y = S * x
}

module P<const K : i8> {
    out y : i8
    y = K
}

module Top {
    in  x  : i8
    out y1 : i8
    out y2 : i8
    out y3 : i8
    out y4 : i8
    out k  : i8

    let s1 = Scaled<-1> { x: x }
    let s2 = Scaled<(-1)> { x: x }
    let s3 = Scaled<0 - 1> { x: x }
    let s4 = Scaled<NEG> { x: x }
    let p = P<-1> { }
    y1 = s1.y
    y2 = s2.y
    y3 = s3.y
    y4 = s4.y
    k = p.y
}

// Values by hand (type-inference.md sec. 3.3: S = -1 selects -x; an `out`
// port reads back the bit pattern, -5 in i8 is 251, -1 is 255).
test "every form of -1 selects -x" {
    let dut = Top { };
    dut.x = 5;
    step(1);
    assert_eq(dut.y1, 251);
    assert_eq(dut.y2, 251);
    assert_eq(dut.y3, 251);
    assert_eq(dut.y4, 251);
    // K = -1 is the literal -1 of i8.
    assert_eq(dut.k, 255);
}

test "S = -1 and x = -128 wrap in i8" {
    let dut = Top { };
    dut.x = 128;
    step(1);
    // -(-128) = +128 does not fit in i8 and wraps to -128 (ADR-0145
    // Decision 8; the target has the width of the product).
    assert_eq(dut.y1, 128);
    assert_eq(dut.y4, 128);
}
