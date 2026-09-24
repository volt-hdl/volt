//~ E2013
// ADR-0077 Karar 1: a value cannot contain directed port fields — a
// plain struct field cannot be a 'struct port' bundle.
struct port Bus {
    out data  : u8
    in  ready : bool
}

struct Frame {
    bus : Bus
//~^ ERROR 'struct port' bundle type
    tag : u4
}

module M {
    in  a : u8
    out y : u8

    y = a
}
