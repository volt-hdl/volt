//~ E2009
// Sayısal tipten Trit'e dönüşüm yasak.
// Sessiz bilgi kaybı olur.

module InvalidCast {
    in  value : i8
    out t     : Trit

    t = value as Trit
    //~^ ERROR 'i8' → 'Trit' dönüşümü geçersiz
}
