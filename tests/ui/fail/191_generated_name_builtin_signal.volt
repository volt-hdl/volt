//~ E1003
// ADR-0090: a built-in primitive's signals are '<instance>_<name>' (the
// CDC primitives' registers are named in SDC/XDC); 'f_full' is taken.

module Buffer {
    in  clk : clock
    in  d   : bool
    out o   : bool

    let f = SyncFifo<u8, 4> { clk: clk, wr_data: 3, wr_en: d, rd_en: d }
//~^ ERROR 'f_full' is both let 'f_full' and signal 'full' of the built-in SyncFifo instance 'f'
    let f_full = d
    o = f.empty ^ f_full
}
