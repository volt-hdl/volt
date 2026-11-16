//~ W1002
//~ W1006
// #143: a generic module that nothing instantiates was still written as an
// SV module, with its parameter `N` resolved to the same-named root const:
// `Fir.sv` held `assign y = 32'd7;` (a value the template does not have,
// and a fatal Verilator WIDTHTRUNC). In the body `N` is the generic
// parameter, which shadows the root const (name-resolution.md §6 example:
// W1002); a template has no parameter value, so no SystemVerilog is
// generated for it and the compiler warns (ADR-0111: W1006). Fixed by
// #174. No test block: the correct output has no hardware for `Fir`.
const N : u32 = 7

module Fir<const N: u32> {
    out y : u8
    y = N
}
