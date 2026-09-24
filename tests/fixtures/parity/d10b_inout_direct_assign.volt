// parity: E4008
// drivers: ok
module M {
    in  v : bits<8>
    inout dq : bits<8>

    dq = v
}
