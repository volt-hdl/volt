//~ E2003
// ADR-0065 R3 (#356): a reset cannot be an operand of ==. 'rst == rst'
// is a constant made from a reset; it is rejected like any other read.

module ResetEqItself {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool

    let t = rst == rst
    //~^ ERROR a reset cannot be compared
    y = sync(t, clk)
}
