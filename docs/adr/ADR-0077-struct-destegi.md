# ADR-0077: Struct Desteği — Düz Struct Değerleri Donanıma İner

> Statü: KABUL EDİLDİ — Aşama 1 (tasarım) ve Aşama 2 (uygulama, notlar sonda); Aşama 3
> (örnekler + kanıt) ayrı PR
> Tarih: 2026-09-24
> Etkilenen (plan): volt-ast (ortak struct düzeni `struct_layout`,
> `enum_layout`'un yanında), volt-syntax (bildirimde alan domain
> notasyonunun korunması, test dilinde struct literali, sözleşme/`if`
> bağlamında literal için öneri), volt-hir (bildirim denetimi, literal
> denetimi, alan erişimi, tip kuralları, `consteval` struct değeri,
> `TypeArena::signal_width`, sürücü analizi düzeni, domain W3003, trust),
> volt-sv-emit (sinyal/port düzleştirme, bütün-struct işlemleri,
> örnek portu bağlantısı, `prev`/`sync`), volt-diagnostics (üç YENİ kod —
> numaralar Aşama 2'de; `explain` iki dil), volt-driver (test dili,
> rapor), volt-lsp (hover), `tests/`, `examples/` (Aşama 3), README.md,
> CHANGELOG.md.
> DOKUNULMADI: docs/spec/, docs/research/, kod.
> Genişletir: `grammar-full.ebnf` §7 (StructDecl, "[F3]") ve §12
> (StructLit), `sv-mapping.md` §2 (tip tablosu) ve §14 (bundle
> düzleştirme kuralı struct değerlerine uygulanır), ADR-0039 (bundle),
> ADR-0050 (Handshake payload'ı), ADR-0069 (generic struct E0003),
> ADR-0073 (kısmi hedefler). Spec salt okunur; bu ADR ilgili bölümlerin
> güncel hâlidir.

## Sorun

ADR-0070 ölçümü: struct tipli port/reg/let `volt check`'te "not supported
yet", `volt build`'de E0003. Struct'ı SV'ye taşıyan tek yol `Handshake<T>`
payload'ı — orada parser alanları tek tek porta açıyor (ADR-0050). Sonuç:
birbirine ait sinyaller (çözülmüş komut alanları, paket başlıkları,
konfigürasyon) ayrı ayrı yazılıyor. `examples/riscv_core.volt:194`:

```volt
let opcode = instr[6:0] as u7
let rd_i   = instr[11:7] as u5
let f3     = instr[14:12] as u3
let rs1_i  = instr[19:15] as u5
let rs2_i  = instr[24:20] as u5
```

Enum (ADR-0074) aynı yöntemle eklendi ve sentezde birebir aynı donanımı
ürettiği ölçüldü (PR #31). Struct için de aynı kanıt beklenir.

## Tespit (ADIM 1.1)

Ölçüm `main` d4d8fc5 (PR #33 sonrası), `target/debug/volt.exe`. 65
sondalık külliyat: `build/struct-sv/probe.py <çıktı-dizini>` (`build/`
gitignore'da; her sonda `volt check` + `volt build --target-dir`,
çıktı `build/struct-sv/probe_main.txt`) ve `build/struct-sv/extra/`
altında 12 tek dosyalık sonda (`volt check`). Sondaların çoğu
`struct P { a : u4  b : bool }` ve tek modül kullanır. Her satırda check
ve build aynı kodları verdi (parite tutuyor — ADR-0070).

### Katman katman

| Katman | Durum | Kanıt (sonda → sonuç) |
|---|---|---|
| Parser: bildirim | TAM; alan ayırıcı satır sonu ya da `,`; generic; `struct port` alanı yön ister | `d01` temiz |
| Parser: literal | `P { a: x, b: go }` ve kısa biçim `P { a, b }` ayrışır; `if`/sözleşme koşulunda literal yalnız parantezle (`allow_struct_lit`, `parser/expr.rs:292`) | `d07` ayrışır; `c04` `cover: p == P { … }` → E0001 `unexpected '{', expected a statement`; `c05` `(P { … })` ayrışır |
| Parser: literal ↔ örnek | `let s = Sub { p: … }` modül adında örnek, struct adında literal olur (çözümlemede) | `x01`, `extra/inst_vs_lit` |
| Parser: alan erişimi / ataması | `p.a`, `p.i.x`, `p.a <= x`, `p.a = x`, `p.v[1] <= x` ayrışır | `o02`, `o03`, `s08`, `s09` |
| Bildirim denetimi | **YOK** | `d08` yinelenen alan `{a : u4  a : bool}` ve `d09` boş struct `{ }` **sessiz**; `extra/struct_has_bundle` bundle tipli alan **sessiz**; `extra/field_domain2` alanda `@Fast` → yalnız W0020 `unknown attribute` (domain notasyonu nitelik sanılıyor) |
| Literal denetimi | Bilinmeyen alan E1008; alan tipi E2003; sığmayan literal E2010; **eksik ve yinelenen alan sessiz** | `d05` → E1008 `struct 'P' has no field 'c'`; `d11` → E2003; `d12` → E2010; `d04` (`b` eksik), `d06` (`a` iki kez) → yalnız sinyal tipi E0003 |
| Alan okuma | tip bildirilen alan tipi; **bilinmeyen alan sessiz** (`typeck/select.rs:224` "bilinmeyen alan sessiz Error") | `o10` `p.nope` → yalnız E0003 |
| Alan ataması | alan tipiyle denetlenir | `o11` `p.a <= true` → E2003 `expected 'u4', found 'bool'` |
| Bütün atama | aynı struct geçer; başka struct E2003; tamsayı E2003 | `o04` yalnız E0003; `o12`, `o17`, `o18` (`reg p : P = 0`) → E2003 |
| Tanı metni | **struct adı yok** | `d13`, `o12` → `expected 'struct', found 'struct'`; `o16` → `comparison operands must have the same type: 'struct' and 'struct'` |
| `==`, `!=` aynı struct | tip denetiminden geçer | `o05`, `o06` yalnız E0003 |
| **Sıralama `<`** | **tip denetiminden GEÇER** | `o13` yalnız E0003 (enum'daki `t08` ile aynı açık) |
| Aritmetik | E2003 | `o07` → `incompatible arithmetic operands: 'struct' and 'struct'` |
| Bit seçimi (bütün) | E2003 | `o14` → `bit selection is only allowed on numeric, bits or array types` |
| `if p` | E2003 | `o20` |
| Cast `struct → uN` / `uN → struct` | E2009 | `o08`, `o09` → `cast 'struct' → 'u5' is invalid` |
| `const` struct | E2021 (consteval struct literali değerlendirmiyor) | `s06` → `expected a constant expression` |
| `let p = P { … }` (tipsiz) | E0003 | `d03` → `not supported yet: struct literals ('p = P { ... }')` |
| Sinyal tipi | E0003 (sv-emit `sig_of_typeref` → `describe_user_type`) | `s01`…`s13`, `x01`, `x02`, `x08`, `t01` → `not supported yet: struct type 'P' as a signal type (ports, reg, wire, let)` |
| Reset değeri | parser zorunlu kılar | `s02` → E0001 `expected '=' for the reg initial value` |
| Alan genişliği (emit) | yok | `o01` `p.a == 3` → ek E2005 `cannot determine the width of the literal '3'` |
| Sürücü analizi (ADR-0073) | **alan düzeyinde ÇALIŞIYOR** (`typeck/stmt.rs:315` `LValueSuffix::Field`, soyut düzen) | `r01` `p.a` iki kez, `r02` `p` + `p.a` → E4001 `'p' is already driven` (alan adı yok) |
| Domain / CDC | `sync(p)` genişliği bilinmiyor → **W3003 sessiz** | `c01` → E2005 `cannot determine the width of the sync() source 'p'`, W3003 yok; `c06` (`u5`) W3003 veriyor |
| Handshake payload | alan alan açılır; enum alan, iç içe struct ve dizi alan çalışır; **bütün `req.data` değer değil** | `x03`, `x04`, `x07` temiz (`req_data_i_a`, `req_data_i_b`, `req_data_c`); `x05` `p <= req.data` ve `x06` `rsp.data = P { … }` → E1001 `undefined name: 'req'` |
| Bundle alanı struct | E0003 (sinyal tipi) | `x02`, `x08` |
| Bundle bir sinyal tipi | E0003 | `extra/bundle_as_reg` → `'struct port' bundle 'B' outside a module port` |
| `@mmio` alanı struct | E0001 (dilbilgisi satır içi alan listesi ister) | `extra/mmio_struct` → `expected '{' for the register fields` |
| Test dili | `dut.p.a` = `MemberPath`, yalnız `load()` hedefi; struct literali ayrışmaz | `hir/src/sim_expr.rs:157` `a path into a sub-instance is only valid as the load() target` |
| LSP | `p.` sonrası alan tamamlama VAR (`volt-lsp/src/completion.rs:250`); hover `a : struct` (ADR-0069 açık notu) | kod okuması |

Yanıtlar: parser tam; typeck alan tiplerini, alan/bütün atamayı, aritmetiği
doğru denetliyor ama bildirimi, literalin eksik/yinelenen alanını,
bilinmeyen alan erişimini ve sıralamayı kaçırıyor; tanılarda struct adı
yok. Sürücü analizi alanları zaten ayırıyor. Handshake payload'ı alan
alan açılıyor; bütün payload bir değer değil.

## Karar 1 — `struct` ile `struct port` ayrımı (ADIM 1.2)

**İki ayrı kavram olarak KALIR.**

| | `struct` (bu ADR) | `struct port` (bundle, ADR-0039) |
|---|---|---|
| Ne | tek yönlü veri **değeri** | yön taşıyan **port grubu** |
| Alan | `ad : Tip` — yön ve domain notasyonu yok | `in`/`out ad : Tip [@Domain]` |
| Kullanım | port, `reg`, `wire`, `let`, `const`, ifade, literal, `==`, `as` | yalnız modül portu (başka yerde E0003) |
| Yön | portun yönü bütün alanlara uygulanır | alan başına; `in` port tersler |
| Değer olarak | evet (atanır, karşılaştırılır, register'da tutulur) | hayır |

Gerekçe: bir bundle'ın `ready` alanı ters yönde akar; onu bir register'da
saklamak ya da `==` ile karşılaştırmak anlamsızdır. Tek bir kavramda
birleştirmek ya "yönü yok say" (bundle'ı bozar) ya da "yönlü değer" (tip
sisteminde yeni boyut) demektir. Anahtar sözcük (`port`) farkı zaten
söylüyor; parser ihlali de ayrıyor (`extra/plain_dir_field` → E0001
`expected struct field name (only 'struct port' fields carry a
direction)`).

Kullanıcıya fark açık mı: bildirim tarafında evet (yukarıdaki E0001),
kullanım tarafında **hayır** — bundle'ı sinyal tipi yapmak (`reg r : B`)
E0003 "not supported yet" diyor, sanki bir gün desteklenecekmiş gibi.
Aşama 2: bu mesaj kalıcı kuralı söyler ("a 'struct port' bundle groups
directed port fields; it is not a value — use a plain 'struct' for data")
ve kodu aynı kalır (E0003 → anlamı değişmez, yalnız metin). Bildirimdeki
iki sessiz ihlal (alan domain notasyonu, bundle tipli alan) YENİ
E2xxx-A'dır (Karar 4).

İç içe kullanım:

- **Bundle alanı struct tipli olabilir: EVET.** `struct port B { out d : P
  in r : bool }` → bundle açılımı `bo_d : P` düz portunu verir (yön bütün
  alana), struct açılımı onu `bo_d_a`, `bo_d_b`'ye indirir (Karar 5).
  Sonuç, bugün Handshake payload'ının ürettiği adlarla aynı biçimdedir.
- **Struct alanı bundle olamaz: HAYIR.** `struct P { x : B }` (bugün
  sessiz) → E2xxx-A. Değerin içinde ters yönlü alan olamaz.
- **`Handshake<P>`'de `P` gerçek struct mı: EVET, hep öyleydi** — parser
  aynı `StructDecl`'i açıyor. Bu turda Handshake açılımı **değişmez**
  (SV ve otomatik kontratlar byte-aynı): alanlar `req_data_<alan>` portları
  olarak kalır ve Karar 5'in B eşlemesi bir `req_data : P` portundan tam
  olarak aynı adları üretir. Bütün payload'ı değer olarak kullanmak
  (`p <= req.data`, `rsp.data = P { … }`) bu turda **E0003** alır —
  bugünkü yanıltıcı `E1001 undefined name: 'req'` yerine
  (`not supported yet: the whole Handshake payload as a value; access its
  fields ('req.data.a')`). Gerekçe: payload'ı struct portu olarak
  yeniden ifade etmek ADR-0050 otomatik kontratlarının metnini (alan başına
  "data stable" kuralı) değiştirir; golden'ı bozmadan yapılamaz — ayrı ADR
  (Sınırlar).

## Karar 2 — Kapsam (ADIM 1.3)

**Bu tur — alan tipleri:** `uN`, `iN`, `bits<N>`, `uint<N>`/`int<N>`,
`bool`, `Trit`, birim varyantlı enum (ADR-0074), dizi (`[T; N]`,
ADR-0056 paketlenmiş vektör kuralıyla), iç içe struct, bunlara çözülen
tip takma adları.

**Bu tur — kullanım:** port (modüller arası dahil), `reg`, `wire`, `let`
(tipli ve tipsiz), `const`, bundle alanı (Karar 1), kontrat gövdesi,
`prev()`, `sync()`, `if` ifadesinin kolları.

**Bu turda DEĞİL (açık tanıyla):**

| Konu | Tanı | Gerekçe |
|---|---|---|
| Generic struct sinyal tipi (`G<u4>`) | E0003 (ADR-0069 — değişmez) | tip parametresi ikamesi dilde yok (mono yalnız modül + const generic) |
| **Struct dizisi** (`[P; 4]`) | E0003 `not supported yet: arrays of structs` | aşağıda — ölçüldü |
| Bütün Handshake payload'ı değer olarak | E0003 | Karar 1 |
| `match p { … }` (struct deseni) | E0003 (bugünkü bağlama/desen E0003'ü) | desenler ayrı iş; `match p.s` (enum alan) çalışır |
| `fn` argümanı/dönüşü struct | E0003 (ifade içi çağrı zaten E0003) | fn gövdesi tip denetimsiz (ADR-0075 açık bulgu) |
| `@mmio` register'ı struct tipli | E0001 (bugünkü) | Karar 6 — bit sırası farkı |

### Struct dizisi — KARAR: SONRA

B eşlemesinde (Karar 5) bir struct sinyali alan başına bir SV sinyaline
açılır. Aynı kural bir diziye uygulanırsa `[P; 256]` "dizilerin
struct'ı" (SoA) olur: alan başına ayrı bellek. Ölçüldü
(`build/struct-sv/ram/gen_ram.py`, 256 × 12 bit, senkron yazma + okuma;
AoS = tek `logic [11:0] mem [0:255]`, SoA = beş alan belleği; Yosys 0.66):

```
$ yosys -p "read_verilog -sv aos.sv; synth_ice40 -top Ram; stat"
aos | synth_ice40  |  59 cells; 34 SB_DFF; 24 SB_LUT4; 1 SB_RAM40_4K;
soa | synth_ice40  |  61 cells; 34 SB_DFF; 22 SB_LUT4; 5 SB_RAM40_4K;
aos | synth_xilinx -noiopad |  2 cells; 1 RAMB18E1;
soa | synth_xilinx -noiopad |  53 cells; 12 FDRE; 4 LUT3; 12 LUT6; 24 RAM64M;
```

SoA iCE40'ta **5 kat BRAM**, xc7'de tek RAMB18 yerine **53 hücre**
(24 RAM64M, 12 FDRE, 16 LUT, 1 BUFG). Struct dizisinin doğru eşlemesi eleman başına
paketlenmiş vektördür (AoS — Karar 5'in C biçimi, dizi içinde); bu,
skaler struct'ın B eşlemesinden farklı ikinci bir kural, `mem[i].a`
dinamik indeksli alan erişimi ve RAM çıkarımı ölçümleri ister. Ayrı ADR.
Bit düzeni (Karar 3) aynı kalacağı için o ADR `as` anlamını
değiştirmez.

## Karar 3 — Bit düzeni (ADIM 1.4, normatif)

**İlk alan MSB'de.** Struct'ın bit düzeni alanların bildirim sırasıyla,
derinlik öncelikli yaprak listesidir; ilk yaprak en anlamlı bitlerdedir.
`W` = yaprak genişlikleri toplamı. SV `typedef struct packed` ile aynı.

```volt
struct Inner { x : u3, y : i2 }                       // 5 bit
enum St { Idle, Run, Done }                           // 2 bit
struct P { a : u4, s : St, b : bool, i : Inner }      // W = 12
```

| Yaprak | Bitler |
|---|---|
| `a` | `[11:8]` |
| `s` | `[7:6]` |
| `b` | `[5]` |
| `i.x` | `[4:2]` |
| `i.y` | `[1:0]` |

Yaprak içi: sayısal alanlar kendi ikili gösterimi (işaretli alan ikiye
tümleyen, `as` işaret genişletmez); `bool` 1 bit; `Trit` ADR-0003'ün 2
bitlik kodu; enum ADR-0074 kodu (`W = clog2(n)` ya da taban tipi); dizi
alanı ADR-0056 paketlenmiş vektörü (eleman 0 alanın en düşük bitlerinde).

Bu düzen `p as uN`, `raw as P` ve LSP hover'ındaki bit aralıklarının
**tek** tanımıdır; Aşama 2'de `volt_ast::struct_layout` (ADR-0074
`enum_layout`'un yanında) olarak bir kez yazılır ve HIR, sv-emit, sürücü
analizi, simülasyon raporu ve LSP onu kullanır.

Kanıt (düzen = SV packed struct): Karar 5 deneyinde A2 (`typedef struct
packed`, `12'(s1_q)`) ile C (bu tablodaki dilimlerle elle yazılmış düz
vektör) aynı netlisti verdi (yalnız port sırası farklı — aşağıda).

Gerekçe:
- Dış protokol biçimleri MSB'den yazılır: RISC-V R-tipi komut
  `funct7 | rs2 | rs1 | funct3 | rd | opcode` (bit 31 → 0).
  `struct RType { funct7 : u7, rs2 : u5, rs1 : u5, funct3 : u3, rd : u5,
  opcode : u7 }` (7+5+5+3+5+7 = 32) ve `instr as RType` spec tablosunu
  birebir okur (Aşama 3 adayı).
- Elle SV yazan entegratörün beklentisi ve ileride olası `typedef struct
  packed` çıktısı (Reddedilenler — A) ek dönüşümsüz aynı bitleri görür.
- Verilator `--trace-structs` alanları bu sırayla gösterir (Karar 5 §6).

**`@mmio` farkı (bilerek):** ADR-0044/0053 register alanları **LSB'den**
dizilir (`pins` `lsb: 0`, `_reserved` `lsb: 8` — ADR-0053 regmap
şeması), çünkü register haritaları bit 0'dan belgelenir. İki kural farklı
nesneleri tanımlar ve bu turda birbirine dokunmaz (struct `@mmio`'da
kullanılamaz — Karar 6). Birleştirme ayrı ADR'dedir ve açık bir düzen
seçimi ister.

## Karar 4 — Tip kuralları (ADIM 1.4)

| İşlem | Karar | Tanı | Bugün |
|---|---|---|---|
| Alan okuma `p.a`, `p.i.x` | alan tipi | — | çalışıyor |
| Bilinmeyen alan `p.nope` | hata | E1008 | **sessiz** |
| Alan ataması `p.a <= x`, `p.a = x`, `p.v[1] <= x`, `p.a[1:0] <= …` | alan tipiyle; kısmi hedef (ADR-0073) | E2003 | çalışıyor |
| Bütün atama | yalnız aynı struct tipinin değeri | E2003 | çalışıyor (mesajda ad yok) |
| Struct literali | **bütün alanlar zorunlu**, her alan bir kez, sıra serbest; kısa biçim `P { a, b }` | eksik/yinelenen: **E2xxx-B**; bilinmeyen: E1008; alan tipi: E2003/E2010 | eksik/yinelenen **sessiz** |
| `==`, `!=` aynı struct | izinli, bütün bitler (alan alan eşitliğin birleşimi); sonuç `bool` | — | tipte geçer |
| `==` farklı struct / tamsayı / literal olmayan başka tip | hata | E2003 | E2003 |
| `<`, `<=`, `>`, `>=` | **hata** | E2003 | **geçiyor** (`o13`) |
| Aritmetik, bit düzeyi, kaydırma, tekli işlemler | hata | E2003 | aritmetik E2003 |
| Bütün struct'ta bit/aralık seçimi `p[0]` | hata (alan adını kullan ya da `as`) | E2003 | E2003 |
| `if p`, kontrat gövdesi `p` | `bool` değil | E2003 / E5004 | E2003 |
| `if c { p } else { q }` | izinli, iki kol aynı struct | E2003 | — |
| `p as uN` / `p as bits<N>`, `N ≥ W` | izinli, Karar 3 düzeni, sıfır genişletme | — | E2009 |
| `p as uN`, `N < W` | hata (bilgi kaybı) | E2009 | E2009 |
| `p as iN`, `p as bool`, `p as Q` (başka struct) | hata | E2009 | E2009 |
| `raw as P`, `raw : uN`/`bits<N>`, `N == W`, `P`'nin hiçbir yaprağı enum ya da `Trit` değil | **izinli**, Karar 3 düzeniyle dilimler | — | E2009 |
| `raw as P`, `N != W` | hata (örtük kesme/genişletme yok) | E2009 | E2009 |
| `raw as P`, `P` (iç içe dahil) enum ya da `Trit` yaprağı içeriyor | **hata** + öneri: alan alan kur | E2009 | E2009 |

**`bits → struct` ve enum/Trit alanı: YASAK.** ADR-0074 `uN → enum`'u
yasakladı: enum tipli bir değer inşa gereği geçerli bir varyanttır; F1 ve
kapsayıcı `match` buna dayanır. Enum alanlı bir struct'a ham bit dökmek
bu kuralı dolaylı yoldan deler (`(raw as P).s` geçersiz kod olabilir).
Aynı gerekçe `Trit` için geçerli: `u2 as Trit` bugün E2009
(`extra/trit_cast` → `cast 'u2' → 'Trit' is invalid`; `10` kodu
geçersiz). Değerlendirilen seçenekler:

- *Yasak* — seçildi. E2009 önerisi çözmeyi açık yazar:
  ```volt
  let hdr : Hdr = Hdr { kind: kind_of(raw[7:6]), len: raw[5:0] as u6 }
  ```
  (`kind_of` bir `comb`/`match` ile — ADR-0074 kalıbı.)
- *İzinli, enum alanlarına kontrat* — reddedildi (ADR-0074 Karar 3 ile
  aynı: kaynak port ise çevreye bağlı, sessiz `assume` ispatı yanlışlar).
- *Yalnız enum alanı olmayan kısmı dök* — `as` tek ifade olarak kalmalı;
  kısmi döküm yazımı literal + alan dilimidir, yukarıdaki öneri.

Enum ya da `Trit` yaprağı olmayan struct'larda `raw as P` serbesttir ve
değerlidir: bütün bit örüntüleri geçerlidir (`instr as RType`).

**Literalde bütün alanlar zorunlu:** donanımda her bitin kaynağı açık
olmalı. Örtük sıfır (Rust `..Default::default()` benzeri) reset değerinde
bir alanı sessizce sıfırlar, `comb`'da ise "unutulan alan" hatasını
gizler. Varsayılan değerli alanlar ve `..base` güncelleme sözdizimi
sonraya (Sınırlar); bugün `p.a <= x` alan ataması aynı işi görür.

**Reset değeri: zorunlu** (parser her `reg` için zaten ister — `s02`); bir
sabit struct değeri olmalı: sabit alanlı literal ya da `const` struct
(E2021 aksi hâlde). `reg p : P = 0` → E2003 (bugün de — `o18`). Alan
başına reset, SV'de alan başına reset atamasına iner (Karar 5).

**`const` struct:** `consteval` struct literalini `ConstValue::Struct`
olarak değerlendirir, `K.b` alan erişimi sabit ifadedir (bugün `s06`
E2021).

**Tanı metinlerinde struct adı:** E2003/E2009 `'struct'` yerine `'P'`
göstermeli (`o12`, `o16` iki farklı struct için `'struct' and 'struct'`
diyor). ADR-0074 enum için aynı düzeltmeyi yaptı (`display_named`).

**Bildirim kuralları (YENİ E2xxx-A — geçersiz struct bildirimi):**

| Kural | Tanı | Bugün |
|---|---|---|
| Alansız struct (`struct P { }`) — `W = 0` sinyal olamaz | E2xxx-A | sessiz (`d09`) |
| Düz struct alanında domain notasyonu (`a : u4 @Fast`) | E2xxx-A + öneri: sinyalin kendisine yaz ya da `struct port` | W0020 `unknown attribute` (`field_domain2`) |
| Düz struct alanı bundle (`x : B`, `B` bir `struct port`) | E2xxx-A | sessiz (`struct_has_bundle`) |
| Yinelenen alan adı | E1003 | sessiz (`d08`) |
| Düz struct alanında yön (`in b : bool`) | E0001 (bugünkü, doğru metin) | E0001 — ama `@Fast`'tan sonra gelirse sessiz (Yan bulgu 2) |

Alansız struct'ın bildirimi değil **sinyal tipi olarak kullanımı** mı
hata olmalı? Bildirim: sıfır alanlı bir değer tipi Volt'ta hiçbir işe
yaramaz (Handshake payload'ı olarak da `W = 0` port üretir) ve ADR-0074
varyantsız enum'u bildirimde reddetti — aynı kural. Mevcut testlerde
alansız struct yok (`grep -rnE "struct [A-Za-z_]+ *\{ *\}" tests/
examples/` boş).

## Karar 5 — SV eşlemesi (ADIM 1.5)

Seçenekler:

- **A** — `typedef struct packed` bir SV paketinde (`<Struct>_pkg.sv`),
  sinyaller `P_pkg::P`, alan erişimi `p.a`.
- **B** — alanları ayrı sinyallere aç (bundle/Handshake kuralı):
  `p_a`, `p_s`, `p_b`, `p_i_x`, `p_i_y`.
- **C** — tek düz vektör `logic [W-1:0] p` + alan dilimleri `p[11:8]`.

### Deney

`build/struct-sv/` (gitignore'da; üreteç `gen.py`). Aynı tasarım üç
biçimde: `Stage` (struct giriş ve çıkış portu, struct register'ı, alan
alan `comb` sürücülü struct `wire`'ı, bütün atama `p <= w`, iç içe alan
ataması `p.i.x <= p.i.x + 1`, enum alanında `case`, `p == d` eşitliği,
işaretli alan `p.i.y < 0`) ve `Top` (iki `Stage` örneği zincirde — struct
portu modüller arası, ADR-0024 modül başına dosya; `raw = s1.q as u12`).
Bit düzeni Karar 3. Enum alanı ADR-0074 `localparam` kuralıyla.

Araçlar: `verilator/verilator:latest` (5.050), `hdlc/yosys:latest`
(Yosys 0.66), `hdlc/formal:latest` (SymbiYosys, Yosys 0.36+42 — `volt
verify`'ın imajı). Bütün komutlar `MSYS_NO_PATHCONV=1 docker run --rm -v
C:/Dev/volthdl/build/struct-sv:/work -w /work/<A|A2|B|C> <imaj> …`.

**1. Yosys A'yı üç yerden reddediyor.** A'nın doğal biçimi struct başına
bir paket (ADR-0074'ün `State_pkg` biçimi); iç içe struct başka paketin
tipine başvurur:

```
$ yosys -q -p "read_verilog -sv Inner_pkg.sv P_pkg.sv Stage.sv Top.sv; synth -top Top"   # A
P_pkg.sv:8: ERROR: Unknown identifier `\Inner_pkg::Inner' used as type name
$ … paket içinde import Inner_pkg::*;                                                  # A3
P_pkg.sv:4: ERROR: syntax error, unexpected TOK_IMPORT
$ … tek paket (Inner + P), reset '{a: 4'd0, s: St_Idle, …}                             # A2
Stage.sv:27: ERROR: syntax error, unexpected ':'
```

Yosys 0.66 paketler arası tip başvurusunu, paket içi `import`'u ve
atama örüntüsünü (`'{a: …}`) okumuyor. Çalışan tek A biçimi (**A2**):
birimin bütün struct'ları **tek pakette**, literal düz birleştirme
`{4'd0, St_Idle, 1'b0, 3'd0, 2'd0}`.

**2. Verilator `-Wall`:**

```
$ verilator --lint-only -Wall --top-module Top Inner_pkg.sv P_pkg.sv Stage.sv Top.sv   # A
- Verilator: Built from 0.073 MB sources in 5 modules, into 0.021 MB in 3 C++ files
$ verilator --lint-only -Wall --top-module Top Stage.sv Top.sv                          # B
- Verilator: Built from 0.048 MB sources in 3 modules, into 0.026 MB in 3 C++ files
$ verilator --lint-only -Wall --top-module Top Stage.sv Top.sv                          # C
- Verilator: Built from 0.046 MB sources in 3 modules, into 0.021 MB in 3 C++ files
```

A2 de uyarısız. Dördü de temiz; Verilator paket sırasına duyarsız
(`P_pkg.sv Inner_pkg.sv …` ters sırada da temiz).

Kısmen kullanılan struct giriş portu (`build/struct-sv/partial/`):

```
PB  (B, d_b okunmuyor): %Warning-UNUSEDSIGNAL: PB.sv:4:24: Signal is not used: 'd_b'
PC  (C, d[0] okunmuyor): %Warning-UNUSEDSIGNAL: PC.sv:3:24: Bits of signal are not used: 'd'[0]
PB2 (B, yalnız d_b çevresinde lint_off UNUSEDSIGNAL): temiz
```

Bir modülün struct'ın bazı alanlarını kullanması olağandır (çözülmüş
komutun tüketicisi). B susturmayı tam o alana daraltabilir; A/C yalnız
bütün portu susturabilir (tamamen kullanılmayan port da gizlenir).

**3. Yosys sentezi (0.66):**
`read_verilog -sv …; synth -top Top -flatten; opt_clean -purge; rename -enumerate; write_verilog -noattr net.v; stat; synth_ice40 -top Top; stat`
ve ayrıca `synth_xilinx -top Top -flatten -noiopad; stat`:

| | genel hücre | iCE40 | xc7 |
|---|---|---|---|
| A2 | 103 | 47 SB_LUT4, 18 SB_DFFESR, 6 SB_DFFSR (71) | 84: 2 CARRY4, 24 FDRE, 11 LUT2, 8 LUT3, 4 LUT4, 12 LUT6, 11 MUXF7, 5 MUXF8 |
| B | 104 | 47 SB_LUT4, 18 SB_DFFESR, 6 SB_DFFSR (71) | 80: 2 CARRY4, 24 FDRE, 11 LUT2, 8 LUT3, 3 LUT4, 12 LUT6, 9 MUXF7, 4 MUXF8 |
| C | 103 | 47 SB_LUT4, 18 SB_DFFESR, 6 SB_DFFSR (71) | 84: A2 ile aynı |

```
$ diff <(grep -v "^/\*" A2/net.v) <(grep -v "^/\*" C/net.v)
2c2
< module Top(clk, rst, ld, raw, hit, neg, d);
---
> module Top(clk, rst, ld, d, raw, hit, neg);
(… yalnız 'input [11:0] d' bildiriminin yeri)
```

A2 ve C netlisti port sırası dışında **byte-aynı** — Yosys struct tipli
portu port listesinin sonuna taşıyor (A'da netlist port sırası kaynak
sırası değil). B genel hücrede 104 (bir `$_XOR_` grubu yerine
`$_AND_`/`$_MUX_`), iCE40'ta aynı, xc7'de **4 hücre daha az** (1 LUT4,
2 MUXF7, 1 MUXF8). Mantık aynı (üç sürüm aynı testbench'te aynı çıktıyı
verdi — §6); fark ABC'nin farklı ağ yapısından aldığı başlangıç
noktasıdır, B'ye özgü bir üstünlük iddia edilmez. Aşama 3 karşılaştırması
(struct'lı ve struct'sız örnek) bu yüzden **aynı biçimin** iki yazımını
karşılaştırır: struct'sız örnek zaten B biçiminde (ayrı sinyaller), B
seçilirse iki SV'nin birebir aynı olması beklenir — ölçülecek.

**4. Dosya sırası — ADR-0074'ün ayırt edici ölçümü burada da geçerli.**

```
$ yosys -q -p "read_verilog -sv Stage.sv; read_verilog -sv P_pkg.sv; read_verilog -sv Top.sv; hierarchy -check -top Top"   # A2
Stage.sv:7: ERROR: syntax error, unexpected TOK_PACKAGESEP, expecting ')' or ',' or '='
$ yosys -q -p "read_verilog -sv P_pkg.sv; read_verilog -sv Stage.sv; read_verilog -sv Top.sv; hierarchy -check -top Top"   # A2
(temiz)
$ yosys -q -p "read_verilog -sv Top.sv; read_verilog -sv Stage.sv; hierarchy -check -top Top"   # C (B aynı biçim)
(temiz)
```

Bu denemede paket adı (`P_pkg`) modül adlarından alfabetik olarak önce
geldiği için `sorted(glob)` şans eseri çalışır; `struct Req` + `module
Master` (`Master.sv` < `Req_pkg.sv`) kırılır. `scripts/sta/run.py:166`
(`sorted(p.name for p in rtl.glob("*.sv"))`, dosya başına
`read_verilog -sv`) tam bu akış. Üstelik §1'e göre A tek birim paketi
gerektirdiğinden her modül dosyası o pakete bağımlıdır — ADR-0024'ün "her
`.sv` kendi başına yeterli" ilkesi A'da hiç sağlanmaz.

**5. Formal (Yosys 0.36, `hdlc/formal`):** `read -formal …; prep -top
Top` A2, B, C'de hatasız. `Stage`'e `always_comb if ($initstate) assume
(rst); always_comb if (!rst) assert (p.s != 2'd3);` ve bir cover
(`build/struct-sv/FA2`, `FC`, `s.sby`: prove depth 4, cover depth 12,
boolector):

```
A2: [s_prv] summary: successful proof by k-induction.   DONE (PASS, rc=0)
    [s_cov] Reached cover statement at Stage.sv:51 …      DONE (PASS, rc=0)
C : [s_prv] summary: successful proof by k-induction.   DONE (PASS, rc=0)
    [s_cov] Reached cover statement at Stage.sv:51 …      DONE (PASS, rc=0)
```

Struct üye erişimi (`p.s`, `p.i.x`) Yosys 0.36 formal ön ucunda okunuyor;
biçimin formal'e etkisi yok.

**6. Dalga formu (VCD) ve simülasyon:** aynı uyarıcı (`tb.sv`),
`verilator --binary --trace --timing` (`volt run` bayrağı `--trace`,
`crates/volt-driver/src/sim/verilator.rs:102`):

```
A2: raw=344 hit=0 neg=1      B: raw=344 hit=0 neg=1      C: raw=344 hit=0 neg=1

A2 (s0):  $var wire 12 $ p [11:0] $end          ← alan adı yok
          $var wire 12 1 w [11:0] $end
B  (s0):  $var wire 4 + p_a [3:0] $end
          $var wire 2 , p_s [1:0] $end  …       ← her alan adıyla
C  (s0):  $var wire 12 $ p [11:0] $end          ← alan adı yok
```

A2 alan adlarını yalnız ek bayrakla gösterir:

```
$ verilator --binary --trace --trace-structs … && grep scope wave.vcd     # A2
   $scope module p $end
    $var wire 4 $ a [3:0] $end
    $var wire 2 % s [1:0] $end
    $var wire 1 & b $end
    $scope module i $end
     $var wire 3 ' x [2:0] $end
     $var wire 2 ( y [1:0] $end
```

B adları bayraksız ve her simülatörde taşır (düz sinyaldir); C hiçbir
koşulda taşımaz.

**7. Verilator C++ (`volt test` akışı, `--cc` + `public_flat_rw`):**
A2'de `SData/*11:0*/ Top__DOT__s0__DOT__p;` — struct düz tamsayı; test
donanımı (ADR-0058 `rootp`) A ve C'de alanı kaydırma+maske ile, B'de
doğrudan sinyal adıyla okur.

### Karar: **B — alanları ayrı sinyallere aç**

| Ölçüt | A (paket) | B (düzleştirme) | C (düz vektör) |
|---|---|---|---|
| Yosys 0.66 okuma | yalnız A2 (tek paket, düz literal); paketler arası tip, paket içi `import`, `'{…}` **ERROR** | temiz | temiz |
| Verilator `-Wall` | temiz | temiz | temiz |
| Sentez (iCE40) | 71 | 71 | 71 |
| Netlist | C ile aynı, port sırası değişir | aynı mantık | A2 ile aynı |
| Formal (Yosys 0.36) | okur, kanıtlar | okur | okur, kanıtlar |
| Glob/alfabetik okuma (Yosys) | **kırılır** | çalışır | çalışır |
| Her `.sv` kendi başına yeterli (ADR-0024) | **hayır** | evet | evet |
| İ1 isim korunumu (`sv-mapping.md` §0) — SDC/XDC alanı adlandırabilir | SV'de `p.a`, netlistte ad yok | **`p_a`** (bundle kuralı) | **yok** (`p[11:8]`) |
| VCD'de alan adı | yalnız `--trace-structs` | **her zaman** | hiçbir zaman |
| Kısmi kullanımda lint susturma | bütün port | **alan başına** | bütün port |
| Handshake/bundle ile tutarlılık | farklı | **aynı adlar** (`req_data_a`) | farklı |
| Dış entegratörün gördüğü | `P_pkg::P` (paket şart) | `d_a`, `d_s`, … ayrı portlar | `logic [11:0] d` + yorum |
| Bütün-struct işlem metni | kısa (`p == d`) | uzun (`{p_a, …} == {d_a, …}`) | kısa |
| Struct dizisi | — | SoA → BRAM kaybı (Karar 2) | AoS doğal |

Gerekçe: A'nın Yosys'te üç ayrı kırılması ve dosya sırası bağımlılığı
ölçüldü; ADR-0074 aynı nedenle paketi reddetti. B ile C arasında donanım
aynı; B, İ1'i (SDC alanı adıyla hedefleyebilir — `p_a`), dalga formunda
adları, alan başına lint susturmayı ve dilin mevcut iki düzleştirme
kuralıyla (bundle ADR-0039, Handshake ADR-0050) tutarlılığı sağlar. C'nin
tek üstünlüğü bütün-struct işlemlerinin kısa SV metni ve dizilere
doğallığıdır; ikincisi struct dizisi ADR'sinde eleman düzeni olarak
kullanılacak (Karar 2). Okunabilirlik (ADR-0012) B'de: kaynak `p.i.x` →
SV `p_i_x`, aynı kural her yerde.

Geri dönülebilir: tipli SV arayüzü isteyenler için ileride
`--sv-struct=packed` (A2 biçimi, tek birim paketi + `files.f`) — ayrı
ADR; bit düzeni (Karar 3) aynı olduğundan anlam değişmez.

### B'nin kuralları

```systemverilog
module Stage (
    input  logic              clk,
    input  logic              rst,
    input  logic              ld,
    // struct P d : a[11:8] s[7:6] b[5] i.x[4:2] i.y[1:0]
    input  logic [3:0]        d_a,
    input  logic [1:0]        d_s,    // St
    input  logic              d_b,
    input  logic [2:0]        d_i_x,
    input  logic signed [1:0] d_i_y,
    ...
    // struct P p : a[11:8] s[7:6] b[5] i.x[4:2] i.y[1:0]
    logic [3:0]        p_a;
    ...
            if (ld) begin
                p_a <= w_a;          // p <= w
                p_s <= w_s;
                ...
    assign hit = {p_a, p_s, p_b, p_i_x, p_i_y} == {d_a, d_s, d_b, d_i_x, d_i_y};
```

1. **Ad:** struct tipli `x`'in her yaprağı `x_<alan>[_<altalan>…]`
   (ADR-0039 §14 kuralı); başka bir tanımlayıcıyla çakışma **E1003**
   (bundle ve Handshake'teki mevcut davranış: `extra/bundle_collision`,
   `extra/hs_collision` → `'bo_d' is already defined in this scope`).
2. **Sıra:** yapraklar bildirim sırasıyla, sinyalin bildirildiği yerde;
   portlarda §1'in in/out gruplaması düz portlara uygulanır (ADR-0039).
   Grubun başında düzen yorumu `// struct P d : a[11:8] …` — `as`
   anlamını (Karar 3) entegratöre belgeler (İ5).
3. **Yaprak tipi:** mevcut `sig_of_typeref` (işaretli alan
   `logic signed`, enum `// <Enum>` yorumu ve ADR-0074 `localparam`'ları,
   `Trit` 2 bit işaretli, dizi alan ADR-0056 paketlenmiş vektör).
4. **Port yönü:** `in p : P` bütün yaprakları `input`, `out` → `output`
   (bundle'daki `in` terslemesi YOK — alanlar yön taşımaz).
5. **Okuma:** `p.i.x` → `p_i_x`; alt struct değeri `p.i` bir yaprak
   grubudur (aşağıdaki bütün-struct kuralları).
6. **Bütün atama** `p <= e` / `p = e` → yaprak başına atama, aynı blokta,
   bildirim sırasıyla; `comb`/sürekli atamada yaprak başına `assign`.
7. **Literal** → yaprak başına değer; reset değeri yaprak başına reset
   ataması.
8. **`==`/`!=`** → iki tarafın bildirim sırası birleştirmesi
   `{p_a, …} == {q_a, …}` (literal tarafı sabit birleştirme).
9. **`p as uN`** → `N'({p_a, p_s, p_b, p_i_x, p_i_y})` (`N == W`'de
   dönüşümsüz birleştirme); **`raw as P`** → yaprak başına dilim
   (`raw[11:8]`, …) — Karar 3 düzeni.
10. **`if c { p } else { q }`** → yaprak başına üçlü.
11. **Örnek portu bağlantısı** `Sub { d: e }` → `.d_a(…), .d_s(…), …`;
    örnek çıkışı `s0.q` → `s0_q_a`, … (bugünkü `dec_cmd` biçimi).
12. **`sync(p, clk)`** → yaprak başına senkronizör (W3003 kuralı Karar 6);
    **`prev(p)`** → yaprak başına `$past`/yardımcı register
    (`past_p_a_1`, …, ADR-0040).
13. **Kullanılmayan yaprak:** bir modülün hiç okumadığı struct giriş
    portu yaprağı ve hiç okunmayan iç struct sinyali yaprağı
    `// verilator lint_off UNUSEDSIGNAL` / `lint_on` ile çevrilir (yorum:
    `struct field unused in this module`) — §2 ölçümü, ADR-0076'nın
    kullanılmayan reset portu kalıbı.
14. **Düzleştirme bütçesi:** yaprak sayısı ADR-0067'nin sınırlarına tabi
    (`MAX_FLAT_PORTS = 4096`, `MAX_NESTING = 8`); aşım **E4010**. (E4009
    döngüleri eler ama elmas iç içelik `A { x: B, y: B }` yaprakları
    üstel çoğaltır.)
15. **Struct kullanmayan modüllerde çıktı byte-aynı** (golden).

## Karar 6 — Diğer katmanlar (ADIM 1.6)

### Sürücü analizi (ADR-0073)

Alan atamaları kısmi hedeftir ve bugün zaten öyle işler (`r01`, `r02` →
E4001). Aşama 2:
- Soyut düzen (`typeck/stmt.rs:364` "yalnız ayrıklık için") ortak
  `struct_layout`'a geçer — ayrıklık aynı kalır, tek tanım.
- E4001 mesajı alan yolunu adlandırır: `'p.a' is already driven`
  (bugün `'p'`).
- `p.a` + `p.b` ayrı `comb` deyimlerinden ya da ayrı bloklardan sürülür →
  temiz; `p.a` iki kez → E4001; `p` + `p.a` → E4001.
- **Sürülmeyen alan (YENİ E4xxx-A):** `comb` ile alan alan sürülen bir
  struct `wire`'ı / `out` portu / tipli `let`'i, bir yaprağı hiç
  sürülmüyorsa hata (`struct field 'p.b' is never driven`). Gerekçe:
  literaldeki "bütün alanlar zorunlu" kuralının alan alan yazımdaki
  karşılığı; sürülmeyen yaprak SV'de X/UNDRIVEN'dir (ADR-0008 x üretmez).
  Register'lar kapsam dışı: sürülmeyen yaprak değerini korur, reset
  değeri literal ile tamdır. Sayısal vektörlerin kısmi sürülmesi bugün
  sessiz (Yan bulgu 3) — bu tur yalnız struct yaprakları; genelleme ayrı
  karar (golden'ı değiştirir).

### Domain / CDC

- Struct tipli sinyal **tek sinyaldir**: bütün yaprakları sinyalin domain'inde.
  Alan başına domain yok (düz struct alanında notasyon E2xxx-A — Karar 4);
  bundle'ın E3013'ü düz struct'a uygulanmaz.
- `TypeArena::signal_width(Ty::Struct)` = `W` (ADR-0074'ün enum için
  eklediği yol). `W > 1` struct `sync()` → **W3003** (bugün sessiz +
  E2005 — `c01`), sayısal ile aynı öneri (Handshake / AsyncFifo).
  1 bitlik struct (`struct F { b : bool }`) uyarı almaz.
- Hedefli SDC (ADR-0065) `sync()` hedeflerini ada göre üretir; struct
  kaynağında yaprak başına senkronizör, SDC satırları yaprak adlarıyla
  (`p_a`, …). Yeni kısıt türü yok.

### Güven seviyesi (ADR-0052)

**Struct tek seviye taşır** (sinyal başına; alan notasyonu yok).
Sınıflandırılmamış struct register'ı yazılan değerlerin en yükseğini alır:
`p.a <= secret` + `p.b <= public` → `p` secret (join, muhafazakâr).
`declassify(p)` bütün struct'ı indirir. Alan başına seviye reddedildi:
seviyesi farklı veriler iki ayrı sinyal olmalı; alan başına seviye güven
denetimini alan duyarlı yapar (ADR-0052 çıkarımı sinyal düzeyinde) —
kazancı ölçülmedi.

### Formal

- Kontratta alan erişimi (`invariant: p.a <= 9`), bütün eşitlik
  (`cover: p == (P { a: 3, b: true })`), `prev(p) == …` desteklenir;
  sv-emit B kurallarıyla iner.
- `if`/kontrat koşulunda parantezsiz literal (`c04`) E0001 kalır
  (Rust'taki `if x == S { … } {` belirsizliği, `allow_struct_lit`), ama
  tanı öneri alır: `wrap the struct literal in parentheses: (P { … })`.
- **ADR-0066 otomatik kontratları struct alanlarına üretilmez:** FSM ve
  sayaç tanıma bugün düz register ister; `match p.s` ya da `p.n <= p.n + 1`
  için F1/F2/F3/sayaç kontratı bu turda yok (belgelenir; yanlış kontrat
  üretilmediği Aşama 2 testiyle sabitlenir). Gelecek iş.
- Struct giriş portuna otomatik `assume` eklenmez (ADR-0074 ile aynı).
- ADR-0050 Handshake kontratları değişmez (Karar 1).

### Simülasyon / test dili

- `dut.q.a` çıkış struct portunun yaprağını okur → düz port `q_a`;
  `dut.p.a = 3` giriş struct portunun yaprağına yazar. Yön kuralları
  portlarınkiyle aynı (E8503 / E8504). Bugünkü
  `MemberPath` (yalnız `load()` hedefi) ilk parçası DUT'un struct portuysa
  alan yolu olarak çözülür; alt örnek yolu (`dut.sub.x`) değişmez.
- Test dilinde struct literali (`P { a: 1, b: true }`, yeni
  `TestExprKind`): `dut.p = P { … }` bütün yazma,
  `assert_eq(dut.p, P { a: 1, b: true })` bütün karşılaştırma.
- Rapor alan adlarını basar ve farkı işaretler:
  `left=P { a: 3, b: true } right=P { a: 1, b: true } (differs: a)`;
  enum yaprağı ADR-0074 biçimiyle (`s: St::Run`). Testbench çıktısı
  (`VOLT-ASSERT-FAIL … left= right=`) bütün bitleri taşır; adlar sürücü
  raporunda Karar 3 düzeniyle eklenir.
- Tip denetimi mevcut kodlarla, yeni kod gerekmez: bilinmeyen alan
  E8502 (bilinmeyen port ailesi), başka struct'ın literali ya da struct'ı
  sayıyla karşılaştırma E8511, eksik literal alanı E8511.

### `@mmio` / HW-SW köprüsü

**SONRA.** `@mmio` register alanları satır içi, LSB'den dizilen bir
kayıttır (Karar 3); adlandırılmış struct tipi yazılamaz (`extra/mmio_struct`
→ E0001). Birleştirme (register'ı struct tipiyle bildirmek, struct'ı
ADR-0053 Rust/C üretecine taşımak) açık bir düzen seçimi (`@layout`
niteliği ya da `@mmio`'da MSB kuralı) ve ADR-0063 tutarlılık denetimi
değişikliği ister — ayrı ADR. Bu turda E0001 kalır.

### LSP

- Hover: struct sinyalinde `p : P (12 bits)` ve düzen
  (`a: u4 [11:8], s: St [7:6], …`); alan erişiminde
  `p.i.x : u3 — bits [4:2] of p`. Bugünkü `a : struct` (ADR-0069 açık
  notu) `display_named` ile düzelir.
- Tamamlama: `p.` sonrası alanlar zaten var (`completion.rs:250`); iç
  içe `p.i.` Aşama 2 testiyle sabitlenir.
- Tanılar paylaşılan boru hattından gelir; yeni LSP yolu yok.

### Parite (ADR-0070)

Yeni tanıların hepsi tip denetçisinde, sürücü analizinde ya da çıktısız
emit'te (E1003 SV ad çakışması, E4010 bütçe) — check = build = LSP
korunur. Her yeni tanıya bir parite sondası (`st*` dizisi); struct
sinyal tipli mevcut sondaların beklentisi (`// parity:` başlığı) E0003 →
temiz olarak güncellenir, sonda silinmez.

## Tanı özeti

| Kod | Durum | Tetik |
|---|---|---|
| E0001 | mevcut | parantezsiz literal `if`/kontrat koşulunda — öneri eklenir; düz struct alanında yön |
| E0003 | mevcut, kapsam daralır | generic struct sinyal tipi, struct dizisi, bütün Handshake payload'ı değer olarak (bugünkü E1001 yerine), struct deseni, fn'de struct; bundle sinyal tipi olarak (metin kalıcı kuralı söyler) |
| E1003 | mevcut | yinelenen alan adı; SV ad çakışması `x_<alan>` |
| E1008 | mevcut, kapsam genişler | bilinmeyen alan erişimi `p.nope` (bugün sessiz) |
| E2003 | mevcut, kapsam genişler | sıralama, farklı struct; mesajlarda struct adı |
| E2009 | mevcut, kapsam daralır | dar `struct → uN`, `struct → iN/bool/struct`, `N != W`, enum/`Trit` yapraklı `bits → struct` |
| E2010 / E2021 | mevcut | literal alanı sığmıyor / reset ya da `const` sabit değil |
| **E2xxx-A** | **YENİ** — geçersiz struct bildirimi | alansız struct, düz struct alanında domain notasyonu, bundle tipli alan |
| **E2xxx-B** | **YENİ** — eksik ya da yinelenen literal alanı | `P { a: x }` (eksik `b`), `P { a: x, a: y, b: z }` |
| E4001 | mevcut | çakışan alan sürücüleri; mesaj alan yolunu adlandırır |
| **E4xxx-A** | **YENİ** — sürülmeyen struct alanı | alan alan `comb` sürülen struct'ta sürülmeyen yaprak |
| E4010 | mevcut, kapsam genişler | struct düzleştirme bütçesi |
| W3003 | mevcut, struct'a genişler | `W > 1` struct `sync()` |

Üç yeni kodun numaraları Aşama 2'de boş numaralardan tahsis edilir
(tasarım ADR'sinde numara yazılmaz — `just consistency` ADR'deki her kodu
tanı enum'unda arar). Hepsi iki dilde mesaj + `volt explain`.

## Reddedilenler

- **SV eşleme A (paket + `typedef struct packed`)** — Karar 5: Yosys
  0.66 paketler arası tipi, paket içi `import`'u ve atama örüntüsünü
  okumuyor; tek paketli biçimde bile glob/alfabetik okuma kırılır (ölçüldü).
- **SV eşleme C (düz vektör)** — İ1 isim korunumunu bozar (alan SV'de
  adsız), VCD'de alan adı yok, lint susturma alan başına yapılamaz
  (ölçüldü); donanım B ile aynı.
- **Tek kavram (struct = bundle)** — Karar 1: yön taşıyan değer anlamsız.
- **Handshake payload'ını bu turda struct portuna çevirmek** — Karar 1:
  ADR-0050 kontrat metni değişir, golden kırılır.
- **Struct dizisi B kuralıyla (SoA)** — Karar 2: iCE40'ta 5× BRAM, xc7'de
  blok RAM yerine LUTRAM (ölçüldü).
- **İlk alan LSB** — Karar 3: SV packed struct, protokol şemaları ve
  `--trace-structs` MSB'den; `@mmio` farkı bilerek ayrı tutuldu.
- **Kısmi literal / örtük sıfır** — Karar 4: reset'te ve `comb`'da
  unutulan alanı gizler.
- **Enum/`Trit` yapraklı `bits → struct`** — Karar 4: ADR-0074 `uN →
  enum` yasağını dolaylı deler.
- **Struct sıralaması (`<`)** — anlamı tanımsız; `as uN` ile açık yazılır.
- **Alan başına domain / güven seviyesi** — Karar 6: sinyal düzeyinde
  kurallarla çelişir, ayrı sinyal yazımı açık.

## Uygulama planı

### Aşama 2 — uygulama (dal `feat/struct`, main'den; ADR'ye sadık, sapmada DUR)

1. **Düzen** (volt-ast): `struct_layout` — yapraklar, `W`, bit aralıkları
   (Karar 3), enum/`Trit` yaprağı var mı, bütçe (E4010). `enum_layout`
   ile aynı yerde; HIR, sv-emit, sürücü analizi, sim, LSP ortak kullanır.
2. **Bildirim + literal + alan** (volt-hir): E2xxx-A, E1003 alan, E2xxx-B,
   E1008 bilinmeyen alan erişimi; parser alan domain notasyonunu AST'de
   korur (bugün nitelik sanılıyor).
3. **Tipler** (volt-hir): sıralama E2003; cast kuralları (Karar 4); tipsiz
   `let` literal çıkarımı; `if` ifadesi struct kolları; mesajlarda
   `display_named`; `signal_width` struct; `consteval` `ConstValue::Struct`
   + alan erişimi.
4. **SV** (volt-sv-emit): `sig_of_typeref` struct → yaprak listesi; B
   kuralları 1–15; dizi/generic/Handshake-bütün E0003; E1003 ad çakışması;
   lint susturma.
5. **Sürücü analizi** (volt-hir): ortak düzen, E4001 mesajında alan yolu,
   E4xxx-A.
6. **Domain / trust / formal**: W3003; trust join; kontrat ve `prev`;
   ADR-0066'nın struct alanlarına kontrat üretmediğini sabitleyen test;
   parantezsiz literal önerisi.
7. **Test dili / rapor** (volt-syntax + volt-hir + volt-driver), **LSP**
   (hover düzeni, iç içe tamamlama).
8. **Tanılar** (volt-diagnostics): üç yeni kod, iki dil, `explain`;
   bundle-sinyal-tipi E0003 metni.
9. **Testler:** `tests/ui/pass/` — modüller arası struct portu, struct reg
   + alan ataması, iç içe struct, enum alanlı struct, `struct → bits` ve
   `bits → struct` (enum'suz), `const` struct, bundle alanı struct (ui/pass
   harness build'i de denetler — ADR-0071); `tests/ui/fail/` — her yeni
   ve genişleyen tanı, çift alan sürücüsü (E4001), sürülmeyen alan;
   parite `st*` sondaları; `volt test` struct'lı tasarım (Docker, alan
   adlı rapor dahil); `volt verify` struct alanlı kontrat (Docker);
   Verilator `-Wall` (Docker), kısmi kullanım susturması dahil.
10. **Mutasyon** (tek tek, `CARGO_BUILD_JOBS=2`, `--test-threads=2`):
    alan tip denetimini kaldır (alan ataması her tipi kabul etsin); bit
    sırasını ters çevir (ilk alan LSB); kısmi alan sürücü denetimini
    kaldır (alan hedefi bütün sinyal sayılsın ya da hiç çakışmasın);
    ayrıca literal eksik alan denetimini kaldır. Her biri en az bir testi
    düşürmeli.
11. **Golden:** referans d4d8fc5 (PR #33). Struct kullanmayan
    tasarımların — Handshake payload'lı olanlar dahil — SV'si ve tanıları
    byte-aynı (`build/` golden betikleri).
12. ADR-0077'ye uygulama notları; CHANGELOG.

### Aşama 3 — örnekler ve kanıt (dal `feat/struct-examples`, main'den)

1. `examples/riscv_core.volt`: komut alanları (`opcode`, `rd`, `f3`,
   `rs1`, `rs2`, `f7` — `riscv_core.volt:194`) → `instr as RType`
   (Karar 3'ün MSB düzeni RISC-V R-tipi şemasıyla birebir) ya da
   `struct Decoded`; hangisi okunabilirliği artırıyorsa. Anlık değerler
   (`imm_*`) biçime göre dağınık bitlerdir; struct'a zorlanmaz.
2. Başka adaylar değerlendirilir: `riscv_pipeline` aşama register'ları
   (IF/ID, ID/EX değerleri), AXI payload'ları (zaten Handshake — Karar 1,
   dokunulmaz). Zorlama yok.
3. Kanıt: `volt test` (Docker; riscv_core 59/59 ve diğerleri), `volt
   verify` (Docker), Verilator `-Wall`; Yosys sentezi struct'lı ve
   struct'sız sürüm stat karşılaştırması (B'de SV birebir aynı olmalı —
   ölç, tablola).
4. README Limitations: struct satırı güncellenir (dizi/generic/@mmio
   kalır); CHANGELOG.

## Sınırlar / Ertelenen

- Struct dizisi (Karar 2, AoS eleman düzeniyle ayrı ADR), generic struct
  sinyal tipi (ADR-0069), struct desenleri (`match p { P { a: 1, .. } }`),
  fn'de struct.
- Bütün Handshake payload'ı değer olarak ve payload'ın struct portu
  olarak yeniden ifadesi (ADR-0050 kontratları ile birlikte).
- `@mmio` ↔ struct birleştirme ve bit düzeni (Karar 6).
- `--sv-struct=packed` (Karar 5).
- Varsayılan değerli alanlar, `..base` güncelleme sözdizimi.
- Struct alanlarında ADR-0066 otomatik kontratları.
- Sayısal vektörlerin kısmi sürülmesi denetimi (Yan bulgu 3).

## Yan bulgular (struct dışı ya da bu ADR'de düzeltilmez)

1. Bütün Handshake payload'ı kullanımı (`p <= req.data`) `E1001
   undefined name: 'req'` diyor — port tanımlı (x05, x06). Karar 1 ile bu
   turda E0003 olur. (ADR-0075 açık notundaki `hs.nope` ile aynı kök.)
2. Düz struct alanında domain notasyonu (`a : u4 @Fast`) sonraki alanın
   niteliği sanılıyor (W0020) ve arkasından gelen `in b : bool` yön
   ihlali **sessiz** kalıyor (`extra/field_domain.volt`); tek başına
   `in b` doğru E0001 alıyor. Aşama 2'nin E2xxx-A'sı ilkini, parser
   düzeltmesi ikincisini kapatır.
3. Sayısal vektörün kısmi sürülmesi sessiz: `wire`'sız `out y : u8`,
   yalnız `y[3:0] = a[3:0]` → check/build temiz, Verilator `-Wall`
   `%Warning-UNDRIVEN: Bits of signal are not driven: 'y'[7:4]`
   (`extra/partial_vec.volt`).
4. Yosys 0.66 struct tipli portu netlist port listesinin sonuna taşıyor
   (Karar 5 §3) — yalnız A seçilseydi önemliydi.

## Uygulama notları — Aşama 2 (2026-09-24, dal `feat/struct`)

Kararlar yazıldığı gibi uygulandı. Aşağıdakiler tasarımın açık bıraktığı
ayrıntılar, kullanıcının Aşama 1 notlarıyla bu aşamaya alınan işler ve
ölçüm sonuçlarıdır.

### Tanı numaraları

| Sembolik | Kod | İleti (EN) |
|---|---|---|
| E2xxx-A | **E2013** | Invalid struct declaration: no fields, a clock-domain annotation on a field, or a 'struct port' bundle as a field type |
| E2xxx-B | **E2014** | Struct literal is missing a field or sets a field twice |
| E4xxx-A | **E4012** | Part of a signal is never driven: a struct field or some bits of a vector assigned piece by piece |

Üçü de iki dilde ileti + `volt explain` (`crates/volt-diagnostics`).
Tip denetimi tanılarında struct adı görünür (`'P' and 'Q'`): `show()`
artık `display_named` (Aşama 1 notu 4).

### Katman yerleşimi

- **Düzen tek yerde:** `volt_ast::struct_layout` (`layout`,
  `struct_of_type`, `field_bits`, `describe`, bütçe `MAX_LEAVES = 4096`,
  `MAX_NESTING = 8`). Tip denetimi (`typeck/structs.rs`: bildirim, literal,
  `as`, reset sabitliği), sürücü analizi (`typeck/stmt.rs`: alan hedefinin
  bitleri artık ortak düzenden — ilk alan MSB), `TypeArena::signal_width`
  (W3003), sv-emit, hedefli SDC, test dili ve LSP aynı fonksiyonu çağırır.
- **SV eşlemesi AST → AST indirgemesi olarak** (`volt-sv-emit/src/
  structs/`): `emit_unit` önce birimin bir kopyasında struct tipli
  sinyalleri yapraklara açar, emitter yalnız düz sinyaller görür. Karar 5
  kuralları 1–15 birebir; tercih nedeni aynı kuralların SVA, `prev`/`sync`,
  örnek bağlantısı ve sıfırlama üretimine ek kod olmadan uygulanması.
  Birimde düz struct yoksa ya da hiçbir modül struct değeri kullanmıyorsa
  indirgeme `None` döner ve emitter özgün AST'yi kullanır (kural 15).
  `validate_unit` aynı yoldan geçer → `check` = `build` = LSP (ADR-0070).
- **`ExprKind::Concat`** (yalnız indirgeme üretir; ayrıştırıcı ve anlamsal
  aşamalar görmez): bütün-struct `==`/`!=` ve `p as uN`. Her öğe yaprağın
  tipini taşır — boyutsuz literal yaprak kendi genişliğinde yazılır.
- **Modül seviyesi `let p = P { … }`**: sözdizimi örnekleme ile literali
  ayırmıyordu (örnekleme okunup tipsiz kalıyordu). Birim sonu desugar'ı
  (`parser/struct_lit.rs`) hedef düz struct ise deyimi tipsiz `let`'e
  çevirir.
- **Düz struct alanında `@Domain`** artık ayrıştırılır (E2013 alır);
  ardından gelen `in`/`out` yön ihlali E0001 alır (Aşama 1 notu 2, yan
  bulgu 2).
- **Bütün Handshake payload'ı** (`req.data`, `rsp.data = …`): çözümleme,
  kapsamda olmayan taban adının bir düzleştirme kaynağının ara düğümü
  olduğunu görünce E0003 verir (Aşama 1 notu 3).
- **Parantezsiz literal** (`cover: p == P { a: 1 }`): başlık ifadesinden
  sonra `Ad { alan:` görülünce E0001 + `(P { ... })` önerisi, literal
  atlanır (kaskad yok).

### E4012'nin sayısal vektörlere genişlemesi (Aşama 1 notu 1)

Karar 6 "bu tur yalnız struct yaprakları" diyordu; kullanıcının Aşama 1
notu genellemeyi bu aşamaya aldı (aynı kod yolu). Kural: bir `wire` ya da
`out` portunun bütün atamaları kısmi ve aralığı derleme zamanında biliniyorsa
aralıkların birleşimi `[0, W)`'yi örtmeli. Bütün-sinyal ataması, dinamik
indeks, `let` başlangıcı, paylaşılan hat ya da giriş portu kapsamı belirsiz
ya da tam sayar; register'lar muaftır. Struct'ta sürülmeyen yapraklar
adıyla (`'p.b'`), vektörde bitler (`bits 4..=7 of 'y'`).

Mevcut kod taraması: `tests/ui`, `tests/fixtures`, `examples` altındaki 381
dosyanın hiçbirinde tetiklenmedi (golden). Tek gerçek eksik sürüm bir birim
test kaynağındaydı: `typeck_tests.rs::array_wire_index_yields_element_type`
yalnız `t[0]`'ı sürüp `t[1]`'i okuyordu (`t[1..3]` sürücüsüz); kaynak dört
elemanı da sürecek biçimde düzeltildi, testin amacı (eleman tipi) aynen
denetleniyor.

### Diğer ayrıntılar

- **Kısmen yazılan struct register'ı:** bir `on` bloğu yapraklardan birini
  yazıyorsa register'ın bütün yaprakları o bloğun reset dalına girer —
  yazılmayan yaprak reset değerini korur (Karar 4; önce hiç sürülmüyordu).
- **Dizi alanı register'da da paketlenmiş vektördür** (`logic [15:0]
  p_v`), unpacked dizi değil: `p as uN` birleştirmesi unpacked öğe alamaz.
  Dizi literali yaprağa `{eN, …, e0}` birleştirmesi olarak iner (eleman 0
  LSB — ADR-0056).
- **`raw as P` kaynağı** adlandırılmış bir değer olmalı (sinyal, örnek
  portu, dizi elemanı): SV'de `(a + b)[3:0]` yazılamaz. Hesaplanan değer
  E0003 (öneri: önce `let raw : uN = …`).
- **Bütün struct üzerinde `match`** (`match p { … }`) ve indirgenemeyen
  konumdaki bütün struct değeri E0003 (Karar 2 tablosu); `match p.s`
  çalışır.
- **Okunmayan yaprak** (kural 13) modül gövdesindeki okumalarla belirlenir;
  yalnız ayrı `.sva` dosyasında okunan yaprak da susturulur (zararsız).
- **Struct dizisi, generic struct sinyali** E0003 metni neyin
  desteklenmediğini söyler (`arrays of structs ('[P; N]' …)`, `generic
  struct type 'G' as a signal type (ADR-0069)`); içindeki struct
  literalinin ikinci E0003'ü kaskad sayılıp bastırılır.
- **Bundle sinyal tipi** E0003 metni kalıcı kuralı söyler (Karar 1).
- **SDC:** struct kaynaklı `sync()` yaprak başına köprü ve satır
  (`sync_p_a_stage0_reg*`).
- **Test dili:** yeni AST biçimleri `TestStmt::SetPort.fields` ve
  `TestExprKind::StructLit`. `volt_hir::expand_struct_tests` yaprak
  yollarını noktalı port adına (`q.a`) indirir; port araması noktalı adı
  yaprak olarak çözer, SV adı `q_a`. Bütün karşılaştırmanın iki tarafı
  Karar 3 düzeniyle paketlenmiş 64 bitlik betik değeridir — **`W > 64`
  struct'ın bütün karşılaştırması E8511** (alan alan karşılaştırın).
  Rapor: `left:  Cmd { op: Op::Add, a: 1, … }`, `differs: a`.
- **LSP:** hover'da düzen (`a: u4 [11:8]`, …) ve alan erişiminde
  `p.i.x : u3 — bits [4:2] of p`; tamamlama noktalı tabanı (`p.i.`,
  `s0.q.`) izler.
- **Güven seviyesi:** mevcut sinyal düzeyi çıkarım struct'ta Karar 6'yı
  zaten sağlıyordu (join); `trust_tests.rs`'e iki test eklendi.
- **ADR-0066:** struct alanlarına otomatik kontrat üretilmediği
  `auto_contract_tests.rs::struct_fields_get_no_automatic_contracts` ile
  sabitlendi.

### Sınırlar (bu tur)

- Karar 2 tablosu aynen (struct dizisi, generic struct, struct deseni,
  `fn`'de struct, `@mmio`).
- Yaprak adı SV anahtar sözcüğüne denk gelirse (`always` + alan `ff`) SV
  geçersiz olur — Volt adlarında SV anahtar sözcüğü denetimi genel olarak
  yok (yan bulgu: `out packed : u32` portu Verilator'da sözdizimi hatası;
  struct'tan bağımsız, düzeltilmedi).

### Doğrulama

- `cargo test --all`: 3056 test (baseline 2982 → 3056), hepsi geçti;
  clippy `-D warnings` temiz.
- Golden (referans `main` ebc5ea9 = PR #34 + PR #33 kodu;
  `build/struct2/golden.py`, `volt check` + `volt build` tanıları ve
  üretilen her `.sv/.sva`): önceki 381 dosyadan değişen yalnız struct
  kullanan 7 parite sondası (`d06c` E4001 iletisi `'s.x'`; `d06d`, `p02e`,
  `p02f`, `p02h`, `p35b`, `q05` E0003 → temiz, `// parity:` başlıkları
  güncellendi); Handshake payload'lı olanlar dahil bütün diğer tasarımlar
  byte-aynı.
- Verilator 5.050 `-Wall` (Docker): `tests/ui/pass/102–107`, Karar 5
  deneyinin `Stage`/`Top` tasarımı ve sim tasarımı temiz (bundle alanı,
  enum alanı, iç içe, `raw as P`, `sync`, kısmi kullanım susturması dahil).
- `volt test` (Docker, gerçek Verilator; `struct_tests.rs` 29/29): alan
  okuma/yazma, bütün yazma, bütün karşılaştırma (işaretli yaprak -1 dahil);
  kasıtlı yanlış iddianın raporu alan adlarıyla.
- `volt verify` (Docker, boolector): enum alanlı struct FSM'i 5 property
  prove (k-induction) + cover (2 cover erişildi), `prev(c).n` dahil;
  `107_struct_sync_contracts` `Keep` 3/3 prove + cover.
- Mutasyon (tek tek, `CARGO_BUILD_JOBS=2`, `--test-threads=2`,
  `build/struct2/mutate.py`): 5/5 yakalandı — alan tip denetimi kaldırıldı,
  bit sırası ters (ilk alan LSB), kısmi alan hedefi bütün sinyal, alan
  sürücüleri hiç çakışmaz, literalde eksik alan denetimi kaldırıldı.
