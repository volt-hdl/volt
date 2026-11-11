# ADR-0109: Mutasyon Testi — cargo-mutants, Yalnız Elle Tetiklenen Parçalı Koşu, Kapsam ve Hariç Tutma Kuralı

> Statü: Uygulandı
> İlgili: ADR-0107 (her parça araçlı işin araçlarıyla ve `VOLT_REQUIRE_TOOLS=all` ile koşar, defterde sıfır SKIP), ADR-0108 (fark testi mutantları yakalayan testlerden biridir; büyük koşusunun elle tetiklenen parçalı yapısı örnek alındı), ADR-0113 (tohumlanmış hata testi derleyiciyi değil tasarımı değiştirir ve güvenlik denetimlerini ölçer)
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

`cargo mutants --list` 964 → 1240 (276 yeni). Hedefli koşudan sonra 7'si
eşdeğer olarak `exclude_re`'ye girdi (aşağıda): kapsam **1233 mutant**.
§3'ün hesabı (t = 210 sn, S = 15 dk):

```
N = en küçük n ki: ⌈M / n⌉ · t + S ≤ 240 dakika   (n ≤ 20)
M = 1233
N = 20: ⌈1233 / 20⌉ = 62 mutant · 210 sn = 217 dk + 15 dk = 232 dk
(N = 19: ⌈1233 / 19⌉ = 65 · 210 sn = 227,5 dk + 15 dk = 242,5 dk > 240)
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

**Diğer üç dosyanın 20 missed'i** sınıflandırıldı. Görev gereği B'lerine bu işte test yazılmadı, yalnız listelendi; A2'lerin kanıtı yazıldı (sona eklenen "eşdeğerliğin kanıtı" kuralı).

| Mutant | Sınıf | Gerekçe ya da açığa çıkaracak girdi |
|---|---|---|
| `enums.rs:108` `enum_variant_sig` → `None` | B | varyant yolunun genişliği: `let t = Mode::B`, `m == Mode::B` |
| `enums.rs:122` `p.segments.len() == 1` → `true` | B | çok parçalı varyant yolu `resolve`'a gider, enum kaybolur: `match Mode::B { … }` |
| `enums.rs:133` `is_prev_call` → `false` | B | `match prev(state) { … }` |
| `enums.rs:133` `is_prev_call` → `true` | B | enum argümanlı fn çağrısı enum sanılır: `match f(state)` |
| `enums.rs:155`, `157` `==` → `!=` | B | alt modülün enum tipli çıkışı: `match u.state` |
| `enums.rs:230`, `231` `\|\|` → `&&` | B | varyant localparam'ı bir örnekle ya da yerleşik örnekle çakışır (`let Mode_A = …`); ui/fail/92 yalnız sinyal çakışmasını sınar |
| `enums.rs:289` `!` silme (3) | B | kapsanmış kolun atlanması: aynı varyantı iki kez adlandıran `match` |
| `enums.rs:293` `\|=` → `&=`, `303` `&&` → `\|\|` | B | bütün varyantlar ve `_` (3 varyant, 2 bit): kod 3 için varsayılan kol değişir |
| `enums.rs:328` koruma `d.name.text == decl.name.text` → `true` | A2, `exclude_re` | ulaşılamaz: tip denetimi başka enum'un varyantını reddeder (E2003 "pattern of type 'Kind' cannot match a value of enum 'Mode'"); test `parser_scope_diag_tests::a_variant_pattern_of_another_enum_is_rejected_before_emission` |
| `enums.rs:337` `\|=` → `&=` | B | `Mode::A \| _` kabul edilir (SV `default: y = 8'd1`); mutantta `_` görülmez |
| `trit.rs:75` `&&` → `\|\|` | B | `Trit * i8` (üçlü MAC, ui/pass/16) Trit sanılır: `(t * a) + (t * a)` |
| `trit.rs:80` `&&` → `\|\|` | B | `if c { t } else { 1 }` Trit sanılır |
| `trit.rs:103` `trit_sum_sig` → `None` | B | `t1 + t2` (i3) genişliği |
| `trit.rs:127` `+` → `*`, `-` (2) | A2, `exclude_re` | ulaşılabilir, SV aynı: `t` `is_trit`'ten geçmiştir (öncelik 12, 11, 10 ya da 0); `emit_prec` yalnız `my_prec < parent_prec` iken parantez koyar (burada `is_right` yanlış). Ebeveyn 7, 6 ve 5 aynı kararı verir. Mutantlı derleyicide `parser_scope_sim_tests::trit_products_of_every_precedence_class_have_their_value` (dört öncelik sınıfı) bayt bayt aynı SV'yi üretir, simülasyon geçer, `verilator --lint-only -Wall` temizdir |

`const_array.rs`: missed yok.

Özet: 20'nin 17'si B, 3'ü A2, A1 ve C yok.

### §4'ün A tanımına ek: eşdeğerliğin kanıtı (#130)

§4'ün karar metni değişmez (AGENTS.md); bu kural onun A tanımına eklenir:

> Üretilen SV metninin değişmesi tek başına eşdeğerlik gerekçesi değildir; eşdeğerlik, ulaşılamazlıkla ya da lint ve simülasyon değerinin aynı kalmasıyla gösterilir.

Uygulaması:

- (a) **Ulaşılamazlık:** bu yola Volt kaynağından ulaşan bir girdi yoktur. Varsayım tek satırla yazılır ve onu sınayan bir tanı ya da davranış testi gösterilir.
- (b) **Ulaşılıyorsa:** mutantlı derleyicinin bu girdi için ürettiği SV `verilator --lint-only -Wall`'dan uyarısız geçer ve simülasyon değeri şartnameden elle hesaplanan değerle aynıdır. Girdi bir davranış testi olarak eklenir; ileride eşdeğerliği bozan bir değişikliği yakalar.

Lint ya da değer farkı çıkarsa mutant A değil B'dir: davranış testiyle öldürülür; bir hataya işaret ediyorsa issue açılır. Bu bölümün katlama mutantları böyle B'ye geçti (#144).


## Hedefli koşu (#144, 2026-11-06)

PR #151'in değiştirdiği kodun mutantları; önceki karar metni değişmedi.

**Kapsam.** Yeni dosya `crates/volt-hir/src/typeck/literal_fold.rs` `examine_globs`'a eklendi. `expr.rs`'in satıra sabitlenmiş `exclude_re` girdileri (`fmt_int` 2040:28 ve 2047:32, `emit_prec` 1352:93 ve 1353:35, `part_select_from_value` 1605:48) yeni kodun kaydırdığı satırlara taşındı; her satırda başka mutantların hâlâ listelendiği denetlendi.

**Koşu.** https://github.com/volt-hdl/volt/actions/runs/42264871411 (94c463c0, iki parça; parça 0 4 sa 20 dk, parça 1 2 sa 47 dk, ikisi de `completed`; her parçada `Tool ledger [mutants-N]: ... SKIP: 0`). `re`:

```
literal_fold\.rs|expr\.rs:.*(fold_literal_arith|has_literal_binary|literal_value|literal_op|pushes_into)|expr\.rs:1016:
```

(`expr.rs:1016` `emit_prec`'in katlanmış literal kolu.) 112 mutant: 93 caught, 14 missed, 1 timeout, 4 unviable.

- **Timeout (1):** `literal_fold.rs:164:26` `+=` → `*=` (`for_each_assignment`'ın sayacı ilerlemez, sonsuz döngü). §4'e göre yakalandı sayılır.
- **Unviable (4):** `literal_vars`, `bounded_loop_var`, `with_loop_values`, `literal_out_of_range`'ın `Default::default()` dönüşleri; derlenmez.
- **A1, A2:** yok.
- **B (9), her birine test yazıldı.** Her mutant elle uygulandı, test düştü, mutant geri alındı. Testler `literal_fold.rs`'in birim testleridir (tanının kodu, mesajı ve notu davranıştır):

  | Mutant | Testi |
  |---|---|
  | `87:37` `==` → `!=` (`collect_literal_vars`, aynı değişken bir kez) | `every_combination_of_nested_loops_is_checked` (mutantta ikinci değişken ortama girmez, derleyici panikler) |
  | `146:35` `>` → `==`, `>=` (bileşim sınırı) | `four_nested_loops_are_checked_in_every_combination` (16⁴ = 65 536 bileşim; u15'e 15⁴ sığmaz). Sınır sonra kaldırıldı (aşağıda, #152); mutantların satırı da kalktı |
  | `185:18` `!` silme (bileşik `-`) | `negation_of_literal_arithmetic_is_folded` (`-(2 + 1)` u2'de E2010) |
  | `286:29` `==` → `!=` (E2023'ün mesajı) | `the_zero_divisor_message_names_the_operator` |
  | `316:29` `==` → `!=` (`push_once`) | `a_diagnostic_is_reported_once_per_position` (`x == 7 / 0`'da tek E2023) |
  | `328:27` `<` → `<=` (u127 aralığı) | `a_target_of_127_bits_or_more_is_not_range_checked` (mutantta `i128` taşması, derleyici düşer) |
  | `336:43` `-` silme (Trit aralığı) | `trit_range_bounds` |
  | `381:54` `==` → `!=` (notun değişkenleri) | `the_note_names_the_failing_iteration_of_a_single_loop` (tek değişkende not boş kalır; iki değişkende mutant aynı notu verir) |

- **C (5): #153, aynı PR'da düzeltildi.** `expr.rs:205:18` `>` → `==`, `>=`; `223:55`, `231:48`, `232:48` `depth + 1` → `depth * 1` (`literal_value`). Mutantlar derinlik sınırını bozar ve daha doğru davranır: 64'ten derin literal ağacında üretici katlamayı bırakıp her literali bağlamın genişliğinde yazıyordu (70 terimlik `1 + … + 1` bölü 10, u3'te 7 yerine 3). §4 C'yi o işte düzeltmez; burada hata mutasyon işinin değil, PR'ın asıl işinin (#144'ün katlaması) eksiğidir ve PR'ın hedefi literal aritmetiğinin şartnameye göre katlanmasıdır. Derinlik sınırı kaldırıldı (özyineleme ayrıştırıcının 256 düzeylik sınırıyla, E0018, zaten sınırlı); `literal_arith_sim_tests::issue_144_a_deep_literal_tree_is_folded_with_its_exact_value` düzeltmeden önce Verilator'da `left: 3, right: 7` ile düştü, sonra geçti. Derinlik parametresi kalktığı için mutantlar da kalktı.

**Koşunun dışında bulunan: #152, aynı PR'da düzeltildi.** B testleri yazılırken bileşim sınırının (`MAX_ASSIGNMENTS` = 65 536) üstü incelendi: aşan `comb` döngü yuvasında sığma denetimi yapılmıyor, üretici kesilmiş değeri yazıyordu (41³ bileşimde `y : u15 = i * j * k` → `15'h7A00`; şartnamede E2010). Şartname bir bileşim sınırı koymaz; sınır kaldırıldı. Maliyet üreticinin açılımıyla aynı mertebede: 4096 × 64 = 262 144 bileşimde `volt check` 0,77 sn'den 0,96 sn'ye (debug derleme), aynı tasarımın SV'si 262 163 satır. `a_loop_nest_above_65536_combinations_is_checked` düzeltmeden önce düştü.

**Düzeltmeden sonraki koşu.** #152 ve #153'ün düzeltmesi (6e600b6a) koşudan sonra yapıldığı için değiştirdiği fonksiyonlar yeniden koşuldu: https://github.com/volt-hdl/volt/actions/runs/42309936490 (6e600b6a, iki parça, ikisi de `completed`; her parçada `Tool ledger [mutants-N]: 132 test(s) asked for a tool; 134 lookup(s) found the tool (134 required), SKIP: 0`). `re`:

```
literal_fold\.rs:.*for_each_assignment|expr\.rs:.*(fold_literal_arith|literal_value)
```

21 mutant: 20 caught, 0 missed, 1 timeout (`literal_fold.rs:154` `+=` → `*=`, sayaç ilerlemez; yukarıdakiyle aynı), 0 unviable. Derinlik sayacının mutantları koddan kalktığı için listede yok.

## Kapsam genişlemesi ve hedefli koşu (#136, #137, #138, 2026-11-08)

PR'ın değiştirdiği fonksiyonların mutantları; önceki karar metni değişmedi.

**Kapsam.** `examine_globs`'a on dosya girdi: ayrıştırıcının ad eşleyen
öteki adımları, #130'un kapsam zincirine bağlandıkları için (ADR-0081 eki
"Ayrıştırıcının öteki adımları da kapsam zincirinden geçer"):

- `crates/volt-syntax/src/parser/bundle.rs`, `bidir.rs`, `stmt.rs`
  (ayrıştırma anındaki `p.drive(v)`), `mmio.rs`, `desugar.rs`;
- `parser/mono/unroll.rs` (modül zinciri ve `eval_const_scoped`'in
  paylaşılan kullanımı);
- `parser/auto_contract/mod.rs`, `scan.rs`, `counter.rs`, `fsm.rs`.

`cargo mutants --list` 1360 → 2478 (bu ekin iki `exclude_re` girdisinden sonra). **Tam koşu §3'e sığmaz:** N = 20'de
⌈2478 / 20⌉ = 124 · 210 sn ≈ 434 dk + 15 dk. #144'ten sonraki 1360 de
sığmıyordu (⌈1360 / 20⌉ = 68 · 210 sn = 238 dk + 15 dk = 253 dk > 240).
#130 ekinin öngördüğü bölme (iki koşu) iş akışında yok. Seçenekler
#156'da, karar bakımcının. Hedefli koşular bu sınırdan etkilenmez.

**Ölçülen t.** Bu koşularda bir mutantın testi 400–680 sn sürdü (derleme
10–34 sn): §3'ün varsaydığı 210 sn'nin iki-üç katı. Araçlı testler
(Verilator, 147–155 araç araması) mutant başına tekrar koşuyor. 64
mutantlık parça bu hızla ~11 saat sürer, `mutants.yml`'ın parça işi
sınırını (`timeout-minutes: 330`) aşar. Bu yüzden ikinci koşu 10 parçaya bölündü (#156'ya
eklendi).

**Hedefli regex.** Değişen fonksiyonlar (`Scopes::module`,
`declare_outer`, `declare_vanishing`, `declare_local`, `is_local`,
`declare_block_local`; `unroll_module`, `eval_const_scoped`;
`parse_block`, `parse_for_stmt`; `bidir.rs`, `bundle.rs`, `mmio.rs`,
`desugar.rs`'in yeniden yazıcıları; `scan_module`, `contract_chain`,
`Walker`'ın `declare`, `scope`, `block`, `if_stmt`, `match_stmt`'i;
`counter`/`fsm` `recognize`, `bound_of`, `is_const_expr`,
`add_auto_contracts`). Kapsam işi: `184 of 189 listed mutants match the
regex`, `all 184 mutants in the list match '…'`. Düşürülen beşi alan
silme mutantı (§ "Hedefli koşu" cargo-mutants hatası).

**Koşular.**

| Koşu | Commit | Liste | Parça | Sonuç |
|---|---|---|---|---|
| 42399483964 | 7602326d | 184 | 3 | İptal. Parça 1/3 runner'ın kapanma sinyaliyle düştü (aşağıda), 0/3 ve 2/3 elle iptal edildi. Düşmeden önce 4 + 6 + 8 missed yazdılar. |
| 42414759728 | bfdc073d | 184 | 10 | Parça 1 yine runner'ın kapanma sinyaliyle düştü; özet `NOT COMPLETED: 1 of 10 shard(s) (1); 19 mutant(s) not tested`, iş düştü (§3). Tamamlanan dokuz parça: 165 test edildi, 136 caught, 27 missed, 0 timeout, 2 unviable; her parçada `Tool ledger [mutants-N]: 147 test(s) asked for a tool; 149 lookup(s) found the tool (149 required), SKIP: 0`. |
| 42418129389 | 1910f9f3 | 24: parça 1/10'un listesi, `stmt.rs:598:32` hariç (18 konum) | 3 | 24 test edildi: 19 caught, 4 missed, 0 timeout, 1 unviable. Üç parçada `Tool ledger [mutants-N]: 153 test(s) asked for a tool; 155 lookup(s) found the tool (155 required), SKIP: 0`. |
| 42425788445 | dc0b7693 | 37 missed konumu (bütün koşuların birleşimi; A1 ve A2 `exclude_re`'de) | 10 | 49 test edildi: 46 caught, 3 missed, 0 timeout, 0 unviable. Missed yalnız üç C mutantı (`unroll.rs:572:61`, `574:64`, `575:64`, #159). On parçada `Tool ledger [mutants-N]: 159 test(s) asked for a tool; 161 lookup(s) found the tool (161 required), SKIP: 0`. |

**Runner'ı düşüren mutant.** `stmt.rs:598:32` (`parse_block`, `&&` →
`||`): `while !self.at(RBrace) && !self.at_eof()` döngüsü dosya sonunda
durmaz. Kapanmamış blokta (`tests/ui/fail`'de var) ayrıştırıcı her
turda bir hata deyimi ekler; bellek dolar ve runner kapanır. İki koşuda
da parça 1, bu mutant listesine geldiği anda `##[error]The runner has
received a shutdown signal` ile düştü. Mutantın test edilen son
mutantından ~1 dk sonra düştü. Parçanın özeti ve `Tool ledger` satırı
yazılmadı, çünkü adım bitmedi. §4'e göre timeout gibi yakalanmış sayılır:
testler sonlanmayan yolu açığa çıkarıyor. Yerelde doğrulandı: kapanmamış `on clk {` bloğunda mutantlı `volt check` 4 sn'de bitmedi, bellek saniyede ~250 MB büyüdü (266, 513, 789, 1017 MB; süreç öldürüldü); mutantsız ikili hemen bitti. Bu mutantı koşudan
çıkarmanın ya da `cargo mutants`'a bellek sınırı vermenin kararı #156'da.

**Sınıflandırma (§4).** Missed'lerin birleşimi (aynı mutant iki koşuda
missed ise bir kez sayıldı). Her B mutantı elle uygulandı, test düştü,
mutant geri alındı. Araçlı testler yerelde Verilator 5.052 imajında
(`VOLT_REQUIRE_TOOLS=verilator`) ya da `volt test`'in Docker arka ucuyla
koştu.

| Mutant | Sınıf | Öldüren test |
|---|---|---|
| `counter.rs:102:28` koruma `c.bound == v` → `false` | B | sim `two_increments_under_the_same_bound_keep_the_counter_contract` |
| `counter.rs:184:45` `&&` → `\|\|` | B | sim `a_guard_on_another_signal_is_not_a_counter_bound` |
| `counter.rs:186:42` `&&` → `\|\|` (aynalanmış kol) | B | sim `a_mirrored_guard_on_another_signal_is_not_a_counter_bound` |
| `fsm.rs:215:44` `&&` → `\|\|` | B | sim `a_register_without_a_match_on_it_is_not_an_fsm` |
| `fsm.rs:269:33` `==` → `!=` (geçiş tekrarı) | B | sim `one_transition_written_twice_in_an_arm_is_one_cover` (mutantta 0 → 1 iki cover) |
| `fsm.rs:280:53` `!=` → `==` (ayrık değer sayımı) | B | sim `an_fsm_that_never_returns_to_reset_keeps_its_transition_covers` (mutantta cover yok) |
| `auto_contract/mod.rs:166:45` `&&` → `\|\|` | B | sim `a_binary_bound_with_one_local_operand_is_not_constant` |
| `auto_contract/mod.rs:168:9` eksi kolu silme | B | sim `a_negated_constant_bound_keeps_the_counter_contract` |
| `scan.rs:197:13` `comb` kolu silme | B | sim `a_match_in_comb_makes_the_register_an_fsm` (mutantta durum cover'ı yok) |
| `scan.rs:198:13` `Assign` kolu silme | A2 | aşağıda |
| `scan.rs:303:49` `\|\|` → `&&` | B | sim `a_constant_written_in_a_loop_gets_no_counter_contract` (mutantta `auto counter wrap`) |
| `scan.rs:366:20` koruma → `true`, `366:42` `&&` → `\|\|` | #138 hücresi a05b | sim `a_match_on_a_local_enum_named_like_the_state_keeps_the_fsm` (aşağıda) |
| `bidir.rs:132:54` koruma → `true`, `157:13` `(BIDIR_READ, _)` kolu silme | B (tanı metni) | `bidir_tests::e4008_names_a_read_statement_and_the_drive_low_arity` |
| `bidir.rs:144:13` kol silme, `147:47` `==` → `!=` | B | aynı test |
| `bidir.rs:211:17` extern kolu silme | B | `bidir_tests::an_extern_inout_port_must_be_scalar` |
| `bundle.rs:237:39` `\|\|` → `&&` | B | sim `no_protocol_check_on_a_bundle_array_port_drops_its_assumptions` |
| `bundle.rs:446:24` koruma → `true` | B | diag `a_bundle_array_length_outside_1_to_256_is_e2008` |
| `bundle.rs:842:38` koruma → `true`, `842:60` `&&` → `\|\|` | B | diag `a_qualified_path_is_not_a_bundle_port` |
| `mmio.rs:1142:20` `!` silme | B | diag `a_module_level_write_to_a_hardware_field_is_rewritten` |
| `mmio.rs:1323:38` koruma → `true`, `1323:60` `&&` → `\|\|` | B | diag `a_qualified_path_is_not_the_register_handle` |
| `desugar.rs:549:43` `<` → `<=` | A1 | aşağıda |
| `unroll.rs:564:14` `>` → `>=`, `==` | B | `unroll_tests::a_bound_nested_64_deep_is_folded` |
| `unroll.rs` 577, 579, 580, 581, 582, 583: `+`, `*`, `/`, `%`, `<<`, `>>` kolunu silme | B | `unroll_tests::every_arithmetic_operator_of_a_bound_is_folded` |
| `unroll.rs:589:50` `depth + 1` → `depth * 1` (const adı) | B | `unroll_tests::a_const_cycle_in_a_bound_is_an_error_not_a_stack_overflow` (mutantta yığın taşar) |
| `unroll.rs` 572:61, 574:64, 575:64 `depth + 1` → `depth * 1` | C | #159 |
| `stmt.rs:598:32` `&&` → `\|\|` | timeout gibi (yukarıda) | runner'ı düşürür |

Özet: 37 konumda 38 missed mutant. 33'ü B ya da #138 (testi yazıldı), 1'i A1, 1'i A2, 3'ü C (#159). Ek olarak `stmt.rs:598:32` sonlanmıyor.

- **A1 (1), `exclude_re`:** `desugar.rs:549:43` `<` → `<=` (`rw_expr`):
  `Some(s) if vstage == s` kolu bir önceki koldur. `vstage == s`'de `<=`
  kolu hiç çalışmaz.
- **A2 (1), `exclude_re`:** `scan.rs:198:13`, `scan_module`'ün
  `StmtKind::Assign` kolunun silinmesi. Ulaşılabilir ama gözlenemez:
  register'a modül düzeyinde `=` her zaman E0019'dur
  (`register_assign.rs`, ADR-0098). Kol silinince register sayaç
  kontratı alır, ama derleme E0019'da durur ve tanılar aynıdır
  (`md5` aynı). Dayanağı test:
  `parser_rewrite_scope_diag_tests::a_counter_assigned_at_module_level_is_only_e0019`.
- **C (3): #159.** `eval_const_scoped`'un derinlik sayacı (`unroll.rs`
  572:61, 574:64, 575:64, `depth + 1` → `depth * 1`). Mutantlar 64'ten
  derin ifadeyi kabul eder: daha doğru davranırlar. Şartnamede derinlik
  sınırı yok: 65 terimlik `1 + … + 1` sınırı E2021 alıyor, döngüsel const
  E2020 yerine E2021 alıyor, 64 halkalı zincirin sayacı kontratsız
  kalıyor. §4'e göre bu işte düzeltilmedi, #159 düzelene kadar açık.
  Aynı sayacın const adı kolundaki mutantı (589:50) döngüsel const'ta
  yığını taşırır, B olarak öldürüldü.
- **Bulunan hata: #138'in bir hücresi (a05b), aynı PR'da düzeltildi.**
  `scan.rs:366:20` (`match_stmt`'in `is_local` koruması → `true`). Bu
  PR'ın koruması `main`'de yoktu. `comb`'da FSM register'ını gölgeleyen
  `let st : Color`'ın `match`'i register'ınki sayılıyordu. `Color::R`
  deseni `u2` register'ına uymadığından FSM düşüyordu (3 geçiş cover'ı →
  0). Test `main` ikilisinde düşüyor.

## Hedefli koşu (#139, 2026-11-08)

PR #164'ün değiştirdiği fonksiyonların mutantları; önceki karar metni değişmedi.

**Kapsam.** `examine_globs`'a dört dosya girdi: dosya sınırındaki görünürlük kuralı (`crates/volt-ast/src/visibility.rs`, YENİ), dosya kapsamlarını kuran import denetimi (`crates/volt-syntax/src/unit.rs`, `volt-hir`'den taşındı), çözücünün kök araması (`crates/volt-hir/src/resolve/imports.rs`) ve Handshake payload'ı (`crates/volt-syntax/src/parser/handshake.rs`).

**Hedefli regex.** Değişen fonksiyonlar: `RootFilter::root_name`, `sees`, `pick`, `item_name`, `mono_base`; `check_entry`; `Resolver::scope_get`; `Scopes::module`, `invisible_name`, `declared_elsewhere`, `invisible_name_error`; `collect_consts`, `visible_const`, `eval_const_scoped`, `Unroller::bounds`; `bundle.rs`'in `visible_type`, `expand`, `bundle_name`, `is_bundle_type`, `collect_bundle_defs`, `expand_port`, `reject_bundle_payload`, `bundle_array_len`, `bundle_index`'i; `handshake.rs`'in `PlainDefs::lookup`, `is_comparable`, `payload_fields`, `collect_plain_struct_defs`, `expand_handshake`'i; `value_of`. Kapsam işi: `186 of 191 listed mutants match the regex`, `all 186 mutants in the list match '…'`. Düşürülen beşi alan silme mutantı (§ "Hedefli koşu" cargo-mutants hatası).

**Koşu.** https://github.com/volt-hdl/volt/actions/runs/42457156304 (428fe0d3, üç parça; parça 0 3 sa 7 dk, parça 1 5 sa 8 dk, parça 2 3 sa 54 dk, üçü de `completed`; her parçada `Tool ledger [mutants-N]: 165 test(s) asked for a tool; 167 lookup(s) found the tool (167 required), SKIP: 0`). 186 mutant: 144 caught, 26 missed, 0 timeout, 16 unviable.

**Sınıflandırma (§4).** Her B mutantı elle uygulandı, test düştü, mutant geri alındı.

| Mutant | Sınıf | Öldüren test |
|---|---|---|
| `auto_contract/mod.rs:119:51` `==` → `!=`, `119:34` koruma → `false`, `121:32` koruma → `false` | B | `auto_contract_tests::enum_typed_const_initial_state_is_followed_to_its_variant` |
| `auto_contract/mod.rs:119:34` koruma → `true` | A2 | aşağıda |
| `auto_contract/mod.rs:121:32` koruma → `true` | açık, #166 | aşağıda |
| `bundle.rs:992:14` `>` → `>=`, `==` (`expand`) | B | `bundle_tests::nesting_exactly_eight_deep_is_flattened` |
| `handshake.rs:145:14` `>` → `>=`, `==`, `155:23` `+` → `*` (`payload_fields`) | B | `bundle_tests::handshake_payload_nesting_limit_counts_the_leaf` |
| `handshake.rs:125:9` yol kolu silme (`is_comparable`) | B | `bundle_tests::builtin_handshake_enum_payload_gets_a_stability_contract` |
| `handshake.rs:126:50` `&&` → `\|\|` (`is_comparable`) | B | `bundle_tests::builtin_handshake_generic_struct_payload_skips_stability_contract` (mutantta kullanıcı `tx.data` W1001'ini görmüyordu) |
| `handshake.rs:178:40` koruma → `true`, `178:51` `&&` → `\|\|` | B | `bundle_tests::builtin_handshake_bare_generic_struct_payload_stays_opaque` (mutantta E0003 yerine E1001 'T') |
| `scope.rs:112:22` `!=` → `==` (`Scopes::module`, takma ad) | B | `cross_file_visibility_tests::an_alias_in_a_non_constant_bound_keeps_e2021` |
| `scope.rs:303:30` koruma → `true` (`invisible_name`) | B | `cross_file_visibility_tests::a_qualified_path_bound_keeps_e2021` |
| `scope.rs:308:9` ikili kol silme | B | `cross_file_visibility_tests::private_const_of_another_file_inside_a_binary_bound_is_e1001` |
| `scope.rs:311:9` eksi kolu silme | B | `cross_file_visibility_tests::private_const_of_another_file_under_a_negation_is_e1001` |
| `scope.rs:323:40` `&&` → `\|\|`, `323:86` `==` → `!=` (`declared_elsewhere`) | B | `cross_file_visibility_tests::an_undefined_name_beside_another_file_keeps_e2021` |
| `scope.rs:334:9` `pub` kolu silme (`invisible_name_error`, yardım metni) | B | `cross_file_visibility_tests::the_e1001_help_says_whether_the_name_is_pub` |
| `unroll.rs:465:31` `\|\|` → `&&` (`bounds`, kurtarılmış sınır) | B | `unroll_tests::a_missing_range_end_is_only_e0001` |
| `unroll.rs:522:14` `-` → `+` (`bounds`, açılım sınırı) | B | `unroll_tests::unroll_limit_counts_iterations_not_bound_values` |
| `unroll.rs:608:61`, `610:64`, `611:64` `depth + 1` → `depth * 1` | C | #159 |

Özet: 26 missed. 21'i B (testi yazıldı), 1'i A2, 3'ü C (#159), 1'i açık (#166).

**Doğrulama koşusu.** https://github.com/volt-hdl/volt/actions/runs/42480097838 (3ffc2f84, testler eklendikten sonra; 10 parça, en uzunu 52 dk). `re` yalnız missed konumlarıdır (aynı satırdaki öteki mutantlarla):

```
^crates/volt-syntax/src/parser/(auto_contract/mod\.rs:(119|121):|bundle\.rs:992:|handshake\.rs:(125|126|145|155|178):|mono/scope\.rs:(112|303|308|311|323|334):|mono/unroll\.rs:(465|522|608|610|611):)
```

`41 of 46 listed mutants match the regex` (A2 `exclude_re` ile listede yok; beşi alan silme). 41 mutant: 37 caught, 4 missed, 0 timeout, 0 unviable. Missed yalnız beklenen dördü: `unroll.rs` 608:61, 610:64, 611:64 (C, #159) ve `auto_contract/mod.rs:121:32` → `true` (açık, #166). On parçada `Tool ledger [mutants-N]: 166 test(s) asked for a tool; 168 lookup(s) found the tool (168 required), SKIP: 0`.

- **A2 (1), `exclude_re`:** `auto_contract/mod.rs:119:34` (`value_of`'un `p.segments.len() == 1` koruması → `true`). Önceki kol `len() >= 2`'yi alır; `true` aynı `len() == 1` yollarını eşler, fark yalnız segmentsiz yolda olurdu. Ulaşılamazlık (§4 eki (a)): dilbilgisi yolu tanımlayıcıyla başlatır (`parse_path`), `parser/`'daki her yeniden yazım en az bir segment kurar. `value_of`'un okuduğu iki konumda (register'ın başlangıç değeri, FSM yazmasının sağ tarafı) boş yol kurmaya çalışan girdiler (`::Idle`, `State::`, `State::::Idle`, `::Run`, boş ifade) E0001'dir ve AST'de segmentsiz yol bırakmaz: `auto_contract_tests::inputs_that_try_an_empty_path_are_e0001_and_leave_no_empty_path`.
- **Açık (1): #166.** `auto_contract/mod.rs:121:32` (const'un değeri yolsa izleme koruması → `true`). Mutant daha yeteneklidir: başka bir const üzerinden tanımlanan enum const'u (`A = B`, `B = State::Idle`) varyantına izler ve FSM 4 otomatik kontrat alır; bugün 0 alır (#139'dan önce de). Tamsayı zincirinde fark yok. Tanıma ADR-0066'ya göre muhafazakârdır, eksik cover yanlış çıktı değildir: C değil, iyileştirme. Sınırlama test ile sabitlenmedi; #166 kapanana kadar açık.
- **C (3): #159.** `eval_const_scoped`'un eksi ve ikili kollarındaki derinlik sayacı. Mutantlar 64'ten derin ifadeyi kabul eder; şartnamede sınır yok. #136–#138 koşusunun aynı mutantlarıyla aynı sınıf.
- **Bulgu, #167:** `unroll.rs:465` için girdi aranırken görüldü. Kurtarılmış bir alt ifade içeren sınır (`0..2 + {`) E0001'in ardından E2021 kaskadı alıyor. Tüm sınır kurtarılmışsa (`0.. {`) almıyor. Açık hata, yanlış sonuç değil.
- **Not:** `RootFilter::sees` tanım dosyasına bakar, öğeye değil. Bir dosya `use other::K as J` ile getirirse ve `other`'da private bir `J` de varsa, `Scopes::module` o `J`'yi de kök çerçeveye koyar. Yanlış sonuç yok: `J` zaten takma adla kök çerçevededir ve değeri `root_name` ile `K`'ye çözülür. `scope.rs:112:22`'nin öldürücü girdisi bu yüzden private `J`'yi üçüncü bir dosyaya koyar.

## Kapsam genişlemesi ve koşu (#189, #193, 2026-11-11)

PR #200'ün (ADR-0114, modül özeti) dosyaları; önceki karar metni değişmedi.

**Kapsam.** `examine_globs`'a beş dosya girdi: özet ve kombinasyonel graf (`crates/volt-hir/src/summary.rs`, YENİ), onu okuyan E4007 denetimi (`crates/volt-hir/src/handshake.rs`), saatsiz hedefin çıkış alanı ve örnek sitesindeki E3001 (`crates/volt-hir/src/domain/instance.rs`, `crates/volt-hir/src/domain/assign.rs`) ve yerleşik primitif tablosu (`crates/volt-ast/src/builtin.rs`). İlk hedefli koşu (42670004438, 6379b29) yalnız değişen satırları denedi: 114 mutant, 92 caught, 9 missed, 13 unviable; sekiz B'nin testi yazıldı, `Walker::block_reads → ()` A2 olarak `exclude_re`'ye girdi (sınıflandırma PR #200'ün açıklamasında). Bu ek beş dosyanın tamamını dener; eski kodun kör noktaları da bu yüzden listede.

**Koşu.** https://github.com/volt-hdl/volt/actions/runs/42888144192 (52b9f402, `re` = `(volt-hir/src/(summary|handshake|domain/instance|domain/assign)|volt-ast/src/builtin)\.rs:`; `228 of 233 listed mutants match the regex`, düşürülen beşi alan silme mutantı; dört parça: 3 sa 1 dk, 3 sa 2 dk, 4 sa 36 dk, 3 sa 50 dk, dördü de `completed`; her parçada `Tool ledger [mutants-N]: 173 test(s) asked for a tool; 175 lookup(s) found the tool (175 required), SKIP: 0`). 228 mutant: 178 caught, 24 missed, 0 timeout, 26 unviable. 24 missed'in 2'si PR #200'ün satırlarında (`instance.rs:471`, `summary.rs:590`), 22'si eski kodda.

**Sınıflandırma (§4).** Her B mutantı atılabilir bir worktree'de elle uygulandı, test düştü, dosya geri yazıldı ve `git diff` her adımda mutanttan önceki hâliyle aynıydı. Beklenen değerler: parametre adları ve aralıklar ADR-0029'dan ("Sabit generic kuralları"), imza kalıpları ADR-0027/0029/0049'dan; alan kararları `domain-inference.md` K5, K6, K8'den.

| Mutant | Sınıf | Öldüren test |
|---|---|---|
| `builtin.rs:353` `const_param_name` → `""`, `"xyzzy"`; `354`, `355`, `356` kol silme (Counter, ShiftRegister, arbiterler) | B | `builtin_param_diag_tests::an_invalid_size_names_the_parameter_and_its_rule_in_both_languages`, `::a_non_literal_size_or_a_wrong_arity_shows_the_signature_in_both_languages` (mutantta `Counter DEPTH is out of range`) |
| `builtin.rs:384` `const_rule_hint_en`, `394` `const_rule_hint_tr` → `""`, `"xyzzy"` | B | `builtin_param_diag_tests::an_invalid_size_names_the_parameter_and_its_rule_in_both_languages` (iki dilde E2025 yardımı) |
| `builtin.rs:404` `generic_shape` → `""`, `"xyzzy"` | B | `builtin_param_diag_tests::a_non_literal_size_or_a_wrong_arity_shows_the_signature_in_both_languages` (E2008 ve E2003 yardımı) |
| `assign.rs:100:42` `==` → `!=` (`check_assign`) | B | `domain::assign::tests::the_source_label_of_a_conditional_assignment_is_where_its_domain_comes_from` (mutantta kaynak etiketi koşul `f` yerine `true`) |
| `assign.rs:404:13` `(_, Timeless) \| (Timeless, _)` kol silme (`check_compat`) | B | `domain::assign::tests::a_constant_driver_leaves_a_wire_unconstrained` (mutantta tel sabite bağlanır, E3001 kaybolur: yanlış sonuç) |
| `assign.rs:405:13` `(Error, _) \| (_, Error)` kol silme | B | `domain::assign::tests::a_driver_whose_join_failed_leaves_a_wire_unconstrained` (mutantta ikinci E3001 kaybolur) |
| `instance.rs:94:64` `==` → `!=`, `97:16` `!` silme (`check_child_clock_edges`) | B | `domain::edgeless::tests::the_e3016_of_a_child_clock_is_at_its_clock_binding` (veri girişi sabitken mutantta E3016 yok) |
| `instance.rs:209:46` `==` → `!=` (`err_clock_conflict`) | B | `domain::instance::tests::the_e3014_message_says_whether_the_domain_is_symbolic` |
| `instance.rs:292` koruma → `false`, `292:44` `==` → `!=` (`expected_port_domain`) | B | `domain::instance::tests::an_unbound_symbolic_domain_is_not_checked` (mutantta bağlanmamış sembolik alanda yanlış E3001) |
| `instance.rs:292` koruma → `true`, `292:44` `==` → `!=` | B | `domain::instance::tests::an_unbound_child_clock_leaves_the_other_operand_checked` (mutantta @Slow işlenenin E3001'i kaybolur) |
| `instance.rs:471:32` koruma → `true` (`merge_fix`) | B | `module_summary_tests::the_merge_fix_synchronizes_one_unannotated_input_or_none`, yeni `merge-two-annotated-same` durumu (aynı alanda iki anotasyonlu giriş; mutantta öneri yok) |
| `instance.rs:642:9` `propagate_binding` → `()` | A2 | aşağıda |
| `summary.rs:590:43` `+` → `-`, `*` (`instance_output`, kesim sayacı) | A2 | aşağıda |

Özet: 24 missed. 21'i B (testi yazıldı), 3'ü A2, A1 ve C yok.

- **A2 (2), `summary.rs:590:43`:** kesim sayacı (`cuts`) yalnız açık bir (örnek, çıkış) açılımına yeniden girilince artar. Ulaşılamazlık (§4 eki (a)): özet temiz isim çözümlemesinden sonra hesaplanır (`pipeline.rs`); bir örneğin bağlaması ancak önce bildirilmiş örneklerin çıkışlarını okuyabilir, sonraki örneği ya da kendi çıkışını okumak E1002'dir (`summary::tests::an_instance_binding_cannot_read_a_later_or_its_own_instance`). Örnekler üzerinden döngü bir `wire`'dan geçer; tel düz bir okumadır, açılım özyinelemesine girmez. `-` mutantıyla `a_combinational_loop_through_instances_terminates` taşma paniği vermeden geçer; iki mutantla da özet birim testleri ve `module_summary_tests` geçer. Aynı fonksiyonda üç satır aşağıdaki açılım sayacının aynı mutantları yakalandığından girdi `590:43`'ü sabitler. Önce yazılan bir döngü testi (doğrudan `let p = Pass { a: q.y }`, `let q = Pass { a: p.y }`) özette `o1` için eksik sonuç verdi; girdi geçersizdi (E1002), test bu kanıta çevrildi.
- **A2 (1), `instance.rs` `propagate_binding → ()`:** hedefte olmayan bir porta bağlama çözümlemede E1009'dur (Volt modülü ve yerleşik primitif) ve derleme çözümleme hatasından sonra alan çıkarımına girmez (`pipeline.rs`); fonksiyon bugün yalnız kapısız `volt_hir::analyze` üzerinden koşar, onu da yalnız testler çağırır. `domain::instance::tests::a_binding_to_a_missing_port_is_e1009_in_name_resolution` mutantla geçer.
- **Not:** `assign.rs:404` ve `405`'in testleri şartnamenin K6 sözde kodunu izler (`(_, Timeless) => {}`, `(Error, _) | (_, Error) => {}`): kısıtsız bir alan değişkeni sabite ya da hataya bağlanmaz. 404'ün mutantı sessiz yanlış sonuçtur (bir CDC ihlali kaybolur); bugünkü kod doğru, yalnız test eksikti.

**Doğrulama koşusu.** https://github.com/volt-hdl/volt/actions/runs/42931607163 (ffd8c507, testler eklendikten sonra; tek parça, 2 sa 47 dk). `re` yalnız missed konumlarıdır (aynı satırdaki öteki mutantlarla):

```
^crates/(volt-ast/src/builtin\.rs:(353|354|355|356|384|394|404):|volt-hir/src/(domain/assign\.rs:(100|404|405):|domain/instance\.rs:(94|97|209|292|471|642):|summary\.rs:590:))
```

`25 of 30 listed mutants match the regex` (düşürülen beşi alan silme mutantı; A2 `exclude_re` ile listede yok: `summary.rs:590:43` ve `propagate_binding → ()` listede görünmez). 25 mutant: 21 B ve aynı satırlardaki 4 komşu (`instance.rs:97:42` `||` → `&&`, `97:45` `!` silme, `471:32` koruma → `false`, `471:34` `!=` → `==`). Parçada `Found 25 mutants to test`, `25 mutants tested in 3h: 25 caught`, `cargo mutants finished, exit code 0` ve `Tool ledger [mutants-0]: 173 test(s) asked for a tool; 175 lookup(s) found the tool (175 required), SKIP: 0`. Yüklenen `mutants-out-0` yapıtında `caught.txt` 25 satır; `missed.txt`, `timeout.txt`, `unviable.txt` boş. "Mutation test summary" işi `shard 0: NOT COMPLETED: no artifact, 25 not tested` ile düştü: tek parçalı koşuda `download-artifact` tek yapıtı alt dizin açmadan `shards/`'a çıkarır, betik `shards/mutants-out-0/` arar (#179). Aynı yapıt `shards/mutants-out-0/` altına konunca `mutants_summary.py` `caught=25 missed=0 timeout=0 unviable=0 untested=0 mutants=25` verir, çıkış 0.

## Kapsam genişlemesi ve hedefli koşular (#206, 2026-11-11)

PR #243'ün (uyarı kaskad bastırma penceresini açmaz, desugar geçitleri pencereyi kullanmaz) dosyaları; önceki karar metni değişmedi.

**Kapsam.** `examine_globs`'a `crates/volt-syntax/src/parser/mod.rs` girdi: ayrıştırıcının tanı giriş noktaları. `push_error` kaskad bastırma penceresini (error-recovery.md §5) yalnız hata için kurar; `push_diag` uyarıları ve desugar geçitlerinin tanılarını pencerenin dışında ekler. Pencereyi bir uyarının açması ardından gelen hatayı yutuyor, kaynak çıkış 0 ile kabul ediliyordu. Dosya önce kapsam dışıydı, çünkü 1. aşama (§2) süreyle sınırlıydı ve sonraki ekler yalnız değiştirdikleri ayrıştırıcı dosyalarını aldı. `examine_globs` dosya düzeyindedir, fonksiyon seçmez; dosyanın 102 mutantı da girdi. `cargo mutants --list` 2845 → 2947; aşağıdaki sınıflandırmanın 12 `exclude_re` girdisinden sonra 2935. Tam koşu §3'e zaten sığmıyordu (#156); bu ek o sınırı değiştirmez, hedefli koşular etkilenmez.

**Koşular.**

| Koşu | Commit | `re` | Liste | Parça | Sonuç |
|---|---|---|---|---|---|
| 43021294446 | 7b01f3f7 | `parser/(mmio\|desugar)\.rs` | 431 (`431 of 436 listed mutants match the regex`; düşen beşi alan silme mutantı) | 7 | 431 test edildi: 269 caught, 51 missed, 0 timeout, 111 unviable, 0 untested (özet işi `caught=269 missed=51 timeout=0 unviable=111 untested=0 mutants=431`). Yedi parçada `Tool ledger [mutants-N]: 173 test(s) asked for a tool; 175 lookup(s) found the tool (175 required), SKIP: 0`. Parçalar 3–5 saat sürdü. |
| 43072419048 | 302f9600 | `parser/mod\.rs:[0-9]+:[0-9]+: .*::push_(error\|diag)( \|$)` | 6 (yerel `--list`) | 2 | (sürüyor) |

İlk koşunun `re`'si iki dosyanın bütün mutantlarıdır, yalnız PR'ın değiştirdiği fonksiyonlarınki değil. PR'ın çağrı adlarını değiştirdiği fonksiyonların çoğunda (`push_error` → `push_diag`) cargo-mutants'ın değiştirebileceği bir ifade yok; PR'ın davranışını değiştirdiği tek yer `parse_generated`. Onun `severity == Error` karşılaştırmasının mutantı (`908:33`, `==` → `!=`) caught.

**Sınıflandırma (§4).** 51 missed: 33 B (hepsine test yazıldı), 1 A1, 11 A2, 6 C. Her B mutantı atılabilir bir worktree'de elle uygulandı, test düştü, mutant geri alındı (`git diff` boş). A2'lerin her biri, varsayımını sınayan kalıcı bir testle `exclude_re`'ye girdi. Mutantla test geçer (eşdeğerlik). Varsayım elle bozulunca (aşağıda) test düşer. Araçlı testler yerelde Verilator 5.052 imajında, `VOLT_REQUIRE_TOOLS=verilator` ile koştu. Testler `crates/volt-driver/tests/desugar_blind_spot_tests.rs`'tedir (aşağıda adlarıyla).

| Mutant | Sınıf | Test ya da issue |
|---|---|---|
| `mmio.rs:518:34` `path_ident` koruması → `true` | B | `an_access_value_with_a_path_prefix_is_e0009` (mutantta `ReadWrite::X` kabul) |
| `mmio.rs:611:25` `volatile` koruması → `true` | B | `a_positional_reg_argument_other_than_volatile_is_e0009` |
| `mmio.rs:648:45` `>` → `>=` | B | `a_register_at_the_last_32_bit_address_is_not_out_of_range` (aşağıda not) |
| `mmio.rs:648:45` `>` → `==` | B | `an_offset_past_the_32_bit_address_space_is_e0015` (mutantta E2010, üretilen metinde) |
| `mmio.rs:467:32` taban koruması → `true` | B | `a_base_wider_than_32_bits_is_e0009` |
| `mmio.rs:846:55` `==` → `!=` (`unit_has_item`) | B | `a_type_alias_in_the_unit_does_not_hide_the_axi_bundles` (mutantta 42 E1001) |
| `mmio.rs:190:9` `has_w1c` → `true` | B | `a_map_without_bus_writes_or_w1c_has_no_write_enable` (mutantta "unused binding: 'mmio_we'") |
| `mmio.rs:190:9` `has_w1c` → `false` | B | `a_map_with_only_a_w1c_flag_has_a_write_enable` (mutantta E1001 `mmio_we`) |
| `mmio.rs:1343:9` `block_assigns` → `true`, `1344:68` `==` → `!=`, `1350:13` `for` kolu silme, `1356:9` `if_assigns` → `true` | B | `a_w1c_clear_goes_into_the_on_block_that_sets_the_field` (mutantlarda E4001) |
| `mmio.rs:1346:13` `match` kolu silme (`block_assigns`) | B | `a_w1c_clear_goes_into_an_on_block_that_sets_the_field_in_a_match` |
| `mmio.rs:532:37` `uN` koruması → `true` (`field_ty`) | B (tanı metni) | `a_field_wider_than_32_bits_is_an_unsupported_type` |
| `mmio.rs:535:13` `UIntN` kolu silme (`field_ty`) | B | `a_uint_n_register_field_is_accepted` |
| `mmio.rs:745:31` ve `745:58` `\|\|` → `&&` (`collect_fields`) | B | `w1c_on_a_non_bool_field_is_e0009` |
| `mmio.rs:128:35` `>` → `<`, `128:33` koruma → `true` (`FieldTy::zero`) | B | `hardware_bits_fields_reset_to_zero_at_every_width` (mutantta `bits<4>` sıfırı `u8`'den daraltılır, `volt check` tanısız kalmaz) |
| `mmio.rs:338:9` `check_driver_names` → `()`, `250:5` `origin_of` → `None`, `381:9` `check_driver_stem` → `()` | B | `ui_fail_138_to_141_mmio_driver_name_collisions_are_e1014` (mutantlarda E1014 kaybolur; 141'de yerine #220'nin 42 E1001'i) |
| `mmio.rs:439:33` `+=` → `*=` (sentetik dosya sayacı) | B | `each_mmio_module_of_a_unit_has_its_own_generated_source` (mutantta iki üretilen kaynak aynı `FileId`'yi paylaşır, GpioRegs'in uyarısı `<mmio:TimerRegs>:7:3`'te `out prot : u3`'ü gösterir) |
| `desugar.rs:592:32` `-` → `/`, `+` | B | `a_relative_stage_reference_past_the_last_stage_is_e5012` (mutantta `stage(+1)` son aşamada kabul) |
| `desugar.rs:889:30` `!` silme (`order_lets`) | B (tanı metni) | `e5015_names_the_values_on_the_cycle` (mutantta ileti boş) |
| `desugar.rs:731:50` `&&` → `\|\|` (`resolve_prefix`) | B | `a_stall_list_with_a_gap_is_e5013` |
| `desugar.rs:462:9` `rw_block` → `()`, `516:9` `rw_if` → `()` | B | `a_stage_reference_inside_an_if_of_an_on_block_is_rewritten` (mutantlarda referans yazılmaz, yalnız W1001) |
| `desugar.rs:1314:5` `collect_arm_idxs` → `()` | B | `a_stage_reference_inside_a_match_arm_of_an_on_block_is_rewritten` |
| `desugar.rs:980:9` `collect_block_names` → `()` | B | `a_stall_guard_read_only_in_an_on_block_is_generated` (mutantta E1001 `stall_f`) |
| `desugar.rs:1276:13` `UIntN`, `1277:13` `SIntN` kolu silme (`copy_ty`) | B | `a_uint_n_value_crosses_a_stage_boundary`, `a_sint_n_value_crosses_a_stage_boundary` (mutantlarda E2005) |
| `mmio.rs:128:35` `>` → `>=` (`FieldTy::zero`) | A1, `exclude_re` | `Bits(8)` kolu öncedir; fark yalnız w = 8'de, oraya ulaşmaz |
| `mmio.rs:128:35` `>` → `==`, `128:33` koruma → `false` | A2, `exclude_re` | `hardware_bits_fields_reset_to_zero_at_every_width` |
| `mmio.rs:1503:8` `!` silme (`render_contracts`) | A2, `exclude_re` | `a_diagnostic_in_the_generated_mmio_source_shows_the_name_it_reports` |
| `mmio.rs:194:39` `\|` → `^` (`read_mask`) | A2, `exclude_re` | `mmio_fields_have_disjoint_bit_ranges` |
| `mmio.rs:945:56` `tail.is_empty()` → `true` (`parse_generated`) | A2, `exclude_re` | `generated_mmio_declarations_come_before_every_other_statement` |
| `desugar.rs:899:20` `!` silme (`order_lets`) | A2, `exclude_re` | `e5015_names_the_values_on_the_cycle` |
| `desugar.rs:1154:23` `==` → `!=` (`bubble_cond`), `1175:18` koruma → `false` (`if_else`) | A2, `exclude_re` | `pipeline_stall_bubble_and_flush_have_their_values` |
| `desugar.rs:1073:39` `<` → `<=`, `1073:35` `+` → `*` (`build_on_block`), `1174:13` `0` kolu silme (`if_else`) | A2 (ulaşılamaz), `exclude_re` | aynı test; aşağıda |
| `mmio.rs:839:9` `unit_has_item` → `false`; `843:17` `Struct`, `844:17` `Module`, `845:17` `Enum`, `846:17` `TypeAlias` kolu silme | C | #244 |
| `desugar.rs:1275:13` `Bits` kolu silme (`copy_ty`) | C | #245 |

A ve C'nin gerekçeleri:

- **A2, `FieldTy::zero` (`==`, koruma → `false`).** Varsayım: üretilen metindeki iki sıfır biçimi de N > 8'de uyarısız genişlemedir ve N bitlik 0'dır. Mutantlarda `bits<16>` alanı `((false as u1) as u16) as bits<16>` biçimini alır. Test iki biçimi `Forms` modülünde elle yazar (16 ve 32 bit) ve `@mmio` modülünde 16, 8 ve 4 bitlik donanım alanları kullanır. `volt check` tanısız, Verilator'da sıfırlama değerleri 0, `Z.sv` ve `Forms.sv` `verilator --lint-only -Wall`'dan bulgusuz geçer. İki mutantla da test geçer (`2 passed`, lint bulgusuz). `N < 8` biçimi `true as u1` yapılınca test `case.volt:64`'te (`q4 == 0`) düşer.
- **A2, `render_contracts` `!`.** Mutant kontrat bölümündeki bir yorum satırını atar, `<mmio:G>`'nin sonraki her satırı bir yukarı kayar (55 → 54). Varsayım: sürücü ve LSP (`analysis.rs`) ayrıştırılan üretilen metni (`ParseResult.generated`) kaydeder; metin diske yazılmaz (ADR-0044 §2) ve `--emit-mmio` gibi bir dökümü yoktur. Bu yüzden tanı, konumu kaymış olsa da aynı metinden doğru adı gösterir. Test üretilen kodda tanılar tetikler: E1001'ler hem yorumdan önce hem sonra (`cover:` ve `let` satırları), W1001'ler port satırında. Her tanının altı çizili parçasının iletideki ad olduğunu (W1001'de satırın o portun bildirimi olduğunu) sınar. Mutantla geçer. Kaydedilen metin bir satır kaydırılınca düşer. Testin E1001 tetikleyicisi bugün #244'tür; #244 düzelince test bilerek düşer ve yeni tetikleyici ister (issue'da not).
- **A2, `read_mask` `|` → `^`.** Varsayım: bir register'ın alanları ayrık bit aralıklarında, 0. bitten sırayla yerleşir (ADR-0044 §2). Ayrık maskelerde OR ile XOR aynıdır. Test depodaki dört `@mmio` tasarımının register haritalarında aralıkların ayrık olduğunu sınar. Yerleşim sayacı `lo += width / 2` yapılınca düşer.
- **A2, `parse_generated` `tail.is_empty()` → `true`.** Varsayım: üretilen metin bütün `reg`/`let` bildirimlerini ilk başka deyimden önce yazar (ADR-0044 §2). Koruma o zaman bir bildirimi hiçbir zaman `on`'dan ya da atamadan sonra görmez. Test dört tasarımın üretilen modül gövdesinde bu sırayı sınar. `render` mantıktan sonra bir `let` yazınca düşer.
- **A2, `order_lets:899:20` `!`.** E5015'ten sonra mutant çevrimdeki `let`'leri değil, yer tutucuları gövdeye koyar; `a` ve `b` düşer. Varsayım: ayrıştırma hatası derlemeyi ad çözümlemesinden önce durdurur. Test aynı dosyadaki tanımsız `z = nope`'un E1001 almadığını, tek tanının E5015 olduğunu sınar. `order_lets`'te aynı değişiklik 883 ve 889'da yakalandığı için girdi `899:20`'yi sabitler.
- **A2, `bubble_cond` `!=` ve `if_else` koruma → `false`.** İkisi de SV'yi değiştirir. `bubble_cond`'da eşit kümede `stall_<b>` yerine aynı koşulların OR'u yazılır. Eşit olmayan kümede `stall_<b>` yazılır; bu bir üst kümedir ve fark yalnız sonraki aşama tutarken çıkar, o zaman da tutma dalı kazanır. `if_else`'te `else if` yerine `else begin if … end end` yazılır. Varsayım: `build_on_block` bubble'ı ancak bir sonraki aşamanın tutmasından sonra sınar. Test iki pipeline'ı simüle eder. P'de iki farklı önekli stall, bubble ve flush var; Q'da tek stall. Değerler ADR-0038'in önceliğinden (flush > hold > bubble > normal) elle hesaplandı. Test ayrıca `P.sv` ve `Q.sv`'yi `-Wall` ile lint eder. Dört mutantla (bu ikisi, `1073:39` ve `1174`) `2 passed`, lint bulgusuz. Bubble 0 yerine 1 yazınca (`zero_expr`) test `case.volt:40` ve `79`'da düşer.
- **A2 (ulaşılamaz), `build_on_block` `1073:39` ve `1073:35`.** `stall_name` yalnız 0..N-1 aşama indislerini tutar (`desugar_pipeline` onu `0..nstages`'ten kurar). `b + 1 == N`'de mutantın aradığı N. aşama yoktur. `1073:35` sabitlendi, çünkü bir satır aşağıdaki aynı değişiklik yakalanıyor.
- **A2 (ulaşılamaz), `if_else` `0` kolu.** Her çağıran boş olmayan bir `else` verir: register'sız sınır `build_on_block`'ta atlanır. Boş `else begin end` zaten etkisizdir. Desugar külliyatında (`tests/`, `examples/`; 31 `@mmio`/pipeline dosyası) üç ulaşılamaz mutantın tanıları ve SV'si mutantsız derleyicininkiyle bayt bayt aynı.
- **C, #244.** Birimde bir AXI bundle adı (`struct port`, takma ad, enum ya da modül olarak) tanımlıysa üretici o bundle'ı atlar. Ama üretilen modül birimdeki tanımı görmez ve üretilen kodda 8 E1001 alır. ADR-0044 §2 "birimde tanımlı değillerse üretilir" diyor. Mutantlarda tek bir E1003 çıkar. Beş mutant karar verilene kadar missed kalır.
- **C, #245.** `bits<K>` değeri bir aşama sınırını geçince E2003 alır ("expected 'bits<8>', found 'integer literal'"). ADR-0038 `bits<K>`'yi sınırı geçebilen skaler tipler arasında sayar. Neden: `zero_expr` bubble değeri olarak `0` literali üretir. Mutantta tanı E2005'e döner.

**Not, `648:45` `>=`.** ADR-0044 bugün `base`'in hizalanmasını istemiyor. `base = 0xFFFF_FFFF`, `offset = 0` son 32 bitlik adrestir. #220 hizasız tabanı reddetmeyi öneriyor. Bu yüzden test yalnız E0015'in olmadığını sınar. #220 hizasız tabanı reddedince bu mutant ulaşılamaz olur ve A2'ye geçer (#220'de not).

**Koşunun dışında bulunanlar.** `tests/ui/fail/138`–`141`'i (E1014) hiçbir test koşmuyordu; şimdi `ui_fail_138_to_141_mmio_driver_name_collisions_are_e1014` koşuyor. Düzleştirilmiş bir bundle alanının W1001'i doğru satırı ama keyfi bir sütunu gösteriyor (`fresh_name_span`): #246 (iyileştirme).

