//~ E3003
// The raw reset port says sync but the domain it feeds resets
// asynchronously (ADR-0065 sec. 1): the synchronizer would assert with
// one kind and release registers of another.
domain Core {
    clock = posedge
    reset = async active_low
}

module Top {
    in  clk   : clock @Core
    in  rst_n : reset(sync, active_low)
    //~^ ERROR E3003
    in  d     : u8
    out q     : u8

    reg r : u8 = 0
    on clk {
        r <= d
    }
    q = r
}
