//~ E8512
// A test writes plain integers to ports and the simulator does not mask
// them (ADR-0059): 8 in a u3 port leaves a stray bit in the model, and
// the lookup below would silently read the wrong entry.

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

test "address past the table" {
    let dut = Table { };
    dut.addr = 8;
    //~^ ERROR value does not fit in port width
    step(1);
    assert_eq(dut.data, 0);
}
