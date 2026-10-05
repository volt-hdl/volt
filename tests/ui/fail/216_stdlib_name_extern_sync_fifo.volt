//~ E1016
// An extern module named like the standard library's SyncFifo. The
// built-in was taken instead, and the error asked for the built-in's
// type argument (E0003 "'SyncFifo' without a <T> type argument").

extern module SyncFifo {
//~^ ERROR extern module 'SyncFifo' has the name of a standard library module
    in  clk : clock
    in  d   : u8
    out q   : u8
}

module StdlibNameExternSyncFifo {
    in  clk : clock
    in  d   : u8
    out q   : u8

    let f = SyncFifo { clk: clk, d: d }
    q = f.q
}
