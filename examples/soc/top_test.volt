// Simulation tests for the SoC (ADR-0033). The sibling rule loads
// top.volt, which pulls in the other files through `use` (ADR-0042);
// SocTop is the DUT.
//
// Only SocTop's own ports are reachable: bundle fields by their flat
// names (dut.aw_data_addr, dut.b_valid, ...) plus gpio_out / gpio_oe /
// timer_irq / uart_tx / sys_dbg. There is no hierarchical access such
// as dut.timer_i.count_r, so internal state is observed through the bus.
//
// Timing (all peripherals share the AxiToReg protocol): a write with
// aw.valid and w.valid high completes on the next edge and b.valid
// rises; with b.ready held high it falls one edge later. A read with
// ar.valid high returns r.valid/r.data after one edge.

test "gpio write then read" {
    let dut = SocTop { };
    dut.b_ready = true;
    dut.r_ready = true;
    dut.w_data_strb = 0xF;
    // DATA_OUT = 0xA5
    dut.aw_data_addr = 0x000;
    dut.w_data_data = 0xA5;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_eq(dut.gpio_out, 0xA5);
    assert_true(dut.b_valid);
    assert_eq(dut.b_data_resp, 0);
    step(1);
    assert_false(dut.b_valid);
    // DIR = 0xF0
    dut.aw_data_addr = 0x004;
    dut.w_data_data = 0xF0;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_eq(dut.gpio_oe, 0xF0);
    step(1);
    // read DATA_OUT back
    dut.ar_data_addr = 0x000;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_true(dut.r_valid);
    assert_eq(dut.r_data_data, 0xA5);
    assert_eq(dut.r_data_resp, 0);
    // The pins land in the volatile DATA_IN register one edge after
    // they change (regs.data_in.pins <= pins_in), and a read captures
    // the register at the accept edge: drive the pins before that edge.
    dut.gpio_in = 0x3C;
    step(1);
    assert_false(dut.r_valid);
    // read the pins
    dut.ar_data_addr = 0x008;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 0x3C);
    step(1);
    // read DIR back
    dut.ar_data_addr = 0x004;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 0xF0);
}

test "timer counts up" {
    let dut = SocTop { };
    dut.b_ready = true;
    dut.r_ready = true;
    dut.w_data_strb = 0xF;
    // COMPARE = 5
    dut.aw_data_addr = 0x108;
    dut.w_data_data = 5;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    step(1);
    // CTRL = enable | irq_en
    dut.aw_data_addr = 0x100;
    dut.w_data_data = 3;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_false(dut.timer_irq);
    step(3);
    // COUNT is sampled by the read edge: 3 (then it becomes 4)
    dut.ar_data_addr = 0x104;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 3);
    assert_eq(dut.r_data_resp, 0);
    step(1);
    assert_false(dut.timer_irq);
    step(1);
    assert_true(dut.timer_irq);
    // STATUS reads 1 while pending
    dut.ar_data_addr = 0x10C;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 1);
    // write-1-to-clear
    dut.aw_data_addr = 0x10C;
    dut.w_data_data = 1;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_false(dut.timer_irq);
    // CTRL reads back 3
    dut.ar_data_addr = 0x100;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 3);
}

test "uart transmit via fifo" {
    let dut = SocTop { };
    dut.b_ready = true;
    dut.r_ready = true;
    dut.w_data_strb = 0xF;
    assert_true(dut.uart_tx);
    // TXDATA = 0x55: write edge 1, FIFO push edge 2, pop edge 3,
    // start sampled edge 4, start bit driven from edge 5.
    dut.aw_data_addr = 0x200;
    dut.w_data_data = 0x55;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    // The bridge refuses a new write while b.valid is pending, so a
    // back-to-back write needs one idle cycle for the b handshake.
    dut.aw_valid = false;
    dut.w_valid = false;
    step(1);
    // queue a second byte (accepted at edge 3, pushed at edge 4)
    dut.w_data_data = 0xAA;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    step(2);
    assert_false(dut.uart_tx);
    // data bit 0 of 0x55 is 1 (edges 9..12), bit 1 is 0 (13..16)
    step(4);
    assert_true(dut.uart_tx);
    step(4);
    assert_false(dut.uart_tx);
    // STATUS: busy (4), FIFO holds the second byte so not empty, not full
    dut.ar_data_addr = 0x204;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 4);
    // One outstanding read at a time: the next ar is accepted only
    // after r.valid has been consumed, so leave a cycle in between.
    step(1);
    // TXDATA reads back the last word written
    dut.ar_data_addr = 0x200;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 0xAA);
    // stop bit of the first frame (edges 41..44) is high
    step(26);
    assert_true(dut.uart_tx);
    // second frame: busy drops ~45, pop, start bit low from ~48; 0xAA
    // bit 0 is also 0 so the line stays low through edge 52.
    step(10);
    assert_false(dut.uart_tx);
    // FIFO drained
    dut.ar_data_addr = 0x204;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 6);
}

test "invalid address returns slverr" {
    let dut = SocTop { };
    dut.b_ready = true;
    dut.r_ready = true;
    dut.w_data_strb = 0xF;
    // write outside the map
    dut.aw_data_addr = 0x1000;
    dut.w_data_data = 0xFFFFFFFF;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_true(dut.b_valid);
    assert_eq(dut.b_data_resp, 2);
    assert_eq(dut.gpio_out, 0);
    step(1);
    assert_false(dut.b_valid);
    // read outside the map
    dut.ar_data_addr = 0x0FFC;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_true(dut.r_valid);
    assert_eq(dut.r_data_resp, 2);
    assert_eq(dut.r_data_data, 0);
    step(1);
    assert_false(dut.r_valid);
    // the last valid page still answers OKAY: SYSREG ID word at 0x310
    dut.ar_data_addr = 0x310;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_resp, 0);
    assert_eq(dut.r_data_data, 0x50C00001);
    step(1);
    // a privileged access to a mapped peripheral is refused by it
    dut.aw_data_addr = 0x000;
    dut.aw_data_prot = 1;
    dut.w_data_data = 0x11;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    dut.aw_data_prot = 0;
    assert_eq(dut.b_data_resp, 2);
    assert_eq(dut.gpio_out, 0);
}

test "concurrent peripheral access" {
    let dut = SocTop { };
    dut.b_ready = true;
    dut.r_ready = true;
    dut.w_data_strb = 0xF;
    // same edge: write GPIO DATA_OUT and read TIMER COUNT
    dut.aw_data_addr = 0x000;
    dut.w_data_data = 0x5A;
    dut.aw_valid = true;
    dut.w_valid = true;
    dut.ar_data_addr = 0x104;
    dut.ar_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    dut.ar_valid = false;
    assert_true(dut.b_valid);
    assert_eq(dut.b_data_resp, 0);
    assert_true(dut.r_valid);
    assert_eq(dut.r_data_resp, 0);
    assert_eq(dut.r_data_data, 0);
    assert_eq(dut.gpio_out, 0x5A);
    step(1);
    assert_false(dut.b_valid);
    assert_false(dut.r_valid);
    // same edge: write SYSREG reg1 and read GPIO DATA_OUT
    dut.aw_data_addr = 0x304;
    dut.w_data_data = 0x1234;
    dut.aw_valid = true;
    dut.w_valid = true;
    dut.ar_data_addr = 0x000;
    dut.ar_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 0x5A);
    assert_eq(dut.sys_dbg, 0x1234);
    step(1);
    // read SYSREG reg1 back through the decoder
    dut.ar_data_addr = 0x304;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_eq(dut.r_data_data, 0x1234);
    assert_eq(dut.r_data_resp, 0);
}
