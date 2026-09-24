// parity: E0003
module M {
    in  x : u2
    out y : u8
    y = match x { 0 => 1u8, _ => 2u8 }
}
