// RV32I core testbench — 25 tests covering all instruction classes.
//
// Observation strategy: the register file is internal, so register
// values are read back through the memory interface — execute a
// `sw rd, 0(x0)` and assert on mem_wdata. The pc output is asserted
// directly. Inputs hold their value between steps, so combinational
// outputs (mem_write, mem_wdata, ...) are asserted after the step
// that executed the driving instruction.
//
// Frequently used encodings:
//   0x00000013  nop            (addi x0, x0, 0)
//   0x00500093  addi x1, x0, 5
//   0x00700113  addi x2, x0, 7
//   0x00102023  sw   x1, 0(x0)
//   0x00302023  sw   x3, 0(x0)

test "reset drives pc to zero" {
    let dut = RiscvCore { };
    assert_eq(dut.pc, 0);
    assert_false(dut.mem_write);
}

test "nop advances pc by four" {
    let dut = RiscvCore { };
    dut.instr = 0x00000013;
    step(1);
    assert_eq(dut.pc, 4);
    step(1);
    assert_eq(dut.pc, 8);
}

test "addi writes register" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_true(dut.mem_write);
    assert_eq(dut.mem_wdata, 5);
    assert_eq(dut.mem_wmask, 15);
}

test "writes to x0 are ignored" {
    let dut = RiscvCore { };
    dut.instr = 0x00700013;      // addi x0, x0, 7
    step(1);
    dut.instr = 0x00002023;      // sw x0, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0);
}

test "add sums two registers" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x002081B3;      // add x3, x1, x2
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 12);
}

test "sub wraps below zero" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x402081B3;      // sub x3, x1, x2
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFFE);
}

test "and masks bits" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x0020F1B3;      // and x3, x1, x2
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 5);
}

test "or merges bits" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x0020E1B3;      // or x3, x1, x2
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 7);
}

test "xor finds differing bits" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x0020C1B3;      // xor x3, x1, x2
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 2);
}

test "slt compares signed" {
    let dut = RiscvCore { };
    dut.instr = 0xFFE00093;      // addi x1, x0, -2
    step(1);
    dut.instr = 0x00100113;      // addi x2, x0, 1
    step(1);
    dut.instr = 0x0020A1B3;      // slt x3, x1, x2  (-2 < 1)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 1);
}

test "sltu compares unsigned" {
    let dut = RiscvCore { };
    dut.instr = 0xFFE00093;      // addi x1, x0, -2  (0xFFFFFFFE)
    step(1);
    dut.instr = 0x00100113;      // addi x2, x0, 1
    step(1);
    dut.instr = 0x0020B1B3;      // sltu x3, x1, x2  (big >u 1)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0);
}

test "slli shifts left" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00409193;      // slli x3, x1, 4
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 80);
}

test "srli shifts in zeros" {
    let dut = RiscvCore { };
    dut.instr = 0xFFE00093;      // addi x1, x0, -2
    step(1);
    dut.instr = 0x0040D193;      // srli x3, x1, 4
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x0FFFFFFF);
}

test "srai keeps the sign" {
    let dut = RiscvCore { };
    dut.instr = 0xFFE00093;      // addi x1, x0, -2
    step(1);
    dut.instr = 0x4040D193;      // srai x3, x1, 4
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFFF);
}

test "lui loads upper immediate" {
    let dut = RiscvCore { };
    dut.instr = 0x123450B7;      // lui x1, 0x12345
    step(1);
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x12345000);
}

test "auipc adds pc to upper immediate" {
    let dut = RiscvCore { };
    dut.instr = 0x00001097;      // auipc x1, 0x1  (pc == 0)
    step(1);
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x1000);
}

test "jal jumps and links" {
    let dut = RiscvCore { };
    dut.instr = 0x008000EF;      // jal x1, +8  (from pc == 0)
    step(1);
    assert_eq(dut.pc, 8);
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 4); // link = pc of jal + 4
}

test "jalr jumps via register and links" {
    let dut = RiscvCore { };
    dut.instr = 0x06400113;      // addi x2, x0, 100
    step(1);
    dut.instr = 0x000100E7;      // jalr x1, 0(x2)  (at pc == 4)
    step(1);
    assert_eq(dut.pc, 100);
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 8); // link = pc of jalr + 4
}

test "beq taken skips ahead" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00500113;      // addi x2, x0, 5
    step(1);
    dut.instr = 0x00208463;      // beq x1, x2, +8  (at pc == 8)
    step(1);
    assert_eq(dut.pc, 16);
}

test "bne not taken falls through" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00500113;      // addi x2, x0, 5
    step(1);
    dut.instr = 0x00209463;      // bne x1, x2, +8  (at pc == 8)
    step(1);
    assert_eq(dut.pc, 12);
}

test "blt taken on signed compare" {
    let dut = RiscvCore { };
    dut.instr = 0xFFE00093;      // addi x1, x0, -2
    step(1);
    dut.instr = 0x00100113;      // addi x2, x0, 1
    step(1);
    dut.instr = 0x0020C463;      // blt x1, x2, +8  (-2 < 1, pc == 8)
    step(1);
    assert_eq(dut.pc, 16);
}

test "bgeu taken on unsigned compare" {
    let dut = RiscvCore { };
    dut.instr = 0xFFE00093;      // addi x1, x0, -2  (0xFFFFFFFE)
    step(1);
    dut.instr = 0x00100113;      // addi x2, x0, 1
    step(1);
    dut.instr = 0x0020F463;      // bgeu x1, x2, +8  (big >=u 1)
    step(1);
    assert_eq(dut.pc, 16);
}

test "lw reads a full word" {
    let dut = RiscvCore { };
    dut.mem_rdata = 0xDEADBEEF;
    dut.instr = 0x00002083;      // lw x1, 0(x0)
    step(1);
    assert_true(dut.mem_read);
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xDEADBEEF);
}

test "lb sign extends lbu does not" {
    let dut = RiscvCore { };
    dut.mem_rdata = 0x00000080;
    dut.instr = 0x00000083;      // lb x1, 0(x0)
    step(1);
    dut.instr = 0x00004103;      // lbu x2, 0(x0)
    step(1);
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFF80);
    dut.instr = 0x00202023;      // sw x2, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x80);
}

test "lh sign extends lhu does not" {
    let dut = RiscvCore { };
    dut.mem_rdata = 0x00008000;
    dut.instr = 0x00001183;      // lh x3, 0(x0)
    step(1);
    dut.instr = 0x00005203;      // lhu x4, 0(x0)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFF8000);
    dut.instr = 0x00402023;      // sw x4, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x8000);
}

test "sb and sh drive the addressed lanes" {
    let dut = RiscvCore { };
    dut.instr = 0x0AB00093;      // addi x1, x0, 171  (0xAB)
    step(1);
    dut.instr = 0x001000A3;      // sb x1, 1(x0)
    step(1);
    assert_eq(dut.mem_wmask, 2);
    assert_eq(dut.mem_wdata, 0xAB00);
    dut.instr = 0x00101123;      // sh x1, 2(x0)
    step(1);
    assert_eq(dut.mem_wmask, 12);
    assert_eq(dut.mem_wdata, 0xAB0000);
}
