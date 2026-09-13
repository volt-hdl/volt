// 8-bit GPIO peripheral, AXI4-Lite subset (via the AxiToReg bridge).
//
//   0x00 DATA_OUT  RW  value driven on pins configured as outputs
//   0x04 DIR       RW  1 = output enable per pin
//   0x08 DATA_IN   RO  pin inputs
//   other          reads as zero, writes ignored

package soc::gpio;

use axi4lite_slave::{AxiWriteAddr, AxiWriteData, AxiWriteResp, AxiReadAddr, AxiReadData};
use soc::axi::{AxiToReg, AxiReadCtl, RegBus};

pub module Gpio {
    in  clk  : clock
    in  aw   : AxiWriteAddr
    in  w    : AxiWriteData
    in  b    : AxiWriteResp
    in  ar   : AxiReadAddr
    in  r    : AxiReadData
    in  pins_in  : u8
    out pins_out : u8
    out pins_oe  : u8

    cover: pins_oe != 0

    // Bundles cannot be passed whole to an instance (ADR-0039 limit):
    // every field is bound by its flattened name.
    let br = AxiToReg {
        clk: clk,
        aw_addr: aw.addr, aw_prot: aw.prot, aw_valid: aw.valid,
        w_data: w.data, w_strb: w.strb, w_valid: w.valid,
        b_ready: b.ready,
        ar_addr: ar.addr, ar_prot: ar.prot, ar_valid: ar.valid,
        r_ready: r.ready,
    }
    aw.ready = br.aw_ready
    w.ready  = br.w_ready
    b.valid  = br.b_valid
    b.resp   = br.b_resp
    ar.ready = br.ar_ready
    r.valid  = br.r_valid
    r.resp   = br.r_resp

    // Registers are full 32-bit words (read back whole, low byte drives
    // the pins) so no bit of the write bus is left unused.
    reg out_r   : u32 = 0
    reg dir_r   : u32 = 0
    reg rdata_r : u32 = 0

    on clk {
        if br.bus_we {
            match br.bus_waddr {
                0x00 => { out_r <= (out_r & ~br.bus_wmask) | br.bus_wdata }
                0x04 => { dir_r <= (dir_r & ~br.bus_wmask) | br.bus_wdata }
                _ => { }
            }
        }
        if br.bus_re {
            match br.bus_raddr {
                0x00 => { rdata_r <= out_r }
                0x04 => { rdata_r <= dir_r }
                0x08 => { rdata_r <= pins_in as u32 }
                _ => { rdata_r <= 0 }
            }
        }
    }

    r.data   = rdata_r
    pins_out = out_r[7:0] as u8
    pins_oe  = dir_r[7:0] as u8
}
