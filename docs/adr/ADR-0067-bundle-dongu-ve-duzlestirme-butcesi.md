# ADR-0067: Özyineli Bundle Tespiti ve Düzleştirme Bütçesi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-23
> Etkilenen: volt-syntax (`parser/bundle.rs` — `find_cycles`, `MAX_FLAT_PORTS`,
> `Overflow`, `Flat::budget`, E4009/E4010 üretimi; `parser/handshake.rs` —
> `PlainDefs` yapısı, `payload_fields` bütçesi), volt-diagnostics (E4009,
> E4010; mesaj + `volt explain` iki dilde), `tests/ui/fail/72-73`,
> `tests/fuzz_regressions/` (YENİ) + `crates/volt-syntax/tests/fuzz_regression_tests.rs`,
> `.github/workflows/{ci,fuzz-nightly}.yml` (regresyon dizini tohum olarak).
> DOKUNULMADI: volt-hir, volt-sv-emit, examples/, README.md, docs/spec/.

## Sorun

PR #19'un tohum girdili fuzz'ı (`tests/ui/` + libFuzzer) parser'da bir
bellek taşması buldu: 2087 baytlık bir girdi (`tests/ui/fail/35_bundle_direction.volt`
mutasyonu) `volt check` altında 60 saniyede bitmiyor, tepe bellek 10,5 GB;
CI'da libFuzzer 2 GB sınırında `out-of-memory` ile düşüyor. Bu,
error-recovery.md'nin "derleyici asla çökmez" ilkesinin ihlalidir ve dil
sunucusu için de risktir: editörde yazılan yarım bir dosya LSP'yi
kilitleyebilir.

Düzeltilene kadar gecelik fuzz her gece kırmızıydı, PR fuzz'ı ara sıra.

## Tespit

**Aşama.** Lexer 324 µs, öğe ayrıştırması 739 µs (4 öğe, 22 ifade);
patlama birim sonu desugar zincirinde (`finish_unit_desugar`). Bellek
dalgalanması (10 GB → 0,5 GB → 9 GB) bir `Vec`'in ikiye katlanarak
büyümesinin izi: tek bir dev liste kuruluyor.

**Kök neden.** Girdideki kapanmayan `struct port Req {` gövdeleri hata
kurtarmayla tek struct'a katılınca `Req`, kendini tipi olarak taşıyan
birden çok alan (`in req : Req`) içeriyor. `bundle.rs::expand` iç içe
bundle'ı özyineli açar; kendine referanslı tanım için tek koruma
`MAX_NESTING = 8`'de **sessizce durmak**tı. k tane kendine dönen alan
k⁹ port üretir:

| k | süre (release, Windows) |
|---|---|
| 3 | 10 ms |
| 5 | 0,79 s |
| 6 | 3,5 s |
| fuzz girdisi | > 60 s, 10,5 GB (öldürüldü) |

Aynı sınıf ikinci yerde de var: `handshake.rs::payload_fields`, `Handshake<T>`
payload'ı olan sade struct'ı aynı derinlik sınırıyla açar; `struct P { f1 : P, … f6 : P }`
+ `in x : Handshake<P>` → 6⁸, 5,4 s.

**Asıl hata bir sınır eksikliği değil, bir tanı eksikliğidir:** kendini
içeren bir port grubunun sonlu düz biçimi yoktur; derleyici bunu bugüne
dek hatasız derliyor ve `req_req_req_addr` gibi anlamsız portlar üretip
her birine W1001 basıyordu. Derinlik sınırı semptomu gizliyordu.

## Seçenekler

**A — `MAX_NESTING`'i düşürmek / port sayısını saymak, döngüyü kabul etmek.**
Semptom bastırma: özyineli bundle yine sessizce "bir şey"e derlenir.
Elendi.

**B — Döngü tespiti + tanı, ARTI genel bütçe (SEÇİLDİ).** Döngü E4009 ile
reddedilir ve düzleştirmeye hiç girmez; döngüsüz ama elmas biçimli bir
çizge (iki alanı olan struct'ın iki alanı olan struct'ı …, 2ᵈ) ya da büyük
bir bundle dizisi (`[Bundle; 256]`, ADR-0056) yine üstel/büyük açılabilir,
bu yüzden açılım bir bütçeyle sınırlanır ve aşımı E4010'dur. İki mekanizma
birbirini tamamlar: E4009 anlam hatası, E4010 kaynak sınırı.

**C — Döngüyü volt-hir'de tip denetiminde yakalamak.** Düzleştirme parser
katmanında (ADR-0039) koştuğundan HIR bundle'ı hiç görmez; tanı orada
gecikmiş olurdu ve patlama zaten olmuş olurdu. Elendi. (HIR'in sade
özyineli struct'ı — `struct P { f : P }` — bugün hiç tanılamadığı ayrıca
tespit edildi; bkz. Sınırlar.)

## Karar

### 1. E4009 — özyineli `struct port` / Handshake payload'ı

`collect_bundle_defs` tanım çizgesini kurar (ad → alanın tip adı) ve
`find_cycles` ile bildirim sırasında, deterministik olarak döngüleri
bulur (tanım sayısı küçüktür, O(n²) DFS). Döngü ÜZERİNDEKİ her struct
için bir E4009: birincil etiket struct adında, ikincil etiket döngüyü
kapatan ilk alanda ("this field closes the cycle"). Karşılıklı döngü
(A ⇄ B) iki tanı üretir. Döngüye **ulaşan** bütün tanımlar (döngüdekiler
ve onlara başvuranlar) `defs`'ten çıkarılır: modül portu olduğu gibi
kalır, kaskad yok, modül zaten hatalıdır.

Handshake tarafında `PlainDefs` artık bir yapıdır (`fields` + `cyclic`);
payload tipi döngüye ulaşıyorsa port E4009 alır ve `data` tek opak alan
olarak kalır.

### 2. E4010 — düzleştirme bütçesi

- `MAX_FLAT_PORTS = 4096`: modül başına düzleştirme sonrası port sayısı
  (256 elemanlı bundle dizisi × 16 alanlık arayüz — sınır dahil).
- `MAX_NESTING = 8`: iç içelik derinliği; eskiden sessiz kesme, artık hata.

Bütçe **açılım sırasında** denetlenir (`Flat::budget`): ilk aşımda açılım
durur, kalan özyinelemeler hemen döner, modül başına tek E4010 üretilir
(port sayısı için modül adında, derinlik için port bildiriminde). Sonradan
saymak yetmez — patlama sayma anından önce olurdu. Maliyet O(bütçe).

### 3. Kalıcı regresyon ve tohum

`tests/fuzz_regressions/` her fuzz bulgusunu ham haliyle saklar (bu ADR:
`oom_recursive_struct_port_2087b.volt`); `fuzz_regression_tests.rs` her
dosyayı ayrı iş parçacığında 5 saniyelik süre sınırıyla ayrıştırır, aşan
düşer. Dizin CI fuzz işlerinde (PR ve gecelik) `tests/ui/` yanında salt
okunur tohumdur — libFuzzer corpus'u gitignore'dadır, kalıcı tek yer
budur.

### 4. Genel güvence değerlendirmesi

| Aday | Karar | Gerekçe |
|---|---|---|
| Hata kurtarmada "her adımda en az bir token" debug assert'i | **EKLENMEDİ** | Kurtarma döngülerinde koşu zamanı ilerleme garantisi zaten var (`parse_items_only`, `parse_struct_fields`, `recover_silent`: `pos == before → bump_any`). Aşama ölçümü lexer/parser'ı akladı (< 1 ms); assert eklemek bu hataya değmeyen ek yüzeydir. |
| Tanı sayısına üst sınır (ör. 1000) | **EKLENMEDİ** | Girdi 608 tanı üretti, bellek etkisi yok; sınır gerçek hataları gizleyebilir ve `//~ ERROR` testlerini kırılganlaştırır. Tanı üretimi patlamanın kaynağı değildi. |
| Açılım/mono'da üretilen düğüm sayısına sınır | **EKLENDİ (bundle için, E4010)** | Patlamanın sınıfı bu. Mono ve `for` açılımının kendi sınırları zaten var: `MAX_ROUNDS = 64`, `MAX_UNROLL = 4096`, `MAX_CONST_DEPTH = 64`, `MAX_BUNDLE_ARRAY = 256`; eksik olan bundle çizgesiydi. |

## Sonuçlar

- Fuzz girdisi: > 60 s / 10,5 GB → anında (regresyon testi 5 s sınırıyla
  geçer), E4009 raporlanır.
- Özyineli bundle artık **derlenmez** (eskiden derleniyordu). Bu, davranış
  değişikliğidir ama eski davranış anlamsız porttu; örneklerde ve
  testlerde kullanan yok.
- `MAX_NESTING` aşımı hata oldu: 9+ seviye iç içe gerçek arayüz yok;
  varsa E4010 açık bir mesajla söyler.
- Kod sayısı 131 → 133 (`explain_tests::CODE_COUNT`).

## Sınırlar / Ertelenen

- **HIR özyineli sade struct'ı tanılamıyor:** `struct P { d : u8, f : P }`
  bir modül portu olarak `volt check`'ten hatasız geçiyor (sonsuz tip). Bu
  ADR yalnız Handshake payload yolunu kapatır (E4009); genel tip denetimi
  ayrı iş.
- **Generic `struct port` portu sessizce geçiyor:** `struct port G<T> { out d : T }`
  + `in g : G<u8>` + `o = g.d` düzleştirilmez (ADR-0039 sınırı) ve HIR de
  hata vermez. Ayrı iş.
- Bütçe değerleri (4096 / 8) ölçümle değil, mevcut sınırlarla tutarlılıkla
  seçildi (`MAX_UNROLL`, `MAX_BUNDLE_ARRAY`); gerçek bir tasarım aşarsa
  yükseltilir.
- Fuzz corpus'u geceden geceye önbellekte taşınır (PR #19); bu bulgudan
  önce corpus'ta olmayan bir yolun bulunması için tohum şarttı —
  tohumsuz 300 sn'lik eski koşu bunu hiç bulmamıştı.
