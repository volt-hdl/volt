// VGA top level: a system-clock pattern writer feeding an 80x60
// frame buffer that a pixel-clock scan-out reads back.
//
//   VgaTop
//    ├── sys_clk (SysDomain)  checkerboard writer FSM, `invert` control
//    │       │  AsyncFifo (inside FrameBuffer) + sync() for `invert`
//    ├── FrameBuffer          write side SysDomain, read side PixDomain
//    ├── VgaTiming            pix_clk only
//    └── pix_clk (PixDomain)  read address, 1-cycle alignment, RGB
//
// Clock-domain crossings (all explicit):
//   sys → pix  pixel data      AsyncFifo<u14,16>   (in FrameBuffer)
//   sys → pix  invert (1 bit)  sync()
//   sys → pix  fill_done (1 bit) sync()
//   pix → sys  frame_tick      sync()  (vsync active, for host pacing)

package vga::vga_top;

use vga::vga_timing::{SysDomain, PixDomain, VgaTiming};
use vga::frame_buffer::FrameBuffer;

pub module VgaTop {
    // ── System domain ─────────────────────────────────────────────
    in  sys_clk    : clock @SysDomain
    in  invert     : bool  @SysDomain   // host control: invert colours
    out fill_done  : bool  @SysDomain   // the whole 80x60 board is written
    out frame_tick : bool  @SysDomain   // vsync, seen from the system side

    // ── Pixel domain ──────────────────────────────────────────────
    in  pix_clk : clock @PixDomain
    out hsync   : bool  @PixDomain
    out vsync   : bool  @PixDomain
    out red     : bool  @PixDomain
    out green   : bool  @PixDomain
    out blue    : bool  @PixDomain
    out fb_dbg  : bool  @PixDomain   // frame buffer port-A read-first value

    // Writer progress stays inside the board.
    invariant: wx_r < 80
    invariant: wy_r < 60
    // Once done the writer never restarts (wx/wy park at 0).
    invariant: done_r -> (wx_r == 0 && wy_r == 0)
    cover: done_r

    // ── Checkerboard writer (SysDomain) ───────────────────────────
    // Walks (x, y) over 80x60 once; cell = 8x8 pixels of the visible
    // 640x480, colour = bit 0 of x XOR bit 0 of y. Stalls on wr_full.
    reg wx_r   : u7   = 0
    reg wy_r   : u6   = 0
    reg done_r : bool = false

    wire fb_full : bool
    let advance : bool = !done_r && !fb_full
    let cell_bit : bool = wx_r[0] ^ wy_r[0]

    on sys_clk {
        if advance {
            if wx_r == 79 {
                wx_r <= 0
                if wy_r == 59 {
                    wy_r   <= 0
                    done_r <= true
                } else {
                    wy_r <= wy_r + 1
                }
            } else {
                wx_r <= wx_r + 1
            }
        }
    }
    fill_done = done_r

    // ── Pixel side ────────────────────────────────────────────────
    let timing = VgaTiming { pix_clk: pix_clk }

    // 640x480 pixels → 80x60 cells: drop the three low bits.
    let rd_x : u7 = timing.pixel_x[9:3] as u7
    let rd_y : u6 = timing.pixel_y[8:3] as u6

    let fb = FrameBuffer {
        sys_clk: sys_clk,
        wr_x:    wx_r,
        wr_y:    wy_r,
        wr_data: cell_bit,
        wr_en:   advance,
        pix_clk: pix_clk,
        rd_x:    rd_x,
        rd_y:    rd_y,
    }
    fb_full = fb.wr_full

    // The RAM read is registered (one pix_clk), so delay the sync and
    // blanking signals by one cycle to keep them aligned with the bit.
    // Also registered: grid lines every 8 pixels (the low pixel bits)
    // and a bottom bar (rows 472..479) that stays red until the writer
    // has filled the board.
    reg hsync_r   : bool = true
    reg vsync_r   : bool = true
    reg visible_r : bool = false
    reg grid_r    : bool = false
    reg bar_r     : bool = false
    on pix_clk {
        hsync_r   <= timing.hsync
        vsync_r   <= timing.vsync
        visible_r <= timing.visible
        grid_r    <= (timing.pixel_x[2:0] as u3) == 0 || (timing.pixel_y[2:0] as u3) == 0
        bar_r     <= timing.pixel_y >= 472
    }

    // Host control and the writer's done flag cross with two-flop
    // synchronizers (1 bit each). `let x = sync(...)` is rejected by
    // the emitter (E0003), so the outputs are wires.
    wire invert_s : bool
    wire filled_s : bool
    invert_s = sync(invert, pix_clk)
    filled_s = sync(done_r, pix_clk)
    let pix     : bool = fb.rd_data ^ invert_s
    let red_bar : bool = bar_r && !filled_s

    hsync  = hsync_r
    vsync  = vsync_r
    red    = visible_r && (pix || red_bar)
    green  = visible_r && pix && !grid_r     // white cells, magenta grid
    blue   = visible_r && !red_bar           // blue background
    fb_dbg = fb.wr_prev

    // vsync is a level (two lines long), safe to carry with sync().
    let vs_active : bool = !vsync_r
    frame_tick = sync(vs_active, sys_clk)
}
