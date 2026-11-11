package lib;

// #250: the module 'Foo' is written to rtl/Foo.sv and formal/foo.sva.
pub module Foo {
    in  clk : clock
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == 1 }
    invariant: !r || true
    y = r
}
