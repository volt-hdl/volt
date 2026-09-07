// Multi-statement sequential block, nested conditional
module Accumulator {
    in  clk    : clock
    in  enable : bool
    in  clear  : bool
    in  data   : u8
    out sum    : u16
    out valid  : bool

    reg acc   : u16 = 0
    reg v     : bool = false

    on clk {
        if clear {
            acc <= 0
            v   <= false
        } else if enable {
            acc <= acc + (data as u16)
            v   <= true
        }
    }

    sum   = acc
    valid = v
}
