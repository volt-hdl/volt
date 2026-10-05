//~ E1016
// An extern module named like the standard library's Counter. The
// built-in was taken instead, and the error asked for the built-in's
// generic argument (E0003 "'Counter' without its generic arguments").

extern module Counter {
//~^ ERROR extern module 'Counter' has the name of a standard library module
    in  clk : clock
    out n   : u4
}

module StdlibNameExternCounter {
    in  clk : clock
    out n   : u4

    let c = Counter { clk: clk }
    n = c.n
}
