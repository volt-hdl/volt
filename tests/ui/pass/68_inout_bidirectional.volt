// Bidirectional data port (ADR-0051): `inout dq : bits<8>` is a pad the
// module either drives or leaves in high impedance. The drive state
// lives in two compiler-synthesised registers, dq_oe and dq_out; the
// module changes them only through dq.drive(value) / dq.release()
// inside an 'on' block, reads the pad with dq.read() and names the
// state in contracts as dq.released / dq.driving. The generated SV is
// `assign dq = dq_oe ? dq_out : {8{1'bz}}`; a direct assignment to dq
// would be a push-pull driver fighting the bus and is E4008.
//
// The SRAM shares this clock, so the port samples dq directly and
// accepts W3007 (an asynchronous peer would go through sync(), as the
// 1-bit Listener below does); ui/pass allows warnings.
module SramPort {
    in  clk     : clock
    in  write   : bool
    in  wr_data : bits<8>
    inout dq    : bits<8>
    out rd_data : bits<8>
    out driving : bool

    // Driver intent: the pad is released whenever the port is reading.
    invariant: !write_r -> dq.released
    invariant: dq.driving -> write_r
    cover: dq.driving

    reg write_r   : bool    = false
    reg rd_data_r : bits<8> = 0 as bits<8>

    on clk {
        write_r <= write
        if write {
            dq.drive(wr_data)
        } else {
            dq.release()
            rd_data_r <= dq.read()
        }
    }

    rd_data = rd_data_r
    driving = dq.driving
}

// A single-bit inout with a plain bool payload and a listener that only
// reads the line (no drive call, so no tri-state buffer is generated).
module Listener {
    in  clk   : clock
    inout pin : bool
    out seen  : bool

    wire pin_s : bool
    pin_s = sync(pin.read(), clk)

    reg seen_r : bool = false
    on clk { seen_r <= pin_s }
    seen = seen_r
}

module Board {
    in  clk     : clock
    in  write   : bool
    in  wr_data : bits<8>
    out rd_data : bits<8>
    out seen    : bool

    wire dq_bus : bits<8>
    wire pin_bus : bool

    let s = SramPort { clk, write, wr_data, dq: dq_bus }
    let l = Listener { clk, pin: pin_bus }

    rd_data = s.rd_data
    seen    = l.seen
}

// Output net (ADR-0079):
//~ LINT-ALLOW: UNUSEDSIGNAL: 'write_r' is a ghost register read only by the driver-intent contracts
