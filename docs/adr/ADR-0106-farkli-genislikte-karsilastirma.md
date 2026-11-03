# ADR-0106: Farklı Genişlikte Aritmetik Karşılaştırma Hatadır (E2031); Bileşik Değerden Seçim Boyut Dönüşümüne İner

> Statü: Uygulandı
> Önceki karar: ADR-0025 — Karar'daki "Operand uyumu aralık KESİŞİMİYLE kurulur" kuralının karşılaştırma işleçlerindeki uygulanışı (kesişimin alt ucunda, taşma bitini atarak karşılaştırma) ve ekin (2026-10-01) zincir kuralının karşılaştırmayı dışarıda bırakması
> İlgili: ADR-0041 (aynı işaretli genişleme ve açık boyut dönüşümü: karşılaştırma bağlamı aynı mekanizmayla basılır), ADR-0099 (iki öneri gidiş-dönüş denetiminden geçer)
> Tarih: 2026-11-03
> Etkilenen: volt-hir (`typeck/compare_width.rs` YENİ, `typeck/binop.rs`, `typeck/diag.rs`),
> volt-sv-emit (`expr.rs`, `let_width.rs`), volt-syntax (`parser::expr_text`,
> `auto_contract/print.rs`), volt-diagnostics (E2031, açıklama en/tr),
> `examples/riscv_core.volt`, `examples/i2c/i2c_master.volt`,
> `templates/mmio/blinker.volt`, `tests/ui/{pass,fail}`, `tests/suggestions/`,
> CHANGELOG, `docs/release-notes/v0.1.0.md`

## Sorun

#106. `a`, `b : u3` iken:

```volt
let s = b + 1
eq  = a == b + 1     // SV: a == b + 3'd1        → 3 bitte sarar
eq2 = a == s         // SV: wire [3:0] s; a == s → 4 bitte, taşma kalır
```

`a = 0`, `b = 7` için `eq = 1`, `eq2 = 0`: ifadeyi tipsiz `let`'e çıkarmak
sonucu değiştiriyordu. `volt check` ikisini de uyarısız kabul ediyordu;
`volt test` ikinci biçimde Verilator `WIDTHEXPAND` ile duruyordu; üretilen
`// carry bit of 's' unused` yorumu, taşma biti `eq2`'de okunduğu hâlde
basılıyordu.

Aynı incelemede üç sessiz durum daha ölçüldü (main e33959d2, `c, d : u8`):

| Volt | Üretilen SV | Sorun |
|---|---|---|
| `c + d == 5` | `c + d == 8'd5` | 8 bitte sarar: 250 + 11 = 261 için doğru |
| `c + d < 300` | `c + d < 8'd300` | literal 8 bite kesilir (44) |
| `c + d == c + r` | `c + d == c + r` | iki toplam da 8 bitte sarar |

Karşılaştırmanın operand bağlamı sol yanın kaba genişliğiydi
(`width_of(lhs)`), tip denetiminin gösterdiği doğal genişlik değil.

Ayrıca E2001'in kendi önerisi `(a + 1)[3:0] as u4` SV'ye `(a + 8'd1)[3:0]`
olarak iniyordu; SystemVerilog parantezli bir ifadeyi seçemez, Verilator
5.052 "syntax error, unexpected '['" ile reddeder. Bu ADR'nin sarma önerisi
aynı biçimi kullandığı için düzeltme buraya alındı.

## Spesifikasyonla ilişki

`docs/spec/type-inference.md` §3.3 (BAĞLAYICI, salt okunur):

```rust
                // Toplama/çıkarma: 1 bit taşma
                BinOp::Add | BinOp::Sub => a + 1,
...
#### Karşılaştırma: Her Zaman Bool
    self.unify_for_comparison(lt, rt, span);   // aynı tip olmalı
```

Harfiyen okununca `a == b + 1` (u3 ile u4) reddedilir. ADR-0025 aritmetik
sonuca `[işlem, doğal]` aralığı verip operandları kesişimle buluşturdu;
karşılaştırmada kesişimin alt ucu seçildi ve taşma biti sessizce atıldı.
Bu ADR karşılaştırma için spesifikasyonun "aynı tip olmalı" kuralına geri
döner: doğal genişlikleri farklı iki değişken karşılaştırılamaz. Tek
sapma sabit istisnasıdır (§1): spesifikasyon literali karşı tarafa uyarlar
(§3.1), bu ADR aynı uyarlamayı derleme anında bilinen sabit ifadelere de
uygular. Spesifikasyon metni değişmez; çelişki burada kayıtlıdır.

`as` daraltması §3.6'da tanımlıdır:

```rust
        // Daraltma — izinli ama uyarı
        (UInt { width: a }, UInt { width: b }) if b < a => {
            self.warn(W2010, span,
                format!("{} bit → {} bit daraltma, üst bitler kesilir", a, b));
            true
        }
```

Yani `(b + 1) as u3` açıkça yazılmış bir kesmedir ama W2010 verir. Önerinin
uyarısız biçimi bu yüzden `(b + 1)[2:0] as u3`'tür: seçim `bits<3>` verir,
aynı genişlikte `bits<3>` → `u3` dönüşümü uyarısızdır (§3.6).

## Ölçüm (uygulamadan önce)

Kural uygulanıp hiçbir dosya düzeltilmeden `volt check --format=json`:

| Külliyat | Dosya | E2031 |
|---|---|---|
| tests/ui/pass | 118 | 0 |
| tests/ui/fail | 207 | 0 |
| tests/fixtures, suggestions, quickfix, fuzz_regressions | 307 | 0 |
| examples | 34 | 6 (3 yer; `_test.volt` kardeşleri aynı satırları yeniden denetler) |
| templates | 8 | 1 |
| demo | 2 | 0 |
| kitap (main, check_book) | 14 blok | 0 |
| kitap (PR #105 dalı) | 30 blok | 1 (`ensures: ... -> level == prev(level) + 1`) |

Hata veren 4 benzersiz yer: `examples/i2c/i2c_master.volt:134`
(`clk_count_r >= clks_per_phase - 1`), `examples/riscv_core.volt:148`
(`mcycle == prev(mcycle) + 1`, kontrat), `:169`
(`div_q == (1u32 << div_cnt) - 1`, kontrat), `templates/mmio/blinker.volt:50`
(`timer_r + 1 >= regs.period.cycles`). Dördünde de erişilebilir durumlarda
taşma olmadığından iki okuma aynı sonucu verir; hepsi bugünkü anlamı koruyan
sarma biçimine geçti.

Sabit istisnası olmasaydı sayaç sınırı deseni (`r == LIMIT - 1`,
`tests/ui/pass/37`, `/90`, `riscv_core` üç yer ve ADR-0066'nın ürettiği
`count_r <= LIMIT - 1` sınır invariant'ları) de hata verirdi.

Emitter değişikliğinden sonra (385 dosya, `volt build --emit=sva`, main
ikilisiyle karşılaştırma) SV ya da SVA metni değişen dosya yalnız yukarıdaki
4 dosyadır; değerleri aynıdır.

## Seçenekler (#106)

**A — Karşılaştırmada da doğal genişlik.** `a == b + 1` 4 bitte. Sarmaya
dayanan karşılaştırmalar sessizce değişir; çıkarmada `x >= LIMIT - 1`
(LIMIT = 0) hiç doğru olmaz.

**B — `let`'te de işlem genişliği.** `a == s` → `a == 3'(s)`. Tipsiz
`let`'in "taşma bitini tutar" kuralına karşılaştırmada istisna doğar.

**C — Farklı doğal genişlik hatadır (SEÇİLDİ, bakıcı kararı).** Volt
sarma ile genişletme arasında seçim yapmaz; yazan genişliği yazar.

## Karar

### 1. E2031

`==`, `!=`, `<`, `<=`, `>`, `>=` işleçlerinde iki yan tam sayıysa, aralıkları
kesişiyorsa (kesişmiyorsa eski E2003) ve doğal genişlikleri (aralığın üst
ucu) farklıysa E2031. Fark yalnız aritmetik genişlemeden gelebilir: somut
tiplerin aralığı tek noktadır ve genişlikleri farklıysa kesişmez. Kural
modülde, `on`/`comb` bloğunda, `fn` gövdesinde ve kontratlarda aynıdır;
tipsiz `let` esnek aralığını taşıdığı için `let s = b + 1; a == s` satır
içi biçimle aynı hatayı verir.

Sabit istisnası (hata yok):

- geniş yan derleme anında değerlendirilebiliyor ve değeri dar yanın
  tipine sığıyor (`r == LIMIT - 1`): iki okuma aynıdır;
- dar yan derleme anında değerlendirilebiliyor (`c + d == LIMIT`,
  literaller zaten uyarlanır): değer geniş yanın doğal genişliğine sığar,
  karşılaştırma o genişlikte yapılır (§2).

Geniş yan değeri dar yana sığmayan bir sabitse (`r == ZERO - 1`) E2031 bir
notla verilir: karşılaştırma her çevrimde aynı sonucu verir. Öneri yok;
yardım satırı sabitin denetlenmesini ister. Ayrı bir "her zaman
doğru/yanlış" uyarısı (W-kodu) bu ADR'nin kapsamı dışında; böyle bir
karşılaştırmanın bugünkü bütün biçimleri (literal sınır taşması E2010, bu
not) zaten hata olduğundan gerek görülmedi.

### 2. Kabul edilen karşılaştırmanın genişliği

Emitter karşılaştırma operandlarını tek bağlamla basar:

| Durum | Genişlik |
|---|---|
| iki yanın doğal genişliği eşit | o genişlik (`c + d == c + r` → 9 bit) |
| geniş yan sabit ve dar yana sığıyor | dar yan (önceki çıktıyla aynı: `r == 8'd6 - 8'd1`) |
| dar yan sabit ya da literal | geniş yanın doğal genişliği (`9'(c) + 9'(d) == 9'd5`) |

Dar atom operandlar ADR-0041'deki gibi `W'(x)` ile açıkça genişletilir.

### 3. Öneriler: her yerde `as`

Yardım satırı ve iki makineyle uygulanabilir öneri, modülde, blokta ve
kontratta aynı yazımı verir (bakıcı yönergesi: okur tek kural öğrenir):

```text
= help: write the width you mean with 'as':
    wrap at 3 bits:  a == (b + 1)[2:0] as u3
    keep the carry:  (a as u4) == b + 1
```

Birinci öneri sarar (bugünkü SV anlamı), ikincisi dar yanı genişletir.
Tipli `let` (`let wrapped : u3 = b + 1`) modülde de çalışır ama kontratta
çalışmaz (kontrat `let` okuyamaz, `prev()` `let`e girmez); bu yüzden öneri
değil, `volt explain E2031` notunda anılır. Öneri metni
`volt_syntax::parser::expr_text` ile kaynaktan yazılır (ad, sayı, işlem,
çağrı, alan, indeks); otomatik kontrat metni alan/indeks yazmayan eski
yazıcıyı kullanmaya devam eder.

### 4. Bileşik değerden seçim

Taban seçilebilir bir ad değilse (ikili/tekli işlem, `if`, `match`, dönüşüm,
çağrı, katlanan sabit) bit ve aralık seçimi boyut dönüşümüne iner; değer
doğal genişliğinde hesaplanır:

| Volt | SV |
|---|---|
| `(a + 1)[3:0]` | `4'(9'(a) + 9'd1)` |
| `(a + b)[8]` | `1'((9'(a) + 9'(b)) >> 8)` |
| `(a + b)[8:5]` | `4'((9'(a) + 9'(b)) >> 5)` |
| `(x + 1)[3:0]` (x: i8) | `$unsigned(4'(9'(x) + 9'sd1))` |

Sinyal, dizi elemanı, alan ve seçim tabanları değişmez (`a[3:0]`). Indexed
part-select (`(ifade)[i +: 4]`) bu ADR'de değişmedi.

### 5. "carry bit unused" yorumu

Karşılaştırma bağlamı doğal genişlik olduğundan tipsiz `let`'in okunan
genişliği doğru kaydedilir: taşma biti karşılaştırmada okunuyorsa yorum ve
`UNUSEDSIGNAL` susturması basılmaz.

## Davranış değişiklikleri

- Farklı doğal genişlikte aritmetik karşılaştırma E2031 (önce sessizce
  operand genişliğinde sararak derleniyordu).
- Kabul edilen karşılaştırmalar doğal genişlikte: `c + d == 5`,
  `c + d < 300`, `c + d == c + r` artık taşma bitini tutar.
- Bileşik değerden seçim geçerli SV üretir.
- Örnekler ve `volt new --template mmio` şablonu sarma biçimine geçti
  (anlam aynı).

## Testler

Önce düşen: `typeck::compare_width` birim testlerinden 5'i (kural kapalıyken),
`compare_width_emit_tests`'ten 5'i (main emitter'ı ile), `ui/fail/220-223`.
Değişen: `binop_tests::comparison_of_flex_result_with_same_width_ok` eski
sessiz sarmayı "geçerli" sayıyordu; adı ve beklentisi E2031 olarak
değişti, `as` biçimleri eklendi. Yeni: `ui/pass/133`, `tests/suggestions/
e2031_{inline,let,contract}` (iki öneri ayrı ayrı uygulanır, tanı gider, yeni
tanı çıkmaz, `volt build` geçer), `compare_width_tests` (öneriler JSON'dan
uygulanır; Verilator'da b = 7, a = 0 için sarma eşit, taşmayı tutma eşit
değil).
