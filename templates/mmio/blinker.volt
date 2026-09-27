// A memory-mapped LED blinker: an AXI4-Lite slave generated from a
// register map.
//
// @mmio turns the @reg declarations into the bus adapter, the address
// decode, the read multiplexer and their contracts; only the LED logic
// below is written by hand. The same map generates the software side:
//
//   volt build blinker.volt --emit=c,rust,regmap,regmap-md
//
// writes build/sw/blinker.h (C), build/sw/blinker.rs (Rust),
// build/sw/blinker.json and build/docs/blinker.md — the firmware never
// restates an offset, a width or an access right by hand.

// control.mode value that makes the LED blink.
const MODE_BLINK : u2 = 2

/// LED blinker with a programmable period.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
pub module Blinker {
    in  clk : clock
    out led : bool

    /// Mode: 0 = off, 1 = on, 2 = blink (3 behaves as off).
    @reg(offset = 0x00, access = ReadWrite)
    control : { mode : u2, @reserved : bits<30> }

    /// Blink half-period in clock cycles; 0 behaves as 1.
    @reg(offset = 0x04, access = ReadWrite)
    period : { cycles : u16, @reserved : bits<16> }

    /// Current LED level and the number of blink toggles so far.
    @reg(offset = 0x08, access = ReadOnly, volatile)
    status : { led : bool, @reserved : bits<15>, toggles : u16 }

    reg timer_r : u16  = 0
    reg phase_r : bool = false

    // The LED level for each mode. Match arms take literal patterns
    // (a name in a pattern would bind a new variable, not compare).
    let led_level : bool = match regs.control.mode {
        1 => true,       // on
        2 => phase_r,    // blink
        _ => false,      // off (0 and the unused 3)
    }

    on clk {
        if regs.control.mode != MODE_BLINK {
            timer_r <= 0
            phase_r <= false
        } else if timer_r + 1 >= regs.period.cycles {
            timer_r <= 0
            phase_r <= !phase_r
            regs.status.toggles <= regs.status.toggles + 1
        } else {
            timer_r <= timer_r + 1
        }
        regs.status.led <= led_level
    }

    cover: phase_r
    cover: regs.status.toggles == 3

    led = led_level
}
