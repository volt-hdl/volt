//~ E1014
// ADR-0079: a field 'raw' of a multi-field register gets the accessor
// 'ctrl_raw', which is also the raw-word accessor of register 'ctrl'
// (the side finding of ADR-0078). Volt does not rename either.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module RawRegs {
    in  clk : clock
    out y   : bool

    @reg(offset = 0x00, access = ReadWrite)
    ctrl : { raw : u8, en : bool, @reserved : bits<23> }
//~^ ERROR field 'ctrl.raw' and register 'ctrl' both generate
    y = regs.ctrl.en
}
