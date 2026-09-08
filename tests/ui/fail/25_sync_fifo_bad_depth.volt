//~ E2025
// SyncFifo DEPTH must be a power of two (ADR-0029): the fill counter
// and address wrap-around assume 2^k entries.

module BadDepth {
    in  clk  : clock
    in  din  : u8
    in  push : bool
    in  pop  : bool
    out dout : u8

    let fifo = SyncFifo<u8, 10> {
    //~^ ERROR SyncFifo DEPTH must be a power of two
        clk: clk,
        wr_data: din,
        wr_en: push,
        rd_en: pop,
    }

    dout = fifo.rd_data
}
