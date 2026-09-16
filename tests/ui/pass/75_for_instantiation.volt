// Module instantiation inside a module-level `for` (ADR-0056).
//
// The loop is unrolled at compile time: each iteration becomes a
// separate instance named `<name>_<i>` (`pe_0` .. `pe_3`), the loop
// variable is substituted as a constant into the bindings and the
// index expressions, and `let` wires declared in the body get the same
// suffix (`sum_0` .. `sum_3`). Array-typed ports and wires are packed
// vectors in the generated SV; `bus[i]` selects an element.

const LANES : u32 = 4

module Accum {
    in  clk  : clock
    in  d    : u8
    out q    : u16

    reg acc : u16 = 0
    on clk { acc <= acc + (d as u16) }
    q = acc
}

module Lanes {
    in  clk  : clock
    in  bus  : [u8; LANES]
    out sums : [u16; LANES]
    out total : u16

    wire plus1 : [u16; LANES]

    for i in 0..LANES {
        let pe = Accum { clk: clk, d: bus[i] }
        let sum : u16 = pe.q + 1
        sums[i]  = pe.q
        plus1[i] = sum
    }

    total = plus1[0] + plus1[LANES - 1]

    invariant: sums[0] == sums[0]
}
