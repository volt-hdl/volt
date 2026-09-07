//~ E5001
// İhlal edilen invariant (F4b): sayaç 10'a kadar çıkabildiği halde
// kontrat 5'in altında kalacağını iddia ediyor. Bu dosya DERLEMEDEN
// geçer — ihlali derleyici değil 'volt verify' (SymbiYosys karşı
// örneği, çıkış kodu 6) yakalar; beklenen kod bu yüzden E5001'dir.

module LeakyCounter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    invariant: count_r < 5

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
