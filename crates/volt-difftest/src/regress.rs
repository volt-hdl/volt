//! Sabit regresyon kümesi: fark testinin bulduğu her kök nedenin en küçük
//! örneği. Her koşunun ilk toplu koşusudur (CI'daki küçük koşu dahil);
//! `VOLT_DIFFTEST_COUNT=0` yalnız bunu koşturur. Her örnek bulunduğu
//! issue'yu taşır.
//!
//! Beklenen değerin iki kaynağı vardır. Her vektörde değer referans
//! modelden gelir (`model.rs`). Ayrıca her issue'nun en az bir örneğinde
//! girişler ve beklenen çıkış spesifikasyondan elle hesaplanmıştır
//! (`pin`), gerekçesi örneğin altındaki yorumda; koşucu bu değerleri
//! Verilator'un çıktısıyla modelden bağımsız karşılaştırır. Model ile
//! derleyici aynı yanlışı yapsa bile elle hesaplanan değer onu yakalar;
//! `pins_agree_with_the_model` modelin de aynı değeri verdiğini sınar.
//! Elle hesap yalnız spesifikasyonun değeri belirlediği örneklerdedir:
//! değeri bir varsayım etiketine (A1–A12, ADR-0108) bağlı örnek (işaretsiz
//! çıkarmanın sarması, esnek toplamın kaydırması) elle sabitlenmez.

use crate::ast::{BinOp, Expr, LitFmt, PortTy, UnOp};

fn p(k: usize) -> Expr {
    Expr::Port(k)
}

fn lit(v: i128) -> Expr {
    Expr::Lit(v, LitFmt::Dec)
}

fn hex(v: i128) -> Expr {
    Expr::Lit(v, LitFmt::Hex)
}

fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::bin(op, a, b)
}

fn cast(a: Expr, t: PortTy) -> Expr {
    Expr::cast(a, t)
}

fn range(a: Expr, h: u32, l: u32) -> Expr {
    Expr::Range(Box::new(a), h, l)
}

fn index(a: Expr, i: Expr) -> Expr {
    Expr::Index(Box::new(a), Box::new(i))
}

/// Spesifikasyondan elle hesaplanmış bir değer: port başına giriş
/// (işaretli tipte işaretli değer) ve ifadenin doğal tipindeki beklenen
/// değer (`bool` için 0/1).
#[derive(Clone, Debug)]
pub struct Pin {
    pub inputs: Vec<i128>,
    pub want: i128,
}

/// Regresyon örneği: issue, açıklama, port tipleri, ifade, elle
/// hesaplanmış değerler.
pub struct Regression {
    pub issue: &'static str,
    pub what: &'static str,
    pub ports: Vec<PortTy>,
    pub expr: Expr,
    pub pins: Vec<Pin>,
    /// C biçiminin `let`'e çıkaracağı alt ifadenin yolu (`Expr::at`);
    /// `None`: rastgele iç düğüm. Sabit yol, r biçiminin gövde `let`'ini
    /// tohumdan bağımsız kılar (#132).
    pub extract: Option<Vec<usize>>,
}

impl Regression {
    fn pin(mut self, inputs: &[i128], want: i128) -> Regression {
        assert_eq!(inputs.len(), self.ports.len(), "{}", self.what);
        self.pins.push(Pin {
            inputs: inputs.to_vec(),
            want,
        });
        self
    }

    fn extracting(mut self, path: &[usize]) -> Regression {
        self.extract = Some(path.to_vec());
        self
    }
}

fn r(issue: &'static str, what: &'static str, ports: Vec<PortTy>, expr: Expr) -> Regression {
    Regression {
        issue,
        what,
        ports,
        expr,
        pins: Vec::new(),
        extract: None,
    }
}

pub fn all() -> Vec<Regression> {
    use PortTy::*;
    vec![
        // ── `as`: SV ifadesinin işareti hedefin işareti değil ──
        r(
            "#115",
            "sign-extending cast to a signed type compares unsigned",
            vec![SInt(10)],
            bin(BinOp::Gt, cast(p(0), SInt(11)), lit(880)),
        )
        // `p0 as i11 > 880`, p0 : i10. type-inference §3.6: i10 → i11
        // genişletmedir, değer korunur (-1 → -1). §3.3/§4: soneksiz 880
        // karşı yanın tipini (i11) alır ve sığar; karşılaştırma iki i11
        // arasında, işaretli: -1 > 880 yanlış. i10'un en büyüğü 511 < 880,
        // ifade hep yanlış. Düzeltmeden önce SV `{{1{a[9]}}, a} > 11'sd880`
        // işaretsizdi: -1 = 2047 > 880, doğru.
        .pin(&[-1], 0)
        .pin(&[-512], 0)
        .pin(&[511], 0),
        r(
            "#115",
            "narrowing cast to a signed type compares unsigned",
            vec![SInt(36)],
            bin(BinOp::Gt, cast(p(0), SInt(1)), lit(0)),
        ),
        r(
            "#115",
            "zero-extending cast to a signed type shifts logically",
            vec![UInt(22), UInt(1)],
            bin(
                BinOp::Gt,
                bin(BinOp::Shr, cast(p(0), SInt(38)), p(1)),
                lit(-92_102_428_598),
            ),
        ),
        r(
            "#115",
            "sign-extending cast then right shift is logical",
            vec![SInt(7)],
            bin(BinOp::Shr, cast(p(0), SInt(50)), lit(8)),
        ),
        r(
            "#115",
            "narrowing a signed sum to an unsigned type keeps the sign",
            vec![SInt(14)],
            cast(bin(BinOp::Add, p(0), p(0)), UInt(7)),
        ),
        r(
            "#115",
            "negating a sign-extended value",
            vec![SInt(2)],
            Expr::un(UnOp::Neg, cast(p(0), SInt(4))),
        )
        // `-(p0 as i4)`, p0 : i2. §3.6: i2 → i4 değer korunur; §3.4: tekli
        // `-` i4'ü i5 yapar, -(-1) = 1 ve -(-2) = 2 sığar (i5: -16..15).
        // Önceden `-(5'({{2{s2[1]}}, s2}))`: birleştirme işaretsiz, 5 bitte
        // sıfırla genişliyor, -(01111) = 0x11 = -15.
        .pin(&[-1], 1)
        .pin(&[-2], 2)
        .pin(&[1], -1),
        // ── literaller ──
        r(
            "#116",
            "hex literal in a signed comparison",
            vec![SInt(50)],
            bin(BinOp::Gt, p(0), hex(0)),
        )
        // `p0 > 0x0`, p0 : i50. §4: soneksiz literal karşı yanın tipini
        // (i50) alır; yazımı (onaltılık) değerini değiştirmez, değer 0.
        // İşaretli karşılaştırma: -1 > 0 yanlış, 1 > 0 doğru. Önceden
        // `50'h0` işaretsizdi ve karşılaştırmayı işaretsiz yapıyordu.
        .pin(&[-1], 0)
        .pin(&[1], 1),
        r(
            "#116",
            "hex literal in a signed bitwise operation",
            vec![SInt(18)],
            bin(BinOp::Gt, bin(BinOp::Or, p(0), hex(0x1FFFF)), lit(95146)),
        ),
        r(
            "#116",
            "most negative literal of the width",
            vec![SInt(1)],
            cast(bin(BinOp::Mul, p(0), lit(-1)), UInt(35)),
        )
        // `(p0 * (-1)) as u35`, p0 : i1. §3.4: `-1` soneksiz literaldir;
        // §3.3: literal karşı yanın tipini (i1) alır, -1 i1'e sığar;
        // i1 * i1 → i2 (`*` genişlikleri toplar), (-1)(-1) = 1, i2'ye
        // sığar. §3.6: i2 → u35 işaret değişimi; değer pozitif olduğundan
        // hangi işaretle genişlerse genişlesin 1. Önceden `-1'sd1` geniş
        // bağlamda +1'e dönüyordu: sonuç 0x7_FFFF_FFFF.
        .pin(&[-1], 1)
        .pin(&[0], 0),
        // ── değeri bağlama göre değişen alt ifade (#106 sınıfı) ──
        r(
            "#117",
            "unsigned subtraction under a wider product",
            vec![UInt(12), UInt(12)],
            bin(BinOp::Mul, bin(BinOp::Sub, p(0), lit(4095)), p(1)),
        ),
        r(
            "#117",
            "unsigned subtraction under a wider sum",
            vec![UInt(8), UInt(8), UInt(9)],
            bin(BinOp::Add, bin(BinOp::Sub, p(0), p(1)), p(2)),
        ),
        r(
            "#117",
            "unsigned subtraction in a widening cast",
            vec![UInt(8), UInt(8)],
            cast(bin(BinOp::Sub, p(0), p(1)), UInt(16)),
        ),
        r(
            "#117",
            "carry of an operand dropped by a bitwise operation, in a wider sum",
            vec![UInt(8), UInt(8), UInt(8)],
            bin(
                BinOp::Add,
                bin(BinOp::Or, bin(BinOp::Add, p(0), p(1)), p(2)),
                p(2),
            ),
        )
        // `((p0 + p1) | p2) + p2`, hepsi u8. §3.3: u8 + u8 → u9; ADR-0025:
        // toplamın aralığı [8, 9], u8 ile bit işleminde kesişim 8: `|`
        // u8'dir, taşma biti düşer. 200 + 100 = 300 → 8 bitte 44; 44 | 3 =
        // 47; u8 + u8 → u9, 47 + 3 = 50. Önceden `|` 9 bitte yapılıyordu:
        // (300 | 3) + 3 = 306.
        .pin(&[200, 100, 3], 50)
        .pin(&[255, 1, 0], 0),
        r(
            "#117",
            "right shift of a sum reads the carry",
            vec![UInt(8), UInt(8), UInt(3)],
            bin(BinOp::Shr, bin(BinOp::Add, p(0), p(1)), p(2)),
        ),
        r(
            "#117",
            "negation inside a shift",
            vec![SInt(20), UInt(2)],
            bin(BinOp::Shr, Expr::un(UnOp::Neg, p(0)), p(1)),
        )
        // `(-p0) >> p1`, p0 : i20, p1 : u2. §3.4: `-` i20'yi i21 yapar;
        // -(-524288) = 524288 = 2^19, i21'e sığar (en büyüğü 2^20 - 1).
        // §3.3: kaydırma sol işlenenin tipinde (i21); değer pozitif,
        // 524288 >> 1 = 262144. p1 = 0 iken kaydırma yok: -3. Önceden
        // `-s20` 20 bitte taşıyordu: -524288 >>> 1 = -262144.
        .pin(&[-524_288, 1], 262_144)
        .pin(&[3, 0], -3),
        r(
            "#117",
            "~ under a same-width cast",
            vec![Bits(10)],
            bin(
                BinOp::Add,
                cast(Expr::un(UnOp::BitNot, p(0)), UInt(10)),
                lit(0),
            ),
        )
        // `(~p0) as u10 + 0`, p0 : bits<10>. §3.4: `~` genişliği korur
        // (bits<10>); §3.6: bits<10> → u10 aynı genişlik; §3.3: 0 u10
        // olur, u10 + u10 → u11. ~0 = 1023, 1023 + 0 = 1023. Önceden `~` 11
        // bitte yapılıyordu: 2047.
        .pin(&[0], 1023)
        .pin(&[0x3FF], 0),
        // ── geçersiz SV ──
        r(
            "#118",
            "-(-x) is written as the SV decrement operator",
            vec![SInt(56)],
            Expr::un(UnOp::Neg, Expr::un(UnOp::Neg, p(0))),
        )
        // `-(-p0)`, p0 : i56. §3.4: her `-` bir bit ekler, i56 → i57 →
        // i58; -(-x) = x. En küçük değer -2^55: -(-2^55) = 2^55 i57'ye
        // sığar, -(2^55) = -2^55. Önceden SV `--(...)` (azaltma işleci),
        // Verilator derlemiyordu.
        .pin(&[-1], -1)
        .pin(&[-36_028_797_018_963_968], -36_028_797_018_963_968),
        r(
            "#119",
            "range of a range of a computed value",
            vec![Bits(70), UInt(6)],
            range(range(bin(BinOp::Shr, p(0), p(1)), 69, 0), 69, 0),
        ),
        r(
            "#119",
            "bit of a range of a cast",
            vec![UInt(34)],
            index(range(cast(p(0), Bits(34)), 28, 0), lit(27)),
        )
        // `(p0 as bits<34>)[28:0][27]`, p0 : u34. §3.6: u34 → bits<34>
        // aynı genişlik, bit örüntüsü aynı; §3.5: `[28:0]` bits<29>, alt 29
        // bit; `[27]` onun 27. biti, yani p0'ın 27. biti (bool). p0 = 2^27
        // → doğru; 27. bit dışında hepsi bir → yanlış. Önceden `29'(q)[27]`
        // geçersiz SV'ydi.
        .pin(&[134_217_728], 1)
        .pin(&[17_045_651_455], 0),
        r(
            "#120",
            "bit 0 of a one-bit value",
            vec![UInt(1)],
            index(p(0), lit(0)),
        )
        // `p0[0]`, p0 : u1. §3.5: sabit indeks 0 < 1 geçerli, sonuç bool:
        // p0'ın tek biti. Önceden `one[0]` skaler `logic`'ten seçimdi,
        // geçersiz SV.
        .pin(&[1], 1)
        .pin(&[0], 0),
        r(
            "#120",
            "range [0:0] of bits<1>",
            vec![Bits(1)],
            range(p(0), 0, 0),
        ),
        r(
            "#120",
            "bit 0 of a signed one-bit value is unsigned (part-select start)",
            vec![Bits(69), SInt(1)],
            Expr::Part(
                Box::new(p(0)),
                Box::new(cast(range(p(1), 0, 0), UInt(1))),
                68,
                true,
            ),
        )
        // `p0[(p1[0:0] as u1) +: 68]`, p0 : bits<69>, p1 : i1. §3.5:
        // `p1[0:0]` bits<1>, p1'in bit örüntüsü (-1 → 1); §3.6: bits<1> →
        // u1, başlangıç 1 (işaretsiz). ADR-0035: `[s +: 68]` s..s+67
        // bitleri, s = 1 sınır içinde (1 + 68 ≤ 69). p0 = 2^68 → bit 68
        // sonucun 67. biti: 2^67; p0 = 1 → 0. p1 = 0 → başlangıç 0, p0 = 1
        // → 1. Önceden başlangıç SV'de işaretli kalıyordu (-1).
        .pin(
            &[295_147_905_179_352_825_856, -1],
            147_573_952_589_676_412_928,
        )
        .pin(&[1, -1], 0)
        .pin(&[1, 0], 1),
        // ── Verilator WIDTH (ölümcül): indeks adres genişliğinde değil ──
        r(
            "#123",
            "variable bit index narrower than the address width",
            vec![UInt(15), UInt(3)],
            index(p(0), p(1)),
        )
        // `p0[p1]`, p0 : u15, p1 : u3. §3.5: değişken indeks, bool; u3'ün
        // en büyüğü 7 < 15, hep sınır içinde. p0 = 2^7: indeks 7 → doğru,
        // 6 → yanlış.
        .pin(&[128, 7], 1)
        .pin(&[128, 6], 0),
        r(
            "#123",
            "variable part-select start wider than the address width",
            vec![UInt(51), UInt(8)],
            Expr::Part(Box::new(p(0)), Box::new(p(1)), 35, true),
        )
        // `p0[p1 +: 35]`, p0 : u51, p1 : u8. ADR-0035: p1..p1+34 bitleri,
        // bits<35>; p1 ≤ 16 iken sınır içinde. p0 = 2^50 + 1: p1 = 16 →
        // 16..50 bitleri, bit 50 sonucun 34. biti: 2^34; p1 = 0 → 0..34
        // bitleri: 1.
        .pin(&[1_125_899_906_842_625, 16], 17_179_869_184)
        .pin(&[1_125_899_906_842_625, 0], 1),
        // ── Ad hijyeni: açılmış fn gövdesinin const'u (r biçimi) ──
        r(
            "#128",
            "constant index in an inlined function called in a loop whose variable has the constant's name",
            vec![UInt(8)],
            index(p(0), lit(1)),
        )
        // `p0[1]`, p0 : u8: bool, bit 1 (ADR-0035). r biçiminde `1` kök
        // `const t<id>r : u32 = 1` olur ve çağrı `for t<id>r in 2..3`
        // içindedir; gövde t<id>r'yi fn kapsamında, kökte çözer (ADR-0081
        // Karar 12.1, name-resolution.md §2): yine bit 1. p0 = 0b10 → 1;
        // p0 = 0b100 → 0 (#128'de döngü değişkeni seçilirdi: bit 2, 1).
        .pin(&[2], 1)
        .pin(&[4], 0),
        // ── Ayrıştırıcının ikamesi: generic parametreyi gölgeleyen `let` ──
        // İki örnek art arda: kimlikleri (listedeki sıra, 25 ve 26) birini
        // modül düzeyi `for` gövdesine, birini `comb`'a koyar
        // (`hygiene::module_level`).
        r(
            "#130",
            "argument through a body let of a module-level for that shadows a generic parameter",
            vec![UInt(8), UInt(8)],
            bin(BinOp::Xor, p(0), p(1)),
        )
        // `p0 ^ p1`, ikisi u8: u8 (type-inference §3.3, bit işlemi). r
        // biçiminde p0, çağıran `R<id>r<0, ..>`'nin `let s<id>r = p0`'ından
        // geçer; `let` generic `s<id>r`'yi gölgeler (name-resolution.md §2,
        // §6), argüman p0'dır: 15 ^ 240 = 255; 1 ^ 2 = 3. (#130'da argüman
        // generic'in değeri 0 olurdu: 240, 2.)
        .pin(&[15, 240], 255)
        .pin(&[1, 2], 3),
        r(
            "#130",
            "argument through a block let in comb that shadows a generic parameter",
            vec![UInt(8), UInt(8)],
            bin(BinOp::Xor, p(0), p(1)),
        )
        // Aynı ifade, çağrı `comb` içinde: blok `let s<id>r` generic'i
        // gölgeler. 85 ^ 170 = 255; 5 ^ 7 = 2 (#130'da 170, 7).
        .pin(&[85, 170], 255)
        .pin(&[5, 7], 2),
        // ── Literal aritmetiği tam değerle katlanır (#144) ──
        // Listenin sonunda: #130'un iki örneğinin kimlikleri (25, 26) kaymaz.
        r(
            "#144",
            "literal division compared with a narrow signal",
            vec![UInt(2)],
            bin(BinOp::Eq, p(0), bin(BinOp::Div, lit(7), lit(3))),
        )
        // `p0 == 7 / 3`, p0 : u2. type-inference §3.3: iki literalin
        // aritmetiği literaldir; const-eval §1, §4: değeri i128'de tam, 7 / 3
        // = 2 (sıfıra doğru). §4 ve ADR-0106 §1: literal karşı yanın tipine
        // (u2) uyarlanır, 2 sığar. p0 = 2'de doğru, 1'de yanlış. (#144'te SV
        // `p0 == 2'h3 / 2'd3`: sağ yan 1.)
        .pin(&[2], 1)
        .pin(&[1], 0),
        r(
            "#144",
            "literal remainder as the bound of an ordering comparison",
            vec![UInt(2)],
            bin(BinOp::Lt, p(0), bin(BinOp::Rem, lit(5), lit(3))),
        )
        // `p0 < 5 % 3`: 5 % 3 = 2 (const-eval §4 `checked_rem`), u2'ye
        // sığar. p0 = 1'de doğru, 2'de yanlış (#144'te sağ yan 1: 1 < 1
        // yanlış).
        .pin(&[1], 1)
        .pin(&[2], 0),
        r(
            "#144",
            "literal division as a bitwise operand",
            vec![UInt(2)],
            bin(BinOp::And, p(0), bin(BinOp::Div, lit(7), lit(3))),
        )
        // `p0 & (7 / 3)`: literal u2'ye uyarlanır (§3.3 bit işlemi), değeri
        // 2: 3 & 2 = 2; 1 & 2 = 0. (#144'te `p0 & 2'h3 / 2'd3` = p0 & 1.)
        .pin(&[3], 2)
        .pin(&[1], 0),
        r(
            "#144",
            "intermediate value of literal arithmetic wider than the context",
            vec![UInt(2)],
            bin(
                BinOp::Add,
                p(0),
                bin(BinOp::Div, bin(BinOp::Mul, lit(100), lit(3)), lit(150)),
            ),
        )
        // `p0 + 100 * 3 / 150`: ara değer 300 hiçbir dar tipe sığmaz ama
        // kesilmez (const-eval §1); 300 / 150 = 2, u2'ye sığar. Toplam u3
        // (§3.3: u2 + u2 → u3): 1 + 2 = 3, 3 + 2 = 5. (#144'te literaller u3
        // bağlamına kesiliyordu: 3'h4 * 3'd3 / 3'h6.)
        .pin(&[1], 3)
        .pin(&[3], 5),
        r(
            "#144",
            "signed literal division compared with a signed signal",
            vec![SInt(4)],
            bin(
                BinOp::Eq,
                p(0),
                bin(BinOp::Div, Expr::un(UnOp::Neg, lit(9)), lit(2)),
            ),
        )
        // `p0 == -9 / 2`, p0 : i4. -9 i4'e (-8..=7) sığmaz ama ara değerdir;
        // -9 / 2 = -4 (const-eval §4 `checked_div`, sıfıra doğru), i4'e sığar.
        // p0 = -4'te doğru, -5'te yanlış. (#144'te `4'sh7 / 4'sd2` = 3.)
        .pin(&[-4], 1)
        .pin(&[-5], 0),
        r(
            "#144",
            "literal arithmetic as an operand of a subtraction pushed into a wider target",
            vec![UInt(11)],
            bin(BinOp::Sub, bin(BinOp::Div, lit(34090), lit(40)), p(0)),
        ),
        // `34090 / 40 - p0`, p0 : u11. Literal aritmetiği literaldir (§3.3):
        // W biçiminde (daha geniş hedef) ADR-0041'in itmesi çıkarmayı hedef
        // genişlikte yapar, literal işlenen itmeyi engellemez (type-inference
        // §5: "literal olmayan operandları"). Düzeltmeden önce üretici yalnız
        // çıplak literali literal sayıyordu: `17'($unsigned(12'd852 -
        // 12'(p0)))`, doğal 12 bitte sarıp genişletiyordu. Değer işaretsiz
        // çıkarmanın sarmasına (A3) bağlı, elle sabitlenmez.
        // ── Açılmış fn'in tipsiz gövde `let`'i (#132) ──
        // İki örnek art arda (kimlikleri 33 ve 34): r biçiminde birinin
        // çağrısı modül düzeyi `for`'da (tel kipi), öbürününki `comb`'da
        // (ikame kipi, `hygiene::module_level`). C biçimi sabit yoldaki
        // `p0 + p1`'i çıkarır; r biçiminde o gövdenin `let s<id>r`'si olur.
        r(
            "#132",
            "untyped body let of an inlined function narrowed by a bitwise operation, in a wider sum (module level)",
            vec![UInt(8), UInt(8), UInt(8)],
            bin(
                BinOp::Add,
                bin(BinOp::Or, bin(BinOp::Add, p(0), p(1)), p(2)),
                p(2),
            ),
        )
        .extracting(&[0, 0])
        // `((p0 + p1) | p2) + p2`, üçü u8 (#132'nin örneği). type-inference
        // §3.3, ADR-0025: `p0 + p1` esnek u8..u9; `| p2` aralıkların
        // kesişiminde, 8 bitte, sonuç u8 (bit işleminde kesişimin üst ucu
        // sonucun doğal genişliğidir, A1'in iki okuması aynı); `+ p2` u8 + u8
        // → u9. 255, 255, 255: 510 → 8 bitte 254; 254 | 255 = 255; 255 + 255
        // = 510. 128, 128, 128: 256 → 0; 0 | 128 = 128; + 128 = 256.
        .pin(&[255, 255, 255], 510)
        .pin(&[128, 128, 128], 256),
        r(
            "#132",
            "untyped body let of an inlined function narrowed by a bitwise operation, in a wider sum (comb)",
            vec![UInt(8), UInt(8), UInt(8)],
            bin(
                BinOp::Add,
                bin(BinOp::Or, bin(BinOp::Add, p(0), p(1)), p(2)),
                p(2),
            ),
        )
        .extracting(&[0, 0])
        // Aynı ifade, çağrı `comb` içinde. Düzeltmeden önce ikame kipi
        // `let`'i doğal genişliğinde yazıyor, `|`'yu 9 bitte yapıyordu:
        // (510 | 255) + 255 = 766 → 9 bitte 254; (256 | 128) + 128 = 512 →
        // 0 (ve Verilator WIDTHEXPAND).
        .pin(&[255, 255, 255], 510)
        .pin(&[128, 128, 128], 256),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Model;
    use crate::value::mask;

    fn raw(v: i128, w: u32) -> u128 {
        (v as u128) & mask(w)
    }

    /// Elle hesaplanan değer modelinkiyle aynı: ikisinden biri yanlışsa
    /// burada görünür (koşucu Verilator'u ikisine de ayrı ayrı karşılaştırır).
    #[test]
    fn pins_agree_with_the_model() {
        for r in all() {
            let m = Model { ports: &r.ports };
            let nat = m
                .synth(&r.expr)
                .expect("geçerli")
                .ty
                .natural()
                .expect("doğal tip");
            for pin in &r.pins {
                let inp: Vec<u128> = pin
                    .inputs
                    .iter()
                    .zip(&r.ports)
                    .map(|(v, t)| {
                        let w = t.width();
                        let (lo, hi) = if t.signed() {
                            (-(1i128 << (w - 1)), (1i128 << (w - 1)) - 1)
                        } else {
                            (0, (1i128 << w) - 1)
                        };
                        assert!(
                            (lo..=hi).contains(v),
                            "{}: giriş {v} {t:?} tipine sığmaz",
                            r.what
                        );
                        raw(*v, w)
                    })
                    .collect();
                assert_eq!(
                    m.eval_check(&r.expr, nat, &inp),
                    Some(raw(pin.want, nat.width())),
                    "{} {}: {:?}",
                    r.issue,
                    r.what,
                    pin.inputs
                );
            }
        }
    }

    /// Her issue'nun en az bir örneği elle hesaplanmış değer taşır.
    #[test]
    fn every_issue_has_a_hand_computed_value() {
        let regs = all();
        let issues: std::collections::BTreeSet<&str> = regs.iter().map(|r| r.issue).collect();
        for i in issues {
            assert!(
                regs.iter().any(|r| r.issue == i && !r.pins.is_empty()),
                "{i}: elle hesaplanmış değer yok"
            );
        }
    }

    /// #132'nin iki örneği r biçiminde biri tel, biri ikame kipindedir:
    /// kimlikleri (listedeki sıra) farklı tekliktedir.
    #[test]
    fn issue_132_examples_cover_both_inlining_modes() {
        let ids: Vec<usize> = all()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.issue == "#132")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(ids.len(), 2, "{ids:?}");
        assert_ne!(
            crate::hygiene::module_level(ids[0]),
            crate::hygiene::module_level(ids[1]),
            "{ids:?}"
        );
    }

    /// #132'nin örneklerinde gövdenin `let`'i tohumdan bağımsız olarak
    /// `p0 + p1`'dir (sabit çıkarma yolu); rastgele seçim `| p2`'yi de
    /// seçebilirdi, o zaman hata görünmezdi.
    #[test]
    fn issue_132_examples_put_the_sum_in_the_body_let_for_every_seed() {
        for seed in [1, 20_261_104, 20_261_106] {
            for (i, r) in all().iter().enumerate().filter(|(_, r)| r.issue == "#132") {
                let mut rng = crate::rng::Rng::new(seed);
                let c = crate::harness::Case::new_extracting(
                    i,
                    r.expr.clone(),
                    &r.ports,
                    &mut rng,
                    r.extract.as_deref(),
                );
                let h = c.hygiene.as_ref().expect("r biçimi");
                assert!(
                    matches!(&h.body_let, Some(Expr::Bin(BinOp::Add, a, b))
                        if matches!(**a, Expr::Port(0)) && matches!(**b, Expr::Port(1))),
                    "tohum {seed}, {}: {:?}",
                    r.what,
                    h.body_let
                );
            }
        }
    }

    #[test]
    fn every_regression_is_valid_in_the_model() {
        for r in all() {
            let m = Model { ports: &r.ports };
            assert!(
                m.synth(&r.expr).is_ok(),
                "{}: {:?}",
                r.what,
                m.synth(&r.expr)
            );
        }
    }
}
