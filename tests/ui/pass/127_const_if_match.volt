// ADR-0083 Aşama 3 (Gelecek iş 6): const başlangıcında if ve match
// ifadesi. HIR consteval değeri hesaplar; SV üreticisinin sabit
// katlayıcısı da aynısını hesaplar — çıktıda tanımsız ad kalmaz.
// Koşul karşılaştırma, &&/||/->, ! ve bool const'u olabilir.

const W : u32 = 8
const WIDE : bool = W > 4 && !(W == 6)
const M : u32 = if WIDE { W * 2 } else { W }
const K : u32 = match M { 16 => 3, _ => 5 }
const MASK : u32 = (1 << K) - 1
const TAG : u8 = if !WIDE { 1 } else if WIDE -> (K >= 3) { 2 } else { 4 }

module ConstIfMatch {
    in  a    : bits<M>
    out y    : bits<M>
    out k    : u32
    out mask : u32
    out tag  : u8
    out wide : bool

    y = a
    k = K
    mask = MASK
    tag = TAG
    wide = WIDE
}
