// parity: E0003
enum Mode { Off, On, Auto }

@mmio(base = 0x4000_0000, bus = AXI4Lite)
module Regs {
    @reg(offset = 0x00, access = ReadWrite)
    ctrl : { mode : Mode, @reserved : bits<30> }
}
