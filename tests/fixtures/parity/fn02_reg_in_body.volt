// parity: E2016
fn f(a: u8) -> u8 {
    reg r : u8 = 0
    a
}
module M { in a : u8  out y : u8  y = f(a) }
