// parity: E4013
fn f(a: u8) -> u8 { g(a) }
fn g(a: u8) -> u8 { f(a) + 1 }
module M { in a : u8  out y : u8  y = f(a) }
