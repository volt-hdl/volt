//~ E1003
// #250, fail/270 in the other order: Top comes first, then 'foo', then
// 'Foo'. The rule does not depend on the order: E1003 at the later name.
module Top {
    in  clk : clock
    in  x   : u8
    out a   : bool
    out b   : bool
    let q = foo { clk: clk, x: x }
    let p = Foo { clk: clk, x: x }
    a = p.y
    b = q.y
}

module foo {
    in  clk : clock
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == 2 }
    invariant: !r || true
    y = r
}

module Foo {
//~^ ERROR module names 'foo' and 'Foo' differ only in letter case
    in  clk : clock
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == 1 }
    invariant: !r || true
    y = r
}
