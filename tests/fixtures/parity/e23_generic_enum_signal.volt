// parity: E0003
enum G<T> { A, B }
module M {
    in  c : G<u8>
    out y : bool
    y = true
}
