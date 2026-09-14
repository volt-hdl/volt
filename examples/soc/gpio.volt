// 8-bit GPIO peripheral, AXI4-Lite slave generated from an @mmio register
// map (ADR-0044). The parser expands the @reg declarations into the bus
// adapter, address decode, read mux, write logic and contracts; only the
// pin logic is written by hand.
//
//   0x00 DATA_OUT  RW  value driven on pins configured as outputs
//   0x04 DIR       RW  1 = output enable per pin
//   0x08 DATA_IN   RO  pin inputs (volatile: the hardware writes it)
//   other          SLVERR
//
// The decoder hands over page-relative addresses, so base = 0.

package soc::gpio;

@mmio(base = 0x0000_0000, bus = AXI4Lite)
pub module Gpio {
    in  clk      : clock
    in  pins_in  : u8
    out pins_out : u8
    out pins_oe  : u8

    cover: pins_oe != 0

    @reg(offset = 0x00, access = ReadWrite)
    data_out : { pins : u8, @reserved : bits<24> }

    @reg(offset = 0x04, access = ReadWrite)
    dir : { pins : u8, @reserved : bits<24> }

    @reg(offset = 0x08, access = ReadOnly, volatile)
    data_in : { pins : u8, @reserved : bits<24> }

    on clk { regs.data_in.pins <= pins_in }
    pins_out = regs.data_out.pins
    pins_oe  = regs.dir.pins
}
