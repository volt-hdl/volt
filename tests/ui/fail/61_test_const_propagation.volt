//~ E8512
// A `let` bound to a constant is a constant (ADR-0060): the test language
// has no `if` and no reassignment, so `n` is 8 wherever it is used. The
// port width check (ADR-0059) therefore fires at compile time instead of
// failing the test at run time.

module Table {
    in  clk  : clock
    in  addr : u3
    in  we   : bool
    out data : u8

    reg mem : [u8; 8] = [0; 8]

    on clk {
        if we {
            mem[addr] <= 1
        }
    }

    data = mem[addr]
}

test "address bound to a constant" {
    let dut = Table { };
    let n = 8;
    dut.addr = n;
    //~^ ERROR value does not fit in port width
    step(1);
    assert_eq(dut.data, 0);
}
