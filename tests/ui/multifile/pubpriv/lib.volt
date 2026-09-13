package lib;

pub module Ticker {
    in  clk    : clock
    in  enable : bool
    out count  : u8
    reg r : u8 = 0
    on clk { if enable { r <= r + 1 } }
    count = r
}

// Not `pub`: importing it from another file is E1004.
module Hidden {
    in  clk : clock
    in  a   : bool
    out b   : bool
    b = a
}
