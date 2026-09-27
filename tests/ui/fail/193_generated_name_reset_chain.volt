//~ E1003
// ADR-0090: a raw reset port gets a release synchronizer per clock
// (ADR-0065), 'rst_sync_<clock>_stage<i>' — named in the SDC.

domain Fast {
    clock = posedge
    reset = async active_low
}

module Chain {
    in  clk   : clock @Fast
//~^ ERROR 'rst_sync_clk_stage0' is both let 'rst_sync_clk_stage0' and stage 0 of the reset synchronizer of clock 'clk'
    in  rst_n : reset(async, active_low)
    in  x     : bool @Fast
    out o     : bool @Fast

    reg r : bool = false
    on clk { r <= x }
    let rst_sync_clk_stage0 = r
    o = rst_sync_clk_stage0
}
