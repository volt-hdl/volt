// #143: an instantiated generic module is generated as its monomorph
// (`Fir_4`, ADR-0041); the template itself is not, and no warning is
// given for it (W1006 is for a template that nothing instantiates).

module Fir<const N: u32> {
    in  x : uint<N>
    out y : uint<N>
    y = x
}

module Top {
    in  a : u4
    out b : u4
    let f = Fir<4> { x: a }
    b = f.y
}
