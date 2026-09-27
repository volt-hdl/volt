// Simulation tests for PacketBuffer. Whole struct values can be driven
// and compared: `Packet { tag: 1, data: 0x10 }`.

test "starts empty" {
    let dut = PacketBuffer { };
    assert_true(dut.empty);
    assert_false(dut.full);
    assert_false(dut.overflow);
}

test "packets come out in order" {
    let dut = PacketBuffer { };
    dut.push = true;
    dut.in_pkt = Packet { tag: 1, data: 0x10 };
    step(1);
    dut.in_pkt = Packet { tag: 2, data: 0x20 };
    step(1);
    dut.push = false;
    assert_false(dut.empty);
    // A pop shows the oldest packet one clock later.
    dut.pop = true;
    step(1);
    assert_eq(dut.out_pkt, Packet { tag: 1, data: 0x10 });
    step(1);
    dut.pop = false;
    assert_eq(dut.out_pkt, Packet { tag: 2, data: 0x20 });
    assert_true(dut.empty);
}

test "a push into a full buffer sets overflow" {
    let dut = PacketBuffer { };
    dut.push = true;
    for i in 0..8 {
        dut.in_pkt = Packet { tag: i, data: i };
        step(1);
    }
    assert_true(dut.full);
    assert_false(dut.overflow);
    step(1);
    dut.push = false;
    assert_true(dut.overflow);
    // The first eight packets are intact.
    dut.pop = true;
    step(1);
    assert_eq(dut.out_pkt, Packet { tag: 0, data: 0 });
    step(1);
    assert_eq(dut.out_pkt, Packet { tag: 1, data: 1 });
}
