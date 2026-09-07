// Module instantiation and port binding
module Adder {
    in  a : u8
    in  b : u8
    out s : u9

    s = a + b
}

module TwoAdders {
    in  clk : clock
    in  w : u8
    in  x : u8
    in  y : u8
    in  z : u8
    out out1 : u9
    out out2 : u9

    let add1 = Adder { a: w, b: x }
    let add2 = Adder { a: y, b: z }

    out1 = add1.s
    out2 = add2.s
}
