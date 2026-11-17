// #303: a chain of `as` casts that keep the width and the sign, around a
// composite expression, used in a wider context. Each cast reinterprets
// the same bits (type-inference.md §3.6); the wider target or cast
// zero-extends (sign-extends a signed value) the value computed at its
// own width (ADR-0025, ADR-0041). The output net lints the generated SV
// with Verilator -Wall: before the fix the composite was evaluated at the
// target width and Verilator reported WIDTHEXPAND, which stops volt test.

module SameWidthCastChain {
    in  a  : u8
    in  b  : u8
    in  c  : u8
    in  w  : u14
    in  s  : i8
    in  t  : i8
    in  ws : i14
    in  p  : u43
    out y1 : u14
    out y2 : u14
    out y3 : u14
    out y4 : u14
    out y5 : u14
    out y6 : u14
    out z1 : i11
    out z2 : i17
    out z3 : i14
    out z4 : bool
    out z5 : i14
    out q  : u49

    // Unsigned: two or three same-width casts into a wider target, a typed
    // let, wider arithmetic, and the chain as an operand of a bitwise op.
    y1 = (a | (b & c)) as bits<8> as u8
    y2 = (a | (b & c)) as u8 as u8
    y3 = (a | (b & c)) as bits<8> as u8 as u8
    let x : u14 = (a | (b & c)) as u8 as u8
    y4 = x
    y5 = (a | (b & c)) as bits<8> as u8 + w
    y6 = ((a | (b & c)) as bits<8> as u8 & c) as bits<8> as u8

    // Signed: a same-width cast, then a widening one; wider arithmetic and
    // comparison.
    z1 = (s ^ (0x35 & t)) as i8 as i11
    z2 = (s ^ (0x35 & t)) as i8 as i11 as i11
    z3 = (s ^ (0x35 & t)) as i8 as i8 + ws
    z4 = (s ^ (0x35 & t)) as i8 as i14 > ws
    z5 = (s ^ (0x35 & t)) as i8 as i8 as i8

    // The example in the title of #303.
    q = ((p | 0x3612E6A3554) & 0x4CBBBD40467) as bits<43> as u43
}
