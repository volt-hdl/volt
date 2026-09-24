// parity: E0003
module M {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    let r : reset(sync, active_high) = rst
    y = false
}
