// parity: ok
// drivers: ok
module M {
    in  a : Trit
    in  b : Trit
    out t : [Trit; 2]

    t[0] = a
    t[1] = b
}
