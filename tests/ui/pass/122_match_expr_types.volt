// ADR-0083 Karar 2: beklenen tip her kola itilir (check kipi) — `a + b`
// 9 bitte hesaplanır, taşma korunur; struct tipli sonuç alan başına
// indirgenir (if gibi).

struct Px {
    v  : u8
    ok : bool
}

module MatchExprTypes {
    in  op : u2
    in  a  : u8
    in  b  : u8
    out w  : u9
    out pv : u8
    out pk : bool

    let wide : u9 = match op { 0 => a + b, 1 => a, _ => b }
    w = wide

    let p : Px = match op {
        0 => Px { v: a, ok: true },
        _ => Px { v: b, ok: false }
    }
    pv = p.v
    pk = p.ok
}
