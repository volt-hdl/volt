# ADR-0075: Yanıltıcı Raporlar ve Tanı Kalitesi — verify Durumları, Tek Kod Tek Bulgu, Kapı Arkasında E0014

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-24
> Etkilenen: volt-driver (`verify.rs`, `verify_report.rs`, `main.rs` —
> sby durumları, E5002, `--timeout`, çıkış kodları 7/8), volt-sv-emit
> (`sby.rs` `timeout`; `enums.rs`/`lib.rs` `default` yorumu ve sayısal
> erişilemez kol; `instance.rs` E4011 bundle etiketi), volt-ast
> (`match_cover.rs` — YENİ), volt-hir (`drivers.rs` W4001/W4002 kaldırıldı,
> E4002 kaynak adı; `typeck/matching.rs` sayısal W2014, E0014 notu;
> `typeck/gated.rs` — YENİ: kapı arkasında enum E0014; `pipeline.rs`;
> `resolve/usage.rs` bundle önerisi), volt-syntax (`parser/bundle.rs`
> kaynak adı kaydı; `parser/stmt.rs` E0014 notu), volt-diagnostics (E5002
> YENİ; W4001/W4002 başlık ve açıklaması; E2012 açıklamasında aşama adı),
> `docs/spec/cli-contract.md` (§2 çıkış kodları, §8a), `docs/spec/`
> E2005 atıfları (ADR-0072 kaynaklı), testler.
> DOKUNULMADI: README.md, docs/research/, examples/.
> Kapatır: ADR-0074 "Kapsam dışı açıklar" (prove UNKNOWN "tool error",
> sayısal yinelenen kol, sayısal E0014 notu "arrives with F3", W1004+W4002
> çifti), ADR-0073 §"Açık bulgular" (bundle alanı düzleşmiş adla),
> ADR-0072 §"Sınırlar" (docs/spec E2005 atıfları).

Son altı görevde (ADR-0068..0074) kapsam dışı bırakılan küçük bulgular.
Bir kısmı kullanıcıyı yanıltıyordu (Bölüm 1), bir kısmı yalnız ileti
kalitesiydi (Bölüm 2).

## 1. `volt verify` — sby'nin beş durumu

sby her görevi beş durumdan biriyle bitirir. Önce yalnız ikisi tanınıyor,
kalanı "tool error" (çıkış 3) oluyordu:

| sby durumu (rc) | Anlamı | Önce | Sonra |
|---|---|---|---|
| `PASS` (0) | tüm kontratlar doğrulandı | `ok`, 0 | `ok`, 0 |
| `FAIL` (2) | karşı örnek | E5001 + `_cex.vcd`, 6 | aynı, 6 |
| `UNKNOWN` (4) | prove: temel durum geçti, tümevarım adımı başarısız | **"tool error (exit code Some(4))", 3** | **E5002** + `_induct.vcd`, **7** |
| `TIMEOUT` (8) | süre sınırı doldu | (süre sınırı verilemiyordu; olsa "tool error", 3) | "timed out" iletisi, **8**; `--timeout <sn>` |
| `ERROR` (16) / `DONE` yok | aracın kendisi başarısız | "tool error (exit code Some(16))", 3 | "tool error (sby status ERROR, exit code 16)" + "this says nothing about the contracts", 3 |

Ölçüm (hdlc/formal, sby 0.36): tümevarımsal olmayan doğru kontrat
(`a <= 10`, `a` ile `b` hep eşit ama tümevarım `a != b`'den başlayabilir)
`--mode prove --depth 8` ile `DONE (UNKNOWN, rc=4)` verir; log'da
`engine_0.induction: ... Assert failed in Shadow: n.sv:40` satırı
kanıtlanamayan iddiayı adlandırır. `timeout 2` + derinlik 400 →
`DONE (TIMEOUT, rc=8)`; bozuk SV → `DONE (ERROR, rc=16)`. Beş durumun
hepsi gerçek sby ile koşturuldu; testlerdeki sahte sby satırları bu
çıktıdan alındı.

Kararlar:

- **Ayrı çıkış kodları: 7 kanıtlanamadı, 8 zaman aşımı.** CI "kontrat
  yanlış" (6), "araç/kurulum bozuk" (3), "daha fazla bütçe gerekir" (8)
  ve "kanıt stratejisi yetmedi" (7) arasında ayrım yapabilmeli. Mevcut
  kodlar (4 yapılandırma, 5 test) başka anlamda kullanıldığından yeni
  numara gerekti; 7 ve 8 boştu.
- **Öncelik 6 > 3 > 8 > 7.** Birden çok görev farklı biterse en kesin ve
  en eyleme dönük sonuç baskındır: çürütülmüş kontrat her şeyden önce;
  araç hatası diğer sonuçları güvenilmez kılar; zaman aşımında hiçbir
  bilgi yoktur, UNKNOWN'da ise temel durum en az geçmiştir.
- **UNKNOWN bir tanıdır (E5002), mesaj satırı değil.** sby
  kanıtlanamayan iddianın satırını verir; E5001 gibi o kontrata işaret
  etmek kullanıcıya NEREYE invariant ekleyeceğini gösterir. Yardım:
  "try a larger --depth, or add an invariant that makes the property
  inductive". Tümevarım izi `<görev>_induct.vcd` olarak kopyalanır ve
  "counterexample" DEĞİL "induction trace (may start from an unreachable
  state)" notuyla verilir — karşı örnek demek yine yanıltırdı.
- **TIMEOUT ve ERROR tanı değil, ileti.** İkisi de kontrat hakkında bilgi
  taşımaz; kaynakta işaret edilecek yer yoktur.
- **`--timeout <sn>`** (görev başına, sby `[options] timeout`). Olmadan
  TIMEOUT durumu erişilemezdi; varsayılan yok — `.sby` çıktısı bayraksız
  koşuda birebir aynı kalır.
- JSON: `modules[].status` ve `properties[].status` `unknown` ve
  `timeout` alır; UNKNOWN'da adı geçen kontrat `unknown`, modülün diğer
  kontratları `unproven` (tümevarım bütün kontratları birlikte kanıtlar).

`cli-contract.md` §2 ve §8a bu ADR'ye göre güncellendi.

## 2. Enum `default` yorumu

Kapsayıcı, `_`'sız enum `match`'inde son kol `default` olur (ADR-0074
Karar 4) ve SV yorumu `// State_Done (and invalid codes)` diyordu. `2^n`
varyantlı (yoğun) enum'da hiçbir varyanta ait olmayan kod yoktur; yorum
artık yalnız `EnumLayout::is_dense()` yanlışsa "(and invalid codes)"
ekler. Üretilen SV'de yalnız yorum değişir (golden: aşağıda).

## 3. İki kod, tek ileti

Tarama iki yöntemle yapıldı: (a) kaynakta `ErrorCode::X` ile gelen ilk
ileti metni ve `messages/en.rs` başlık tablosu — aynı metni farklı
kodla taşıyan çiftler; (b) ampirik: `tests/ui`, `tests/fixtures`,
`examples` ve bu görevin sondaları (373 dosya) `volt check` ile koşturulup
AYNI konumda farklı kodla verilen tanılar.

| Çift | Kapsam | Karar |
|---|---|---|
| W1004 + W4002 "register written but never read" | payload'a özgü DEĞİL — her yazılıp okunmayan register | W1004 kalır |
| W1001 + W4001 (sürülüp okunmayan `wire`) | aynı bulgu, farklı metin | W1001 kalır |
| W1004 "unused register" + W3001 "never written in any 'on' block" | iki ayrı bulgu (okuma yok / yazma yok) | ikisi kalır |

W4001/W4002 (`drivers.rs::check_write_only`) çözümleyicinin `res.reads`
kümesine bakıyordu — W1001/W1004 ile AYNI veri; eklediği bilgi yoktu.
W1xxx kalır: tek kullanım izleyicisi çözümleme aşamasındadır, `_`/`pub`/
extern muafiyetlerini ve ADR-0068 kaynak adını (`pe_0` → `pe`) zaten
uygular; W4xxx açılmış adı sızdırıyordu. W4001/W4002 kodları enum'da,
spec tablosunda ve `explain`'de KALIR ama "ayrılmış — bu derleyici
üretmez" olarak belgelenir: `explain W4002`'nin eskiden anlattığı
netlist düzeyi analiz (okuyucusu kendisi ölü olan register) hiç
uygulanmamıştı; kod o analiz için ayrılmıştır. Kod silmek `W4002`'yi
yapılandırmasında anan kullanıcıları kırardı.

## 4. Kapı arkasında enum E0014

Parser yol desenli, `_`'sız `match`'in E0014'ünü tip denetimine erteler
(ADR-0074 Karar 4). Birimde çözümleme hatası (E1xxx) varsa tip denetimi
koşmaz (ADR-0070 kapısı) ve enum E0014'ü kayboluyordu — sayısal match'in
E0014'ü parser'da verildiği için aynı birimde görünüyordu.

Karar: kapı kapalıyken `typeck::gated_enum_exhaustiveness` tipsiz ve
yalnız KESİN durumu bildirir: sınanan, bildirilen tipi enum olan bir
port/register/wire/tipli `let` adıdır; muhafızsız kolların bütün yol
desenleri o enum'un varyantlarına çözülmüştür; joker yoktur; eksik
varyant vardır. Tanı tip denetimininkiyle birebir aynıdır (ortak
`enum_not_exhaustive`). Emin olunamayan her durum — çözülmemiş varyant
(E1007 zaten var), ifade sınanan, başka enum'un varyantı (E2003'e kalır)
— sessizdir; çözümleme hatası giderilince tip denetimi karar verir.

**fn gövdesi — düzeltilmedi, gerekçe:** `fn` gövdeleri çözümlenir ama
tip denetiminden HİÇ geçmez (`typeck` yalnız modül ve extern gezer);
gövdedeki `match` bir İFADEDİR (`pat => expr,`) ve ifade `match`'inin
ADR-0032'de kapsayıcılık kuralı yoktur — sayısal olanı da E0014 almaz.
`fn` çağrısı donanımda E0003'tür, yani bu kod hiçbir çıktıya ulaşmaz.
Düzeltmek fn gövdesi tip denetimi (parametre/dönüş tipleri, ifade
`match`'inin tipi ve kapsayıcılığı) demektir — ayrı bir özellik, ayrı ADR.

## 5. Sayısal `match`'te yinelenen kol

`match x { 1 => a, 1 => b, _ => c }` uyarı vermiyordu; SV'ye iki `2'd1:`
etiketi yazılıyordu. Enum'da aynı durum W2014 alır ve kol SV'ye yazılmaz.
Karar: aynı kural sayıda — muhafızsız kolun BÜTÜN literal değerleri
önceki muhafızsız kollarda geçtiyse kol erişilemezdir: W2014 ("an earlier
arm already covers this value") ve SV'de kol atlanır. Değer karşılaştırılır,
yazım değil (`1`, `0x1`, `0b01` aynı; `-0` = `0`); kısmi örtüşme (`1`
sonra `0 | 1`) uyarı almaz (kol hâlâ `0`'ı seçer) — enum'daki kuralla
aynı. Joker/bağlama sonrası kollar ve muhafızlı kollar değerlendirilmez
(enum'da da öyle). Kural tek yerde, `volt_ast::match_cover`: HIR uyarısı
ile sv-emit atlaması aynı kolu seçer.

## 6. Aşama adları

Kullanıcıya görünen dizgelerde (tanı iletileri, `explain`, clap yardım
metni) `F0`..`F9`/`F4b` taraması (`#[cfg(test)]` dışı, çok satırlı
dizgeler dahil): **5 ileti, 8 dizge** — sayısal E0014 notu (parser ve
HIR kopyası, en/tr: 4 dizge), E5014 açıklaması "F0's rule" (en/tr: 2),
`volt --help` satırları `verify (F4b; ...)` ve `lsp (F5a; ...)` (2).
Hepsi güncellendi; E0014 notu artık "a match on a number covers every
value only with a '_' arm (ADR-0032); an enum match is checked variant by
variant instead (ADR-0074)" der. Kalan 5 eşleşme iç tiplerin `///`
belge yorumlarıdır (`--help`'te görünmez), dokunulmadı.

## 7. Bundle alanı kaynak adıyla

Bundle düzleştirmesi (ADR-0039) `out hs : Handshake`'i `hs_data`,
`hs_valid`, ... portlarına açar; tanılar düzleştirilmiş adı basıyordu.
ADR-0072'nin açılım adı deseni: `parser/bundle.rs` her düzleştirilmiş
portun benzersiz ad span'i için `GenerateInfo::source_names`'e
`hs.data` yazar; ad basan tanılar `source_name` üzerinden okur.

| Tanı | Önce | Sonra |
|---|---|---|
| W1001 | `unused input port: 'hs_data'`; öneri `_hs_data` | `'hs.data'`; öneri "add a '_' prefix to the bundle port to silence all its fields: _hs" (alan tek başına yeniden adlandırılamaz; `_hs` bütün alanları `_hs_...` yapar ve susturur) |
| E4001 | `'hs_data' is already driven` | `'hs.data' is already driven` |
| E4002 | `output port 'hs_data' is not driven`; öneri `hs_data = ...` | `'hs.data'`; öneri `hs.data = ...` |
| E4011 | `input port 'hs_ready' of instance 'p' is not bound` | `input port 'hs_ready' (bundle field 'hs.ready') of instance 'p' is not bound` |

E4011'de düzleştirilmiş ad KALIR: örnekleme literalinde bağlama anahtarı
odur (`examples/soc/top.volt`: `aw_data_addr: ...`); yalnız kaynak yolu
eklenir. E4005 zaten `hs.data` diyordu; E3001/E2010 ad basmıyor.

## 8. Spec atıfları

ADR-0072 E2005'i böldü; spec'te 6 atıf eskimişti. `const-eval.md` §8
(iki yer: `for` sınırı → E2021), `sv-mapping.md` (bağlanmamış giriş portu
ve çift yönlü port bağlantısı → E4011; `for` sınırı → E2021),
`type-inference.md` §7 E2005 başlığı ("Genişlik ya da uzunluk
belirlenemiyor"). Hâlâ doğru olan iki atıf (literal genişliği, indekssiz
sabit dizi) değişmedi. W4001/W4002'nin `type-inference.md` §11.5/11.6
satırları bu ADR'nin kapsamı dışında bırakıldı; güncel anlamları §3'tedir.

## Test ve doğrulama

- 1.1: `volt-driver/tests/verify_status_tests.rs` (8 test: her durum,
  Türkçe ileti, `--timeout 0` kullanım hatası, `.sby` `timeout` satırı,
  JSON durumları, öncelik); `verify.rs`/`verify_report.rs` birim testleri
  (gerçek UNKNOWN/TIMEOUT/ERROR log biçimi, öncelik tablosu, rapor
  satırları); `sby.rs` `timeout` satırı. Beş durum gerçek sby ile de koşuldu.
- 1.2: `enum_tests.rs` yoğun (4 varyant) / seyrek (açık değerli) enum.
- 1.3: `typeck_tests.rs` tek W1004 / tek W1001, payload'lı enum register'ı.
- 1.4: parite sondaları `e26` (E1001 + E0014), `e27` (tam kapsama: yalnız
  E1001), `e28` (çözülmemiş varyant: yalnız E1007), `e29` (tanımsız enum
  yolu: yalnız E1001).
- 2.1: `matching.rs` birim testleri (yazım, `|`, işaretli/`-0`/bool,
  muhafız, joker sonrası); `enum_tests.rs` SV'de kol atlanır; `tests/ui/
  fail/97` (W2014), `tests/ui/pass/98` (kısmi örtüşme temiz).
- 2.3: `bundle_semantic_tests.rs` (W1001 + öneri, E4001, E4002 + öneri,
  `_hs` susturması), `bundle_emit_tests.rs` (E4011 etiketi).
- Mutasyon (`build/cleanup/mutate.py`, tek tek): 17 mutasyondan 16'sı
  yakalandı. Kaçan tek mutant eşdeğer: `gated.rs`'de çözümsüz desen yolu
  için `?` dalı — çözümleyici her yol desenini (hata tanımına bile)
  `pattern_resolutions`'a yazar; asıl koruyucu dal ("EnumVariant değil →
  sessiz") ayrı mutasyonla yakalandı (e28/e29).
- Golden (PR #31 sonrası `main` ikilisi, 370 ortak dosya): geçerli
  tasarımların build çıktısında tek fark 1.2 yorumu — `UartTx.sv`
  (`examples/uart_tx`, `soc/uart`, `soc/top`, `riscv_core`,
  `riscv_core_test`, `riscv_sw/hello_soc`) ve `tests/ui/fail/89`
  (`State_Run`, 2 varyant). Check farkları: bundle adı (d06, d06b,
  ui/fail/36), E0014 notu (e24, p29, ui/fail/27), W4002'nin kalkması
  (ui/pass/37).

## Sınırlar ve açık bulgular

- Bundle portunun `@Alan` anotasyonu düzleştirilmiş her alan için ayrı
  (benzersiz, sentetik 1 karakterlik) span taşır (ADR-0047 `use_spans`
  anahtarı); tanımsız alan E3002'yi alan sayısı kadar, her biri farklı
  sütuna işaret ederek verir. Anahtar ile tanı konumunu ayırmak ayrı iş.
- `hs.nope` (bundle'da olmayan alan) `undefined name: 'hs'` (E1001)
  verir — `hs` tanımlı bir porttur; ileti yanlış şeyi söyler.
- `_` ya da bağlama deseninden SONRAKİ kollar erişilemezdir ama ne enum'da
  ne sayıda uyarı alır (iki taraf tutarlı; ayrı karar).
- `fn` gövdeleri tip denetiminden geçmez (§4).
