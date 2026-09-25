//~ E1014
// ADR-0079: 'GpioRegs' and 'GPIORegs' both write build/sw/gpio_regs.*;
// the second driver would silently overwrite the first.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module GpioRegs {
    in  clk : clock
    out y   : bool
    @reg(offset = 0x00, access = ReadWrite)
    a : { b : bool, @reserved : bits<31> }
    y = regs.a.b
}

@mmio(base = 0x4001_0000, bus = AXI4Lite)
module GPIORegs {
//~^ ERROR both write the driver files build/sw/gpio_regs
    in  clk : clock
    out z   : bool
    @reg(offset = 0x00, access = ReadWrite)
    c : { d : bool, @reserved : bits<31> }
    z = regs.c.d
}
