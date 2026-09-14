// 80x60 1-bit frame buffer written from SysDomain and read from
// PixDomain.
//
// Why it is NOT a dual-clock memory: the stdlib `DualPortRam` has one
// `clk` port (ADR-0029) and the language offers no way to place two
// ports of a `reg` array in two clock domains (K4/K7 reject it, see
// README). So the crossing is moved to the WRITE PATH instead:
//
//   sys_clk ──► pack (x, y, bit) ──► AsyncFifo<u14,16> ──► pix_clk
//                                                          │
//                            DualPortRam<bool, 8192> ◄── pop + write
//                            (single clock: pix_clk)  ──► read (x, y)
//
// The memory itself is single-clock (PixDomain); the system side only
// ever sees the FIFO's `wr_full`. Address = y * 128 + x (a shift, no
// multiplier), so DEPTH is the next power of two above 60 * 128.
//
// Write latency (both clocks running): the FIFO write lands on the
// next sys edge, the gray pointer needs two pix edges to synchronize,
// the pop registers the word on the following pix edge and the RAM
// write happens one edge later — about six edges before a read of the
// same address returns the new bit.

package vga::frame_buffer;

use vga::vga_timing::{SysDomain, PixDomain};

pub module FrameBuffer {
    // ── System side (writer) ──────────────────────────────────────
    in  sys_clk : clock @SysDomain
    in  wr_x    : u7    @SysDomain   // 0..79
    in  wr_y    : u6    @SysDomain   // 0..59
    in  wr_data : bool  @SysDomain
    in  wr_en   : bool  @SysDomain
    out wr_full : bool  @SysDomain   // FIFO back-pressure

    // ── Pixel side (reader) ───────────────────────────────────────
    in  pix_clk : clock @PixDomain
    in  rd_x    : u7    @PixDomain
    in  rd_y    : u6    @PixDomain
    out rd_data : bool  @PixDomain   // one pix_clk after rd_x/rd_y
    out wr_prev : bool  @PixDomain   // port A is read-first: the bit that was overwritten

    // The FIFO is drained every cycle it is non-empty, so at most one
    // word waits for the RAM write; the write-enable register is the
    // only pixel-side state.
    cover: fifo_valid_r
    cover: wr_full

    // Pack the write command into one FIFO word: bit 13 = data,
    // bits 12..7 = y, bits 6..0 = x. (No concatenation operator in
    // the grammar; shifts on the widened operands do the job.)
    let wr_bit : u14 = if wr_data { 1 } else { 0 }
    let wr_cmd : u14 = (wr_bit << 13) | ((wr_y as u14) << 7) | (wr_x as u14)

    // Pop whenever a word is available (declared ahead: the module
    // body resolves top-down and the FIFO output is not visible yet).
    wire pop : bool

    let q = AsyncFifo<u14, 16> {
        wr_clk:  sys_clk,
        wr_data: wr_cmd,
        wr_en:   wr_en,
        rd_clk:  pix_clk,
        rd_en:   pop,
    }
    pop     = !q.rd_empty
    wr_full = q.wr_full

    // AsyncFifo.rd_data is registered: valid the cycle AFTER rd_en.
    reg fifo_valid_r : bool = false
    on pix_clk {
        fifo_valid_r <= pop
    }

    let cmd      : u14  = q.rd_data
    let cmd_addr : bits<13> = cmd[12:0]
    let cmd_bit  : bool     = cmd[13]

    // Address ports are bits<N>, arithmetic wants uN: build in u13,
    // then cast (same width, so the cast is allowed).
    let rd_addr_u : u13      = ((rd_y as u13) << 7) | (rd_x as u13)
    let rd_addr   : bits<13> = rd_addr_u as bits<13>

    // Port A writes, port B reads. b_wr_en is tied low, so the W3006
    // same-address collision warning is moot (the compiler still
    // emits it: it cannot see that the constant excludes it).
    let mem = DualPortRam<bool, 8192> {
        clk:       pix_clk,
        a_addr:    cmd_addr,
        a_wr_data: cmd_bit,
        a_wr_en:   fifo_valid_r,
        b_addr:    rd_addr,
        b_wr_data: false,
        b_wr_en:   false,
    }
    rd_data = mem.b_rd_data
    wr_prev = mem.a_rd_data
}
