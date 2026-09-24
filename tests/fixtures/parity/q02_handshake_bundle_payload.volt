// parity: E0003
struct port Bus {
    out a : u8
    in b : bool
}

module M {
    in clk : clock
    out h : Handshake<Bus>
    h.valid = false
}
