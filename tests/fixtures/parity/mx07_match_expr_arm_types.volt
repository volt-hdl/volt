// parity: E2003
// ADR-0083 §6 sondası.
module M {
    in  op : u2
    in  a  : u8
    out y  : u8
    let r = match op { 0 => a, _ => true }
    y = a
}
