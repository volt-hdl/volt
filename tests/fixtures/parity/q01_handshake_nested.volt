// parity: E0003
module M {
    in clk : clock
    in h : Handshake<Handshake<u8>>
    out y : u8
    y = 0
}
