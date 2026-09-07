//~ E2004
// Arithmetic cannot be performed on bits<N>.
// bits is a raw bit vector, not a number.

module BitsArithmetic {
    in  a : bits<8>
    in  b : bits<8>
    out r : bits<8>

    r = a + b
    //~^ ERROR cannot perform arithmetic on bits<N>
}
