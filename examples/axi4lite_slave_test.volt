// Simulation tests for the AXI4-Lite register slave (ADR-0033 test
// blocks, ADR-0039 bundle ports).
//
// Bundle fields are driven and observed through their flattened names
// (`dut.aw_addr`, `dut.b_valid`, ...). Inputs hold their value between
// steps, so a well-behaved master drops `*_valid` after the handshake
// edge — otherwise the slave would (correctly) accept a second
// transaction.
//
// Timing: a write with aw.valid and w.valid high completes on the next
// edge (registers update, b.valid rises); b.valid falls on the first
// edge where b.ready is high. A read with ar.valid high returns r.valid
// and r.data after one edge; r.valid falls once r.ready is seen.

test "reset state: idle channels and zeroed registers" {
    let dut = Axi4LiteSlave { };
    assert_false(dut.aw_ready);
    assert_false(dut.w_ready);
    assert_false(dut.b_valid);
    assert_false(dut.ar_ready);
    assert_false(dut.r_valid);
    assert_eq(dut.reg0, 0);
    assert_eq(dut.reg1, 0);
    assert_eq(dut.reg2, 0);
    assert_eq(dut.reg3, 0);
    assert_eq(dut.b_resp, 0);
    assert_eq(dut.r_resp, 0);
}

test "full-word write reaches reg1 and completes with b.valid" {
    let dut = Axi4LiteSlave { };
    dut.aw_addr = 0x4;
    dut.aw_valid = true;
    dut.w_data = 0xDEADBEEF;
    dut.w_strb = 0xF;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_eq(dut.reg1, 0xDEADBEEF);
    assert_eq(dut.reg0, 0);
    assert_true(dut.b_valid);
    assert_false(dut.aw_ready);
    dut.b_ready = true;
    step(1);
    assert_false(dut.b_valid);
}

test "byte strobes mask the written bytes" {
    let dut = Axi4LiteSlave { };
    dut.aw_addr = 0x8;
    dut.aw_valid = true;
    dut.w_data = 0x11223344;
    dut.w_strb = 0x3;
    dut.w_valid = true;
    step(1);
    dut.w_data = 0xAABBCCDD;
    dut.w_strb = 0xC;
    dut.b_ready = true;
    step(1);
    assert_eq(dut.reg2, 0x00003344);
    assert_false(dut.b_valid);
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_eq(dut.reg2, 0xAABB3344);
}

test "read returns the written register and holds until r.ready" {
    let dut = Axi4LiteSlave { };
    dut.aw_addr = 0xC;
    dut.aw_valid = true;
    dut.w_data = 0x0BADF00D;
    dut.w_strb = 0xF;
    dut.w_valid = true;
    dut.b_ready = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    dut.ar_addr = 0xC;
    dut.ar_valid = true;
    step(1);
    dut.ar_valid = false;
    assert_true(dut.r_valid);
    assert_eq(dut.r_data, 0x0BADF00D);
    assert_false(dut.ar_ready);
    step(2);
    assert_true(dut.r_valid);
    dut.r_ready = true;
    step(1);
    assert_false(dut.r_valid);
    assert_eq(dut.reg3, 0x0BADF00D);
}

test "status is read-only at 0x10 and unmapped addresses read zero" {
    let dut = Axi4LiteSlave { };
    dut.status = 0xCAFE;
    dut.aw_addr = 0x10;
    dut.aw_valid = true;
    dut.w_data = 0xFFFFFFFF;
    dut.w_strb = 0xF;
    dut.w_valid = true;
    dut.b_ready = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_eq(dut.reg0, 0);
    assert_eq(dut.reg3, 0);
    dut.ar_addr = 0x10;
    dut.ar_valid = true;
    dut.r_ready = true;
    step(1);
    assert_true(dut.r_valid);
    assert_eq(dut.r_data, 0xCAFE);
    dut.ar_addr = 0x1C;
    step(2);
    assert_eq(dut.r_data, 0);
    assert_eq(dut.r_resp, 0);
    dut.ar_valid = false;
    // A privileged access (prot != 0) is refused with SLVERR and writes nothing.
    dut.aw_addr = 0x0;
    dut.aw_prot = 4;
    dut.aw_valid = true;
    dut.w_valid = true;
    step(1);
    dut.aw_valid = false;
    dut.w_valid = false;
    assert_true(dut.b_valid);
    assert_eq(dut.b_resp, 2);
    assert_eq(dut.reg0, 0);
}
