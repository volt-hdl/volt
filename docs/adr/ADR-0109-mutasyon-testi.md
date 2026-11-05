# ADR-0109: Mutasyon Testi — cargo-mutants, Yalnız Elle Tetiklenen Parçalı Koşu, Kapsam ve Hariç Tutma Kuralı

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

### 3. İş akışı: yalnız `workflow_dispatch`, parça sayısı ölçülen hızdan

`.github/workflows/mutants.yml`:

- **Yalnız elle tetiklenir** (`workflow_dispatch`). PR'da ve push'ta koşmaz,
  zorunlu kontrol değildir. Bir koşu on beş işte saatler sürer; her PR'a bu
  maliyeti eklemek CI'ı kullanılmaz kılar. Sonuç bir ölçümdür, kapı değil:
  missed mutant işi düşürmez.
- **Parçalar** (`cargo mutants --shard k/N`, `sharding = "round-robin"`:
  mutant i, i mod N numaralı parçada; dosyalar parçalara karışır, maliyet
  dengelenir). N iş akışında `MUTANTS_SHARDS`'tır, şimdi **15**; matris
  0..N-1'i sayar, bir adım ikisinin eşitliğini denetler. Her parça temel
  koşuyu da yapar (derleme ve bütün takım).
- **N'nin hesabı.** Hedef: bir parça 4 saati geçmesin (GitHub'ın iş sınırı
  6 saat; aradaki pay yavaş bir parça içindir). Herkese açık depoda aynı
  anda en çok 20 iş koşar; N'nin üst sınırı 20'dir.

  ```
  N = en küçük n ki: ⌈M / n⌉ · t + S ≤ 240 dakika   (n ≤ 20)
  M = kapsamdaki mutant sayısı           = 940
  t = mutant başına ölçülen süre          = 210 sn
  S = parçanın kurulumu ve temel koşusu   = 15 dakika
  N = 15: ⌈940 / 15⌉ = 63 mutant · 210 sn = 220,5 dk + 15 dk = 235,5 dk
  (N = 14: 68 · 210 sn = 238 dk + 15 dk = 253 dk > 240)
  ```

  t, on üç parçalı tam koşunun (41905442445, aşağıda Kanıt) en yavaş
  parçasından ölçüldü: 5. parçanın mutant adımı 73 mutantta 254,8 dakika
  sürdü, mutant başına 209 sn (`outcomes.json`'daki derleme + test
  sürelerinin toplamı adımın süresine eşit). Parça ortalamaları 109–209
  sn, bütün koşunun ortalaması 154 sn; t en yavaş parçanın ortalamasının
  yukarı yuvarlanmışıdır, çünkü süreyi en yavaş parça belirler ve
  round-robin dağılım pahalı mutantları (missed: bütün takım koşar)
  parçalara eşit dağıtmaz. S ölçülen 6–9 dakikalık kurulumun (önbellek
  dolu) önbelleksiz bir koşu için büyütülmüşüdür.

  Önceki hesap (N = 13, t = 170 sn) iptal edilen sekiz parçalı koşunun
  test ettiği 580 mutanttan ölçülmüştü (ortalama 133 sn; yakalanan 118
  sn, missed 329 sn, unviable 2,5 sn; parça ortalamaları 97–167 sn). On
  üç parçalı koşuda iki parça 4 saati geçti (5. parça 4 sa 23 dk, 8.
  parça 4 sa 17 dk): kısmi koşunun ölçtüğü mutantlar bütünü temsil
  etmiyordu. t her tam koşudan sonra yeniden ölçülür.

  **Kapsam büyüdükçe ya da takım yavaşladıkça N yeniden hesaplanır**: M
  `cargo mutants --list`'ten, t son tam koşunun `outcomes.json`'undan (en
  yavaş parçanın ortalaması). 20 parça yetmezse kapsam iki koşuya bölünür.
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
- cargo-mutants 300 dakikada `timeout` ile kesilir (çıkış kodu 124); işin
  sınırı 330 dakikadır (`timeout-minutes`, 6 saatin altında). cargo-mutants
  her sonucu geldikçe yazar; "Upload mutants.out" adımı `if: always()`
  ile koşar, adım düşse, iş iptal edilse ya da sınırına varsa da o ana
  kadarki sonuçlar yüklenir (sekiz parçalı koşu iptal edildiğinde sekiz
  parça da yükledi).
- İlk iş her parçanın mutant listesini (`cargo mutants --list --shard
  k/N`) `mutants-list` artifact'ı olarak yükler. Her parça `mutants.out/`
  klasörünü `mutants-out-<k>` artifact'ı olarak yükler. Son iş
  (`scripts/ci/mutants_summary.py`) parçaları birleştirir ve iş özetine
  toplam / test edilen / test edilmeyen / yakalanan / missed / timeout /
  unviable sayılarını, parça tablosunu ve missed listesini (dosya:satır,
  değişiklik) yazar; birleşik listeler `mutants-summary` artifact'ıdır.
  **Tamamlanmayan parça** (artifact'ı yok ya da listesindeki her mutantın
  sonucu yok) özetin başında "NOT COMPLETED" diye adlandırılır, test
  edilmeyen mutantları listelenir ve özet işi düşer; sessizce atlanmaz.
- Mutasyon skoru için rozet yoktur; README skor iddiası taşımaz.

### 4. Missed mutantların sınıflandırılması ve hariç tutma kuralı

Her koşunun missed listesi üç gruba ayrılır:

- **(A) Eşdeğer mutant:** hiçbir girdi için gözlenebilir çıktıyı
  değiştirmeyen değişiklik; çıktı üretilen SV, tanı kodları **ve tanı
  metinleridir** (ileti, etiket, yardım satırı). Yalnız bir tanının
  metnini değiştiren mutant eşdeğer değildir, (B)'dir: tanı metni
  kullanıcıya görünür ve onu denetleyen bir test mutantı yakalar. Eşdeğer
  olduğu kanıtlanamayan mutant da (B)'dir. Gerekçesiyle
  `.cargo/mutants.toml`'daki `exclude_re` listesine girer: her girdi mutant
  adını (`cargo mutants --list` biçimi) eşleyen bir regex'tir, üstünde
  gerekçesini söyleyen bir yorum vardır. Girdi dosyayı, değişikliği ve
  fonksiyonu adlandırır; satır:sütunu yalnız aynı değişiklik aynı
  fonksiyonda birden çok kez geçiyorsa sabitler. Kod kayınca sabitlenmiş
  bir girdi hiçbir şeyi eşlemez ve mutantı missed listesine geri döner;
  başka bir mutantı sessizce gizlemez. **Kodun içine `#[mutants::skip]`
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

**Kapsam.** `cargo mutants --list`: hariç tutmalardan önce 952 mutant
(expr.rs 500, let_width.rs 64, select.rs 65, stmt.rs 60, ty.rs 44,
check.rs 44, width.rs 43, binop.rs 40, synth.rs 34, cast.rs 29,
compare_width.rs 29); `exclude_re`'nin 12 girdisiyle 940. `--shard k/15`
parça başına 62 ya da 63.

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
  (`examine_globs` yalnız `cast.rs`) koştu. Yapılandırmadaki
  `exclude_re` girdileri alan silme mutantı değildir; yerelde `cargo
  mutants --list` 952'den 940'a indi, düşen 12 mutant girdilerin tam
  olarak eşledikleridir.

**Sekiz parçalı koşular.**

- 41875380104: sekiz parça da "Put the tools on PATH" adımında düştü
  (`cargo-mutants --version` doğrudan çağrılınca alt komut adını ister:
  `cargo-mutants mutants --version`). Düzeltildi.
- 41876659281: sekiz parçanın temel koşusu araçlarla geçti; mutantlar
  17:26–17:29 UTC'de başladı. Ölçülen hızla 119 mutantlık bir parça ~4,4–5,5
  saat sürecekti, 6 saat sınırına çarpacaktı; koşu 20:11 UTC'de iptal
  edildi. İptalde sekiz parça da `mutants.out/`'unu yükledi: 952 mutantın
  580'i test edilmişti (430 caught, 80 missed, 0 timeout, 70 unviable);
  §3'teki t bu 580 mutanttan ölçüldü.

**On üç parçalı koşu** (41905442445, `55d0a57c`). 20:19–00:42 UTC, 4 sa
23 dk; parçalar 2 sa 18 dk ile 4 sa 23 dk arası (5. ve 8. parça 4 saati
geçti, §3). Her parçanın temel koşusu araçlarla geçti, her parçanın
defteri `SKIP: 0`. 952 mutantın hepsi test edildi, tamamlanmayan parça
yok:

| Toplam | Caught | Missed | Timeout | Unviable |
|---:|---:|---:|---:|---:|
| 952 | 711 | 157 | 0 | 84 |

Sınıflandırma (§4): 12 missed mutant (A) eşdeğer, `exclude_re`'ye girdi;
144'ü (B) kör nokta; 1'i (`expr.rs:825:34`, `emit_plain`) (C): açılan
fn'in global const'u aynı adlı döngü değişkeniyle değiştiriliyor, issue
[#128](https://github.com/volt-hdl/volt/issues/128). Kör noktaların
listesi PR #127'dedir; testleri ayrı işlerdir.

## Reddedilenler

- **PR'da koşmak.** On beş iş, saatler; her PR'ı bekletir. Değişen satırlarla
  sınırlı koşu aşağıda "İleride"dir.
- **`cargo test` ile koşmak.** §1: birim testlerin yakaladığı mutant bile
  bütün takımın süresine mal olur.
- **Yalnız mutasyona uğrayan crate'in testleri** (`test_workspace = false`).
  Üretici hatalarını yakalayan testlerin önemli bir kısmı volt-driver'dadır
  (fark testi, simülasyon testleri); crate'in kendi testleri onları görmez.
- **Kodda `#[mutants::skip]`.** Gerekçesiz, dağınık ve `mutants` crate'ine
  bağımlılık ister; §4.
- **Dört, sekiz ya da on üç parça.** §3: parça başına 238 ya da 119
  mutant 6 saate sığmaz; on üç parçada iki parça 4 saati geçti.
- **Test kapsamını daraltmak** (yalnız mutasyona uğrayan crate'in testleri
  ya da araçsız testler). Mutant başına süreyi kısaltırdı, ama üretici
  hatalarını en çok Verilator'lı testler yakalar; parça sayısı artırılır,
  kapsam daraltılmaz.
- **Fazla parça, 20'ye kadar.** Toplam iş süresi aynı kalır, her parça
  kurulumu yeniden öder ve eşzamanlı iş sınırının tamamını alıp aynı
  hesaptaki diğer CI işlerini bekletir; N hedefi tutan en küçük sayıdır.
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
