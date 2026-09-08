// Built-in EdgeDetect primitive (ADR-0029) -- single-domain edge
// detector: rising/falling/both pulses derived from a one-cycle
// history register. It contains NO synchronizer; to move a pulse
// across clock domains use PulseSync instead.
module ButtonEdge {
    in  clk      : clock
    in  btn      : bool
    out pressed  : bool
    out released : bool
    out changed  : bool

    let ed = EdgeDetect {
        clk: clk,
        signal: btn,
    }

    pressed  = ed.rising
    released = ed.falling
    changed  = ed.both
}
