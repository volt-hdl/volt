// parity: E1003
fn f(a: u8) -> u8 { a + 1 }
module M {
    in  a : u8
    out y : u8
    let f_0 = a
    y = f(a + 1) + f_0
}
