# ADR-0070: Tanı Tutarlılığı — `check`, LSP ve `build` Aynı Şeyi Söyler

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-24
> Etkilenen: volt-hir (`pipeline.rs` — YENİ: kapılı ortak boru hattı
> `run_semantic_stages`/`pre_resolve_checks`; `unit_load.rs` — volt-driver'ın
> `unit.rs`'inden taşındı, `load_unit_with_text` + `LoadedUnit::source_names`;
> `ty.rs` — `display_named`; `typeck/stmt.rs` — W2012 çözüm metni;
> Cargo.toml: volt-syntax dev→normal bağımlılık), volt-driver (`compile_all`
> emit geçişini `check` için de koşar; `run_semantic_stages` ortak fonksiyonu
> çağırır; `cap_diagnostics` ortak uygulamaya bağlandı), volt-lsp
> (`analysis.rs` — tanılar birim yükleyicisi + ortak boru hattı + çıktısız
> emit + `annotate_generate` + üst sınır; `hover.rs` — tip adı, bundle portu;
> `lib.rs` — `doc_path`; Cargo.toml: volt-sv-emit bağımlılığı),
> volt-diagnostics (`cap.rs` — YENİ: `cap_diagnostics`,
> `LSP_MAX_DIAGNOSTICS`; E0003 `explain` notu), volt-sv-emit (`alias.rs` —
> YENİ: takma ad çözümü, tür adlı E0003 metinleri; `validate_unit`,
> `unit_source_texts`; E0003 metinleri; C sınıfı düzeltmeler), volt-syntax
> (`parser/bundle.rs` — Handshake payload'u olarak bundle E0003),
> `tests/fixtures/parity/` (72 sonda), `tests/ui/pass/92`, `tests/ui/fail/83`,
> `crates/volt-driver/tests/{parity_tests,lsp_protocol_tests}.rs`.
> DOKUNULMADI: examples/, README.md, docs/spec/, docs/research/.
> Genişletir: ADR-0068 (§"Sınırlar": LSP katlama açığı kapanır), ADR-0069
> (§"Sınırlar": geçerli sade struct/enum/takma ad port tipi).

## Sorun

Son üç görevde aynı desen çıktı: kullanıcı editörde (LSP) ya da
`volt check`'te hata görmüyor, `volt build`'de anlamsız bir mesajla
karşılaşıyor.

- ADR-0069 (issue #21): özyineli struct için `check` temiz, LSP temiz,
  `build` ilgisiz bir E0003 ("user-defined types").
- ADR-0068: tanı katlama CLI'da var, LSP `annotate_generate`'i çağırmıyor —
  düzeltilen tanı seli editörde açık kaldı.
- ADR-0069 sonrası ölçüm: GEÇERLİ sade struct, enum, takma ad (`type W = u8`
  bile), demet port tipi ve struct tipli `let` → `check` 0 tanı, `build`
  E0003.

Kök neden tek: tanıyı üreten mantık üç yerde çoğaltılmış ve kopyalar
sapmıştı.

1. **Üç boru hattı.** Sürücünün `compile()`'ı, LSP'nin `analyze()`'ı ve
   `volt_hir::analyze` aşama listesini ayrı ayrı yazıyordu. LSP'de güven
   seviyesi (E3009/W3008), L1 zamanlama, Handshake protokolü, test blokları,
   import denetimi ve katlama hiç yoktu.
2. **SV eşleme kararı yalnız emitter'da.** `check` "çıktı üretmeden
   doğrulama" (cli-contract §6) diye emit'i hiç koşmuyordu; emitter'ın 45
   tanı noktasının 23'ü analizde bilinebilir olduğu hâlde yalnız `build`'de
   görünüyordu.
3. **LSP tek dosya ayrıştırıyordu.** `use` ile çok dosyalı birimde
   (`examples/hybrid_accel/hybrid_top.volt`) editör, `volt check` temizken
   sahte bir E2005 gösteriyordu (içe aktarılan sabit görünmüyor).

## İlke

**Analizde bilinebilen her hata `volt check`'te ve editörde görünür.**
Yalnız `build`'de çıkabilecek tanılar, belirli bir çıktı kipine özgü
olanlardır (A sınıfı, aşağıda listeli). Bu kural bir testle sabitlenir
(§4).

## Karar

### 1. Tek kapılı boru hattı

`volt_hir::run_semantic_stages(ast, resolve, test_files, out)` aşamaları ve
kapılamayı tek yerde tanımlar: resolve → const+typeck → domain (+ güven,
RDC) → zamanlama, Handshake, test blokları, kısıtlar. Sürücü `resolve_unit`,
LSP'nin editör verisi (`hover`, tamamlama) `resolve_file` ile çağırır; aşama
eklemenin tek yeri burasıdır. `pre_resolve_checks` çözümlemeden önceki
denetimleri (W0021) toplar.

Birim yükleyicisi (ADR-0042) volt-driver'dan volt-hir'e taşındı
(`unit_load`). `load_unit_with_text` ana dosya metnini editör tamponundan
alır. LSP tanıları artık `volt check`'in izlediği yolun aynısını izler:
birim yükleme → ön denetim → import → ortak boru hattı → çıktısız emit →
toplayıcı. Yalnız bu belgeye düşen tanılar yayımlanır; başka dosyadaki
ikincil etiketler `dosya:satır:sütun` notu olur. Editör verisi tek dosya
analizinde kalır (hover/tamamlama davranışı değişmedi).

volt-hir → volt-syntax artık normal bağımlılıktır (önce yalnız dev). Katman
sırasına uygundur (syntax → ast → hir), döngü yoktur, yeni dış bağımlılık
yoktur.

### 2. `check` emit doğrulamasını koşar, çıktıyı atar

`volt check`, önceki aşamalar hatasızsa `volt build`'in varsayılan emit'ini
(`SvaMode::None`) bellekte koşar ve tanılarını raporlar; üretilen SV atılır.
LSP aynı işi `volt_sv_emit::validate_unit` ile yapar. Emit girdisi
(`unit_source_texts`) iki tüketicide ortaktır.

| Seçenek | Değerlendirme |
|---|---|
| A — 23 B sınıfı denetimi HIR'e tek tek taşımak | Aynı kararın ikinci kopyası: emitter'ın desteği genişledikçe (ör. struct portu) kopya geride kalır. Bu görevin kapattığı hata sınıfını yeniden üretir. Elendi. |
| B — Durum korunur, yalnız yeni tanılar | "Check temiz, build hatalı" sınıfı açık kalır. Elendi. |
| C — `check` emitter'ın doğrulamasını koşar (SEÇİLDİ) | Parite yapısal olarak sağlanır: emitter'a yarın eklenen bir tanı da `check`'te ve editörde görünür. cli-contract §6'daki "çıktı üretmeden" korunur (dosya yazılmaz). Ölçülen maliyet: en büyük örnekte `check` 42 → 44 ms, 45 → 48 ms (debug, süreç başlatma dahil). |

Kapılama korunur: emit yalnız analiz hatasızsa koşar, analiz tanılarını
engellemez (eski yorumdaki "sv-emit sınırları analizi engellememeli"
gerekçesi bununla karşılanır).

### 3. LSP'de katlama ve üst sınır

- **Katlama**: LSP, CLI ile aynı toplayıcıyı (`annotate_generate`) çağırır.
  Katlanmış tanının notu ("reported once; occurs in N unrolled 'for'
  iterations") **mesajın içinde** görünür, `relatedInformation`'da değil.
  `relatedInformation` bir konum ister, notun kendi konumu yoktur; mevcut
  dönüşüm (`convert.rs`) notları ve çözümü mesaj kuyruğuna zaten
  yazıyordu. Böylece editör CLI'daki metni aynen gösterir.
- **Üst sınır: 200** (`LSP_MAX_DIAGNOSTICS`, belge başına). Katlama kök
  nedeni çözer; sınır yalnız FARKLI tanılar için yedek güvencedir. 1000
  (CLI varsayılanı) editörde çok fazladır: sorunlar paneli okunmaz hâle
  gelir ve liste her tuş vuruşunda yeniden serileştirilir (katlamadan önce
  283 yinelemeli bir tasarım her düzenlemede 168 KB gönderiyordu).
  200 gerçek dosyalarda erişilmez: külliyatın en kalabalık dosyası 5 tanı
  üretir. Sınır aşılınca editör de W0023 alır; çözüm metni
  editöre uygun: "`volt check` lists them all (--max-diagnostics=0)".
  Sınırlayıcı sürücüyle ortaktır (`volt_diagnostics::cap_diagnostics`).
  Yapılandırılabilir yapılmadı (YAGNI; ihtiyaç doğarsa
  `initializationOptions`).

### 4. Kalıcı parite testi

`crates/volt-driver/tests/parity_tests.rs`, külliyat (`tests/ui`,
`examples`, `tests/fixtures/parity`) üzerinde:

- `every_default_build_diagnostic_is_reported_by_check`: varsayılan
  `volt build`'in her tanısı (kod + mesaj + birincil konum) `check`'te de
  var. A sınıfı listesi bu kip için BOŞTUR.
- `sva_only_diagnostics_are_listed_a_class`: `--emit=sva` farkları yalnız
  A sınıfı listesindeki kodlar olabilir.
- `parity_fixtures_report_expected_errors_in_check`: her sonda
  (`// parity: <kodlar>|ok` başlığı) `check`'te beklenen hatayı verir;
  `ok` sondaları `build`'den de geçer.
- `lsp_reports_the_same_diagnostics_as_check`: editör tanıları `check` ile
  aynı (ana dosyaya düşenler).

`tests/fixtures/parity/`, emitter'ın her tanı noktası için en küçük
programdır (§5 tablosu). Yeni bir emitter tanısı eklenirse ve `check` onu
görmezse test düşer.

### 5. Sınıflandırma (ADIM 2.1)

Ölçüm iki katmanlı yapıldı. Mevcut külliyatta (219 dosya: tests/ui,
examples) yalnız `build`'de görünen tanı 3 dosyadaydı (ADR-0069 sondalarıyla
227 dosyada 9). Emitter'ın kendi tanı noktaları külliyatta pek
tetiklenmediği için her nokta için ayrı bir sonda yazıldı (45 nokta, 65
sonda):

| Sınıf | Adet | Noktalar |
|---|---|---|
| B — analizde bilinebilir, yalnız emitter söylüyordu | 23 | ifade içinde çağrı/sync; match/string/struct/tuple ifadesi, `todo!()`; sabit diziyi değer olarak kullanmak; bağlamsız literal (cast, `3 == 3`); extern modül örneği; çocuğun reset'i ebeveynde yok; bağlanmamış giriş; inout bağlaması; çıkışı literalde bağlamak; blok içi `for` sınırı sabit değil / 4096'yı aşıyor; katlanamayan sabit dizi elemanı; tipsiz reg; `let` içinde sync; desteklenmeyen tipte wire; sync biçim kuralları (4 nokta); `on clk.reset`; blok içi `let`; match muhafızı / deyimde ifade kolu / bağlama-yol-tuple deseni; örnek portuna atama; port dışında reset tipi; dizi/tuple tip konumları; struct/enum/takma ad tipleri; eksik zorunlu primitif bağlaması |
| Zaten paritede (analiz önce yakalıyor) | 10 | sabit dizi indeks taşması (E2006), uzunluk uyuşmazlığı (E2003), sync saat argümanı alan erişimi (E2003), `[T; N]`/`bits<N>` sabit değil (E2021), primitif generic argümanları (E2003/E2025/E2008), primitif saat bağlaması (E2003) |
| Ulaşılamadı | 1 | sync kaynağının genişliği (hedef hep port/wire) |
| A — üretim kipine özgü | 1 (+ kipler) | aşağıda |

**B sınıfının hepsi artık `check`'te ve editörde** (§2 ile; her biri için
`tests/fixtures/parity/` sondası). C sınıfı (yanıltıcı mesaj) düzeltmeleri:

- E0003 metni NEYİN desteklenmediğini söyler, nerede denetlendiğini değil:
  "not supported yet: struct type 'P' as a signal type (ports, reg, wire,
  let)". Eskimiş "F0 SV generation" / "will be added in F1+" ifadeleri tüm
  E0003 ve E2012 metinlerinden kalktı.
- Hedefi bulunamayan örnek: extern hedef → "instances of extern module
  'Ext' (ADR-0047)"; struct adı → "struct literals ('p = Pt { ... }')"
  (önce ikisi de "target module is not in this file" diyordu; başka
  dosyadaki modül zaten çözülüyordu).
- `let t = sync(..)` ve desteklenmeyen tipli `let`: kaskad "cannot
  determine the width" (E2005) yerine asıl neden (E0003) — değer
  üretilir, kendi tanısı yoksa E2005 kalır.
- Desteklenmeyen tipte `wire`: aynı sorun için ikinci E0003 kalktı.
- Port dışında reset tipi: "the explicit 'reset' port" değil, "values of
  type 'reset' outside an input port".
- Eksik zorunlu primitif bağlaması: "henüz desteklenmiyor" (E0003) değil,
  kullanıcı modülündeki bağlanmamış girişle aynı tanı (E2005, "port
  'wr_data' of 'Ram' instance 'r' is not bound").

**A sınıfı listesi** (yalnız belirli bir çıktı kipinde bilinebilir):

| Kip | Tanı | Neden check'te yok |
|---|---|---|
| `--emit=sva` | E0003 / E2005 — SVA'ya inemeyen ya da boyutlandırılamayan kontrat ifadesi | Kontratlar varsayılan build'de üretilmez; `volt test`'te aynı ifade W5001 ile izlenmez (ADR-0064). SVA istenmediğinde hata değildir. |
| `volt test` | W5001 | Simülasyon izleyicisi kipine özgü (ADR-0064). |
| `--emit=sdc,xdc` | W0022 | Kısıt çıktısına özgü (ADR-0054). |
| `--emit=rust,c`, `--check-regmap` | sw-emit tanıları, E9003/E9004 | Yazılım çıktısı / dış sürücü karşılaştırması (ADR-0053, ADR-0063). |

**LSP istisnaları**: test veri dosyası içeriği (E8508, E8510) editörde
okunmaz; birimdeki başka bir dosyaya düşen tanı o dosya açılınca görünür.

### 6. Sade struct / enum port tipi (ADIM 2.2)

Gerçek SV desteği bu ADR'nin kapsamı DIŞINDA (ayrı ADR, ayrı görev).
Karar: `check` ve editör dürüstçe "not supported yet: struct type 'P' as a
signal type (ports, reg, wire, let)" der (E0003); `build` ile aynı tanı.
Bundle (`struct port`) ve Handshake payload'u olarak sade struct zaten
destekliydi, değişmedi.

**Takma adlar düzeltildi** (hata düzeltmesi): generic olmayan takma adlar
SV eşlemesinden ÖNCE hedef tipe açılır (`volt-sv-emit/src/alias.rs`).
`type W = u8`, `type C = clock`, `type A = [u4; 3]` ve zincirler
(`type V = W`) port, reg, wire ve let'te tipin kendisi gibi çalışır
(`tests/ui/pass/92`; üretilen SV Verilator `-Wall` lint'inden geçti). Struct
ya da enum'a çözülen takma ad o tipin tanısını alır. Generic takma ad
(`type W<N> = bits<N>`) eşlenmez: "generic type alias 'W'".

### 7. Mesaj kalitesi (Bölüm 3)

- `Handshake<Handshake<u8>>` (ve `Handshake<BirStructPort>`): yanıltıcı
  E1001 ("undefined name: 'Handshake'") yerine E0003 "a Handshake payload
  cannot be a port bundle" + not (payload üreticinin sürdüğü veridir,
  bundle'ın kendi yönleri var). Payload hata tipine çevrilir, kaskad yok
  (`tests/ui/fail/83`).
- LSP hover: struct/enum tipli port `a : P` / `e : E` gösterir (`a :
  struct` değil; dizi elemanında da: `[P; 2]`). Bundle portu yazıldığı tiple
  ve açılan portlarla: `hs : Handshake<u8>` + `out hs_data : u8`, `out
  hs_valid : bool`, `in hs_ready : bool` (önce `bus` için hover yoktu, `hs`
  için açılmış `hs_ready : bool` görünüyordu).
- W2012 çözümünde üretilmiş ad (`let t_0 : i32`) artık yok; çözüm adı
  anmaz ("write the type explicitly after the name, e.g. let x : i32 =
  ..."). Tip denetçisi kaynak metnine erişmez; üretilmiş addan kaynak adı
  güvenle geri çıkarılamaz (kullanıcı `t_0` diye ad verebilir).

## Sonuçlar

| Ölçüm | Önce | Sonra |
|---|---|---|
| LSP, 283 yinelemeli `for` + 2 gövde hatası | 566 tanı, 168 KB | 2 tanı, 1,3 KB (katlama notuyla) |
| LSP, ADR-0068 fuzz girdisi | 63 tanı, katlama/yineleme notu yok | 63 tanı, notlarla |
| LSP, `hybrid_accel/hybrid_top.volt` (çok dosyalı) | sahte E2005 | 0 (check ile aynı) |
| Yalnız build'de tanı veren dosya (301 dosya: tests/ui, examples, ADR-0069 ve parite sondaları; varsayılan emit) | 59 | 0 |
| `check` süresi (en büyük örnek, debug) | 42–45 ms | 44–48 ms |

- Golden (`tests/ui`, `tests/fixtures`, `examples`; `check` insan + JSON,
  `build --emit=sva` çıktı hash'leri): 216 dosya birebir. Daha önce başarılı
  olan HİÇBİR build çıktısı değişmedi. Build farkı olan 4 dosyanın hepsi
  takma ad sondası (önce hata → şimdi başarılı). Check farkları: yeni
  sondalar ve `tests/ui/pass/62_extern_domains.volt` (extern örneği artık
  `check`'te de E0003 — `build` onu zaten reddediyordu; analiz katmanı
  testi `ui_semantic_tests` değişmedi).
- Mutasyon (hepsi yakalandı): LSP'de katlama kaldırıldı → `lsp_folds_*`
  düştü; `check` emit'i atlıyor → parite testi düştü; `check` E0003'ü
  süzüyor → parite testi düştü; LSP emit doğrulamasını atlıyor →
  `lsp_reports_the_same_diagnostics_as_check` düştü; hover `display`'e
  döndü → hover testi düştü; takma ad çözümü kapalı → sonda testi düştü.

## Sınırlar

- W1001 ("unused binding") açılmış `for` gövdesinde üretilmiş adı gösterir
  (`unused_0`, `unused_1`, ...) ve mesaj her kopyada farklı olduğundan
  KATLANMAZ. Çözümlemenin kaynak adını bilmesi gerekir (açılımın
  yeniden adlandırma tablosu AST'ye taşınmalı); ayrı iş.
- Emitter bağlanmamış port için E2005 kullanır; kodun kısa açıklaması
  "Literal width cannot be determined". Mevcut davranış korundu, yeni kod
  ayrı karar.
- `sync()` argüman sayısı hatası E0003 kalır; aslında tip hatasıdır (E2003
  ailesi).
- Bağlamsız literal (`(1+2) as u8`, `3 == 3`) literal başına bir E2005
  verir (2 tanı); emitter sabit katlasa hiç gerekmezdi.
- Extern modül örneklerinin SV üretimi hâlâ yok (ADR-0047 sınırı); artık
  `check`'te de görünür.
- Takma ad ve örnek hedefi aramaları ada göredir (emitter'ın mevcut
  kuralı); birimde iki dosya aynı adlı takma ad tanımlarsa emitter ilk
  bulduğunu kullanır.
- A sınıfı SVA listesi kod düzeyindedir (E0003/E2005); kontrat dışı bir
  SVA-özel tanı varsayılan build testinde yakalanır.
