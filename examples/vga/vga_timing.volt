// VGA sync generator for 640x480 @ 60 Hz (VESA "DMT 640x480 60Hz").
//
//   Pixel clock  25.175 MHz
//   Horizontal   640 visible + 16 front porch + 96 sync + 48 back porch = 800
//   Vertical     480 visible + 10 front porch +  2 sync + 33 back porch = 525
//   hsync/vsync are ACTIVE LOW (the standard polarity for this mode).
//
// The design has two clock domains; both are declared here because a
// multi-file unit shares one root scope (ADR-0042) and every file
// needs the same two names. This module itself lives entirely in
// PixDomain, so it is a single-clock module (K2): the @PixDomain
// annotation on pix_clk is the only one required, every other signal
// is inferred.

package vga::vga_timing;

/// System side: pattern writer, host control.
pub domain SysDomain {
    clock = posedge,
    reset = sync active_high,
    frequency = 100.mhz,
}

/// Pixel side: sync generator, frame-buffer read, RGB output.
pub domain PixDomain {
    clock = posedge,
    reset = sync active_high,
    frequency = 25_175.khz,      // 25.175 MHz; decimal literals do not lex
}

const H_VISIBLE   : u10 = 640
const H_SYNC_BEG  : u10 = 656   // 640 + 16 front porch
const H_SYNC_END  : u10 = 752   // 656 + 96 sync
const H_TOTAL     : u10 = 800   // 752 + 48 back porch
const V_VISIBLE   : u10 = 480
const V_SYNC_BEG  : u10 = 490   // 480 + 10 front porch
const V_SYNC_END  : u10 = 492   // 490 + 2 sync
const V_TOTAL     : u10 = 525   // 492 + 33 back porch

// The pixel clock is declared once, in PixDomain (frequency = 25_175.khz).
// This module only states what it NEEDS: at least 25.175 MHz, or the
// 640x480@60 timing below is wrong. Since ADR-0054 the compiler enforces
// the attribute: `volt build --emit=sdc` checks it against the domain
// (a slower domain is E0017) and writes create_clock -period 39.722 to
// build/constraints/VgaTiming.sdc. A decimal literal (25.175.mhz) does
// not lex, so the requirement is spelled in kHz.
@timing(pix_clk >= 25_175.khz)
pub module VgaTiming {
    in  pix_clk : clock @PixDomain
    out hsync   : bool
    out vsync   : bool
    out visible : bool
    out pixel_x : u10
    out pixel_y : u10

    // Counter ranges (inductive on their own: the wrap happens exactly
    // at TOTAL-1, and the reset value is 0).
    invariant: pixel_x < 800
    invariant: pixel_y < 525
    // Visible only inside the 640x480 window (ADR-0034 implication).
    invariant: visible -> (pixel_x < 640 && pixel_y < 480)
    // Sync pulses only inside their windows.
    invariant: !hsync -> (pixel_x >= 656 && pixel_x < 752)
    invariant: !vsync -> (pixel_y >= 490 && pixel_y < 492)
    // Both pulses are reachable (active low).
    cover: !hsync
    cover: !vsync
    cover: visible && pixel_x == 639 && pixel_y == 479

    reg h_cnt_r : u10 = 0
    reg v_cnt_r : u10 = 0

    on pix_clk {
        if h_cnt_r == H_TOTAL - 1 {
            h_cnt_r <= 0
            if v_cnt_r == V_TOTAL - 1 {
                v_cnt_r <= 0
            } else {
                v_cnt_r <= v_cnt_r + 1
            }
        } else {
            h_cnt_r <= h_cnt_r + 1
        }
    }

    hsync   = !(h_cnt_r >= H_SYNC_BEG && h_cnt_r < H_SYNC_END)
    vsync   = !(v_cnt_r >= V_SYNC_BEG && v_cnt_r < V_SYNC_END)
    visible = h_cnt_r < H_VISIBLE && v_cnt_r < V_VISIBLE
    pixel_x = h_cnt_r
    pixel_y = v_cnt_r
}
