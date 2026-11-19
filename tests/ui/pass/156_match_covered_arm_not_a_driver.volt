// #331: an assignment in a match arm that an earlier arm already covers
// is not a driver. The arm is never taken and not part of the design
// (sv-mapping.md §19.1, W2014), so it does not conflict with the driver
// of `c` outside the block: no E4001 (type-inference.md §11.1), only the
// W2014 of each covered arm. The number and the enum form are the same.

enum Mode { Idle, Run, Stop }

module CoveredArmNotADriver {
    in  s : u2
    in  m : Mode
    out a : u8
    out b : u8
    out c : u8
    out d : u8

    comb {
        match s {
            0 => { a = 1 }
            0 => { a = 9; c = 3 }
            _ => { a = 7 }
        }
        match m {
            Mode::Idle => { b = 1 }
            Mode::Run => { b = 2 }
            Mode::Idle => { b = 9; d = 3 }
            Mode::Stop => { b = 4 }
        }
    }
    c = 5
    d = 6
}
