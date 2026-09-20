// Constant propagation in a test block (ADR-0060): constants that fit are
// written without a run-time check; values that are only known during the
// run (port reads, loop counters) keep the run-time check of ADR-0059.

const LAST : u8 = 7

module Table {
    in  clk  : clock
    in  addr : u3
    in  we   : bool
    out data : u8
    out echo : u3

    reg mem : [u8; 8] = [0; 8]

    on clk {
        if we {
            mem[addr] <= 1
        }
    }

    data = mem[addr]
    echo = addr
}

test "constant bindings fit the port" {
    let dut = Table { };
    let n = 7;
    dut.addr = n;
    let half = n / 2;
    dut.addr = half + 1;
    dut.addr = LAST;
    step(1);
    assert_eq(dut.echo, LAST);
}

test "run-time values stay run-time" {
    let dut = Table { };
    let seen = dut.echo;
    dut.addr = seen;
    for i in 0..8 {
        let next = i;
        dut.addr = next;
        step(1);
        assert_eq(dut.echo, i);
    }
}
