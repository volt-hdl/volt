//~ E3002
// ADR-0099: two clock ports annotated with each other. No domain
// declaration and no clock edge is reached; before, only the generator
// stopped it.

module Loop {
    in  c1 : clock @c2
    //~^ ERROR E3002
    in  c2 : clock @c1
    in  d  : bool  @c1
    out q  : bool  @c1

    reg r : bool @c1 = false
    on c1 {
        r <= d
    }
    q = r
}
