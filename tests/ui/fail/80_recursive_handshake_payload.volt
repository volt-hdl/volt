//~ E4009
// A Handshake payload is flattened field by field (ADR-0050). ADR-0067
// caught a payload struct that contains itself directly; one that contains
// itself through an array ('[P; 2]') was still flattened silently. The
// struct is reported once, the port keeps an opaque 'data' field.
struct P {
//~^ ERROR E4009
    d : u8
    f : [P; 2]
}

module M {
    in  clk  : clock
    in  rx   : Handshake<P>
    out seen : bool

    rx.ready = true
    seen = rx.valid
}
