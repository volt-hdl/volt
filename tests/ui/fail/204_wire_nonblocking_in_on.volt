//~ E0020
// A wire written with '<=' in an 'on' block. It used to become a flip-flop
// with an empty reset branch: after reset `w` kept whatever value it had.

module WireNonblockingInOn {
    in  clk : clock
    in  d   : u8
    out q   : u8

    wire w : u8
    on clk { w <= d }
    //~^ ERROR wire 'w' cannot be assigned with '<=' in an 'on' block
    q = w
}
