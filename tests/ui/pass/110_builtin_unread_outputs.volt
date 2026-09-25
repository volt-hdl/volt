// Built-in primitive outputs that the module never reads (ADR-0079): like
// an unread output of a user module instance (ADR-0072), leaving one
// unread is legal. The generated SV declares every output wire and wraps
// the unread ones in a Verilator UNUSEDSIGNAL waiver, so the output net
// (-Wall, zero warnings) stays clean. HandshakeSync.busy in
// 87_sdc_targeted_all_bridges takes the same path across two clocks.
module UnreadOutputs {
    in  clk   : clock
    in  din   : u8
    in  push  : bool
    in  pop   : bool
    in  addr  : bits<6>
    out o1    : bool
    out o2    : bits<8>
    out o3    : bits<8>
    out o4    : bool
    out o5    : u8
    out o6    : bits<4>

    // rd_data unread (full is used inside the FIFO itself).
    let fifo = SyncFifo<u8, 16> { clk: clk, wr_data: din, wr_en: push, rd_en: pop }
    o1 = fifo.empty

    // overflow unread.
    let cnt = Counter<8> { clk: clk, enable: push, clear: pop }
    o2 = cnt.count

    // data_out unread.
    let sr = ShiftRegister<bool, 8> { clk: clk, data_in: push, shift_en: pop }
    o3 = sr.taps

    // falling and both unread.
    let ed = EdgeDetect { clk: clk, signal: push }
    o4 = ed.rising

    // b_rd_data unread.
    let shared = DualPortRam<u8, 64> {
        clk: clk,
        a_addr: addr, a_wr_data: din, a_wr_en: push,
        b_addr: addr, b_wr_data: din, b_wr_en: pop,
    }
    o5 = shared.a_rd_data

    let rr = RoundRobinArbiter<4> { clk: clk, req: 0b1010 as bits<4> }
    o6 = rr.grant
}
