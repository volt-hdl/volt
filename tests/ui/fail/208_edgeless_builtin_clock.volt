//~ E3016
// A built-in primitive clocked by an edgeless domain. SyncFifo is built from
// flip-flops; they used to become `always_ff @(posedge clk)`.

domain Async { clock = none }

module EdgelessBuiltinClock {
    in  clk : clock @Async
    in  d   : u8
    in  we  : bool
    out q   : u8

    let f = SyncFifo<u8, 4> {
        clk: clk,
        //~^ ERROR SyncFifo clocked by clock domain 'Async', which has no clock edge
        wr_data: d,
        wr_en: we,
        rd_en: true,
    }
    q = f.rd_data
}
