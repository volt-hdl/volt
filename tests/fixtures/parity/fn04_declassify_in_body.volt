// parity: E3015
fn f(a: u8) -> u8 { declassify(a, "debug") }
module M { in a : u8  out y : u8  y = f(a) }
