// RV32IM + Zicsr core — 37 base-ISA instructions, the 8 M-extension
// instructions and the 6 CSR instructions over a minimal machine-mode
// CSR set. Single-cycle, except division.
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
// Signed operations use the native forms (ADR-0036): immediates and
// SRA sign-extend through `(x as i32) >> n`, which emits the SV
// arithmetic shift (`$signed(x) >>> n`), and SLT/BLT compare through
// `(a as i32) < (b as i32)`, which emits `$signed(a) < $signed(b)`.
//
// M extension. MUL/MULH/MULHSU/MULHU are single-cycle: one unsigned
// 32x32 -> 64 multiplier (a DSP inference target) whose high word is
// corrected for the signed variants (hi - a31*b - b31*a), so the
// three MULH flavours share the multiplier. DIV/DIVU/REM/REMU are
// multi-cycle: a 1-bit-per-cycle restoring divider over magnitudes,
// 34 cycles per division (latch, 32 steps, write-back). While it
// runs `stall_o` is high, pc holds and nothing is written; the
// instruction memory must keep presenting the same `instr`. A
// combinational 32-bit divider would put a ~32-deep subtract chain on
// the critical path of every instruction.
// The RISC-V corner cases fall out of the datapath: a zero divisor
// never fails the trial subtraction, so the quotient fills with ones
// and the remainder collects the dividend (x/0 = -1, x%0 = x); the
// quotient sign fix is skipped for a zero divisor; MIN / -1 has
// magnitude 2^31 / 1, whose negation wraps back to MIN, remainder 0.
//
// CSRs are hand-written, not `@mmio`: @mmio generates an AXI4-Lite
// valid/ready slave with a registered read, while a CSR access is an
// atomic read-modify-write inside one instruction, addressed by a
// 12-bit number, next to hardware-updated counters.
//   mstatus 0x300  MIE/MPIE writable, MPP reads 0b11 (M-mode only)
//   mtvec   0x305  direct mode only (low two bits read zero)
//   mepc    0x341  4-byte aligned (no C extension)
//   mcause  0x342
//   mcycle  0xB00/0xB80, minstret 0xB02/0xB82  64-bit, READ-ONLY here
//     (the spec makes them writable; a software write would break
//     minstret <= mcycle, so writes are ignored in this core).
// Unmapped CSR numbers read zero and ignore writes; nothing traps
// yet (ECALL/MRET/interrupts are a later round), so mtvec/mepc/mcause
// are plain storage for now.

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
    out stall_o   : bool
    out cyc_wrap_o : bool

    // x0 reads as zero forever — the guard `rd_i != 0` below keeps
    // this inductive from the reset state [0; 32].
    invariant: regs[0] == 0
    // Instructions are 4-byte aligned in RV32I (no compressed
    // extension): +4 keeps bit 0, branch/JAL immediates have bit 0
    // hardwired zero, and JALR masks it explicitly.
    invariant: !pc_r[0]
    // Counters: mcycle ticks every cycle (0 = reset or 2^64 wrap),
    // and no more instructions retire than cycles elapse — until
    // mcycle itself wraps, which the sticky `cyc_wrap` records so the
    // bound stays inductive (contracts see only ports and registers,
    // hence a real flop with a debug port, as in riscv_pipeline).
    invariant: mcycle != 0 -> mcycle == prev(mcycle) + 1
    invariant: !cyc_wrap -> minstret <= mcycle
    invariant: !mepc[0] && !mepc[1]
    // Divider control: busy counts 0..31, done sits at 32.
    invariant: !(div_busy && div_done)
    invariant: div_busy -> div_cnt < 32
    invariant: div_done -> div_cnt == 32
    // Restoring-division remainder bound.
    invariant: (div_busy || div_done) && div_d != 0 -> div_r < div_d
    // Division by zero, step by step: after `div_cnt` steps the
    // quotient holds that many ones and the remainder holds the
    // dividend's top `div_cnt` bits ...
    invariant: (div_busy || div_done) && div_d == 0
        -> div_q == (1u32 << div_cnt) - 1
    invariant: (div_busy || div_done) && div_d == 0
        -> div_r == (div_n >> (32 - div_cnt))
    // ... so the result matches the spec: DIVU x/0 = 2^32-1 (DIV: -1,
    // no sign fix on a zero divisor), REMU/REM x/0 = x.
    invariant: div_done && div_d == 0 -> div_q == 0xFFFFFFFF
    invariant: div_done && div_d == 0 -> div_r == div_n
    // Reachability: the memory interface actually fires.
    cover: mem_write
    cover: mem_read
    cover: pc_r != 0
    // A MUL executed; a division by zero ran to completion.
    cover: (instr[6:0] as u7) == 0x33 && (instr[31:25] as u7) == 1
        && (instr[14:12] as u3) == 0
    cover: div_done && div_d == 0

    reg pc_r : u32 = 0
    reg regs : [u32; 32] = [0; 32]

    // Divider state: |dividend|, |divisor|, quotient and remainder.
    reg div_busy : bool = false
    reg div_done : bool = false
    reg div_cnt  : u6  = 0
    reg div_n    : u32 = 0
    reg div_d    : u32 = 0
    reg div_q    : u32 = 0
    reg div_r    : u32 = 0

    // Machine-mode CSRs.
    reg mstatus  : u32 = 0
    reg mtvec    : u32 = 0
    reg mepc     : u32 = 0
    reg mcause   : u32 = 0
    reg mcycle   : u64 = 0
    reg minstret : u64 = 0
    reg cyc_wrap : bool = false

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
    // M extension: R-type with funct7 == 1; f3[2] splits MUL* / DIV*.
    let is_m      = is_alu_r && (instr >> 25) == 1
    let is_mul    = is_m && !f3[2]
    let is_div    = is_m && f3[2]
    // Zicsr: SYSTEM opcode with f3 != 0 (f3 == 0 is ECALL/MRET, a NOP here).
    let is_csr    = opcode == 0x73 && f3 != 0

    // ── Register file read (dynamic indexing, ADR-0035) ───────────
    let rs1_v = regs[rs1_i]
    let rs2_v = regs[rs2_i]

    // ── Immediates (sign extension via arithmetic shift) ──────────
    let imm_i = ((instr as i32) >> 20) as u32

    let imm_s = ((((instr as i32) >> 25) << 5) as u32)
              | ((instr >> 7) & 0x1F)

    let imm_b = ((((instr as i32) >> 31) << 12) as u32)
              | (((instr >> 7) & 1) << 11)
              | (((instr >> 25) & 0x3F) << 5)
              | (((instr >> 8) & 0xF) << 1)

    let imm_u = instr & 0xFFFFF000

    let imm_j = ((((instr as i32) >> 31) << 20) as u32)
              | (instr & 0xFF000)
              | (((instr >> 20) & 1) << 11)
              | (((instr >> 21) & 0x3FF) << 1)

    // ── ALU ───────────────────────────────────────────────────────
    let alu_b = if is_alu_r { rs2_v } else { imm_i }
    let shamt = alu_b & 31
    // Bit 30 selects SUB (R-type f3=0) and SRA (f3=5, R and I alike).
    let alt_op = instr[30]

    let alu_out =
        if f3 == 0 {
            if is_alu_r && alt_op { rs1_v - alu_b } else { rs1_v + alu_b }
        } else if f3 == 1 { rs1_v << shamt
        } else if f3 == 2 {                                      // SLT(I)
            if (rs1_v as i32) < (alu_b as i32) { 1 } else { 0 }
        } else if f3 == 3 { if rs1_v < alu_b { 1 } else { 0 }    // SLTU(I)
        } else if f3 == 4 { rs1_v ^ alu_b
        } else if f3 == 5 {                                      // SRA / SRL
            if alt_op { ((rs1_v as i32) >> shamt) as u32 } else { rs1_v >> shamt }
        } else if f3 == 6 { rs1_v | alu_b
        } else { rs1_v & alu_b }

    // ── Branch decision ───────────────────────────────────────────
    let br_taken =
        if f3 == 0 { rs1_v == rs2_v                          // BEQ
        } else if f3 == 1 { rs1_v != rs2_v                   // BNE
        } else if f3 == 4 { (rs1_v as i32) < (rs2_v as i32)  // BLT
        } else if f3 == 5 { (rs1_v as i32) >= (rs2_v as i32) // BGE
        } else if f3 == 6 { rs1_v < rs2_v                    // BLTU
        } else { rs1_v >= rs2_v }                            // BGEU

    // ── Multiplier (single cycle) ─────────────────────────────────
    // Unsigned product; a signed operand a = au - 2^32*a31 takes
    // a31*b off the high word. rs1 is signed for MULH (f3=1) and
    // MULHSU (f3=2), rs2 only for MULH.
    let prod : u64 = rs1_v * rs2_v
    let prod_hi = prod[63:32] as u32
    let corr_a = if (f3 == 1 || f3 == 2) && rs1_v[31] { rs2_v } else { 0 }
    let corr_b = if f3 == 1 && rs2_v[31] { rs1_v } else { 0 }
    let mul_out = if f3 == 0 { prod[31:0] as u32 } else { prod_hi - corr_a - corr_b }

    // ── Divider (multi-cycle, restoring, on magnitudes) ───────────
    // f3: DIV=4 DIVU=5 REM=6 REMU=7 — bit 0 unsigned, bit 1 remainder.
    let a_neg = !f3[0] && rs1_v[31]
    let b_neg = !f3[0] && rs2_v[31]
    let a_mag = if a_neg { 0 - rs1_v } else { rs1_v }
    let b_mag = if b_neg { 0 - rs2_v } else { rs2_v }

    // One step: shift dividend bit 31-cnt into the remainder (33
    // bits: r < d can exceed 2^31), subtract the divisor if it fits —
    // the difference is below 2^32, so 32-bit arithmetic is exact.
    let n_bit = (div_n >> (31 - div_cnt)) & 1
    let r_sh : u33 = ((div_r as u33) << 1) | (n_bit as u33)
    let r_lo = r_sh[31:0] as u32
    let r_fits = r_sh >= (div_d as u33)
    let r_nx = if r_fits { r_lo - div_d } else { r_lo }
    let q_nx = (div_q << 1) | (if r_fits { 1 } else { 0 })

    // Quotient is negative when the signs differ — except for a zero
    // divisor, which must stay all ones. Remainder follows the dividend.
    let q_neg = (a_neg ^ b_neg) && div_d != 0
    let div_out =
        if f3[1] { if a_neg { 0 - div_r } else { div_r }
        } else { if q_neg { 0 - div_q } else { div_q } }

    let stall_d = is_div && !div_done

    // ── CSR access ────────────────────────────────────────────────
    let csr_addr = instr[31:20] as u12
    let csr_rdata =
        if csr_addr == 0x300 { mstatus | 0x1800           // MPP = M
        } else if csr_addr == 0x305 { mtvec
        } else if csr_addr == 0x341 { mepc
        } else if csr_addr == 0x342 { mcause
        } else if csr_addr == 0xB00 { mcycle[31:0] as u32
        } else if csr_addr == 0xB80 { mcycle[63:32] as u32
        } else if csr_addr == 0xB02 { minstret[31:0] as u32
        } else if csr_addr == 0xB82 { minstret[63:32] as u32
        } else { 0 }

    // f3[2]: source is the zero-extended rs1 field (CSRRWI/SI/CI).
    // f3[1:0]: 1 = write, 2 = set, 3 = clear. Set/clear with rs1 == x0
    // (or uimm == 0) must not write at all.
    let csr_src = if f3[2] { rs1_i as u32 } else { rs1_v }
    let csr_op  = f3 & 3
    let csr_wdata =
        if csr_op == 1 { csr_src
        } else if csr_op == 2 { csr_rdata | csr_src
        } else { csr_rdata & (csr_src ^ 0xFFFFFFFF) }
    let csr_we = is_csr && (csr_op == 1 || rs1_i != 0)

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
        } else if is_csr { csr_rdata
        } else if is_mul { mul_out
        } else if is_div { div_out
        } else { alu_out }

    // A stalled division writes nothing until its result is ready.
    let wb_en = (is_lui || is_auipc || is_jal || is_jalr
             || is_load || is_alu_i || is_alu_r || is_csr) && !stall_d

    // ── State update ──────────────────────────────────────────────
    on clk {
        if stall_d {
            pc_r <= pc_r
        } else if is_branch {
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

        // Divider sequencer: latch magnitudes, 32 steps, one done
        // cycle for the write-back. Leaving the DIV instruction
        // (never happens while `instr` is held) drops the operation.
        if !is_div {
            div_busy <= false
            div_done <= false
        } else if div_done {
            div_done <= false
        } else if div_busy {
            div_r   <= r_nx
            div_q   <= q_nx
            div_cnt <= div_cnt + 1
            if div_cnt == 31 {
                div_busy <= false
                div_done <= true
            }
        } else {
            div_n    <= a_mag
            div_d    <= b_mag
            div_q    <= 0
            div_r    <= 0
            div_cnt  <= 0
            div_busy <= true
        }

        // CSR writes, WARL masks applied on the way in.
        if csr_we {
            if csr_addr == 0x300 { mstatus <= csr_wdata & 0x88 }   // MIE, MPIE
            if csr_addr == 0x305 { mtvec   <= csr_wdata & 0xFFFFFFFC }
            if csr_addr == 0x341 { mepc    <= csr_wdata & 0xFFFFFFFC }
            if csr_addr == 0x342 { mcause  <= csr_wdata }
        }

        mcycle <= mcycle + 1
        if mcycle == 0xFFFFFFFFFFFFFFFF {
            cyc_wrap <= true
        }
        if !stall_d {
            minstret <= minstret + 1
        }
    }

    // ── Outputs ───────────────────────────────────────────────────
    pc        = pc_r
    stall_o   = stall_d
    cyc_wrap_o = cyc_wrap
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
