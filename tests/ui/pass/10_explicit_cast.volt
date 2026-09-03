// Açık tip dönüşümü — örtük genişleme yasak
module ExplicitCast {
    in  small : u8
    in  large : u16
    out sum   : u17
    out narrow: u8

    // Genişletme: as ile açık
    sum = (small as u16) + large

    // Daraltma: as ile açık (uyarı üretir ama geçerli)
    narrow = large as u8
}
