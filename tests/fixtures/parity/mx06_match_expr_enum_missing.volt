// parity: E0014
// ADR-0083 §6 sondası.
enum E { A, B, C }
module M {
    in  e : E
    out y : u8
    y = match e { E::A => 1, E::B => 2 }
}
