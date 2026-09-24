//~ E8513
// ADR-0078: 'volt test' makes Probe the Verilator top; its C++ class
// VProbe already has a member 'name()', so the model would not compile.
module Probe {
    in  clk  : clock
    in  d    : u8
    out name : u8
//~^ ERROR collides with the C++ class Verilator generates
    reg r : u8 = 0
    on clk { r <= d }
    name = r
}

test "probe" {
    let dut = Probe { };
    dut.d = 1;
    step(1);
    assert_eq(dut.name, 1);
}
