//~ E1013
// ADR-0078: an @mmio field is a Rust method and parameter name in the
// generated driver ('pub fn set_ctrl_mod(&mut self, mod: bool)').
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module KwRegs {
    in  clk : clock
    out y   : bool

    @reg(offset = 0x00, access = ReadWrite)
    ctrl : { mod : bool, enable : bool, @reserved : bits<30> }
//~^ ERROR 'mod' is a Rust keyword
    y = regs.ctrl.mod && regs.ctrl.enable
}
