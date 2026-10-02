//~ E3016
// The sync() source is a port of an edgeless domain that also has a clock
// port: the bridge first captures the source on that clock, which used to
// become `always_ff @(posedge aclk)`.

domain Async { clock = none }
domain Fast { clock = posedge }

module EdgelessSyncSource {
    in  aclk : clock @Async
    in  fclk : clock @Fast
    in  d    : bool @Async
    out q    : bool @Fast

    q = sync(d, fclk)
    //~^ ERROR sync() source 'd' is captured on clock 'aclk' of domain 'Async', which has no clock edge
}
