//~ E3016
// Contracts are sampled on the module's first clock port. Here it is in an
// edgeless domain; the assertions used to become `@(posedge clk)`.

domain Async { clock = none }

module EdgelessContracts {
    in  clk : clock @Async
    in  a   : u8
    out y   : u8

    y = a
    invariant: y == a
    //~^ ERROR contracts of 'EdgelessContracts' are checked on clock 'clk' of domain 'Async', which has no clock edge
}
