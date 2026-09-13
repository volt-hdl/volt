// SoC top level: one AXI4-Lite host port, an address decoder, and
// four peripherals on a single clock.
//
//   SocTop
//    ├── BusDecoder     (bus.volt)       address -> peripheral steering
//    ├── Gpio           (gpio.volt)      0x0000 - 0x00FF
//    ├── Timer          (timer.volt)     0x0100 - 0x01FF
//    ├── UartCtrl       (uart.volt)      0x0200 - 0x02FF
//    │    ├── SyncFifo<u8, 16>  (stdlib)
//    │    └── UartTx            (examples/uart_tx.volt, reused)
//    └── Axi4LiteSlave  (examples/axi4lite_slave.volt, reused) 0x0300 - 0x03FF
//         (four scratch registers + a read-only ID word at 0x310)
//
// Each peripheral is a complete AXI4-Lite slave (through the AxiToReg
// bridge), so the decoder only steers valid/ready/resp; addr, data,
// strb and prot fan out from the host to every peripheral.

package soc::top;

use axi4lite_slave::{AxiWriteAddr, AxiWriteData, AxiWriteResp, AxiReadAddr, AxiReadData, Axi4LiteSlave};
use soc::bus::BusDecoder;
use soc::gpio::Gpio;
use soc::timer::Timer;
use soc::uart::UartCtrl;

pub module SocTop {
    in  clk       : clock
    in  aw        : AxiWriteAddr
    in  w         : AxiWriteData
    in  b         : AxiWriteResp
    in  ar        : AxiReadAddr
    in  r         : AxiReadData
    in  gpio_in   : u8
    out gpio_out  : u8
    out gpio_oe   : u8
    out timer_irq : bool
    out uart_tx   : bool
    out sys_dbg   : u32     // XOR of the four SYSREG scratch words

    // Host-visible protocol contracts, re-stated at the top so the
    // composed system is proven, not just the parts.
    invariant: aw.ready == w.ready
    invariant: b.resp == 0 || b.resp == 2
    invariant: r.resp == 0 || r.resp == 2
    cover: b.valid && b.ready
    cover: r.valid && r.ready

    // The decoder and the peripherals reference each other. Bodies
    // resolve top-down, so the decoder's outputs are forward-declared
    // as wires, the peripherals are instantiated against the wires,
    // the decoder is instantiated against the peripheral outputs, and
    // the wires are assigned last.
    wire off_aw_addr : u32
    wire off_ar_addr : u32
    wire gpio_aw_valid  : bool
    wire gpio_w_valid   : bool
    wire gpio_b_ready   : bool
    wire gpio_ar_valid  : bool
    wire gpio_r_ready   : bool
    wire timer_aw_valid : bool
    wire timer_w_valid  : bool
    wire timer_b_ready  : bool
    wire timer_ar_valid : bool
    wire timer_r_ready  : bool
    wire uart_aw_valid  : bool
    wire uart_w_valid   : bool
    wire uart_b_ready   : bool
    wire uart_ar_valid  : bool
    wire uart_r_ready   : bool
    wire sys_aw_valid   : bool
    wire sys_w_valid    : bool
    wire sys_b_ready    : bool
    wire sys_ar_valid   : bool
    wire sys_r_ready    : bool

    let gpio_i = Gpio {
        clk: clk,
        aw_addr: off_aw_addr, aw_prot: aw.prot, aw_valid: gpio_aw_valid,
        w_data: w.data, w_strb: w.strb, w_valid: gpio_w_valid,
        b_ready: gpio_b_ready,
        ar_addr: off_ar_addr, ar_prot: ar.prot, ar_valid: gpio_ar_valid,
        r_ready: gpio_r_ready,
        pins_in: gpio_in,
    }
    let timer_i = Timer {
        clk: clk,
        aw_addr: off_aw_addr, aw_prot: aw.prot, aw_valid: timer_aw_valid,
        w_data: w.data, w_strb: w.strb, w_valid: timer_w_valid,
        b_ready: timer_b_ready,
        ar_addr: off_ar_addr, ar_prot: ar.prot, ar_valid: timer_ar_valid,
        r_ready: timer_r_ready,
    }
    let uart_i = UartCtrl {
        clk: clk,
        aw_addr: off_aw_addr, aw_prot: aw.prot, aw_valid: uart_aw_valid,
        w_data: w.data, w_strb: w.strb, w_valid: uart_w_valid,
        b_ready: uart_b_ready,
        ar_addr: off_ar_addr, ar_prot: ar.prot, ar_valid: uart_ar_valid,
        r_ready: uart_r_ready,
    }
    let sys_id : u32 = 0x50C00001
    let sys_i = Axi4LiteSlave {
        clk: clk,
        aw_addr: off_aw_addr, aw_prot: aw.prot, aw_valid: sys_aw_valid,
        w_data: w.data, w_strb: w.strb, w_valid: sys_w_valid,
        b_ready: sys_b_ready,
        ar_addr: off_ar_addr, ar_prot: ar.prot, ar_valid: sys_ar_valid,
        r_ready: sys_r_ready,
        status: sys_id,
    }

    let dec_i = BusDecoder {
        clk: clk,
        aw_addr: aw.addr,
        ar_addr: ar.addr,
        host_aw_valid: aw.valid, host_w_valid: w.valid, host_b_ready: b.ready,
        host_ar_valid: ar.valid, host_r_ready: r.ready,
        gpio_aw_ready: gpio_i.aw_ready, gpio_w_ready: gpio_i.w_ready,
        gpio_b_valid: gpio_i.b_valid, gpio_b_resp: gpio_i.b_resp,
        gpio_ar_ready: gpio_i.ar_ready, gpio_r_valid: gpio_i.r_valid,
        gpio_r_data: gpio_i.r_data, gpio_r_resp: gpio_i.r_resp,
        timer_aw_ready: timer_i.aw_ready, timer_w_ready: timer_i.w_ready,
        timer_b_valid: timer_i.b_valid, timer_b_resp: timer_i.b_resp,
        timer_ar_ready: timer_i.ar_ready, timer_r_valid: timer_i.r_valid,
        timer_r_data: timer_i.r_data, timer_r_resp: timer_i.r_resp,
        uart_aw_ready: uart_i.aw_ready, uart_w_ready: uart_i.w_ready,
        uart_b_valid: uart_i.b_valid, uart_b_resp: uart_i.b_resp,
        uart_ar_ready: uart_i.ar_ready, uart_r_valid: uart_i.r_valid,
        uart_r_data: uart_i.r_data, uart_r_resp: uart_i.r_resp,
        sys_aw_ready: sys_i.aw_ready, sys_w_ready: sys_i.w_ready,
        sys_b_valid: sys_i.b_valid, sys_b_resp: sys_i.b_resp,
        sys_ar_ready: sys_i.ar_ready, sys_r_valid: sys_i.r_valid,
        sys_r_data: sys_i.r_data, sys_r_resp: sys_i.r_resp,
    }

    off_aw_addr    = dec_i.off_aw_addr
    off_ar_addr    = dec_i.off_ar_addr
    gpio_aw_valid  = dec_i.gpio_aw_valid
    gpio_w_valid   = dec_i.gpio_w_valid
    gpio_b_ready   = dec_i.gpio_b_ready
    gpio_ar_valid  = dec_i.gpio_ar_valid
    gpio_r_ready   = dec_i.gpio_r_ready
    timer_aw_valid = dec_i.timer_aw_valid
    timer_w_valid  = dec_i.timer_w_valid
    timer_b_ready  = dec_i.timer_b_ready
    timer_ar_valid = dec_i.timer_ar_valid
    timer_r_ready  = dec_i.timer_r_ready
    uart_aw_valid  = dec_i.uart_aw_valid
    uart_w_valid   = dec_i.uart_w_valid
    uart_b_ready   = dec_i.uart_b_ready
    uart_ar_valid  = dec_i.uart_ar_valid
    uart_r_ready   = dec_i.uart_r_ready
    sys_aw_valid   = dec_i.sys_aw_valid
    sys_w_valid    = dec_i.sys_w_valid
    sys_b_ready    = dec_i.sys_b_ready
    sys_ar_valid   = dec_i.sys_ar_valid
    sys_r_ready    = dec_i.sys_r_ready

    aw.ready = dec_i.host_aw_ready
    w.ready  = dec_i.host_w_ready
    b.valid  = dec_i.host_b_valid
    b.resp   = dec_i.host_b_resp
    ar.ready = dec_i.host_ar_ready
    r.valid  = dec_i.host_r_valid
    r.data   = dec_i.host_r_data
    r.resp   = dec_i.host_r_resp

    gpio_out  = gpio_i.pins_out
    gpio_oe   = gpio_i.pins_oe
    timer_irq = timer_i.irq
    uart_tx   = uart_i.tx
    // Every instance output must be consumed: Volt has no "don't care"
    // binding, and Verilator -Wall flags unused instance outputs.
    sys_dbg   = sys_i.reg0 ^ sys_i.reg1 ^ sys_i.reg2 ^ sys_i.reg3
}
