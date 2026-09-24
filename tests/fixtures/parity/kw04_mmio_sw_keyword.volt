// parity: E1013
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module KwRegs {
    in  clk : clock
    out y   : bool
    @reg(offset = 0x00, access = ReadWrite)
    ctrl : { mod : bool, @reserved : bits<31> }
    y = regs.ctrl.mod
}
