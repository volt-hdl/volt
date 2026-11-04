# ADR-0109: Mutasyon Testi — cargo-mutants, Yalnız Elle Tetiklenen Sekiz Parçalı Koşu, Kapsam ve Hariç Tutma Kuralı

> Statü: Uygulandı
> İlgili: ADR-0107 (her parça araçlı işin araçlarıyla ve `VOLT_REQUIRE_TOOLS=all` ile koşar, defterde sıfır SKIP), ADR-0108 (fark testi mutantları yakalayan testlerden biridir; büyük koşusunun elle tetiklenen parçalı yapısı örnek alındı)
> Tarih: 2026-11-04
> Etkilenen: `.cargo/mutants.toml` (YENİ), `.github/workflows/mutants.yml`
> (YENİ), `scripts/ci/mutants_summary.py` (YENİ),
> `.github/workflows/README.md`, `.gitignore`, `CHANGELOG.md`

## Sorun

Fark testi (ADR-0108) üretilen SV'nin doğruluğunu ölçtü. Testlerin
kendisini ölçen bir şey yok: derleyicinin koduna küçük bir hata girse
testlerin bunu fark edip etmediğini bilmiyoruz. Satır kapsamı (`just
coverage`) bir satırın koştuğunu söyler, sonucunun denetlendiğini söylemez.

Mutasyon testi bunu ölçer. Araç kodda tek tek küçük değişiklikler yapar
(`+` yerine `-`, `<` yerine `<=`, bir fonksiyonun gövdesi yerine varsayılan
değer) ve her biri için bütün testleri koşar. Testler hâlâ geçiyorsa o
değişiklik (mutant) "missed"dir: o davranışı hiçbir test denetlemiyor.

## Karar

### 1. Araç: cargo-mutants 27.1.0, test koşucusu cargo-nextest 0.9.151

- cargo-mutants **27.1.0** (`cargo install cargo-mutants --locked`).
  Yapılandırması depo kökünde `.cargo/mutants.toml`'dır; iş akışı aynı
  sürümü sabit kurar (`CARGO_MUTANTS_VERSION`).
- Her mutant için **bütün çalışma alanının testleri** koşar
  (`test_workspace = true`): üretici hatalarını başka crate'lerdeki testler
  yakalar (volt-driver'daki `difftest_tests`, ADR-0108).
- Testleri **cargo-nextest 0.9.151** koşar (`test_tool = "nextest"`).
  `cargo test` test ikililerini sırayla koşar ve ilk düşen ikilide durur;
  volt-hir ve volt-sv-emit birim testleri bu sırada en sondadır (araçlı
  işte ~506. saniyeden sonra). Bu testlerin yakaladığı bir mutant bütün
  takımın süresine mal olurdu. nextest ikilileri paralel koşar;
  `--max-fail=1:immediate` ile düşen ilk testte o an çalışan testleri de
  keser (fark testi tek başına ~75 saniyedir). nextest doc-test koşmaz; kapsamdaki dosyalarda doc-test
  yoktur.
- `docker_e2e_tests` araçlı işteki gibi adıyla dışarıdadır
  (`-E 'not test(new_test_verify_and_run_through_docker)'`): ~650 MB imaj
  çeker, yerel araçları gizler, kendi işinde koşar (ADR-0107 §2).
- **Temel koşu (baseline) iş akışınındır.** cargo-mutants 27.1.0
  değiştirilmemiş ağacı `test_workspace = true` iken de yalnız mutasyona
  uğrayan paketlerle test eder (`src/lab.rs`, `run_baseline`; yerel denemede
  görüldü: temel koşu `--package=volt-hir`, 1331 test, mutantlar
  `--workspace`). Zaman aşımı bu süreden türetilirse çalışma alanı takımı
  için çok kısadır; ayrıca çalışma alanında düşen bir test temel koşuda
  görünmez ve her mutantı "caught" gösterir. Bu yüzden her parça önce
  değiştirilmemiş ağaçta bütün çalışma alanı testlerini, mutantlarla aynı
  komut ve araçlarla koşar; düşerse parça durur. Süresi ölçülür, cargo-mutants
  `--baseline skip --timeout <iki katı, en az 300 sn>` ile çağrılır.
  Yapılandırmadaki `minimum_test_timeout = 600` açık `--timeout`
  verilmeyen (yerel) koşular içindir.
- `cap_lints = true`: yalnız uyarı ekleyen mutant derlenir.

### 2. Kapsam, 1. aşama

Kapsam `.cargo/mutants.toml`'daki `examine_globs` listesidir; liste dışında
hiçbir dosya mutasyona girmez. Sıra görevdeki sıradır:

1. **SystemVerilog üretimi:** `crates/volt-sv-emit/src/expr.rs` (ifade
   üretimi; öncelik, işaretli dönüşüm ve literal, genişliğe bağlı alt
   ifadelerin yalıtılması `width_dependent`, `tree_width_dependent`,
   `isolate_if_dependent`, öz-belirlenimli konumlar, seçim biçimleri) ve
   `let_width.rs` (tipsiz `let`'in doğal genişliği).
2. **Genişlik ve tür çıkarımı:** `crates/volt-hir/src/ty.rs` ve
   `typeck/`'ten `width.rs`, `binop.rs`, `compare_width.rs` (E2031),
   `cast.rs` (W2010, E2009), `check.rs`, `synth.rs`, `stmt.rs` (`let`,
   doğal genişlikli `let`), `select.rs`.

Kapsamda **952 mutant** vardır (`cargo mutants --list`). Diğer crate'ler bu
turda kapsam dışıdır. Kapsamı süre sınırlar: bir mutantın yakalanmadığını
göstermek bütün takımı koşmaktır (araçlı işte ~8,5 dakika), GitHub'ın bir
işe verdiği süre 6 saattir. volt-sv-emit'in ve volt-hir'in kalan dosyaları
geniş bir taslak listede 3293 mutant ediyordu; 1. aşama yukarıdaki
dosyalardır. Sonraki aşamaların adayları, bu sırayla: `match_expr.rs`,
`lib.rs` (modül ve deyim üretimi), `structs/`, `inline/`,
`builtin_prim.rs`; volt-hir'de `typeck/`'in kalanı.

### 3. İş akışı: yalnız `workflow_dispatch`, sekiz parça

`.github/workflows/mutants.yml`:

- **Yalnız elle tetiklenir** (`workflow_dispatch`). PR'da ve push'ta koşmaz,
  zorunlu kontrol değildir. Bir koşu sekiz işte saatler sürer; her PR'a bu
  maliyeti eklemek CI'ı kullanılmaz kılar. Sonuç bir ölçümdür, kapı değil:
  missed mutant işi düşürmez.
- **Sekiz parça** (`cargo mutants --shard k/8`, `sharding =
  "round-robin"`: mutant i, i mod 8 numaralı parçada; dosyalar parçalara
  karışır, maliyet dengelenir). 952 mutant parça başına 119 eder. Bir
  parça temel koşuyu da yapar (derleme ve bütün takım). Dört parça (fark testinin büyük
  koşusu) mutant başına ~3 dakikalık bir bütçe ile 6 saate sığmazdı; sekiz
  parçada ~2,5 saat payla sığar ve koşu, aynı anda verilen eşzamanlı iş
  sınırının içinde kalır.
- Her parça araçlı işin (ADR-0107) kurulumunu yapar: sabit tarihli OSS CAD
  Suite (Verilator, Yosys, sby, çözücüler); `VOLT_REQUIRE_TOOLS=all`.
  Araçsız bir parça temel koşuda düşer: bir mutantta düşen araçlı test
  "yakalandı" sayılmasın diye araç eksikliği koşuyu mutantlardan önce
  durdurur.
  Fark testi araçlı işteki tohum ve sayıyla koşar (20261104, 2000). Parça
  defterini `--require-zero-skips` ile raporlar.
- `--in-place`: mutant ağacın kopyasında değil checkout'ta denenir, önbellekli
  `target/` yeniden kullanılır; dosya her mutanttan sonra geri yazılır.
  Yerelde kopya kipi kullanılır (§5).
- cargo-mutants 320 dakikada `timeout` ile kesilir; iş 355 dakikada.
  Böylece bitmeyen bir parça da o ana kadarki sonuçlarını yükler ve kırmızı
  görünür (çıkış kodu 124).
- Her parça `mutants.out/` klasörünü ve parçanın mutant listesini
  `mutants-out-<k>` artifact'ı olarak yükler. Son iş
  (`scripts/ci/mutants_summary.py`) parçaları birleştirir ve iş özetine
  toplam / yakalanan / missed / timeout / unviable sayılarını, parça
  tablosunu ve missed listesini (dosya:satır, değişiklik) yazar; birleşik
  listeler `mutants-summary` artifact'ıdır. Eksik ya da bitmemiş parça bu
  işi düşürür.
- Mutasyon skoru için rozet yoktur; README skor iddiası taşımaz.

### 4. Missed mutantların sınıflandırılması ve hariç tutma kuralı

Her koşunun missed listesi üç gruba ayrılır:

- **(A) Eşdeğer mutant:** davranışı gerçekten değiştirmeyen değişiklik (ör.
  yalnız hata metnini, günlüğü ya da performansı etkiler). Gerekçesiyle
  `.cargo/mutants.toml`'daki `exclude_re` listesine girer: her girdi mutant
  adını (`cargo mutants --list` biçimi) eşleyen bir regex'tir, üstünde
  gerekçesini söyleyen bir yorum vardır. **Kodun içine `#[mutants::skip]`
  yazılmaz**: hariç tutmalar tek yerde, gerekçeleriyle okunur.
- **(B) Kör nokta:** davranışı değiştiren ama hiçbir testin yakalamadığı
  değişiklik. Listelenir (dosya:satır, değişiklik, onu açığa çıkaracak Volt
  girdisi tahmini); testi ayrı bir işte yazılır.
- **(C) Şüpheli:** mutant, bugünkü davranışın şartnameye aykırı olduğunu
  düşündürüyorsa. Her biri için başlığında "silent-wrong?" geçen bir issue
  açılır; derleyici o işte düzeltilmez.

Timeout ve unviable sayıları ayrıca raporlanır. Timeout bir mutantın
sonsuz döngüye ya da çok yavaş bir yola soktuğunu gösterir, testlerin onu
yakaladığı anlamına gelir ama ucuz değildir; unviable mutant derlenmez.

### 5. Yerelde

Tam koşu yerelde yapılmaz. Yerelde yalnız `cargo mutants --list`
(kapsamdaki mutantları sayar, derlemez) ve tek bir küçük dosyada `-j 1`
ile yapılandırma denemesi yapılır; deneme kopya kipinde koşar, geçici
kopyaları kendisi siler, `mutants.out/` `.gitignore`'dadır ve iş bitince
silinir. AGENTS.md'nin kısıtlı makine kuralları geçerlidir.

Kopya `TMP`'nin altına açılır. Windows'ta kullanıcı adı ASCII dışı harf
taşıyorsa (`C:\Users\Ç...`) `docker_backend_tests`'in sahte `docker.bat`'ı
günlüğünü yazamaz (testin geçici dizini ASCII yol varsayar, kopyada
`CARGO_TARGET_TMPDIR` kopyanın altındadır) ve dört testi her mutantta
düşer: her mutant yanlışlıkla "caught" görünür. Yerel denemede `TMP` ve
`TEMP` ASCII bir dizine (ör. `C:\mt-tmp`) çevrilir.

## Kanıt

**Kapsam.** `cargo mutants --list`: 952 mutant (expr.rs 500, let_width.rs
64, select.rs 65, stmt.rs 60, ty.rs 44, check.rs 44, width.rs 43,
binop.rs 40, synth.rs 34, cast.rs 29, compare_width.rs 29); `--shard k/8`
her parçada 119.

**Yerel deneme** (Windows, `-j 1`, kopya kipi, `typeck/cast.rs`'in
`check_enum_cast` fonksiyonundaki 8 mutant, `--timeout 900`):

- İlk denemede temel koşu `cargo nextest run --package=volt-hir@0.1.0`
  idi (1331 test, 5 sn), mutantlar `--workspace` (3585 test). §1'in
  bulgusu buradan çıktı. Aynı denemede ilk mutant `docker_backend_tests`'in
  dört testiyle "caught" göründü: kopya `C:\Users\Ç...\AppData\Local\Temp`
  altındaydı (§5). Deneme durduruldu, kopya (18 GB) silindi.
- `TMP` ASCII dizine çevrilerek yeniden: 8 mutant, 7 caught, 1 missed, 9
  dakika, çıkış kodu 2. Yakalanan ilk mutantı ilgili test yakaladı
  (`enum_tests::ui_fail_88_uint_to_enum_cast_e2009`). Missed:
  `cast.rs:107:13: delete match arm (Ty::Enum(_), _) in check_enum_cast`.
  Missed bir mutant bu makinede 32 sn derleme + 23 sn test (araçlı testler
  araç olmadığından atlandı).
- cargo-mutants 27.1.0'da `-E/--exclude-re` ve `--re` alan silme
  mutantlarını ("delete field ... from struct ... expression") süzmedi;
  bu yüzden deneme yapılandırmanın tek dosyalık bir kopyasıyla
  (`examine_globs` yalnız `cast.rs`) koştu. `exclude_re` girdileri ilk
  koşudan sonra bu türde mutantlar için ayrıca sınanır.

**İlk CI koşusu.** Bu bölüm koşu bitince doldurulur.

## Reddedilenler

- **PR'da koşmak.** Sekiz iş, saatler; her PR'ı bekletir. Değişen satırlarla
  sınırlı koşu aşağıda "İleride"dir.
- **`cargo test` ile koşmak.** §1: birim testlerin yakaladığı mutant bile
  bütün takımın süresine mal olur.
- **Yalnız mutasyona uğrayan crate'in testleri** (`test_workspace = false`).
  Üretici hatalarını yakalayan testlerin önemli bir kısmı volt-driver'dadır
  (fark testi, simülasyon testleri); crate'in kendi testleri onları görmez.
- **Kodda `#[mutants::skip]`.** Gerekçesiz, dağınık ve `mutants` crate'ine
  bağımlılık ister; §4.
- **Dört parça.** §3.
- **Rozet ya da README'de skor.** Skor kapsamın ve hariç tutmaların
  fonksiyonudur; kapsam aşama aşama genişlerken bir sayı yanıltır.

## Sınırlar

- Kapsam 1. aşamadır (§2); volt-sv-emit ve volt-hir'in kalanı ve diğer
  crate'ler ölçülmedi.
- Bir mutantın "caught" olması onu yakalayan testin doğru sebeple düştüğünü
  göstermez; yalnız bir testin düştüğünü gösterir.
- Fark testi sabit tohumla 2000 ifade koşar; daha büyük bir koşunun
  yakalayacağı bir mutant burada missed görünebilir.

## İleride: `cargo mutants --in-diff`

PR'larda yalnız değişen satırlardaki mutantları deneyen bir iş
(`git diff origin/main... > pr.diff; cargo mutants --in-diff pr.diff`)
bir PR'ın eklediği kör noktayı o PR'da gösterir. Bu turda kurulmadı:
önce tam koşunun süresi ve missed listesinin eşdeğer kalemleri
(`exclude_re`) oturmalı, yoksa her PR aynı eşdeğer mutantları yeniden
raporlar.
