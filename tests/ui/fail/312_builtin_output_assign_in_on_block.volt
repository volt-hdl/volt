//~ E4011
// #378: written in an `on` block, the built-in output reached the SV
// generator, whose audit (E0020) called it a compiler bug.

module M {
    in  clk : clock
    in  en  : bool
    out w   : bool
    let cnt = Counter<8> { clk: clk, enable: en, clear: false }
    on clk {
        cnt.overflow <= true
        //~^ ERROR instance output written
    }
    w = cnt.overflow
}
