// Simulation of a design whose extern bodies come from @source (ADR-0076).

test "extern bodies come from @source" {
    let dut = ExtTop { };
    dut.d = 0x0F;
    step(1);
    assert_eq(dut.inv, 0xF0);
    assert_eq(dut.dly, 0xF0);
    dut.d = 0xA5;
    step(1);
    assert_eq(dut.inv, 0x5A);
    assert_eq(dut.dly, 0x5A);
}
