// parity: E0003
fn f(a: u8) -> u8 { match a { 0 => 1, _ => a } }
module M { in a : u8  out y : u8  y = f(a) + f(a + 1) }
