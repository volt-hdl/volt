// İç içe koşullar, else zorunluluğu
module NestedCond {
    in  a : bool
    in  b : bool
    in  c : bool
    in  x : u8
    in  y : u8
    out r : u8

    // if ifadesinde else ZORUNLU (E0008 önleme)
    r = if a {
            if b { x } else { y }
        } else {
            if c { y } else { 0 }
        }
}
