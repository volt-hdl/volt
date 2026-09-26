// ADR-0083: modül düzeyinde match ifadesi. Tüm sağ taraf (let, atama)
// kök konumdur → always_comb + case; operand iç konumdur → üçlü zincir.
// Sayısal sınanan `_` ister; enum'da bütün varyantlar yeter (son kol
// geçersiz kodları da alır — ADR-0074 Karar 4); alternatif desen `A | B`.

enum AluOp { Add, Sub, And, Or, Xor }

const SEL : u2 = 2
const K : u8 = match SEL { 0 => 10, 1 | 2 => 20, _ => 30 }

module MatchExprModule {
    in  op   : u2
    in  eop  : AluOp
    in  a    : u8
    in  b    : u8
    out r1   : u8
    out r2   : u8
    out r3   : u8
    out r4   : u8

    // Kök: modül let'i → case, `1 | 2` tek kol.
    let pick = match op { 0 => a, 1 | 2 => b, _ => a ^ b }
    r1 = pick

    // Kök: enum sınanan, `_`'sız — son kol default.
    r2 = match eop {
        AluOp::Add => a + b,
        AluOp::Sub => a - b,
        AluOp::And => a & b,
        AluOp::Or  => a | b,
        AluOp::Xor => a ^ b
    }

    // İç: operand ve iç içe match.
    r3 = (match op { 0 => match eop { AluOp::Add => a, _ => b }, _ => K }) + 1

    // Sabit bağlamda match derleme zamanında seçilir.
    r4 = K
}
