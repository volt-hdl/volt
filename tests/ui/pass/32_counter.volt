// Built-in Counter primitive (ADR-0029) -- WIDTH-bit wrap-around
// counter with enable/clear (clear wins) and a one-cycle overflow
// pulse on wrap. The `count < 2^WIDTH` invariant is provable.
module TickCounter {
    in  clk   : clock
    in  en    : bool
    in  clr   : bool
    out ticks : bits<8>
    out wrap  : bool

    let cnt = Counter<8> {
        clk: clk,
        enable: en,
        clear: clr,
    }

    ticks = cnt.count
    wrap  = cnt.overflow
}
