//~ E0015
// Two @reg registers at the same offset (ADR-0044): the address decoder
// must select exactly one register per word address.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module Regs {
    in  clk : clock
    out v   : u8

    @reg(offset = 0x00, access = ReadWrite)
    a : {
        value : u8,
        @reserved : bits<24>,
    }

    @reg(offset = 0x00, access = ReadOnly, volatile)
    b : {
    //~^ ERROR E0015
        value : u8,
        @reserved : bits<24>,
    }

    on clk {
        regs.b.value <= regs.a.value
    }
    v = regs.b.value
}
