// parity: E0014
// ADR-0083 §6 sondası.
module M {
    in  op : u2
    out y  : u8
    y = match op { 0 => 1, 1 => 2, 2 => 3, 3 => 4 }
}
