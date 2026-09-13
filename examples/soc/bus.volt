// SoC bus layer: the address decoder that steers the host's AXI4-Lite
// channels to one peripheral.
//
// Address map (bits [31:8] select the peripheral, [7:0] the register):
//   0x0000_0000 - 0x0000_00FF  GPIO
//   0x0000_0100 - 0x0000_01FF  Timer
//   0x0000_0200 - 0x0000_02FF  UART
//   0x0000_0300 - 0x0000_03FF  SYSREG (the reused Axi4LiteSlave)
//   anything else              SLVERR from the decoder itself
//
// `AxiSel` is private to this file: only BusDecoder's ports mention it,
// and the flattened port names are what SocTop binds.

package soc::bus;

// One decoder-to-peripheral link, from the decoder's point of view.
// Only the handshake and response signals are steered; addr/data/strb/
// prot fan out from the host straight to every peripheral (they are
// don't-care while *_valid is low).
struct port AxiSel {
    out aw_valid : bool
    in  aw_ready : bool
    out w_valid  : bool
    in  w_ready  : bool
    in  b_valid  : bool
    in  b_resp   : u2
    out b_ready  : bool
    out ar_valid : bool
    in  ar_ready : bool
    in  r_valid  : bool
    in  r_data   : u32
    in  r_resp   : u2
    out r_ready  : bool
}

pub module BusDecoder {
    in  clk     : clock
    in  aw_addr : u32
    in  ar_addr : u32
    in  host    : AxiSel      // flipped: the host drives *_valid, we drive *_ready
    out gpio    : AxiSel
    out timer   : AxiSel
    out uart    : AxiSel
    out sys     : AxiSel
    out off_aw_addr : u32     // aw_addr with the page bits cleared
    out off_ar_addr : u32

    // At most one peripheral sees a write request at a time.
    invariant: !(gpio.aw_valid && timer.aw_valid)
    invariant: !(gpio.aw_valid && uart.aw_valid)
    invariant: !(gpio.aw_valid && sys.aw_valid)
    invariant: !(timer.aw_valid && uart.aw_valid)
    invariant: !(timer.aw_valid && sys.aw_valid)
    invariant: !(uart.aw_valid && sys.aw_valid)
    // At most one peripheral sees a read request at a time.
    invariant: !(gpio.ar_valid && timer.ar_valid)
    invariant: !(gpio.ar_valid && uart.ar_valid)
    invariant: !(gpio.ar_valid && sys.ar_valid)
    invariant: !(timer.ar_valid && uart.ar_valid)
    invariant: !(timer.ar_valid && sys.ar_valid)
    invariant: !(uart.ar_valid && sys.ar_valid)
    // An accepted request outside the map is answered with SLVERR.
    invariant: prev(host.aw_valid) && prev(host.aw_ready) && prev(aw_addr) >= 0x400
               -> host.b_valid && host.b_resp == 2
    invariant: prev(host.ar_valid) && prev(host.ar_ready) && prev(ar_addr) >= 0x400
               -> host.r_valid && host.r_resp == 2
    // Every peripheral is reachable.
    cover: gpio.aw_valid && gpio.aw_ready
    cover: timer.aw_valid && timer.aw_ready
    cover: uart.aw_valid && uart.aw_ready
    cover: sys.aw_valid && sys.aw_ready
    cover: gpio.ar_valid && gpio.ar_ready
    cover: timer.ar_valid && timer.ar_ready
    cover: uart.ar_valid && uart.ar_ready
    cover: sys.ar_valid && sys.ar_ready
    cover: host.b_valid && host.b_resp == 2
    cover: host.r_valid && host.r_resp == 2

    // Page index of the current request: 0..3 valid, anything else bad.
    let aw_page : u32 = aw_addr >> 8
    let ar_page : u32 = ar_addr >> 8
    let aw_bad : bool = aw_page >= 4
    let ar_bad : bool = ar_page >= 4

    // Which peripheral owns the pending response (4 = decoder's SLVERR).
    reg wsel_r : u3 = 0
    reg rsel_r : u3 = 0
    // Decoder-generated error responses.
    reg bad_bvalid_r : bool = false
    reg bad_rvalid_r : bool = false

    let bad_wr_fire : bool = host.aw_valid && host.w_valid && aw_bad && !bad_bvalid_r
    let bad_rd_fire : bool = host.ar_valid && ar_bad && !bad_rvalid_r

    on clk {
        if host.aw_valid && host.aw_ready {
            wsel_r <= if aw_bad { 4 } else { aw_page[2:0] as u3 }
        }
        if host.ar_valid && host.ar_ready {
            rsel_r <= if ar_bad { 4 } else { ar_page[2:0] as u3 }
        }
        if bad_wr_fire {
            bad_bvalid_r <= true
        }
        if bad_bvalid_r && host.b_ready {
            bad_bvalid_r <= false
        }
        if bad_rd_fire {
            bad_rvalid_r <= true
        }
        if bad_rvalid_r && host.r_ready {
            bad_rvalid_r <= false
        }
    }

    off_aw_addr = aw_addr & 0xFF
    off_ar_addr = ar_addr & 0xFF

    // Request steering (combinational).
    gpio.aw_valid  = host.aw_valid && aw_page == 0
    timer.aw_valid = host.aw_valid && aw_page == 1
    uart.aw_valid  = host.aw_valid && aw_page == 2
    sys.aw_valid   = host.aw_valid && aw_page == 3
    gpio.w_valid   = host.w_valid && aw_page == 0
    timer.w_valid  = host.w_valid && aw_page == 1
    uart.w_valid   = host.w_valid && aw_page == 2
    sys.w_valid    = host.w_valid && aw_page == 3
    gpio.ar_valid  = host.ar_valid && ar_page == 0
    timer.ar_valid = host.ar_valid && ar_page == 1
    uart.ar_valid  = host.ar_valid && ar_page == 2
    sys.ar_valid   = host.ar_valid && ar_page == 3

    host.aw_ready = (aw_page == 0 && gpio.aw_ready)
                  || (aw_page == 1 && timer.aw_ready)
                  || (aw_page == 2 && uart.aw_ready)
                  || (aw_page == 3 && sys.aw_ready)
                  || bad_wr_fire
    host.w_ready  = (aw_page == 0 && gpio.w_ready)
                  || (aw_page == 1 && timer.w_ready)
                  || (aw_page == 2 && uart.w_ready)
                  || (aw_page == 3 && sys.w_ready)
                  || bad_wr_fire
    host.ar_ready = (ar_page == 0 && gpio.ar_ready)
                  || (ar_page == 1 && timer.ar_ready)
                  || (ar_page == 2 && uart.ar_ready)
                  || (ar_page == 3 && sys.ar_ready)
                  || bad_rd_fire

    // Response steering follows the registered owner. (`match` as an
    // expression parses but is E0003 in SV generation, hence if-chains.)
    host.b_valid = if wsel_r == 0 { gpio.b_valid }
                   else if wsel_r == 1 { timer.b_valid }
                   else if wsel_r == 2 { uart.b_valid }
                   else if wsel_r == 3 { sys.b_valid }
                   else { bad_bvalid_r }
    host.b_resp  = if wsel_r == 0 { gpio.b_resp }
                   else if wsel_r == 1 { timer.b_resp }
                   else if wsel_r == 2 { uart.b_resp }
                   else if wsel_r == 3 { sys.b_resp }
                   else { 2 }
    gpio.b_ready  = host.b_ready && wsel_r == 0
    timer.b_ready = host.b_ready && wsel_r == 1
    uart.b_ready  = host.b_ready && wsel_r == 2
    sys.b_ready   = host.b_ready && wsel_r == 3

    host.r_valid = if rsel_r == 0 { gpio.r_valid }
                   else if rsel_r == 1 { timer.r_valid }
                   else if rsel_r == 2 { uart.r_valid }
                   else if rsel_r == 3 { sys.r_valid }
                   else { bad_rvalid_r }
    host.r_resp  = if rsel_r == 0 { gpio.r_resp }
                   else if rsel_r == 1 { timer.r_resp }
                   else if rsel_r == 2 { uart.r_resp }
                   else if rsel_r == 3 { sys.r_resp }
                   else { 2 }
    host.r_data  = if rsel_r == 0 { gpio.r_data }
                   else if rsel_r == 1 { timer.r_data }
                   else if rsel_r == 2 { uart.r_data }
                   else if rsel_r == 3 { sys.r_data }
                   else { 0 }
    gpio.r_ready  = host.r_ready && rsel_r == 0
    timer.r_ready = host.r_ready && rsel_r == 1
    uart.r_ready  = host.r_ready && rsel_r == 2
    sys.r_ready   = host.r_ready && rsel_r == 3
}
