//~ E0001
// ADR-0077 Karar 1: only 'struct port' fields carry a direction. (After a
// field with a domain annotation the direction used to be silent.)
domain Fast {
    clock = posedge
    reset = sync active_high
}

struct Sample {
    value : u8 @Fast
    in valid : bool
//~^ ERROR only 'struct port' fields carry a direction
}

module M {
    in  a : u8
    out y : u8

    y = a
}
