// parity: E1003
module M {
    in  clk : clock
    in  x   : bool
    out o   : bool
    reg rst : bool = false
    on clk { rst <= x }
    o = rst
}
