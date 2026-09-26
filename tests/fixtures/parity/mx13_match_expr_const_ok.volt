// parity: ok
// ADR-0083 §6 sondası.
const K : u8 = match 3 { 1 => 10, 3 => 30, _ => 0 }
module M {
    out y : u8
    y = K
}
