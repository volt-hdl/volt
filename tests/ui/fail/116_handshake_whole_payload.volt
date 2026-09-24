//~ E0003
// ADR-0077 Karar 1: the Handshake payload is flattened field by field;
// using the whole payload as a value is not supported yet (it was the
// misleading "undefined name: 'req'" before).
struct Beat {
    data : u8
    last : bool
}

module M {
    in  clk : clock
    in  req : Handshake<Beat>
    out got : bool

    reg b : Beat = Beat { data: 0, last: false }
    on clk { if req.valid { b <= req.data } }
//~^ ERROR the whole Handshake payload 'req.data'
    req.ready = true
    got = b.last
}
