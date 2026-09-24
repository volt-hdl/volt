// parity: E4001
// drivers: 20 19
module Od {
    in  clk : clock
    in  p : bool
    opendrain sda : bool

    on clk {
        if p { sda.drive_low() } else { sda.release() }
    }
}

module M {
    in  clk : clock
    in  p : bool
    out l : bool

    wire bus : bool
    let d = Od { clk, p, sda: bus }
    bus = p
    l = bus
}
