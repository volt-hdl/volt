//~ E8512
// #376: a port width given by a const expression (`2 + 2`) is known to the
// test language. A u4-wide port can never read 300, so `assert_ne` would
// pass on any hardware (ADR-0059 §5); before the fix the width was unknown
// and the comparison was accepted.

const W : u32 = 2 + 2

module M {
    in  clk : clock
    in  a   : uint<W>
    out q   : uint<W>

    reg r : uint<W> = 0
    on clk { r <= a }
    q = r
}

test "never matches" {
    let dut = M { };
    dut.a = 5;
    step(1);
    assert_ne(dut.q, 300);
    //~^ ERROR value does not fit in port width
}
