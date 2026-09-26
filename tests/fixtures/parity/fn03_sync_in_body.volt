// parity: E2016
fn f(a: u8, c: u8) -> u8 { sync(a, c) }
module M { in a : u8  out y : u8  y = a }
