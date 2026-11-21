//~ E8512
// #375: a whole-struct comparison checks every literal field against its
// field's type, as a whole-struct write does (ADR-0059, ADR-0077). 17
// does not fit the u4 field `a`; before the fix the testbench masked it to
// 1 and the assertion passed for hardware that holds a = 1.

struct P {
    a : u4
    b : bool
}

module M {
    in  clk : clock
    in  x   : P
    out q   : P

    reg r : P = P { a: 0, b: false }
    on clk { r <= x }
    q = r
}

test "masked literal" {
    let dut = M { };
    dut.x = P { a: 1, b: true };
    step(1);
    assert_eq(dut.q, P { a: 17, b: true });
    //~^ ERROR value does not fit in port width
}
