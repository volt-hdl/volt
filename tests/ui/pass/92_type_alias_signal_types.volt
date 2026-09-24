// ADR-0070: non-generic type aliases resolve to their target before SV
// mapping — an alias of a supported type (primitive, clock, array, or
// another alias) works like the type itself in ports, reg, wire and let.
type Byte = u8
type Word = Byte
type Clk = clock
type Nibbles = [u4; 3]

module M {
    in  clk : Clk
    in  p   : Word
    in  q   : Nibbles
    out y   : Byte

    wire w : Byte
    reg r : Word = 0
    on clk { r <= p }
    w = r
    let z : Byte = w ^ (q[1] as u8)
    y = z
}
