//~ E1003
// #250 across files: 'Foo' lives in lib.volt and 'foo' in this file. The
// two names differ only in letter case: E1003, in either file order.
use lib::Foo;

module foo {
    in  clk : clock
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == 2 }
    invariant: !r || true
    y = r
}

module Top {
    in  clk : clock
    in  x   : u8
    out a   : bool
    out b   : bool
    let p = Foo { clk: clk, x: x }
    let q = foo { clk: clk, x: x }
    a = p.y
    b = q.y
}
