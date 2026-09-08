// Built-in HandshakeSync primitive (ADR-0027) -- single multi-bit
// transfer with a 4-phase req/ack handshake. The data register stays
// stable in the source domain until the destination acknowledges, so
// the multi-bit value crosses safely.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module HsBridge {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  din      : u8    @Fast
    in  send_req : bool  @Fast
    out busy     : bool  @Fast
    out dout     : u8    @Slow
    out valid    : bool  @Slow

    let hs = HandshakeSync<u8> {
        src_clk: fast_clk,
        data_in: din,
        send: send_req,
        dst_clk: slow_clk,
    }

    // busy is high while a transfer is in flight (source domain);
    // valid strobes for one destination cycle when dout updates.
    busy  = hs.busy
    dout  = hs.data_out
    valid = hs.valid
}
