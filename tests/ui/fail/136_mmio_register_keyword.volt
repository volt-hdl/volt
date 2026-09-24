//~ E1013
// ADR-0078: a single-field register is a Rust getter named after the
// register ('pub fn loop(&self)').
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module KwRegs {
    in  clk : clock
    out y   : u8

    @reg(offset = 0x00, access = ReadWrite)
    loop : { value : u8, @reserved : bits<24> }
//~^ ERROR 'loop' is a Rust keyword
    y = regs.loop.value
}
