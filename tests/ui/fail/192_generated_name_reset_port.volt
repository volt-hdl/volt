//~ E1003
// ADR-0090: the default domain has a synchronous active-high reset, so Volt
// adds the port 'rst'; a register of the same name was a duplicate
// declaration in the generated SystemVerilog.

module Latch {
    in  clk : clock
//~^ ERROR 'rst' is both register 'rst' and the reset port Volt adds for clock 'clk'
    in  x   : bool
    out o   : bool

    reg rst : bool = false
    on clk { rst <= x }
    o = rst
}
