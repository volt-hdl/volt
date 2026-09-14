// UART controller: AXI4-Lite subset in front of a 16-byte SyncFifo
// (stdlib) feeding the reused UartTx (examples/uart_tx.volt).
//
//   0x00 TXDATA  RW  writing pushes the low byte into the FIFO (ignored
//                    when full); reads return the last word written
//   0x04 STATUS  RO  bit0 fifo full, bit1 fifo empty, bit2 tx busy
//
// Draining: when the FIFO is not empty and the transmitter is idle a
// one-cycle `pop` is issued; the FIFO's registered rd_data is valid on
// the next cycle, which is exactly when `pop_r` pulses `start`.

package soc::uart;

use axi4lite_slave::{AxiAddr, AxiWData, AxiResp, AxiRData};
use soc::axi::{AxiToReg, AxiReadCtl, RegBus};
use uart_tx::UartTx;

pub module UartCtrl {
    in  clk : clock
    in  aw  : Handshake<AxiAddr>
    in  w   : Handshake<AxiWData>
    out b   : Handshake<AxiResp>
    in  ar  : Handshake<AxiAddr>
    out r   : Handshake<AxiRData>
    out tx  : bool

    // Pops are never back-to-back (the FIFO data needs a cycle to
    // land). Contracts only see ports and registers, so the natural
    // "pop_r -> !prev(tx_busy)" cannot be written: tx_busy is a wire.
    invariant: pop_r -> !prev(pop_r)
    cover: pop_r

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

    reg pop_r    : bool = false
    reg push_r   : bool = false
    reg txdata_r : u32  = 0
    reg rdata_r  : u32  = 0

    // Module bodies resolve top-down (name-resolution.md §5): the FIFO
    // needs `pop`, `pop` needs the FIFO's `empty` and the transmitter's
    // `busy`, the transmitter needs the FIFO's data. The cycle is broken
    // with forward `wire` declarations assigned after the instances.
    wire fifo_is_empty : bool
    wire tx_busy    : bool

    let wr_txdata : bool = br.bus_we && br.bus_waddr == 0x00
    let pop : bool = !fifo_is_empty && !tx_busy && !pop_r
    let tx_byte : u8 = txdata_r[7:0] as u8

    let fifo = SyncFifo<u8, 16> {
        clk: clk,
        wr_data: tx_byte,
        wr_en: push_r,
        rd_en: pop,
    }
    let u = UartTx {
        clk: clk,
        start: pop_r,
        data: fifo.rd_data,
    }
    fifo_is_empty = fifo.empty
    tx_busy    = u.busy

    let status_word : u32 = (if fifo.full { 1 } else { 0 })
                          | (if fifo.empty { 2 } else { 0 })
                          | (if u.busy { 4 } else { 0 })

    on clk {
        pop_r  <= pop
        push_r <= wr_txdata
        if wr_txdata {
            txdata_r <= (txdata_r & ~br.bus_wmask) | br.bus_wdata
        }
        if br.bus_re {
            match br.bus_raddr {
                0x00 => { rdata_r <= txdata_r }
                0x04 => { rdata_r <= status_word }
                _ => { rdata_r <= 0 }
            }
        }
    }

    r.data.data = rdata_r
    tx = u.tx
}
