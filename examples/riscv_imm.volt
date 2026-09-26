// RISC-V immediate decoding, one function per format (RISC-V spec,
// Figure "Types of immediate produced by RISC-V instructions"). Shared by
// examples/riscv_core.volt and examples/riscv_pipeline.volt.
//
// A library file: pure functions, no module. Built on its own it writes
// no SystemVerilog (ADR-0042 addendum). A `fn` is pure combinational
// logic expanded at each call site (ADR-0081); the SystemVerilog has no
// function, only the expression. Sign extension goes through the
// arithmetic shift `(x as i32) >> n` (ADR-0036).

// I-type: inst[31:20], sign-extended.
pub fn imm_i_of(instr: u32) -> u32 {
    ((instr as i32) >> 20) as u32
}

// S-type: inst[31:25] | inst[11:7], sign-extended.
pub fn imm_s_of(instr: u32) -> u32 {
    ((((instr as i32) >> 25) << 5) as u32)
    | ((instr >> 7) & 0x1F)
}

// B-type: inst[31] | inst[7] | inst[30:25] | inst[11:8] | 0, sign-extended.
pub fn imm_b_of(instr: u32) -> u32 {
    ((((instr as i32) >> 31) << 12) as u32)
    | (((instr >> 7) & 1) << 11)
    | (((instr >> 25) & 0x3F) << 5)
    | (((instr >> 8) & 0xF) << 1)
}

// U-type: inst[31:12] | 12 zero bits.
pub fn imm_u_of(instr: u32) -> u32 {
    instr & 0xFFFFF000
}

// J-type: inst[31] | inst[19:12] | inst[20] | inst[30:21] | 0, sign-extended.
pub fn imm_j_of(instr: u32) -> u32 {
    ((((instr as i32) >> 31) << 20) as u32)
    | (instr & 0xFF000)
    | (((instr >> 20) & 1) << 11)
    | (((instr >> 21) & 0x3FF) << 1)
}
