# ADR-0057: SV Üretiminde Operatör Önceliği — Parantez Kararı Hedef Dilin Tablosuyla Verilir

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-20
> Etkilenen: volt-sv-emit (`expr.rs`: `sv_prec` IEEE tablosuna geçti,
> `is_bitwise` / `is_comparison` / `paren_comparison_under_bitwise` YENİ;
> `tests/precedence_tests.rs` YENİ, 24 test), `.test-baseline`,
> CHANGELOG.md. RTL ve SVA aynı `emit_expr` yolunu kullandığından ikisi
> birlikte düzeldi (`sva.rs` değişmedi).
> Üretilen çıktısı değişen depo dosyası: yalnız
> `tests/ui/pass/04_operator_precedence.volt` (kaynak değişmedi).
> DOKUNULMADI: examples/, README.md, docs/spec/ (bkz. "Spec notu").
> İlgili: ADR-0013 §2.2 (bit düzeyi operatörler karşılaştırmadan sıkı),
> ADR-0012 (okunabilir SV), ADR-0034 (implikasyon açılımı).

## Sorun

RV32IM turunda (examples/riscv_core.volt) bir hizalama invariant'ı
yazılırken bulundu: **sessiz yanlış derleme**.

```volt
(mepc & 3) == 0
```
```systemverilog
mepc & 32'd3 == 32'd0        // üretilen — YANLIŞ
(mepc & 32'd3) == 32'd0      // olması gereken
```

SystemVerilog'da `==`, `&`'den sıkı bağlanır; üretilen metin
`mepc & (32'd3 == 32'd0)` = `mepc & 1'b0` okunur. Hiçbir tanı, hiçbir
lint uyarısı çıkmaz; Verilator `-Wall` de susar.

Anlam kayması sabite bağlıdır — tek bir "hep doğru / hep yanlış" kuralı yok:

| Volt | SV'nin okuduğu | Sonuç |
|---|---|---|
| `(x & M) == 0` | `x & (M == 0)` = `x & 0` | sabit **yanlış** |
| `(x & M) != 0` | `x & (M != 0)` = `x & 1` | `x[0]` |
| `(x & 1) == 1` | `x & (1 == 1)` = `x & 1` | tesadüfen doğru |
| `(x \| M) != 0` | `x \| 1` | sabit **doğru** |
| `(x ^ y) < d` | `x ^ (y < d)` | ilgisiz bir değer |

**Boş-doğru (vacuous) kanıt** üç yoldan doğar: (a) sabit-yanlış ifade
implikasyonun ÖNCÜLÜNDE ise (`(x & 3) == 0 -> p` → öncül hiç tutmaz,
property her zaman geçer); (b) `assume`/`requires` içinde ise (varsayım
sabit yanlış → tüm kanıtlar geçer); (c) `|` biçiminde sabit doğru ise.
RV32IM turundaki invariant bu sınıftandı.

Yosys ile araç kanıtı (`hdlc/formal`, `sat -prove`), `a : u8`:

```
y_ref = (t == 0), t = a & 8'hF
y_new = (a & 8'hF) == 8'd0   →  SAT proof finished - no model found: SUCCESS!
y_old = a & 8'hF == 8'd0     →  model found: FAIL!   (karşı örnek a = 0)
```

## Kök neden

`crates/volt-sv-emit/src/expr.rs` içindeki `sv_prec`, belge yorumunda da
yazdığı gibi "Volt tablosuyla uyumlu, operator-precedence.md §3'ten
türetildi". AST'de parantez düğümü yoktur; parantez üreticide, çocuk ile
ebeveynin önceliği karşılaştırılarak yeniden kurulur. Bu karşılaştırma
**metni okuyacak dilin** tablosuyla yapılmalıdır — kaynağın değil.

Volt tablosu kaynağı AYRIŞTIRMAK için doğrudur; aynı tabloyla
YAZDIRMAK, ancak iki dilin tabloları özdeşse doğrudur. Değiller.

## Kapsam tespiti (Adım 1)

Volt (docs/spec/operator-precedence.md §1) ↔ SV (IEEE 1800-2017 §11.3.2,
Tablo 11-2), gevşekten sıkıya:

| Düzey | Volt | SystemVerilog |
|---|---|---|
| 1 | `->` (sağ) | — (ifade karşılığı yok; `!a \|\| b` açılır, ADR-0034) |
| 2 | `\|\|` | `\|\|` |
| 3 | `&&` | `&&` |
| 4 | `==` `!=` | **`\|`** |
| 5 | `<` `>` `<=` `>=` | **`^`** |
| 6 | **`\|`** | **`&`** |
| 7 | **`^`** | `==` `!=` |
| 8 | **`&`** | `<` `>` `<=` `>=` |
| 9 | `<<` `>>` | `<<` `>>` (`>>>`) |
| 10 | `+` `-` | `+` `-` |
| 11 | `*` `/` `%` | `*` `/` `%` |
| 12 | tekli `!` `~` `-` | tekli |

Tek ayrışma: **{`&`, `^`, `|`} bloğu ile {`==` `!=`} + {`<` `>` `<=`
`>=`} bloğu yer değiştirmiş.** Blokların İÇ sırası iki dilde aynı
(`|` < `^` < `&`; eşitlik < ilişkisel).

- Etkilenen operatör çifti: 3 bit düzeyi × 6 karşılaştırma = **18 çift**
  (öncelik düzeyi olarak 3 × 2 = 6 düzey çifti).
- Yanlış derlenen ağaç biçimi: bit düzeyi işlem, karşılaştırmanın
  OPERANDI (solda ya da sağda): `(a & b) == c`, `a < (b | c)`.
- Ters biçim (`a & (b == c)`) eski tabloda tesadüfen doğru çıkıyordu:
  Volt tablosunda karşılaştırma bit düzeyinden gevşek olduğu için
  parantez basılıyordu.

Taranıp FARK BULUNMAYAN yerler:

- `<<` / `+` / `*`: aynı sıra (`a << 2 + 1` iki dilde de `a << (2 + 1)`).
- `||` / `&&`: aynı sıra.
- Birleşme yönü: SV'de tüm ikili operatörler sol birleşmeli. Volt'un
  birleşmesiz karşılaştırmaları (E0010) ağaçta yalnız açık parantezle
  iç içe geçer; sağ operand eşit öncelikte parantezlenir, sol operand
  SV'nin sol birleşmesiyle zaten aynı ağaçtır.
- `->`: sağ birleşmeli ama SV'ye `!a || b` olarak açılır; sol operand
  tekli düzeyde, sağ operand `||` düzeyinde sağ-operand kuralıyla basılır.
- Tekli operatörler, ternary (koşul ve kollar tekli düzeyde basılır),
  cast / boyut dönüşümü / `$signed` (operand atom düzeyinde basılır ya da
  kendi parantezini taşır), kaydırma miktarı (her zaman parantezli).

Bu liste elle tarama değildir: `every_operator_pair_round_trips_through_
ieee_precedence` testi 19 × 19 operatör çiftini iki ağaç biçiminde (722
ifade) üretir, çıkan metni testin İÇİNDEKİ bağımsız bir IEEE öncelik
ayrıştırıcısıyla geri okur ve ağacı karşılaştırır. Test düzeltmeden
ÖNCEKİ `expr.rs` ile koşturuldu: tam 36 ifade başarısız = yukarıdaki 18
çift × (sol operand, sağ operand); başka hiçbir çift başarısız değil.

## Karar

### 1. `sv_prec` = IEEE 1800-2017 Tablo 11-2

```
|| 1 · && 2 · | 3 · ^ 4 · & 5 · == != 6 · < > <= >= 7 · << >> 8 · + - 9 · * / % 10
```

`Imp` `||` düzeyinde kalır (açılımının düzeyi). Genel kural (çocuk <
ebeveyn → parantez; eşitse sağ operand → parantez) değişmedi — yalnız
tablo değişti. Böylece `(a & b) == 0` parantezini korur.

### 2. Ayrışan düzeyde parantez HER İKİ yönde basılır

IEEE tablosu `a & (b == 0)` için parantez istemez; `a & b == 0` SV'de
doğru okunur. Yine de basılır (`paren_comparison_under_bitwise`):

- `a & b == 0` metni, Volt bilen okura `(a & b) == 0`, SV bilen okura
  `a & (b == 0)` der. Çıktıyı inceleyen kişi Volt kaynağından geliyor;
  iki dilin ayrıştığı TEK düzeyde metin her iki tabloyla da aynı ağaca
  ayrışmalı.
- Önceki çıktı da bu biçimi parantezli basıyordu → bu yönde çıktı
  değişmez, snapshot kayması olmaz.

### 3. Reddedilen: "karışık öncelikli her alt ifadeye parantez"

Basit ve kesin doğru, ama ADR-0012/ADR-0008 ile çelişir: `a + b * c`,
`a == b && c < d`, `a << 2 | b >> 1` gibi iki dilde özdeş okunan her
ifade parantezle dolar, `counter.expected.sv` dahil depodaki 156 SV
dosyasının büyük bölümü değişir ve çıktı "elle yazılmış gibi" okunmaz
olur. Doğruluk kazancı da yoktur: hata tablonun YANLIŞ olmasıydı,
parantezin az olması değil. Doğru tablo + gidiş-dönüş testi aynı
güvenceyi çıktıyı bozmadan verir. Kabul edilen çözüm bu alternatifin
yalnız gerekli dilimini alır (Karar 2: ayrışan düzey).

## Mevcut çıktılara etkisi (Adım 4)

Yöntem: düzeltmeden önceki ve sonraki derleyiciyle depodaki hata-testi
olmayan 105 `.volt` dosyası `--emit=sva --sva inline` ile derlendi (99'u
derlenir; kalan 6'sı bilinçli hata/ertelenmiş senaryo: multifile
cyclic/notfound/pubpriv, 16_trit, 62_extern), 156 `.sv` dosyası
`diff -r` ile karşılaştırıldı.

- `tests/fixtures/counter.expected.sv`: **değişmedi** (kalıp yok).
- `examples/`: **hiçbir çıktı değişmedi.** riscv_core.volt hatayı
  bulduktan sonra `!mepc[0] && !mepc[1]` biçiminde yazılmıştı.
- Değişen tek dosya: `tests/ui/pass/04_operator_precedence.volt` →
  `assign r2 = a & 8'hF == 8'd0;` → `assign r2 = (a & 8'hF) == 8'd0;`.
  Bu, operator-precedence.md §2.2'nin bayrak örneğidir ve F0'dan beri
  yanlış derleniyordu.

## Formal etkisi (Adım 5)

`sva.rs` kontrat ifadelerini aynı `emit_expr` ile basar (satır 190, 221,
222, 239, 240, 245) → hata SVA'da da VARDI, aynı düzeltmeyle kapandı
(`sva_*` testleri).

Boş-doğru tarama: yukarıdaki karşılaştırma inline SVA'yı içerir — 74
dosyada 476 assert/assume/cover metni. **Hiçbiri değişmedi → depoda
kanıtlanmış property'lerden hiçbiri bu hatadan etkilenmiyor; boş-doğru
property bulunmadı.** Önceki ADR'lerdeki formal sonuçlar (ADR-0040,
0049, 0051, 0055 ölçümleri) geçerliliğini korur.

Depo DIŞINDAKİ kullanıcı kodu için: kontratında `&` `|` `^` ile
karşılaştırmayı aynı ifadede kullanan her tasarım yeniden doğrulanmalı
(CHANGELOG'da duyuruldu).

## Neden fark edilmedi

1. **Test yalnız "temiz SV üretiyor mu"ya bakıyordu.**
   `04_operator_precedence.volt` `ui_pass_sweep` içinde `must_emit`
   listesindeydi ama çıktının METNİ hiç iddia edilmedi. Tek öncelik
   emit testi (`precedence_parens_preserved_in_output`) `(a + b) * c` idi
   — iki dilde aynı düzey.
2. **Parser testleri doğruydu ve güven verdi.** `a & MASK == 0` →
   `(== (& a MASK) 0)` vektörü geçiyordu; hata ayrıştırmada değil,
   yazdırmadaydı ve o yarının gidiş-dönüş testi yoktu.
3. **Araçlar susar.** İfade SV'de geçerli ve genişlik-temiz; Verilator
   `-Wall`, Yosys, sby uyarı vermez.
4. **Örnekler kalıbı nadiren kullanıyor.** Donanım kodunda maske çoğunlukla
   bit/aralık seçimiyle (`x[1:0] == 0`) yazılıyor; 105 dosyada tek örnek.
5. **Formal bunu yakalayamaz, tersine gizler:** yanlış derlenen ifade
   kontratın kendisindeyse property ya boş-doğru geçer ya da gerçek bir
   hata gibi görünür ve yazar ifadeyi (RV32IM'deki gibi) sessizce başka
   biçimde yazar.

Alınan ders, kalıcı önlem: **üreticinin tablosu, ondan bağımsız yazılmış
bir hedef-dil ayrıştırıcısıyla her operatör çifti için sınanır.** Yeni
bir ikili operatör eklendiğinde test dosyasındaki `OPS` ve `ieee_prec`
güncellenmeden test derlenmez/geçmez.

## Spec notu

`docs/spec/` salt okunurdur ve bu turun kapsamı dışındadır.
sv-mapping.md §6 operatörleri "doğrudan" eşler ama parantez kuralını hiç
söylemez; bir sonraki spec turunda §6'ya şu not eklenmelidir: "Parantez
kararı IEEE 1800-2017 Tablo 11-2 ile verilir; bit düzeyi ↔ karşılaştırma
karışımı her iki yönde parantezlenir (ADR-0057)."

## Ölçütler

- `cargo test --all` yeşil; volt-sv-emit'e +24 test (`precedence_tests.rs`).
- `(a & b) == 0` → `(a & b) == 8'd0`; `a & (b == 0)` → `a & (b == 8'd0)`;
  `(a | b) != c`, `(a ^ b) < d` parantezli; `a << 2 + 1` → `a << (3)`
  (değişmedi).
- 722 ifadelik gidiş-dönüş testi yeşil.
- Yosys `sat -prove`: yeni çıktı referansla eşdeğer, eski çıktı değil.
- Verilator `--lint-only -Wall` yeni `Precedence.sv` üzerinde temiz.
- `just check`, `just consistency`, `just clippy-strict` yeşil.
