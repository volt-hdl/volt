// parity: E4011
module Child {
    inout sda : bool
    in    oe  : bool
}
module M {
    in    x : bool
    inout pin : bool
    let c = Child { sda: !x, oe: x }
}
