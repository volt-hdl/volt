// ADR-0083 §5.5 karşı örneği: comb bloğunda let bildirim noktasındaki
// değeri tutar — sonraki blocking atama onu değiştirmez (modül teline
// taşınsaydı z son y'yi okurdu). Yerel değişken sıralı anlamı korur.

module BlockLetCombOrder {
    in  c : bool
    in  a : u8
    out y : u8
    out z : u8

    comb {
        y = 0
        let t = y
        if c {
            y = a
        }
        z = t
    }
}
