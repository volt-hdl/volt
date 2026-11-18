//~ E4002
// #331: an output assigned only in a match arm that an earlier arm
// already covers is not driven. The first matching arm wins, so the
// second arm `0` is never taken; it is not part of the design
// (sv-mapping.md §19.1, W2014) and its assignments drive nothing; an
// output with no driver is E4002 (type-inference.md §11.3).

module CoveredArmOutput {
    in  s : u2
    out a : u8
    out c : u8
    //~^ ERROR output port 'c' is not driven

    comb {
        match s {
            0 => { a = 1 }
            0 => { a = 9; c = 3 }
            _ => { a = 7 }
        }
    }
}
