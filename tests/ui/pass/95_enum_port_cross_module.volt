// ADR-0074: enum-typed ports between modules. Every module file declares
// the localparams it uses (ADR-0024: each .sv stands on its own); the
// port itself is a plain logic vector.
enum Cmd { Nop, Read, Write }

module Decoder {
    in  op  : u2
    out cmd : Cmd

    comb {
        match op {
            1 => { cmd = Cmd::Read }
            2 => { cmd = Cmd::Write }
            _ => { cmd = Cmd::Nop }
        }
    }
}

module Unit {
    in  clk : clock
    in  op  : u2
    out rd  : bool
    out wr  : bool
    out raw : u4

    let dec = Decoder { op: op }
    reg last : Cmd = Cmd::Nop

    on clk {
        last <= dec.cmd
    }

    rd  = last == Cmd::Read
    wr  = dec.cmd == Cmd::Write
    raw = last as u4
}
