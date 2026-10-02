//~ E2004
// Unary '-' on a bits<N> value passed the type checker without a word and
// reached SystemVerilog as `assign y = -b;`. bits is a raw bit vector, not
// a number; negation needs a signed number (ADR-0098).

module NegateBits {
    in  b : bits<8>
    out y : bits<8>

    y = -b
    //~^ ERROR cannot negate a bits<N> value
}
