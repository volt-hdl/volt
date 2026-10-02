//~ E3016
// An edgeless clock drives the clock port of a module whose port is in a
// domain with an edge (here the implicit one, posedge): the child clocks its
// registers on an edge the parent's domain says does not exist.

domain Async { clock = none }

module Child {
    in  clk : clock
    in  d   : u8
    out q   : u8

    reg r : u8 = 0
    on clk { r <= d }
    q = r
}

module EdgelessChildClock {
    in  clk : clock @Async
    in  d   : u8
    out q   : u8

    let c = Child { clk: clk, d: d }
    //~^ ERROR clock of domain 'Async', which has no clock edge, drives clock port 'clk' of 'Child'
    q = c.q
}
