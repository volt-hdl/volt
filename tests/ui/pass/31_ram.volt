// Built-in Ram and DualPortRam primitives (ADR-0029) -- synchronous
// read-first memories in a single clock domain. DEPTH must be a power
// of two, so the clog2(DEPTH)-bit address is in range by construction
// (the `addr < DEPTH` invariant). DualPortRam reminds about same-cycle
// same-address write-write collisions with W3006 (port B wins).
module Memories {
    in  clk    : clock
    in  addr   : bits<8>
    in  wdata  : u16
    in  we     : bool
    out rdata  : u16
    in  a_addr : bits<6>
    in  a_data : u8
    in  a_we   : bool
    out a_q    : u8
    in  b_addr : bits<6>
    in  b_data : u8
    in  b_we   : bool
    out b_q    : u8

    let spad = Ram<u16, 256> {
        clk: clk,
        addr: addr,
        wr_data: wdata,
        wr_en: we,
    }

    let shared = DualPortRam<u8, 64> {
        clk: clk,
        a_addr: a_addr,
        a_wr_data: a_data,
        a_wr_en: a_we,
        b_addr: b_addr,
        b_wr_data: b_data,
        b_wr_en: b_we,
    }

    rdata = spad.rd_data
    a_q   = shared.a_rd_data
    b_q   = shared.b_rd_data
}
