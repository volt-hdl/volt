// #373: sync() of a struct value is one synchronizer per field in the SV
// (ADR-0077 Karar 5 kural 12: `sync_<src>_<field>_stage<i>`), whatever
// gives the source its struct type: a declared type, or, for an untyped
// `let` or `reg`, its value (a struct literal, another struct signal, an
// `if`, a child instance's struct output, a struct constant, the data
// output of a primitive that stores a struct, ADR-0087). The .sdc and
// .xdc constrain the path into each field's first stage and mark each
// stage ASYNC_REG (ADR-0065 sec. 4.2), so every get_cells pattern names a
// flop of the netlist: the timing job's sweep reads this design into
// OpenSTA, and the cell-name test (volt-driver sdc_emit_tests) checks
// the patterns against the registers of the generated SV.
domain Fast {
    clock = posedge,
    reset = none,
    frequency = 100.mhz,
}

domain Slow {
    clock = posedge,
    reset = none,
    frequency = 25.mhz,
}

struct P {
    a : bool,
    b : bool,
}

const K : P = P { a: true, b: false }

module Src {
    in  clk : clock @Fast
    in  d   : bool  @Fast
    out o   : P     @Fast

    reg r : P = P { a: false, b: false }
    on clk {
        r <= P { a: d, b: !d }
    }
    o = r
}

pub module SyncStructSources {
    in  fclk : clock @Fast
    in  sclk : clock @Slow
    in  d    : bool  @Fast
    in  pin  : P     @Fast
    out q1   : P     @Slow
    out q2   : P     @Slow
    out q3   : P     @Slow
    out q4   : P     @Slow
    out q5   : P     @Slow
    out q6   : P     @Slow
    out q7   : P     @Slow

    reg a_r : bool = false
    reg b_r : bool = false
    reg ru = P { a: false, b: false }
    on fclk {
        a_r <= d
        b_r <= !d
        ru <= P { a: d, b: a_r }
    }

    // The design of #373: an untyped let bound to a struct literal.
    let lit = P { a: a_r, b: b_r }
    q1 = sync(lit, sclk)
    // An untyped register whose initial value is a struct literal.
    q2 = sync(ru, sclk)
    // An untyped let bound to a struct port.
    let alias = pin
    q3 = sync(alias, sclk)
    // An untyped let bound to an if of two struct values.
    let sel = if d { pin } else { lit }
    q4 = sync(sel, sclk)
    // An untyped let bound to a child instance's struct output.
    let s0 = Src { clk: fclk, d: d }
    let inst = s0.o
    q5 = sync(inst, sclk)
    // An untyped let bound to a struct constant.
    let c = K
    q6 = sync(c, sclk)
    // An untyped let bound to the data output of a struct FIFO.
    let fifo = SyncFifo<P, 4> { clk: fclk, wr_data: lit, wr_en: d, rd_en: !d }
    let popped = fifo.rd_data
    q7 = sync(popped, sclk)
}
