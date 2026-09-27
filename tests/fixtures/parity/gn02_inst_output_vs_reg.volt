// parity: E1003
module Timer {
    in  clk : clock
    out irq : bool
    reg c : u4 = 0
    on clk { c <= c + 1 }
    irq = c == 0
}
module M {
    in  clk : clock
    out o   : bool
    reg timer_irq : bool = false
    let timer = Timer { clk }
    on clk { timer_irq <= timer.irq }
    o = timer_irq
}
