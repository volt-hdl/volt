# ADR-0080: Özyineleme Derinliği — Parser Ağacı Sınırlar, Derleyici Bilinen Yığında Koşar

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-25
> Etkilenen: volt-syntax (`parser/depth.rs` — YENİ: `MAX_DEPTH` = 256,
> `descend`, sınırda grup atlama; `stack.rs` — YENİ: `COMPILER_STACK_SIZE`
> = 64 MB, `with_compiler_stack`; `parser/expr.rs` zincir halkası sayımı,
> `if` ifadesi `else if` zinciri döngüye; `parser/stmt.rs` blok ve `if`
> deyimi `else if` zinciri döngüye; `parser/item.rs` tip, `parser/pattern.rs`
> desen koruması; `parser/type_graph.rs` açılmış tip derinliği; `parse` /
> `parse_unit` / `parse_expr` derleyici yığınında), volt-driver (`main`
> derleyici yığınında), volt-lsp (tokio işçileri derleyici yığınıyla),
> volt-diagnostics (E0018, iki dilde + `volt explain`), tests
> (`fuzz_regressions/stack_*` 14 girdi, ui pass 111 / fail 142)
> Güncelleme (2026-09-26, #44): fuzz hedefi iş parçacığı açmaz, `-max_len=4096`
> — bkz. §8.
> İlgili: ADR-0067 (fuzz regresyonları), ADR-0068 §6 "Sınırlar" (bu ADR'nin
> konusu olan bulgu), ADR-0058 (test dili derinlik sınırı — aynı sayacı
> kullanır), ADR-0069 (tip çizgesi), ADR-0070 (CLI/LSP ortak boru hattı)

## Sorun

ADR-0068 §6 "Sınırlar" bulgusu: 30 000 terimlik `a + a + …` zinciri generic
şablonda `Cloner` özyinelemesinde, generic olmayan modülde `volt check`'te
yığını taşırıyordu. Parser'ın derinlik sınırı (`MAX_DEPTH` = 200) yalnız
**özyinelemeyi** sayıyordu: sol-birleşimli zincir Pratt döngüsünde düz
kalır ama ağaçta her halka bir kattır. Sonraki geçitlerin hiçbirinde
koruma yoktu.

Rust'ta yığın taşması panik değil **abort**'tur: `catch_unwind` yakalayamaz,
süreç ölür; LSP'de dil sunucusu editörün altından gider. "Derleyici asla
çökmez" ilkesinin en sert ihlali.

## 1. Ölçüm (ADIM 1)

**Yöntem.** `build/depth/probe.py` her yapıyı n derinlikte üretir ve `volt
check` / `volt build --emit sva,sdc` çıkış kodunda taşmayı (0xC00000FD)
arayarak eşiği ikili arama ile bulur. Geçit başına maliyet için geçici bir
koşum (`depthprobe`, commit edilmedi) hedef geçidi verilen yığınlı bir iş
parçacığında, öncekileri 1 GB yığında koşturup n = 190'da geçtiği en küçük
yığını arar. **En kötü durum ölçüldü:** debug derleme (CI ve testler),
Windows ana iş parçacığı 1 MB. Linux ana iş parçacığı 8 MB'tır; çerçeveler
aynı olduğundan eşikler ~8 katıdır (ayrı ölçülmedi — en kötü durum
bağlayıcıdır). Test ve tokio iş parçacıkları 2 MB'tır.

### 1.1 Düzeltmeden önce — `volt check` taşma eşiği (Windows, debug, 1 MB)

| Yapı | Örnek | Eşik (kat) | Taşan yer |
|---|---|---|---|
| Sol-birleşimli zincir | `a + a + …` | 147–152 | HIR/emit (parser zinciri sayamıyordu) |
| … generic şablonda | aynı, `module G<N>` | 147–152 | aynı; parser'daki `Cloner` 2 KB/kat (§1.2) |
| Parantez | `((…a…))` | 166–171 | parser'ın kendisi — sınır 200'e ulaşmadan |
| `if` ifadesi | `if c { if c { … } } else …` | 161–166 | parser |
| `as` zinciri | `a as u8 as u8 …` | 137–142 | emit |
| İç içe `match` deyimi | `match a { 0 => { match … } }` | 181–186 | parser |
| Dizi tipi | `[[[u8; 1]; 1]; …]` | 391–410 | parser (tip yolu sayılmıyordu) |
| `else if` deyim zinciri | `if … else if … else if …` | 1640–1718 | parser (zincir sayılmıyordu) |
| Tip takma adı zinciri | `type T1 = T0` … | 1171–1210 | typeck (`resolve_named_type`) |
| Sağ-birleşimli (parantezli) zincir, tekli işleç, iç içe indeks | | taşmadı | eski E0001 sınırı (200) yakaladı |
| … generic şablonda sağ-birleşimli / indeks | | 586–605 / 488–508 | `Cloner` |
| İç içe `if` blokları, iç içe `for` | | taşmadı, 40 000'de **zaman aşımı** | eski sınır tek token atlıyordu: tanı kaskadı |
| `const` zinciri (`C1 = C0` …) | | > 40 000 | özyineleme yok |
| struct alan zinciri | | — | E4010 (8 kat, ADR-0077) |
| Modül hiyerarşisi (M1 örnekler M0 …) | | > 20 000, taşmaz | **ama 5 000'de 5,3 s, 20 000'de 86 s** (§7) |
| Geniş `match` (5 000 kol) | | taşmaz | — |
| fn çağrı zinciri | | — | E0003 (ifade içinde çağrı desteklenmiyor) |

### 1.2 Geçit başına yığın (n = 190; geçidin geçtiği en küçük yığın, KB)

Yalnız özyinelemeli geçitler; `pre`, `consteval`, `trust`, `rdc`, `timing`,
`handshake`, `tests`, `constraints`, `annotate` her yapıda 11 KB'ta
(özyineleme yok ya da sığ) geçti.

| Yapı | parse | resolve | typeck | domain | emit (SV+SVA) |
|---|---:|---:|---:|---:|---:|
| zincir | 11 | 199 | 199 | 199 | 1223 |
| zincir, generic | 391 | 199 | 199 | 199 | 1223 |
| parantez | 1095 | 11 | 11 | 11 | 11 |
| tekli işleç | 775 | 131 | 455 | 131 | 1095 |
| `if` ifadesi | 1095 | 199 | 67 | 199 | 1095 |
| iç içe indeks | 711 | 199 | 199 | 199 | 1095 |
| `as` zinciri | 11 | 131 | 131 | 131 | **1351** |
| iç içe blok | 583 | 199 | 67 | 131 | 455 |
| iç içe `match` | 1031 | 199 | 131 | 119 | 583 |
| iç içe `for` | 839 | 199 | 131 | 103 | 327 |
| dizi tipi | 455 | 67 | 131 | 11 | 11 |

Kat başına en ağır: **emit ~7,1 KB** (`as` zinciri), **parser ~5,8 KB**
(parantez), `Cloner` ~2 KB, çözümleme/tip/saat alanı ≤ 2,4 KB. Parser
tek başına bile eski sınırda (200 × 5,8 KB ≈ 1,1 MB) Windows ana iş
parçacığını aşıyordu: eski sınır debug'da hiçbir zaman güvenli değildi.

### 1.3 Gerçek tasarımlar

`examples/` ve `tests/ui/pass/` içindeki her dosyada parser sayacının
ulaştığı en yüksek değer (geçici sayaçla ölçüldü): **18**
(`tests/ui/pass/72_mmio_driver_generation.volt`, üretilen `@mmio` sürücü
kodu); örneklerde 17 (`examples/riscv_core.volt`). Sınır 256 → ~14 kat pay.

## 2. Karar (ADIM 2)

**A + D.** Parser tek giriş noktasıdır: `MAX_DEPTH`'ten derin ağaç
ÜRETMEZ (A); derleyici her platformda aynı, bilinen boyutta bir yığında
koşar (D). İkisi birlikte bir güvencedir: A ağacın yüksekliğini, D o
yüksekliği yürümek için gereken yığını sabitler. Bildirim zincirleri
(ağaçta değil, bildirimden bildirime uzanır) de parser katmanında, tip
çizgesinde (ADR-0069) sınırlanır — B'nin gerektiği tek sınıf, yine tek
noktada.

### 2.1 A — Ağaç yüksekliği tek sayaçta

Sayaç özyinelemeyi değil **ağaç yüksekliğini** izler (`parser/depth.rs`,
`Parser::descend`):

- iç içe ifade (`parse_expr_bp` girişi), blok, tip, desen: kat başına bir;
- **zincir halkası** — sol-birleşimli ikili operatör ve postfix (`[]`,
  `.`, `()`, `as`): parser'da döngüdür, ağaçta her halka bir kattır
  (`chain_link`; ADR-0058'in test dili ifade ayrıştırıcısındaki kural);
- **`else if` zinciri** — deyimde ve ifadede döngüye çevrildi (eskiden
  sayılmayan özyineleme); iç içe düğümler sondan kurulur, span'ler aynı.

Sınırda:

- Açılış ayracındaysa o **dengeli grup**, değilse (`if`, `~`, ad…)
  **kapsayan grubun kapanışına** ya da açık bir deyim/öğe başlangıcına
  kadar atlanır (yinelemeli sayaç — atlanan grup ne kadar derin olursa
  olsun yığın kullanmaz). Zincir kesilir: `lhs` Error yaprağı olur, kalan
  halkalar girişteki derinlikte ayrıştırılıp atılır.
- E0018 **dosya başına bir kez** raporlanır. Atlamanın bittiği konumda
  üretilen parser hatası atlamanın sonucudur ve bastırılır
  (`cut_by_depth`; kaskad penceresinin derinlik karşılığı); başlığı
  atlanan `match x {` / `if c {` gövdesi aranmaz, kapsayan grubun `}`'ı
  tüketilmez. Sonuç: ölçülen 14 yapının hepsinde (20 000–30 000 kat)
  **tek tanı: E0018**, < 0,15 s.

### 2.2 Bildirim zincirleri — açılmış tip derinliği

Takma ad / struct alanı / enum varyantı zinciri tek `TypeRef`'te derin
değildir, bildirimden bildirime uzanır; typeck onu özyinelemeyle açar
(önbelleğe rağmen: ilk çözüm zincirin tepesinden gelirse tüm zincir
yığında). Parser'ın tip çizgesi (ADR-0069) zaten her tip bildirimini ve
üye kenarlarını kurar; Tarjan bileşenleri ters topolojik sıradadır, böylece
**açılmış derinlik** tek geçişte, yinelemeli hesaplanır:
`derinlik(T) = 1 + max(üye TypeRef yüksekliği + hedef bildirimin derinliği)`.
Takma ad başına iki kat (bildirim + hedef tip). `MAX_DEPTH`'i ilk aşan
bildirimde tek E0018 (zincirin her halkasında değil); döngüler E4009 almış
olduğundan sayılmaz. Bildirimin kendisi de bir kattır: parser'ın kesmediği
255 katlı bir tek tip yalnız bu denetimden E0018 alır. Parser dosyada
ağacı zaten kestiyse bu denetim koşmaz (aynı derin tip iki kez
raporlanmaz; dosya başına tek E0018). Sıraya bağlı değildir (typeck önbelleğinde olsaydı
bildirim sırası tanıyı değiştirirdi).

Diğer zincirler ölçüldü ve koruma gerektirmedi: `const` zinciri özyineli
değil (> 40 000); struct iç içeliği E4010 ile 8 kat; modül hiyerarşisi
20 000 katta taşmıyor (ama yavaş, §7); fn çağrısı ifade içinde E0003.

### 2.3 D — Derleyici yığını

`volt_syntax::COMPILER_STACK_SIZE` = 64 MB, `with_compiler_stack(f)`:
`f`'yi o yığınlı bir iş parçacığında koşar, paniği çağırana taşır,
iş parçacığı açılamazsa çağıranın yığınında koşar.

- `volt` sürücüsünün `main`'i bütünüyle (tüm alt komutlar, `volt lsp`'nin
  `block_on`'u dahil) derleyici yığınında.
- LSP tokio çalışma zamanı `thread_stack_size(COMPILER_STACK_SIZE)`:
  `didChange` analizi işçi iş parçacığında koşar (varsayılan 2 MB).
- `parse` / `parse_unit` / `parse_expr` kendi derleyici yığınını açar:
  kütüphaneyi hangi iş parçacığından çağırırsa çağırsın (test, fuzz,
  LSP, dış kullanıcı) sınırdaki girdi taşmaz. Maliyet parse başına bir iş
  parçacığı açılışı (§6).

**Sınır ve yığın birlikte seçildi:** 256 kat × 7,1 KB (debug'daki en ağır
geçit) ≈ 1,8 MB — Windows ana iş parçacığını aşar, yani D olmadan A
yetmez (sınırı ~100'e çekmek gerçek tasarımlara yakın, ASan'lı fuzz'da
yine sınırda olurdu). 64 MB bunun ~35 katı; kullanılmayan sayfa fiziksel
bellek tutmaz. Test dili (ADR-0058) aynı sayacı paylaşır: sınırı 200'den
256'ya çıktı.

### Reddedilenler

- **B — her geçide kendi koruması:** ~15 geçit (mono, çözümleme, tip,
  saat alanı, güven, zamanlama, sürücü analizi, const-eval, SV/SVA/SDC/SW
  üretimi, LSP hover/tamamlama) ve her gelecekteki geçit sınırı hatırlamak
  zorunda kalır; biri unutulunca abort geri gelir. Tanılar da geçit geçit
  farklı yerde, farklı biçimde olurdu.
- **C — derin yürüyüşleri yinelemeli yapmak:** yüzlerce özyinelemeli
  fonksiyon; açık yığınla yeniden yazım hem büyük hem hataya açık, üstelik
  ağaç sınırsız kalırsa bellek (ADR-0068) sınıfına geçer. Yeni kod
  özyinelemeli yazılmaya devam eder.
- **Yalnız D (büyük yığın):** yalnız eşiği yükseltir — 30 000 terimlik
  zincir × 7,1 KB = 210 MB; sınırsız girdi yine abort eder.
- **`stacker` gibi yığın büyüten crate:** yeni bağımlılık (CLAUDE.md
  gerekçe ister) ve `psm` üzerinden platforma özgü kod; iş parçacığı
  açmak aynı güvenceyi standart kütüphaneyle verir.
- **Sınırda tek token atlamak (eski davranış):** `if a == 0 { … }`
  içinde sınıra `if`'te çarpınca kalanı kaskad yaptı — 999 E0001 ve
  insan biçimli çıktıda 140 s.

## 3. Tanı: E0018

`nesting is too deep: more than 256 levels` / `iç içelik çok derin: 256
kattan fazla`; birincil etiket sınıra ulaşılan token, öneri ara `let` /
ayrı modül, not: zincir halkalarının da kat sayıldığı. Tip zinciri için
`type 'T' is nested too deep … once its aliases, fields and variants are
expanded`. `volt explain E0018` iki dilde: neden (abort, tek nokta),
kat sayılanlar, gerçek tasarımların sınırın çok altında kaldığı, dengeli
biçimin hem sığ hem daha iyi donanım olduğu; not: 64 MB yığın, platformdan
bağımsız denetim.

## 4. Testler (ADIM 4)

- `tests/fuzz_regressions/stack_*.volt` — 14 girdi (zincir düz/generic
  30 000, parantez, tekli, indeks, `as`, alan zinciri, `if` ifadesi, `else
  if`, iç içe `if`/`match`, desen, dizi tipi, takma ad zinciri). Eski
  ikilide 11'i abort, 3'ü eski E0001 sınırında (kaskadlı).
- volt-syntax `depth_limit_tests.rs`: her yapı derin → tek E0018 + ağaç
  yüksekliği sınırlı (arenalar üzerinde yinelemeli ölçüm) + < 1 s; sınır
  altı → E0018 yok; dosyada birden çok derin yer → tek E0018; kesilen
  zincirden sonraki öğe kaybolmaz; `else if` yapısı ve span'leri; takma ad
  zinciri sınırı aşan bildirimde; **256 KB yığınlı çağırandan `parse`**.
  `fuzz_regression_tests.rs`: her `stack_*` tek E0018, < 1 s.
- volt-driver `depth_limit_tests.rs` — **ayrı süreçte** (abort çıkış
  kodu olarak görünür): her `stack_*` için `check` ve `build` çıkış 1 +
  tek E0018; sınırın 8 altındaki 9 geçerli tasarım (zincir, generic
  zincir, parantez, tekli, `as`, `if` ifadesi, `else if`, iç içe `if`,
  iç içe `match`) `build --emit sva,sdc` ile tüm geçitlerden geçer — yığın
  bütçesinin kanıtı.
- volt-driver `lsp_protocol_tests.rs`: derin girdiler pull ve push
  (`didChange` → tokio işçisi) yolunda tek E0018, sunucu sonra hover'a
  yanıt verir; sınır altı tasarım tanısız, en derin yaprakta hover ve belge
  sembolleri yanıtlanır.
- `tests/ui/pass/111` (64 terimlik XOR zinciri, 16 halkalı `else if`, 32
  parantez — Verilator `-Wall` temiz), `tests/ui/fail/142` (E0018).

### 4.1 Mutasyon (`build/depth/mutate.py`)

Her mutasyon kaynağı değiştirir, ilgili testleri koşar, `try/finally` ile
geri yükler. **Abort** = test ikilisi ya da alt süreç "has overflowed its
stack" ile öldü (sürücü testi bunu `ABORT — süreç çöktü` olarak raporlar).

| Mutasyon | Sonuç | Düşüren test |
|---|---|---|
| M1 sınır yok (`descend` hep geçer) | **ABORT** (zincir 30 000, parantez 20 000 — `volt-compiler` iş parçacığı taşar) | driver `stack_regressions…` |
| M2 zincir halkası sayılmaz | **ABORT** | driver `stack_regressions…` |
| M3 `else if` deyim zinciri sayılmaz | assert: E0018 yok (3 000 halka 64 MB'a sığar) | driver `stack_regressions…` |
| M4 `else if` ifade zinciri sayılmaz | assert | syntax `every_deep_construct…` |
| M5 açılmış tip derinliği yok | assert | syntax `type_alias_chain…` |
| M6 `main` derleyici yığınında değil | **ABORT** (sınır altı zincir, Windows 1 MB) | driver `designs_just_below_the_limit…` |
| M7 `parse` kendi yığınını açmıyor | **ABORT** (256 KB yığınlı çağıran) | syntax `parse_is_safe_from_a_small_caller_stack` |
| M8 LSP işçileri tokio varsayılanı (2 MB) | **hayatta kaldı** | — |
| M9 sınırda tek token atlanır | assert (kaskad) | syntax `every_deep_construct…` |
| M10 atlama sonrası hata bastırılmaz | assert (kaskad) | syntax `every_deep_construct…` |
| M11 derleyici yığını 64 KB | **ABORT** | driver (iki test) |

**M8 bilinçli olarak kanıtsız paydır:** LSP'nin en ağır yolu (sınırın 8
altındaki `as` zinciri, `didChange` → SV doğrulaması tokio işçisinde)
debug derlemede tokio'nun 2 MB varsayılanına sığıyor. Ayar, gelecekteki
daha ağır bir geçit ya da daha büyük çerçeveli bir derleyici sürümü için
tek satırlık güvencedir; test onu zorlayamaz (sınır bunu engeller).

## 5. Golden

PR #41 sonrası `main` ikilisine karşı `build/depth/golden.py` (check insan
+ JSON, build SV/SVA/SDC): **457 dosyada sıfır fark** (ui/pass, ui/fail,
fixtures, examples, önceki fuzz girdileri). Geçerli tasarımlar ve mevcut
hatalı girdilerin tanıları değişmedi.

## 6. Maliyet

`with_compiler_stack` her çağrıda 64 MB'lık (ayrılmış, işlenmemiş) yığınla
bir iş parçacığı açar. Ölçüm (Windows, `build/depth/spawnbench`, 5 000
yineleme): açılış **~78 µs**; küçük bir modülün `parse`'ı açılış dahil
release'te ~92 µs, debug'da ~146 µs. `volt` komutu başına bir kez (main) +
`parse` başına bir kez; LSP'de analiz başına ~0,1 ms — önemsiz. Fuzz
hedefi (`parse_never_panics`) de her yürütmede bir iş parçacığı açar;
etkisi CI fuzz işinin exec/s değeriyle izlenir (PR açıklaması).

## 7. Sınırlar / Ertelenen

- **Modül hiyerarşisi derinliği yavaş, çökmez:** M1 → M0 … zinciri 5 000
  modülde `check` 5,3 s, 20 000'de 86 s (karesel bir örnek çizgesi
  yürüyüşü). Yığın taşmıyor; zaman aşımı sınıfı. Ayrı iş.
- **İnsan biçimli tanı çıktısı dev tek satırda yavaş:** 1 MB'lık tek
  satırlı girdide 255 tanı basmak 68 s tutuyordu (her tanı satırın
  tamamını basıyor). Derinlik sınıfında artık tek tanı olduğundan
  görünmüyor; uzun satırı kırpma ayrı iş.
- Test dili ifadeleri ayrı bir ağaçtır (`TestExpr`, kutulu); aynı sayaç ve
  sınırla korunur, ağaç yüksekliği değişmez testi yalnız ana AST içindir.
- Sınır, üretilmiş koddaki uzun düz zincirleri (ör. binlerce terimli XOR)
  reddeder; öneri dengeli biçim ya da ara `let`. Gerçek ihtiyaç doğarsa
  parser dengeli ağaç kurabilir (değerlendirme sırası birleşme özelliğine
  bağlı) — bu ADR'nin kapsamı dışında.

## 8. Güncelleme — fuzz hızı (2026-09-26, PR #44)

§6'daki maliyet fuzz'da önemsiz çıkmadı: CI `Fuzz (60 s)` işi PR #41'de
1221, bu ADR'den sonra 713 exec/s idi. İki neden vardı:

- **Tohum boyutu.** 60 KB'lık `stack_*` regresyon girdileri fuzz tohumu
  olarak da veriliyor; libFuzzer azami girdi boyunu en büyük tohumdan
  aldığı için `max_len` 2974'ten 60143'e çıktı. Karar: iki iş akışında
  (`ci.yml`, `fuzz-nightly.yml`) sabit **`-max_len=4096`**. Boyut filtresi
  derin tohumları fuzz'dan tümüyle düşürürdü. `max_len` ise onları 4 KB'lık
  öneklerine kırpar; önek yine 256 katı aşar, yani derin yol fuzz'da kalır.
  Fuzz yapılandırması artık tohum boyutundan bağımsız.
- **Parse başına iş parçacığı.** Linux'ta açılış ~160 µs, kalıcı bir işçiye
  kanalla devir bile ~49 µs; küçük bir girdinin ayrıştırılması ~20 µs.
  Karar: yeni **`volt_syntax::parse_on_current_stack`**, `parse`'ın iş
  parçacığı açmayan biçimi (`parse` artık onu `with_compiler_stack` ile
  sarar). Fuzz hedefi onu libFuzzer'ın **ana iş parçacığında** (Linux 8 MB)
  çağırır; §2.3'teki "`parse` kendi yığınını açar" güvencesi diğer bütün
  çağıranlar için geçerlidir. Sözleşme testi
  (`parse_on_current_stack_fits_in_two_megabytes`): sınırdaki her girdi
  debug derlemede 2 MB yığında ayrışır, 1 MB'ta taşar.

Yan kazanç: fuzz artık 64 MB'ın arkasına saklanmaz, parser'ın derinlik
korumasındaki bir gerileme fuzz'da yığın taşması olarak görünür. Ters
deneyde `descend` sınırı kaldırılınca fuzz ikilisi tam parantez girdisinde ve
4 KB'lık önekinde ASan stack-overflow verdi. Sınır yerindeyken kırpılmamış
19 regresyon girdisi 8 MB'lık ana iş parçacığında çökmeden geçer.

| Ölçüm | lim | exec/s |
|---|---|---|
| Yerel (Linux/ASan, 60 s, aynı tohumlar), bu ADR'den sonra | 60143 | 428 |
| … yalnız `-max_len=4096` | 4096 | 775 |
| … `max_len` + `parse_on_current_stack` | 4096 | 1669 |
| CI: PR #41 / bu ADR / #44 (iki koşu) | | 1221 / 713 / 1095, 1150 |

Kalan ~%8 `max_len`'den gelmiyor: yerelde 4096 ile 2974 ayırt edilemedi
(1912/1889'a karşı 1808/2031). Olası nedenler CI koşuları arasındaki
dalgalanma ve bu ADR'nin her ayrıştırmaya eklediği iş (tip çizgesinde
ikinci derinlik geçişi); ayrıştırılmadı.
