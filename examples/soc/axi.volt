// SoC AXI layer: the internal register bus bundle and the AXI4-Lite to
// register-bus bridge shared by every peripheral.
//
// The five AXI4-Lite channel bundles are imported from
// examples/axi4lite_slave.volt (reused by reference, ADR-0042) — the
// bridge speaks exactly the subset that Axi4LiteSlave implements.

package soc::axi;

use axi4lite_slave::{AxiWriteAddr, AxiWriteData, AxiWriteResp, AxiReadAddr, AxiReadData};

// Internal register bus, seen from the bridge (the bridge drives `out`).
// Addresses are the full (page-masked) words: peripherals compare whole
// 32-bit values, because slicing `addr[7:0]` leaves the upper bits
// unused and Verilator -Wall flags every partially used signal.
// Writes arrive already byte-masked: the peripheral does
//   reg <= (reg & ~bus.wmask) | bus.wdata
// Reads are two-phase: `re` pulses with `raddr`, the peripheral must
// register the selected value the same cycle and drive it on r.data
// itself (the bridge never sees read data, so there is no
// combinational path from an instance output back into its own input).
pub struct port RegBus {
    out waddr : u32
    out wdata : u32
    out wmask : u32
    out we    : bool
    out raddr : u32
    out re    : bool
}

// Read-response control only: the peripheral owns r.data.
pub struct port AxiReadCtl {
    in  valid : bool
    in  resp  : u2
    out ready : bool
}

// AXI4-Lite subset to register bus. Same protocol as Axi4LiteSlave:
// aw and w are accepted together, one response per accepted request,
// responses held until the master takes them, prot != 0 -> SLVERR.
pub module AxiToReg {
    in  clk : clock
    in  aw  : AxiWriteAddr
    in  w   : AxiWriteData
    in  b   : AxiWriteResp
    in  ar  : AxiReadAddr
    in  r   : AxiReadCtl
    out bus : RegBus

    invariant: aw.ready == w.ready
    invariant: b.valid -> !aw.ready
    invariant: r.valid -> !ar.ready
    invariant: prev(b.valid) && !prev(b.ready) -> b.valid
    invariant: prev(r.valid) && !prev(r.ready) -> r.valid
    invariant: prev(aw.valid) && prev(aw.ready) -> b.valid
    invariant: prev(ar.valid) && prev(ar.ready) -> r.valid
    assume: prev(aw.valid) && !prev(aw.ready) -> aw.valid
    assume: prev(w.valid) && !prev(w.ready) -> w.valid
    assume: prev(ar.valid) && !prev(ar.ready) -> ar.valid
    cover: b.valid && b.ready
    cover: r.valid && r.ready

    reg bvalid_r : bool = false
    reg berr_r   : bool = false
    reg rvalid_r : bool = false
    reg rerr_r   : bool = false

    let wr_fire : bool = aw.valid && w.valid && !bvalid_r
    let rd_fire : bool = ar.valid && !rvalid_r
    let wmask : u32 = (if w.strb[0] { 0x000000FF } else { 0 })
                    | (if w.strb[1] { 0x0000FF00 } else { 0 })
                    | (if w.strb[2] { 0x00FF0000 } else { 0 })
                    | (if w.strb[3] { 0xFF000000 } else { 0 })

    on clk {
        if wr_fire {
            bvalid_r <= true
            berr_r   <= aw.prot != 0
        }
        if bvalid_r && b.ready {
            bvalid_r <= false
        }
        if rd_fire {
            rvalid_r <= true
            rerr_r   <= ar.prot != 0
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
    r.resp   = if rerr_r { 2 } else { 0 }

    bus.waddr = aw.addr
    bus.wdata = w.data & wmask
    bus.wmask = wmask
    bus.we    = wr_fire && aw.prot == 0
    bus.raddr = ar.addr
    bus.re    = rd_fire
}
