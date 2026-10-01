// ADR-0097, three levels done right: Mid states the precondition it needs
// (`requires: v < 10`), which discharges Leaf's obligation in Mid's task;
// Top drives 5 and discharges both Mid's and Leaf's obligations in its
// own task (exit 0).

module Leaf {
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

module Mid {
    in  clk : clock
    in  v   : u8
    out y   : u8

    requires: v < 10

    let l = Leaf { clk: clk, x: v }
    y = l.y
}

module Top {
    in  clk : clock
    in  rst : reset
    out y   : u8

    let m = Mid { clk: clk, v: 5 }
    y = m.y
}
