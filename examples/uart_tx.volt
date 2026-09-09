// UART transmitter, 8N1 frame format: 1 start bit (low), 8 data bits
// (LSB first), no parity, 1 stop bit (high). The line idles high.
//
// Handshake: pulse `start` high for one cycle while `busy` is low to
// send `data`. `busy` stays high until the stop bit has been fully
// shifted out; `start` is ignored while `busy` is high.
//
// Baud rate configuration: one transmitted bit lasts CLKS_PER_BIT
// clock cycles, where CLKS_PER_BIT = CLK_HZ / BAUD. This example uses
// 1.8432 MHz / 460 800 baud = 4 clocks per bit, deliberately small so
// formal verification reaches the STOP state within a tractable BMC
// depth. A 50 MHz / 115 200 setup would use 434 instead — the `u10`
// counter type below is sized for exactly that (ADR-0031).

const CLKS_PER_BIT : u10 = 4

module UartTx {
    in  clk   : clock
    in  start : bool
    in  data  : u8
    out tx    : bool
    out busy  : bool

    // At most 8 data bits are ever counted in one frame.
    invariant: bit_count_r <= 8
    // The line idles high: whenever the transmitter is not busy the
    // tx output is high. Volt has no implication operator, so
    // `!busy -> tx` is written as its disjunctive form `busy || tx`.
    invariant: busy_r || tx_r
    // Induction helper: outside IDLE (state 0) the transmitter is
    // always busy. Without this, `busy_r || tx_r` alone is not
    // inductive and `--mode prove` fails.
    invariant: state_r == 0 || busy_r

    // Reachability targets: every one of the four states is visited.
    cover: state_r == 0    // IDLE
    cover: state_r == 1    // START
    cover: state_r == 2    // DATA
    cover: state_r == 3    // STOP

    // State encoding (an enum would be the natural fit once [F3]
    // enum patterns land): 0 IDLE, 1 START, 2 DATA, 3 STOP.
    reg state_r     : u2   = 0
    reg clk_count_r : u10  = 0
    reg bit_count_r : u4   = 0
    reg data_r      : u8   = 0
    reg tx_r        : bool = true
    reg busy_r      : bool = false

    on clk {
        match state_r {
            // IDLE: keep the line high, wait for a start pulse.
            0 => {
                tx_r        <= true
                busy_r      <= false
                clk_count_r <= 0
                bit_count_r <= 0
                if start {
                    data_r  <= data
                    busy_r  <= true
                    state_r <= 1
                }
            }
            // START: drive the start bit (low) for one full bit period.
            1 => {
                tx_r <= false
                if clk_count_r == CLKS_PER_BIT - 1 {
                    clk_count_r <= 0
                    state_r     <= 2
                } else {
                    clk_count_r <= clk_count_r + 1
                }
            }
            // DATA: shift the frame out LSB first.
            2 => {
                tx_r <= data_r[0]
                if clk_count_r == CLKS_PER_BIT - 1 {
                    clk_count_r <= 0
                    data_r      <= data_r >> 1
                    // `>=` instead of `==` keeps the bit_count invariant
                    // inductive: even from an unreachable count above 7
                    // the machine falls through to STOP.
                    if bit_count_r >= 7 {
                        bit_count_r <= 0
                        state_r     <= 3
                    } else {
                        bit_count_r <= bit_count_r + 1
                    }
                } else {
                    clk_count_r <= clk_count_r + 1
                }
            }
            // STOP (any unreachable encoding also drains here):
            // drive the stop bit (high), then release the bus.
            _ => {
                tx_r <= true
                if clk_count_r == CLKS_PER_BIT - 1 {
                    clk_count_r <= 0
                    busy_r      <= false
                    state_r     <= 0
                } else {
                    clk_count_r <= clk_count_r + 1
                }
            }
        }
    }

    tx   = tx_r
    busy = busy_r
}
