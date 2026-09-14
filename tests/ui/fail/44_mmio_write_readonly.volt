//~ E4006
// Writing a bus-owned (non-volatile) @reg field from RTL (ADR-0044):
// 'input' is ReadOnly but not 'volatile', so the generated bus logic is
// its only writer; the hardware-side assignment is a second driver.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module Gpio {
    in  clk     : clock
    in  pins_in : u8
    out pins_out : u8

    @reg(offset = 0x00, access = ReadWrite)
    output : {
        pins : u8,
        @reserved : bits<24>,
    }

    @reg(offset = 0x08, access = ReadOnly)
    input : {
        pins : u8,
        @reserved : bits<24>,
    }

    on clk {
        regs.input.pins <= pins_in
        //~^ ERROR E4006
    }
    pins_out = regs.output.pins
}
