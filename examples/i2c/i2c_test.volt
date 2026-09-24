// Simulation tests for the I2C master (ADR-0033).
//
// There is no sibling i2c.volt: the master comes in through `use`
// (ADR-0042). Test scripts are linear (set a port, step, assert), so
// the slave cannot be scripted -- it is a Volt module in this file,
// `I2cSlaveModel`, with the same two `opendrain` pads as the master
// (ADR-0051). `I2cTb` declares two plain wires and binds both devices
// to them; the compiler emits them as `tri1` nets (pull-up + wired-AND)
// and every device's `assign sda = sda_drive_low ? 1'b0 : 1'bz` drives
// the shared net -- there is no hand-written bus model any more.
// `volt test` can only drive `in` and read `out` ports of the DUT
// (E8503 / E8504), so I2cTb re-exports the resolved bus levels and the
// slave's observations as plain outputs.
//
// Both devices read the lines through sync() (two clocks of latency);
// the slave detects edges one clock after that, so it sees a bus event
// three clocks after it happened.
//
// Timing model (standard mode, CLKS_PER_BIT = 32, 8 clocks per phase;
// E = the edge that samples the start pulse; SCL is released at the
// phase-0 tick and pulled low at the phase-2 tick of every bit):
//   START      E      .. E+32    SDA falls at E+16 with SCL high
//   ADDR       E+32   .. E+288   bit k occupies E+32+32k .. E+64+32k,
//                                SCL high E+40+32k .. E+56+32k
//   ADDR_ACK   E+288  .. E+320   slave drives ACK from E+283, master
//                                samples at E+312 (sda_s = line at E+309)
//   DATA       E+320  .. E+576
//   DATA_ACK   E+576  .. E+608   sampled at E+600
//   STOP       E+608  .. E+640   SCL rises at E+616, SDA rises at E+624
//   IDLE       E+640            busy falls, done pulses
// A NACK on the address goes straight to STOP: idle at E+352.
// Fast mode is two clocks per phase, but phase 1 also waits one extra
// clock for the synchronised SCL: 9 clocks per bit, START 8, so the
// whole write takes 179 clocks.

package i2c::i2c_test;

use i2c::i2c_master::{I2cMaster, I2cState};

// System clocks a stretching slave holds SCL low after the address ACK.
// The master's phase 0 + phase 1 already span 16 clocks after the ACK
// slot; 40 makes the stretch clearly longer than that (22 clocks).
const STRETCH_CYCLES : u6 = 40

// Behavioural I2C slave: one 7-bit address, ACKs when `ack_en`,
// receives one byte on a write, returns `tx_data` on a read, and
// optionally stretches the clock after the address ACK slot. Bus edges
// are detected on the synchronised lines (three clocks late), which is
// how a real synchronous slave sees the bus as well.
module I2cSlaveModel {
    in  clk        : clock
    opendrain scl  : bool
    opendrain sda  : bool
    in  my_addr    : u7
    in  tx_data    : u8
    in  ack_en     : bool
    in  stretch_en : bool
    out got_byte   : u8      // last complete byte seen on the bus
    out byte_cnt   : u4      // bytes since the last START
    out start_cnt  : u4      // START conditions seen (repeated ones included)
    out stop_cnt   : u4      // STOP conditions seen
    out addr_hit   : bool    // last address byte matched my_addr
    out master_ack : bool    // the master's ACK/NACK after a read byte
    out scl_held   : bool    // the slave is stretching the clock

    // Wherever the slave is, it never drives SDA and SCL at once
    // (SCL only while stretching, SDA only in an ACK slot or read bit).
    invariant: !(stretch_r != 0 && sda.driving && !reading_r)
    invariant: bit_r <= 9

    wire scl_s : bool
    wire sda_s : bool
    scl_s = sync(scl.read(), clk)
    sda_s = sync(sda.read(), clk)

    reg scl_q_r      : bool = true
    reg sda_q_r      : bool = true
    reg active_r     : bool = false
    reg is_addr_r    : bool = true
    reg reading_r    : bool = false    // master reads: the slave drives data bits
    reg bit_r        : u4   = 0        // 0..7 data bits, 8 = ACK slot begun, 9 = ACK sampled
    reg shift_r      : u8   = 0
    reg tx_shift_r   : u8   = 0
    reg got_byte_r   : u8   = 0
    reg byte_cnt_r   : u4   = 0
    reg start_cnt_r  : u4   = 0
    reg stop_cnt_r   : u4   = 0
    reg addr_hit_r   : bool = false
    reg master_ack_r : bool = false
    reg stretch_r    : u6   = 0

    let scl_rise   = scl_s && !scl_q_r
    let scl_fall   = !scl_s && scl_q_r
    // START: SDA falls while SCL is high. STOP: SDA rises while SCL is high.
    let start_cond = scl_s && scl_q_r && sda_q_r && !sda_s
    let stop_cond  = scl_s && scl_q_r && !sda_q_r && sda_s
    let addr_match = (shift_r >> 1) == (my_addr as u8)

    on clk {
        scl_q_r <= scl_s
        sda_q_r <= sda_s

        // Clock stretching: hold SCL for STRETCH_CYCLES, then release.
        if stretch_r != 0 {
            stretch_r <= stretch_r - 1
            if stretch_r == 1 {
                scl.release()
            }
        }

        if start_cond {
            active_r    <= true
            is_addr_r   <= true
            reading_r   <= false
            bit_r       <= 0
            byte_cnt_r  <= 0
            start_cnt_r <= start_cnt_r + 1
            sda.release()
        } else if stop_cond {
            active_r   <= false
            stop_cnt_r <= stop_cnt_r + 1
            sda.release()
        } else if active_r {
            if scl_rise {
                if bit_r < 8 {
                    // Data bit (for a read this is the slave's own bit).
                    shift_r <= shift_r << 1 | (if sda_s { 1 } else { 0 })
                    bit_r   <= bit_r + 1
                } else {
                    // ACK slot, SCL high: on a read the master answers here.
                    if reading_r && !is_addr_r {
                        master_ack_r <= !sda_s
                    }
                    bit_r <= 9
                }
            }
            if scl_fall {
                if bit_r == 8 {
                    // Eight bits in: the ACK slot begins on this low phase.
                    got_byte_r <= shift_r
                    byte_cnt_r <= byte_cnt_r + 1
                    if is_addr_r {
                        addr_hit_r <= addr_match
                        reading_r  <= addr_match && shift_r[0]
                        if ack_en && addr_match { sda.drive_low() } else { sda.release() }
                    } else if reading_r {
                        sda.release()                   // the master ACKs/NACKs
                    } else {
                        if ack_en && addr_hit_r { sda.drive_low() } else { sda.release() }
                    }
                } else if bit_r == 9 {
                    // ACK slot over.
                    bit_r <= 0
                    if is_addr_r && stretch_en && addr_hit_r {
                        scl.drive_low()
                        stretch_r <= STRETCH_CYCLES
                    }
                    if is_addr_r && reading_r {
                        // First data bit of the read.
                        if tx_data[7] { sda.release() } else { sda.drive_low() }
                        tx_shift_r <= tx_data << 1
                    } else {
                        sda.release()
                    }
                    is_addr_r <= false
                } else if reading_r && !is_addr_r {
                    // Next data bit of the read.
                    if tx_shift_r[7] { sda.release() } else { sda.drive_low() }
                    tx_shift_r <= tx_shift_r << 1
                }
            }
        }
    }

    got_byte   = got_byte_r
    byte_cnt   = byte_cnt_r
    start_cnt  = start_cnt_r
    stop_cnt   = stop_cnt_r
    addr_hit   = addr_hit_r
    master_ack = master_ack_r
    scl_held   = scl.driving
}

// Master + slave on one open-drain bus: two wires, each bound to an
// `opendrain` pad of both devices (SV: `tri1`). Everything the tests
// need to see is re-exported as an `out` port.
module I2cTb {
    in  clk          : clock
    // Master command side.
    in  start        : bool
    in  addr         : u7
    in  rw           : bool
    in  wr_data      : u8
    in  fast_mode    : bool
    // Slave configuration.
    in  slave_addr   : u7
    in  slave_tx     : u8
    in  ack_en       : bool
    in  stretch_en   : bool
    // Master observations.
    out busy         : bool
    out done         : bool
    out ack_ok       : bool
    out rd_data      : u8
    out m_state      : I2cState
    out stretched    : bool
    out repeated     : bool
    // Resolved bus levels.
    out sda          : bool
    out scl          : bool
    // Slave observations.
    out s_got_byte   : u8
    out s_byte_cnt   : u4
    out s_start_cnt  : u4
    out s_stop_cnt   : u4
    out s_addr_hit   : bool
    out s_master_ack : bool
    out s_scl_held   : bool

    wire sda_bus : bool
    wire scl_bus : bool

    let m_i = I2cMaster {
        clk, start, addr, rw, wr_data, fast_mode,
        sda: sda_bus,
        scl: scl_bus,
    }
    let s_i = I2cSlaveModel {
        clk,
        scl: scl_bus,
        sda: sda_bus,
        my_addr: slave_addr,
        tx_data: slave_tx,
        ack_en, stretch_en,
    }

    busy         = m_i.busy
    done         = m_i.done
    ack_ok       = m_i.ack_ok
    rd_data      = m_i.rd_data
    m_state      = m_i.state
    stretched    = m_i.stretched
    repeated     = m_i.repeated
    sda          = sda_bus
    scl          = scl_bus
    s_got_byte   = s_i.got_byte
    s_byte_cnt   = s_i.byte_cnt
    s_start_cnt  = s_i.start_cnt
    s_stop_cnt   = s_i.stop_cnt
    s_addr_hit   = s_i.addr_hit
    s_master_ack = s_i.master_ack
    s_scl_held   = s_i.scl_held
}

test "idle bus is released" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = true;
    step(5);
    assert_true(dut.sda);
    assert_true(dut.scl);
    assert_false(dut.busy);
    assert_eq(dut.m_state, I2cState::Idle);
}

test "start condition" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = true;
    dut.addr = 0x50;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    assert_true(dut.busy);
    assert_eq(dut.m_state, I2cState::Start);
    // Phase 1: SCL released, SDA still high.
    step(15);
    assert_true(dut.sda);
    assert_true(dut.scl);
    // Phase 2: SDA falls while SCL is high -- the START condition (E+16).
    step(1);
    assert_false(dut.sda);
    assert_true(dut.scl);
    // The slave saw it three clocks later (E+19).
    step(3);
    assert_eq(dut.s_start_cnt, 1);
    // Phase 3: SCL pulled low (E+24), then the address bits begin (E+32).
    step(5);
    assert_false(dut.scl);
    step(8);
    assert_eq(dut.m_state, I2cState::Addr);
}

test "address transmission" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = true;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    // Bit 0 (address MSB = 1) on the line while SCL is high: E+48.
    step(48);
    assert_true(dut.sda);
    assert_true(dut.scl);
    // Bit 7 (R/W = 0) while SCL is high: E+48+32*7 = E+272.
    step(224);
    assert_false(dut.sda);
    assert_true(dut.scl);
    // After the eighth falling edge (E+280, seen at E+283) the slave has
    // the whole byte: 0x50 << 1 | 0 = 0xA0; the master is in ADDR_ACK
    // from E+288.
    step(16);
    assert_eq(dut.s_got_byte, 0xA0);
    assert_true(dut.s_addr_hit);
    assert_eq(dut.s_byte_cnt, 1);
    assert_eq(dut.m_state, I2cState::AddrAck);
}

test "ack handling" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = true;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    // ACK slot with SCL high (E+296 .. E+312): the slave holds SDA low.
    step(300);
    assert_false(dut.sda);
    assert_true(dut.scl);
    assert_eq(dut.m_state, I2cState::AddrAck);
    // The master sampled the ACK at E+312 and moved on to the data byte
    // at E+320.
    step(20);
    assert_eq(dut.m_state, I2cState::Data);
    step(320);
    assert_true(dut.done);
    assert_true(dut.ack_ok);
}

test "nack handling" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = false;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    // ACK slot: nobody pulls SDA low.
    step(300);
    assert_true(dut.sda);
    assert_true(dut.scl);
    // NACK: straight to STOP at E+320, idle at E+352 with ack_ok low.
    step(20);
    assert_eq(dut.m_state, I2cState::Stop);
    step(32);
    assert_false(dut.busy);
    assert_true(dut.done);
    assert_false(dut.ack_ok);
    assert_eq(dut.s_byte_cnt, 1);
    assert_eq(dut.s_stop_cnt, 1);
}

test "address mismatch is a nack" {
    let dut = I2cTb { };
    dut.slave_addr = 0x51;
    dut.ack_en = true;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    step(352);
    assert_false(dut.busy);
    assert_false(dut.ack_ok);
    assert_false(dut.s_addr_hit);
    assert_eq(dut.s_byte_cnt, 1);
}

test "write byte" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = true;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    step(640);
    assert_true(dut.done);
    assert_false(dut.busy);
    assert_true(dut.ack_ok);
    assert_eq(dut.s_got_byte, 0x3C);
    assert_eq(dut.s_byte_cnt, 2);
    assert_eq(dut.s_start_cnt, 1);
    assert_eq(dut.s_stop_cnt, 1);
    assert_true(dut.sda);
    assert_true(dut.scl);
    step(1);
    assert_false(dut.done);
}

test "stop condition" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = true;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    // STOP phase 0 (E+608 .. E+616): both lines low.
    step(612);
    assert_eq(dut.m_state, I2cState::Stop);
    assert_false(dut.sda);
    assert_false(dut.scl);
    // Phase 1 (from E+616): SCL released and high, SDA still low.
    step(6);
    assert_false(dut.sda);
    assert_true(dut.scl);
    // Phase 2 (from E+624): SDA rises while SCL is high -- the STOP condition.
    step(6);
    assert_true(dut.sda);
    assert_true(dut.scl);
    // The slave counted it three clocks later; the master idles at E+640.
    step(3);
    assert_eq(dut.s_stop_cnt, 1);
    step(13);
    assert_false(dut.busy);
    assert_eq(dut.m_state, I2cState::Idle);
}

test "read byte" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.slave_tx = 0xC3;
    dut.ack_en = true;
    dut.addr = 0x50;
    dut.rw = true;
    dut.start = true;
    step(1);
    dut.start = false;
    step(640);
    assert_true(dut.done);
    assert_true(dut.ack_ok);
    assert_eq(dut.rd_data, 0xC3);
    assert_eq(dut.s_got_byte, 0xC3);
    // A single-byte read ends with the master's NACK.
    assert_false(dut.s_master_ack);
    assert_eq(dut.s_stop_cnt, 1);
}

test "clock stretching" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = true;
    dut.stretch_en = true;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    // The slave grabs SCL when it sees the ACK slot end (E+315) and the
    // master, waiting in DATA phase 1 from E+336, has to stall.
    step(340);
    assert_true(dut.s_scl_held);
    assert_false(dut.scl);
    assert_eq(dut.m_state, I2cState::Data);
    assert_true(dut.stretched);
    // Without stretching the write would be over at E+640.
    step(300);
    assert_true(dut.busy);
    // The slave releases SCL at E+355; the master sees it at E+358 and
    // finishes 22 clocks late (E+662) with the data intact.
    step(22);
    assert_true(dut.done);
    assert_false(dut.busy);
    assert_true(dut.ack_ok);
    assert_eq(dut.s_got_byte, 0x3C);
    assert_eq(dut.s_byte_cnt, 2);
}

test "fast mode write" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.ack_en = true;
    dut.fast_mode = true;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0xA5;
    dut.start = true;
    step(1);
    dut.start = false;
    // START 8 clocks, then 19 bit periods of 9 clocks (phase 1 waits
    // one clock for the synchronised SCL): 8 + 19*9 = E+179.
    step(178);
    assert_true(dut.busy);
    step(1);
    assert_true(dut.done);
    assert_true(dut.ack_ok);
    assert_eq(dut.s_got_byte, 0xA5);
    assert_eq(dut.s_byte_cnt, 2);
    assert_eq(dut.s_stop_cnt, 1);
}

test "repeated start write then read" {
    let dut = I2cTb { };
    dut.slave_addr = 0x50;
    dut.slave_tx = 0x5A;
    dut.ack_en = true;
    dut.addr = 0x50;
    dut.rw = false;
    dut.wr_data = 0x3C;
    dut.start = true;
    step(1);
    dut.start = false;
    // Queue the next command while the write is in flight: the master
    // samples addr/rw at the repeated START (E+608).
    step(50);
    dut.rw = true;
    dut.start = true;
    step(1);
    dut.start = false;
    // First byte delivered, then a START instead of a STOP.
    step(557);
    assert_eq(dut.s_got_byte, 0x3C);
    assert_eq(dut.m_state, I2cState::Start);
    assert_true(dut.repeated);
    assert_true(dut.busy);
    // Second START on the wire at E+624, seen by the slave at E+627; no
    // STOP yet.
    step(19);
    assert_eq(dut.s_start_cnt, 2);
    assert_eq(dut.s_stop_cnt, 0);
    // Second transaction: ADDR 256 + ACK 32 + DATA 256 + ACK 32 + STOP 32
    // after E+640 -> idle at E+1248.
    step(621);
    assert_true(dut.done);
    assert_true(dut.ack_ok);
    assert_eq(dut.rd_data, 0x5A);
    assert_eq(dut.s_start_cnt, 2);
    assert_eq(dut.s_stop_cnt, 1);
}
