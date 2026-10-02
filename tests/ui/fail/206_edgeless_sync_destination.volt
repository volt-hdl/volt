//~ E3016
// sync() into a domain declared with `clock = none`. The synchronizer stages
// used to become `always_ff @(posedge aclk)`: flip-flops timed by an edge the
// domain says does not exist.

domain Async { clock = none }
domain Fast { clock = posedge }

module EdgelessSyncDestination {
    in  aclk : clock @Async
    in  fclk : clock @Fast
    in  d    : bool @Fast
    out q    : bool @Async

    q = sync(d, aclk)
    //~^ ERROR sync() into clock domain 'Async', which has no clock edge
}
