//~ E1014
// ADR-0079: a single-field register is a getter named after the register;
// 'read' is the Rust driver's own private word reader.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module ReadRegs {
    in  clk : clock
    out y   : u8

    @reg(offset = 0x00, access = ReadWrite)
    read : { value : u8, @reserved : bits<24> }
//~^ ERROR generates the Rust identifier 'read'
    y = regs.read.value
}
