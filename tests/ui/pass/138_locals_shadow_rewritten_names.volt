// #137, #138: the parser rewrites bidirectional port members (ADR-0051)
// and pipeline stage values (ADR-0038), and infers counter contracts
// (ADR-0066), before name resolution. A name is still resolved in the
// scope chain of name-resolution.md §2: a block let or a loop variable
// shadows the port, the stage value or the root const (W1002 from the
// resolver, §6), and its uses read the local. Shadowing a bundle port or
// the @mmio handle warns in the parser; those cases are in
// crates/volt-driver/tests/parser_rewrite_scope_*_tests.rs.

struct St {
    released : bool
    driving  : bool
}

const K : u8 = 5

module InoutShadow {
    in  clk : clock
    in  s   : bool
    in  w   : bits<8>
    inout dq : bits<8>
    out y   : bool
    reg r : bool = false
    on clk {
        dq.drive(w)
        let dq : St = St { released: s, driving: false }
        r <= dq.released
    }
    y = r
}

pipeline(2) StageValueShadow {
    in  clk : clock
    in  x   : u32
    out y0  : u32
    out y1  : u32
    reg r : [u32; 2] = [0; 2]
    stage A {
        let a : u32 = x + 1
    }
    stage B {
        for a in 0..2 {
            r[a] <= x
        }
    }
    y0 = r[0]
    y1 = r[1]
}

// No counter contract: the bound K is the block let (the input lim), not
// the root const.
module CounterBoundShadow {
    in  clk : clock
    in  lim : u8
    out q   : u8
    reg r : u8 = 0
    on clk {
        let K : u8 = lim
        if r == K {
            r <= 0
        } else {
            r <= r + 1
        }
    }
    q = r
}
