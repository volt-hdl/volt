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
  başka bir mutantı sessizce gizlemez. Girdiler iki alt gruptadır, yorumu
  etiketini taşır:
  - **A1, koşulsuz eşdeğer:** fonksiyonun kendi içinde, kim çağırırsa
    çağırsın eşdeğer.
  - **A2, varsayıma bağlı eşdeğer:** bugünkü çağıranlara ya da üreticinin
    yazdığı metne bağlı eşdeğer; yorum dayandığı varsayımı söyler. A2'deki
    bir girdinin dayandığı fonksiyon ya da çağıran değişirse, o girdi
    yeniden incelenir.

  **Kodun içine `#[mutants::skip]`
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

Sınıflandırma (§4): 12 missed mutant (A) eşdeğer (7 A1, 5 A2),
`exclude_re`'ye girdi; 144'ü (B) kör nokta; 1'i (`expr.rs:825:34`, `emit_plain`) (C): açılan
fn'in global const'u aynı adlı döngü değişkeniyle değiştiriliyor, issue
[#128](https://github.com/volt-hdl/volt/issues/128). Kör noktaların
listesi PR #127'dedir; testleri ayrı işlerdir.

**On beş parçalı koşu** (41944996471, `0469e3e4`, 12 `exclude_re`
girdisiyle 940 mutant). 00:56–04:27 UTC, 3 sa 30 dk; parçalar 1 sa 59 dk
ile 3 sa 30 dk arası, hepsi 4 saatin altında (§3'ün tahmini 235,5 dk; en
yavaş parçanın mutant başına süresi 193 sn, t = 210 sn'nin altında).
Her parçanın temel koşusu araçlarla geçti (231–397 sn), her parçanın
defteri `SKIP: 0`, cargo mutants her parçada çıkış kodu 2 ile bitti
(missed var). 940 mutantın hepsi test edildi, tamamlanmayan parça yok:

| Toplam | Caught | Missed | Timeout | Unviable |
|---:|---:|---:|---:|---:|
| 940 | 711 | 145 | 0 | 84 |

940 mutantın her biri on üç parçalı koşudaki sonucunu aldı (mutant mutant
karşılaştırıldı, fark yok): hariç tutma yalnız 12 eşdeğer missed mutantı
düşürdü, sonuçlar parça bölümünden bağımsız.

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

## Hedefli koşu (#128, 2026-11-05)

Bir düzeltmenin öldürmesi gereken mutantlar (bir PR'ın kör noktası,
missed listesinin bir dosyası) için on beş parçalı tam koşu gereksiz
pahalıdır. `mutants.yml` iki isteğe bağlı `workflow_dispatch` girdisi
alır; önceki karar metni (§1–§5) değişmedi, girdiler boşken koşu tam
koşudur.

- **`re`**: mutant adına (`cargo mutants --list`'in yazdığı biçim) karşı
  regex; yalnız eşleşen mutantlar denenir.
- **`shards`**: parça sayısı, 1–20. Boşsa §3'ün hesabı, koşunun mutant
  sayısıyla (M = süzülmüş listenin uzunluğu, t = 210 sn, S = 15 dk, en çok
  20): kapsam işi ("Tools and the mutants in scope") `scripts/ci/mutants_target.py shards` ile N'yi bulur, matris
  ve özet ondan kurulur. Tam koşu da artık N'yi böyle bulur; sabit
  `MUTANTS_SHARDS: 15` kalktı. Parça mutanttan fazla olmaz.

**cargo-mutants hatası.** 27.1.0'da `--re` ve `--exclude-re` alan silme
mutantlarını ("delete field … from struct … expression") süzmüyor (bkz.
aşağıda "Hedefli koşunun kanıtı"): `--re 'scope\.rs'` listesine `stmt.rs:161` ve
`let_width.rs:283`'ün alan silme mutantları da girdi. Bu yüzden hedefli
koşu listeyi ikinci kez, satırla daraltır: `mutants_target.py diff`
`--re` listesinden regex'e uyan adları seçer ve hunk'ları tam o satırlara
dokunan bir birleşik diff yazar (her satır kendisine "değişir", diff ağaçla
eşleşir); her `cargo mutants` çağrısı `--re <re> --in-diff target.diff`
ile koşar. Alan silme mutantı istenen bir mutantla aynı satırdaysa yine
sızabilir; bu yüzden kapsam işi son listeyi `mutants_target.py check` ile
doğrular: listede regex'e uymayan bir ad varsa ya da liste boşsa iş düşer.
Her parça kendi listesini kapsam işinin parça listesiyle karşılaştırır; farklıysa
düşer. Süzgeç (`filter.txt`) ve diff `mutants-list` artifact'ındadır.

**Kapsam.** Üreticinin ad çözücüsü `crates/volt-sv-emit/src/scope.rs`
`examine_globs`'a girdi: #128'in düzeltmesi `expr.rs`'teki hijyen
denetimlerini (PR #127'nin missed listesinde `expr.rs` 219, 222, 440, 825
ve `let_width.rs` 222) oraya taşıdı. `expr.rs`'in satırları kaydığından
satır sabitli beş `exclude_re` girdisi yeni satırlarına taşındı (1974 →
1940, 1981 → 1947, 1280 → 1252, 1281 → 1253, 1527 → 1505; aynı satır,
aynı sütun). Kapsam 964 mutant; §3 hesabıyla tam koşu 16 parça
(⌈964 / 16⌉ = 61 · 210 sn = 213,5 dk + 15 dk = 228,5 dk; 15 parçada
⌈964 / 15⌉ = 65, 242,5 dk).

### Hedefli koşunun kanıtı

- **Alan silme hatası.** 41981695174'ün kapsam işi, `--re` listesinden
  regex'e uymayan iki adı düşürdü ve son listeyi doğruladı:
  `dropped (does not match the regex): crates/volt-hir/src/typeck/stmt.rs:161:17: delete field start from struct Span expression …`,
  `dropped (does not match the regex): crates/volt-sv-emit/src/let_width.rs:283:21: delete field hi from struct Range expression …`,
  `all 78 mutants in the list match '…'`.
- **Koşu 1** (41981695174, `re` = `scope.rs` ve hijyen denetimlerinin
  `expr.rs` satırları, 78 mutant, `shards` boş → 2 parça): 36 caught,
  4 missed, 0 timeout, 38 unviable. `scope.rs`'in bütün mutantları
  caught. Missed dördü `expr.rs`'te: `eval_const_depth` ve
  `const_match_key`'in derinlik sayacı (`depth + 1` → `* 1`, `- 1`) ve
  `index_width_mismatch`'in `||` → `&&`'si. Bunlardan üçü gerçek bir eksiği
  gösterdi: `const_match_key` derinlik sınırını hiç denetlemiyordu,
  döngüsel enum const zincirinde üretici yığın taşıyordu (#131'de
  düzeltildi; zincir sınırı #133). Öldüren testler: volt-sv-emit
  `name_scope_tests`'te döngüsel u32 ve enum zinciri ve 64 halkalı zincirin
  E0003'ü (tanı testleri), volt-driver `name_scope_sim_tests`'te 63 halkalı
  zincirin simüle edilen değeri.
  Dördüncüsünü ayıran girdi, tipi adres genişliğinden farklı ve 32
  bit olmayan sabit indekstir (`const K : u4`, `a[K]`); o girdide
  bugünkü çıktı (`a[4'd2]`) Verilator'da WIDTHTRUNC verir, mutantınki
  (`1'((a) >> 4'd2)`) vermez. Bugünkü biçimi sabitleyen test yanlış olurdu
  (ui/pass/135'e eklenen örnek CI'ın Verilator ağına takıldı, 41991580408):
  mutant "#134 düzelene kadar açık".
- **Testler davranışı sınar.** Koşu 1'in caught'larının bir kısmını
  üretilen SV'nin metnini karşılaştıran testler öldürüyordu. Bu testler
  davranış testine çevrildi (`name_scope_sim_tests`: Verilator
  simülasyonu, elle hesaplanmış değerler; AGENTS.md "Emitter tests test
  behavior"); metin testi yalnız biçimi bir ADR tanımlıyorsa kalır
  (esnek `let`'in doğal genişlikte bildirimi, ADR-0025 eki). Davranış
  testlerinden kaçan mutant metin testiyle öldürülmez: A2 (gerekçesiyle)
  ya da bağlı olduğu hata düzelene kadar açık.
- **Koşu 2** (42001466794, testler davranışa çevrildikten sonra; `re`
  Koşu 1'in satırları ve `const_match_key`'in yeni koruması, 85 mutant,
  2 parça): kapsam işi `85 of 87 listed mutants match the regex`,
  `all 85 mutants in the list match '…'`; 46 caught, 1 missed, 0 timeout,
  38 unviable; iki parçada da `Tool ledger [mutants-N]: … SKIP: 0`.
  Koşu 1'in caught'ları ve yeni korumanın mutantları davranış ve tanı
  testleriyle caught. Missed tek mutant `expr.rs:1428`
  (`index_width_mismatch`, `||` → `&&`; Koşu 1'deki satırı 1423):
  "#134 düzelene kadar açık", gerekçesi yukarıda.


## Kapsam genişlemesi ve hedefli koşu (#130, 2026-11-05)

**Kapsam.** `examine_globs`'a altı dosya girdi:

- ayrıştırıcının ikamesi (#130, ADR-0081 eki "Ayrıştırıcının ikamesi
  kapsam zincirinden geçer"):
  - `crates/volt-syntax/src/parser/mono/scope.rs` (kapsam zinciri, 14 mutant);
  - `mono/clone.rs` (zinciri soran klonlayıcı, 128);
  - `mono/pattern.rs` (kolun kapsamı, desen adı, 10);
- PR #131'de `Emitter::resolve` ile ad çözen ama kapsam dışında kalan
  üretici dosyaları: `crates/volt-sv-emit/src/const_array.rs` (37),
  `enums.rs` (71), `trit.rs` (16).

`cargo mutants --list` 964 → 1240 (276 yeni). Hedefli koşudan sonra 4'ü
eşdeğer olarak `exclude_re`'ye girdi (aşağıda): kapsam **1236 mutant**.
§3'ün hesabı (t = 210 sn, S = 15 dk):

```
N = en küçük n ki: ⌈M / n⌉ · t + S ≤ 240 dakika   (n ≤ 20)
M = 1236
N = 20: ⌈1236 / 20⌉ = 62 mutant · 210 sn = 217 dk + 15 dk = 232 dk
(N = 19: ⌈1236 / 19⌉ = 66 · 210 sn = 231 dk + 15 dk = 246 dk > 240)
```

Tam koşu 20 parçadır, yani §3'ün üst sınırında. Kapsam bir kez daha
büyürse ya da t artarsa tam koşu iki koşuya bölünür (§3).

**Hedefli koşu** (42034192061):

- Kapsam:
  - `re` = `(volt-sv-emit/src/(const_array|enums|trit)|volt-syntax/src/parser/mono/(scope|clone|pattern))\.rs:`
  - `shards` boş → 5 parça.
- Kapsam işinin çıktısı:
  - `276 of 278 listed mutants match the regex`;
  - iki alan silme mutantı regex'e uymadığı için düşürüldü (`stmt.rs:161`, `let_width.rs:283`; hata yukarıda);
  - `all 276 mutants in the list match '…'`.
- Özet: `| 276 | 276 | 0 | 140 | 44 | 0 | 92 |` (Mutants, Tested, Not tested, Caught, Missed, Timeout, Unviable).
- Beş parçada da `Tool ledger [mutants-N]: 112 test(s) asked for a tool; 114 lookup(s) found the tool (114 required), SKIP: 0`.

İlk tetikleme (42032815198) temel koşuda düştü. r biçiminin monomorf
dosyaları Verilator'a verilmiyordu (MODMISSING, ADR-0108 eki); koşucu
düzeltildikten sonra koşu yeniden tetiklendi.

| Dosya | Caught | Missed | Unviable |
|---|---:|---:|---:|
| `const_array.rs` | 29 | 0 | 8 |
| `enums.rs` | 48 | 15 | 8 |
| `trit.rs` | 10 | 5 | 1 |
| `mono/scope.rs` | 8 | 1 | 5 |
| `mono/clone.rs` | 44 | 22 | 62 |
| `mono/pattern.rs` | 1 | 1 | 8 |

**Yerine koyma dosyalarının 24 missed'i** (`scope.rs`, `clone.rs`, `pattern.rs`):

- **A1 (2), `exclude_re`:** `fold_binary`'nin `r != 0` korumaları (340, 341) → `true`. r = 0'da `checked_div`/`checked_rem` `None` verir; bu, korumanın düştüğü `_ => None` ile aynıdır. Yol ulaşılabilir (döngü değişkeninin bölümü). Mutantlı derleyicide aşağıdaki üç katlama testi (dar hedef, geniş konumlar, E2010) geçer, geniş testin SV'si `verilator --lint-only -Wall` ile temizdir.
- **A2 (3, biri hariç tutulamaz):**
  - `Concat` kolunun silinmesi. Ulaşılamaz: ayrıştırıcı `Concat` kurmaz, yalnız üreticinin struct indirgemesi kurar (ADR-0077 Karar 5; volt-ast belgesi).
  - `declare_pattern_binding` → `()`. Ulaşılamaz: yük taşıyan enum desteklenmez (E0003); kısaltma bağı SV'ye ulaşmaz. Tanılar aynıdır; elle uygulanınca `enum_patterns_in_a_clone_keep_their_arguments_and_bindings` geçti. Yük taşıyan enum gelince yeniden incelenir.
  - `clone.rs:388` `AutoOrigin`'in `from` alanının silinmesi. Ulaşılamaz: otomatik kontratlar monomorfizasyondan sonra eklenir (`add_auto_contracts` son geçit), klonlanan kontratta `auto` yoktur. cargo-mutants 27.1.0 `exclude_re`'yi alan silme mutantlarına uygulamadığı için listeye giremez (yukarıdaki hata); missed listesinde görünmeye devam eder.
- **B, katlama (5): ilk sınıflandırmada A2 sayıldı, yanlıştı.** `fold_binary`'nin `Sub` kolunun silinmesi, `Div`/`Rem` korumaları → `false` ve `!=` → `==`. İlk gerekçe "katlama yalnız SV metnini değiştirir" idi; bu bir eşdeğerlik kanıtı değil. Yol ulaşılabilir: döngü değişkeni ikame edilince `i - 1`, `i / 2`, `i % 3` iki literalli aritmetiktir. Katlanmamış literal aritmetiğini üretici her literali bağlamın genişliğine keserek yazar (#144):
  - `i / 2` (i = 2, u1): `1'h0 / 1'h0` → 0, beklenen 1;
  - `i % 3` (i = 4, u1): `1'h0 % 1'h1` → 0, beklenen 1;
  - `x < i - 1` (i = 3, x : u1): mutantta E2010 yerine `x < 1'h1 - 1'd1` (`x < 0`).

  Öldüren testler:
  - `parser_scope_sim_tests::folded_loop_arithmetic_fits_a_narrow_target`: Div, Rem; mutantta `left: 0, right: 1`.
  - `parser_scope_diag_tests::a_folded_loop_constant_that_does_not_fit_the_comparison_is_e2010`: Sub.
  - Bekçi: `folded_loop_arithmetic_in_wide_positions_has_its_value` geniş konumlarda (indeks, parça seçimi, aralık, değer) bütün mutantlarda geçer, lint temizdir.

  Her mutant elle uygulandı, testi düştü, geri alındı.
- **B (14), her birine test yazıldı.** Her mutant elle uygulandı, test düştü, mutant geri alındı:

  | Mutant | Testi |
  |---|---|
  | `clone.rs:218` `+` → `*`, `-` (`rename_name`'in bütçe sayımı) | `parser_scope_diag_tests::the_expansion_budget_counts_a_renamed_declaration_once` (40 yinelemede E2027 yok, 1500'de var; eşik bugün 1324) |
  | `clone.rs:253` `&&` → `\|\|` (`check_pattern_name`) | `a_let_of_an_inner_module_level_loop_in_a_pattern_is_a_let` |
  | `clone.rs:264` `enter_scope` → `()`, `clone.rs:268` `leave_scope` → `()` | `parser_scope_sim_tests::a_match_arm_scope_ends_with_the_arm_and_the_block_scope_with_the_block` (Verilator; mutantta `left: 0, right: 3`) |
  | `clone.rs:46` `module_level` → `true` | `a_loop_inside_a_match_arm_is_not_a_module_level_loop` |
  | `clone.rs:536` `\|\|` → `&&` (`bare_param_type`) | `a_type_argument_with_its_own_arguments_is_not_a_bare_parameter` |
  | `clone.rs:666` `Range` kolunun silinmesi | `parser_scope_sim_tests::a_range_select_in_a_clone_reads_the_substituted_bounds` (mutantta derleyici panikler) |
  | `clone.rs:766` `ArrayLit` ve `clone.rs:779` `TupleLit` kollarının silinmesi | `array_and_tuple_literals_in_a_clone_give_the_handwritten_diagnostics` |
  | `clone.rs:809` `copy_attrs_shallow` → `vec![]` | `a_statement_attribute_in_a_generic_module_reaches_the_constraints` (`@multicycle(2)` monomorfun SDC'sinde) |
  | `clone.rs:900` `LetDecl.domain`'in silinmesi | `a_domain_annotation_on_a_let_survives_monomorphization` (E3001) |
  | `pattern.rs:77` `copy_pattern_args` → `None` | `enum_patterns_in_a_clone_keep_their_arguments_and_bindings` |
  | `scope.rs:149` `!=` → `==` (W1002'nin ikincil etiketi) | `w1002_labels_the_shadowed_definition` |

- **C:** yok.

**Diğer üç dosyanın 20 missed'i** yalnız listelenir. Görev gereği bu işte test yazılmadı. Eşdeğerliği gösterilmedi, (B) sayılır:

- **`enums.rs`:**
  - 108 `enum_variant_sig` → `None` (varyant yolunun genişliği; girdi: varyantın genişlik bağlamında kullanımı);
  - 122 `p.segments.len() == 1` → `true` (çok parçalı varyant yolu `resolve`'a gider);
  - 133 `is_prev_call` → `false`/`true` (`match prev(m)`);
  - 155, 157 `==` → `!=` (alt modülün enum tipli çıkışı, `match u.state`);
  - 230, 231 `||` → `&&` (`enum_localparams`'ın ad çakışması: örnek ya da yerleşik örnek adıyla);
  - 289 `!` silme (üç mutant), 293 `|=` → `&=`, 303 `&&` → `||` (`enum_match_plan`: kapsanan kol, varsayılan kol);
  - 328 koruma → `true`, 337 `|=` → `&=` (`pattern_variants`: başka enum'un varyantı, `|` deseni).
- **`trit.rs`:**
  - 75, 80 `&&` → `||` (`is_trit`: `Trit * Trit`, `if`);
  - 103 `trit_sum_sig` → `None`;
  - 127 `+` → `*`, `-` (`try_emit_trit_mul`).
- **`const_array.rs`:** missed yok.

### §4'ün A tanımına ek: eşdeğerliğin kanıtı (#130)

§4'ün karar metni değişmez (AGENTS.md); bu kural onun A tanımına eklenir:

> Üretilen SV metninin değişmesi tek başına eşdeğerlik gerekçesi değildir; eşdeğerlik, ulaşılamazlıkla ya da lint ve simülasyon değerinin aynı kalmasıyla gösterilir.

Uygulaması:

- (a) **Ulaşılamazlık:** bu yola Volt kaynağından ulaşan bir girdi yoktur. Varsayım tek satırla yazılır ve onu sınayan bir tanı ya da davranış testi gösterilir.
- (b) **Ulaşılıyorsa:** mutantlı derleyicinin bu girdi için ürettiği SV `verilator --lint-only -Wall`'dan uyarısız geçer ve simülasyon değeri şartnameden elle hesaplanan değerle aynıdır. Girdi bir davranış testi olarak eklenir; ileride eşdeğerliği bozan bir değişikliği yakalar.

Lint ya da değer farkı çıkarsa mutant A değil B'dir: davranış testiyle öldürülür; bir hataya işaret ediyorsa issue açılır. Bu bölümün katlama mutantları böyle B'ye geçti (#144).

