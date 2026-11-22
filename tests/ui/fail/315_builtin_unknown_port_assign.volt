//~ E1009
// #378: a write to a port a built-in primitive does not have is E1009
// (name resolution), not E4011: the port must exist before its direction
// decides the rule.

module M {
    in  clk : clock
    in  en  : bool
    out w   : bool
    let cnt = Counter<8> { clk: clk, enable: en, clear: false }
    cnt.overflw = true
    //~^ ERROR unknown port
    w = cnt.overflow
}
