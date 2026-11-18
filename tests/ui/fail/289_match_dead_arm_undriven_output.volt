//~ E4002
// #328: an output assigned only in a match arm after the first wildcard
// is not driven. The first matching arm wins and arms after the first
// wildcard are not part of the design (sv-mapping.md §19.1, W2014), so
// their assignments drive nothing; an output with no driver is E4002
// (type-inference.md §11.3).

module DeadArmOutput {
    in  s : u2
    out a : u8
    out c : u8
    //~^ ERROR output port 'c' is not driven

    comb {
        match s {
            0 => { a = 1 }
            _ => { a = 7 }
            1 => { a = 9; c = 3 }
        }
    }
}
