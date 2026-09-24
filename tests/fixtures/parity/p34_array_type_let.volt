// parity: E0003
module M {
    in  x : u8
    out y : u8
    let a : [u8; 2] = [x, x]
    y = a[0]
}
