//~ E3016
// A register in a domain declared with `clock = none`. The 'on' block used
// to become `always_ff @(posedge clk)`: a flip-flop timed by an edge the
// domain says does not exist.

domain Async { clock = none }

module EdgelessDomainRegister {
    in  clk : clock @Async
    in  d   : u8
    out q   : u8

    reg r : u8 = 0
    on clk { r <= d }
    //~^ ERROR register in clock domain 'Async', which has no clock edge
    q = r
}
