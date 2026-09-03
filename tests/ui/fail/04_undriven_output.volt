//~ E4002
// Çıkış portu hiç sürülmüyor.

module UndrivenOutput {
    in  a : u8
    out y : u8
    out z : u8

    y = a
    // z hiç atanmadı
}
//~^ ERROR 'z' çıkış portu sürülmüyor
