// ADR-0083 Karar 6-8, 11: blok içi let saf bir ara addır (register
// değil) — süreç içi yerel değişken. on bloğunda register'ı okuyan let
// güncellenmeden önceki değeri görür (r <= … sonrası da); comb
// bloğunda bildirim noktasına kadarki blocking atamaları görür. Kapsam
// içinde bulunduğu { } bloğudur: if dalı, match kolu, for gövdesi.

module BlockLetOnComb {
    in  clk : clock
    in  en  : bool
    in  op  : u2
    in  a   : u8
    in  b   : u8
    out q   : u8
    out w   : u8
    out y   : u8
    out z   : u9

    reg r : u8 = 0
    reg old : u8 = 0
    on clk {
        let s = a + b
        if en {
            let t = s ^ a
            r <= t
        }
        // r <= … sonrası okuma: eski r (non-blocking).
        let prior = r
        old <= prior
        match op {
            0 => {
                let k = a & b
                old <= k
            }
            _ => { }
        }
    }
    q = r
    w = old

    comb {
        y = 0
        for i in 0..4 {
            let acc = y + a
            y = acc
        }
        let typed : u9 = a + b
        z = typed
    }
}
