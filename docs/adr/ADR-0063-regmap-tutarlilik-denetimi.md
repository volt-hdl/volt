# ADR-0063: Register Haritası Tutarlılık Denetimi — `check-regmap` ve `--check-regmap`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-21
> Etkilenen: volt-sw-emit (`check/` YENİ — `mod.rs`, `decls.rs`,
> `parse_c.rs`, `parse_rust.rs`, `parse_json.rs`, `diff.rs`, `hash.rs`,
> `header.rs`, `code.rs`, `rtl.rs`; `c.rs`/`rust.rs`/`json.rs` imza ve
> reset sabitleri), volt-driver (`regmap_check.rs` YENİ; `main.rs`
> `check-regmap` komutu, `build --check-regmap`), volt-diagnostics
> (E9003, E9004), cli-contract.md (§1, §4, §5, §6a, §17).
> DOKUNULMADI: volt-syntax (`desugar_mmio`, RTL üretimi aynı), volt-hir,
> examples/ (kayıtlı üretilmiş sürücü dosyası yok).
> İlgili: ADR-0044 (`@mmio` haritası, reset hep 0), ADR-0053 (HW-SW
> köprüsü; `volt-regmap/1`, "reset anahtarı eklemeli gelecek"),
> ADR-0021 (çıkış kodları), ADR-0004 (tanı sözleşmesi).

## Sorun

`docs/research/rekabet-2026-09.md` §5, aday 3: RTL ile sürücü tutarlılık
denetimi taranan 22 araçta yok; Volt'taki karşılığı yalnız derleyicinin
kendi test paketinde, üç sabit tasarım üzerinde çalışıyordu. Kullanıcıya
sunulmadığı için bir ürün ayrışması değildi.

Gerçek dünyada register haritası hataları üç yoldan doğar:

1. Üreticilerden birinde hata — SV ile sürücü ayrışır.
2. Bayat dosya — firmware deposunda RTL'den geride kalmış başlık.
3. Üretilmiş dosyada elle düzenleme.

"Aynı kaynaktan üretiliyor" (ADR-0053) 1'e karşı kısmen, 2 ve 3'e karşı
hiç korumaz: firmware derlenir, bitstream derlenir, ilk belirti kartta
yanlış register'a giden bir yazmadır.

### Durum tespiti (bu ADR'nin 1. adımı)

`volt-driver/tests/sw_emit_tests.rs::assert_regmap_matches_rtl` (ADR-0053):

| Olgu | Karşılaştırılıyor muydu? | Nasıl |
|---|---|---|
| Adres kümesi (`base + offset`) | evet | SV `wire mmio_whit/rhit = ...` satırlarındaki `32'h` literalleri |
| Okunabilirlik / bus yazması (erişim) | evet | `case (ar_addr)` / `case (aw_addr)` kolları |
| RW register okuma maskesi | evet | `wire [31:0] mmio_<reg>_rd = ... & 32'h..` |
| `@w1c` biti | evet | okuma görünümünde aynı bit |
| Reset değeri | HAYIR | — (harita reset taşımıyordu; ADR-0044: hep 0) |
| Alan adı / konumu | dolaylı | yalnız maske üzerinden |

- RTL tarafı **üretilen SV metni ayrıştırılarak** okunuyordu (HIR değil).
  Doğru seçim: denetlenen şey sentezlenecek metnin kendisi.
- Mantık **test dosyasındaydı** (panik eden yardımcı), girdi olarak
  yalnız `regmap.json` alıyordu; C/Rust sabitleri ayrı bir testte
  (`rust_constants_match_regmap_json`) metin araması ile bakılıyordu.

## Karar

### 1. Ortak görünüm: `DriverView`

Üç biçim de (`.h`, `.rs`, `.json`) bir sürücünün yazılıma VERDİĞİ
SÖZLERE indirgenir: modül, taban adres; register başına ad, offset,
erişim (okuma/yazma erişimcisi var mı), adlandırılmış bit maskesi, reset
değeri; alan başına ad, lsb, kaydırılmamış maske, davranış (`normal` /
`W1C` / `self-clearing`). Adlar `UPPER_SNAKE`'e normalleşir (C
önekleriyle aynı biçim). **Yorum, boşluk ve bildirim sırası görünüme
girmez** — biçim farkı tanım gereği ayrışma değildir.

Erişim yorumdan değil erişimci VARLIĞINDAN okunur (C `<mod>_<reg>_read`/
`_write`, Rust `<reg>_raw`/`set_<reg>_raw`, `trigger_*`, `clear_*`):
sürücünün erişim sözü tam olarak hangi fonksiyonları sunduğudur. C'de
operatif offset erişimcilerin kullandığı adres makrosudur
(`<MOD>_<REG> (<MOD>_BASE + off)`); `_OFFSET` sabiti ondan ayrılırsa
"dosya kendi içinde çelişiyor" kalemi düşer.

### 2. Seviye 1 — `volt build --check-regmap`

Karşılaştırma testten kütüphaneye taşındı: `volt_sw_emit::check::check_rtl(
&DriverView, sv) -> Vec<Drift>`. Eski dört olguya reset eklendi
(sıfırlama dalındaki `mmio_<reg>[_<alan>] <= N'dV` atamaları). Test
yardımcısı artık bu fonksiyonu çağırır (davranış aynı; mutasyon testi
korunur ve tutarlı kaydırma için ikinci bir iddia eklendi).

`--check-regmap` her `@mmio` modülü için istenen sürücü türlerini
(hiçbiri istenmediyse üçünü bellekte) yeniden OKUR, haritanın görünümüyle
ve modülün SV'siyle karşılaştırır. Ayrışma derleyici hatasıdır (ikisi de
aynı haritadan gelir): E9003, konum `@mmio` modülünün adı, çıkış 1.

### 3. Seviye 2 — `volt check-regmap <tasarım> --against <dosya>...`

Asıl değer. `--against` tekrarlanabilir. Sıra:

1. Uzantı `.h`/`.rs`/`.json` değilse → E9004.
2. İmza yoksa → E9004 (elle yazılmış dosya ayrıştırılmaz; sessizlik
   yerine açık tanı). ADR-0053 başlığı olup imzası olmayan dosya
   "eski Volt" diye ayrıca bildirilir.
3. Dosya görünüme indirgenir; görünüm hash'i tasarımınkine eşitse
   ayrıntılı fark atlanır (**hızlı yol**).
4. Değilse kalem kalem fark: offset, erişim, maske, reset, taban, alan
   konumu/maskesi/davranışı, eksik (RTL'de var, dosyada yok) ve fazla
   (dosyada var, RTL'de yok) register/alan, iç çelişki.
5. Dosya ŞU ANKİ Volt sürümünce üretilmişse ve 3-4 temizse, aynı
   haritadan yeniden üretilen metinle yorumsuz belirteç karşılaştırması
   (JSON'da değer karşılaştırması; `doc`/`generator`/`source`/
   `regmap_hash` hariç). Görünümün göremediği el düzenlemesini (getter
   gövdesindeki kaydırma sayısı, JSON `volatile`) yakalar.

Neden kalemi beyan edilen hash'ten türetilir: beyan = tasarım → "üretildikten
sonra düzenlenmiş"; beyan = dosyanın kendi içerik hash'i → "bayat";
ikisi de değil → "başka haritadan üretilmiş ve düzenlenmiş".

**Hızlı yolun sınırı (bilinçli):** beyan edilen hash satırına tek başına
güvenilmez — el düzenlemesinden sonra da yerinde durur. Hızlı yol, dosyanın
İÇERİĞİNDEN yeniden hesaplanan hash'e bakar; dosya her durumda ayrıştırılır
(milisaniyeler).

Çıkış kodları (cli-contract §2 ile): 0 hepsi uyumlu; 1 ayrışma, E9004 ya
da tasarım derleme hatası; 2 tasarımda `@mmio` modülü yok ya da
`--against` verilmedi; 3 dosya okunamadı.

### 4. Üretilen dosyalara imza

```text
// Generated by Volt 0.1.0 from gpio.volt
// regmap-hash: 1f0c3a9d2e4b5a67
```

`.h` ve `.rs` dosyalarının ilk iki satırı (ADR-0053 başlığı hemen ardından);
`regmap.json`'a `regmap_hash` anahtarı. Ayrıca sürücüler reset değerini
kod olarak taşır: C `#define <MOD>_<REG>_RESET`, Rust `<REG>_RESET`, JSON
register başına `reset` (ADR-0053'ün öngördüğü eklemeli genişleme; şema
`volt-regmap/1` kalır). Rust'a C'deki gibi alan başına
`<REG>_<ALAN>_SHIFT`/`_MASK` sabitleri eklendi — alan olguları Rust'ta
yalnız erişimci gövdelerindeydi.

Hash: kanonik metin (`volt-regmap-hash/1`; modül, taban, register'lar
(offset, ad) sırasında, alanlar (lsb, ad) sırasında) üzerinde FNV-1a 64,
16 küçük hex hane. Doc yorumu, kaynak adı, Volt sürümü ve bildirim sırası
girmez. Kriptografik değildir — amaç kasıtsız bayatlık/düzenlemeyi
görmektir, kötü niyetli sahteciliği değil. Yeni bağımlılık yok.

Etkilenen testler: `generators_tests.rs::c_header_has_guard_includes_and_base`
(başlık artık imza ile başlar), `sw_emit_tests.rs::assert_schema`
(`regmap_hash`, `reset` eklendi). Depoda kayıtlı üretilmiş sürücü dosyası
yok (examples/ değişmedi).

### 5. Tanılar

| Kod | Anlam | Nerede |
|---|---|---|
| E9003 | Sürücü dosyası ile RTL arasında register haritası ayrışması | Seviye 2: `--against` dosyasının `regmap-hash` satırı; Seviye 1: `@mmio` modül adı |
| E9004 | Volt'un üretmediği register haritası dosyası (uzantı, imza yok, eski Volt, bozuk imzalı) | `--against` dosyasının ilk satırı |

Kategori "Release discipline" (E9xxx): üretilmiş çıktının kaynağıyla
tutarlılığı, E9002 determinizmiyle aynı ailedendir. Kalem satırları
`= note:` (`GPIO_CONTROL offset: file 0x0C, RTL 0x10`), neden `= reason:`,
çözüm `= help: regenerate with volt build --emit=c gpio.volt`.
`--format json` zarfına `regmap_check` nesnesi eklenir
(`design_hashes`, `files[{path, format, status, drift[{kind, subject,
file, rtl}], cause, fast_path, code_compared, ...}]`).

## Reddedilen seçenekler

- **Yalnız hash karşılaştırması:** bayatlığı görür, el düzenlemesini
  görmez (hash satırı düzenlemeden sonra da doğrudur) ve farkı
  söylemez.
- **Yorumdaki `(ReadWrite)` notundan erişim okumak:** yorum değişikliği
  ayrışma sayılmamalı; erişimci varlığı gerçek sözleşmedir.
- **Elle yazılmış başlıkları sezgisel ayrıştırmak:** hangi makronun
  offset olduğunu tahmin etmek gerçek uyumsuzluğu sessizliğe çevirir.
- **Görünümü HIR'dan almak (Seviye 1):** HIR `@mmio` bilgisini tutmaz
  (ADR-0044 silme ilkesi); denetlenmesi gereken zaten üretilen SV'dir.
- **Rust erişimci gövdelerini sabitlere bağlamak:** bu turda gövdeler
  değişmedi (ADR-0053 testleri gövde metnine bakıyor); gövdeleri aynı
  sürüm için belirteç karşılaştırması korur.

## Sınırlar / Gelecek iş

- **Seviye 3 (gelecek iş, uygulanmadı):** `@mmio` bloğuna salt okunur
  `REGMAP_HASH` register'ı; sürücü açılışta donanımdaki hash'i kendi
  sabitiyle karşılaştırır. Bayat bitstream + güncel firmware (ya da
  tersi) çalışma zamanında yakalanır. 32 bitlik sözcük ⇒ hash'in alt
  32 biti; adres rezervasyonu ve `volt-regmap/1` eklemesi ayrı ADR.
- Başka Volt sürümünün ürettiği dosyada erişimci gövdeleri
  karşılaştırılmaz (üretici çıktısı sürümler arasında değişebilir);
  görünüm yine tam karşılaştırılır.
- JSON'da `volatile`, `type`, `bus` görünüme girmez (C/Rust'ta kod
  olarak yoklar); aynı sürümün dosyasında değer karşılaştırması görür.
- Ad eşlemesi `UPPER_SNAKE` ile yapılır. `irq` + alan `status_rx` ile
  `irq_status` + alan `rx` aynı `_SHIFT` adını verir: önce tasarımdaki
  bölünme aranır, yoksa en uzun register öneki seçilir. SV'deki ham ad
  (`mmio_dataOut_rd`) `upper_snake` ile eşlenir.
- Erişimci adları alt çizgisiz normal biçimde karşılaştırılır (`ab` ve
  `a_b` register'ları aynı `..._read` adına düşer); böyle bir çift
  tasarımda varsa birinin erişimcisinin silinmesi görünmeyebilir.
- Dosyanın BOM'u ve C/Rust satır düzeni (biçimleyicinin dönüş tipinden
  sonra satır kırması, `#  define`, `#[must_use] pub fn` tek satırda,
  iç içe Rust blok yorumu) biçim farkıdır; 32 bite sığmayan değerler
  sessizce kesilmez (JSON'da bozuk dosya, C/Rust'ta iç çelişki).
- Reset değeri haritada hâlâ yok (ADR-0044: hep 0); `reset_value()` tek
  noktadır, sabit kimlik sözcüğü geldiğinde oradan okunacak.
- SVD / IP-XACT çıktısı ve elle yazılmış başlık ayrıştırma kapsam dışı.
- Markdown belgeye (`--emit=regmap-md`) imza eklenmedi; denetlenmez.

## Testler

+47 `volt-sw-emit/tests/check_tests.rs` (her biçimde offset/maske/erişim/
reset/taban/alan konumu/davranış/silinen/eklenen register mutasyonları,
yorum ve biçim değişikliği ayrışma DEĞİL, gövde el düzenlemesi, bayat
sınıflandırma, Volt dışı/eski Volt/bozuk dosya), +1 FNV-1a başvuru
vektörleri (`hash.rs`), +16 `volt-driver/tests/regmap_check_tests.rs`
(CLI çıkış kodları, insan/JSON çıktısı, bayat dosya, `--check-regmap` dört
tasarımda, SV mutasyonları: adres kayması, okuma maskesi, yazma kolu,
reset, W1C biti; camelCase ve belirsiz adlı tasarımlar), +1
`explain_tests.rs`. Bağımsız kod incelemesinin bulduğu yanlış pozitif
(belirsiz alan adı, camelCase SV adı, biçimleyici satır düzeni, BOM) ve
yanlış negatif (başka sürümde standart dışı adres makrosu, 32 bit
kesilmesi) durumları düzeltildi ve bu testlerle sabitlendi. Seviye 1 elle de sınandı:
`desugar_mmio`'ya geçici hata (yazma kolu `addr + 4`, adres kümesi
`addr + 0x100`) sokuldu, `--check-regmap` ikisini de E9003 ile yakaladı,
değişiklik geri alındı.
