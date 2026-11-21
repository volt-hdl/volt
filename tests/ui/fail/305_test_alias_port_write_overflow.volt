//~ E8512
// #376: a port whose width comes from a type alias has that width in a
// test block too (ADR-0059). 31 does not fit the 4-bit port `a`; before
// the fix the test language did not know the width, the value reached the
// simulator unmasked and `q` read back 31, a value the hardware cannot hold.

type Nib = u4

module M {
    in  clk : clock
    in  a   : Nib
    out q   : Nib

    reg r : Nib = 0
    on clk { r <= a }
    q = r
}

test "alias write" {
    let dut = M { };
    dut.a = 31;
    //~^ ERROR value does not fit in port width
    step(1);
    assert_eq(dut.q, 15);
}
