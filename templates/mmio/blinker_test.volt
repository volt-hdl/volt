// Simulation tests for Blinker, driven through its AXI4-Lite ports the
// way a CPU would. Registers (base 0x4000_0000):
//   0x00 control  mode: 0 off, 1 on, 2 blink
//   0x04 period   blink half-period in cycles
//   0x08 status   bit 0 = LED, bits 31:16 = toggle count (read-only)
//
// Bus timing: a write with aw_valid and w_valid high completes on the
// next edge (b_valid rises); a read with ar_valid high returns r_data
// and r_valid after one edge.

test "led is off after reset" {
    let dut = Blinker { };
    step(5);
    assert_false(dut.led);
}

test "mode 1 turns the led on" {
    let dut = Blinker { };
    dut.b_ready = true;
    dut.w_strb = 0xF;
    dut.aw_addr = 0x4000_0000;   // control
    dut.w_data = 1;              // mode = on
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_true(dut.b_valid);
    assert_eq(dut.b_resp, 0);    // OKAY
    assert_true(dut.led);
}

test "blink mode toggles every period and counts toggles" {
    let dut = Blinker { };
    dut.b_ready = true;
    dut.r_ready = true;
    dut.w_strb = 0xF;
    // period = 2 cycles
    dut.aw_addr = 0x4000_0004;
    dut.w_data = 2;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    // The write response (b_valid) is taken on the next edge; only then
    // is the slave ready for the next write.
    step(1);
    // control.mode = blink
    dut.aw_addr = 0x4000_0000;
    dut.w_data = 2;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_false(dut.led);
    step(2);
    assert_true(dut.led);
    step(2);
    assert_false(dut.led);
    step(2);
    assert_true(dut.led);
    // The status register follows the LED one clock later, and a read
    // returns the value at the edge that accepts it.
    step(1);
    dut.ar_addr = 0x4000_0008;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_true(dut.r_valid);
    assert_eq(dut.r_resp, 0);
    assert_eq(dut.r_data, 0x0003_0001);   // toggles = 3, LED on
}
