//~ E4011
// #378: an output of a built-in primitive instance is driven by the
// instance. Before the fix the write was accepted and the SV drove
// `cnt_overflow` from the counter's always_ff and from an assign (illegal
// SV; Verilator MULTIDRIVEN, Yosys a netlist with two drivers).

module M06 {
    in  clk : clock
    in  en  : bool
    out q   : bits<8>
    out w   : bool
    let cnt = Counter<8> { clk: clk, enable: en, clear: false }
    cnt.overflow = true
    //~^ ERROR instance output written
    q = cnt.count
    w = cnt.overflow
}
