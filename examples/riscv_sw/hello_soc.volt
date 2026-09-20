// Simulation SoC: RiscvCore + a program memory + a small RAM + a UART
// receiver that collects what the program prints.
//
// The program is not part of this file: the test reads hello.hex with
// read_hex() and writes it into `imem` with load() (ADR-0058), so a
// new program needs no new hardware. In real hardware the same memory
// is filled through the prog_* port (a boot loader, a debugger); the
// test leaves that port idle. A test still cannot watch a serial line
// by itself, so the receiver stays in hardware and shows the result
// through plain ports:
//   rx_count          bytes received so far
//   rx_byte           rx_buf[rx_sel], the test pages through the text
//   halted            the program reached `halt: j halt` (start.S)
//   trap_seen         sticky: any trap at all (none is expected)
//   unexpected        sticky: a bus access outside ROM/RAM, a store to
//                     ROM, a pc outside the program, an interrupt or
//                     an MRET — nothing hello.c should ever cause
//   cycles, instrs    clock cycles / retired instructions until halt
//
// Memory map (link.ld): program memory at 0x0000_0000 on both the
// instruction and the data bus (the string constants live in .rodata),
// 1024 words (4 KB) that the program can only read; RAM at
// 0x1000_0000. The linker reserves 64 KB of RAM; this model keeps the
// simulation small with 64 words (256 bytes) that alias through the whole
// window, so the stack at the top of RAM lands on the last words.
// 0x2000_0000 (UART) is decoded inside the core and never shows up here.

use riscv_core::RiscvCore;

const HALT_INSTR : u32 = 0x0000006F    // jal x0, 0
const UART_CLKS_PER_BIT : u6 = 4       // uart_tx.volt CLKS_PER_BIT
const UART_DATA_END : u6 = 36          // first cycle of the stop bit
const UART_STOP_AT : u6 = 37           // inside the stop bit (36..39)

pub module HelloSoc {
    in  clk       : clock
    in  prog_we   : bool     // program port: imem[prog_addr] <= prog_data
    in  prog_addr : u10
    in  prog_data : u32
    in  rx_sel    : u5
    out rx_count  : u8
    out rx_byte   : u8
    out halted    : bool
    out trap_seen : bool
    out unexpected : bool
    out cycles    : u32
    out instrs    : u32

    // A zero word is an illegal instruction, so running off the end of
    // the program traps.
    reg imem     : [u32; 1024] = [0; 1024]
    reg ram      : [u32; 64] = [0; 64]
    reg rx_buf   : [u8; 32]  = [0; 32]
    reg rx_cnt   : u8   = 0
    reg rx_busy  : bool = false
    reg rx_t     : u6   = 0
    reg rx_sh    : u8   = 0
    reg trap_r   : bool = false
    reg odd_r    : bool = false
    reg cycles_r : u32  = 0
    reg instrs_r : u32  = 0

    // ── Core + memories ───────────────────────────────────────────
    // The core and its memories feed each other (pc -> instr,
    // mem_addr -> mem_rdata), so the two read buses are forward wires.
    wire instr_w : u32
    wire rdata_w : u32

    let cpu = RiscvCore {
        clk: clk,
        instr: instr_w,
        mem_rdata: rdata_w,
        irq: false,
    }
    let pc_w   : u32 = cpu.pc
    let addr_w : u32 = cpu.mem_addr
    let is_rom = (addr_w >> 28) == 0
    let is_ram = (addr_w >> 28) == 1
    let ram_i  = addr_w[7:2] as u6

    // Addresses wrap inside imem (bits [11:2] index it); ifault / dfault
    // report an address that does not belong to it.
    let ifault = (pc_w >> 12) != 0 || (pc_w & 3) != 0
    let dfault = (addr_w >> 12) != 0
    instr_w = imem[pc_w[11:2] as u10]
    rdata_w = if is_ram { ram[ram_i] } else { imem[addr_w[11:2] as u10] }

    // Byte-lane write: the core presents a full word plus a lane mask.
    let wmask = cpu.mem_wmask
    let lanes : u32 =
          (if wmask[0] { 0x000000FF } else { 0 })
        | (if wmask[1] { 0x0000FF00 } else { 0 })
        | (if wmask[2] { 0x00FF0000 } else { 0 })
        | (if wmask[3] { 0xFF000000 } else { 0 })
    let ram_old = ram[ram_i]
    let ram_new = (ram_old & (lanes ^ 0xFFFFFFFF)) | (cpu.mem_wdata & lanes)

    // ── Sanity monitor ────────────────────────────────────────────
    // The lane mask is a u8 on the core, only four lanes exist.
    let bus_rd = cpu.mem_read
    let bus_wr = cpu.mem_write
    let bad_bus = (bus_rd && !is_ram && !(is_rom && !dfault))
               || (bus_wr && (!is_ram || (wmask >> 4) != 0))
    let odd_w = bad_bus || ifault
             || cpu.irq_ack_o || cpu.mret_o || cpu.cyc_wrap_o

    // ── UART receiver (8N1, same clock as the transmitter) ────────
    // rx_t counts cycles since the start bit's first low cycle; data
    // bit k spans 4+4k .. 7+4k and is sampled one cycle in.
    let txd = cpu.uart_txd
    let rx_data_phase = rx_t >= UART_CLKS_PER_BIT && rx_t < UART_DATA_END
    let rx_sample = rx_data_phase && (rx_t & 3) == 1
    let rx_in : u8 = if txd { 0x80 } else { 0 }

    let halt_w = instr_w == HALT_INSTR && !trap_r

    on clk {
        if prog_we {
            imem[prog_addr] <= prog_data
        }
        if bus_wr && is_ram {
            ram[ram_i] <= ram_new
        }

        if !rx_busy {
            if !txd {
                rx_busy <= true
                rx_t    <= 1
            }
        } else {
            rx_t <= rx_t + 1
            if rx_sample {
                rx_sh <= (rx_sh >> 1) | rx_in
            }
            if rx_t == UART_STOP_AT {
                rx_busy <= false
                rx_buf[rx_cnt[4:0] as u5] <= rx_sh
                rx_cnt  <= rx_cnt + 1
            }
        }

        if cpu.trap_o {
            trap_r <= true
        }
        if odd_w {
            odd_r <= true
        }
        if !halt_w {
            cycles_r <= cycles_r + 1
            if !cpu.stall_o && !cpu.trap_o {
                instrs_r <= instrs_r + 1
            }
        }
    }

    rx_count  = rx_cnt
    rx_byte   = rx_buf[rx_sel]
    halted    = halt_w
    trap_seen = trap_r
    unexpected = odd_r
    cycles    = cycles_r
    instrs    = instrs_r
}
