// ADR-0087: built-in primitives store a struct (or enum) T as ONE packed
// word (ADR-0077 bit order, first field in the high bits) — the memory
// is `logic [W-1:0] mem [DEPTH]`, so it still maps to block RAM. Input
// data ports take a whole struct, output data ports give one back.
// A struct with an enum field is fine in the FIFO family: the output
// register only ever shows written values or T's default (reset) value.

// The external reset comes in raw (ADR-0065) and is released
// synchronously to each clock.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

enum Mode { Off, Slow, Fast }

struct Packet {
    tag  : u4
    data : u8
}

struct Cmd {
    mode : Mode
    arg  : u6
}

module PrimStructs {
    in  clk      : clock @Fast
    in  clk2     : clock @Slow
    in  rst      : reset(sync, active_high)
    in  pkt      : Packet @Fast
    in  cmd      : Cmd @Fast
    in  push     : bool @Fast
    in  pop      : bool @Fast
    in  pop2     : bool @Slow
    in  addr     : bits<4> @Fast
    out fifo_out : Packet @Fast
    out xfer_out : Packet @Slow
    out ram_out  : Packet @Fast
    out cmd_out  : Cmd @Fast
    out hs_out   : Cmd @Slow
    out hs_ready : bool @Slow
    out full     : bool @Fast
    out empty    : bool @Fast
    out wr_full  : bool @Fast
    out rd_empty : bool @Slow

    let fifo = SyncFifo<Packet, 8> { clk: clk, wr_data: pkt, wr_en: push, rd_en: pop }
    let xfer = AsyncFifo<Packet, 8> {
        wr_clk: clk, wr_data: pkt, wr_en: push,
        rd_clk: clk2, rd_en: pop2,
    }
    let ram  = Ram<Packet, 16> { clk: clk, addr: addr, wr_data: pkt, wr_en: push }
    let line = ShiftRegister<Cmd, 3> { clk: clk, data_in: cmd, shift_en: push }
    let hs   = HandshakeSync<Cmd> { src_clk: clk, data_in: cmd, send: push, dst_clk: clk2 }

    fifo_out = fifo.rd_data
    xfer_out = xfer.rd_data
    ram_out  = ram.rd_data
    cmd_out  = line.data_out
    hs_out   = hs.data_out
    hs_ready = hs.valid
    full     = fifo.full
    empty    = fifo.empty
    wr_full  = xfer.wr_full
    rd_empty = xfer.rd_empty
}
