// #328: an assignment in a match arm after the first wildcard is not a
// driver. The arm is not part of the design (sv-mapping.md §19.1, W2014),
// so it does not conflict with the driver of `c` outside the block: no
// E4001 (type-inference.md §11.1), only the W2014 of the dead arm.

module DeadArmNotADriver {
    in  s : u2
    out a : u8
    out c : u8

    comb {
        match s {
            0 => { a = 1 }
            _ => { a = 7 }
            1 => { a = 9; c = 3 }
        }
    }
    c = 5
}
