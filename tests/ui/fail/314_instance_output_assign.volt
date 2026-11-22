//~ E4011
// #378: the rule for a user module instance, now checked in one place
// (typeck) for every instance; before, only the SV generator rejected it.

module Child {
    in  d : u8
    out q : u8
    q = d
}

module M {
    in  d : u8
    out y : u8
    let u = Child { d: d }
    u.q = d
    //~^ ERROR instance output written
    y = u.q
}
