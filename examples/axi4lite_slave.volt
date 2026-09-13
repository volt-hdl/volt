// AXI4-Lite register slave written with bundle ports (ADR-0039).
//
// The five AXI channels are five `struct port` bundles declared from
// the MASTER's point of view (`out` = master drives it). The slave
// declares each channel with `in`, which flips every field: aw.addr
// becomes an input, aw.ready becomes an output. One definition serves
// both sides of the bus, and the generated SystemVerilog is flat
// (`aw_addr`, `aw_valid`, `aw_ready`, ...) — no interface/modport.
//
// Register map (full 32-bit address compare):
//   0x00 reg0   RW    0x04 reg1   RW    0x08 reg2   RW    0x0C reg3   RW
//   0x10 status RO (input port)         other addresses read as zero
//
// Protocol: the write channels are accepted together (aw.ready and
// w.ready rise only when both aw.valid and w.valid are high, which
// AXI4-Lite permits), the write happens on that edge and b.valid holds
// until the master raises b.ready. Reads are accepted whenever no read
// response is pending; r.valid holds until r.ready. Byte strobes are
// honoured through a write mask. The registers accept only secure,
// unprivileged data accesses (prot == 0): any other access is refused,
// the write is dropped and the response is SLVERR (2); accepted
// accesses respond OKAY (0).

pub struct port AxiWriteAddr {
    out addr  : u32
    out prot  : u3
    out valid : bool
    in  ready : bool
}

pub struct port AxiWriteData {
    out data  : u32
    out strb  : u4
    out valid : bool
    in  ready : bool
}

pub struct port AxiWriteResp {
    in  resp  : u2
    in  valid : bool
    out ready : bool
}

pub struct port AxiReadAddr {
    out addr  : u32
    out prot  : u3
    out valid : bool
    in  ready : bool
}

pub struct port AxiReadData {
    in  data  : u32
    in  resp  : u2
    in  valid : bool
    out ready : bool
}

pub module Axi4LiteSlave {
    in  clk    : clock
    in  aw     : AxiWriteAddr
    in  w      : AxiWriteData
    in  b      : AxiWriteResp
    in  ar     : AxiReadAddr
    in  r      : AxiReadData
    in  status : u32
    out reg0   : u32
    out reg1   : u32
    out reg2   : u32
    out reg3   : u32

    // Both write channels are accepted in the same cycle.
    invariant: aw.ready == w.ready
    // No new write is accepted while a write response is pending.
    invariant: b.valid -> !aw.ready
    // No new read is accepted while a read response is pending.
    invariant: r.valid -> !ar.ready
    // Responses are OKAY or SLVERR, never EXOKAY/DECERR.
    invariant: b.resp == 0 || b.resp == 2
    invariant: r.resp == 0 || r.resp == 2

    // Sequential protocol rules (ADR-0040, prev()). The master must hold
    // *_valid until the matching *_ready — a slave cannot enforce that on
    // its inputs, so these are environment assumptions.
    assume: prev(aw.valid) && !prev(aw.ready) -> aw.valid
    assume: prev(w.valid) && !prev(w.ready) -> w.valid
    assume: prev(ar.valid) && !prev(ar.ready) -> ar.valid
    // The slave holds its responses until the master accepts them.
    invariant: prev(b.valid) && !prev(b.ready) -> b.valid
    invariant: prev(r.valid) && !prev(r.ready) -> r.valid
    // Every accepted request is answered on the very next cycle.
    invariant: prev(aw.valid) && prev(aw.ready) -> b.valid
    invariant: prev(ar.valid) && prev(ar.ready) -> r.valid

    cover: b.valid && b.ready
    cover: r.valid && r.ready
    cover: b.valid && b.resp == 2
    cover: reg3 != 0

    reg reg0_r   : u32  = 0
    reg reg1_r   : u32  = 0
    reg reg2_r   : u32  = 0
    reg reg3_r   : u32  = 0
    reg bvalid_r : bool = false
    reg berr_r   : bool = false
    reg rvalid_r : bool = false
    reg rerr_r   : bool = false
    reg rdata_r  : u32  = 0

    let wr_fire : bool = aw.valid && w.valid && !bvalid_r
    let rd_fire : bool = ar.valid && !rvalid_r
    let wmask : u32 = (if w.strb[0] { 0x000000FF } else { 0 })
                    | (if w.strb[1] { 0x0000FF00 } else { 0 })
                    | (if w.strb[2] { 0x00FF0000 } else { 0 })
                    | (if w.strb[3] { 0xFF000000 } else { 0 })
    let wdata : u32 = w.data & wmask

    on clk {
        if wr_fire {
            bvalid_r <= true
            berr_r   <= aw.prot != 0
            if aw.prot == 0 {
                match aw.addr {
                    0x0 => { reg0_r <= (reg0_r & ~wmask) | wdata }
                    0x4 => { reg1_r <= (reg1_r & ~wmask) | wdata }
                    0x8 => { reg2_r <= (reg2_r & ~wmask) | wdata }
                    0xC => { reg3_r <= (reg3_r & ~wmask) | wdata }
                    _ => { }
                }
            }
        }
        if bvalid_r && b.ready {
            bvalid_r <= false
        }
        if rd_fire {
            rvalid_r <= true
            rerr_r   <= ar.prot != 0
            match ar.addr {
                0x0  => { rdata_r <= reg0_r }
                0x4  => { rdata_r <= reg1_r }
                0x8  => { rdata_r <= reg2_r }
                0xC  => { rdata_r <= reg3_r }
                0x10 => { rdata_r <= status }
                _ => { rdata_r <= 0 }
            }
        }
        if rvalid_r && r.ready {
            rvalid_r <= false
        }
    }

    aw.ready = wr_fire
    w.ready  = wr_fire
    b.valid  = bvalid_r
    b.resp   = if berr_r { 2 } else { 0 }
    ar.ready = rd_fire
    r.valid  = rvalid_r
    r.data   = rdata_r
    r.resp   = if rerr_r { 2 } else { 0 }
    reg0 = reg0_r
    reg1 = reg1_r
    reg2 = reg2_r
    reg3 = reg3_r
}
