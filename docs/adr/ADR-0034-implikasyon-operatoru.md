# ADR-0034: İmplikasyon Operatörü `->`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-09
> Etkilenen: grammar-full.ebnf §13, operator-precedence.md §1/§3/§4,
> volt-ast (BinOp::Imp), volt-syntax (Pratt tablosu), volt-hir
> (typeck/consteval), volt-sv-emit (RTL açılımı + SVA `|->`)
> Uygulama aşaması: F4a raporu bulgusu

## Sorun

F4a raporu: "Volt gramerinde `->` operatörü yok; `ensures` için üst
düzey `!a || b` deseni `a |-> b`'ye çevriliyor." Kontratların en
yaygın biçimi "A ise B" implikasyonudur; kullanıcı bunu doğal
biçimde (`a -> b`) yazamıyor, De Morgan çevirisini elle yapıyordu.
Desen tabanlı `|->` üretimi de yalnız `ensures`'te ve yalnız tam
`!a || b` şeklinde çalışıyordu.

## Seçenekler

**Seçenek A — Desen tanımayı genişletmek**
`!a || b` benzeri biçimleri tüm kontrat türlerinde tanı. Kullanıcı
hâlâ implikasyonu değilleme + veya ile kodlamak zorunda; niyet
kaynakta görünmez. Elenmiştir.

**Seçenek B — `->` yalnız kontrat bağlamında**
Kontrat ifadelerine özel mini gramer. İki ayrı ifade dili doğurur;
`let ok = a -> b` gibi RTL kullanımına izin vermez. Elenmiştir.

**Seçenek C — `->` genel ifade operatörü (SEÇİLDİ)**
`ImplExpr = OrExpr [ "->" ImplExpr ]` — 0. öncelik seviyesi, sağ
birleşmeli, her ifade konumunda geçerli. Kontratta SVA `|->`,
RTL'de `!a || b` açılımı.

## Karar

- **Gramer:** `Expr = ImplExpr`, `ImplExpr = OrExpr [ "->" ImplExpr ]`.
  Öncelik 0 (en düşük, `||` altında), SAĞ birleşmeli:
  `a -> b -> c ≡ a -> (b -> c)`.
- **`fn` dönüş tipiyle çakışma yoktur:** `->` iki ayrı bağlamda
  yaşar ve parser bağlam güdümlüdür. `fn` imzasındaki `->`, kapanış
  `)`'dan hemen sonra ÖĞE parser'ında tüketilir; ifade parser'ı o
  konumda hiç çalışmaz. İfade döngüsüne ulaşan her `->` implikasyondur.
  Karar LL(2) içinde verilir: `)` görüldükten sonra tek token ileri
  bakış (`->` mu, `{`/kontrat anahtar kelimesi mi) yeterlidir;
  geri alma (backtracking) gerekmez.
- **Pratt bağlama güçleri:** `Imp => (1, 0)`. Sol güç `||` ile eşit
  (1) olsa da çakışma doğurmaz: implikasyonun sağ operandı
  `min_bp = 0` ile ayrıştırıldığından `||` ve üzeri her operatör
  implikasyondan sıkı bağlanır; `r_bp = 0 < l_bp = 1` sağ birleşmeyi
  verir. `a || b -> c → (a || b) -> c`.
- **Semantik:** `a -> b ≡ !a || b`. İki operand da `Bool` olmalı,
  değilse E2003; sonuç `Bool`. Sabit değerlendirmede aynı eşdeğerlik.
- **Kontrat gövdesi struct literal içermez:** kontrat koşulu `if`
  başlığı kuralıyla (`no_struct_lit`) ayrıştırılır; aksi hâlde
  `fn f() -> bool requires: a -> b { b }` içindeki `b {`, fn gövdesini
  yutan bir yapı literali sanılırdı.
- **SVA üretimi:** kontratın üst düzey `a -> b` ifadesi HER kontrat
  türünde örtüşmeli gerektirmeye (`a |-> b`) çevrilir. İç içe
  implikasyonlar boolean açılımıyla yazılır (yalnız üst düzey `|->`).
  `ensures`'ün eski `!a || b` deseni geriye uyumluluk için `|->`
  üretmeye devam eder. Yosys-uyumlu immediate modda `|->` yoktur;
  `!a || b` açılımı kullanılır (tek döngüde eşdeğer).
- **RTL üretimi:** SV'nin ifade düzeyinde `->` operatörü yok
  (yalnız kısıt bloklarında); `a -> b` çıktıda `!a || b` olur,
  ikili sol operand parantezlenir: `!(a && b) || c`.

## Sonuçlar

- `invariant: !busy -> tx` doğal biçimde yazılır ve `!busy |-> tx`
  üretir; De Morgan elle çevirisi gerekmez.
- `examples/uart_tx.volt` kontratları `->` ile sadeleşti; kontratlar
  Docker (sby) akışında kanıtlanmaya devam eder.
- operator-precedence.md §1 tablosuna 0. seviye, §3 Pratt tablosuna
  `Imp => (1, 0)`, §4'e `a -> b -> c` ve `a || b -> c` vektörleri
  eklendi (EN + tr).
- `tests/ui/pass/40_implication_operator.volt` uçtan uca kullanımı,
  `tests/ui/fail/29_implication_not_bool.volt` E2003'ü sabitler.

## Emsal

SVA `|->` (örtüşmeli gerektirme), VHDL-2019/PSL `->`, Dafny/Why3 gibi
kanıt dillerinde `==>` — kontrat dillerinde implikasyon birinci sınıf
operatördür ve hepsinde en düşük öncelikli + sağ birleşmelidir.
