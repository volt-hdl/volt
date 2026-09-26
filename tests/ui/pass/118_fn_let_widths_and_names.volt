// ADR-0081 (Aşama 2 inceleme bulguları): ikame kipinde her let tel
// kipindeki telinin genişliğinde hesaplanır; üretilen adlar kendi
// aralarında çakışmaz (let parametreyi gölgeler, struct sonucun yaprağı
// bir let teliyle aynı ad olur) — _2 soneki.

struct Pair {
    a : u8
    b : u8
}

fn addw(a: u8, b: u8) -> u9 {
    let t = a + b
    t
}

fn trunc16(a: u8, b: u8) -> u16 {
    let s : u8 = a + b
    s
}

fn sh(a: u8) -> u8 {
    let a = a ^ 3
    a + 1
}

fn mk(x: u8) -> Pair {
    let a = x + 1
    Pair { a, b: x }
}

module FnLetWidths {
    in  a  : u8
    in  b  : u8
    out yw : u9
    out yc : u9
    out tw : u16
    out tc : u16
    out q  : u8
    out r  : u8

    yw = addw(a, b)
    tw = trunc16(a, b)
    comb {
        yc = addw(a, b)
        tc = trunc16(a, b)
    }
    q = sh(a ^ b) + b
    r = mk(a).a ^ mk(b).b
}
