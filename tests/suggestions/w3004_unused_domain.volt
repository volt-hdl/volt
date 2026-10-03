// suggestion: W3004
// A domain that no signal uses.
domain Spare {
    clock = posedge
}

module U {
    in  a : u8
    out y : u8

    y = a
}
