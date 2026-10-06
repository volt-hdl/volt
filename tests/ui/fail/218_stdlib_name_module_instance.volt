//~ E1016
// A Volt module named like the standard library's EdgeDetect, placed in
// another module. The built-in was inlined in its place, as for an extern
// (215). A module with such a name that no module instantiates stays
// valid: the `volt new` template's Counter is a top module.

module EdgeDetect {
    in  clk    : clock
    in  signal : bool
    out rise   : bool

    reg last : bool = false
    on clk { last <= signal }
    rise = signal && !last
}

module StdlibNameModuleInstance {
    in  clk : clock
    in  b   : bool
    out r   : bool

    let e = EdgeDetect { clk: clk, signal: b }
    //~^ ERROR module 'EdgeDetect' has the name of a standard library module
    r = e.rise
}
