// Simulation tests for the UART transmitter (ADR-0033).
//
// The sibling file uart_tx.volt is parsed automatically; these tests
// answer the four questions formal verification could not:
//   1. Is the start bit low?
//   2. Are the 8 data bits sent LSB first?
//   3. Does one character occupy exactly 10 bit periods?
//   4. Does the line idle high?
//
// Timing model (CLKS_PER_BIT = 4, edges counted after the harness
// releases reset): the start pulse is sampled at edge 1; the start bit
// drives tx low after edges 2..5; data bit k is visible after edges
// 6+4k .. 9+4k (sampled mid-bit at edge 7+4k); the stop bit raises tx
// after edge 38 and busy falls at edge 41 — 10 bit periods in total.

test "idle line is high" {
    let dut = UartTx { };
    assert_true(dut.tx);
    assert_false(dut.busy);
    step(5);
    assert_true(dut.tx);
    assert_false(dut.busy);
}

test "start bit is low" {
    let dut = UartTx { };
    dut.data = 0xA5;
    dut.start = true;
    step(1);
    dut.start = false;
    step(1);
    assert_false(dut.tx);
    assert_true(dut.busy);
    step(3);
    assert_false(dut.tx);
}

test "data bits lsb first" {
    let dut = UartTx { };
    dut.data = 0xA5;
    dut.start = true;
    step(1);
    dut.start = false;
    step(6);
    assert_eq(dut.tx, true);
    step(4);
    assert_eq(dut.tx, false);
    step(4);
    assert_eq(dut.tx, true);
    step(4);
    assert_eq(dut.tx, false);
    step(4);
    assert_eq(dut.tx, false);
    step(4);
    assert_eq(dut.tx, true);
    step(4);
    assert_eq(dut.tx, false);
    step(4);
    assert_eq(dut.tx, true);
}

test "frame is 10 bits" {
    let dut = UartTx { };
    dut.data = 0x00;
    dut.start = true;
    step(1);
    dut.start = false;
    assert_true(dut.busy);
    step(39);
    assert_true(dut.busy);
    step(1);
    assert_false(dut.busy);
    assert_true(dut.tx);
}
