// ADR-0097, two instances of the same module: each instance carries its
// own copy of the obligation. `ok` meets it, `bad` does not; the
// counterexample names `bad` only.

module Child {
    in  clk : clock
    in  x   : u8
    out y   : u8

    requires: x < 10

    reg r : u8 = 0
    on clk {
        r <= x
    }
    y = r
}

module Parent {
    in  clk : clock
    in  rst : reset
    out y   : u8
    out z   : u8

    let ok  = Child { clk: clk, x: 3 }
    let bad = Child { clk: clk, x: 12 }
    y = ok.y
    z = bad.y
}
