# ADR-0085: Desendeki Çıplak Ad Bir Değerdir — Bağlama Deseni Yok

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-27
> Etkilenen: volt-syntax (`parser/pattern.rs`, `mono/pattern.rs`,
> `auto_contract/scan.rs`), volt-ast (`PatternKind::Binding` kaldırıldı,
> `match_cover`), volt-hir (`resolve/pattern.rs` yeni, `typeck/matching.rs`,
> `consteval.rs`, `typeck/gated.rs`, `sim.rs`), volt-sv-emit (`match_arm_label`,
> `inline/`), volt-diagnostics (E1015); spec: grammar-full.ebnf §12,
> ast-nodes.md §8, name-resolution.md §5.1 (yeni) ve §6
> Kaynak: ADR-0084 §5 bulgu 2 (şablon yazarken)

## Sorun

```volt
const LIMIT : u8 = 10
match x { LIMIT => a, _ => b }
```

Rust'ta ve Volt'un ADR-0083'e kadarki ayrıştırıcısında tek parçalı ad
desende **bağlamadır**: `LIMIT` sabitle karşılaştırılmaz, her değeri
yakalayan yeni bir değişken olur, `_` kolu hiç çalışmaz. Tek işaret
W1002 ("shadows").

### Ölçüm — bugün sessiz mi?

Görev metni "tek uyarı W1002, sessizce yanlış donanım" diyordu. Sonda
(`build/c1/gen.py`, 34 kaynak, main 9fb467e) bunu DÜZELTTİ: bugün bağlama
deseni sv-emit'te **E0003** ("binding, path and tuple patterns in 'match'")
alır; `check` de aynısını verir (ADR-0070 ortak boru hattı). Donanım
üretilmiyor — sessiz değil. Ama tuzak yerinde duruyor:

- Tanı yanlış şeyi anlatıyor: kullanıcı bir sabitle karşılaştırdığını
  sanıyor, ileti "bağlama deseni desteklenmiyor" diyor.
- ADR-0083 "Gelecek iş"te bağlama deseni var; eklendiği gün `LIMIT`,
  yazım hatası `LIMT` ve port adı `lim` **sessizce** her şeyi yakalayan
  kola dönüşürdü (Rust'ın bilinen tuzağı; rustc bunu yalnız büyük harf
  lint'iyle yakalar).
- Çıplak varyant (`Idle`) aynı yoldan geçiyordu (ADR-0074: E0003 +
  `State::Idle` önerisi, sv-emit'te).

Sınıf taramasında (aşağıda) gerçekten SESSİZ iki durum bulundu:
`u8` sınananda `300 =>` deseni SV'ye `8'd300` (= 44) olarak iniyordu
(tanı yok) ve test dilinde `let LIMIT = 3` tasarım sabitini uyarısız
gölgeliyordu.

## Karar

**C, genişletilmiş: desendeki çıplak ad HER ZAMAN bir değerdir.** Volt'ta
bağlama deseni yoktur.

| Desendeki ad çözülürse... | Sonuç |
|---|---|
| `const`, generic `const` parametresi | değer deseni — `case` etiketi sabitin değeri |
| port, register, wire, `let`, döngü değişkeni, örnek, fn, tip, alan | **E1015** "'lim' in a pattern must be a constant, but it is a port" + "Volt has no binding patterns" notu |
| hiçbir şeye (tanımsız) | **E1001**; ad bir enum varyantıysa yardım + fix-it `State::Idle`, değilse en yakın ad (`LIMT` → `LIMIT`) ya da "use '_'" |

Gerekçe:

1. **Bağlama zaten yoktu** (E0003) — hiçbir çalışan program anlam
   değiştirmez; golden farkı yalnız bağlama deseni yazan hata fixture'ları.
2. **A (yalnız çözülmeyen ad bağlama) tuzağı erteler**: bağlama deseni
   eklendiği gün yazım hatası yine her şeyi yakalar. B (gölgeleyen
   bağlama hata) tek başına `LIMIT`'i düzeltmez. Bu karar üçünü birden
   kapatır: sabit → değer (A'nın doğru yarısı), sinyal → hata (B), tanımsız
   → hata (A'nın tehlikeli yarısı yok).
3. **Donanım dillerinin anlamı budur**: SV `case` etiketi ve VHDL `when`
   seçimi ifadedir; bağlama kavramı yazılım dillerinden gelir.
4. **Çıplak varyant tutarlı**: varyantlar kök kapsama bağlanmaz
   (ADR-0074), yani `Idle` tanımsız addır → E1001 + `State::Idle` fix-it.
   ADR-0074'ün sv-emit'teki E0003'ü ad çözümlemeye taşındı (tek yer).
5. İleride bağlama gerekirse açık sözdizimiyle gelir (`x @ _`), çıplak
   adla değil.

### Değer deseninin tiplenmesi — `x == P` gibi

Desen `P`, `x == P` karşılaştırmasıyla aynı kuralı izler (sınıf
taramasının sessiz bulgusu):

- literal sınananın tipine sığmalı: `u8` üzerinde `300` → **E2010**
  (önceden tanısız `8'd300` = 44);
- `const` adının **değeri** sığmalı (bildirilen genişlik değil — `case`
  değeri karşılaştırır): `const SEVEN : u16 = 7` `u8` sınananda geçer,
  `BIG : u16 = 300` → E2010 "constant 'BIG' = 300 does not fit in type u8
  of the matched value"; desende cast yazılamadığı için E2001'in "as u8"
  önerisi yanlış olurdu;
- enum tipli `const` sayısal sınananda E2003 (yol deseniyle aynı ileti);
  aynı enum sınananda değerinin varyantını kapsar (kapsayıcılık,
  ADR-0074 Karar 4);
- yinelenen değer (`10 => ..., LIMIT => ...`) **W2014** — `match_cover`
  kuralı katmanın kendi sabit değerlendiricisini alır (HIR consteval,
  sv-emit `eval_const`); iki katman aynı kolu erişilemez sayar;
- SV etiketi katlanmış değerdir, **sınananın genişliğinde** (`8'd7`,
  `16'd7` değil — Verilator `WIDTH`); ad SV'ye inmez.
- Otomatik FSM kontratı (ADR-0066) sabit adlı kolu değeriyle tanır,
  kontrat metnine adı kopyalar (`prev(s) == IDLE && s == 1`).

## Sınıf taraması (ADIM 1.2) — "ad sessizce başka şeye dönüşüyor mu?"

Sonda: `build/c1/gen.py` (+ test dili `build/c1/t/`), önce = main,
sonra = bu ADR.

| Yer | Önce | Sonra | Sessiz miydi? |
|---|---|---|---|
| desende `const` (`LIMIT =>`) — deyim, ifade, fn gövdesi, `comb` | W1002 + E0003 | değer deseni, temiz | hayır (E0003) — tuzak kapandı |
| desende `const` — const bağlamında (`const K = match 3 { LIMIT => ..}`) | W1002 + E0003 (katlanamaz) | değer, katlanır | hayır |
| desende çıplak varyant `Idle` | E0003 + fix-it (sv-emit) | E1001 + fix-it `State::Idle` (ad çözümleme) | hayır |
| desende port / register adı | W1002 + E0003 | E1015 | hayır |
| desende taze ad `n => n` | E0003 | tek E1001 (gövdedeki `n` kaskad üretmez) | hayır |
| desende döngü değişkeni | E0003 | E1015 | hayır |
| desende generic `const N` | E0003 | değer (`8'd4`) | hayır |
| desende enum tipli `const` | E0003 | aynı enum'da varyantı; sayıda E2003 | hayır |
| **desende sığmayan literal (`300` on `u8`)** | **tanı yok, SV `8'd300` = 44** | E2010 | **EVET — kapatıldı** |
| `for I` döngü değişkeni `const I` adıyla | W1002, döngü değişkeni kazanır | aynı | hayır (uyarı, niyet döngü) |
| `for x` döngü değişkeni port adıyla | W1002 | aynı | hayır |
| fn parametresi `const` adıyla (`fn f(LIMIT: u8)`) | W1002 | aynı | hayır |
| fn parametresi port adıyla | tanı yok — fn modül dışında, port kapsamda değil | aynı | hayır (gölgeleme yok) |
| blok `let` porta / `const`'a gölge (ADR-0083) | W1002; SV yerel `logic [7:0] LIMIT` süreç içinde, modül sabiti `8'd10` olarak katlanır — ad yakalama yok | aynı | hayır |
| fn gövdesinde `let v = v + 1` | W1002; açılımda `f_0_v` (ADR-0083 a_2 yeniden adlandırma) | aynı | hayır |
| **test dilinde `let LIMIT` tasarım sabitini gölgeler** | **tanı yok** | W1002 (tasarım dilindeki gibi); yalnız kardeşli kipte (`volt test`, sabitler bilinir) | **EVET — kapatıldı** |
| test dilinde aynı adla ikinci `let` | E8506 | aynı | hayır |

## Mevcut kod taraması (ADIM 1.3)

Golden (`build/c5/golden.py`, check insan+JSON, build SV+SVA+SDC):
2045 kaynak (templates, examples, tests/ui, tests/fixtures,
tests/fuzz_regressions, derleyici test külliyatı). **examples/, templates/,
tests/ui/ çıktısı bayt bayt aynı** — tuzağa düşen kod yok (düşseydi E0003
alırdı). Değişen 9 dosyanın hepsi bağlama desenini kasıtlı yazan hata
fixture'ı: 6 parser test külliyatı kaynağı (`build/m2/corpus`), 3 parite
fixture'ı (`e07` çıplak varyant → E1001, `p29c` bağlama → E1001, `p29d`
tuple deseni literal `(0, 1)`'e çevrildi ki E0003 sınıfını korusun).

## Tanılar

| Kod | Durum | Tetikleyici |
|---|---|---|
| **E1015** | YENİ | desendeki ad sabit değil (sinyal, örnek, döngü değişkeni...) |
| E1001 | kapsam genişledi | desende tanımsız ad; varyant ise fix-it `Enum::Varyant`, not "Volt has no binding patterns" |
| E2010 | kapsam genişledi | değer deseni sınananın aralığına sığmıyor |
| E2003 | kapsam genişledi | enum tipli `const` sayısal sınananda |
| W2014 | kapsam genişledi | `const` adı önceki kolun değerini yineliyor |
| W1002 | kapsam genişledi | test dilinde `let`/`for` tasarım sabitini gölgeliyor |
| E0003 | daraldı | `binding pattern 'Idle'` iletisi kalktı; kalan: yol/tuple deseni |

## Doğrulama

- Testler: `crates/volt-driver/tests/pattern_name_tests.rs` (7 ui/fail
  `*_pattern_name_*`, 1 ui/pass — SV etiketleri, generic, enum const,
  geniş const), `resolve/pattern.rs`, `typeck/matching.rs` (6),
  `sim.rs`, `auto_contract_tests.rs` (sabit adlı FSM kolu), parite
  `p29e` (temiz) / `p29f` (E1015).
- Mutasyon (`build/c1/mutate.py`): 12/12 düştü — sinyal adını kabul
  etmek, döngü değişkenini sabit saymak, kaskadı geri getirmek, varyant
  önerisini kaldırmak, literal/const sığma denetimini kaldırmak, enum
  const kapsamını kaldırmak, sayısal E2003'ü kaldırmak, W2014'ten sabiti
  çıkarmak, SV etiketini const genişliğine döndürmek, test dili W1002'yi
  susturmak, otomatik FSM'in sabit kolunu tanımaması.

## Sonuçlar

- (+) Yazım hatası, gölgeleyen ad ve çıplak varyant hiçbir gün "her şeyi
  yakalayan kol" olamaz; bağlama gelirse açık sözdizimi gerekir.
- (+) Sabit adlı desen yazılabilir (şablonlar literal kullanmak zorunda
  değil); otomatik FSM kontratları bu kolları tanır.
- (+) İki sessiz yanlış kapandı (sığmayan literal, test dili gölgelemesi).
- (−) Rust'tan gelen kullanıcı `n => n` yazamaz; E1001 notu nedenini
  söyler. `PatternKind::Binding` AST'den kalktı (`Foo { x }` struct deseni
  kısayolu `PatternBinding` bildirmeye devam eder — struct deseni E0003).
- (−) `match_cover` imzası sabit değerlendirici alır (iki çağıran).

## Gelecek İş

1. Bağlama deseni gerekirse `x @ desen` ile (ADR-0083 Gelecek iş 2).
2. Desende nitelikli sabit (`pkg::LIMIT`) çok dosyalı birimde — bugün yol
   deseni olarak enum varyantı sayılır.
3. `volt check x_test.volt` tek dosya kipinde tasarım sabitlerini görmez;
   test dili W1002 yalnız `volt test`'te (kardeşli kip) çıkar.
