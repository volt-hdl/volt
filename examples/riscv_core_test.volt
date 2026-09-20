// RV32IM + Zicsr core testbench — 59 tests: 26 for the base ISA, 11 for
// the M extension (every RISC-V division corner case), 7 for the CSRs,
// 9 for traps (ECALL/EBREAK/MRET/illegal/misaligned), 3 for the external
// interrupt, 2 for the memory-mapped UART, and 1 that runs a compiled C
// program (riscv_sw/hello.c) end to end.
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
//   0x00402023  sw   x4, 0(x0)
//   M extension, <op> x3, x1, x2 (funct7 = 1, f3 = 0..7):
//     mul 0x022081B3  mulh 0x022091B3  mulhsu 0x0220A1B3  mulhu 0x0220B1B3
//     div 0x0220C1B3  divu 0x0220D1B3  rem    0x0220E1B3  remu  0x0220F1B3
//
// A division holds the core for 34 cycles (latch, 32 steps, write-back);
// the held `instr` input plays the part of the instruction memory that
// keeps presenting the word at the stalled pc.

use riscv_sw::hello_soc::HelloSoc;

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

test "mul basic" {
    let dut = RiscvCore { };
    dut.instr = 0x00500093;      // addi x1, x0, 5
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x022081B3;      // mul x3, x1, x2
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 35);
}

test "mul overflow high bits" {
    let dut = RiscvCore { };
    dut.instr = 0x123450B7;      // lui x1, 0x12345
    step(1);
    dut.instr = 0x10000113;      // addi x2, x0, 256
    step(1);
    dut.instr = 0x022081B3;      // mul x3, x1, x2  (0x12_34500000)
    step(1);
    dut.instr = 0x02209233;      // mulh x4, x1, x2
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x34500000);
    dut.instr = 0x00402023;      // sw x4, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x12);
}

test "mulhu unsigned" {
    let dut = RiscvCore { };
    dut.instr = 0xFFF00093;      // addi x1, x0, -1
    step(1);
    dut.instr = 0xFFF00113;      // addi x2, x0, -1
    step(1);
    dut.instr = 0x0220B1B3;      // mulhu x3, x1, x2  ((2^32-1)^2)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFFE);
    dut.instr = 0x022091B3;      // mulh x3, x1, x2  (-1 * -1 = 1)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0);
    dut.instr = 0x0220A1B3;      // mulhsu x3, x1, x2  (-1 * (2^32-1))
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFFF);
}

test "div basic" {
    let dut = RiscvCore { };
    dut.instr = 0x06400093;      // addi x1, x0, 100
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x0220C1B3;      // div x3, x1, x2
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 14);
}

test "div signed truncates toward zero" {
    let dut = RiscvCore { };
    dut.instr = 0xF9C00093;      // addi x1, x0, -100
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x0220C1B3;      // div x3, x1, x2  (-100 / 7)
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFF2);
    dut.instr = 0x0220E1B3;      // rem x3, x1, x2  (sign of the dividend)
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFFE);
}

test "div by zero returns minus one" {
    let dut = RiscvCore { };
    dut.instr = 0x02A00093;      // addi x1, x0, 42
    step(1);
    dut.instr = 0x0220C1B3;      // div x3, x1, x2  (x2 == 0)
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFFF);
    dut.instr = 0xF9C00093;      // addi x1, x0, -100
    step(1);
    dut.instr = 0x0220C1B3;      // div x3, x1, x2  (-100 / 0: no sign fix)
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFFF);
}

test "divu by zero returns max" {
    let dut = RiscvCore { };
    dut.instr = 0x02A00093;      // addi x1, x0, 42
    step(1);
    dut.instr = 0x0220D1B3;      // divu x3, x1, x2  (x2 == 0)
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFFFF);
}

test "rem basic" {
    let dut = RiscvCore { };
    dut.instr = 0x06400093;      // addi x1, x0, 100
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x0220E1B3;      // rem x3, x1, x2
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 2);
    dut.instr = 0x0220F1B3;      // remu x3, x1, x2
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 2);
}

test "rem by zero returns dividend" {
    let dut = RiscvCore { };
    dut.instr = 0xF9C00093;      // addi x1, x0, -100
    step(1);
    dut.instr = 0x0220E1B3;      // rem x3, x1, x2  (x2 == 0)
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFF9C);
    dut.instr = 0x0220F1B3;      // remu x3, x1, x2
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0xFFFFFF9C);
}

test "div min by minus one overflow" {
    let dut = RiscvCore { };
    dut.instr = 0x800000B7;      // lui x1, 0x80000  (MIN)
    step(1);
    dut.instr = 0xFFF00113;      // addi x2, x0, -1
    step(1);
    dut.instr = 0x0220C1B3;      // div x3, x1, x2
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x80000000);
    dut.instr = 0x0220E1B3;      // rem x3, x1, x2
    step(34);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0);
}

test "div stalls pipeline correctly" {
    let dut = RiscvCore { };
    dut.instr = 0x06400093;      // addi x1, x0, 100
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    assert_eq(dut.pc, 8);
    assert_false(dut.stall_o);
    dut.instr = 0x0220C1B3;      // div x3, x1, x2
    step(1);
    assert_true(dut.stall_o);
    assert_eq(dut.pc, 8);
    step(31);
    assert_true(dut.stall_o);        // 32 cycles in: last divider step
    assert_eq(dut.pc, 8);
    step(1);
    assert_false(dut.stall_o);       // write-back cycle: result ready,
    assert_eq(dut.pc, 8);            // the instruction retires this cycle
    step(1);
    assert_eq(dut.pc, 12);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_false(dut.stall_o);
    assert_eq(dut.mem_wdata, 14);
    assert_eq(dut.pc, 16);
}

test "csr read write" {
    let dut = RiscvCore { };
    dut.instr = 0x7F000093;      // addi x1, x0, 0x7F0
    step(1);
    dut.instr = 0x30509073;      // csrrw x0, mtvec, x1
    step(1);
    dut.instr = 0x02A00093;      // addi x1, x0, 42
    step(1);
    dut.instr = 0x305091F3;      // csrrw x3, mtvec, x1  (swap: old value out)
    step(1);
    dut.instr = 0x30502273;      // csrrs x4, mtvec, x0  (plain read)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x7F0);
    dut.instr = 0x00402023;      // sw x4, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 40);  // 42 with the mode bits masked
}

test "csr set and clear bits" {
    let dut = RiscvCore { };
    dut.instr = 0x7F000093;      // addi x1, x0, 0x7F0
    step(1);
    dut.instr = 0x0F000113;      // addi x2, x0, 0x0F0
    step(1);
    dut.instr = 0x3420A073;      // csrrs x0, mcause, x1
    step(1);
    dut.instr = 0x34213073;      // csrrc x0, mcause, x2
    step(1);
    dut.instr = 0x342021F3;      // csrrs x3, mcause, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x700);
}

test "csr immediate variants" {
    let dut = RiscvCore { };
    dut.instr = 0x342AD073;      // csrrwi x0, mcause, 21
    step(1);
    dut.instr = 0x34256073;      // csrrsi x0, mcause, 10
    step(1);
    dut.instr = 0x3422F073;      // csrrci x0, mcause, 5
    step(1);
    dut.instr = 0x342021F3;      // csrrs x3, mcause, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 26);
}

test "mstatus keeps only mie and mpie" {
    let dut = RiscvCore { };
    dut.instr = 0xFFF00093;      // addi x1, x0, -1
    step(1);
    dut.instr = 0x30009073;      // csrrw x0, mstatus, x1  (all ones)
    step(1);
    dut.instr = 0x300021F3;      // csrrs x3, mstatus, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x1888);  // MPP = 0b11 | MPIE | MIE
}

test "mcycle increments" {
    let dut = RiscvCore { };
    dut.instr = 0x00000013;      // nop
    step(3);
    dut.instr = 0xB00021F3;      // csrrs x3, mcycle, x0  (cycle 3)
    step(1);
    dut.instr = 0xB0002273;      // csrrs x4, mcycle, x0  (cycle 4)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 3);
    dut.instr = 0x00402023;      // sw x4, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 4);
}

test "counter writes are ignored" {
    let dut = RiscvCore { };
    dut.instr = 0xFFF00093;      // addi x1, x0, -1
    step(1);
    dut.instr = 0xB0009073;      // csrrw x0, mcycle, x1
    step(1);
    dut.instr = 0xB00021F3;      // csrrs x3, mcycle, x0  (cycle 2)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 2);
}

test "minstret counts instructions" {
    let dut = RiscvCore { };
    dut.instr = 0x06400093;      // addi x1, x0, 100
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x0220C1B3;      // div x3, x1, x2  (cycles 2..35, one instruction)
    step(34);
    dut.instr = 0xB02021F3;      // csrrs x3, minstret, x0  (cycle 36)
    step(1);
    dut.instr = 0xB0002273;      // csrrs x4, mcycle, x0  (cycle 37)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 3);
    dut.instr = 0x00402023;      // sw x4, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 37);
}

// ── Traps ─────────────────────────────────────────────────────────

test "ecall sets mcause 11" {
    let dut = RiscvCore { };
    dut.instr = 0x00000073;      // ecall
    step(1);
    dut.instr = 0x342021F3;      // csrrs x3, mcause, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 11);
}

test "ecall saves pc to mepc" {
    let dut = RiscvCore { };
    dut.instr = 0x00000013;      // nop, nop
    step(2);
    dut.instr = 0x00000073;      // ecall  (at pc == 8)
    step(1);
    assert_true(dut.trap_o);
    dut.instr = 0x341021F3;      // csrrs x3, mepc, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 8);
}

test "ecall jumps to mtvec" {
    let dut = RiscvCore { };
    dut.instr = 0x10000093;      // addi x1, x0, 0x100
    step(1);
    dut.instr = 0x30509073;      // csrrw x0, mtvec, x1
    step(1);
    dut.instr = 0x00000073;      // ecall
    step(1);
    assert_eq(dut.pc, 0x100);
}

test "ebreak sets mcause 3" {
    let dut = RiscvCore { };
    dut.instr = 0x00100073;      // ebreak
    step(1);
    dut.instr = 0x342021F3;      // csrrs x3, mcause, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 3);
}

test "mret returns to mepc" {
    let dut = RiscvCore { };
    dut.instr = 0x04000093;      // addi x1, x0, 0x40
    step(1);
    dut.instr = 0x34109073;      // csrrw x0, mepc, x1
    step(1);
    dut.instr = 0x30200073;      // mret
    step(1);
    assert_eq(dut.pc, 0x40);
}

test "mret restores mie" {
    let dut = RiscvCore { };
    dut.instr = 0x30045073;      // csrrwi x0, mstatus, 8  (MIE = 1)
    step(1);
    dut.instr = 0x00000073;      // ecall: MPIE <- MIE, MIE <- 0
    step(1);
    dut.instr = 0x300021F3;      // csrrs x3, mstatus, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x1880);  // MPP | MPIE
    dut.instr = 0x30200073;      // mret: MIE <- MPIE, MPIE <- 1
    step(1);
    dut.instr = 0x300021F3;      // csrrs x3, mstatus, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x1888);  // MPP | MPIE | MIE
}

test "illegal instruction traps" {
    let dut = RiscvCore { };
    dut.instr = 0x10000093;      // addi x1, x0, 0x100
    step(1);
    dut.instr = 0x30509073;      // csrrw x0, mtvec, x1
    step(1);
    dut.instr = 0xFFFFFFFF;      // not an instruction  (at pc == 8)
    step(1);
    assert_eq(dut.pc, 0x100);
    dut.instr = 0x342021F3;      // csrrs x3, mcause, x0
    step(1);
    dut.instr = 0x34102273;      // csrrs x4, mepc, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 2);
    dut.instr = 0x00402023;      // sw x4, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 8);
}

test "misaligned load traps" {
    let dut = RiscvCore { };
    dut.instr = 0x00000013;      // nop
    step(1);
    dut.instr = 0x0010A183;      // lw x3, 1(x0)  (at pc == 4)
    step(1);
    assert_false(dut.mem_read);  // the access never reaches the bus
    assert_eq(dut.pc, 0);        // mtvec
    dut.instr = 0x342021F3;      // csrrs x3, mcause, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 4);
    dut.instr = 0x00102123;      // sw x1, 2(x0)  (misaligned store)
    step(1);
    assert_false(dut.mem_write);
    dut.instr = 0x342021F3;      // csrrs x3, mcause, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 6);
}

test "misaligned jump traps without linking" {
    let dut = RiscvCore { };
    dut.instr = 0x10000093;      // addi x1, x0, 0x100
    step(1);
    dut.instr = 0x30509073;      // csrrw x0, mtvec, x1
    step(1);
    dut.instr = 0x002000EF;      // jal x1, +2  (at pc == 8, target bit 1 set)
    step(1);
    assert_eq(dut.pc, 0x100);
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x100);  // x1 kept its value: no link
    dut.instr = 0x341021F3;      // csrrs x3, mepc, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 8);
}

// ── Interrupts ────────────────────────────────────────────────────
//   0x10000293  addi  x5, x0, 0x100     0x30529073  csrrw x0, mtvec, x5
//   0xFFF00293  addi  x5, x0, -1        0x30429073  csrrw x0, mie, x5
//   0x30045073  csrrwi x0, mstatus, 8   (MIE = 1)

test "interrupt taken when mie set" {
    let dut = RiscvCore { };
    dut.instr = 0x10000293;      // addi x5, x0, 0x100
    step(1);
    dut.instr = 0x30529073;      // csrrw x0, mtvec, x5
    step(1);
    dut.instr = 0xFFF00293;      // addi x5, x0, -1
    step(1);
    dut.instr = 0x30429073;      // csrrw x0, mie, x5  (MEIE sticks)
    step(1);
    dut.instr = 0x30045073;      // csrrwi x0, mstatus, 8
    step(1);
    assert_eq(dut.pc, 20);
    dut.instr = 0x00500093;      // addi x1, x0, 5  (displaced by the irq)
    dut.irq = true;
    step(1);
    assert_eq(dut.pc, 0x100);
    assert_false(dut.irq_ack_o); // MIE is off now: the held line does not nest
    dut.instr = 0x342021F3;      // csrrs x3, mcause, x0
    step(1);
    dut.instr = 0x34102273;      // csrrs x4, mepc, x0
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x8000000B);
    dut.instr = 0x00402023;      // sw x4, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 20);    // the instruction that did not run
    dut.instr = 0x00102023;      // sw x1, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0);     // ... and really did not run
}

test "interrupt ignored when mie clear" {
    let dut = RiscvCore { };
    dut.instr = 0xFFF00293;      // addi x5, x0, -1
    step(1);
    dut.instr = 0x30429073;      // csrrw x0, mie, x5  (MEIE on, MIE off)
    step(1);
    dut.instr = 0x00000013;      // nop
    dut.irq = true;
    step(2);
    assert_false(dut.irq_ack_o);
    assert_eq(dut.pc, 16);
    dut.instr = 0x344021F3;      // csrrs x3, mip, x0  (pending all the same)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0x800);
}

test "interrupt waits during division" {
    let dut = RiscvCore { };
    dut.instr = 0x06400093;      // addi x1, x0, 100
    step(1);
    dut.instr = 0x00700113;      // addi x2, x0, 7
    step(1);
    dut.instr = 0x10000293;      // addi x5, x0, 0x100
    step(1);
    dut.instr = 0x30529073;      // csrrw x0, mtvec, x5
    step(1);
    dut.instr = 0xFFF00293;      // addi x5, x0, -1
    step(1);
    dut.instr = 0x30429073;      // csrrw x0, mie, x5
    step(1);
    dut.instr = 0x30045073;      // csrrwi x0, mstatus, 8
    step(1);
    assert_eq(dut.pc, 28);
    dut.instr = 0x0220C1B3;      // div x3, x1, x2
    step(1);                     // operands latched, divider running
    dut.irq = true;
    step(10);
    assert_true(dut.stall_o);
    assert_false(dut.irq_ack_o);
    assert_eq(dut.pc, 28);
    step(22);                    // 33 cycles in: the write-back cycle
    assert_false(dut.stall_o);
    assert_false(dut.irq_ack_o);
    assert_eq(dut.pc, 28);
    step(1);                     // division retired
    assert_eq(dut.pc, 32);
    assert_true(dut.irq_ack_o);  // boundary reached: now it is taken
    step(1);
    assert_eq(dut.pc, 0x100);
    dut.irq = false;
    dut.instr = 0x34102273;      // csrrs x4, mepc, x0
    step(1);
    dut.instr = 0x00402023;      // sw x4, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 32);    // the instruction after the div
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 14);    // the quotient survived
}

// ── Memory-mapped I/O ─────────────────────────────────────────────
//   0x200000B7  lui x1, 0x20000        0x05500113  addi x2, x0, 0x55
//   0x00208023  sb  x2, 0(x1)          0x0040A183  lw   x3, 4(x1)
// UartTx runs at 4 clocks per bit: 0x55 goes out LSB first as 1,0,1,0,...

test "uart write goes to peripheral" {
    let dut = RiscvCore { };
    dut.instr = 0x200000B7;      // lui x1, 0x20000
    step(1);
    dut.instr = 0x05500113;      // addi x2, x0, 0x55
    step(1);
    assert_true(dut.uart_txd);   // line idles high
    dut.instr = 0x00208023;      // sb x2, 0(x1)
    step(1);
    assert_false(dut.mem_write); // an I/O store stays off the memory bus
    assert_eq(dut.mem_wmask, 0);
    dut.instr = 0x00000013;      // nop
    step(1);
    assert_false(dut.uart_txd);  // start bit
    step(4);
    assert_true(dut.uart_txd);   // data bit 0 of 0x55
    step(4);
    assert_false(dut.uart_txd);  // data bit 1
    step(4);
    assert_true(dut.uart_txd);   // data bit 2
}

test "uart status readable" {
    let dut = RiscvCore { };
    dut.mem_rdata = 0xFFFFFFFF;  // what the memory bus would answer
    dut.instr = 0x200000B7;      // lui x1, 0x20000
    step(1);
    dut.instr = 0x0040A183;      // lw x3, 4(x1)
    step(1);
    assert_false(dut.mem_read);  // an I/O load stays off the memory bus
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0); // idle
    dut.instr = 0x00208023;      // sb x2, 0(x1)
    step(1);
    dut.instr = 0x0040A183;      // lw x3, 4(x1)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 1); // busy
    dut.instr = 0x00000013;      // nop: let the 40-cycle frame drain
    step(45);
    dut.instr = 0x0040A183;      // lw x3, 4(x1)
    step(1);
    dut.instr = 0x00302023;      // sw x3, 0(x0)
    step(1);
    assert_eq(dut.mem_wdata, 0); // idle again
}

// ── A real C program ──────────────────────────────────────────────
// riscv_sw/hello.c, compiled with GCC (-march=rv32im -O2) and linked
// into riscv_sw/hello_rom.volt. HelloSoc (riscv_sw/hello_soc.volt)
// wraps the core with that ROM, a RAM and a UART receiver, because a
// test block cannot load memory or collect serial bytes on its own:
// the receiver stores every byte and the test pages through them with
// rx_sel (one step lets the selected byte settle; the program has
// halted by then, so extra cycles change nothing).
// The program prints the greeting, then 7 * 6 as "42" — MUL for the
// product, DIV and REM for the two digits — one 40-cycle frame per byte.

test "hello program prints greeting" {
    let dut = HelloSoc { };
    step(5000);
    assert_true(dut.halted);         // reached `halt: j halt` ...
    assert_false(dut.trap_seen);     // ... without a single trap
    assert_false(dut.unexpected);    // ... or a stray bus access
    assert_eq(dut.rx_count, 20);     // "Hello from Volt!\n42\n"
    dut.rx_sel = 0;   step(1);  assert_eq(dut.rx_byte, 0x48);   // H
    dut.rx_sel = 1;   step(1);  assert_eq(dut.rx_byte, 0x65);   // e
    dut.rx_sel = 2;   step(1);  assert_eq(dut.rx_byte, 0x6C);   // l
    dut.rx_sel = 3;   step(1);  assert_eq(dut.rx_byte, 0x6C);   // l
    dut.rx_sel = 4;   step(1);  assert_eq(dut.rx_byte, 0x6F);   // o
    dut.rx_sel = 5;   step(1);  assert_eq(dut.rx_byte, 0x20);   // space
    dut.rx_sel = 6;   step(1);  assert_eq(dut.rx_byte, 0x66);   // f
    dut.rx_sel = 7;   step(1);  assert_eq(dut.rx_byte, 0x72);   // r
    dut.rx_sel = 8;   step(1);  assert_eq(dut.rx_byte, 0x6F);   // o
    dut.rx_sel = 9;   step(1);  assert_eq(dut.rx_byte, 0x6D);   // m
    dut.rx_sel = 10;  step(1);  assert_eq(dut.rx_byte, 0x20);   // space
    dut.rx_sel = 11;  step(1);  assert_eq(dut.rx_byte, 0x56);   // V
    dut.rx_sel = 12;  step(1);  assert_eq(dut.rx_byte, 0x6F);   // o
    dut.rx_sel = 13;  step(1);  assert_eq(dut.rx_byte, 0x6C);   // l
    dut.rx_sel = 14;  step(1);  assert_eq(dut.rx_byte, 0x74);   // t
    dut.rx_sel = 15;  step(1);  assert_eq(dut.rx_byte, 0x21);   // !
    dut.rx_sel = 16;  step(1);  assert_eq(dut.rx_byte, 0x0A);   // newline
    dut.rx_sel = 17;  step(1);  assert_eq(dut.rx_byte, 0x34);   // 4  (42 / 10)
    dut.rx_sel = 18;  step(1);  assert_eq(dut.rx_byte, 0x32);   // 2  (42 % 10)
    dut.rx_sel = 19;  step(1);  assert_eq(dut.rx_byte, 0x0A);   // newline
    // Program-specific: 847 instructions in 913 cycles — the DIV and
    // the REM each hold the core for 33 extra cycles.
    assert_eq(dut.cycles, 913);
    assert_eq(dut.instrs, 847);
}
