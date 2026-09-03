//~ E2001
// Bit genişliği uyumsuzluğu: örtük genişleme yok.
// Açık dönüşüm (as) gerekli.

module WidthMismatch {
    in  small : u8
    in  large : u16
    out sum   : u16

    sum = small + large
    //~^ ERROR bit genişliği uyumsuzluğu: u8 ve u16
}
