// ADR-0097: the parent meets the instance's precondition (x = 3 < 10),
// so the obligation is proven in Parent's task (exit 0). Child's own
// `ensures` is checked in Child's task under its `requires` and again
// inside Parent's task.

module Child {
    in  clk : clock
    in  x   : u8
    out y   : u8

    requires: x < 10
    ensures:  y < 10

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

    invariant: y < 10

    let c = Child { clk: clk, x: 3 }
    y = c.y
}
