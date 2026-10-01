// ADR-0097, two instances that both meet the obligation (exit 0). The
// second one is driven from a register that wraps at 9, so the parent
// proves the precondition from its own logic, not from a constant.

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

    reg n : u8 = 0
    on clk {
        if n == 9 {
            n <= 0
        } else {
            n <= n + 1
        }
    }

    let a = Child { clk: clk, x: 3 }
    let b = Child { clk: clk, x: n }
    y = a.y
    z = b.y
}
