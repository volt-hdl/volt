// parity: E3001
// ADR-0083 Karar 8: comb koşulunun alanı atamaya katılır (K5).
domain Fast { clock = posedge, reset = sync active_high }
domain Slow { clock = posedge, reset = sync active_high }

module CombConditionCdc {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  fs       : bool  @Fast
    in  sa       : u8    @Slow
    out y        : u8    @Slow

    comb {
        if fs { y = sa } else { y = 0 }
    }
}
