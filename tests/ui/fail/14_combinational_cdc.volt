//~ E3001
// Clock domain mixing in a combinational expression.
// domain-inference.md K5 -- glitch risk

domain Fast { clock = posedge, reset = sync active_high }
domain Slow { clock = posedge, reset = sync active_high }

module CombinationalCdc {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  fast_sig : bool  @Fast
    in  slow_sig : bool  @Slow
    out result   : bool  @Slow

    result = fast_sig & slow_sig
    //~^ ERROR different clock domains cannot be combined combinationally
}
