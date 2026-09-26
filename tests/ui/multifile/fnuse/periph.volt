package periph;

// A library file: a pub fn next to modules. A unit that only calls the
// fn does not reach these modules, so they are not emitted (ADR-0042
// addendum): no Blink.sv / GpioRegs.sv, no .sva, no SDC, no driver.
pub fn low_nibble(x: u8) -> u8 {
    x & 0x0F
}

pub module Blink {
    in  clk : clock
    out led : bool
    reg r : bool = false
    on clk { r <= !r }
    led = r
    invariant: r == r
}

@mmio(base = 0x4000_0000, bus = AXI4Lite)
module GpioRegs {
    in  clk      : clock
    out pins_out : bits<8>

    @reg(offset = 0x00, access = ReadWrite)
    output : {
        pins : bits<8>,
        @reserved : bits<24>,
    }

    pins_out = regs.output.pins
}
