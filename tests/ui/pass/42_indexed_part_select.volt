// Indexed part-select (ADR-0035): data[i +: W] takes W bits starting
// at a variable index i; data[i -: W] takes W bits ending at i.
// W must be a compile-time constant; the result type is bits<W>.
module PartSelect {
    in  clk   : clock
    in  data  : u32
    in  i     : bits<5>
    in  bit_i : bits<3>
    out byte_up   : bits<8>
    out byte_down : bits<8>
    out one_bit   : bool
    out reg_byte  : u8

    reg acc : u8 = 0

    on clk {
        acc <= data[i +: 8] as u8
    }

    byte_up   = data[i +: 8]
    byte_down = data[31 -: 8]
    one_bit   = data[bit_i]
    reg_byte  = acc
}
