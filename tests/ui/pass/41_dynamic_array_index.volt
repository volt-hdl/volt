// Dynamic array indexing (ADR-0035): a register file addressed by a
// runtime index. No compile-time bounds check for variable indices —
// the idx < N guarantee is expressed as a contract.
module RegFile {
    in  clk     : clock
    in  we      : bool
    in  waddr   : bits<5>
    in  raddr   : bits<5>
    in  wdata   : u32
    out rdata   : u32

    reg regs : [u32; 32] = [0; 32]

    on clk {
        if we {
            regs[waddr] <= wdata
        }
    }

    rdata = regs[raddr]
}
