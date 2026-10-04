# ADR-0089: `volt test` — Proje Kökünden Özyinelemeli Test Keşfi

> Statü: Kısmen yerini aldı: ADR-0101 — "Compiling" satırlarındaki `./x_test.volt` öneki kalktı (`x_test.volt`); keşif kuralları yürürlükte ve `volt check` de aynı keşfi kullanır
> Önceki karar: ADR-0033 — test dosyası keşfi
> Tarih: 2026-09-27
> Etkilenen: volt-driver (`sim/discover.rs` yeni, `sim/test_files.rs`,
> `sim/test_cmd.rs`, `sim/test_build.rs`), volt-hir (`unit_load.rs`:
> `Manifest::test_paths`, `parent_dir`; `manifest_search.rs`),
> `templates/*/Volt.toml` yorumu; cli-contract.md §8; ADR-0084 §5 "Yapı
> kararı" güncellemesi
> Kaynak: ADR-0084 §5 (şablon yapısı DÜZ, çünkü `volt test` yalnız
> çalışma dizinini tarıyordu)

## Sorun

`volt test` (argümansız) yalnız çalışma dizinindeki `*_test.volt`'u
tarıyordu (`read_dir(".")`); alt dizindeki test bulunmuyordu. ADR-0084
şablonları bu yüzden düz. Ölçerken iki bağlı hata daha çıktı:

1. **Proje alt dizininde çıplak dosya adıyla derleme `use`'u
   çözemiyordu:** `cd tests; volt check counter_test.volt` →
   E1011 `module not found: 'rtl::counter'`, oysa `volt check
   ../tests/counter_test.volt` çalışıyordu. Neden: `Path::parent()` çıplak
   adda `""` döner ve manifest araması (ADR-0061) göreli yolun başında
   ("göreli yolun başı — tavan görülmedi") duruyordu.
2. `build/sim/<dosya adı>` çıktı dizini: iki alt dizindeki aynı adlı
   test (`a/fifo_test.volt`, `b/fifo_test.volt`) aynı dizine yazardı.

## Karar

**C — A varsayılan + B daraltma.**

- **A:** Proje (`Volt.toml`, ADR-0061 araması çalışma dizininden yukarı)
  varsa proje KÖKÜNDEN özyinelemeli tara. Alt dizinden çalıştırmak da
  bütün projeyi koşar (cargo gibi). Atlananlar:
  - `.` ile başlayan dizinler (`.git`, `.venv`),
  - `build/` ve `target/` — her düzeyde (derleme çıktısı; Volt'un
    varsayılan `--target-dir`'i `build`),
  - kendi `Volt.toml`'u olan alt dizin (iç içe başka proje — onun
    testleri onun kökünden koşar),
  - kökteki `.gitignore`'un DÜZ dizin girdileri: `vendor/` (her
    düzeyde ad), `/out` ve `docs/gen` (kökten yol). Joker (`*?[`), `\`
    ve `!` içeren satırlar yok sayılır — git'in tam desen dili yeni bir
    bağımlılık (`ignore`) ister; düz girdiler pratikte dizin
    dışlamalarının çoğunu karşılar, kalanı B ile daraltılır.
  - sembolik bağlantılar izlenmez (döngü yok), derinlik 32 ile sınırlı.
- **B:** `Volt.toml` `[test] paths = ["tests", "rtl/fifo"]` taramayı bu
  dizinlere (ya da dosyalara, kökten göreli) daraltır.
- Proje yoksa ESKİ davranış: yalnız çalışma dizini, özyinelemesiz.

Neden yalnız B değil: sıfır yapılandırma — `tests/` ya da `rtl/`
altına bir test koymak onu çalıştırmalı; unutulmuş `[test]` satırı
testlerin sessizce koşmaması demekti. Neden yalnız A değil: büyük
projede (vendored IP, üretilmiş kaynak) açık kapsam gerekir.

### Kardeş dosya kuralı alt dizinde

Değişmedi ve her dizinde aynı: `X_test.volt` **aynı dizindeki**
`X.volt`'un modüllerini görür (ADR-0058). `rtl/fifo_test.volt` →
`rtl/fifo.volt`. Başka dizindeki tasarım `use` ile gelir: `tests/`
altındaki bir test `use rtl::counter::Counter` yazar (kaynak kökü
`Volt.toml` `src`'sinden, ADR-0042). Kardeşi dizinler arası aramak
("en yakın `X.volt`") reddedildi: iki `counter.volt` varsa hangisi
olduğu belirsizleşir; `use` açıktır.

### Bağlı düzeltmeler

- Manifest araması göreli yolun başına gelince kanonik yoldan sürer
  (tavan kuralları — git kökü, ev dizini, `VOLT_MANIFEST_DIR` — aynen).
  Çıplak adın dizini `.` sayılır. Göreli önek boyunca yollar göreli
  kalır (iletiler değişmez).
- Çıktı dizini `build/sim/<göreli yol>/<ad>_test/`; kökteki test için
  eskisiyle aynı (`build/sim/counter_test/`).
- "Compiling" satırları çalışma dizinine göreli: kökte `./x_test.volt`
  (eski biçim), alt dizinden `../tests/x_test.volt`.

### Şablon yapısı — DEĞİŞMEDİ (ADR-0084 §5 güncellemesi)

Şablonlar düz kalır (`Volt.toml`, `<ana>.volt`, `<ana>_test.volt`):

- Başlangıç projesi tek tasarım + tek test dosyası; `src/` + `tests/`
  bölmesi `use` satırı ve paket yolu kavramını ilk dakikaya ekler, bir
  şey kazandırmaz.
- `volt check counter.volt` / `volt build counter.volt` şablon README'si
  ve `Next:` satırları çalışma dizininden aynen çalışır.
- `examples/` düz; iki düzen tutarlı.
- Proje büyüyünce alt dizinler artık kendiliğinden çalışır — düz
  başlangıç bir çıkmaz sokak değil. Değişen tek şey şablon `Volt.toml`
  yorumudur ("subdirectories included").

## Doğrulama

- `test_discovery_tests.rs`: kökten özyineleme (kök, kardeşli `rtl/`,
  `use`'lu `tests/deep/`), atlananlar (`build/`, `target/`, `.hidden/`,
  `.gitignore` `vendor/` ve `/out`, iç içe proje), alt dizinden
  çalıştırma, `[test] paths`, manifestsiz düz tarama, alt dizinde çıplak
  adla `volt check`; Verilator varsa (integration işi) gerçek koşu 4/4 ve
  `build/sim/{counter_test, a/counter_test, rtl/counter_test}` ayrı
  dizinler. `discover.rs` birim: `.gitignore` alt kümesi, göreli yol,
  sim anahtarı.
- Docker (`volt-eng`, Linux, gerçek Verilator): keşif testleri 6/6,
  şablon `volt test` zinciri geçti.
- Mutasyon (`build/c4/mutate.py`, E1–E7): kök aramamak, `build/target`'ı
  taramak, iç içe projeye inmek, `.gitignore`'u yok saymak,
  `[test] paths`'i okumamak, göreli yolda aramayı durdurmak (yerelde
  düştü), sim dizinini dosya adına indirmek (yalnız gerçek Verilator'la
  düşer — Docker'da doğrulandı; CI integration işi zorunlu kılar).

## Sonuçlar

- (+) `tests/`, `rtl/<blok>/` düzenleri yapılandırmasız çalışır; alt
  dizinden `volt test` bütün projeyi koşar.
- (+) Proje alt dizininde çıplak adla `volt check`/`build` `use`'u çözer.
- (−) `.gitignore`'un joker satırları desteklenmez (belgelendi; `[test]
  paths` ile daraltılır).
- (−) Alt dizinden çalıştırmada çıktı yine çalışma dizininin `build/`'ine
  gider (bütün komutlarla aynı `--target-dir` kuralı).
