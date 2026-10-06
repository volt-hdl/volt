//~ E1016
// An extern module named like the standard library's EdgeDetect, with the
// same ports. The built-in edge detector used to be inlined in its place
// and the generated SystemVerilog read undeclared wires (e_rise, e_fall):
// `volt build` exited 0 and the extern's SystemVerilog was never used.

extern module EdgeDetect {
//~^ ERROR extern module 'EdgeDetect' has the name of a standard library module
    in  clk    : clock
    in  signal : bool
    out rise   : bool
    out fall   : bool
}

module StdlibNameExternEdgeDetect {
    in  clk : clock
    in  b   : bool
    out r   : bool
    out f   : bool

    let e = EdgeDetect { clk: clk, signal: b }
    r = e.rise
    f = e.fall
}
