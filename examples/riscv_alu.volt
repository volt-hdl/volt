// RV32I integer ALU and branch condition, each a `match` on funct3
// (RISC-V spec, "Integer Computational Instructions" and "Conditional
// Branches"). Used by examples/riscv_core.volt.
//
// A library file: pure functions, no module (ADR-0042 addendum). A `fn`
// is pure combinational logic expanded at each call site (ADR-0081); a
// `match` expression that is the whole right-hand side of a wire lowers
// to a SystemVerilog `case` (ADR-0083). Numeric `match` needs a `_` arm
// (E0014): the arm under `_` is what the unused funct3 codes compute.

// OP / OP-IMM result. `is_r` is set for OP (register operand): bit 30
// then selects SUB; for SRA/SRAI it is set in both formats.
pub fn alu_of(f3: u3, is_r: bool, alt: bool, a: u32, b: u32) -> u32 {
    let shamt = b & 31
    match f3 {
        0 => if is_r && alt { a - b } else { a + b },          // ADD / SUB
        1 => a << shamt,                                      // SLL
        2 => if (a as i32) < (b as i32) { 1 } else { 0 },     // SLT
        3 => if a < b { 1 } else { 0 },                       // SLTU
        4 => a ^ b,                                           // XOR
        5 => if alt { ((a as i32) >> shamt) as u32 } else { a >> shamt }, // SRA / SRL
        6 => a | b,                                           // OR
        _ => a & b                                            // AND (7)
    }
}

// BRANCH condition. funct3 2 and 3 are not branches (the decoder traps
// them as illegal); `_` gives them BGEU's value, which is never used.
pub fn branch_taken(f3: u3, a: u32, b: u32) -> bool {
    match f3 {
        0 => a == b,                        // BEQ
        1 => a != b,                        // BNE
        4 => (a as i32) < (b as i32),       // BLT
        5 => (a as i32) >= (b as i32),      // BGE
        6 => a < b,                         // BLTU
        _ => a >= b                         // BGEU (7)
    }
}
