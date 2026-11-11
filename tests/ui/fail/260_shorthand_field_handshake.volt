//~ E4007
// #204: `Gate { ready }` means `Gate { ready: ready }`. valid reads the
// literal, the literal reads tx.ready: valid depends combinationally on
// ready (ADR-0050 §4).
struct Gate { ready : bool }

module Producer {
    in  clk  : clock
    in  have : bool
    out tx   : Handshake<u8>

    reg count : u8 = 0
    on clk {
        if tx.fired {
            count <= count + 1
        }
    }
    let ready = tx.ready
    let g = Gate { ready }
    tx.valid = have && g.ready
    //~^ ERROR handshake 'tx': valid depends combinationally on ready
    tx.data  = count
}
