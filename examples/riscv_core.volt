// RV32I single-cycle core — all 37 base-ISA instructions.
//
// Rewritten with ADR-0035 features: the register file is one
// `reg regs : [u32; 32]` with dynamic indexing (regs[rs1_i], regs[rd_i])
// instead of 32 separate registers plus two 32-arm match blocks, and
// sub-word loads use an indexed part-select (mem_rdata[off8 +: 8])
// instead of shift-and-mask workarounds.
//
// Memory model: combinational (single-cycle) instruction and data
// ports. `instr` is the instruction at address `pc`; `mem_rdata` is
// the word at `mem_addr` in the same cycle. Stores drive `mem_wdata`
// shifted into the addressed byte lane with `mem_wmask` byte enables
// (bit i = byte lane i), so SB/SH/SW all present a full 32-bit word.
//
// x0 is hardwired to zero the RISC-V way: writes to rd == 0 are
// suppressed, so regs[0] never leaves its reset value. This is now a
// provable one-line contract (invariant: regs[0] == 0) instead of a
// convention buried in a 32-arm match.
//
// Sign extension avoids arithmetic right shift on purpose: `>>` maps
// to the logical SV operator, so immediates OR in the upper sign mask
// when instr[31] is set, SRA fills with ~(ones >> shamt), and signed
// compares use the bias trick (a ^ 0x80000000).

module RiscvCore {
    in  clk       : clock
    in  instr     : u32
    in  mem_rdata : u32
    out pc        : u32
    out mem_addr  : u32
    out mem_wdata : u32
    out mem_wmask : u8
    out mem_write : bool
    out mem_read  : bool

    // x0 reads as zero forever — the guard `rd_i != 0` below keeps
    // this inductive from the reset state [0; 32].
    invariant: regs[0] == 0
    // Instructions are 4-byte aligned in RV32I (no compressed
    // extension): +4 keeps bit 0, branch/JAL immediates have bit 0
    // hardwired zero, and JALR masks it explicitly.
    invariant: !pc_r[0]
    // Reachability: the memory interface actually fires.
    cover: mem_write
    cover: mem_read
    cover: pc_r != 0

    reg pc_r : u32 = 0
    reg regs : [u32; 32] = [0; 32]

    // ── Decode ────────────────────────────────────────────────────
    let opcode = instr[6:0] as u7
    let rd_i   = instr[11:7] as u5
    let f3     = instr[14:12] as u3
    let rs1_i  = instr[19:15] as u5
    let rs2_i  = instr[24:20] as u5

    let is_lui    = opcode == 0x37
    let is_auipc  = opcode == 0x17
    let is_jal    = opcode == 0x6F
    let is_jalr   = opcode == 0x67
    let is_branch = opcode == 0x63
    let is_load   = opcode == 0x03
    let is_store  = opcode == 0x23
    let is_alu_i  = opcode == 0x13
    let is_alu_r  = opcode == 0x33

    // ── Register file read (dynamic indexing, ADR-0035) ───────────
    let rs1_v = regs[rs1_i]
    let rs2_v = regs[rs2_i]

    // ── Immediates (manual sign extension via upper-mask OR) ──────
    let imm_i = if instr[31] { (instr >> 20) | 0xFFFFF000 }
                else         { instr >> 20 }

    let imm_s_lo = ((instr >> 25) << 5) | ((instr >> 7) & 0x1F)
    let imm_s    = if instr[31] { imm_s_lo | 0xFFFFF000 } else { imm_s_lo }

    let imm_b_lo = (((instr >> 8) & 0xF) << 1)
                 | (((instr >> 25) & 0x3F) << 5)
                 | (((instr >> 7) & 1) << 11)
    let imm_b    = if instr[31] { imm_b_lo | 0xFFFFF000 } else { imm_b_lo }

    let imm_u = instr & 0xFFFFF000

    let imm_j_lo = (((instr >> 21) & 0x3FF) << 1)
                 | (((instr >> 20) & 1) << 11)
                 | (instr & 0xFF000)
    let imm_j    = if instr[31] { imm_j_lo | 0xFFF00000 } else { imm_j_lo }

    // ── ALU ───────────────────────────────────────────────────────
    let alu_b = if is_alu_r { rs2_v } else { imm_i }
    let shamt = alu_b & 31
    // Bit 30 selects SUB (R-type f3=0) and SRA (f3=5, R and I alike).
    let alt_op = instr[30]

    // Signed compare via bias: (a ^ MIN) <u (b ^ MIN)  ==  a <s b.
    let sa  = rs1_v ^ 0x80000000
    let sb  = alu_b ^ 0x80000000    // SLT(I): rs2 or immediate
    let sbr = rs2_v ^ 0x80000000    // branches always compare rs2

    // Arithmetic shift fill: ones above the shifted-in position when
    // the sign bit is set; zero otherwise (shamt == 0 → fill == 0).
    let ones : u32 = 0xFFFFFFFF
    let sra_fill = if rs1_v[31] { ~(ones >> shamt) } else { 0 }

    let alu_out =
        if f3 == 0 {
            if is_alu_r && alt_op { rs1_v - alu_b } else { rs1_v + alu_b }
        } else if f3 == 1 { rs1_v << shamt
        } else if f3 == 2 { if sa < sb { 1 } else { 0 }          // SLT(I)
        } else if f3 == 3 { if rs1_v < alu_b { 1 } else { 0 }    // SLTU(I)
        } else if f3 == 4 { rs1_v ^ alu_b
        } else if f3 == 5 {
            if alt_op { (rs1_v >> shamt) | sra_fill } else { rs1_v >> shamt }
        } else if f3 == 6 { rs1_v | alu_b
        } else { rs1_v & alu_b }

    // ── Branch decision ───────────────────────────────────────────
    let br_taken =
        if f3 == 0 { rs1_v == rs2_v            // BEQ
        } else if f3 == 1 { rs1_v != rs2_v     // BNE
        } else if f3 == 4 { sa < sbr           // BLT
        } else if f3 == 5 { sa >= sbr          // BGE
        } else if f3 == 6 { rs1_v < rs2_v      // BLTU
        } else { rs1_v >= rs2_v }              // BGEU

    // ── Data memory interface ─────────────────────────────────────
    let addr = rs1_v + (if is_store { imm_s } else { imm_i })
    let off8 = (addr & 3) << 3    // byte lane → bit offset

    // Sub-word extraction with a variable-start part-select
    // (ADR-0035) — previously a shift-and-mask detour. The halfword
    // offset is inlined so no wire carries lint-unused upper bits.
    let lb_u = mem_rdata[off8 +: 8] as u8
    let lb_s = lb_u as i8
    let lh_u = mem_rdata[((addr & 2) << 3) +: 16] as u16
    let lh_s = lh_u as i16

    let load_val =
        if f3 == 0 { (lb_s as i32) as u32      // LB
        } else if f3 == 1 { (lh_s as i32) as u32  // LH
        } else if f3 == 2 { mem_rdata          // LW
        } else if f3 == 4 { lb_u as u32        // LBU
        } else { lh_u as u32 }                 // LHU

    // ── Write-back value ──────────────────────────────────────────
    let wb_val =
        if is_lui { imm_u
        } else if is_auipc { pc_r + imm_u
        } else if is_jal || is_jalr { pc_r + 4
        } else if is_load { load_val
        } else { alu_out }

    let wb_en = is_lui || is_auipc || is_jal || is_jalr
             || is_load || is_alu_i || is_alu_r

    // ── State update ──────────────────────────────────────────────
    on clk {
        if is_branch {
            if br_taken {
                pc_r <= pc_r + imm_b
            } else {
                pc_r <= pc_r + 4
            }
        } else if is_jal {
            pc_r <= pc_r + imm_j
        } else if is_jalr {
            pc_r <= (rs1_v + imm_i) & 0xFFFFFFFE
        } else {
            pc_r <= pc_r + 4
        }

        // Dynamic indexed write (ADR-0035); rd_i != 0 preserves x0.
        if wb_en {
            if rd_i != 0 {
                regs[rd_i] <= wb_val
            }
        }
    }

    // ── Outputs ───────────────────────────────────────────────────
    pc        = pc_r
    mem_addr  = if is_store || is_load { addr } else { 0 }
    mem_read  = is_load
    mem_write = is_store
    mem_wdata = rs2_v << off8
    mem_wmask =
        if !is_store { 0
        } else if f3 == 0 { 1u8 << (addr & 3)   // SB
        } else if f3 == 1 { 3u8 << (addr & 2)   // SH
        } else { 15 }                           // SW
}
