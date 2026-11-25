//~ E1003
// A domain field key is written at most once in a declaration (#420): a
// second 'reset' line gave the passes that read the domain two answers.

domain Sys {
    clock = posedge
    frequency = 50.mhz
    reset = none
    reset = sync active_high
    //~^ ERROR field 'reset' is written twice in domain 'Sys'
}

module DomainFieldTwice {
    in  clk : clock @Sys
    in  d   : u8
    out q   : u8

    reg r : u8 = 0
    on clk { r <= d }
    q = r
}
