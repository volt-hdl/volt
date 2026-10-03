// suggestion: W3002
// sync() within one clock domain only adds latency: the fix reads the
// signal directly.
module S {
    in  clk : clock
    in  a   : bool
    out y   : bool
    out q   : bool

    reg r : bool = false
    on clk {
        r <= a
    }
    q = r
    y = sync(a, clk)
}
