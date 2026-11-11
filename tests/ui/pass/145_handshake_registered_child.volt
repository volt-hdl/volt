// ADR-0114: a register inside a child cuts the combinational path, as an
// 'on' block does in the producer itself (ADR-0050 §4). So does a
// built-in whose outputs are registers (ShiftRegister).
module ReadyReg {
    in  clk : clock
    in  r   : bool
    out q   : bool
    reg q_r : bool = false
    on clk {
        q_r <= r
    }
    q = q_r
}

module Producer {
    in  clk  : clock
    in  have : bool
    out tx   : Handshake<u8>
    out late : bool

    reg count : u8 = 0
    on clk {
        if tx.fired {
            count <= count + 1
        }
    }
    let rr = ReadyReg { clk: clk, r: tx.ready }
    let sr = ShiftRegister<bool, 2> { clk: clk, data_in: tx.ready, shift_en: true }
    tx.valid = have && rr.q
    late     = sr.data_out
    tx.data  = count
}
