# ADR-0081: Fonksiyon Desteği — Saf Kombinasyonel `fn` Donanıma İner

> Statü: KABUL EDİLDİ — Aşama 1 (tasarım). Aşama 2 (uygulama) ve Aşama 3
> (örnekler + eşdeğerlik kanıtı) ayrı PR'lar.
> Tarih: 2026-09-26
> Etkilenen (plan): volt-ast (paylaşılan çizge yardımcısı `graph` —
> ADR-0069'un Tarjan'ı buraya taşınır), volt-syntax (fn gövdesinde
> saf olmayan deyimler için E2xxx-B), volt-hir (fn gövdesi tip denetimi,
> çağrı tip kuralları, çağrı çizgesi: özyineleme E4xxx-A ve açılım bütçesi,
> `sync`/örnek/`declassify` saflık denetimleri, fn imzası kısıtları),
> volt-sv-emit (YENİ `inline/`: çağrı yerinde açılım, `structs/`'tan
> önce), volt-diagnostics (dört YENİ kod — numaralar Aşama 2'de: E2xxx-A, E2xxx-B, E3xxx-A, E4xxx-A;
> `volt explain` iki dil), volt-lsp (hover'da imza, tamamlamada imza
> ayrıntısı), `tests/`, `docs/spec/` (Aşama 2), `examples/` (Aşama 3).
> DOKUNULMADI: kod, docs/spec/, docs/research/.
> Genişletir: `grammar-full.ebnf` §8 (FnDecl, "[F2] — kombinasyonel
> mantık"), `domain-inference.md` K5, ADR-0034 (fn kontrat ayrıştırması),
> ADR-0068 (açılım bütçesi), ADR-0069 (çizge döngüsü), ADR-0070 (parite),
> ADR-0080 (derinlik sınırı).

## Sorun

`fn` gramerde (`grammar-full.ebnf:295`) ve parser'da (F1a) var, ama
donanımda kullanılamıyor. Ortak hesaplama (immediate çözme, ALU işlemi,
doyma, parite, CRC adımı) her kullanım yerinde elle tekrarlanıyor.
`examples/riscv_core.volt:251-267` beş immediate biçimini satır içinde
yazıyor; aynı çözme bir ikinci çekirdekte ya da bir çözücü testinde
yeniden yazılmak zorunda. SystemVerilog'da `function` sıradan bir yapı.

Struct (ADR-0077) ve enum (ADR-0074) aynı yöntemle eklendi: önce ölçüm ve
tasarım, sonra uygulama, sonra örnekler ve eşdeğerlik kanıtı. Bu ADR
birinci aşamadır: bugünkü durumu ölçer, anlamı ve kapsamı belirler, SV
eşlemesini araçlarla seçer.

## 1. Mevcut durum (ölçüm)

Sondalar `build/fn1/` (gitignore'da), `target/debug/volt.exe check`,
`main` @ 5c5d88f. Her satır bir komut çıktısıdır.

| Katman | Durum | Kanıt |
|---|---|---|
| Parser | İmza, generic, `->` tipi, `requires`/`ensures` (ADR-0034), gövde ayrıştırılıyor. Gövde yalnız `let`, `for` ve son ifade kabul ediyor (`parser/item.rs:1758` `parse_fn_block`). `reg`, `on`, atama sözdizimi hatası. | `p34`: `error[E0001]: unexpected 'reg', expected let or an expression in the function body`; `p33` (`on`) aynı; `p35` (`y = a`): `E0001 unexpected '=': expected the final expression …`; `p32` (`return a`): `return` anahtar sözcük değil, `E0001 unexpected 'a' …` |
| Resolve | Gövde çözülüyor (`resolve/body.rs:63` `resolve_fn_body`): kapsamın ebeveyni kök; parametreler `LocalBinding`, kontratlar `in_contract`. Modül sinyalleri görünmez. | `p02` (`a + nope`): `E1001 undefined name: 'nope'`; `p10` (`a + x`, `x` modül portu): `E1001 undefined name: 'x'`; `p11` global `const` okuma: temiz |
| Typeck | Gövde **hiç denetlenmiyor**: `typeck/mod.rs:134` öğe döngüsü yalnız `Module` ve `Extern` işler (`_ => {}`). Kullanıcı fn çağrısı `Error` tipi döner (`typeck/synth.rs:127`, yalnız `prev` tip taşır); argümanlar sentezlenir ama imzayla karşılaştırılmaz. | `p01` (`a + true`): **0 hata**; `p08` (`-> bool`, gövde `a: u8`): **0 hata**; `p07` (son ifade yok): yalnız `W1001 unused binding: 't'`; `p29` (dönüş tipi yok): 0 hata |
| Özyineleme | Denetlenmiyor. | `p05` (`f` → `f`) ve `p13` (`f` → `g` → `f`): **0 hata** |
| Arity | Denetlenmiyor (E0003 onu gölgeliyor). | `p12` (`inc(x, x)`): yalnız E0003 |
| SV üretimi | İfade içindeki her kullanıcı çağrısı E0003 (`sv-emit/src/expr.rs:617`). ADR-0070 ile `check`'te de. | `p03`, `p14` (`on` içinde): `E0003 not supported yet: function calls inside expressions …` |
| Kontratta çağrı | `check` ve varsayılan `build` temiz; `build --emit=sva` E0003. ADR-0070 §"A sınıfı" SVA'ya özgü fark olarak listeli. | `p06b`: `check` 0 hata; `build --emit=sva`: `E0003 … 9:15 requires: is_small(x)` |
| Const bağlamı | Yalnız yerleşikler değerlendiriliyor (`consteval.rs:649`). | `p04` (`const N: u32 = dbl(4)`): `E2021 expected a constant expression` |
| Test dili | Kullanıcı fn'i bilinmiyor. | `p09`: `E8505 unknown test builtin 'inc'` |
| Domain | `domain/expr.rs:101` `call_domain` yerleşik değilse `join_list(args)` (K5). | `p31`: `E3001 different clock domains cannot be combined combinationally` çağrı yerinde (`23:14`) |
| Trust | `trust.rs:351` yerleşik değilse argüman etiketlerinin `join`'i. | kod |
| Timing | `timing.rs:450` argüman gecikmelerinin `combine`'ı (uyuşmazlıkta E5010). | kod |
| Saflık kaçışları | `sync()` fn gövdesinde tanısız; `prev()` zaten E5017; gerekçeli `declassify` parser'da soyuluyor, W3008 veriyor. | `p22`: 0 hata; `p23`: `E5017 prev() can only be used inside contracts`; `p27b`: `W3008 deliberate trust downgrade: "test"` |
| `for` | `BlockContext::Function` (`parser/stmt.rs:813`) gövdede `=` atamasını kabul ediyor; dış ad çözülmüyor, `let`'e atama tanısız. | `p18` (`y = a`): `E1001 undefined name: 'y'`; `p19` (`p = p ^ a[i]`, `p` bir `let`): **0 hata** |
| Gölgeleme | fn adı ile modül `let`'i / const ile parametre çakışması W1002. | `p24`, `p25`: `W1002 '…' shadows a definition in an outer scope` |
| `match` ifadesi | Modülde de SV'ye inmiyor (fn'e özgü değil). | `m1.volt`: `E0003 not supported yet: 'match' expressions` |
| LSP | Hover yalnız ad ve tür etiketi; imza yok. Tanıma git genel (`definition.rs` `def_at`). Tamamlama `FUNCTION` türünde, ayrıntı yok. Belge sembolleri `FUNCTION`. | `build/i21/lsp_probe.py p03 7 9`: `hover: "```volt\ninc\n```\nfunction"` |

**fn kullanan mevcut kaynaklar** (tahmin yok — Aşama 2 ADIM 2.1 ölçer):
`tests/`, `examples/`, `tests/fixtures/` altındaki hiçbir `.volt` dosyası
`fn` bildirmiyor (`grep -rlE "^\s*(pub\s+)?fn\s+\w+" --include=*.volt`
boş). Rust testlerine gömülü Volt kaynakları:

| Dosya:satır | Kaynak | Not |
|---|---|---|
| `volt-syntax/tests/parser_tests.rs:289, 309, 1150, 1166, 1178, 1190` | `imp`, `g` (kontratlı), `parity`, `f` (iki `let`), `div` (`requires`/`ensures`), generic `f<const N: u32>` | yalnız parser |
| `volt-hir/tests/resolve_tests.rs:146, 151` | `parity(x, y) -> bool { x ^ y == 0 }`, generic `f<const N: u32>(a: bits<N>)` | `assert_clean_of_errors` |
| `volt-hir/tests/prev_tests.rs:114` | `requires: prev(a) == a { prev(a) }` | E5017 bekler |

Gövdesi denetlenince geçersiz çıkabilecek adaylar: `resolve_tests.rs:146`
(operatör önceliğine bağlı), `:151` ve `parser_tests.rs:1190` (generic fn
— bu turda E0003, Karar 7), `parser_tests.rs:309, 1178` ve `prev_tests.rs:114` (fn kontratı —
bu turda E0003, Karar 8; `prev_tests.rs:114` bugün E5017 bekliyor). Hangisinin hangi testte gerçekten değiştiği
Aşama 2'de ölçülür.

## 2. Anlam — fn nedir?

### Karar 1: fn saf kombinasyonel bir ifadedir

fn, parametrelerinden ve birimin `const`'larından tek bir değer hesaplayan
isimli bir ifadedir. Donanım karşılığı çağrı başına bir kombinasyonel
devredir; iki çağrı iki devredir (paylaşım yok — sentez aracı özdeş
argümanlı iki çağrıyı yine birleştirebilir).

- **Durum yok:** `reg`, `on`, `comb`, örnek (`let u = M { … }`),
  `sync()`/`sync3()` (register zinciri kurar) yasak — çağrının kendisi
  E2xxx-B alır, argüman tip denetiminden önce (hedef saat bir fn'de zaten
  bulunamaz: modül saatleri görünmez, `clock` parametresi Karar 4 ile
  yasak). `prev()` zaten yalnız
  kontratta (E5017) — fn gövdesi kontrat değildir.
- **Yan etki yok:** gövde hiçbir sinyale atama yapamaz. Modül sinyalleri
  zaten görünmez (fn kapsamının ebeveyni kök kapsamdır, `p10` E1001);
  yalnız sözdizimsel olarak mümkün olan atama biçimleri (`y = a` deyimi,
  `for` gövdesindeki atama) E2xxx-B ile reddedilir.
- **Okuma:** parametreler, gövdedeki `let`'ler, birim `const`'ları,
  enum varyantları, diğer fn'ler ve saf yerleşikler (`zext`, `sext`,
  `trunc`, `concat`, `replicate`, `popcount`, `clog2`).

**Gerekçe:** Volt'un her durum öğesinin saati, sıfırlaması ve genişliği
görünür olmalıdır (E5017'nin gerekçesi aynı). Durum taşıyan bir fn aynı
çağrının iki yerde iki register üretmesi demektir ve bu, kaynağı okuyana
görünmez; durum isteyen tasarım modül yazar. Saf olunca fn'in anlamı
bağlamdan bağımsızdır: `on` içinde, `comb` içinde, kontratta aynı değer.

### Karar 2: Gövde = `let`'ler + son ifade; erken dönüş yok

Gramerin yorumu (`grammar-full.ebnf:306` "Son ifade dönüş değeridir (Rust
semantiği)") korunur. `return` anahtar sözcüğü eklenmez.

- Her yolun değer döndürmesi yapısal olarak garantidir: gövde bir
  ifadedir, `if` ifadesi `else` ister (E0008, `p26`), `match` ifadesinin
  kapsayıcılığı mevcut `match` kurallarıyla denetlenir. Erken dönüş
  olmadığı için "dönmeyen yol" analizi gerekmez.
- Dönüş tipi **zorunlu**; son ifade **zorunlu**. İkisinden biri yoksa
  **E2xxx-A** (fn tanımında). Değer döndürmeyen saf fn'in donanım anlamı
  yoktur (`p29` bugün sessiz geçiyor).
- `let` çıkarımı modül `let`'iyle birebir aynı kurallar
  (`type-inference.md`, ADR-0041); son ifade dönüş tipine `check` edilir
  — yazılı hedefe atama kuralı (aynı işaretli genişleme örtük, daralma
  `as` ister).

**Reddedilen — erken `return`:** Donanımda erken dönüş bir öncelik
kodlayıcısıdır; `if/else` zinciri aynı şeyi açıkça söyler. Ayrıca
§5'teki ölçüm: Yosys 0.66 ve 0.36 SV `return` deyimini ayrıştıramıyor.

### Karar 3: `for` bu turda fn gövdesinde E0003

Parser `for`'u fn gövdesinde kabul ediyor ve gövdesinde `let`'e atamaya
izin veriyor (`p19`: tanısız). Bu, `let`'i değiştirilebilir biriktirici
yapan tanımsız bir anlamdır; modülde `let`'e atama çift sürücüdür
(ADR-0073). Biriktirici anlamı (katlama) ayrı bir tasarım ister → **E0003**
(geçerli Volt, bu turda eşlemesi yok). `for` gövdesindeki atamanın hedefi
fn dışı bir adsa E2xxx-B (Karar 1).

Parite/doyma gibi tipik kullanımlar yerleşiklerle yazılabilir
(`popcount(x) & 1`, `if` zinciri); ertelemenin maliyeti düşük.

## 3. Kapsam

### Karar 4: İmza tipleri

| Tip | Parametre | Dönüş | Gerekçe |
|---|---|---|---|
| `uN`, `iN`, `bool`, `bits<N>` | ✓ | ✓ | |
| `Trit` | ✓ | ✓ | ADR-0062 emit'i ifade düzeyinde |
| enum | ✓ | ✓ | ADR-0074 `localparam`'ları açılımdan sonra toplanır |
| düz struct | ✓ | ✓ | açılım `structs/` indirgemesinden önce koşar (Karar 12) |
| dizi `[T; N]` | ✓ (argüman yalın ad olmalı, değilse E0003) | E0003 | parametre argümanın adıyla yer değiştirir; dizi tipli ara telin (argüman ya da dönüş) emit'i ölçülmedi |
| `clock`, `reset` | E2xxx-B | E2xxx-B | saat parametresi fn'i bir saat alanına bağlar; kombinasyonel değer değildir (modülde `let c : clock` kabul ediliyor, `p36` — bu yasak fn'e özgü) |
| `Delayed<T, N>` | E0003 | E0003 | gecikme çağrı yerinde çıkarılır (Karar 10) |
| `struct port` / `Handshake` | E0003 | E0003 | bundle bir değer değildir (ADR-0077 "struct ≠ struct port") |

### Karar 5: Çağrı yerleri — bu tur

Kombinasyonel ifade (modül `let`'i, atama), `on` bloğu, `comb` bloğu,
modül düzeyi `for` (parser açar, ADR-0056), iç içe fn çağrısı ve
**kontratlar** (ikame kipi, Karar 12.3). Çağrı kuralları:

- Argüman sayısı imzayla eşit değilse **E2003** (yerleşik `sync`
  arity'sinin kodu, `typeck/synth.rs:136`).
- Her argüman parametre tipine `check` edilir (yazılı hedef kuralı —
  port bağlantısıyla aynı); uyuşmazlık mevcut E2001/E2002/E2003.
- Çağrının tipi dönüş tipidir.

### Karar 6: Özyineleme — E4xxx-A, çağrı çizgesinde

Doğrudan ya da karşılıklı her özyineleme **E4xxx-A**. Çizge volt-hir'de,
çözümlemeden sonra kurulur: düğüm = fn tanımı (`DefKind::Function`),
kenar = gövdedeki (ve fn kontratlarındaki) çağrı. Ad tabanlı değil
çözüm tabanlı, çünkü fn adı bir `let` tarafından gölgelenebilir (`p24`).

**ADR-0069 mekanizmasının yeniden kullanımı:** `parser/type_graph.rs`'in
Tarjan'ı (`components`, yinelemeli, derin zincirde yığın taşmaz) ve
`path_within` (döngü yolunu en kısa yol olarak verir) tip düğüm modelinden
bağımsız algoritmalardır; ardıl listesi alan ortak bir yardımcıya
(`volt_ast::graph`) taşınır. Tip çizgesi ve çağrı çizgesi ikisi de onu
kullanır. Taşıma saf yeniden düzenlemedir (golden değişmez). Tanı biçimi
E4009 ile aynı: döngüdeki **her** fn E4xxx-A alır (birincil etiket fn
adında, ikincil etiket döngüyü kapatan çağrıda, not olarak döngü yolu
`f → g → f`).

Aynı çizge **açılım bütçesini** de verir (Karar 11): döngüsüz çizgede
her fn'in açılmış düğüm sayısı bir kez hesaplanır.

### Değerlendirilen, sonraya bırakılan

| Konu | Karar | Gerekçe ve gelecek plan |
|---|---|---|
| **Karar 7 — const generic fn** `fn f<const W: u32>(x: uN<W>)` | E0003 (fn tanımında) | Modül mono'su parser'da ve argüman değerleriyle ikame eder (ADR-0041/0056). fn için çağrı yerinden `W` çıkarımı (argüman genişliğinden) gerekir; bu tip çıkarımı genişletmesidir. Sonra: fn mono'su açılım geçidinin parçası olur, `W` argüman tipinden birleştirilir. |
| **Const bağlamında çağrı** (derleme zamanı tablo) | Bu turda E2021 kalır | fn saf olduğu için `consteval` bir yorumlayıcıyla (parametre ortamı + gövde) genişletilebilir. Test dili ile aynı yorumlayıcıyı paylaşmalı; birlikte gelir. |
| **Test dilinde çağrı** (beklenen değer) | Bu turda E8505 kalır | Test dili değerleri testbench üretiminde hesaplanır (ADR-0058); aynı sabit yorumlayıcıyla çözülür. |
| **Karar 8 — fn üzerinde `requires`/`ensures`** | E0003 (kontratın konumunda) | Bugün sessizce yok sayılıyor. Anlam: `requires` çağıranın yükümlülüğüdür → her çağrı yerinde argümanlar ikame edilerek `assert` (verify ve `volt test` izleyicisi, ADR-0064); `ensures` sonucu adlandırmalı → gramerde `result` bağlaması gerekir, ayrı ADR. fn kombinasyonel olduğundan `ensures` fn başına bir kez, serbest girişli sarmalayıcı modülle (derinlik 1) kanıtlanabilir. |
| `match` ifadesi | fn'e özgü değil: modülde de E0003 (`m1.volt`), fn gövdesinde de | Açılım onu çağrı yerine taşır; indirgeme (üçlü zincir, son kol `default`) ölçüldü: §5 "Bt" biçimi B ile 129/129 eşdeğer. Ayrı iş olarak modül ve fn için birlikte açılır. |
| Değişken sayılı argüman, varsayılan argüman, adlandırılmış argüman | yok | Gramerde yok. |
| Kullanılmayan fn uyarısı | yok | Çok dosyalı birimde `pub fn` başka dosyadan kullanılır (ADR-0042); modül/const için de uyarı yok. |

## 4. Diğer katmanlarla etkileşim

### Karar 9: Domain — fn domain-polimorfik, sonuç = argümanların join'i

fn gövdesinin domain'i yoktur (parametreler çağrıdan alır; const'lar
`Timeless`). Çağrının domain'i argüman domain'lerinin K5 join'idir; iki
argüman farklı domain'den geliyorsa **E3001 çağrı yerinde**. Bu
davranış bugün zaten `call_domain` → `join_list` ile üretiliyor (`p31`);
Aşama 2 onu fn'e özgü testle ve mutasyonla korur.

Muhafazakâr yön: gövdede kullanılmayan bir parametre de join'e girer.
Kullanılmayan parametre W1001 alır (`p28`); parametre başına bağımlılık
analizi eklenmez (K5 basit ve sağlam kalır).

### Karar 10: Trust ve timing — bilgi fn'den geçer

- **trust_level:** sonuç = argümanların en yüksek seviyesi (`trust.rs:351`
  `join`). Güven, açılımdan önce imza düzeyinde hesaplanır; açılım bir
  bilgi akışını gizleyemez.
- **`declassify` fn gövdesinde yasak — E3xxx-A.** `declassify` bir güvenlik
  kararıdır ve ADR-0052 onu kullanım yerinde gerekçeyle görünür kılar. fn
  içindeki bir `declassify` her çağrı yerinde görünmez bir düşürme olurdu
  (bugün `p27b` yalnız fn tanımında bir W3008 veriyor, çağrı sayısından
  bağımsız). Düşürme çağıranın modülünde, çağrının sonucuna yazılır.
- **Delayed<T, N> (ADR-0037):** fn kombinasyonel; gecikmeyi değiştirmez.
  Sonucun gecikmesi argümanların `combine`'ıdır (`timing.rs:450`), farklı
  gecikmeli iki argüman **E5010 çağrı yerinde**. İmzada `Delayed` E0003
  (Karar 4).

### Sürücü analizi (ADR-0073)

fn gövdesinde sürücü yoktur (Karar 1); çağrının argümanları okumadır.
Açılımın ürettiği teller (Karar 12) emit'te, analizden sonra doğar ve tek
sürücülüdür; sürücü tablosu değişmez.

### Derinlik sınırı (ADR-0080) ve açılım bütçesi (ADR-0068)

- **Karar 11 — bütçe:** çağrı ağacı üsteldir: `f_k(a) = f_{k-1}(a) ^
  f_{k-1}(~a)` 2^k yaprak açar (§5.8 ölçümü). Açılmış düğüm sayısı
  çizgeden (Karar 6) hesaplanır ve **ikame boyutunu** sayar: gövdedeki
  bir `let`'e her başvuru, o `let`'in değerinin boyutu kadar (ikame
  kipinde `let` kullanım başına kopyalanır; tel kipi için bu bir üst
  sınırdır). Birim başına toplam `MAX_EXPANSION_NODES` (1<<18) aşılırsa
  **E2027 çağrı yerinde**, açılım yapılmaz. Sabit bugün volt-syntax'ta
  `pub(crate)` (`parser/mono/budget.rs:24`); Aşama 2 onu parser ile
  volt-hir'in birlikte gördüğü `volt-ast`'e taşır (tek sabit). E2027'nin
  `explain` metni bugün yalnız döngü açılımını anlatır ("Loop unrolling
  limit"); fn açılımını da kapsayacak şekilde genişletilir. Hesap volt-hir'de
  olduğu için `check` = `build` = LSP.
- **Derinlik:** Karar 12'nin tel kipinde her çağrı sonucu bir tel adıdır;
  açılmış ifadenin yüksekliği fn gövdesinin yüksekliğini aşmaz (parser
  onu zaten 256 ile sınırlar). İkame kipinde (Karar 12.3) yükseklikler toplanır;
  toplam `MAX_DEPTH` (256) aşılırsa **E0018 çağrı yerinde** (emitter
  doğrulaması — ADR-0070 C seçeneği gereği `check`'te de koşar). Ölçülen
  en derin gerçek tasarım 18 (ADR-0080).
- **AstWriter:** ADR-0068 §6'nın `AstWriter`'ı parser'ın mono geçidine
  aittir (`parser/mono/budget.rs`, `pub(super)`). fn açılımı parser'da
  değil emit'te yapılır (Karar 12 — hijyen), bu yüzden kapı aynı değildir;
  eşdeğer güvence bütçenin HIR'da önceden hesaplanması ve emitter'ın bütçe
  aşılmış çağrıyı açmamasıdır (savunma: açılım yığını döngü görürse
  durur).

### Parite (ADR-0070)

Tüm yeni tanılar ya HIR analizinde (E2003, E2xxx-A, E2xxx-B'nın anlamsal
kısmı, E3xxx-A, E4xxx-A, E2027) ya parser'da (E2xxx-B'nın sözdizimsel kısmı)
ya da emitter doğrulamasında (E0018, E1003 sentetik ad) doğar; üçü de
`check`, `build` ve LSP'nin ortak boru hattında. Kontrattaki çağrı artık
`--emit=sva`'ya özgü E0003 vermez (ADR-0070 A sınıfı listesinden bir
kalem düşer). `tests/fixtures/parity/` her yeni tanı için bir sonda alır.

## 5. SV eşlemesi (ölçüm)

### Deney

`build/fn-sv/` (gitignore'da). Kaynak `dec.volt` (varsayımsal — bugün
derlenmez): riscv_core'un immediate çözme ve dallanma kodu fn'lere
taşınmış hâli. Beş immediate fn'i (`imm_b` iki `let`'li), iç içe çağrı +
`match` + enum parametreli `imm(instr, f: Fmt)`, `if` zinciri + struct
parametreli `br_taken(o: Opnd, f3: u3)`. Çağrılar: iki modül düzeyi
atama (`imm`), bir struct literal argümanlı modül düzeyi atama
(`br_taken(Opnd { … }, f3)`, tüm sağ taraf), bir `always_ff` içi
(`imm_i`), iki `invariant` + bir `cover` (`imm`). `comb` bloğu ve blok
içi `for` içindeki çağrı (ikame kipi) bu deneyde **yok**; ikame kipi
yalnız kontratlarda ölçüldü. Enum ADR-0074
`localparam`'larıyla, struct ADR-0077 alan başına sinyalle.

Üç biçim, elle yazıldı (emitter'ın üreteceği biçimde):

- **A — çağrı yerinde açılım:** fn SV'de görünmez. `build/fn-sv/genA.py`
  Karar 12'nin kuralını harfiyen uygular: her çağrı örneği `<fn>_<k>`
  (iç çağrılar da: `imm_0`'ın gövdesi `imm_i_1`, `imm_s_0`, `imm_b_0`, …),
  her `let` `<fn>_<k>_<let>` (`imm_b_0_sign`), yalın olmayan argüman
  `<fn>_<k>_<param>` (struct literal → `br_taken_0_o_a`/`_o_b`), tüm sağ
  taraf olan `br_taken` çağrısına sonuç teli yok; `match` üçlü zincir;
  kontratlar tel üretmeden ikame. (İlk ölçüm turundaki, kuraldan sapan el
  yazımı A — `imm_0_imm_b_sign` adları — yeni A ile 225/225 `$equiv`
  eşdeğer; bütün §5 sayıları yeni A'dandır.)
- **B — modül içinde `function automatic`:** kullanılan her fn, onu
  kullanan her modülün içinde bir kopya (ADR-0024 modül başına dosya);
  `match` → `case`, struct parametre → alan başına argüman (`o_a, o_b`).
- **C — paket fonksiyonu:** birimin fn'leri tek pakette (`dec_fn_pkg`,
  enum `localparam`'ları da orada — paket modülün `localparam`'ını
  göremez), çağrı `dec_fn_pkg::imm(…)`.
- Yardımcı biçimler: **Bt** (B, ama `case` yerine A'nın üçlü zinciri),
  **Bm** (B + kasıtlı hata: `imm_b`'de `<< 1` → `<< 2`), **Bf** (B +
  kasıtlı yanlış assert), **S** (B + Volt'un `--emit=sva` ayrı dosya/`bind`
  biçimi), **B3** (B + modülde `imm` adlı tel).

Araçlar: `verilator/verilator:latest` (5.050), `hdlc/yosys:latest`
(Yosys 0.66), `hdlc/formal:latest` (SymbiYosys, Yosys 0.36+42 — `volt
verify`'ın imajı). Komutlar `MSYS_NO_PATHCONV=1 docker run --rm -v
C:/Dev/volthdl/build/fn-sv:/work -w /work/<biçim> <imaj> …`.

### 5.1 Yosys `function` desteği (özellik matrisi)

`build/fn-sv/yf/` — her özellik ayrı dosya, `yf/run.sh`:
`yosys -q -p "read_verilog -sv $f; synth -top T"` (0.66) ve
`yosys -q -p "read -formal $f; prep -top T"` (0.36):

```
01_return.sv: 01_return.sv:10: ERROR: syntax error, unexpected TOK_ID     # 0.66 ve 0.36
02_name_assign.sv: OK            # f = ifade;
03_unsigned_in_fn.sv: OK         # $unsigned($signed(a) >>> 2)
04_local_vars.sv: OK             # fonksiyon içi logic yerelleri
05_nested_call.sv: OK            # iç içe çağrı
06_case.sv: OK                   # case + modül localparam'ı (enum)
07_if.sv: OK                     # if / else if
08_for.sv: OK                    # for (int i …) biriktirici
09_in_always_ff.sv: OK           # always_ff içinde çağrı
10_output_arg.sv: OK             # void + output argümanlar
11_packed_ret_slice.sv: OK       # {h, l} = f(x)
12_struct_flat_args.sv: OK       # struct alan başına argüman
13_func_in_ternary_nested_expr.sv: OK   # f(f(x)) üçlü içinde
14_localparam_in_fn_const.sv: OK
15_signed_ret.sv: OK             # logic signed dönüş
16_dyn_index_in_fn.sv: OK        # a[i]
17_part_select_in_fn.sv: OK
18_no_automatic.sv: OK
19_let_init_decl.sv: 19_let_init_decl.sv:10: ERROR: Invalid nesting of always blocks and/or initializations.   # logic t = a << 1;
```

İki Yosys sürümünde aynı sonuç. **Kırılanlar:** SV `return` deyimi ve
fonksiyon yerelinin bildirimde ilk değeri. İç içe çağrı, `case`/`if`,
enum `localparam`'ı, struct argümanları, `always_ff` içi çağrı
kırılmıyor. B ve C bu yüzden `f = ifade;` biçiminde ve yerelleri ayrı
bildirim + atama olarak yazılmak zorunda (ölçümün geri kalanı bu biçimle).

### 5.2 Verilator `-Wall`

```
$ verilator --lint-only -Wall --top-module Dec Dec.sv                   # A
- Verilator: Built from 0.033 MB sources in 2 modules, into 0.041 MB in 3 C++ files …
$ verilator --lint-only -Wall --top-module Dec Dec.sv                   # B
%Warning-VARHIDDEN: Dec.sv:60:105: Declaration of signal hides declaration in upper scope: 'f3'
%Error: Exiting due to 1 warning(s)
$ verilator --lint-only -Wall --top-module Dec dec_fn_pkg.sv Dec.sv     # C (ters sırada da temiz)
- Verilator: Built from 0.047 MB sources in 3 modules, into 0.041 MB in 3 C++ files …
```

B'ye özgü iki uyarı sınıfı ve bir hata:

1. **VARHIDDEN:** fn parametresi (`f3`) modülde aynı adlı bir sinyalle
   çakışıyor. Volt'ta olağan bir durum: parametre adları çağıranın
   sinyal adlarıyla sık sık aynıdır (`instr`, `f3`).
2. **UNUSEDSIGNAL (bit düzeyi):** `yf/17` —
   `%Warning-UNUSEDSIGNAL: 17_part_select_in_fn.sv:9:56: Bits of function
   variable are not used: 'a'[7,2:0]`. Çözücü fn'lerinin doğası bu
   (`fn rd(i: u32) -> u5 { i[11:7] as u5 }`). A'da parametre yoktur,
   argüman olan modül sinyalinin öteki bitleri başka yerde okunur.
3. **Ad çakışması hata:** B3 (modülde `wire [31:0] imm`, aynı modülde
   `function imm`):
   ```
   %Error: Dec.sv:77:17: Unsupported in C: Variable has same name as function: 'imm'
   %Error: Dec.sv:77:17: Found definition of 'imm' as a FUNC but expected a variable
   ```
   Volt bu durumu W1002 ile kabul ediyor (`p24`: modül `let imm` fn `imm`'i
   gölgeler; fn yine başka bir fn üzerinden dolaylı çağrılabilir → B'de
   aynı modüle ikisi de yazılır).

### 5.3 Sentez (Yosys 0.66)

`synth.sh`: `synth -top Dec -flatten; opt_clean -purge; rename -enumerate;
write_verilog -noattr net.v; stat` + `synth_ice40 -top Dec; stat` +
`synth_xilinx -top Dec -flatten -noiopad; stat`:

| | genel | iCE40 | xc7 |
|---|---|---|---|
| A | 852 | 392: 235 SB_LUT4, 125 SB_CARRY, 32 SB_DFFSR | 210: 72 LUT6, 36 LUT2, 17 LUT4, 8 LUT3, 7 LUT5, 36 CARRY4, 32 FDRE, 1 MUXF7, 1 BUFG |
| B | 848 | 398: 241 SB_LUT4, 125 SB_CARRY, 32 SB_DFFSR | 211: 61 LUT6, 36 LUT2, 28 LUT4, 15 LUT3, 1 LUT5, 36 CARRY4, 32 FDRE, 1 MUXF7, 1 BUFG |
| C | 848 | B ile aynı | B ile aynı |
| Bt | 858 | **A ile aynı** (392, aynı dağılım) | **A ile aynı** (210, aynı dağılım) |

B ve C'nin netlisti **byte-aynı** (`diff B/net.v C/net.v` → 0 satır).
A-B farkı fonksiyon mekanizmasından değil, `match`'in indirgenmesinden
geliyor: B'nin `case`'i A'nın üçlü zinciriyle değiştirilince (Bt) iCE40
ve xc7 hücre dağılımı A ile birebir aynı. Genel hücre sayısındaki
küçük fark (852/858) ABC'nin başlangıç ağından; ADR-0077 bulgusu
(sentez sayısı satır/ad duyarlıdır, eşdeğerlik kanıtı değildir) burada
da geçerli.

### 5.4 Eşdeğerlik (Yosys 0.66, ADR-0077 yöntemi)

`eq.sh` (`build/struct3/eq/eq.ys` ile aynı akış: `prep; flatten;
equiv_make gold gate equiv; equiv_simple -seq 1; equiv_induct -seq 1;
equiv_status -assert`):

```
A <-> B:  rc=0  Of those cells 129 are proven and 0 are unproven.
A <-> C:  rc=0  Of those cells 129 are proven and 0 are unproven.
B <-> Bt: rc=0  Of those cells 129 are proven and 0 are unproven.
A <-> Bt: rc=0  Of those cells 129 are proven and 0 are unproven.
A <-> Bm: rc=1  Of those cells 67 are proven and 62 are unproven.
              ERROR: Found 62 unproven $equiv cells in 'equiv_status -assert'.
A <-> A_old: rc=0  Of those cells 225 are proven and 0 are unproven.
```

Dört biçim aynı donanım; kasıtlı hata (Bm) denetimi düşürüyor.

### 5.5 Formal ve SVA

**Immediate kipi** (`volt verify`'ın kipi, `sva.rs:37`: kontrat tasarım
modülünün içine `always @(posedge clk) if (!rst) assert (…); // volt:<ad>`
olarak gömülür). `formal.sh`: `read -formal …; prep -top Dec` + sby
(prove derinlik 4, cover derinlik 6, boolector):

```
A read -formal: OK   prv: summary: successful proof by k-induction. DONE (PASS, rc=0)
                     cov: Reached cover statement at Dec.sv:78 … DONE (PASS, rc=0)
B read -formal: OK   prv: … successful proof by k-induction. DONE (PASS, rc=0)
                     cov: Reached cover statement at Dec.sv:86 … DONE (PASS, rc=0)
C read -formal: OK   prv: … successful proof by k-induction. DONE (PASS, rc=0)
                     cov: Reached cover statement at Dec.sv:41 … DONE (PASS, rc=0)
Bf (yanlış assert):  prv: Assert failed in Dec: Dec.sv:82.20-82.62 … DONE (FAIL, rc=2)
```

Fonksiyon çağıran immediate assert Yosys 0.36 formal ön ucunda üç
biçimde de çalışıyor; kanıt boş geçmiyor (Bf).

**Ayrı dosya kipi** (`--emit=sva` varsayılanı, `sva.rs:26`: kontratlar
ayrı `<mod>_sva` modülünde, `bind <Mod> <mod>_sva sva_inst (.*);`).
S biçimi:

```
$ verilator --lint-only -Wall -Wno-VARHIDDEN --top-module Dec Dec.sv dec.sva
%Error: dec.sva:10:10: Can't find definition of task/function: 'imm_b'
```

B'nin modül içi fonksiyonu bağlanan SVA modülünden görünmez. B'de
kontratta çağrı için fonksiyonların SVA modülüne de kopyalanması gerekir;
A'da kontrat zaten ikame edilmiş ifadedir.

### 5.6 Dosya sırası (C)

```
$ yosys -q -p "read_verilog -sv Dec.sv; read_verilog -sv dec_fn_pkg.sv; hierarchy -check -top Dec"
Dec.sv:25: ERROR: Can't resolve function name `\dec_fn_pkg::imm_i'.
$ yosys -q -p "read_verilog -sv dec_fn_pkg.sv; read_verilog -sv Dec.sv; hierarchy -check -top Dec"
(temiz)
```

`sorted(glob)` sırası `Dec.sv dec_fn_pkg.sv` — `scripts/sta/run.py:166`
(`sorted(p.name for p in rtl.glob("*.sv"))`, dosya başına
`read_verilog -sv`) tam bu akışla kırılır. ADR-0074/0077'deki paket
bulgusunun aynısı; ayrıca her modül dosyası pakete bağımlı olur,
ADR-0024'ün "her `.sv` kendi başına yeterli" ilkesi bozulur.

### 5.7 Simülasyon, dalga formu ve netlist adları

Aynı uyarıcı (`tb.sv`, 40 döngü), `verilator --binary --trace --timing`:

```
A: SIG acc=ba3868f3 pc=00000435
B: SIG acc=ba3868f3 pc=00000435
C: SIG acc=ba3868f3 pc=00000435
```

VCD'de `dut` kapsamındaki sinyaller:

```
A: … pc_r imm_i_0 imm_i_1 imm_s_0 imm_b_0_sign imm_b_0_mid imm_b_0 imm_u_0 imm_j_0 imm_0
   imm_i_2 imm_s_1 imm_b_1_sign imm_b_1_mid imm_b_1 imm_u_1 imm_j_1 imm_1 br_taken_0_o_a br_taken_0_o_b
B: … pc_r                       (fonksiyon yerelleri ve sonuçları yok)
C: … pc_r                       (aynı; ek olarak boş dec_fn_pkg kapsamı)
```

Sentez sonrası netlist adları (`synth -top Dec; select -list w:*imm* …`):

```
A: Dec/imm_0  Dec/imm_b_0  Dec/imm_b_0_sign  Dec/imm_b_0_mid  Dec/imm_i_0 …
   Dec/br_taken_0_o_a  Dec/br_taken_0_o_b                                              (19)
B: Dec/imm_b$func$Dec.sv:53$14.sign  Dec/imm$func$Dec.sv:75$11.instr
   Dec/br_taken$func$Dec.sv:76$17.o_a …                                                (21)
```

A'nın ara değerleri kaynak konumundan bağımsız, öngörülebilir adlarla
netliste kalıyor. B'nin adları SV satır numarası ve Yosys'in iç sayacını
taşıyor: kaynağa bir satır eklenince değişir. ADR-0012 ("sentetik adlar
öngörülebilir kalıpta", ECO) ve ADR-0054 (SDC hücre adları) açısından
belirleyici.

### 5.8 Açılım maliyeti (iç içe çağrı ağacı)

`build/fn-sv/chain/`: `f0(a) = a + 1`, `fk(a) = f{k-1}(a) ^ f{k-1}(~a)`
(2^N yaprak), A ifade açılımı, B fonksiyon zinciri. Yosys 0.66
`read_verilog -sv; synth -top Ch` süresi:

| N | A SV | B SV | A Yosys | B Yosys |
|---|---|---|---|---|
| 4 | 376 B | 652 B | 34 ms | 82 ms |
| 8 | 4 336 B | 1 080 B | 85 ms | 288 ms |
| 12 | 67 696 B | 1 519 B | 1 116 ms | 26 714 ms |

Donanım her iki biçimde de çağrı başına bir kopyadır; B yalnız metni
kısaltır, açılımı Yosys'e bırakır ve orada **24 kat yavaş** açılır.
Açılım bütçesi (Karar 11) biçimden bağımsız olarak gereklidir.

### Karar 12: A — çağrı yerinde açılım (SEÇİLDİ)

| Ölçüt | A | B | C |
|---|---|---|---|
| Yosys 0.66 / 0.36 ön ucu | temiz | `return` ve bildirim-ilk-değer yasak (§5.1) | B + dosya sırası (§5.6) |
| Verilator `-Wall` | temiz | VARHIDDEN, bit düzeyi UNUSEDSIGNAL, fn/tel ad çakışması hatası (§5.2) | temiz |
| Sentez | eşdeğer (129/129) | eşdeğer | eşdeğer, B ile byte-aynı netlist |
| Immediate formal | çalışıyor | çalışıyor | çalışıyor |
| `--emit=sva` (bind) | çalışıyor (ikame) | fonksiyon görünmez (§5.5) | paket import'u gerekir |
| Dalga formu | ara değerler adlı teller | görünmez | görünmez |
| Netlist adı (ADR-0012) | `<fn>_<k>_<let>` kararlı | `$func$<satır>$<sayaç>` | B ile aynı |
| ADR-0024 (dosya başına yeterlilik) | ✓ | ✓ | ✗ |
| ADR-0078 (ayrılmış sözcük) | fn, parametre ve yerel adı SV'de hiç çıplak geçmez | fn/parametre/yerel adı SV tanımlayıcısı olur → E1013 kapsamı genişlemeli | B ile aynı |
| SV okunabilirliği | gövde her çağrıda tekrar (başlık yorumu kaynağa bağlar) | fn kaynaktaki gibi görünür | B ile aynı |
| Açılım maliyeti | Volt'ta, bütçeli | Yosys'te, 24× yavaş (§5.8) | B ile aynı |

B'nin tek üstünlüğü SV metninde fn'in görünmesi. Bunun bedeli üç yeni
Verilator sınıfı (her biri ad karıştırma ya da `lint_off` sarması ister),
iki Yosys kısıtı, SVA modülüne fonksiyon kopyalama ve dalga formunda,
netlistte ara değerlerin kaybı. C, B'nin bütün bedeline ADR-0074/0077'nin
reddettiği paket sırası sorununu ekler. **A seçildi.**

**A'nın biçimi (Aşama 2'nin uygulayacağı kural):**

1. **Nerede:** volt-sv-emit'te, `emit_unit` içinde AST kopyası üzerinde
   bir indirgeme (`inline/`), ADR-0077'nin `structs/` deseni. `structs/`
   ve enum `localparam` toplamasından **önce** koşar: açılımın ürettiği
   struct tipli teller ve enum varyant kullanımları mevcut geçitlerden
   geçer. fn yoksa indirgeme `None` döner → fn kullanmayan tasarımın
   çıktısı byte-aynı kalır (golden). Hijyen için ad değil çözüm
   kullanılır: gövdede bir parametreye başvuru (`resolutions` →
   parametre `DefId`) argümanla, bir `let`'e başvuru onun teliyle, bir
   `const`'a başvuru const'ın emit biçimiyle yer değiştirir. Açılım
   parser'da yapılmaz: orada çözüm yoktur ve fn gövdesindeki `K`, çağıran
   modüldeki bir `let K`'ye yanlış bağlanırdı.
2. **Tel kipi** (varsayılan): çağrı modül düzeyi bir `let`/atamanın sağ
   tarafında ya da bir `on` bloğundaysa ve argümanları yalnız modül
   düzeyi adlara (port, reg, tel, `let`, const, örnek çıkışı) başvuruyorsa:
   - her çağrı örneği modül içinde sıra numarası alır: `<fn>_<k>`, `k`
     fn başına 0'dan, açılım sırasında (kaynak sırası; önce dış çağrı,
     sonra gövdesindeki çağrılar);
   - fn'in her `let`'i bir tel: `<fn>_<k>_<let>`;
   - sonuç teli `<fn>_<k>`; çağrı bir modül `let`inin ya da atamanın tüm
     sağ tarafıysa sonuç teli yazılmaz, hedef açılmış ifadeyle doğrudan
     sürülür (`k` yine ayrılır);
   - yalın ad/alan yolu ya da literal olmayan argüman bir tel alır:
     `<fn>_<k>_<param>` (dalga formunda argüman da görünür);
   - teller, çağrıyı içeren deyimden (ya da `always_ff`'ten) hemen önce,
     bağımlılık sırasıyla bildirilir; başlarında
     `// <fn>(<argümanlar>) — <dosya>:<satır>` yorumu;
   - üretilen ad birimdeki bir adla çakışırsa **E1003** (ADR-0077 §"Ad"
     ve bundle ile aynı kural), çağrı yerinde, not: hangi fn'in hangi
     çağrısının ürettiği.
   - `on` bloğunda `if`/`match` altındaki çağrının teli koşulsuz kurulur;
     fn saf olduğundan bu anlamı değiştirmez (tel kenar öncesi değerleri
     okur — `<=` ile aynı). §5'te ölçülen `always_ff` çağrısı koşulsuzdu.
3. **İkame kipi:** tel kipinin koşulu tutmayan her çağrı — `comb` bloğu,
   blok içi `for` (döngü değişkeni argümanda olabilir), argümanı blok
   yereli bir ada başvuran `on` bloğu çağrısı ve kontratlar (her SVA kipi: ayrı dosya, gömülü, immediate,
   simülasyon izleyicisi). Tel üretilmez; parametreler argüman ifadesiyle
   (parantezli), `let`'ler değerleriyle ikame edilir. Kontratta tel
   üretilmediği için `--emit=sva` RTL'yi değiştirmez (golden). `comb`
   bloğunda tel kipi, blok içinde sonradan atanan bir sinyali okuyan
   çağrının anlamını değiştirebileceği için kullanılmaz. Struct tipli
   parametrenin argümanı bu kipte yalın yol olmalı; değilse E0003.
4. **fn gövdesi çağrılmasa da bir kez doğrulanır:** emitter her fn
   gövdesini tanım konumunda ikame kipinde bir kez emit edip atar; E0003
   (örn. `match` ifadesi) fn tanımında ve çağrı sayısından bağımsız bir
   kez raporlanır.

## 6. LSP ve tanılar

### Karar 13: LSP — bu tur

- **Hover:** imza ve belge yorumu:
  ```volt
  fn imm(instr: u32, f: Fmt) -> u32
  ```
  (bugün yalnız `inc` + "function"). Çağrı yerinde ve tanımda aynı.
- **Tanıma git:** çözüm tabanlı `def_at` zaten fn tanımına gidiyor;
  Aşama 2'de protokol testiyle sabitlenir.
- **Tamamlama:** fn adları mevcut (`FUNCTION`); `detail` alanına imza
  eklenir (enum varyantı ve port tamamlamasındaki `detail` deseni).
- **Sonraya:** `textDocument/signatureHelp` (parametre ipucu) yeni bir
  sunucu yeteneğidir (`serverInfo` yetenek listesi değişir); ayrı iş.

### Tanılar

| Kod | Yeni/mevcut | Anlam | Konum |
|---|---|---|---|
| **E2xxx-A** | YENİ | fn'in sonucu yok: dönüş tipi yazılmamış ya da gövde son ifadeyle bitmiyor | fn tanımı (ad / kapanış `}`) |
| **E2xxx-B** | YENİ | fn kombinasyonel değil: gövdede `reg`, `on`, `comb`, atama, örnek, `sync()`/`sync3()`; imzada `clock`/`reset` | gövdedeki yapı / imza; not: "durum gerekiyorsa modül yazın" |
| **E3xxx-A** | YENİ | fn gövdesinde `declassify` | gövdedeki `declassify` |
| **E4xxx-A** | YENİ | özyineli fn (doğrudan/karşılıklı) | döngüdeki her fn tanımı; ikincil etiket kapatan çağrı; not döngü yolu |
| E2001 / E2002 / E2003 | mevcut | argüman sayısı (E2003); argüman ↔ parametre, son ifade ↔ dönüş tipi | çağrı / argüman / son ifade |
| E3001 | mevcut | farklı domain'den argümanlar | çağrı |
| E5010 | mevcut | farklı gecikmeli argümanlar | çağrı |
| E5017 | mevcut | fn gövdesinde `prev()` | gövde |
| E1003 | mevcut | açılımın ürettiği tel adı çakışıyor | çağrı |
| E2027 | mevcut (`explain` genişler) | açılım bütçesi | çağrı |
| E0018 | mevcut | ikame kipinde açılmış yükseklik > 256 | çağrı |
| E0003 | mevcut | generic fn, fn kontratı, fn gövdesinde `for`, dizi dönüşü, yalın ad olmayan dizi argümanı, imzada `Delayed`/bundle, struct literal argümanı ikame kipinde, `match` ifadesi | tanım / çağrı |
| E2021 | mevcut | const bağlamında çağrı | çağrı |
| E8505 | mevcut | test dilinde çağrı | çağrı |

**Tanının yeri ilkesi:** fn'in kendisiyle ilgili hata (gövde tipi,
saflık, sonuç, özyineleme, desteklenmeyen yapı) **tanımda ve bir kez**
— çağrı sayısı ne olursa olsun. Kullanımla ilgili hata (arity, argüman
tipi, domain, gecikme, ad çakışması, bütçe) **çağrı yerinde**. Gövde
imzaya karşı bir kez denetlendiği için açılım gövde hatası üretemez; bu,
her çağrının kendi kopyasını denetleyen bir açılım modelinin tanıyı çağrı
sayısıyla çoğaltmasından kaçınır (ADR-0068 katlamasına gerek kalmaz).

## Reddedilenler

- **B — modül içi `function automatic`:** §5.2 (üç Verilator sınıfı),
  §5.1 (Yosys `return`/ilk değer), §5.5 (bind'li SVA), §5.7 (dalga formu,
  kararsız netlist adları), §5.8 (Yosys'te 24× yavaş açılım).
- **C — paket fonksiyonu:** B'nin bedeli + dosya sırası (§5.6) + ADR-0024.
- **Parser'da açılım (mono gibi):** hijyen (Karar 12.1) ve tanıların
  çağrı sayısıyla çoğalması; tip denetimi imza yerine her kopyada
  yapılırdı.
- **Erken `return`:** Karar 2.
- **Durumlu fn (`reg`'li):** Karar 1 — durum modülün işidir.
- **fn'de `declassify`:** Karar 10.
- **Parametre başına domain bağımlılığı:** Karar 9 — K5 join yeterli,
  kullanılmayan parametre zaten W1001.

## Gelecek iş

1. `match` ifadesinin SV'ye inmesi (modül ve fn birlikte; üçlü zincir
   biçimi §5.4'te B ile eşdeğer ölçüldü).
2. Const bağlamında ve test dilinde fn çağrısı: ortak sabit yorumlayıcı.
3. Const generic fn (`W` argüman genişliğinden çıkarılır).
4. fn kontratları: `requires` çağrı yerinde assert; `ensures` için
   `result` bağlaması (gramer, ayrı ADR); fn başına sarmalayıcı modülle
   kanıt.
5. fn gövdesinde `for` (katlama/biriktirici anlamı).
6. Dizi dönüş tipi.
7. LSP `signatureHelp`.

## Uygulama planı

### Aşama 2 — uygulama (dal `feat/fn`, main'den)

1. **ADIM 2.1 — gövde tip denetimi (önce, tek başına commit):**
   `typeck/mod.rs:134` döngüsüne `ItemKind::Fn` — imza tiplerinin
   çözümü, gövde `let`'leri, son ifadenin dönüş tipine `check`'i, E2xxx-A.
   Önce/sonra tanı karşılaştırması: `tests/ui/**`, `examples/**`,
   `tests/fixtures/**` ve §1'deki gömülü Rust test kaynakları (golden
   betiği ADR-0080'in `build/depth/golden.py` desenini izler). Yeni tanı
   çıkan her dosya sınıflanır: A (gerçek hata → fixture düzeltilir,
   `examples/` raporlanır) / B (yanlış alarm → kural düzeltilir). Tablo
   PR açıklamasında.
2. Çağrı tip kuralları (Karar 5): `synth_call` kullanıcı fn'inde arity
   (E2003), argüman `check`'i, dönüş tipi.
3. Saflık: parser'da E2xxx-B (fn gövdesinde `reg`/`on`/`comb`/atama
   deyimi, `for` gövdesinde atama); HIR'da `sync`/örnek (E2xxx-B),
   `declassify` (E3xxx-A); `for` E0003; imza kısıtları (Karar 4; `clock`/`reset` E2xxx-B).
4. `volt_ast::graph` (Tarjan + `path_within`, `type_graph.rs`'ten taşıma
   — önce, davranış değişmeden, golden ile); çağrı çizgesi, E4xxx-A;
   açılım bütçesi, E2027.
5. Generic fn ve fn kontratı E0003 (Karar 7, 8).
6. `volt-sv-emit/src/inline/` (Karar 12): tel ve ikame kipleri, ad
   ayırma ve E1003, E0018 yükseklik, çağrılmayan gövdenin doğrulanması;
   `expr.rs:617`'nin E0003'ü kaldırılır.
7. Domain/trust/timing: davranış mevcut (§1); fn'e özgü testler.
8. LSP (Karar 13): hover imzası, tamamlama `detail`'i, tanıma git
   protokol testi.
9. Tanılar: dört yeni kod iki dilde + `volt explain`; `explain_tests`
   `CODE_COUNT` ve kod listesi; parite sondaları
   (`tests/fixtures/parity/`), her yeni tanı için.
10. Spec (ADR kaynaklı): `grammar-full.ebnf` §8 yorumu (saflık, E2xxx-A,
    `for`), `type-inference.md` (fn kuralları), `domain-inference.md` K5
    (fn çağrısı), `sv-mapping.md` (açılım biçimi), `name-resolution.md`
    (fn kapsamı yalnız öğeleri görür), `const-eval.md` (fn çağrısı sabit
    değil).
11. Testler: `tests/ui/pass/` — basit fn, iç içe çağrı, struct/enum
    parametreli fn, `on` bloğunda çağrı, `comb` bloğunda çağrı, kontratta
    çağrı, iki dosyalı birimde `pub fn` (ADR-0042); `tests/ui/fail/` — her
    yeni kod, doğrudan ve karşılıklı özyineleme, fn'de `reg`/`on`, dış
    sinyale atama, farklı domain argümanı (E3001), arity. `ui/pass` sayımı
    beş yerde (ADR-0080 notu: `parser_tests.rs` `total`+`clean`,
    `ui_semantic_tests.rs`, `ui_pass_build_tests.rs`, consistency).
    Simülasyon: fn içeren tasarımda `volt test` (Docker). Formal: fn
    çağıran kontratta `volt verify` (Docker). Çıktı ağı (ADR-0079): fn
    içeren `ui/pass` dosyalarının SV'si Verilator + Yosys'ten geçer.
12. Mutasyon (tek tek, `CARGO_BUILD_JOBS=2`, `--test-threads=2`): saflık
    denetimini kaldır → test düşmeli; özyineleme tespitini kaldır → test
    düşmeli; fn çağrısında domain join'ini atla → CDC testi düşmeli;
    ek olarak arity denetimi, dönüş tipi `check`'i, açılım bütçesi.
13. Golden: PR #44 sonrası referans; fn kullanmayan her tasarımın
    çıktısı byte-aynı (Karar 12.1'in `None` yolu).

ADR'den sapma gerekirse (örn. tel kipi kuralının bir durumda tutmaması,
struct literal argümanının `structs/` ile açılamaması) uygulama durur ve
ADR güncellenir.

### Aşama 3 — örnekler ve kanıt (dal `feat/fn-examples`)

`examples/riscv_core.volt`: immediate çözme (I/S/B/U/J), ALU işlemi,
dallanma koşulu fn'lere; başka adaylar (FIR doyma, UART parite, AXI yanıt
kodu) okunabilirliği artırıyorsa. Donanımı değiştiren taşıma yapılmaz.
Kanıt ADR-0077 yöntemiyle: `volt test` (riscv_core 59/59 ve diğerleri),
`volt verify`, Verilator `-Wall`, Yosys `equiv_make`/`equiv_induct` eski ↔
yeni (tüm hücreler + kasıtlı hatayla negatif kontrol), Yosys `stat`
karşılaştırması (fark varsa ad/satır duyarlılığıyla açıklanır). Beklenen
SV farkı: `let imm_i = sext_i(instr)` biçiminde sonuç teli yazılmaz
(Karar 12.2), fn'in `let`'leri `<fn>_<k>_<let>` telleri olarak çıkar.
README "Limitations" ve CHANGELOG güncellenir.

## Sonuçlar

- fn'in anlamı saf kombinasyonel; durum, yan etki, erken dönüş ve
  `declassify` dışarıda.
- Gövde imzaya karşı bir kez denetlenir; tanılar tanımda ya da çağrı
  yerinde, çağrı sayısıyla çoğalmaz.
- Domain, trust ve timing çağrı yerinde argümanları birleştirir — mevcut
  davranış, bu ADR onu sözleşme yapar.
- SV'de fn görünmez; çağrı yerinde açılır, ara değerler öngörülebilir
  adlı tellerdir. Üç biçimin aynı donanım olduğu 129/129 `$equiv`
  hücresiyle kanıtlandı.
- Özyineleme ADR-0069'un çizge algoritmasıyla (ortak yardımcıya
  taşınarak) reddedilir; açılım ADR-0068 bütçesiyle sınırlanır.
- Kontratta fn çağrısı verify ve `--emit=sva`'da çalışır; ADR-0070'in
  A sınıfı listesinden bir kalem düşer.
