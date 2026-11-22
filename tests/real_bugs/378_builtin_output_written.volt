//~ E4011
// #378: an assignment to an output of a built-in primitive instance
// (`cnt.overflow = true`) was accepted, and the SV drove `cnt_overflow`
// from the counter's always_ff and from an assign: illegal SV (IEEE
// 1800-2017 §6.5; Verilator MULTIDRIVEN), and Yosys built a netlist with
// two drivers. An instance port is driven by its instance and its inputs
// are bound in the instance literal: `inst.port = ...` is E4011 for every
// instance (ADR-0072), one driver per net (type-inference.md §11.2).
// Fixed by the pull request that closes #378.
module M06 {
    in  clk : clock
    in  en  : bool
    out q   : bits<8>
    out w   : bool
    let cnt = Counter<8> { clk: clk, enable: en, clear: false }
    cnt.overflow = true
    q = cnt.count
    w = cnt.overflow
}
