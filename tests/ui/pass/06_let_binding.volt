// let bağlaması: ara değerler, tip çıkarımı
// let → SV'de wire olarak üretilir
module LetBinding {
    in  a : u8
    in  b : u8
    in  c : u8
    out y : u16

    // Tip çıkarımı: u8 + u8 → u9
    let sum = a + b

    // Genişleme sonrası
    let widened = sum as u16

    // Zincirli kullanım
    let scaled = widened * (c as u16)

    y = scaled
}
