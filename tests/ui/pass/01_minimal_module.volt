// Minimal modül: sadece bir port bağlantısı
module Passthrough {
    in  a : u8
    out b : u8

    b = a
}
