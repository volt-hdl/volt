//~ E2013
// ADR-0077 Karar 1: a plain struct field has no clock domain — the whole
// value lives in the domain of the signal that holds it. Per-field
// domains belong to a 'struct port'.
domain Fast {
    clock = posedge
    reset = sync active_high
}

struct Sample {
    value : u8 @Fast
//~^ ERROR clock-domain annotation
    valid : bool
}

module M {
    in  a : u8
    out y : u8

    y = a
}
