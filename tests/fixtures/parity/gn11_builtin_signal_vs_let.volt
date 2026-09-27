// parity: E1003
module M {
    in  clk : clock
    in  d   : bool
    out o   : bool
    let f = SyncFifo<u8, 4> { clk: clk, wr_data: 3, wr_en: d, rd_en: d }
    let f_full = d
    o = f.empty ^ f_full
}
