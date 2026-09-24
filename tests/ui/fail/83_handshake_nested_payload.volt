//~ E0003
// ADR-0070 §3.1: a Handshake payload is data driven by the producer; a
// port bundle (another Handshake or a 'struct port') has its own
// directions and cannot be nested. Before, the inner Handshake stayed
// unflattened and was reported as a misleading "undefined name" (E1001).
module M {
    in  clk : clock
    in  h   : Handshake<Handshake<u8>>
//~^ ERROR E0003
    out y   : u8

    y = 0
}
