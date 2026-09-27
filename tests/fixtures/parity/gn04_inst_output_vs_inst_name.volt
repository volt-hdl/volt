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
    let timer = Timer { clk }
    let timer_irq = Timer { clk }
    o = timer.irq ^ timer_irq.irq
}
