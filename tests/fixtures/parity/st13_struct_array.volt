// parity: E0003
struct P {
    a : u4
    b : bool
}
module M {
    in  clk : clock
    in  x   : u4
    out y   : bool
    reg ps : [P; 2] = [P { a: 0, b: false }; 2]
    on clk { ps[0] <= P { a: x, b: true } }
    y = ps[1].b
}
