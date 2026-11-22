//~ E4011
// #378: a built-in input is bound in the instance literal; a write to it
// would be a second driver of the bound value.

module M {
    in  clk : clock
    in  en  : bool
    in  go  : bool
    out q   : bits<8>
    let cnt = Counter<8> { clk: clk, enable: en, clear: false }
    cnt.enable = go
    //~^ ERROR instance input written
    q = cnt.count
}
