# ADR-0140: Domain Bildiriminde Her Alan Bir Kez — İkinci Yazım E1003, Alanlar Tek Okuyucudan

> Statü: Önerildi
> İlgili: ADR-0137 (§1'in kuralının `domain` bildirimine uygulanışı; §5'in ayıklamasındaki 8. satırı kapatır)
> Issue: #420
> Tarih: 2026-11-25
> Etkilenen: `crates/volt-ast/src/domain_fields.rs` (YENİ, ortak okuyucu), `crates/volt-syntax/src/parser/item.rs` (E1003), `crates/volt-hir/src/domain/tables.rs`, `crates/volt-hir/src/domain/rdc/facts.rs`, `crates/volt-hir/src/constraints/walk.rs`, `crates/volt-sv-emit/src/lib.rs`, `crates/volt-diagnostics/src/explain/en.rs` ve `tr.rs` (E1003 açıklaması), `crates/volt-driver/tests/domain_field_once_tests.rs` (YENİ), `docs/design/tek-yerde-bilgi-envanteri.md`, `CHANGELOG.md`

## Sorun

Gramer bir `domain` bildirimine istenen sayıda alan yazdırır
(`grammar-full.ebnf` §3: `{ DomainField }`); spec aynı anahtarın ikinci
yazımı için bir şey söylemez. Bildirimin alanlarını beş yer okuyordu ve
yinelenen alanda üç ayrı kural uyguluyorlardı:

| Okuyucu | Okuduğu | Kural |
|---|---|---|
| tip denetiminin alan tablosu (`domain/tables.rs` `collect_domain_decls`) | kenar, reset, `trust_level` | son alan |
| RDC (`domain/rdc/facts.rs` `declared_reset`) | reset | son alan |
| SV üretici (`volt-sv-emit` `collect_domains`) | kenar, reset | son alan |
| kısıt modeli (`constraints/walk.rs` `collect_domains`) | reset, `frequency` | reset'te "bir alan `none` diyorsa reset'siz" (`any`) |
| kısıt modeli (`constraints/walk.rs` `apply_edges`) | kenar | ilk alan |

#420'nin örneğinde (`reset = none` ardından `reset = sync active_high`)
SV ve RDC son alanı aldı ve reset senkronizörünü
(`rst_sync_clk_stage0/1`) üretti; kısıt modeli alanı reset'siz saydı ve XDC
o zincirin `set_property ASYNC_REG TRUE` satırını yazmadı (ADR-0054, XDC
satırı). `volt build` tanısız ve çıkış 0 ile bitti: sessiz yanlış.
Kenarda da aynı ayrışma vardı: `clock = posedge` ardından `clock =
negedge` ile SV düşen kenarı kullandı, SDC'nin saat yorumu yükselen kenarı
varsaydı (yalnız yorum satırı; kısıt komutları aynıydı).

## Karar

### 1. Kural

Bir `domain` bildiriminde her alan anahtarı en çok bir kez yazılır. Aynı
anahtarın ikinci yazımı, değeri ne olursa olsun (aynı değer de),
**E1003**'tür (aynı kapsamda çift tanım): birincil etiket ikinci alanda,
ikincil etiket ilk alanda; yardım satırı iki satırdan birinin
silinmesini söyler. Kural ayrıştırıcının tanıdığı her anahtar için
geçerlidir: `clock`, `frequency`, `reset`, `reset_cycles`,
`reset_sequence`, `trust_level`. Üçüncü ve sonraki yazımların her biri
ilk yazıma karşı ayrı bir E1003 alır. Kapsam bildirimdir: iki ayrı domain
aynı anahtarı birer kez yazar.

Bilinmeyen anahtar (W0020) bir alan değildir: hiçbir geçit onu okumaz,
yinelenmesi E1003 almaz (Sınırlar).

### 2. Denetimin yeri

Denetim tek yerdedir: ayrıştırıcı, bildirimi kurduğu yerde
(`volt-syntax/src/parser/item.rs` `repeated_domain_fields`), ortak
okuyucunun `DomainDecl::repeated_fields`'ını okuyarak raporlar. Bildirimin
ilk okunduğu yer orasıdır; ad çözümlemeden ve sonraki geçitlerden
bağımsızdır.

### 3. Tek okuyucu

`volt_ast::domain_fields` bildirimin tek okuyucusudur: `DomainDecl::field`,
`clock_edge`, `reset`, `trust_level`, `frequency`. Beş okuyucu alanı oradan
alır; "son alan", `any` ve "ilk alan" kodda kalmaz. Okunan alan anahtarın
ilk yazımıdır: yinelenen alan zaten bir hatadır, ilk yazım E1003'ün "ilk
burada" dediği alandır; fark yalnız hatalı birimi okuyan yerlerde (LSP,
hatadan sonra koşan denetimler) görünür. `reset = none` ortak okuyucuda
`ResetSync::None` olarak döner.

Yazılmamış alanın varsayılanı okuyucuda kalır: SV, RDC ve kısıt modeli spec
varsayılanını (`sync active_high`, sv-mapping.md) kullanır; tip
denetiminin tablosu yazılmamış reset'e `none` verir. Bu ikincisi #422'nin
bulgusudur ve bu ADR'nin kapsamında değildir; davranışı değişmez.

Okuyucu `volt-ast`'tedir, çünkü ayrıştırıcı, `volt-hir` ve `volt-sv-emit`
ona bağımlıdır (#419'daki `volt_ast::bit_width` ile aynı kalıp).

### 4. Sürüm

Kural daha önce kabul edilen bir girdiyi reddeder. Kabul edilen girdinin
tek bir anlamı yoktu (geçitler farklı alanı okuyordu), bu yüzden bir
sessiz yanlış düzeltmesidir: ADR-0100'ün istisnasıyla son hanede çıkar.
Yeni tanı kodu, seçenek ya da JSON alanı yoktur (ADR-0125 §7); E1003'ün
`volt explain` metnine bu durumu anlatan bir paragraf ve örnek eklenir.
CHANGELOG'da "Davranış değişikliği" olarak yazılır.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; kullanıcıdan yeni bir yazım istenmez, bir
   satır silinir.
2. **Çıkarım (İ2):** kural bir çıkarımı kaldırır. Önce derleyici iki
   değerden birini seçiyordu ve her geçit başka birini seçebiliyordu; tek
   cevaplı değildi. Şimdi belirsizlik bir hatadır ("belirsizlikte hata").
3. **Anlam değişikliği (İ3):** geçerli hiçbir tasarımın anlamı değişmez:
   her anahtarı bir kez yazan tasarımın çıktısı aynıdır (Doğrulama).
   Reddedilen tasarımın tek bir anlamı yoktu; aynı tasarım dünkü kodla
   başka bir devre üretmez, derlenmez. Bu yüzden geçiş uyarısı yoktur;
   değişiklik ADR-0100'ün sessiz yanlış istisnasıdır.
4. **Niyet tahmini (İ4):** derleyici hangi satırın kastedildiğini tahmin
   etmez ve bir satırı kendiliğinden silmez; ileti iki satırı da gösterir.
   Makinece uygulanabilir öneri yoktur.
5. **Metamorfik yeniden yazım:** kural alanların sırasını anlamsız kılar
   (her anahtar bir kez); bildirimin alanlarının sırasını değiştiren bir
   yeniden yazım bu ADR'ye dayanarak eklenebilir. Bu ADR'yi getiren PR onu
   eklemez.

## Doğrulama

- `crates/volt-driver/tests/domain_field_once_tests.rs`: #420'nin örneği
  (iki sırada), her anahtarın ikinci yazımı, aynı değerin yinelenmesi,
  üçüncü yazım, iki domain'de aynı anahtar, bilinmeyen anahtar, iki
  konumlu ileti (JSON), tek reset alanında SV ile XDC'nin aynı cevabı.
  Altısı main'in koduyla düşer. `crates/volt-hir/tests/constraints_tests.rs`:
  kısıt modelinde saatin kenarı ve `reset = none`'da reset zinciri
  olmaması, ortak okuyucudan.
- `tests/ui/fail/317_domain_field_written_twice.volt`,
  `tests/ui/pass/164_domain_every_field_once.volt`,
  `tests/real_bugs/420_domain_reset_written_twice.volt` (ADR-0117),
  tohumlanmış hata türü `rdc.domain-reset-written-twice` (ADR-0113).
- İkili karşılaştırma (ADR-0136'nın yöntemi): main'in (2503284d) ve bu
  dalın `volt` ikilisi gölgenin girdisinin her biri için (493 dosya, 25
  kitap bloğu) `volt build --single-file --emit=sva,sdc,xdc,rust,c,regmap`
  ile çalıştırıldı. 517 girdide çıkış kodu, tanılar ve üretilen dosyalar
  (dalda 1016 dosya) sürüm satırı dışında aynı; tek fark
  `tests/real_bugs/420_domain_reset_written_twice.volt` (main: çıkış 0, üç
  dosya; dal: çıkış 1, E1003, dosya yok).

## Sınırlar

- Bilinmeyen anahtar sayılmaz. Gramerin [V1] anahtarları (`voltage`,
  `always_on`, `retention`, `isolation`) bugün ayrılmış anahtar kelime
  olarak E0003 alır; uygulandıklarında bu kurala girerler.
- Ayrılmış `reset_cycles` ve `reset_sequence` her yazımda E0003 alır;
  ikinci yazım ayrıca E1003 alır.
- Spec bu ADR'yi getiren PR'da değişmez; `grammar-full.ebnf` §3 ve
  `name-resolution.md` §10 için yama bakımcıya ayrı verilir.

## Reddedilenler

- **Her yerde son alan kazanır** (#420'deki a seçeneği). Geçitler
  ayrışmazdı ama kullanıcının yazdığı bir satır sessizce yok sayılırdı;
  iki değer yazan kullanıcı hangisinin geçerli olduğunu öğrenmezdi.
- **Yeni bir tanı kodu.** Son hane yeni kod getirmez (ADR-0125 §7); E1003
  aynı kapsamda çift tanımı zaten anlatır (struct'ta iki kez bildirilen
  alan).
- **Denetimi okuyucularda ya da tip denetiminde yapmak.** Kural yine birden
  çok yerde yazılırdı; bildirimin ilk okunduğu yer ayrıştırıcıdır.
- **Uyarı.** İki cevap bir belirsizliktir; İ2'ye göre hatadır.
