//~ E0015
// #206: the @mmio desugar runs after the whole file is parsed. A W0010
// warning in the last statement of the file used to open the cascade-
// suppression window at the end of the file, so every E0015 of the
// desugar was swallowed: the 40-bit register and the unaligned register
// disappeared from the RTL and the drivers. Both are E0015 (ADR-0044:
// unsupported field type; unaligned offset), each reported on its own.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    in  c : u8
    out o : u8

    @reg(offset = 0x00, access = ReadWrite)
    ctrl : {
        en : bool,
        @reserved : bits<31>,
    }

    @reg(offset = 0x04, access = ReadWrite)
    wide : {
        v : bits<40>,
        //~^ ERROR register field type must be bool, bits<N> or uN (N <= 32)
    }

    @reg(offset = 0x0A, access = ReadWrite)
    //~^ ERROR offset 0xa of register 'odd' is not a 4-byte aligned 32-bit address
    odd : {
        v : bits<8>,
    }

    o = a & b | c
}
