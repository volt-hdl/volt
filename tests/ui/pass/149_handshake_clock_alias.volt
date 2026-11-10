// ADR-0114 K5 with a clock written through a type alias (ADR-0070):
// 'Clk' is 'clock', so VendorFifo is a clocked extern and stays opaque to
// E4007, exactly as with 'in clk : clock' (146_handshake_fifo_drain).
// The FIFO drain reads empty, the FIFO's registered state; rd_en only pops.
type Clk = clock

extern module VendorFifo {
    in  clk   : Clk
    in  wr_en : bool
    in  din   : u8
    in  rd_en : bool
    out dout  : u8
    out empty : bool
}

module DrainAlias {
    in  clk : Clk
    in  we  : bool
    in  d   : u8
    out tx  : Handshake<u8>

    let f = VendorFifo { clk: clk, wr_en: we, din: d, rd_en: tx.fired }
    tx.valid = !f.empty
    tx.data  = f.dout
}

// Output net (ADR-0079):
//~ NET-SKIP: the extern has no @source body on purpose (the fixture shows the handshake rule); the design is incomplete without external SV
