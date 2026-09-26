// ADR-0081: saf kombinasyonel fn — modül let'i, atama ve ifade içinde çağrı.
// SV'de fn görünmez: her çağrı çağrı yerinde açılır (Karar 12, tel kipi).

fn sat_inc(a: u8) -> u8 {
    if a == 255 { a } else { a + 1 }
}

fn parity(x: u8) -> bool {
    let lo = x[3:0] ^ x[7:4]
    let q = lo[1:0] ^ lo[3:2]
    q[0] != q[1]
}

module FnSimple {
    in  a   : u8
    in  b   : u8
    out inc : u8
    out sum : u8
    out par : bool

    // Tüm sağ taraf: sonuç teli yazılmaz, `t` açılmış ifadeyle sürülür.
    let t = sat_inc(a)
    inc = t
    // İfade içinde: sonuç teli sat_inc_1.
    sum = sat_inc(b) + a
    // Yalın olmayan argüman tel alır: parity_0_x.
    par = parity(a ^ b)
}
