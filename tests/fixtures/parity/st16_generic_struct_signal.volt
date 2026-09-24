// parity: E0003
struct G<T> {
    v : T
}
module M {
    in  clk : clock
    out y   : bool
    reg g : G<u4> = G { v: 0 }
    on clk { g <= g }
    y = true
}
