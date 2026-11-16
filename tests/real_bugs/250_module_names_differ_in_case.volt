//~ E1003
// #250: two module names that differ only in letter case shared their
// generated files. The SVA file and checker use the lower-case name
// (formal/foo.sva, foo_sva), so on every operating system the contracts
// of one module were silently overwritten by the other's; on Windows and
// macOS rtl/Foo.sv and rtl/foo.sv are one file, and one module's
// SystemVerilog was lost. `volt build` exited 0. Right answer: Volt does
// not rename generated names (ADR-0090), and two module names that differ
// only in case are E1003 at the later one (sv-mapping.md §9, #250). Fixed
// by #260.
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
