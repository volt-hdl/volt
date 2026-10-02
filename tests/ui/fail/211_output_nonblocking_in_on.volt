//~ E0020
// An output port written with '<=' in an 'on' block. It used to become a
// flip-flop with an empty reset branch: after reset `q` kept whatever value
// it had. A port declaration has no reset value; only a 'reg' has one.

module OutputNonblockingInOn {
    in  clk : clock
    in  d   : u8
    out q   : u8

    on clk { q <= d }
    //~^ ERROR output port 'q' cannot be assigned with '<=' in an 'on' block
}
