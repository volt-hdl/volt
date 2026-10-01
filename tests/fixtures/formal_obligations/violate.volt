// ADR-0097: an instance's `requires` is an obligation of the module that
// instantiates it. Parent drives Child.x = 12 although Child requires
// x < 10, so `volt verify` must fail in Parent's task (exit 6) and point
// at the instance. Before ADR-0097 the requires became an `assume` in
// Parent's task as well and the run passed.

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

    let c = Child { clk: clk, x: 12 }
    y = c.y
}
