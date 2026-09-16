# ADR-0056: Düzenli Yapılar — `for` İçinde Örnekleme, Bundle Dizileri, Paketlenmiş Dizi Portları

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-17
> Etkilenen: volt-ast (`SourceFile::generate`: `GenerateInfo`,
> `GenerateIter`, `block_generic_args`), volt-syntax (`parser/mono/unroll.rs`
> YENİ; `mono/clone.rs` yeniden adlandırma + sabit katlama; `mono/mod.rs`
> tur içi açılım; `parser/bundle.rs` bundle dizileri + E2008;
> `parser/handshake.rs` prefix parametresi; `parser/item.rs`
> `finish_unit_desugar`: mono → flatten → bidir sırası; `parser/stmt.rs`
> blok içi `let w = W<8> { }`), volt-hir (`annotate_generate`), volt-sv-emit
> (`generate.rs` modül seviyesi açılım SİLİNDİ; `packed_arrays`,
> `signal_sig`/`port_sig`, `packed_select`; `expr.rs`, `instance.rs`),
> volt-driver (tanı notu), docs/spec/const-eval.md §8 ve sv-mapping.md §2
> (bu ADR kaynaklı notlar), examples/systolic/ (YENİ), tests/ui/pass/75-77,
> tests/ui/fail/58, crate testleri (unroll_tests, bundle_array_tests,
> generate_semantic_tests, generate_emit_tests).
> DOKUNULMADI: README.md, grammar-full.ebnf (sözdizimi değişmedi).

## Sorun

docs/design/noc-tasarimi.md iki dil sınırı saptadı; ikisi de NoC'ye özgü
değil, her düzenli yapı (sistolik dizi, N kanallı arayüz, paralel işlem
birimleri, router mesh) aynı duvara çarpıyordu:

1. **Modül seviyesi `for` yalnız atama açıyordu.** Gövde `Block`/`BlockStmt`
   olarak ayrışır; `BlockStmt`'in `Instance` varyantı yoktur, `let pe = Pe {
   ... }` gövdede `BlockStmt::Let` + `StructLit` olarak kalır ve
   volt-sv-emit/generate.rs (`emit_for_body_assigns`) `If | Match | Let`
   kolunda E0003 ("future") üretiyordu. Teknik engel mimariydi: döngü en son
   aşamada (emitter) açılıyordu, oysa bir örnekleme isim çözümleme, tip
   denetimi, alan çıkarımı ve sürücü analizinden geçmek zorundadır. Açılım
   HIR'dan ÖNCE yapılmalıydı.
2. **Bundle dizileri düzleşmiyordu.** `flatten_bundles` yalnız
   `TypeRefKind::Path` tipli portu bundle sayar; `[Handshake<u8>; 2]`
   `(None, None)` koluna düşüp olduğu gibi geçiyor, resolve E1001
   "undefined name: Handshake" veriyordu. Ayrıca düzleştirme mono'dan
   önce koştuğundan `ch[i].valid` indeksleri literal değildi.
3. **Ek bulgu:** dizi tipli port ve `wire`'ın SV eşlemesi yoktu (lib.rs
   `sig_of_typeref`, E0003; yalnız reg dizileri). Komşu bağlantı telleri
   olmadan düzenli yapı yazılamaz. Docker ölçümü: Yosys unpacked dizi
   PORTUNU reddediyor (`syntax error, unexpected '['`), Verilator kabul
   ediyor; paketlenmiş vektör + `+:` part-select'i ikisi de kabul ediyor.

4×4 mesh elle ~1.400 satır; pratikte yazılamıyordu.

## Karar

### 1. Modül seviyesi `for` PARSER'DA açılır (mono turu içinde)

ADR-0038/0039/0041 silme ilkesi: `parser/mono/unroll.rs` her modül
seviyesi `for`u `monomorphize` turunun içinde (`process_module`, istek
toplamadan önce) açar. Gövde yineleme başına `Cloner` ile modül gövdesine
klonlanır:

- Döngü değişkeni literale ikame edilir (mono'nun `subst` haritası, artık
  `i128`; negatif değer `-lit`). İki literalli `+ - * / %` katlanır
  (`(0 + 1) * 2 + 1` → `3`), SV çıktısı okunur kalır.
- Gövdede bildirilen isimler (`let`) yineleme soneki alır: `pe` → `pe_0`;
  iç içe `pe_0_1` (dış indeks önce). Referanslar (`pe.out`, LValue tabanı)
  aynı haritayla yeniden yazılır; alan ve port adlarına dokunulmaz.
- `let ad = Modül { ... }` (tip anotasyonsuz yapı literali) `InstanceDecl`e
  dönüşür — stmt.rs [N3] kuralının aynısı. Blok içi `let w = W<8> { }`
  parser'da yapı literali + `generate.block_generic_args` yan tablosu
  olarak saklanır, kaldırma sırasında `generic_args`a taşınır; aynı mono
  turu bu isteği toplar (`Pe<8>` for içinde örneklenebilir).
- Her yineleme benzersiz `Span.ctx` taşır (mono'nun `next_ctx` sayacı
  paylaşılır) → resolve'un span anahtarlı tabloları çakışmaz; kaynak
  konumu korunur. ctx → (`var`, `value`, `for_span`, `parent`) kaydı
  `SourceFile::generate.iterations`a yazılır.
- Sınırlar parser sabit değerlendiricisiyle (`eval_const`: literal, üst
  düzey `const`, aritmetik) çözülür: sabit değil E2005, ters aralık E2028,
  > 4096 yineleme E2027 (const-eval.md §8 kodları). Hatalı döngü gövdeyle
  birlikte düşürülür (kaskad bastırma).
- Gövdede `if`/`match` E0003 (öneri: `comb` bloğu ya da `if` ifadesi);
  `<=` zaten parser'da E0007.
- İç içe `for` özyinelemeli açılır; sonek ve ctx zinciri (`parent`) taşınır.
- `on`/`comb`/`stage` gövdesindeki `for` DEĞİŞMEDİ: emitter açar
  (ADR-0041 yolu, `emit_for_in_block`). Modül seviyesi
  `emit_module_for`/`emit_for_body_assigns` silindi; `StmtKind::For`
  emitter'a ulaşmaz.

**SV çıktısı düz açılımdır** (`generate for`/`genvar` yok): mevcut ilke
(araç bağımsız, `generate` bloğu Yosys/Verilator/Vivado arasında farklı
davranır), örnek adları indeks taşıdığından (`pe_1_2`) okunabilirlik
kaybı yok; formal karşı örneklerinde hiyerarşik ad doğrudan kaynağı
gösterir.

### 2. Desugar sırası: mmio → mono(+açılım) → flatten → bidir

`parse_source_file` ve `parse_unit` ortak `finish_unit_desugar`
çağırır. Bundle düzleştirmesi artık mono'dan SONRA koşar: bundle dizisi
indeksleri (`ch[i].valid`) açılımla literal olur, `[Handshake<u8>; K]`
uzunluğu monomorfta literaldir. `fresh_name_span` ve `rw_virtual` sentetik
span'lerde port/öğe `ctx`'ini korur — aynı şablonun iki monomorfu aynı
konumları üretir, ctx ayırır.

### 3. Bundle dizileri

`in ch : [Handshake<u32>; 4]` ya da `[Req; 2]` (kullanıcı `struct port`)
eleman eleman açılır: `ch_0_data`, `ch_0_valid`, `ch_0_ready`, ...
Yeniden yazma anahtarı `ch[k].alan` (`BundleOrigin::port` metni `ch[k]`,
E4005 mesajı bunu gösterir). ADR-0039 yön terslemesi ve ADR-0050 otomatik
protokol kontratları eleman başına uygulanır; sanal alanlar (`ch[1].fired`)
çalışır; `@no_protocol_check` port üzerinde tüm elemanları kapatır.

- İndeks derleme zamanı sabiti olmalı: literal, `const`, modül seviyesi
  `for` değişkeni (açılımda literal). Sinyal indeks **E2008** ("index into
  bundle array 'ch' must be a compile-time constant"; help `for` /
  düz alan mux'ı önerir; reason notu ADR). Aralık dışı sabit indeks de
  E2008 ("out of range"). Uzunluk sabit değil ya da 1..=256 dışı E2008,
  port düşürülür.
- `on`/`comb` içindeki `for` emitter'da açıldığından `comb { for i in
  0..N { ... ch[i].valid ... } }` E2008 alır — sınır. Çözüm: modül
  seviyesi `for` ile `wire valids : [bool; N]` doldurup blokta düz diziyi
  indekslemek (tests/ui/pass/77 deseni).

### 4. Dizi port/wire → paketlenmiş vektör (sv-mapping.md §2 notu)

`[T; N]` tipli port ve `wire` SV'de `logic [N*W-1:0]` olarak üretilir
(`packed_arrays` tablosu; `signal_sig`/`port_sig`); eleman erişimi
`ad[W*i +: W]` (`packed_select`; literal indekste çarpım katlanır:
`a[8 +: 8]`), işaretli eleman okuması `$signed(...)` ile sarılır. Bütün
dizi ataması (`y = a`, örnek çıkışı `s.v`) vektör kopyasıdır. Sinyal
indeks (`a[sel]`) `a[8 * (sel) +: 8]` — bundle dizisinin aksine serbest.
Reg dizileri unpacked kalır (BRAM çıkarımı, ADR-0035). Gerekçe: Yosys
unpacked port reddi (Sorun 3).

### 5. Tanı kalitesi

Klon span'leri kaynak konumunu koruduğundan hata her zaman kullanıcının
yazdığı satırı gösterir. `volt_hir::annotate_generate` birincil span'in
`ctx`'i `generate.iterations`ta ise `= not: in the unrolled 'for' iteration
i = 2 (ADR-0056)` notu ve `for` deyimine ikincil etiket ("'for' loop
unrolled at compile time here") ekler; iç içe döngüde zincir `y = 1, x = 2`.
`analyze()` sonunda ve sürücüde (`compile`, hem başarı hem `fail` yolu)
uygulanır; idempotenttir (im: "(ADR-0056)"). Yineleme başına ayrı tanı
üretilir (3 yineleme → 3 E2003), her biri kendi notuyla.

## Span bütçesi ölçümü (Adım 4)

NoC notu ADR-0039'un sentetik span mekanizmasının ~1.100 üretilmiş bildirimi
taşımayacağından korkuyordu. Bu risk `for` açılımı için GEÇERLİ DEĞİL:
açılım sentetik span üretmez — klonlar kaynak span'ini korur, yalnız
`ctx` (u16) değişir. Ölçüm (unroll_tests
`span_budget_measurement_4x4_mesh_uses_28_contexts`, examples/systolic
deseni):

| Yapı | Yineleme (= ctx) | Üretilen modül deyimi |
|---|---|---|
| `for y in 0..4` | 4 | 4 |
| `for x in 0..4` | 4 | 4 |
| `for y { for x }` | 4 + 16 | 48 (16 örnek + 32 atama) |
| **Toplam** | **28** | **56** |

u16 bütçesi 65.535 ctx / derleme birimi (mono ile paylaşılır); 4×4 bunun
%0,04'ü, 64×64 iç içe 4.160. Bütçe biterse E2027 ("span context budget
exhausted"). Bundle düzleştirmesinin sentetik span'leri (port bildirimi
içi tek karakter, sonra öğe sonundan geriye sıfır genişlik) yalnız düz
port sayısı kadar: 4×4 mesh'te 16 düğüm × 5 sinyal = 80 — ADR-0039
mekanizması yeterli, değişiklik gerekmedi (yalnız ctx korunması eklendi).

## Sonuçlar (examples/systolic/pe_array.volt)

4×4 çıkış-sabit sistolik dizi: `Pe` (a/b operandları bir atlama geciktirilir,
`acc += a*b`, `clr`) ve `PeArray` (`[i8; N]` kenar girişleri, `[i16; N*N]`
çıkışı, `a_link`/`b_link` bağlantı dizileri, doğu/güney kenar çıkışları),
iki iç içe `for` ile:

| Ölçüt | Değer |
|---|---|
| Kaynak | **96 satır** (hedef < 100; elle ~400) |
| Üretilen SV | Pe 49 + PeArray 336 satır, 16 `Pe pe_y_x` örneği |
| Verilator `--lint-only -Wall` | temiz (0 uyarı) |
| `volt verify` (sby Docker) | 5 property: `bmc 8` 1,7 s, `prove 3 --engine boolector` 1,4 s, `cover 12` 1,9 s |

Testler: tests/ui/pass/75 (for içinde örnekleme + `let` teli + dizi port),
76 (2×2 iç içe, katlanan indeks), 77 (Handshake + struct port dizileri,
for ile erişim, sanal alan, kontratlar); tests/ui/fail/58 (E2008 sinyal
indeks). Crate testleri: volt-syntax unroll_tests 17 + bundle_array_tests
10 + parser_tests 3 yeni; volt-hir generate_semantic_tests 10; volt-sv-emit
generate_emit_tests 10. Toplam +54 (hedef +35).

**4×4 mesh artık yazılabilir mi?** Evet (tahmin): NoC notundaki Router
(`[Handshake<NocFlit>; 5]` giriş/çıkış — bundle dizisi) + `Mesh` (iç içe
`for` ile 16 router, `wire` bağlantı dizileri) ≈ 150–200 satır; elle
~1.400. Kalan maliyet Router gövdesinin kendisi ve kenar tie-off'u (edge
elemanlarına sabit atama: `for` dışında 8 satır).

## Sınırlar / Ertelenen

- `on`/`comb`/`fn` gövdesindeki `for` emitter'da açılır; oradan bundle
  dizisi indekslemek E2008 (yukarıdaki düz dizi geçici çözümü).
- Modül seviyesi `for` gövdesinde `if`/`match` yok (E0003); derleme
  zamanı koşullu üretim (`if x == 0 { ... }` kenar durumu) V1 — bugün kenar
  atamaları döngü dışında yazılır.
- Örneklenmemiş generic şablon açılmaz (mono şablonu işlemez); gövdesinde
  for-let varsa HIR `BlockStmt::Let`+`StructLit` görür (mevcut davranış).
- Dizi port/wire: iç içe dizi yok (`array_reg_sig` sınırı), `let x : [T; N]`
  yok, dizi literali (`[0; N]`) wire'a atanamaz; `test` bloklarında dizi
  portu bağlama denenmedi. Örnek çıkışı `s.v[k]` (alan üzerinde indeks)
  desteklenmez — önce `wire`a alınır.
- Bundle dizisi uzunluğu ≤ 256; iç içe bundle dizisi (`[[Req; 2]; 2]`) yok.
- E2008 mesajı düzleştirme sırasında (parse içinde) üretilir; ui/fail
  taraması parser listesinde (parser_tests `ui_fail_files_produce_expected_codes`).
- Döngü değişkeni modül sinyaliyle aynı adda ise ikame kazanır (gölgeleme
  tanısı yok).

## Ölçütler

- [x] `cargo test --all` yeşil; mevcut testler değişmeden (3 parser testi
      yeni davranışa göre güncellendi: açılım artık parse içinde).
- [x] examples/systolic derleniyor, 96 satır, Verilator temiz, 5 kontrat
      bmc/prove/cover kanıtlı.
- [x] İç içe `for` 4×4 çalışıyor; bundle dizileri düzleşiyor; span bütçesi
      ölçüldü (28 ctx), yeterli.
- [x] Tanı notu "for i = k yinelemesinde" + kaynak satırı.
