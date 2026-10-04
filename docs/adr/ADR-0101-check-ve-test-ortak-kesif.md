# ADR-0101: `volt check` ve `volt test` Ortak Keşif — Tek Keşif Fonksiyonu, Aynı Yol Biçimi, Kardeşsiz Test Dosyasında Tam Denetim

> Statü: Uygulandı
> Önceki karar: ADR-0089 — "Compiling" satırlarında kökteki test dosyasının `./x_test.volt` biçimi; yollar artık öneksiz (`x_test.volt`)
> İlgili: ADR-0095 (proje kipi: argümansız `volt check`'in test dosyalarını denetlemesi, 2026-10-01 eki; bu ADR keşfi `volt test` ile birleştirir ve kardeşsiz test dosyasını kapsar).
> Tarih: 2026-10-04
> Etkilenen: volt-driver (`sim/discover.rs`: `discover_test_files` tek keşif,
> `relative_to` öneksiz; `sim/mod.rs`, `main.rs` `check_project`, `project.rs`),
> volt-hir (`unit_load.rs`: `TestTarget`, `is_test_file_name`; `pipeline.rs`
> `run_semantic_stages` imzası), volt-lsp (`analysis.rs`), kitap
> (`tour/new-check-build.md`, `tour/tests-and-waveforms.md`), `CHANGELOG.md`

## Sorun

Okur testi (Windows, kurulu ikili): `volt new blinky`, `counter_test.volt`
sonuna bozuk bir satır; `volt check` yalnız `Checking counter.volt` yazdı,
"0 error(s)", çıkış 0. `volt test` aynı satırı E0001 ile yakaladı. Kitap
"volt check reads every source file and test file" diyor.

Ölçüm:

- Kurulu ikili 2026-09-29 tarihli; argümansız `volt check`'in test
  dosyalarını denetlemesi 2026-10-01'de geldi (PR #72, `5c4fb97c`,
  ADR-0095 eki). Main'de gerileme yok: aynı proje main'de E0001 ve çıkış
  1 verir. Yeni entegrasyon testleri #72'nin üst commit'inde (`5c4fb97c^`)
  düşer, main'de geçer.
- Kalan üç açık:
  1. Keşif iki yoldan yapılıyordu: `volt test` `discover_test_files`,
     `volt check` ayrı bir `project_test_files_here` kullanıyordu; aynı
     kural, iki giriş.
  2. Yol biçimi ayrışıyordu: `check` `counter_test.volt`, `test`
     `.\counter_test.volt` (ADR-0089: kökte `./x_test.volt`).
  3. Kardeş `X.volt`'u olmayan `X_test.volt` (ör. `sim/deep_test.volt`,
     DUT başka dizinde): `volt test` bilinmeyen modülü E8501 ile bildirir,
     `volt check` ve editör "0 error(s)" der. Boru hattı, kardeşi
     olmayan ve modül taşımayan dosyada modül-varlık denetimini
     atlıyordu (ADR-0095 eki: "testler bilinmeyen bir dosyanın
     modüllerini kullanıyordur").

## Karar

### 1. Tek keşif

`sim::discover_test_files()` (ADR-0089 kuralları: proje kökünden
özyinelemeli, `[test] paths`, `build`/`target`/gizli/iç içe proje ve
`.gitignore` dizinleri atlanır; projesiz çalışma dizini) `volt test` ve
argümansız `volt check` için tek giriştir. Ad kuralı (`*_test.volt`) tek
yerde: `volt_hir::unit_load::is_test_file_name`.

### 2. Yol biçimi

Keşif yolları çalışma dizinine göredir ve `./` öneki taşımaz: kökte
`counter_test.volt`, alt dizinde `sim/deep_test.volt`, alt dizinden
çalışırken `../tests/x_test.volt`. `Checking` ve `Compiling` satırları
aynı yolu basar. Bu, ADR-0089'un "Compiling satırları: kökte
`./x_test.volt`" maddesinin yerini alır; simülasyon çıktı dizini
(`build/sim/<göreli yol>/<ad>_test/`) değişmez.

### 3. `volt check`'in kapsamı

Argümansız `volt check`:

- `[package] src` altındaki her `.volt` kaynağı, üst modül kullansın ya
  da kullanmasın (ADR-0095 `check_roots`: `use` ile yüklenen dosya o
  birimde, kalan her kaynak bir kez);
- `volt test`'in keşfettiği her test dosyası (madde 1), her biri bir
  `Checking` satırıyla.

`volt build`, `volt run` ve `volt verify` üst modülün birimini okur;
tasarımın kullanmadığı bir dosyadaki hata onlarda görünmez, `volt
check`'te görünür. Kitap bunu söyler.

### 4. Test dosyasında tam denetim

`volt_hir::unit_load::TestTarget`:

- `Full(kardeş)`: `*_test.volt` dosyası birim kipinde (`volt check`,
  `volt build`, editörün birim analizi). `volt test` ile aynı tam
  denetim: birim (`use` ile yüklenenler dahil) ve varsa kardeş tasarım
  modüllerin tamamıdır; bulunmayan modül E8501.
- `Partial(kardeş)`: `*_test.volt` olmayan dosya ya da tek dosya
  analizi (birim görülmez); dosyada hiç modül yoksa modül-varlık
  denetimi eskisi gibi atlanır.

## Gerekçe

- İki keşif girişi, iki komutun farklı dosya kümesi görmesine açıktır;
  okurun gördüğü belirti ("check temiz, test hatalı") tam bu sınıftır.
- Aynı dosyanın iki komutta iki farklı adla yazılması okuru iki ayrı
  dosya olduğu sanısına götürür; `./` öneki bilgi taşımaz.
- `volt check` temiz deyip `volt test` derleme hatası veriyorsa check'e
  güvenilmez. `volt test`'in denetimiyle aynı küme ve aynı kaynaklar
  (`compiled.ast` + kardeş) kullanıldığından yeni yanlış pozitif yoktur:
  depodaki 15 `*_test.volt` dosyası temiz kalır.

## Alternatifler

- **`volt test` de `./` ile, `check` da öyle:** iki komut tutarlı olurdu,
  ama `project.rs` kaynak yolları zaten öneksizdi (`Checking
  counter.volt`); öneksiz biçim daha az değişiklik ister. Reddedildi.
- **Kardeşsiz test dosyasını check'te atlamak:** `volt test`'in koşacağı
  bir dosyayı denetlememek tam olarak düzeltilen açıktır. Reddedildi.
- **Editör tüm projeyi tanılasın** (`workspace_diagnostics`): açık
  olmayan dosyaların tanıları ayrı bir LSP yeteneğidir; kapsam dışı,
  issue açıldı.

## Sonuçlar

- **Davranış değişikliği:** `volt test`'in `Compiling` satırları
  `.\x_test.volt` yerine `x_test.volt` yazar. Kardeşsiz, modül
  taşımayan bir `*_test.volt` dosyası `volt check`, `volt build` ve
  editörde artık E8501/E8506 verebilir (önceden yalnız `volt test`).
- Testler: `test_file_check_tests` (sözdizimi hatası, alt dizin ve
  `build/`, top'un kullanmadığı kaynak, iki komutun aynı yolları,
  kardeşsiz test dosyası), `test_discovery_tests` (öneksiz yollar), LSP
  `editor_checks_a_test_file_without_a_sibling_like_volt_test`.
