// ADR-0083 Karar 5/10: kontrattaki match ifadesi üçlü zincire iner
// (immediate ve --emit=sva); aynı seçim register'ı sürer.

module MatchExprContract {
    in  clk : clock
    in  op  : u2
    in  a   : u8
    in  b   : u8
    out y   : u8

    reg q : u8 = 0
    on clk {
        q <= match op { 0 => a, 1 => b, _ => 0 }
    }
    y = q

    // q, önceki çevrimin seçimidir; seçim 3'te sıfırdır.
    invariant: (match op { 3 => 0u8, _ => 1u8 }) <= 1
    invariant: (match op { 0 => a, 1 => b, _ => 0 }) == (if op == 0 { a } else if op == 1 { b } else { 0 })
}
