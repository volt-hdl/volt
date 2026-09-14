// @mmio access control (ADR-0044): ReadOnly, WriteOnly, volatile and
// @w1c registers. The bus cannot change a ReadOnly register, a
// WriteOnly register reads back as zero, a volatile register is owned
// by the hardware (written here with '<=') and a @w1c flag is set by
// the hardware and cleared when software writes a 1 to its bit.
@mmio(base = 0x0000_0100, bus = AXI4Lite)
module TimerRegs {
    in  clk    : clock
    in  tick   : bool
    out irq    : bool
    out count  : u16

    @reg(offset = 0x00, access = ReadWrite)
    ctrl : {
        enable : bool,
        clear  : bool @self_clearing,
        @reserved : bits<30>,
    }

    @reg(offset = 0x04, access = ReadOnly)
    id : {
        value : u16,
        @reserved : bits<16>,
    }

    @reg(offset = 0x08, access = WriteOnly)
    compare : {
        value : u16,
        @reserved : bits<16>,
    }

    @reg(offset = 0x0C, access = ReadOnly, volatile)
    status : {
        count   : u16,
        expired : bool @w1c,
        @reserved : bits<15>,
    }

    // User contracts see the register fields like any other signal.
    invariant: !regs.ctrl.enable -> regs.status.count == prev(regs.status.count) || regs.ctrl.clear || prev(regs.ctrl.clear)
    cover: regs.status.expired && !prev(regs.status.expired)

    on clk {
        if regs.ctrl.clear {
            regs.status.count <= 0
        } else if regs.ctrl.enable && tick {
            regs.status.count <= regs.status.count + 1
        }
        if regs.status.count == regs.compare.value && regs.ctrl.enable {
            regs.status.expired <= true
        }
    }
    irq   = regs.status.expired
    count = regs.status.count
}
