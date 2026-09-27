//~ E1003
// ADR-0090: the instance output wire '<instance>_<port>' is a Volt-made
// SystemVerilog name; 'timer' + 'irq' = 'timer_irq', which is also a port.
// Before, check was clean and Verilator reported a duplicate declaration.
module Timer {
    in  clk : clock
    out irq : bool
    reg c : u4 = 0
    on clk { c <= c + 1 }
    irq = c == 0
}

module Soc {
    in  clk       : clock
    out timer_irq : bool
    out o         : bool

    let timer = Timer { clk }
//~^ ERROR 'timer_irq' is both port 'timer_irq' and output 'irq' of instance 'timer'
    timer_irq = true
    o = timer.irq
}
