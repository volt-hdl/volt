//~ E0003
// ADR-0081 Karar 4: struct port bir değer değildir (ADR-0077).
struct port Link {
    out d : u8
    in  r : bool
}

fn f(l: Link) -> u8 {
//~^ ERROR E0003 not supported yet: a 'struct port' bundle in a function signature
    0
}
