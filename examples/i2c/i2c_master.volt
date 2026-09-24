// I2C master controller (single-byte write / read, 7-bit addressing,
// START / STOP / REPEATED START, ACK/NACK, clock stretching).
//
// Open-drain pad interface (ADR-0051). I2C lines are pulled up by an
// external resistor; a device only ever pulls a line LOW:
//
//     opendrain sda : bool      the pad itself; the compiler synthesises
//     opendrain scl : bool      sda_drive_low / scl_drive_low registers
//
//     sda.drive_low()           pull the line low     (on clk only)
//     sda.release()             leave it to the pull-up
//     sda.read()                the resolved line level
//     sda.released / .driving   the drive intent, for contracts
//
// The generated SV is `assign sda = sda_drive_low ? 1'b0 : 1'bz` — no
// hand-written wrapper. The other end of each line is a slave with its
// own timing, so both lines are read through sync() (two flops, two
// clocks of latency); a direct sda.read() in the FSM would be W3007.
//
// Bit timing. Every bus event (START, data bit, ACK slot, STOP) is one
// bit period of four equal phases, CLKS_PER_BIT / 4 system clocks each:
//
//     phase 0   SCL held low, SDA set for this bit
//     phase 1   SCL released; the phase does not END until scl_s reads
//               high (a slave may stretch the clock here; the two
//               synchroniser clocks also count as a short stall)
//     phase 2   SCL high, SDA stable; the master samples sda_s at the
//               end of this phase
//     phase 3   SCL pulled low again
//
// The system clock is 3.2 MHz in this example: standard mode (100 kHz)
// is 32 clocks per bit, fast mode (400 kHz) is 8. Both are deliberately
// small so formal verification reaches STOP within a tractable depth; a
// 50 MHz / 100 kHz design would use 500 / 125. In fast mode the
// synchroniser latency shows: phase 1 has to wait one extra clock for
// scl_s, so a bit takes 9 clocks (355 kHz) instead of 8.
//
// Command interface: pulse `start` with `addr`, `rw`, `wr_data`
// valid. `busy` rises on the next edge and falls after STOP; `done`
// pulses for one cycle with `ack_ok` (address acknowledged, and for a
// write the data byte as well) and, for a read, `rd_data`. A `start`
// pulse while `busy` is high queues a REPEATED START: after the data
// ACK slot the master issues a new START instead of STOP and samples
// `addr` / `rw` / `wr_data` again at that moment.

package i2c::i2c_master;

// Master states (ADR-0074). Seven variants in a 3-bit binary encoding
// (Idle = 0 .. Stop = 6); code 7 belongs to no variant, and the
// compiler adds the state-valid invariant for it (ADR-0066 F1).
pub enum I2cState { Idle, Start, Addr, AddrAck, Data, DataAck, Stop }

// 3.2 MHz / 100 kHz and 3.2 MHz / 400 kHz.
const CLKS_PER_BIT_STD  : u6 = 32
const CLKS_PER_BIT_FAST : u6 = 8
const CLKS_PER_PHASE_STD  : u6 = CLKS_PER_BIT_STD / 4
const CLKS_PER_PHASE_FAST : u6 = CLKS_PER_BIT_FAST / 4

pub module I2cMaster {
    in  clk       : clock

    // Command side.
    in  start     : bool
    in  addr      : u7
    in  rw        : bool    // false = write, true = read
    in  wr_data   : u8
    in  fast_mode : bool    // false = 100 kHz, true = 400 kHz
    out rd_data   : u8
    out busy      : bool
    out done      : bool
    out ack_ok    : bool

    // The bus: two open-drain pads (see the header comment).
    opendrain sda : bool
    opendrain scl : bool

    // Observability for tests and formal: current state, whether a slave
    // stretched the clock, and whether the last START was a repeated one.
    out state     : I2cState
    out stretched : bool
    out repeated  : bool

    // ── Contracts ──────────────────────────────────────────────
    // "state_r is one of the seven variants" (code 7 is never reached)
    // is not written here: the compiler generates it from I2cState.
    invariant: bit_count_r <= 8
    invariant: clk_count_r < CLKS_PER_PHASE_STD
    // An idle master leaves both lines to the pull-ups. This is the
    // drive INTENT (the synthesised drive registers), not the z value:
    // formal proves the master never pulls a line while idle.
    invariant: !busy_r -> (sda.released && scl.released)
    // Induction helpers: outside IDLE the master is busy; the phase
    // counter only runs while busy.
    invariant: state_r != I2cState::Idle -> busy_r
    invariant: !busy_r -> (phase_r == 0 && clk_count_r == 0 && !stall_r)

    cover: ack_seen_r                       // a slave acknowledged
    cover: nack_seen_r                      // a slave (or the master) did not
    cover: stretched_r                      // a slave stretched SCL
    cover: state_r == I2cState::Stop        // STOP reached
    cover: state_r == I2cState::Start && repeated_r   // REPEATED START issued
    cover: done_r && rw_r                   // a read completed
    cover: sda.driving && scl.driving       // both lines held (START p3, STOP p0)

    // ── Line inputs: synchronised (ADR-0051, W3007) ────────────
    wire sda_s : bool
    wire scl_s : bool
    sda_s = sync(sda.read(), clk)
    scl_s = sync(scl.read(), clk)

    // ── State ──────────────────────────────────────────────────
    reg state_r     : I2cState = I2cState::Idle
    reg phase_r     : u2  = 0
    reg clk_count_r : u6  = 0
    reg stall_r     : bool = false       // phase 1 waited for scl_s last clock
    reg bit_count_r : u4  = 0
    reg shift_r     : u8  = 0
    reg rd_data_r   : u8  = 0
    reg addr_r      : u7  = 0
    reg rw_r        : bool = false
    reg wr_data_r   : u8  = 0
    reg busy_r      : bool = false
    reg done_r      : bool = false
    reg ack_r       : bool = false       // ACK sampled in the current slot
    reg ack_ok_r    : bool = false
    reg ack_seen_r  : bool = false
    reg nack_seen_r : bool = false
    reg pending_r   : bool = false       // start pulse seen while busy
    reg repeated_r  : bool = false       // current START is a repeated one
    reg stretched_r : bool = false

    // ── Bit-period clock divider ───────────────────────────────
    let clks_per_phase : u6 = if fast_mode { CLKS_PER_PHASE_FAST } else { CLKS_PER_PHASE_STD }
    let phase_end = clk_count_r >= clks_per_phase - 1
    // Phase 1 is "SCL released": it only ends once the line really reads
    // high. A slave holding SCL low (clock stretching) — or just the two
    // synchroniser clocks — stalls the divider here.
    let stalling = busy_r && phase_end && phase_r == 1 && !scl_s
    let tick = busy_r && phase_end && !stalling

    on clk {
        done_r <= false

        // Divider: runs only while busy; holds while phase 1 waits for SCL.
        if !busy_r {
            clk_count_r <= 0
            phase_r     <= 0
            stall_r     <= false
        } else if tick {
            clk_count_r <= 0
            phase_r     <= phase_r + 1
            stall_r     <= false
        } else if stalling {
            // A second stalled clock is more than the synchroniser
            // latency explains: a slave is stretching the clock.
            stall_r <= true
            if stall_r {
                stretched_r <= true
            }
        } else {
            clk_count_r <= clk_count_r + 1
        }

        // A start pulse during a transaction queues a repeated start.
        if start && busy_r {
            pending_r <= true
        }

        match state_r {
            // IDLE: lines released, wait for a command.
            I2cState::Idle => {
                sda.release()
                scl.release()
                if start {
                    addr_r      <= addr
                    rw_r        <= rw
                    wr_data_r   <= wr_data
                    busy_r      <= true
                    ack_ok_r    <= false
                    stretched_r <= false
                    repeated_r  <= false
                    state_r     <= I2cState::Start
                }
            }
            // START: SDA falls while SCL is high.
            //   p0 SCL low (or still high from idle), SDA released
            //   p1 SCL released   p2 SDA pulled low = START   p3 SCL low
            I2cState::Start => {
                if tick {
                    match phase_r {
                        0 => { scl.release() }
                        1 => { sda.drive_low() }
                        2 => { scl.drive_low() }
                        _ => {
                            shift_r     <= (addr_r as u8) << 1 | (if rw_r { 1 } else { 0 })
                            bit_count_r <= 0
                            if addr_r[6] { sda.release() } else { sda.drive_low() }
                            state_r     <= I2cState::Addr
                        }
                    }
                }
            }
            // ADDR: 7 address bits + R/W, MSB first, one bit per period.
            I2cState::Addr => {
                if tick {
                    match phase_r {
                        0 => { scl.release() }
                        1 => { }
                        2 => { scl.drive_low() }
                        _ => {
                            if bit_count_r >= 7 {
                                bit_count_r <= 0
                                sda.release()              // release SDA for the ACK slot
                                state_r     <= I2cState::AddrAck
                            } else {
                                bit_count_r <= bit_count_r + 1
                                shift_r     <= shift_r << 1
                                if shift_r[6] { sda.release() } else { sda.drive_low() }
                            }
                        }
                    }
                }
            }
            // ADDR_ACK: the slave pulls SDA low while SCL is high.
            I2cState::AddrAck => {
                if tick {
                    match phase_r {
                        0 => { scl.release() }
                        1 => { }
                        2 => {
                            scl.drive_low()
                            ack_r       <= !sda_s
                            ack_seen_r  <= ack_seen_r || !sda_s
                            nack_seen_r <= nack_seen_r || sda_s
                        }
                        _ => {
                            if !ack_r {
                                sda.drive_low()            // NACK: go straight to STOP
                                state_r  <= I2cState::Stop
                            } else if rw_r {
                                sda.release()              // read: the slave drives SDA
                                bit_count_r <= 0
                                state_r     <= I2cState::Data
                            } else {
                                shift_r     <= wr_data_r
                                if wr_data_r[7] { sda.release() } else { sda.drive_low() }
                                bit_count_r <= 0
                                state_r     <= I2cState::Data
                            }
                        }
                    }
                }
            }
            // DATA: write shifts wr_data out, read samples sda_s.
            I2cState::Data => {
                if tick {
                    match phase_r {
                        0 => { scl.release() }
                        1 => { }
                        2 => {
                            scl.drive_low()
                            if rw_r {
                                rd_data_r <= rd_data_r << 1 | (if sda_s { 1 } else { 0 })
                            }
                        }
                        _ => {
                            if bit_count_r >= 7 {
                                bit_count_r <= 0
                                sda.release()              // write: ACK slot; read: NACK (last byte)
                                state_r     <= I2cState::DataAck
                            } else {
                                bit_count_r <= bit_count_r + 1
                                shift_r     <= shift_r << 1
                                if !rw_r {
                                    if shift_r[6] { sda.release() } else { sda.drive_low() }
                                }
                            }
                        }
                    }
                }
            }
            // DATA_ACK: write = slave ACK, read = master NACK (SDA left high).
            I2cState::DataAck => {
                if tick {
                    match phase_r {
                        0 => { scl.release() }
                        1 => { }
                        2 => {
                            scl.drive_low()
                            if rw_r {
                                ack_r       <= true        // the master's own NACK ends a read cleanly
                                nack_seen_r <= true
                            } else {
                                ack_r       <= !sda_s
                                ack_seen_r  <= ack_seen_r || !sda_s
                                nack_seen_r <= nack_seen_r || sda_s
                            }
                        }
                        _ => {
                            ack_ok_r <= ack_r
                            if pending_r {
                                pending_r  <= false
                                repeated_r <= true
                                addr_r     <= addr
                                rw_r       <= rw
                                wr_data_r  <= wr_data
                                sda.release()              // START p0 wants SDA released
                                state_r    <= I2cState::Start
                            } else {
                                sda.drive_low()            // SDA low ahead of STOP
                                state_r  <= I2cState::Stop
                            }
                        }
                    }
                }
            }
            // STOP: SDA rises while SCL is high, then both lines are left
            // released. The match names every variant, so there is no `_`
            // arm: this last arm becomes the SV `default` and also takes
            // the unused code 7, as the numeric `_ =>` arm did.
            I2cState::Stop => {
                if tick {
                    match phase_r {
                        0 => { scl.release() }
                        1 => { sda.release() }
                        2 => { }
                        _ => {
                            // Already released in p1/p2; repeated here so the
                            // idle-line invariant is inductive from any state.
                            sda.release()
                            scl.release()
                            busy_r   <= false
                            done_r   <= true
                            state_r  <= I2cState::Idle
                        }
                    }
                }
            }
        }
    }

    rd_data   = rd_data_r
    busy      = busy_r
    done      = done_r
    ack_ok    = ack_ok_r
    state     = state_r
    stretched = stretched_r
    repeated  = repeated_r
}
