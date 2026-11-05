// ADR-0081 Karar 12.1, #128: açılmış fn gövdesindeki bir ad fn kapsamında
// çözülür; ebeveyni kök kapsamdır. Gövdedeki const (indeks, parça seçimi
// başlangıcı, tip genişliği, desen, dizi sabiti) çağıranın aynı adlı döngü
// değişkenine ya da portuna bağlanmaz. Gölgeleme yasak değil, W1002
// (name-resolution.md §6).

const I : u32 = 1
const W : u32 = 4
const T : [u8; 4] = [10, 20, 30, 40]
const B : u4 = 2

fn pick(x: [u8; 4]) -> u8 { x[I] }

fn sl(a: u8) -> u2 { a[I +: 2] as u2 }

fn inc(a: uint<W>) -> uint<W> { a + 1 }

fn cls(x: u2) -> u8 { match x { I => 7, _ => 0 } }

fn lut(i: u2) -> u8 { T[i] }

// Sabit indeksin tipi (u4) vektörün adres genişliğinden (3 bit) farklı:
// seçim biçiminde kalır, `B` çağıranın döngü değişkenine bağlanmaz.
fn bit(v: u8) -> bool { v[B] }

module FnConstHygiene {
    in  xs : [u8; 4]
    in  a  : u8
    in  b  : u4
    in  s  : u2
    in  W  : u8
    out ys : [u8; 4]
    out ps : [u2; 2]
    out qs : [u4; 2]
    out y  : u8
    out z  : u8
    out xo : [u8; 4]
    out ao : u8
    out bs : [bool; 2]

    comb {
        for I in 0..4 { ys[I] = pick(xs) }
        for I in 0..2 { ps[I] = sl(a) }
        for T in 0..2 { qs[T] = inc(b) }
        for B in 0..2 { bs[B] = bit(a) }
    }
    y = cls(s) + lut(s)
    z = W
    // Girişlerin bütün bitleri okunur (Verilator -Wall UNUSEDSIGNAL).
    xo = xs
    ao = a
}
