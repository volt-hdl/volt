//~ E1013
// ADR-0078: an @mmio field is a parameter name in the generated C header
// ('void kw_regs_set_ctrl_default(uint8_t default)').
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module KwRegs {
    in  clk : clock
    out y   : u4

    @reg(offset = 0x00, access = ReadWrite)
    ctrl : { default : u4, @reserved : bits<28> }
//~^ ERROR 'default' is a C/C++ keyword
    y = regs.ctrl.default
}
