// 32-bit timer with compare interrupt, AXI4-Lite subset.
//
//   0x00 CTRL     RW  bit0 enable, bit1 irq_en
//   0x04 COUNT    RW  free-running count (write to preload)
//   0x08 COMPARE  RW  irq pending is set when COUNT == COMPARE
//   0x0C STATUS   RW1C bit0 irq pending (write 1 to clear)
//
// The count wraps at 2^32. `irq` = pending && irq_en.

package soc::timer;

use axi4lite_slave::{AxiAddr, AxiWData, AxiResp, AxiRData};
use soc::axi::{AxiToReg, AxiReadCtl, RegBus};

pub module Timer {
    in  clk : clock
    in  aw  : Handshake<AxiAddr>
    in  w   : Handshake<AxiWData>
    out b   : Handshake<AxiResp>
    in  ar  : Handshake<AxiAddr>
    out r   : Handshake<AxiRData>
    out irq : bool

    // The interrupt line is gated by irq_en.
    invariant: irq -> irq_en_r
    // A pending flag only rises through the compare match.
    invariant: !prev(pending_r) && pending_r -> prev(enable_r) && prev(count_r) == prev(compare_r)
    cover: irq

    let br = AxiToReg {
        clk: clk,
        aw_data_addr: aw.data.addr, aw_data_prot: aw.data.prot, aw_valid: aw.valid,
        w_data_data: w.data.data, w_data_strb: w.data.strb, w_valid: w.valid,
        b_ready: b.ready,
        ar_data_addr: ar.data.addr, ar_data_prot: ar.data.prot, ar_valid: ar.valid,
        r_ready: r.ready,
    }
    aw.ready    = br.aw_ready
    w.ready     = br.w_ready
    b.valid     = br.b_valid
    b.data.resp = br.b_data_resp
    ar.ready    = br.ar_ready
    r.valid     = br.r_valid
    r.data.resp = br.r_resp

    reg enable_r  : bool = false
    reg irq_en_r  : bool = false
    reg count_r   : u32  = 0
    reg compare_r : u32  = 0
    reg pending_r : bool = false
    reg rdata_r   : u32  = 0

    let ctrl_word : u32 = (if enable_r { 1 } else { 0 }) | (if irq_en_r { 2 } else { 0 })
    let wr_ctrl    : bool = br.bus_we && br.bus_waddr == 0x00
    let wr_count   : bool = br.bus_we && br.bus_waddr == 0x04
    let wr_compare : bool = br.bus_we && br.bus_waddr == 0x08
    let wr_status  : bool = br.bus_we && br.bus_waddr == 0x0C
    let hit : bool = enable_r && count_r == compare_r

    on clk {
        if wr_ctrl && br.bus_wmask[0] {
            enable_r <= br.bus_wdata[0]
            irq_en_r <= br.bus_wdata[1]
        }
        if wr_count {
            count_r <= (count_r & ~br.bus_wmask) | br.bus_wdata
        } else if enable_r {
            count_r <= count_r + 1
        }
        if wr_compare {
            compare_r <= (compare_r & ~br.bus_wmask) | br.bus_wdata
        }
        // Set wins over clear when both happen in the same cycle.
        if hit {
            pending_r <= true
        } else if wr_status && br.bus_wdata[0] {
            pending_r <= false
        }
        if br.bus_re {
            match br.bus_raddr {
                0x00 => { rdata_r <= ctrl_word }
                0x04 => { rdata_r <= count_r }
                0x08 => { rdata_r <= compare_r }
                0x0C => { rdata_r <= if pending_r { 1 } else { 0 } }
                _ => { rdata_r <= 0 }
            }
        }
    }

    r.data.data = rdata_r
    irq = pending_r && irq_en_r
}
