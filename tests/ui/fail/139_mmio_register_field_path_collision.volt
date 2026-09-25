//~ E1014
// ADR-0079: 'irq.status_rx' and 'irq_status.rx' both become
// 'irq_status_rx' (IRQ_STATUS_RX_SHIFT, irq_status_rx(), ...).
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module IrqRegs {
    in  clk : clock
    out y   : bool

    @reg(offset = 0x00, access = ReadWrite)
    irq : { status_rx : bool, tx : bool, @reserved : bits<30> }
    @reg(offset = 0x04, access = ReadWrite)
    irq_status : { rx : bool, ov : bool, @reserved : bits<30> }
//~^ ERROR field 'irq_status.rx' and field 'irq.status_rx' both generate
    y = regs.irq.tx && regs.irq_status.ov
}
