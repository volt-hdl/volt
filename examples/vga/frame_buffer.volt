// 80x60 1-bit frame buffer written from SysDomain and read from
// PixDomain -- a true dual-clock memory (ADR-0049).
//
//   sys_clk ──► wr_addr / wr_data / wr_en ──► AsyncDualPortRam<bool, 8192> ──► rd_data ──► pix_clk
//                                              (the array itself is the CDC boundary)
//
// Nothing is synchronized by hand: each port's address and data stay
// in their own domain and the compiler checks that (E3001 on a
// wrong-domain binding, see the README). Address = y * 128 + x (a
// shift, no multiplier), so DEPTH is the next power of two above
// 60 * 128.
//
// W3006 (expected, once per instance): a pixel read in the same cycle
// it is being written from sys_clk returns an undefined value. The
// scan-out then shows that one pixel a frame late -- harmless here.
//
// Write latency: the write lands on the next sys_clk edge; a read of
// the same address returns the new bit one pix_clk after rd_x/rd_y.

package vga::frame_buffer;

use vga::vga_timing::{SysDomain, PixDomain};

pub module FrameBuffer {
    // ── System side (writer) ──────────────────────────────────────
    in  sys_clk : clock @SysDomain
    in  wr_x    : u7    @SysDomain   // 0..79
    in  wr_y    : u6    @SysDomain   // 0..59
    in  wr_data : bool  @SysDomain
    in  wr_en   : bool  @SysDomain

    // ── Pixel side (reader) ───────────────────────────────────────
    in  pix_clk : clock @PixDomain
    in  rd_x    : u7    @PixDomain
    in  rd_y    : u6    @PixDomain
    out rd_data : bool  @PixDomain   // one pix_clk after rd_x/rd_y

    // Address ports are bits<13>, arithmetic wants u13: build in u13,
    // then cast (same width, so the cast is allowed).
    let wr_addr : bits<13> = (((wr_y as u13) << 7) | (wr_x as u13)) as bits<13>
    let rd_addr : bits<13> = (((rd_y as u13) << 7) | (rd_x as u13)) as bits<13>

    let mem = AsyncDualPortRam<bool, 8192> {
        wr_clk:  sys_clk,
        wr_addr: wr_addr,
        wr_data: wr_data,
        wr_en:   wr_en,
        rd_clk:  pix_clk,
        rd_addr: rd_addr,
    }
    rd_data = mem.rd_data
}
