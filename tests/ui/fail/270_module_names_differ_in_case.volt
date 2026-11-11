//~ E1003
// #250: each module is written to a file named after it (rtl/Foo.sv,
// rtl/foo.sv, constraints/Foo.sdc), and the SVA file and checker of a
// module use its name in lower case (formal/foo.sva, foo_sva). Two module
// names that differ only in letter case write one SVA file on every
// operating system, and one RTL and constraints file on Windows and macOS:
// one module's SystemVerilog or contracts were silently lost. Volt does
// not rename module names (ADR-0090): E1003, and one module is renamed.
module Foo {
    in  clk : clock
    in  x   : u8
    out y   : bool
    reg r : bool = false
    on clk { r <= x == 1 }
    invariant: !r || true
    y = r
}

module foo {
//~^ ERROR module names 'Foo' and 'foo' differ only in letter case
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
