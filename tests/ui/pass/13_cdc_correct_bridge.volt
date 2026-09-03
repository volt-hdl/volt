// Doğru CDC köprüsü — sync() ile geçiş
// domain-inference.md K9
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module CdcBridge {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow

    // Tek bit sinyal için iki-flop senkronizasyon
    slow_flag = sync(fast_flag, slow_clk)
}
