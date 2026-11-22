//~ E4011
// #378: the same rule for SyncFifo's data output.

module M {
    in  clk  : clock
    in  d    : u8
    in  push : bool
    in  pop  : bool
    out q    : u8
    let f = SyncFifo<u8, 4> { clk: clk, wr_data: d, wr_en: push, rd_en: pop }
    f.rd_data = d
    //~^ ERROR instance output written
    q = f.rd_data
}
