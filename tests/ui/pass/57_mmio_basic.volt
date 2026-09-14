// @mmio register map (ADR-0044): the @reg declarations of an @mmio module
// are expanded by the parser into an AXI4-Lite slave (bundle ports,
// ADR-0039), register storage, address decode, read mux, write logic
// and the automatic contracts of ADR-0044 §5. RTL reads bus-owned
// registers through 'regs.<reg>.<field>' and writes volatile ones.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module GpioRegs {
    in  clk      : clock
    in  pin_values : bits<8>
    out pins_out : bits<8>
    out enabled  : bool

    @reg(offset = 0x00, access = ReadWrite)
    direction : {
        pins : bits<8>,
        @reserved : bits<24>,
    }

    @reg(offset = 0x04, access = ReadWrite)
    output : {
        pins : bits<8>,
        @reserved : bits<24>,
    }

    @reg(offset = 0x08, access = ReadOnly, volatile)
    input : {
        pins : bits<8>,
        @reserved : bits<24>,
    }

    @reg(offset = 0x0C, access = ReadWrite)
    control : {
        enable : bool,
        reset  : bool @self_clearing,
        @reserved : bits<30>,
    }

    // User contracts sit next to the generated ones.
    invariant: regs.control.reset -> prev(aw_valid)

    on clk {
        regs.input.pins <= pin_values      // hardware-side write
    }
    let dir = regs.direction.pins
    let en  = regs.control.enable
    pins_out = regs.output.pins & dir
    enabled  = en
}
