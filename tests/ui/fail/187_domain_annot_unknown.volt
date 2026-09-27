//~ E3002
// ADR-0088: an annotation that names no domain. Before it was read as an
// unknown attribute of the next line (W0020) and silently ignored.

domain Fast {}

module UnknownDomain {
    in  clk : clock @Fast
    in  a   : bool  @Fast
    out y   : bool  @Fast

    wire s : bool @Fsat
    //~^ ERROR undefined clock domain: 'Fsat'
    s = a
    y = s
}
