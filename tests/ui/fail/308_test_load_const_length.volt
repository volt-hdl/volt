//~ E8510
// #376: `load` checks the number of values against the length of the
// memory (ADR-0058). `[u8; N]` with `const N : u32 = 2 + 2` holds 4
// elements; 5 do not fit. Before the fix the length was unknown and the
// check was skipped.

const N : u32 = 2 + 2

module M {
    in  clk  : clock
    in  addr : u2
    out q    : u8

    reg mem : [u8; N] = [0; N]
    on clk { }
    q = mem[addr]
}

test "load long" {
    let dut = M { };
    let v = [1, 2, 3, 4, 5];
    load(dut.mem, v);
    //~^ ERROR do not fit
    dut.addr = 1;
    step(1);
    assert_eq(dut.q, 2);
}
