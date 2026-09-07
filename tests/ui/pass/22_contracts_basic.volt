// Kontrat sistemi (F4a): requires + ensures + invariant + cover.
// Tip kontrolü (E5004), kapsam kuralları ve SVA üretimi bu modülü
// temel alır: requires/ensures portları, invariant register'ları,
// cover ikisini birden görür.

module SpiCtrl {
    in  clk   : clock
    in  start : bool
    in  speed : u8
    out busy  : bool
    out done  : bool

    requires:  speed <= 2
    ensures:   !start || busy
    invariant: !(busy_r && done_r)
    cover:     speed == 2 && done_r

    reg busy_r : bool = false
    reg done_r : bool = false

    on clk {
        busy_r <= start
        done_r <= busy_r
    }

    busy = busy_r
    done = done_r
}
