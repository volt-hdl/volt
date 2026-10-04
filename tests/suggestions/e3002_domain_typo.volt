// suggestion: E3002
// A misspelt domain annotation: the closest domain is suggested.
domain Fast {
    clock = posedge
}

module M {
    in  clk : clock @Fasst
    in  a   : bool
    out y   : bool

    reg r : bool = false
    on clk {
        r <= a
    }
    y = r
}
