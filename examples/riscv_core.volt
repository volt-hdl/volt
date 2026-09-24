// RV32IM + Zicsr core — 37 base-ISA instructions, the 8 M-extension
// instructions and the 6 CSR instructions over a minimal machine-mode
// CSR set, with traps (ECALL/EBREAK/illegal/misaligned + MRET), one
// external interrupt line and a memory-mapped UART. Single-cycle,
// except division.
//
// Rewritten with ADR-0035 features: the register file is one
// `reg regs : [u32; 32]` with dynamic indexing (regs[ins.rs1], regs[ins.rd])
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
//   mie     0x304  only MEIE (bit 11) is writable
//   mip     0x344  read-only, MEIP (bit 11) mirrors the `irq` pin
//   mscratch 0x340 plain storage for the trap handler
// Unmapped CSR numbers read zero and ignore writes.
//
// Traps. Synchronous exceptions: illegal instruction (mcause 2),
// instruction-address-misaligned on a taken jump/branch whose target
// has bit 1 set (0), EBREAK (3), misaligned load (4) / store (6),
// ECALL (11). A trap is taken *instead of* the instruction at pc: no
// register, CSR, memory or I/O write happens, minstret does not
// count, and mepc <- pc, mcause <- cause, MPIE <- MIE, MIE <- 0,
// pc <- mtvec. MRET undoes it: pc <- mepc, MIE <- MPIE, MPIE <- 1.
// WFI and FENCE are NOPs.
//
// Interrupt: one level-sensitive external line `irq` (expected
// synchronous to clk), taken when mstatus.MIE && mie.MEIE with
// mcause = 0x8000000B. The core is single-cycle, so every cycle starts
// on an instruction boundary — except inside a division. The interrupt
// therefore waits while the divider is running (div_busy || div_done);
// the division retires first and mepc is the pc of the *next*
// instruction, the one the interrupt displaced. `irq_ack_o` pulses in
// the cycle the interrupt is taken, `trap_o` for every trap, `mret_o`
// for an executed MRET.
//
// Memory-mapped I/O. Loads/stores whose address has the top nibble 2
// (0x2xxx_xxxx) never reach the memory bus (mem_read/mem_write stay
// low); everything else does, and the external decoder splits
// 0x0000_xxxx ROM from 0x1000_xxxx RAM.
//   0x2000_0000  UART TX data   a store sends the low byte of rs2
//                               through UartTx (examples/uart_tx.volt);
//                               ignored while the transmitter is busy
//   0x2000_0004  UART TX status bit 0 = busy
// Only address bit 2 is decoded inside the I/O window.

use uart_tx::UartTx;

// The R-type field layout of the RISC-V base encoding, bit 31 first. A
// struct packs its first field into the most significant bits (ADR-0077),
// so `instr as RType` reads the spec table directly; every format shares
// opcode, rd, funct3, rs1 and rs2 at these positions. Immediates are
// scattered across the word per format and stay explicit shifts below.
struct RType {
    funct7 : u7   // [31:25]
    rs2    : u5   // [24:20]
    rs1    : u5   // [19:15]
    funct3 : u3   // [14:12]
    rd     : u5   // [11:7]
    opcode : u7   // [6:0]
}

pub module RiscvCore {
    in  clk       : clock
    in  instr     : u32
    in  mem_rdata : u32
    in  irq       : bool
    out pc        : u32
    out mem_addr  : u32
    out mem_wdata : u32
    out mem_wmask : u8
    out mem_write : bool
    out mem_read  : bool
    out stall_o   : bool
    out cyc_wrap_o : bool
    out trap_o    : bool
    out irq_ack_o : bool
    out mret_o    : bool
    out uart_txd  : bool

    // x0 reads as zero forever — the guard `ins.rd != 0` below keeps
    // this inductive from the reset state [0; 32].
    invariant: regs[0] == 0
    // Instructions are 4-byte aligned in RV32I (no compressed
    // extension): +4 keeps bit 0, branch/JAL immediates have bit 0
    // hardwired zero, and JALR masks it explicitly.
    invariant: !pc_r[0]
    // ... and bit 1 too: a jump to a target with bit 1 set traps
    // instead (instruction-address-misaligned), and mtvec/mepc are
    // masked on the way in. So a trap always saves a valid pc.
    invariant: !pc_r[1]
    invariant: !mtvec[0] && !mtvec[1]
    // Counters: mcycle ticks every cycle (0 = reset or 2^64 wrap),
    // and no more instructions retire than cycles elapse — until
    // mcycle itself wraps, which the sticky `cyc_wrap` records so the
    // bound stays inductive (contracts see only ports and registers,
    // hence a real flop with a debug port, as in riscv_pipeline).
    invariant: mcycle != 0 -> mcycle == prev(mcycle) + 1
    invariant: !cyc_wrap -> minstret <= mcycle
    invariant: !mepc[0] && !mepc[1]
    // Trap entry: mepc holds the pc the trap was taken at, with
    // interrupts masked. MRET: the pc comes back from mepc.
    invariant: prev(trap_o) -> mepc == prev(pc_r)
    invariant: prev(trap_o) -> !mstatus[3]
    invariant: prev(mret_o) -> pc_r == prev(mepc)
    // An interrupt needs MIE and MEIE, and never cuts into a division.
    invariant: irq_ack_o -> mstatus[3] && mie_meie && irq
    invariant: irq_ack_o -> !div_busy && !div_done
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
    cover: ins.opcode == 0x33 && ins.funct7 == 1 && ins.funct3 == 0
    cover: div_done && div_d == 0
    // An ECALL trapped, an MRET ran, an interrupt was taken, and a
    // store to 0x2000_0000 pulled the UART line low (start bit).
    cover: trap_o && instr == 0x00000073 && !irq_ack_o
    cover: mret_o
    cover: irq_ack_o
    cover: !uart_txd

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
    reg mscratch : u32 = 0
    reg mie_meie : bool = false
    reg mcycle   : u64 = 0
    reg minstret : u64 = 0
    reg cyc_wrap : bool = false

    // ── Decode ────────────────────────────────────────────────────
    let ins : RType = instr as RType
    // funct3 selects the operation in nearly every decoder below.
    let f3 = ins.funct3

    let is_lui    = ins.opcode == 0x37
    let is_auipc  = ins.opcode == 0x17
    let is_jal    = ins.opcode == 0x6F
    let is_jalr   = ins.opcode == 0x67
    let is_branch = ins.opcode == 0x63
    let is_load   = ins.opcode == 0x03
    let is_store  = ins.opcode == 0x23
    let is_alu_i  = ins.opcode == 0x13
    let is_alu_r  = ins.opcode == 0x33
    // M extension: R-type with funct7 == 1; f3[2] splits MUL* / DIV*.
    let is_m      = is_alu_r && ins.funct7 == 1
    let is_mul    = is_m && !f3[2]
    let is_div    = is_m && f3[2]
    // Zicsr: SYSTEM opcode with f3 != 0; f3 == 0 is the privileged group.
    let is_csr    = ins.opcode == 0x73 && f3 != 0
    let is_ecall  = instr == 0x00000073
    let is_ebreak = instr == 0x00100073
    let is_mret   = instr == 0x30200073
    let is_wfi    = instr == 0x10500073   // NOP: the core never sleeps
    let is_fence  = ins.opcode == 0x0F        // FENCE / FENCE.I: NOP, no caches

    // Everything else is an illegal instruction: unknown opcodes,
    // unassigned f3 values, shifts and R-type with a stray funct7.
    let sh_ok =
        if f3 == 1 { ins.funct7 == 0
        } else if f3 == 5 { ins.funct7 == 0 || ins.funct7 == 0x20
        } else { true }
    let r_ok = ins.funct7 == 0 || ins.funct7 == 1 || (ins.funct7 == 0x20 && (f3 == 0 || f3 == 5))
    let legal = is_lui || is_auipc || is_jal || (is_jalr && f3 == 0)
             || (is_branch && f3 != 2 && f3 != 3)
             || (is_load && f3 != 3 && f3 < 6)
             || (is_store && f3 < 3)
             || (is_alu_i && sh_ok) || (is_alu_r && r_ok)
             || (is_csr && f3 != 4)
             || is_ecall || is_ebreak || is_mret || is_wfi || is_fence

    // ── Register file read (dynamic indexing, ADR-0035) ───────────
    let rs1_v = regs[ins.rs1]
    let rs2_v = regs[ins.rs2]

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

    // ── Jump target ───────────────────────────────────────────────
    let do_jump = is_jal || is_jalr || (is_branch && br_taken)
    let jump_tgt =
        if is_jal { pc_r + imm_j
        } else if is_jalr { (rs1_v + imm_i) & 0xFFFFFFFE
        } else { pc_r + imm_b }

    // ── Data address, I/O decode ──────────────────────────────────
    let addr = rs1_v + (if is_store { imm_s } else { imm_i })
    let off8 = (addr & 3) << 3    // byte lane → bit offset
    let is_io = (is_load || is_store) && (addr >> 28) == 2

    // ── Traps ─────────────────────────────────────────────────────
    // Halfword accesses (f3[1:0] == 1) need bit 0 clear, words both.
    let acc_mis = ((f3 & 3) == 1 && addr[0]) || (f3 == 2 && (addr & 3) != 0)
    let if_mis  = do_jump && jump_tgt[1]
    let ld_mis  = is_load && acc_mis
    let st_mis  = is_store && acc_mis
    let exc = !legal || if_mis || is_ecall || is_ebreak || ld_mis || st_mis
    let exc_cause : u32 =
        if !legal { 2
        } else if if_mis { 0
        } else if is_ebreak { 3
        } else if ld_mis { 4
        } else if st_mis { 6
        } else { 11 }

    // Interrupts are taken on instruction boundaries only: never while
    // the divider holds the current instruction half-done.
    let take_irq  = irq && mstatus[3] && mie_meie && !div_busy && !div_done
    let take_trap = take_irq || exc
    // An interrupt can displace a division that has not started yet.
    let hold = stall_d && !take_irq
    let do_mret = is_mret && !take_trap

    // ── UART (memory-mapped, 0x2000_0000 data / 0x2000_0004 status) ─
    let uart_we = is_store && is_io && !addr[2] && !take_trap
    let u_tx = UartTx {
        clk: clk,
        start: uart_we,
        data: rs2_v[7:0] as u8,
    }
    let io_rdata : u32 = if addr[2] && u_tx.busy { 1 } else { 0 }
    let rdata = if is_io { io_rdata } else { mem_rdata }

    // ── CSR access ────────────────────────────────────────────────
    let csr_addr = instr[31:20] as u12
    let csr_rdata =
        if csr_addr == 0x300 { mstatus | 0x1800           // MPP = M
        } else if csr_addr == 0x304 { if mie_meie { 0x800 } else { 0 }
        } else if csr_addr == 0x305 { mtvec
        } else if csr_addr == 0x340 { mscratch
        } else if csr_addr == 0x341 { mepc
        } else if csr_addr == 0x342 { mcause
        } else if csr_addr == 0x344 { if irq { 0x800 } else { 0 }    // MEIP
        } else if csr_addr == 0xB00 { mcycle[31:0] as u32
        } else if csr_addr == 0xB80 { mcycle[63:32] as u32
        } else if csr_addr == 0xB02 { minstret[31:0] as u32
        } else if csr_addr == 0xB82 { minstret[63:32] as u32
        } else { 0 }

    // f3[2]: source is the zero-extended rs1 field (CSRRWI/SI/CI).
    // f3[1:0]: 1 = write, 2 = set, 3 = clear. Set/clear with rs1 == x0
    // (or uimm == 0) must not write at all.
    let csr_src = if f3[2] { ins.rs1 as u32 } else { rs1_v }
    let csr_op  = f3 & 3
    let csr_wdata =
        if csr_op == 1 { csr_src
        } else if csr_op == 2 { csr_rdata | csr_src
        } else { csr_rdata & (csr_src ^ 0xFFFFFFFF) }
    let csr_we = is_csr && (csr_op == 1 || ins.rs1 != 0) && !take_trap

    // ── Load data (memory or I/O) ─────────────────────────────────
    // Sub-word extraction with a variable-start part-select
    // (ADR-0035) — previously a shift-and-mask detour. The halfword
    // offset is inlined so no wire carries lint-unused upper bits.
    let lb_u = rdata[off8 +: 8] as u8
    let lb_s = lb_u as i8
    let lh_u = rdata[((addr & 2) << 3) +: 16] as u16
    let lh_s = lh_u as i16

    let load_val =
        if f3 == 0 { (lb_s as i32) as u32      // LB
        } else if f3 == 1 { (lh_s as i32) as u32  // LH
        } else if f3 == 2 { rdata              // LW
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

    // A stalled division writes nothing until its result is ready,
    // a trapping instruction writes nothing at all.
    let wb_en = (is_lui || is_auipc || is_jal || is_jalr
             || is_load || is_alu_i || is_alu_r || is_csr)
             && !stall_d && !take_trap

    // ── State update ──────────────────────────────────────────────
    on clk {
        if take_trap {
            pc_r <= mtvec
        } else if stall_d {
            pc_r <= pc_r
        } else if is_mret {
            pc_r <= mepc
        } else if do_jump {
            pc_r <= jump_tgt
        } else {
            pc_r <= pc_r + 4
        }

        // Dynamic indexed write (ADR-0035); ins.rd != 0 preserves x0.
        if wb_en {
            if ins.rd != 0 {
                regs[ins.rd] <= wb_val
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
        } else if !take_irq {
            div_n    <= a_mag
            div_d    <= b_mag
            div_q    <= 0
            div_r    <= 0
            div_cnt  <= 0
            div_busy <= true
        }

        // Trap entry / MRET / CSR writes (WARL masks applied on the
        // way in). mstatus: MIE is bit 3, MPIE bit 7.
        if take_trap {
            mepc    <= pc_r
            mcause  <= if take_irq { 0x8000000B } else { exc_cause }
            mstatus <= if mstatus[3] { 0x80 } else { 0 }
        } else if is_mret {
            mstatus <= if mstatus[7] { 0x88 } else { 0x80 }
        } else if csr_we {
            if csr_addr == 0x300 { mstatus <= csr_wdata & 0x88 }   // MIE, MPIE
            if csr_addr == 0x304 { mie_meie <= csr_wdata[11] }
            if csr_addr == 0x305 { mtvec   <= csr_wdata & 0xFFFFFFFC }
            if csr_addr == 0x340 { mscratch <= csr_wdata }
            if csr_addr == 0x341 { mepc    <= csr_wdata & 0xFFFFFFFC }
            if csr_addr == 0x342 { mcause  <= csr_wdata }
        }

        mcycle <= mcycle + 1
        if mcycle == 0xFFFFFFFFFFFFFFFF {
            cyc_wrap <= true
        }
        if !stall_d && !take_trap {
            minstret <= minstret + 1
        }
    }

    // ── Outputs ───────────────────────────────────────────────────
    // I/O accesses and trapping instructions stay off the memory bus.
    let mem_rd = is_load && !is_io && !take_trap
    let mem_wr = is_store && !is_io && !take_trap

    pc        = pc_r
    stall_o   = hold
    trap_o    = take_trap
    irq_ack_o = take_irq
    mret_o    = do_mret
    uart_txd  = u_tx.tx
    cyc_wrap_o = cyc_wrap
    mem_addr  = if mem_rd || mem_wr { addr } else { 0 }
    mem_read  = mem_rd
    mem_write = mem_wr
    mem_wdata = rs2_v << off8
    mem_wmask =
        if !mem_wr { 0
        } else if f3 == 0 { 1u8 << (addr & 3)   // SB
        } else if f3 == 1 { 3u8 << (addr & 2)   // SH
        } else { 15 }                           // SW
}
