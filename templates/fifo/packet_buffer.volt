// A packet buffer on the built-in SyncFifo (single clock).
//
// SyncFifo<T, DEPTH> is one of Volt's built-in components (`volt explain
// stdlib`): the compiler generates its body and its contracts (it never
// overflows, never underflows, `count <= DEPTH`). Across two clock
// domains use AsyncFifo<T, DEPTH> the same way, with wr_clk and rd_clk
// (see `volt new --template cdc` for the clock-domain rules).

// One packet: a plain struct is one value made of named fields.
struct Packet {
    tag  : u4
    data : u8
}

pub module PacketBuffer {
    in  clk     : clock
    in  push    : bool
    in  in_pkt  : Packet
    in  pop     : bool
    out out_pkt : Packet
    out full    : bool
    out empty   : bool
    out overflow : bool  // a push was refused because the buffer was full

    // The FIFO stores raw bits: `as u12` packs the 12 bits of a Packet
    // (fields in declaration order, the first field in the high bits)
    // and `as Packet` unpacks them again.
    // A pop shows the oldest packet on rd_data one clock later.
    let fifo = SyncFifo<u12, 8> {
        clk: clk,
        wr_data: in_pkt as u12,
        wr_en: push,
        rd_en: pop,
    }

    // Sticky: stays set once a packet has been lost.
    reg overflow_r : bool = false
    on clk {
        if push && fifo.full {
            overflow_r <= true
        }
    }

    // The buffer is never full and empty at once.
    invariant: !(full && empty)
    cover: full
    cover: overflow_r

    out_pkt = fifo.rd_data as Packet
    full    = fifo.full
    empty   = fifo.empty
    overflow = overflow_r
}
