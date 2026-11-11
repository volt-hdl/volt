// ADR-0114 K5: the standard FIFO drain. empty is the FIFO's registered
// state and rd_en = tx.fired only pops. A clocked extern stays opaque to
// E4007; the built-in SyncFifo's summary says empty depends on no input.
extern module VendorFifo {
    in  clk   : clock
    in  wr_en : bool
    in  din   : u8
    in  rd_en : bool
    out dout  : u8
    out empty : bool
}

module DrainExt {
    in  clk : clock
    in  we  : bool
    in  d   : u8
    out tx  : Handshake<u8>

    let f = VendorFifo { clk: clk, wr_en: we, din: d, rd_en: tx.fired }
    tx.valid = !f.empty
    tx.data  = f.dout
}

module DrainBuiltin {
    in  clk  : clock
    in  we   : bool
    in  d    : u8
    out full : bool
    out tx   : Handshake<u8>

    let f = SyncFifo<u8, 4> { clk: clk, wr_data: d, wr_en: we, rd_en: tx.fired }
    tx.valid = !f.empty
    tx.data  = f.rd_data
    full     = f.full
}

// Output net (ADR-0079):
//~ NET-SKIP: the extern has no @source body on purpose (the fixture shows the handshake rule); the design is incomplete without external SV
