// Simulation tests for the VGA controller (ADR-0033).
//
// The harness drives EVERY clock port of the DUT from the same
// generator (one `step` = one posedge on sys_clk AND pix_clk, see
// volt-sv-emit/src/sim.rs `run_cycle`), so the two domains run
// lock-step at 1:1 here. That is enough to check the sync timing, the
// frame-buffer path and the synchronizer latencies, but it does NOT
// exercise real asynchronous clocks (metastability, phase drift).
//
// Full frame = 800 x 525 = 420,000 cycles. The vsync and visible-region
// tests walk whole frames on VgaTiming (three registers, cheap); the
// VgaTop tests stay inside the first 30 lines.

// ── VgaTiming (pixel clock only) ─────────────────────────────────
// After `step(n)` from reset: pixel_x = n mod 800, pixel_y = n / 800.

test "hsync timing" {
    let dut = VgaTiming { };
    assert_true(dut.hsync);
    assert_eq(dut.pixel_x, 0);
    step(655);                     // last pixel before the pulse
    assert_true(dut.hsync);
    step(1);                       // 656: pulse starts (active low)
    assert_false(dut.hsync);
    step(95);                      // 751: last pulse pixel
    assert_false(dut.hsync);
    step(1);                       // 752: back porch
    assert_true(dut.hsync);
    step(48);                      // 800 → wraps to 0, next line
    assert_eq(dut.pixel_x, 0);
    assert_eq(dut.pixel_y, 1);
    step(656);                     // period is 800: low again at 656
    assert_false(dut.hsync);
}

test "vsync timing" {
    let dut = VgaTiming { };
    assert_true(dut.vsync);
    step(392000);                  // 490 lines
    assert_eq(dut.pixel_y, 490);
    assert_false(dut.vsync);
    step(1599);                    // line 491, last pixel
    assert_false(dut.vsync);
    step(1);                       // line 492: pulse over
    assert_true(dut.vsync);
    step(26400);                   // 33 more lines = 525 → wraps
    assert_eq(dut.pixel_y, 0);
    assert_eq(dut.pixel_x, 0);
    assert_true(dut.vsync);
}

test "visible region" {
    let dut = VgaTiming { };
    assert_true(dut.visible);
    step(639);
    assert_true(dut.visible);
    assert_eq(dut.pixel_x, 639);
    step(1);                       // 640: front porch
    assert_false(dut.visible);
    step(160);                     // (0, 1)
    assert_true(dut.visible);
    assert_eq(dut.pixel_y, 1);
    step(383200);                  // 479 lines → (0, 480)
    assert_eq(dut.pixel_y, 480);
    assert_false(dut.visible);
}

// ── FrameBuffer (both clocks) ────────────────────────────────────

test "frame buffer write read" {
    let dut = FrameBuffer { };
    dut.wr_x = 3;
    dut.wr_y = 2;
    dut.wr_data = true;
    dut.wr_en = true;
    step(1);
    dut.wr_en = false;
    step(10);                      // FIFO crossing + RAM write
    dut.rd_x = 3;
    dut.rd_y = 2;
    step(2);                       // registered read
    assert_true(dut.rd_data);
    dut.rd_x = 4;                  // neighbour untouched
    step(2);
    assert_false(dut.rd_data);
    dut.rd_x = 3;
    dut.rd_y = 3;                  // next row, same column
    step(2);
    assert_false(dut.rd_data);
}

// ── VgaTop (both clocks) ─────────────────────────────────────────
// Outputs are one cycle behind pixel_x (registered RAM read + aligned
// syncs): after `step(n)` the RGB pins show pixel n-1.

test "checkerboard fill" {
    let dut = VgaTop { };
    assert_false(dut.fill_done);
    step(24000);                   // 30 lines; 4800 writes need < 10k
    assert_true(dut.fill_done);
    assert_false(dut.blue);        // showing pixel (799, 29): blanking
    step(1);                       // (0, 30): cell (0, 3) → white, grid x
    assert_true(dut.red);
    assert_false(dut.green);
    assert_true(dut.blue);
    step(1);                       // (1, 30): white, no grid
    assert_true(dut.red);
    assert_true(dut.green);
    assert_true(dut.blue);
    step(8);                       // (9, 30): cell (1, 3) → background
    assert_false(dut.red);
    assert_false(dut.green);
    assert_true(dut.blue);
    dut.invert = true;             // sys → pix: 3 edges through sync()
    step(5);                       // (14, 30): inverted → white
    assert_true(dut.red);
    assert_true(dut.green);
    assert_true(dut.blue);
}

test "fill bar until done" {
    let dut = VgaTop { };
    // Line 0 is not in the bar, and the board is still being written.
    step(2);
    assert_false(dut.red);         // (1, 0): cell (0,0) = 0, no bar
    assert_true(dut.blue);
}

test "frame tick reaches system side" {
    let dut = VgaTop { };
    assert_false(dut.frame_tick);
    step(392010);                  // vsync low at 392000 (+1 reg, +2 sync)
    assert_true(dut.frame_tick);
    step(1600);                    // pulse is two lines long
    assert_false(dut.frame_tick);
}
