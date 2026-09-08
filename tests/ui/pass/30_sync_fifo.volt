// Built-in SyncFifo primitive (ADR-0029) -- single-clock FIFO with a
// fill counter. Unlike AsyncFifo there is no two-flop synchronizer
// lag, so both `count <= DEPTH` and `!(full && empty)` are provable.
module FifoBuffer {
    in  clk   : clock
    in  din   : u8
    in  push  : bool
    in  pop   : bool
    out dout  : u8
    out full  : bool
    out empty : bool

    let fifo = SyncFifo<u8, 16> {
        clk: clk,
        wr_data: din,
        wr_en: push,
        rd_en: pop,
    }

    // Outputs are read back with field access.
    dout  = fifo.rd_data
    full  = fifo.full
    empty = fifo.empty
}
