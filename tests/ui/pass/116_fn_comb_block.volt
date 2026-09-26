// ADR-0081: comb bloğunda ve blok içi for'da çağrı — ikame kipi: tel yok,
// parametreler argümanla, let'ler değerleriyle yer değiştirir; genişlik
// SV boyut dönüşümüyle korunur.

fn mix(a: u8, b: u8) -> u8 {
    let t = a ^ b
    t + (t >> 1)
}

module FnComb {
    in  xs : [u8; 4]
    in  k  : u8
    out ys : [u8; 4]
    out m  : u8

    comb {
        m = mix(xs[0] + 1, k)
        for i in 0..4 {
            ys[i] = mix(xs[i], k)
        }
    }
}
