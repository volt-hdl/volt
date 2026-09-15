// HW-SW bridge (ADR-0053): every @reg feature that reaches the generated
// drivers, with doc comments on the module, the registers and the fields.
// `volt build --emit=rust,c,regmap,regmap-md` turns this map into
// build/sw/pwm_regs.{rs,h,json} and build/docs/pwm_regs.md; the software
// side never restates an offset, a width or an access right by hand.

/// PWM channel controller.
/// One 16-bit counter compared against a duty threshold.
@mmio(base = 0x4001_0000, bus = AXI4Lite)
module PwmRegs {
    in  clk    : clock
    in  tick   : bool
    out pwm    : bool
    out active : bool

    /// Channel control.
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        /// 1 = counter runs.
        enable : bool,
        /// Write 1 to restart the counter from 0.
        restart : bool @self_clearing,
        /// Clock prescaler, 0 = every tick.
        prescale : u4,
        @reserved : bits<26>,
    }

    /// Duty threshold: pwm is high while count < duty.
    @reg(offset = 0x04, access = ReadWrite)
    duty : { value : u16, @reserved : bits<16> }

    /// Period reload value (write-only latch).
    @reg(offset = 0x08, access = WriteOnly)
    period : { value : u16, @reserved : bits<16> }

    /// Live counter state.
    @reg(offset = 0x0C, access = ReadOnly, volatile)
    status : {
        /// Current counter value.
        count : u16,
        /// Set when the counter wraps; write 1 to clear.
        wrapped : bool @w1c,
        @reserved : bits<15>,
    }

    /// Block identifier (constant).
    @reg(offset = 0x10, access = ReadOnly)
    id : { value : bits<32> }

    cover: regs.status.wrapped

    on clk {
        if regs.control.restart {
            regs.status.count <= 0
        } else if regs.control.enable && tick {
            if regs.status.count == regs.period.value {
                regs.status.count <= 0
                regs.status.wrapped <= true
            } else {
                regs.status.count <= regs.status.count + 1
            }
        }
    }
    pwm    = regs.control.enable && regs.status.count < regs.duty.value
    active = regs.control.enable
}
