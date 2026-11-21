//~ E2003
// ADR-0065 R3 (#356): two reset ports cannot be compared either. The
// reset checks may object to this design for other reasons; the
// comparison itself is E2003.

module ResetNeTwoPorts {
    in  clk  : clock
    in  rst  : reset(sync, active_high)
    in  rst2 : reset(sync, active_high)
    out y    : bool

    let t = rst != rst2
    //~^ ERROR a reset cannot be compared
    y = sync(t, clk)
}
