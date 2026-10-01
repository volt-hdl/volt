// ADR-0097, three levels: Top -> Mid -> Leaf. Mid passes its input
// straight to Leaf without a precondition of its own, so Leaf's
// `requires` fails in Mid's task (v is free); Top drives 12, so it fails
// in Top's task as well.

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

    let l = Leaf { clk: clk, x: v }
    y = l.y
}

module Top {
    in  clk : clock
    in  rst : reset
    out y   : u8

    let m = Mid { clk: clk, v: 12 }
    y = m.y
}
