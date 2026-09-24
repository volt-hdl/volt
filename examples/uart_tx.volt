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

// Transmitter states (ADR-0074). Four variants fill the 2-bit binary
// encoding exactly (Idle = 0 .. Stop = 3), so every code is a valid
// state and no state-valid invariant is needed.
enum TxState { Idle, Start, Data, Stop }

pub module UartTx {
    in  clk   : clock
    in  start : bool
    in  data  : u8
    out tx    : bool
    out busy  : bool

    // At most 8 data bits are ever counted in one frame.
    invariant: bit_count_r <= 8
    // The line idles high: whenever the transmitter is not busy the
    // tx output is high (ADR-0034 implication, SVA: !busy_r |-> tx_r).
    invariant: !busy_r -> tx_r
    // Induction helper: outside Idle the transmitter is always busy.
    // Without this, the idle-line invariant alone is not inductive and
    // `--mode prove` fails.
    invariant: state_r != TxState::Idle -> busy_r

    // Reachability targets: every one of the four states is visited.
    cover: state_r == TxState::Idle
    cover: state_r == TxState::Start
    cover: state_r == TxState::Data
    cover: state_r == TxState::Stop

    reg state_r     : TxState = TxState::Idle
    reg clk_count_r : u10  = 0
    reg bit_count_r : u4   = 0
    reg data_r      : u8   = 0
    reg tx_r        : bool = true
    reg busy_r      : bool = false

    on clk {
        match state_r {
            // Idle: keep the line high, wait for a start pulse.
            TxState::Idle => {
                tx_r        <= true
                busy_r      <= false
                clk_count_r <= 0
                bit_count_r <= 0
                if start {
                    data_r  <= data
                    busy_r  <= true
                    state_r <= TxState::Start
                }
            }
            // Start: drive the start bit (low) for one full bit period.
            TxState::Start => {
                tx_r <= false
                if clk_count_r == CLKS_PER_BIT - 1 {
                    clk_count_r <= 0
                    state_r     <= TxState::Data
                } else {
                    clk_count_r <= clk_count_r + 1
                }
            }
            // Data: shift the frame out LSB first.
            TxState::Data => {
                tx_r <= data_r[0]
                if clk_count_r == CLKS_PER_BIT - 1 {
                    clk_count_r <= 0
                    data_r      <= data_r >> 1
                    // `>=` instead of `==` keeps the bit_count invariant
                    // inductive: even from an unreachable count above 7
                    // the machine falls through to Stop.
                    if bit_count_r >= 7 {
                        bit_count_r <= 0
                        state_r     <= TxState::Stop
                    } else {
                        bit_count_r <= bit_count_r + 1
                    }
                } else {
                    clk_count_r <= clk_count_r + 1
                }
            }
            // Stop: drive the stop bit (high), then release the bus. The
            // match names every variant, so no `_` arm is needed; this
            // last arm becomes the SV `default`.
            TxState::Stop => {
                tx_r <= true
                if clk_count_r == CLKS_PER_BIT - 1 {
                    clk_count_r <= 0
                    busy_r      <= false
                    state_r     <= TxState::Idle
                } else {
                    clk_count_r <= clk_count_r + 1
                }
            }
        }
    }

    tx   = tx_r
    busy = busy_r
}
