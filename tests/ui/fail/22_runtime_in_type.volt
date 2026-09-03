//~ E2021
// Tip pozisyonunda çalışma zamanı değeri.
// Tip genişlikleri derleme zamanında bilinmeli.

module RuntimeInType {
    in  width_sig : u8
    in  data      : bits<width_sig>
    //~^ ERROR sabit ifade bekleniyor
    out r         : u8

    r = 0
}
