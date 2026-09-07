// Kanıtlanabilir invariant (F4b): sayaç 10'da sarar, bu yüzden
// count_r <= 10 erişilebilir her durumda doğrudur. 'volt verify'
// bmc/prove kipinde bu tasarımı çıkış kodu 0 ile geçirmelidir.

module BoundedCounter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    invariant: count_r <= 10

    reg count_r : u8 = 0

    on clk {
        if enable {
            if count_r == 10 {
                count_r <= 0
            } else {
                count_r <= count_r + 1
            }
        }
    }

    count = count_r
}
