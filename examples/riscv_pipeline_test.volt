// RiscvPipeline testbench — every hazard path plus fill/flush
// timing, asserted cycle-exactly.
//
// Timing model (no stall/flush): the instruction presented on
// `instr` during cycle c is latched into IF/ID at step c+1, sits in
// MEM (mem_* outputs visible) during cycle c+3, i.e. after the
// (c+3)-th step, and its result is written to the register file at
// step c+5. Inputs hold between steps, so trailing NOPs only need
// to be set once.
//
// Register values are observed the riscv_core way: execute
// `sw rN, 0(x0)` and assert on mem_wdata three steps later.
//
// Frequently used encodings:
//   0x00000013  nop              (addi x0, x0, 0)
//   0x00500093  addi x1, x0, 5
//   0x00700093  addi x1, x0, 7
//   0x00500113  addi x2, x0, 5
//   0x00700113  addi x2, x0, 7
//   0x00102023  sw   x1, 0(x0)
//   0x00202023  sw   x2, 0(x0)
//   0x00302023  sw   x3, 0(x0)
//   0x00402023  sw   x4, 0(x0)
//   0x00502023  sw   x5, 0(x0)

test "reset drives pc and memory controls to zero" {
    let dut = RiscvPipeline { };
    assert_eq(dut.pc, 0);
    assert_false(dut.mem_write);
    assert_false(dut.mem_read);
    assert_false(dut.stall_o);
    assert_false(dut.flush_o);
}

test "pipeline fill" {
    // Five instructions enter back to back; pc advances by 4 every
    // cycle (no stalls anywhere in this stream).
    let dut = RiscvPipeline { };
    dut.instr = 0x00500093;      // c0: addi x1, x0, 5
    step(1);
    assert_eq(dut.pc, 4);
    dut.instr = 0x00700113;      // c1: addi x2, x0, 7
    step(1);
    assert_eq(dut.pc, 8);
    dut.instr = 0x002081B3;      // c2: add x3, x1, x2 (x1 dist-2, x2 dist-1)
    step(1);
    assert_eq(dut.pc, 12);
    dut.instr = 0x00302023;      // c3: sw x3, 0(x0)   (x3 dist-1)
    step(1);
    assert_eq(dut.pc, 16);
    dut.instr = 0x00000013;      // c4+: nop
    step(1);
    assert_false(dut.mem_write); // sw still in EX
    step(1);                     // sw now in MEM (cycle 6)
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 12);
    assert_eq(dut.mem_addr, 0);
    step(1);
    assert_false(dut.mem_write); // exactly one MEM cycle
}

test "back to back add" {
    // Each add consumes the result produced one cycle earlier —
    // pure EX to EX forwarding, twice in a row.
    let dut = RiscvPipeline { };
    dut.instr = 0x00500093;      // c0: addi x1, x0, 5
    step(1);
    dut.instr = 0x00108133;      // c1: add x2, x1, x1  (x1 dist-1)
    step(1);
    dut.instr = 0x002101B3;      // c2: add x3, x2, x2  (x2 dist-1)
    step(1);
    dut.instr = 0x00302023;      // c3: sw x3, 0(x0)
    step(1);
    dut.instr = 0x00000013;      // c4+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 20); // (5+5)+(5+5)... (5+5)=10, 10+10=20
}

test "raw hazard ex to ex" {
    // x2 is settled long before; x1 is produced by the instruction
    // immediately preceding its use.
    let dut = RiscvPipeline { };
    dut.instr = 0x00500113;      // c0: addi x2, x0, 5
    step(1);
    dut.instr = 0x00000013;      // c1: nop
    step(2);                     // c2: nop
    dut.instr = 0x00700093;      // c3: addi x1, x0, 7
    step(1);
    dut.instr = 0x402081B3;      // c4: sub x3, x1, x2  (x1 dist-1 → EX→EX)
    step(1);
    dut.instr = 0x00302023;      // c5: sw x3, 0(x0)
    step(1);
    dut.instr = 0x00000013;      // c6+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 2); // 7 - 5
}

test "raw hazard mem to ex" {
    // One nop between producer and consumer: the result lives in
    // MEM/WB when the consumer executes → MEM→EX forwarding.
    let dut = RiscvPipeline { };
    dut.instr = 0x00500113;      // c0: addi x2, x0, 5
    step(1);
    dut.instr = 0x00000013;      // c1: nop
    step(2);                     // c2: nop
    dut.instr = 0x00700093;      // c3: addi x1, x0, 7
    step(1);
    dut.instr = 0x00000013;      // c4: nop
    step(1);
    dut.instr = 0x402081B3;      // c5: sub x3, x1, x2  (x1 dist-2 → MEM→EX)
    step(1);
    dut.instr = 0x00302023;      // c6: sw x3, 0(x0)
    step(1);
    dut.instr = 0x00000013;      // c7+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 2);
}

test "raw hazard wb to id" {
    // Two nops between producer and consumer: the producer is in WB
    // while the consumer reads the register file in ID — this only
    // works through the WB→ID bypass, the register array itself is
    // written one edge too late.
    let dut = RiscvPipeline { };
    dut.instr = 0x00500113;      // c0: addi x2, x0, 5
    step(1);
    dut.instr = 0x00000013;      // c1: nop
    step(2);                     // c2: nop
    dut.instr = 0x00700093;      // c3: addi x1, x0, 7
    step(1);
    dut.instr = 0x00000013;      // c4: nop
    step(2);                     // c5: nop
    dut.instr = 0x402081B3;      // c6: sub x3, x1, x2  (x1 dist-3 → WB→ID)
    step(1);
    dut.instr = 0x00302023;      // c7: sw x3, 0(x0)
    step(1);
    dut.instr = 0x00000013;      // c8+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 2);
}

test "load use stall" {
    // add depends on lw one cycle earlier → exactly one stall
    // cycle, then the load lands via MEM→EX forwarding.
    let dut = RiscvPipeline { };
    dut.mem_rdata = 21;
    dut.instr = 0x00002083;      // c0: lw x1, 0(x0)
    step(1);
    dut.instr = 0x00108133;      // c1: add x2, x1, x1
    step(1);
    // Cycle 2: lw in EX, add in ID → hazard unit must stall.
    assert_true(dut.stall_o);
    assert_eq(dut.pc, 8);
    dut.instr = 0x00202023;      // c2: sw x2, 0(x0) — NOT latched (stall)
    step(1);
    assert_eq(dut.pc, 8);        // pc held
    assert_false(dut.stall_o);   // bubble now in EX, hazard gone
    assert_true(dut.mem_read);   // the stall did not delay lw itself:
                                 // it is in MEM during cycle 3
    step(1);                     // c3: sw finally enters IF/ID
    assert_eq(dut.pc, 12);
    dut.instr = 0x00000013;      // c4+: nop
    step(2);                     // sw reaches MEM in cycle 6
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 42); // 21 + 21
}

test "branch flush squashes both shadow instructions" {
    // beq at pc 8 (taken, +12 → target 20). The two instructions
    // fetched behind it must never execute. Both branch operands
    // arrive through forwarding (x1 dist-2, x2 dist-1).
    let dut = RiscvPipeline { };
    dut.instr = 0x00500093;      // c0: addi x1, x0, 5
    step(1);
    dut.instr = 0x00500113;      // c1: addi x2, x0, 5
    step(1);
    dut.instr = 0x00208663;      // c2: beq x1, x2, +12
    step(1);
    dut.instr = 0x06300193;      // c3: addi x3, x0, 99  (shadow 1, pc 12)
    step(1);
    // Cycle 4: beq resolves in EX with forwarded operands.
    assert_true(dut.flush_o);
    dut.instr = 0x05800213;      // c4: addi x4, x0, 88  (shadow 2, pc 16)
    step(1);
    assert_eq(dut.pc, 20);       // redirect landed
    assert_false(dut.flush_o);
    dut.instr = 0x00300293;      // c5: addi x5, x0, 3   (branch target)
    step(1);
    dut.instr = 0x00000013;      // c6: nop
    step(1);
    // Read back x3, x4, x5 through the store port.
    dut.instr = 0x00302023;      // c7: sw x3 — must still be 0
    step(1);
    dut.instr = 0x00402023;      // c8: sw x4 — must still be 0
    step(1);
    dut.instr = 0x00502023;      // c9: sw x5 — must be 3
    step(1);
    dut.instr = 0x00000013;      // c10+: nop
    assert_eq(dut.mem_wdata, 0); // sw x3 in MEM (cycle 10)
    assert_true(dut.mem_write);
    step(1);
    assert_eq(dut.mem_wdata, 0); // sw x4 in MEM (cycle 11)
    step(1);
    assert_eq(dut.mem_wdata, 3); // sw x5 in MEM (cycle 12)
}

test "branch not taken falls through" {
    let dut = RiscvPipeline { };
    dut.instr = 0x00500093;      // c0: addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // c1: addi x2, x0, 7
    step(1);
    dut.instr = 0x00208663;      // c2: beq x1, x2, +12 (5 != 7 → not taken)
    step(1);
    dut.instr = 0x06300193;      // c3: addi x3, x0, 99 (must execute)
    step(1);
    assert_false(dut.flush_o);   // cycle 4: beq in EX, not taken
    dut.instr = 0x00000013;      // c4: nop
    step(1);
    dut.instr = 0x00302023;      // c5: sw x3, 0(x0)
    step(1);
    dut.instr = 0x00000013;      // c6+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 99);
}

test "bne taken flushes" {
    let dut = RiscvPipeline { };
    dut.instr = 0x00500093;      // c0: addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // c1: addi x2, x0, 7
    step(1);
    dut.instr = 0x00209663;      // c2: bne x1, x2, +12 (5 != 7 → taken)
    step(1);
    dut.instr = 0x06300193;      // c3: addi x3, x0, 99 (shadow, squashed)
    step(1);
    assert_true(dut.flush_o);
    dut.instr = 0x00000013;      // c4: nop (shadow 2 slot)
    step(1);
    assert_eq(dut.pc, 20);
    dut.instr = 0x00302023;      // c5: sw x3 — x3 must still be 0
    step(1);
    dut.instr = 0x00000013;      // c6+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 0);
}

test "jal redirects and links" {
    let dut = RiscvPipeline { };
    dut.instr = 0x010000EF;      // c0: jal x1, +16  (pc 0 → target 16)
    step(1);
    dut.instr = 0x06300193;      // c1: addi x3, x0, 99 (shadow 1, pc 4)
    step(1);
    // Cycle 2: jal resolves in EX.
    assert_true(dut.flush_o);
    dut.instr = 0x05800213;      // c2: addi x4, x0, 88 (shadow 2, pc 8)
    step(1);
    assert_eq(dut.pc, 16);       // redirect landed
    dut.instr = 0x00300293;      // c3: addi x5, x0, 3  (jump target)
    step(1);
    dut.instr = 0x00102023;      // c4: sw x1 — link value
    step(1);
    dut.instr = 0x00302023;      // c5: sw x3 — must still be 0
    step(1);
    dut.instr = 0x00502023;      // c6: sw x5 — must be 3
    step(1);
    dut.instr = 0x00000013;      // c7+: nop
    assert_eq(dut.mem_wdata, 4); // sw x1 in MEM (cycle 7): link = 0 + 4
    step(1);
    assert_eq(dut.mem_wdata, 0); // sw x3 in MEM (cycle 8)
    step(1);
    assert_eq(dut.mem_wdata, 3); // sw x5 in MEM (cycle 9)
}

test "lui loads upper immediate" {
    let dut = RiscvPipeline { };
    dut.instr = 0x123450B7;      // c0: lui x1, 0x12345
    step(1);
    dut.instr = 0x00102023;      // c1: sw x1, 0(x0)  (x1 dist-1)
    step(1);
    dut.instr = 0x00000013;      // c2+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 0x12345000);
}

test "logic ops forward through both paths" {
    let dut = RiscvPipeline { };
    dut.instr = 0x00500093;      // c0: addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // c1: addi x2, x0, 7
    step(1);
    dut.instr = 0x0020F1B3;      // c2: and x3, x1, x2 (x1 dist-2, x2 dist-1)
    step(1);
    dut.instr = 0x00302023;      // c3: sw x3
    step(1);
    dut.instr = 0x0020E233;      // c4: or  x4, x1, x2 (regfile reads)
    step(1);
    dut.instr = 0x0020C2B3;      // c5: xor x5, x1, x2
    step(1);
    assert_eq(dut.mem_wdata, 5); // sw x3 in MEM (cycle 6): 5 & 7
    dut.instr = 0x00402023;      // c6: sw x4 (x4 dist-2 store data)
    step(1);
    dut.instr = 0x00502023;      // c7: sw x5 (x5 dist-2 store data)
    step(1);
    dut.instr = 0x00000013;      // c8+: nop
    step(1);
    assert_eq(dut.mem_wdata, 7); // sw x4 in MEM (cycle 9): 5 | 7
    step(1);
    assert_eq(dut.mem_wdata, 2); // sw x5 in MEM (cycle 10): 5 ^ 7
}

test "lw without dependency needs no stall" {
    let dut = RiscvPipeline { };
    dut.mem_rdata = 0xDEADBEEF;
    dut.instr = 0x00402083;      // c0: lw x1, 4(x0)
    step(1);
    dut.instr = 0x00000013;      // c1: nop
    step(1);
    assert_false(dut.stall_o);
    step(1);                     // lw in MEM during cycle 3
    assert_true(dut.mem_read);
    assert_eq(dut.mem_addr, 4);
    dut.instr = 0x00102023;      // c3: sw x1, 0(x0)
    step(1);
    dut.instr = 0x00000013;      // c4+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 0xDEADBEEF);
}

test "writes to x0 are ignored" {
    let dut = RiscvPipeline { };
    dut.instr = 0x00700013;      // c0: addi x0, x0, 7
    step(1);
    dut.instr = 0x00002023;      // c1: sw x0, 0(x0)
    step(1);
    dut.instr = 0x00000013;      // c2+: nop
    step(2);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 0);
}
