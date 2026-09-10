// RV32I classic 5-stage pipeline (IF, ID, EX, MEM, WB) — rewritten
// with the `pipeline` syntax (ADR-0038). The compiler now generates
// every inter-stage register (`<stage>_<name>_r`), the stall guards
// and the flush bubbles that the hand-built version (283 lines)
// spelled out by hand.
//
// Instruction subset (12): ADD SUB ADDI AND OR XOR LW SW BEQ BNE
// JAL LUI. Full-word loads/stores only, riscv_core memory model:
// combinational instruction port (age 0) and combinational data port
// answering for the instruction sitting in MEM (`Delayed<u32, 3>`).
//
// Hazard handling, all in source:
//   * EX→EX / MEM→EX forwarding: the `stage(Memory)` / `stage(Writeback)`
//     arms of `fwd_a`/`fwd_b` — a stage() reference pins the let to its
//     own stage's delay (automatic retiming assertion, ADR-0038 §3).
//   * WB→ID bypass: the `stage(Writeback)` arm of `rs1_v`/`rs2_v`.
//   * Load-use stall: `stall when` in Decode — holds Fetch+Decode,
//     bubbles Execute. The IF/ID hold guard is GENERATED; the bug
//     class "forgot !stall on IF/ID" is no longer expressible.
//   * Branch/JAL resolve in EX: `flush Fetch, Decode when redirect`.
//
// Bubble encoding: generated bubbles are all-zeros; instruction word 0
// decodes to no opcode class, and wb_en=false makes the slot inert.

pipeline(5) RiscvPipeline {
    in  clk       : clock
    in  instr     : u32
    in  mem_rdata : Delayed<u32, 3>
    out pc        : u32
    out mem_addr  : u32
    out mem_wdata : u32
    out mem_write : bool
    out mem_read  : bool
    out stall_o   : bool
    out flush_o   : bool

    // x0 is never written: decode drops wb_en for rd == 0, and the
    // per-stage contracts carry that fact down the pipe (inductive).
    invariant: regs[0] == 0
    // PCs stay 4-byte aligned through every stage.
    invariant: !pc_r[0]
    invariant: !stage(Decode).pc_f[0]
    invariant: !stage(Execute).pc_f[0]
    // A write-back never targets x0, stage by stage.
    invariant: !(stage(Execute).wb_en && stage(Execute).rd == 0)
    invariant: !(stage(Memory).wb_en && stage(Memory).rd == 0)
    invariant: !(stage(Writeback).wb_en && stage(Writeback).rd == 0)
    // An instruction is at most one memory class.
    invariant: !(stage(Execute).is_load && stage(Execute).is_store)
    invariant: !(stage(Memory).is_load && stage(Memory).is_store)
    // Branch targets stay aligned: B/J immediates have bit 0 clear.
    invariant: !((stage(Execute).is_branch || stage(Execute).is_jal) && stage(Execute).imm[0])
    cover: mem_write
    cover: mem_read
    cover: stall_o
    cover: flush_o

    // ── Architectural state ───────────────────────────────────────
    reg pc_r : u32 = 0
    reg regs : [u32; 32] = [0; 32]

    stage Fetch {
        let ir   : u32 = instr
        let pc_f : u32 = pc_r
        pc_r <= if stage(Execute).redirect { stage(Execute).redirect_pc } else { pc_r + 4 }
    }

    stage Decode {
        let opcode = ir[6:0] as u7
        let rd  : u5 = ir[11:7] as u5
        let f3  : u3 = ir[14:12] as u3
        let rs1 : u5 = ir[19:15] as u5
        let rs2 : u5 = ir[24:20] as u5

        let is_lui    : bool = opcode == 0x37
        let is_jal    : bool = opcode == 0x6F
        let is_branch : bool = opcode == 0x63
        let is_load   : bool = opcode == 0x03
        let is_store  : bool = opcode == 0x23
        let is_alu_i  : bool = opcode == 0x13
        let is_alu_r  : bool = opcode == 0x33

        // rd != 0 here (not at the write port) keeps the wb_en/rd
        // contracts provable and the forwarding comparators x0-free.
        let wb_en : bool = (is_lui || is_jal || is_load || is_alu_i || is_alu_r) && rd != 0

        let imm_i = ((ir as i32) >> 20) as u32
        let imm_s = ((((ir as i32) >> 25) << 5) as u32) | ((ir >> 7) & 0x1F)
        let imm_b = ((((ir as i32) >> 31) << 12) as u32) | (((ir >> 7) & 1) << 11)
                  | (((ir >> 25) & 0x3F) << 5) | (((ir >> 8) & 0xF) << 1)
        let imm_u = ir & 0xFFFFF000
        let imm_j = ((((ir as i32) >> 31) << 20) as u32) | (ir & 0xFF000)
                  | (((ir >> 20) & 1) << 11) | (((ir >> 21) & 0x3FF) << 1)
        let imm : u32 =
            if is_store { imm_s } else if is_branch { imm_b
            } else if is_jal { imm_j } else if is_lui { imm_u } else { imm_i }

        let alt     : bool = is_alu_r && ir[30]
        let use_imm : bool = is_alu_i || is_load || is_store

        // WB→ID bypass: the writer sits in WB this very cycle; the
        // stage(Writeback) arm reads it before the regfile updates.
        let rs1_v : u32 = if stage(Writeback).wb_en && stage(Writeback).rd == rs1 {
            stage(Writeback).result } else { regs[rs1] }
        let rs2_v : u32 = if stage(Writeback).wb_en && stage(Writeback).rd == rs2 {
            stage(Writeback).result } else { regs[rs2] }

        // Load-use hazard: LW in EX, dependent here → hold one cycle.
        let uses_rs1 = is_alu_r || is_alu_i || is_load || is_store || is_branch
        let uses_rs2 = is_alu_r || is_store || is_branch
        stall when stage(Execute).is_load
              && ((uses_rs1 && stage(Execute).rd == rs1 && rs1 != 0)
               || (uses_rs2 && stage(Execute).rd == rs2 && rs2 != 0))
    }

    stage Execute {
        // Forwarding: EX→EX beats MEM→EX (younger result wins).
        let fwd_a : u32 = if stage(Memory).wb_en && stage(Memory).rd == rs1 { stage(Memory).alu
            } else if stage(Writeback).wb_en && stage(Writeback).rd == rs1 { stage(Writeback).result
            } else { rs1_v }
        let fwd_b : u32 = if stage(Memory).wb_en && stage(Memory).rd == rs2 { stage(Memory).alu
            } else if stage(Writeback).wb_en && stage(Writeback).rd == rs2 { stage(Writeback).result
            } else { rs2_v }

        let alu_b = if use_imm { imm } else { fwd_b }
        let alu_op =
            if f3 == 0 { if alt { fwd_a - alu_b } else { fwd_a + alu_b }
            } else if f3 == 4 { fwd_a ^ alu_b
            } else if f3 == 6 { fwd_a | alu_b
            } else { fwd_a & alu_b }

        // LW/SW carry f3 == 2 — the effective address bypasses the mux.
        let alu : u32 = if is_load || is_store { fwd_a + imm
            } else if is_lui { imm
            } else if is_jal { pc_f + 4
            } else { alu_op }
        let store_v : u32 = fwd_b

        // Branch resolution on forwarded operands; 2-cycle penalty.
        let br_eq = fwd_a == fwd_b
        let redirect    : bool = (is_branch && (if f3 == 0 { br_eq } else { !br_eq })) || is_jal
        let redirect_pc : u32  = pc_f + imm
        flush Fetch, Decode when redirect
    }

    stage Memory {
        let result : u32 = if is_load { mem_rdata } else { alu }
    }

    stage Writeback {
        if wb_en {
            regs[rd] <= result
        }
    }

    // ── Outputs ───────────────────────────────────────────────────
    pc        = pc_r
    mem_addr  = if stage(Memory).is_load || stage(Memory).is_store { stage(Memory).alu } else { 0 }
    mem_read  = stage(Memory).is_load
    mem_write = stage(Memory).is_store
    mem_wdata = stage(Memory).store_v
    stall_o   = stall_decode
    flush_o   = flush_fetch
}
