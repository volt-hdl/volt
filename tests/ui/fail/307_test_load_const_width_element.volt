//~ E8510
// #376: `load` checks every value against the element width of the
// memory (ADR-0058). The element type `uint<W>` with `const W = 2 + 2` is
// 4 bits; 31 does not fit. Before the fix the width was unknown and the
// value was loaded unchecked.

const W : u32 = 2 + 2

module M {
    in  clk  : clock
    in  addr : u2
    out q    : uint<W>

    reg mem : [uint<W>; 4] = [0; 4]
    on clk { }
    q = mem[addr]
}

test "load wide" {
    let dut = M { };
    let v = [1, 31, 2, 3];
    load(dut.mem, v);
    //~^ ERROR value does not fit
    dut.addr = 1;
    step(1);
    assert_eq(dut.q, 15);
}
