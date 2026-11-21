//~ E2003
// ADR-0065 R3 (#356): a reset cannot be the scrutinee of a match. The
// copy is an ordinary signal: the synchronizer of sync() is cleared by
// the same reset, and no reset check sees it.

module ResetMatchThroughSync {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool

    let t = match rst { true => true, _ => false }
    //~^ ERROR a reset cannot be tested by match
    y = sync(t, clk)
}
