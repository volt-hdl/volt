# CI iş akışları

| Dosya | Tetik | İçerik |
|---|---|---|
| `ci.yml` | push (main), PR, elle (`workflow_dispatch`; tek girdisi `ccache`, aşağıda "ccache") | biçim + clippy + test, Verilator, tutarlılık, formal, OpenSTA, coverage, **60 sn fuzz** |
| `fuzz-nightly.yml` | her gün 18:40 UTC (ADR-0123), elle (`workflow_dispatch`) | **30 dk fuzz**, corpus geceden geceye taşınır |
| `release.yml` | yalnız tam `vX.Y.Z` tag'i (`-rc`, aşama ve önekli tag'ler tetiklemez; tutarlılık kontrolü 18), elle (`workflow_dispatch` = kuru koşu, herhangi bir daldan) | 4 platform arşivi + her platformda duman testi + kurulum betikleri bu arşivlerle + `.vsix` + `SHA256SUMS` + sürüm notu (`docs/release-notes/vX.Y.Z.md`); yalnız tag'de `publish` işi: derleme kaynağı kaydı ve **taslak** Release (ADR-0093, ADR-0105). Kuru koşu yalnız artifact yükler. PR'da koşmaz |
| `book.yml` | push (main), `vX.Y.Z` tag'i, PR, elle | kitap (`book/`, mdBook): denetleyicinin öz-testi, her ```` ```volt ```` bloğu `volt check`'ten, her test bloğu Verilator'dan geçer; mdbook uyarısız derlenir; site kökü ve `/dev/` birleştirilir (aşağıda "Kitap ve Pages düzeni"). Yalnız main'de GitHub Pages'e yayımlar (https://volt-hdl.github.io/volt/; depo ayarı Settings → Pages → Source: GitHub Actions gerekir). Tag'de yayımlamaz, main'de yeni bir koşu başlatır |
| `mutants.yml` | PR (kapı; `opened`, `synchronize`, `reopened`, `edited`'da değil), elle (`workflow_dispatch`), push (main; yalnız `cache-warm` işi, aşağıda "Rust önbelleği"); zorunlu kontrol değildir | **mutasyon testi** (ADR-0109): cargo-mutants, kapsam `.cargo/mutants.toml` (her crate'in `src/`'i; dışarıda kalan dosya `exclude_globs`'ta adıyla ve gerekçesiyle, tutarlılık kontrolü 19). **PR kapısı** (ADR-0109 eki "PR kapısı", #253): her PR'da PR'ın değiştirdiği Rust satırlarındaki mutantlar (`cargo mutants --in-diff`, birleştirme commit'inin ilk ebeveynine göre fark), parça sayısı ölçülen t = 600 sn ile §3'ün hesabından; `exclude_re`'nin kapsamadığı her missed mutant özet işini düşürür, özet onları ve sınıflandırma kurallarını listeler; kapsamdaki Rust'a dokunmayan PR'da iş bir şey denemeden yeşil biter ve nedenini yazar; aynı PR'a yeni push eski koşuyu iptal eder (`concurrency`). Aynı PR'ın daha önce temiz geçmiş bir kapı koşusunun anahtarı aynıysa parça koşmaz, karar o koşunun sonuçlarından verilir ve özet "reused: run …" der (aşağıda "Mutasyon kapısının önceki sonucu", #318). Elle kapı: `-f gate_base=<revizyon>`. Hariç tutma denetimi `mutants-exclusions.yml`'dedir (aşağıda); kapı özeti C kayıtlarının dışarıda tuttuğu mutantları issue numarasıyla listeler. Elle koşu (ölçüm): `MUTANTS_SHARDS` parça (`--shard k/N`; N ilk işte ADR-0109 §3'ün hesabıyla koşunun mutant sayısından bulunur, `shards` girdisi onu ezer), temel koşu koşu başına tek bir iştir ve parçalarla paralel koşar (ADR-0128 §2: "Mutation baseline" işi; geçmezse özet mutant kararı vermeden düşer; aşağıda "Mutasyon kapısının temel koşusu ve zaman aşımı"), temel koşu işinde ve her parçada araçlı işin OSS CAD Suite'i ve `VOLT_REQUIRE_TOOLS=all`, testler nextest ile, `mutants` profilinin sırasıyla (`.config/nextest.toml`, ADR-0128: PR'ın dokunduğu test ikilileri önce, yavaş Verilator ikilileri en sonda; aşağıda "Mutasyon kapısının test sırası"); temel koşunun ve her parçanın derleme ve testleri `scripts/ci/mutants_watchdog.py` altında koşar: süreç ağacı `MUTANTS_MEMORY_LIMIT_MIB`'i aşınca en büyük süreç öldürülür, mutant "memory limit" (yakalandı) diye raporlanır, parça sürer (#160); temel koşunun derleme ve testlerinin en yüksek belleği (`--peak`) özetin temel koşu tablosundadır, sınırın yarısını aşarsa uyarı (#253); mutant başına test zaman aşımı max(2 × temel koşu, `MUTANTS_TIMEOUT_FLOOR_SECONDS` = 3610 sn) (ADR-0128 §3); ilk iş her parçanın mutant listesini `mutants-list` artifact'ı olarak yükler, her parça `mutants-out-<k>` artifact'ını kesilse de yükler (`if: always()`), son iş toplam/caught/missed/timeout/memory limit/unviable ve missed listesini iş özetine yazar (`mutants-summary` artifact'ı; tek parçalı koşunun düz yapıtını da okur, #179). Elle koşuda missed mutant işi düşürmez; tamamlanmayan parça (artifact'ı yok ya da 300 dakikada bitmedi; iş sınırı 330 dakika) özetin başında test edilmeyen mutantlarıyla "NOT COMPLETED" diye adlandırılır ve özet işini düşürür. Tetik: `gh workflow run mutants.yml --ref <dal>`. Hedefli koşu (ADR-0109, "Hedefli koşu"): `-f re=<regex>` yalnız adı eşleşen mutantları dener; cargo-mutants `--re` ile alan silme mutantlarını süzmediğinden liste `scripts/ci/mutants_target.py`'nin yazdığı satır diff'iyle (`--in-diff`) ikinci kez daraltılır ve son listede eşleşmeyen bir mutant varsa ya da liste `-f expect=<n>` (regex'in yazıldığı mutant sayısı; `re` ile zorunlu, #253) kadar mutant tutmuyorsa ilk iş düşer; `-f shards=<1-20>` parça sayısı |
| `known-failures.yml` | PR (`opened`, `synchronize`, `reopened`, `edited`), push (main), her gün 03:17 UTC, elle | **bilinen başarısızlıklar** (ADR-0117 §4, #254): `crates/volt-driver/tests/metamorphic/known_failures.json`'daki her kaydın issue'su açık olmalı; issue kapalıysa, yoksa ya da PR'sa, PR'da da PR o issue'yu kapatıyor (`Closes #N`) ve kayıt duruyorsa düşer (`scripts/ci/known_failures.py`; GitHub istemcisi ve kurallar `mutants-exclusions.yml`'in C kayıtlarınınkidir, #283). `github.token` ile okur (`issues: read`, `pull-requests: read`); token ya da ağ yoksa düşer. Gece de koşar: issue elle kapatılabilir. Gecelik metamorfik testin issue adımının self-test'i de burada koşar (`scripts/ci/nightly_issue.py --self-test`) |
| `metamorphic-nightly.yml` | her gün 18:21 UTC (ADR-0123), elle (`ccache` girdisi) | **metamorfik test, gecelik katman** (ADR-0117 §5, #254): bütün yeniden yazımlar bütün derlemde (`tests/ui`, `examples/`, kitabın ```` ```volt ```` blokları, `tests/real_bugs/`; `VOLT_METAMORPHIC_TIER=nightly`), Verilator zorunlu, defterde sıfır SKIP. Uyaranın tohumu gecenin UTC tarihidir: koşunun başladığı andan 12 saat önceki UTC tarihi (`scripts/ci/nightly_seed.py`, ADR-0123) ("Seed of this night" adımı, rapor ve issue onu yazar; `VOLT_METAMORPHIC_SEED=<tohum>` koşuyu yeniden üretir). Kırmızı gece sabit başlıklı issue'yu açar ya da açık olanına raporlarla ve tohumla yorum ekler (`scripts/ci/nightly_issue.py`; sahte bir `gh` ile self-test'i her PR'da `known-failures.yml`'de koşar). PR katmanı (`tests/real_bugs/`, `tests/ui/`) `ci.yml`'in araçlı işinde koşar |
| `difftest.yml` | her gün 17:03 UTC (#255, ADR-0123), elle (`seed`, `count`, `div_operands`, `issue` girdileri) | **fark testinin büyük koşusu** (ADR-0108): dört parça, ardışık tohumlar (parça k: tohum + k), parça başına 12.500 kabul edilmiş ifade, Verilator zorunlu (sabit hakem), her parçada defterde sıfır SKIP ve `diffs=0` ("Differential test summary"). Gecenin ilk tohumu gecenin UTC tarihidir, `YYYYMMDD00`: koşunun başladığı andan 12 saat önceki UTC tarihi (`scripts/ci/nightly_seed.py`, ADR-0123; gecikip gece yarısını geçen gece tarihini korur, iki gece hiçbir parçanın tohumunu paylaşmaz). Tohumu ayrı bir "Seed of this run" işi bir kez alır (bütün parçalar aynı tohumu koşar); günlüğe bir `::notice`, iş özetine her parçanın komutu yazılır: `VOLT_DIFFTEST_SEED=<tohum + k> VOLT_DIFFTEST_COUNT=12500 VOLT_REQUIRE_TOOLS=verilator cargo test --release -p volt-driver --test difftest_tests` k. parçayı sabit Verilator'la yeniden üretir. Elle koşuda `seed` boşsa aynı formül (son başlayan gecenin tohumu), doluysa o. Kırmızı gece sabit başlıklı issue'yu ("Nightly differential test is red (ADR-0108)") açar ya da açık olanına tohumla, komutlarla ve parçaların raporlarıyla yorum ekler (`scripts/ci/nightly_issue.py --kind difftest`; self-test'i her PR'da `known-failures.yml`'de koşar); yeşil gece bir şey yazmaz. Kırmızı elle koşu issue'ya yalnız `-f issue=on` ile yazar. ccache yalnız iş içindedir (aşağıda "ccache"). Saat ve süre: aşağıda "Gecelik saatler" |
| `mutants-exclusions.yml` | PR (`opened`, `synchronize`, `reopened`, `edited`), push (main), elle | **hariç tutma denetimi** (ADR-0109 eki "PR kapısı" §3, #239; eki "Kapı zorunlu yapılmadan önce", #283): "Mutation exclusions match their mutants" işi her `exclude_re` kaydının `# expect:` satırlarındaki mutantlarla tam eşleştiğini `cargo mutants --list` ile denetler; C kaydı (`# C.` etiketi, tek `# issue: #N` satırı) issue'su kapalıysa ya da yoksa, PR'da da PR o issue'yu kapatıyor (`Closes #N`) ve kayıt duruyorsa iş düşer; issue'ları `github.token` ile okur (`issues: read`, `pull-requests: read`), token ya da ağ yoksa düşer. `mutants.yml`'den ayrıdır: PR açıklaması düzenlenince (`edited`) yalnız bu iş akışı koşar, kendi `concurrency` grubu vardır; düzenleme mutant kapısını yeniden başlatmaz, koşusunu iptal etmez. main'de her push'ta koşar: birleşen bir PR'ın kapattığı issue'nun kaydı kalmışsa main'de düşer |
| `install.yml` | PR/push (yalnız `scripts/install/`, `scripts/release/`, `install.yml`, `release.yml` değişince), pazartesi 05:17 UTC, elle | kurulum betikleri (ADR-0096): shellcheck + PSScriptAnalyzer; üç platformda derle → `scripts/release/package.sh` → `VOLT_ARCHIVE` ile kur, yeni kabukta `volt --version`, yeniden kur, kaldır (dosya ve PATH izi kalmaz), bozuk `SHA256SUMS`; sahte GitHub'a (`test/fake-github.py`) karşı ağ senaryoları (sürüm yok, varlık 404, kopan indirme, API hız sınırı → yönlendirme, geçici hata → yeniden deneme, yanıt yok); Windows'ta PowerShell 7 ve 5.1 ayrı adım. Haftalık/elle: gerçek tek satırlık komutlar; sürüm yokken "henüz sürüm yok" iletisini, sürüm varsa kurulumu sınar |

## Gecelik saatler (#255, ADR-0123)

Zamanlanmış iş akışları aynı koşucu havuzunu (eşzamanlı 20 iş) PR'ların
CI'ı ve mutasyon kapısıyla paylaşır. GitHub zamanlanmış koşuları geç
başlatır: Kasım 2026'da 6 sa 35 dk ile 8 sa 38 dk arası (fuzz 03:00 →
09:35-11:17; install 05:17 → 13:30-13:55). Eski saatlerle (01:23, 02:41,
03:00) gece işleri Türkiye saatiyle öğlen, PR'ların CI'ıyla aynı
saatlerde koşuyordu. Üç gece işi bu yüzden UTC 17:00 civarındadır;
aralarındaki sıra ve boşluklar korunur, :00 ve :30 kullanılmaz:

| İş akışı | cron (UTC) | Gecikmesiz, TR | 8 saat gecikmeyle, TR | Eşzamanlı iş | Ölçülen süre |
|---|---|---|---|---|---|
| `difftest.yml` | her gün 17:03 | 20:03 | 04:03 | 1 (tohum, 2 s), sonra 4 parça; kırmızı gecede +1 | duvar saati 7 dk 10 s; parçalar 325-423 s, toplam ≈ 24 iş-dakikası (koşu 44237991576) |
| `metamorphic-nightly.yml` | her gün 18:21 | 21:21 | 05:21 | 1; kırmızı gecede +1 | ~2 dk (koşu 44070588184) |
| `fuzz-nightly.yml` | her gün 18:40 | 21:40 | 05:40 | 1 | ~31 dk |
| `known-failures.yml` | her gün 03:17 | | | 1 | ~10 s |
| `install.yml` | pazartesi 05:17 | | | 3 platform | |

- Aynı gün kurulan işler yaklaşık aynı gecikmeyi alır (2026-11-17'de
  02:41, 03:00 ve 03:17 → 11:04, 11:17 ve 11:51); 78 ve 19 dakikalık
  boşluklar bu yüzden korunur.
- **Gecenin tarihi.** Fark testinin ve metamorfik gecenin tohumu
  koşunun başladığı andan 12 saat önceki UTC tarihidir
  (`scripts/ci/nightly_seed.py`). 17:03'e kurulan gece ertesi gün 12:00
  UTC'ye kadar (18 sa 57 dk), 18:21'e kurulan 17 sa 39 dk gecikse de
  kurulduğu günün tarihini alır; bir gece gecikip ertesi gece gecikmese de
  iki gece aynı tohumu koşmaz. Self-test (`--self-test`, her PR'da
  `known-failures.yml`'de ve her gece koşunun başında) gecikmesiz, 8 saat
  gecikmeli ve gece yarısını geçen durumları, iş akışlarının cron
  satırlarına karşı 0-16 saatlik bütün gecikmeleri sınar. Bir cron'u
  değiştiren PR bu pencereyi bozarsa self-test düşer.
- Gecikmeler yeni saatlerle 2026-11-25'te ölçülüp saat yeniden
  değerlendirilecek (ADR-0123 §8).

## Koşucu imajı

- Her Linux işi sabit bir Ubuntu sürümünde koşar: `runs-on: ubuntu-24.04`
  (matrislerde `os: ubuntu-24.04`). Hareketli etiket `ubuntu-latest`
  yazılmaz: GitHub o etiketi kendi takvimiyle yeni sürüme geçirir
  (2026-11-13 ile 2026-11-15 arasında 24.04'ten 26.04'e) ve aynı commit
  başka derleyici ve araç sürümleriyle sınanır; ccache anahtarı da imajı
  içerdiğinden önbellek boş başlar.
- **Neden 24.04** (PR #301, iki imajda CI karşılaştırması):
  - Sabit hakemin imajı (`verilator/verilator:v5.054`, Docker arka ucu)
    Ubuntu 24.04 ve g++ 13.3'tür. CI Verilator'ı o imajdan kopyalar,
    ürettiği C++'ı koşucunun g++'ı derler (ADR-0107, "Sınırlar": "ikisi de
    Ubuntu 24.04"). 24.04 koşucusunda CI, Docker arka ucunun kullanıcıya
    verdiği derleyiciyle (g++ 13.3, glibc 2.39) simüle eder; 26.04'te g++
    15.2 ve glibc 2.43 ile, yani hakemden farklı bir araç zinciriyle.
  - 26.04'te `install.yml`'in "Install test (Linux)" işi düşer: `/usr/bin`
    ve `/bin` taranırken sarkan bir sembolik bağ (`grub-ntldr-img`)
    `[ -e ]` testinden geçmez, ikinci `ln -s` "Already exists" verir
    (koşu 43994126886). Bu iş 26.04'te ilk kez o PR'da koşmuştu.
  - Öbür sonuçlar (testler, fark testi, metamorfik test, kitap) iki
    imajda aynıydı; ayrıntı ve koşu numaraları PR #301'de.
- 26.04'e geçiş, hakem imajı da 26.04 tabanına geçtiğinde (Verilator
  güncellemesi, ADR-0107 kural 4) ya da 24.04 GitHub'da kullanımdan
  kalkmadan önce, ayrı bir PR'da yapılır; o PR `install.yml`'deki
  sembolik bağ adımını düzeltir.
- Tutarlılık denetimi (kontrol 20) `.github` altındaki YAML'larda hareketli
  etiketi (yorumda da) ve `runs-on` / matris `os` satırlarında birden çok
  Ubuntu sürümünü reddeder.
- **Başka bir imaj sürümüne geçiş ayrı bir PR'dır ve bilinçli yapılır:**
  bütün etiketler birlikte değişir; PR'da CI'ın tamamı (PR iş akışları ve
  elle koşanlar: `difftest.yml`, `metamorphic-nightly.yml`,
  `fuzz-nightly.yml`, `release.yml` kuru koşusu) yeni imajda koşar ve
  sonuçlar eski imajınkilerle karşılaştırılır. Böyle bir PR'da Rust ve
  ccache önbellekleri boş başlar (anahtarları `ImageOS`'u içerir, PR
  kaydetmez), CI süresi o PR'da uzar; birleştikten sonra main yeni
  önbellekleri kaydeder.
- Sabitlenen etiket imajın haftalık sürümünü sabitlemez: aynı koşunun
  işleri arasında bile farklı sürüm gelebilir (`ImageVersion`, ör.
  20261111.82.6 ve 20261111.196.1). ccache anahtarı `ImageVersion`'ı
  içerdiğinden böyle bir işte ccache önbelleği boş başlar.
- `windows-latest` ve `macos-latest` (`install.yml`, `release.yml`) bu
  kuralın dışındadır.

## Rust önbelleği

- Her `Swatinem/rust-cache` adımı `save-if: ${{ github.ref ==
  'refs/heads/main' }}` ve `env-vars: ImageOS` taşır (tutarlılık denetimi,
  kontrol 21). Rust önbelleklerini yalnız main kaydeder; PR'lar main'in
  önbelleğini okur. GitHub'da bir PR'ın kaydettiği önbelleği başka bir PR
  okuyamaz: PR kopyaları yalnız aynı PR'ın sonraki push'larına yarar,
  10 GB'lık önbellek alanını doldurur ve main'in girdilerini çıkarır.
- `mutants.yml` dışındaki her adım `key: ${{ hashFiles('Cargo.toml') }}`
  taşır. rust-cache'in kendi anahtarı `Cargo.toml`'dan yalnız
  bağımlılıkları özetler, profilleri değil: profil değişince (PR #395,
  hata ayıklama bilgisi) anahtar aynı kalır, tam eşleşen önbellek yeniden
  kaydedilmez ve her iş bağımlılıkları eski profilin önbelleğinin üstüne
  yeniden derler. Kanıt: profil değişikliğinden sonra test işi
  `Cache hit for: v0-rust-test-Linux-x64-75234bab-e566b306` (main'in
  anahtarı) aldı. `mutants.yml` bu satırı taşımaz: iş akışı ortamı iki
  hata ayıklama düzeyini de 0 yapar, parçaların derlediği birimler profil
  değişikliğinden etkilenmez.
- `ImageOS` (ör. `ubuntu24`) anahtarın ortam özetine girer: imaj değişince
  başka bir imajda derlenmiş build script ve proc-macro ikilileri (daha
  yeni glibc'ye bağlı olabilir) geri yüklenmez, önbellek boş başlar.
- **Mutasyon kapısı** main'de koşmaz. Bu yüzden `mutants.yml` main'e her
  push'ta yalnız `cache-warm` işini koşar: parçaların önbelleğini
  (`shared-key: mutants`, aynı iş akışı ortamı, yani aynı anahtar) geri
  yükler; anahtar değiştiyse (Cargo.lock, araç zinciri, imaj) parçanın
  temel koşusunun derlediğini (`cargo test --workspace --no-run`) derler
  ve kaydeder. Anahtar tam eşleşirse derlemez ve bunu `::notice` ile yazar.
  Parçalar bu önbelleği okur, kaydetmez. Ölçülen kazanç küçüktür: önbelleksiz
  bir parça 150 crate derler (47,0 s, koşu 43803864010), önbellekli parça
  yalnız 16 çalışma alanı crate'ini (29,5-44,2 s, koşu 43734127184,
  43754250315).
- Önbellek mutant başına süre sınırını değiştirmez: sınır, temel koşunun
  derlemeden (`cargo nextest run --no-run`) sonra başlayan test süresinden
  ölçülür (#300'deki tuzak ccache içindir: Verilator derlemeleri testin
  içindedir).
- Araç önbellekleri (`oss-cad-suite-*`, `opensta-*`, `verilator-*`,
  `mutants-tools-*`), fuzz corpus'u ve ccache bu kuralın dışındadır;
  ccache'in kendi kuralı aşağıda.
- `difftest.yml` her gece main'de koşar ve `difftest` önbelleğini
  (anahtar değiştiyse) kaydeder; bir dalda elle koştuğunda main'inkini
  okur.

## Mutasyon kapısının test sırası (ADR-0128, #366)

Yakalanan bir mutant ilk düşen testte durur (`--max-fail=1:immediate`);
ne kadar sürdüğünü yakalayan testin ne zaman başladığı belirler. #366'nın
ölçümünde nextest'in varsayılan sırası `volt-driver`'ın Verilator'lı
ikililerini öne koyuyordu: yakalayan test (çoğu PR'ın eklediği 0,01
saniyelik bir test) 4222 testin 800-2400. sırasındaydı, yakalanan mutant
medyanda 960 sn test koşuyordu.

- **Profil `mutants`** (`.config/nextest.toml`): her test için ilk eşleşen
  kural; öncelik 100 PR'ın dokunduğu test ikilileri, −100 yavaş 22
  `volt-driver` ikilisi (temel koşuda testlerinin toplamı ≥ 40 sn), 50
  `volt-driver` dışındaki her crate. Yerel `cargo nextest run` ve CI'ın
  öteki işleri (`cargo test`) varsayılan profili kullanır.
- **Parçanın "Test order" adımı** temel koşudan önce
  `scripts/ci/mutants_nextest.py pr --changed mutants-list/changed.txt`
  ile PR'ın ikililerini profile yazar (yalnız parçanın çalışma kopyasına),
  `check` ile profilin adlandırdığı her ikilinin var olduğunu denetler ve
  `NEXTEST_PROFILE=mutants`'ı iş ortamına koyar. Temel koşu ve her mutant
  aynı sırayla koşar.
- **Geçici girdi** `nextest_order` (`priority` | `default`): aynı commit'i
  iki sırayla koşturup karşılaştırmak için (ADR-0128 "Doğrulama
  yöntemi"); PR'da her zaman `priority`.
- **Karşılaştırma:** `gh run download <id> --repo volt-hdl/volt --dir A`
  ve `B`, sonra `python3 scripts/ci/mutants_compare.py A B`: mutant
  listeleri ve her mutantın sonucu aynı değilse 1 ile çıkar, farkları
  adlarıyla ("missed -> timeout: …"), iki koşunun parça ve mutant başına
  derleme/test sürelerini yazar.
- `volt-sv-emit/build.rs` yalnız var olan git dosyalarını izler: sığ
  checkout'ta `.git/packed-refs` yoktur ve cargo var olmayan yolu her
  çağrıda değişmiş sayıyordu; her mutantın test aşaması `volt-sv-emit`,
  `volt-lsp` ve `volt-driver`'ı yeniden derliyordu (medyan 13,7 sn).

## Mutasyon kapısının temel koşusu ve zaman aşımı (ADR-0128 §2–§3, #366)

Her parça 18-31 dakikalık kendi temel koşusunu koşuyordu; yeni sırayla
(§1) bir parçanın süresini artık o belirliyordu.

- **Tek temel koşu.** "Mutation baseline" işi ilk işten sonra, parçalarla
  aynı anda başlar. Parçanın eski adımının aynısını yapar: etkisiz
  yolları siler, `mutants` profiliyle, bellek bekçisi altında, araçlar
  zorunlu (`VOLT_REQUIRE_TOOLS=all`) bütün çalışma alanı testlerini koşar.
  `mutants-baseline` artifact'ını yükler: derleme ve testlerin en yüksek
  belleği (#253), damga (`stamp.json`, #318: araç sürümleri, koşucu imajı,
  etkisiz yollar, temel koşunun saniyesi ve kuralın zaman aşımı), araç
  defteri. Defter denetimi burada `--require-zero-skips --min-ran 60`'tır.
- **Parçada temel koşu yok.** Parça testleri bir kez derler
  (`cargo nextest run --workspace --no-run`; ilk mutant da yalnız
  değiştirdiğini derlesin diye), kendi damgasını yazar (`--role shard`,
  kullandığı zaman aşımıyla) ve mutantları koşar. Defter denetimi
  `--require-zero-skips`'tir; `--min-ran` yok: bir parçanın mutantlarının
  kaç araçlı teste ulaştığı nerede yakalandıklarına bağlıdır.
- **Özet.** Temel koşu işi geçmediyse (`failure`, `cancelled`, `skipped`)
  ya da artifact'ı yoksa özet "no mutant verdict" der ve düşer; tablolar
  yalnız tanı içindir.
- **Zaman aşımı** (§3): kural max(2 × temel koşu, F), F =
  `MUTANTS_TIMEOUT_FLOOR_SECONDS` = 3610 sn (ADR-0128'den önceki en büyük
  değer; yeni sıradaki en uzun tam takımın iki katından büyük). Parçalar
  temel koşuyu beklemeden F ile başlar. Özet her parçanın kullandığı
  zaman aşımını temel koşunun damgasındaki kural değeriyle karşılaştırır.
  Kuraldan kısaysa o parçanın her timeout mutantı "not decided" olur ve
  özet düşer; parçada timeout mutant yoksa yalnız uyarır. Yakalanan bir
  mutant zaman aşımından önce düşmüş, missed bir mutant onun içinde
  geçmiştir; ikisinin sonucu zaman aşımına bağlı değildir.
- **Yeniden kullanım** (#318): önceki koşunun `mutants-baseline`'ı da
  okunur: defteri `--min-ran 60` ile, damgası anahtara karşı denetlenir.
  Artifact'ı olmayan koşu yeniden kullanılmaz.

## Mutasyon kapısının yeniden denemesi (#364, ADR-0129)

Missed mutantlar için gelen düzeltme çoğu zaman yalnız testtir; sonraki push
bütün listeyi baştan denemez.

- **Taban:** PR'ın son tam kapı koşusu. Tam koşu, bütün listeyi kendisi
  sınamış, tamamlanmış ve iptal edilmemiş koşudur; yeniden kullanım ve
  yeniden deneme koşuları atlanır. Kapıyı geçmiş olması gerekmez. Temel
  koşusu geçmiş, her parçası tamamlanmış, SKIP 0 olmalıdır.
- **Koşul:** tabanla aynı anahtar kısımları (`mutants`, `environment`), ve
  anahtarın ağacında değişen her yol bir paketin `tests/`'inde
  (`crates/<paket>/tests/…`). Kaynak dosya (kendi `#[cfg(test)]`'iyle
  birlikte), kök `tests/`, `Cargo.toml` ya da başka bir yol değiştiyse kapı
  tam koşar; günlük nedenini yazar.
- **Yeniden sınananlar:**
  - tabanın missed, timeout ve bellek sınırı mutantları;
  - tabanın sınamadıkları;
  - yakalayan test ikilisi bu push'ta değişen caught mutantlar.
- **Yakalayan ikili** tabanın parça artifact'ındaki günlükten okunur: ilk
  nextest hata satırı (`FAIL [..] (i/N) <ikili> <test>`), "Cancelling"
  satırından öncesi. Bir hedefin kök dosyası o ikilidir; `tests/` altındaki
  başka her yol paketin bütün ikilileridir. Hata satırı yoksa mutant yeniden
  sınanır.
- **Başka paketten okunan test dosyası.** Bir test dosyası başka bir
  koddan adıyla okunabilir ya da içeri alınabilir: `#[path]`, `include!`,
  `include_str!`, `../` ile yol, `CARGO_MANIFEST_DIR`'den ya da depo
  kökünden yol. Bunu bulmak için `crates/` altındaki her Rust dosyasının
  string literal'leri okunur; yorumlar sayılmaz. Bir literal dosyanın
  dizinine, paketin dizinine ve depo köküne göre çözülür. Literal yalnız
  başka bir paketin dizin adıysa o paketin bütün `tests/`'ini adlandırmış
  sayılır.
  - Okuyan bir test hedefinin kök dosyasıysa onun ikilisi de değişmiş
    sayılır.
  - Okuyan `tests/` altındaki başka bir dosyaysa paketinin bütün
    ikilileri değişmiş sayılır.
  - Okuyan test dışı kodsa (src/, build.rs, benches/, examples/) kapı
    tam koşar.
  - Bugün tek örnek `crates/volt-tools/tests/verilator_flags_tests.rs`:
    volt-driver'ın `tests/verilator_canary_tests.rs` ve
    `tests/metamorphic/sim.rs` dosyalarını okur.
- **Taşınanlar** (yakalayan ikilisi değişmemiş caught, unviable) özette
  "Carried over" olarak sayılır. Özetin başında `retest: run <id> (SHA
  <sha>): k of N retested, N-k carried over` yazar.
- **Liste** hedefli koşu gibi daraltılır (`--re` tam adlarla, `--in-diff`
  satırlarıyla). `mutants_reuse.py retest-check` listenin yeniden
  sınanacak her mutantı tuttuğunu ve kapının listesi dışında hiçbir şey
  tutmadığını kanıtlar; kanıtlanamazsa kapı tam koşar.
- **Elle:** `-f gate_base=<revizyon> -f retest_from=<koşu>`.
  `mutants_compare.py` bir yeniden denemeyi bütün kapı olarak okur
  (`gate-all.txt` ve taşınan sonuçlar). Böylece aynı commit'teki tam
  koşuyla karşılaştırılır.

## Mutasyon kapısının önceki sonucu (#318; ADR-0109 eki "Kapının önceki sonucu")

Kapı her push'ta değişen Rust satırlarının mutantlarını baştan dener. Yalnız
belge değişen bir push aynı mutantları aynı kodla ve aynı testlerle
yeniden dener (#308'in ikinci push'u: 101 dk). Kapı artık parçalardan önce
bir **yeniden kullanım anahtarı** hesaplar (`scripts/ci/mutants_reuse.py
key`, `mutants-list/key.json`) ve aynı PR'ın anahtarı aynı, temiz geçmiş
bir koşusunu arar (`find`).

- **Anahtar, üç parça** (her biri `key.json`'da tam durur; iki anahtar
  özetle değil alan alan karşılaştırılır, günlük neyin farklı olduğunu
  yazar):
  1. **mutantlar:** kapının hariç tutmalardan önceki listesi
     (`unexcluded.txt`) ve mutasyona uğrayan her dosyanın blob'u;
  2. **ağaç:** birleştirme commit'inin ağacı (yol, kip, blob), etkisiz
     yollar (`scripts/ci/mutants_inert.txt`, her biri gerekçesiyle) ve
     `.cargo/mutants.toml` çıkarılmış;
  3. **ortam:** `.cargo/mutants.toml` ayrıştırılmış, `exclude_re`'siz
     (`exclude_re` yalnız listeyi süzer; bugünkü kayıtlar önceki sonuçlara
     yeniden uygulanır); iş akışının blob'u; Verilator sabiti
     (`verilator.env`) ve eylemi; etkisiz yol listesi; iş akışının araç ve
     sınır değişkenleri (cargo-mutants, nextest, OSS CAD Suite tarihi,
     bellek sınırı, t); `rustc -vV`, `cargo mutants --version`, `cargo
     nextest --version`; koşucu imajı (`ImageOS` ve haftalık sürümü, yani
     `ImageVersion`'ın tarih kısmı) ve koşucunun ölçülen araç zinciri (g++,
     glibc, binutils, make, python3, git). `ImageVersion`'ın geri kalanı
     bir derleme numarasıdır ve aynı koşunun işleri arasında değişir
     (koşu 44186837945: kapsam işi 20261111.370.1, parçası 20261111.127.1);
     anahtara girseydi yeniden kullanım hiç tutmazdı (ilk kanıt push'u
     44194628766 bu yüzden baştan koştu).
- **Etkisiz yollar** dar tutulur: `docs/adr/`, `docs/spec/`,
  `docs/design/`, `docs/research/`, `docs/release-notes/`,
  `docs/roadmap.md`, `CHANGELOG.md`. `README.md` etkisiz değildir
  (`verilator_pin_tests` okur); `.test-baseline` ve `.github/badges.json`
  da listede yok (yalnız testlerle değişirler, testler anahtarı zaten
  değiştirir). Her parça temel koşudan önce bu yolları siler (`mask`):
  kaydettiği sonuçlar onlarsız hesaplanır, onları okuyan bir test her
  kapı koşusunun temel koşusunu düşürür. Tutarlılık kontrolü 22 listeyi
  ve `crates/` altında onları okuyan kodu denetler.
- **Kullanılan önceki koşu:** aynı PR numarası, aynı baş dalı ve aynı baş
  deposu (fork değil), olay `pull_request` (elle koşu değil), iş akışı
  `mutants.yml`; tamamlanmış. İptal edilmiş, anahtarsız ya da kendisi
  yeniden kullanmış koşular atlanır; anahtarı aynı en yeni koşu karar
  verir ve `success` bitmiş olmalıdır. Her parçası tamamlanmış: her
  mutantın sonucu var, missed = 0, timeout = 0, bellek sınırı = 0; her
  parçanın araç defteri `--require-zero-skips --min-ran 60`'tan geçer
  (`Tool ledger [mutants-k]: … SKIP: 0` satırı bu koşunun günlüğünde
  yeniden yazılır); her parçanın damgası (`stamp.json`, temel koşu
  geçtikten sonra yazılır) anahtardaki rustc, cargo-mutants, nextest,
  araç zincirini ve imajın haftalık sürümünü adlandırır ve etkisiz yolların yokluğunu gösterir; bu koşunun
  listesindeki (bugünkü `exclude_re`'den sonra) her mutant o koşuda
  denenmiş ve caught ya da unviable'dır. Biri tutmazsa kapı baştan koşar,
  günlük nedenini yazar (`not reused, the gate runs in full: …`).
- **Kaynak ve güven:** önceki sonuç `actions/cache`'ten değil, o koşunun
  artifact'larından gelir (`mutants-list`, `mutants-out-<k>`), API
  üzerinden (`gh api`, `github.token`, iş izni `actions: read`); her
  zip'in sha256'sı GitHub'ın yüklemede kaydettiği özetle karşılaştırılır,
  sonra açılır. Artifact'ı yalnız kendi koşusu yükleyebilir. Anahtar iş
  akışını ve `scripts/`'i içerdiği için önceki sonucu bugünkü kodun
  aynısı üretmiştir: elle değiştirilmiş bir iş akışının yüklediği bir
  sonuç başka bir anahtar taşır. İmza yok: PR'ın kendi koşusu bir sırrı
  da okuyabilirdi, imza bir şey katmazdı.
- **Günlük ve özet:** kapsam işi `Reuse key …`, her önceki koşu için bir
  satır ve `reused: run <id> (SHA <tam sha>), key <özet>` yazar
  (`::notice title=Mutation gate::`); parça işi atlanır; özet işi kararı
  önceki koşunun sonuçlarından yeniden denetleyerek verir, başlığı
  `Pull-request gate: passed (reused)`'dır ve o koşunun tablolarını,
  bugünkü `exclude_re`'nin dışarıda bıraktığı mutantları gösterir.
- **Hiç yeniden kullanılmayanlar:** timeout ve bellek sınırı (yüke ve
  koşucuya bağlı), tamamlanmamış ya da iptal edilmiş koşu, SKIP'li ya da
  araçları farklı koşu, missed içeren koşu, başka bir PR'ın ya da fork'un
  koşusu, `workflow_dispatch` koşusu.
- **Sınırlar:** main ilerleyip birleştirme ağacı etkisiz olmayan bir yolda
  değişince kapı baştan koşar. Ağaç dışında kalan girdiler: `build.rs`'in
  sürüm metnine yazdığı commit kısaltması (testler onu git'in kendi
  çıktısıyla karşılaştırır, değerine bağlı değildir) ve koşucunun yükü
  (timeout'lar bu yüzden yeniden kullanılmaz).

## Kitap ve Pages düzeni (ADR-0100)

| Yol | İçerik |
|---|---|
| `/` | En büyük `vX.Y.Z` tag'inin kitabı, bantsız. Henüz sürüm tag'i yoksa main'in kitabı, her sayfanın başında "Pre-release" bandıyla |
| `/dev/` | Her zaman main'in kitabı, "Development version" bandıyla |
| `/install.ps1`, `/install.sh` | Her zaman main'deki `scripts/install/` (ADR-0096) |

- Tag seçimi: `git tag --sort=-v:refname`, yalnız `^v[0-9]+\.[0-9]+\.[0-9]+$`.
  Aşama tag'leri (`v0.1.0-f1`, `v0.2.0-f2`, `v0.3.0-f4`) ve sonekli her
  tag atlanır; kitapları yoktur.
- Bantları `book/tools/assemble_site.py` ekler (mdBook'un uyarı kutusu
  işaretlemesi, her sayfada `<main>`'in başı). Yerelde:
  `mdbook build book && python book/tools/assemble_site.py --main book/book --out site`.
- `github-pages` ortamı yalnız main'den yayına izin verir. Bu yüzden sürüm
  tag'i push'unda iş yayımlamaz; `gh workflow run book.yml --ref main` ile
  main'de yeni bir koşu başlatır, o koşu yeni tag'i bulur. Elle aynı komut
  siteyi yeniden kurar.
- PR'da site aynı adımlarla kurulur (bantlar, `/dev/`, kurulum betikleri),
  yüklenmez.
- `volt explain` bağlantıları kökü gösterir (kurulu sürümün kitabı);
  tutarlılık denetimi (kontrol 15, 17) `/dev/` adreslerini de
  `book/src`'ye eşler, explain'de `/dev/` bağlantısını ve `/dev/` altındaki
  kurulum betiği adresini (kontrol 13) reddeder.

## Verilator: sabit hakem (ADR-0107, "Hakem sürümü"; #145; 5.054: ADR-0121, #309)

- Simülasyon yapan her iş (`ci.yml` "Verilator lint", `book.yml` "Book code
  examples", `difftest.yml`, `mutants.yml`'nin parçaları) Verilator'ı
  `.github/actions/verilator` eyleminden alır. Sürüm tek bir yerde
  tanımlıdır: `scripts/ci/verilator.env` (sürüm, imaj, özet).
- Eylem Verilator'ı Docker arka ucunun özetle sabitlenmiş imajından
  (`verilator/verilator:v5.054`) kopyalar, `~/verilator` altında önbelleğe
  alır (anahtar: sürüm + özet), OSS CAD Suite'in kendi Verilator'ını
  (geliştirme derlemesi) siler ve `verilator --version` beklenen sürümü
  vermezse ya da PATH'te birden çok `verilator` varsa işi düşürür.
- Yosys, sby ve çözücüler OSS CAD Suite'ten gelmeye devam eder.
- `crates/volt-tools/tests/verilator_pin_tests.rs` CI'ın sürümüyle Docker
  arka ucunun imajını (`SIMULATION_IMAGE`) karşılaştırır; kitap, README'ler,
  `volt explain` ve testler imajı adıyla anıyorsa sabitlenen sürümü
  anmalıdır (ADR'ler, CHANGELOG ve sürüm notları tarih kaydıdır,
  taranmaz).
- Sürüm güncellemesi ayrı bir PR'dır: `verilator.env` ve `docker.rs`
  birlikte değişir, kanarya testi
  (`crates/volt-driver/tests/verilator_canary_tests.rs`) yeni sürümle
  geçer ve her ölçüm tablosuna (`ISSUE_302_MEASURED`, `ISSUE_9256_MEASURED`,
  `DFG_ON_MEASURED`) yeni sürümün ölçülmüş satırı eklenir; fark testinin
  büyük koşusu, metamorfik gecelik koşu ve kitap yeni hakemle yeniden
  koşar. Kitaptaki `volt test` ve `volt doctor` çıktıları gerçek bir
  koşudan yeniden alınır.
- `verilated.mk`'nin `OBJCACHE ?= ccache` satırı kopyada `OBJCACHE ?=`
  olur: değişken ortamdan gelir. ccache'i kuran iş onu `ccache` yapar
  (aşağıda "ccache"); kurmayan iş doğrudan derler.
- Sınır: 5.052'nin ve 5.054'ün de aynı sınıftan bir DFG yanlış derlemesi
  var (#148, Verilator #9060; `-fno-dfg` kalır); kanarya yalnız #145'in
  tetikleyicisini kapsar. 5.054'te işaretli `>>>`'nın dar bir hatası kalır
  (Verilator #9256, ADR-0121 §4).

## ccache: Verilator derlemelerinin C++'ı

Verilator her derlemede ürettiği C++'ı ve kendi çalışma zamanı
kitaplığını (`verilated.cpp` ve diğerleri) `verilated.mk` ile derler; her
derleme satırı `$(OBJCACHE) $(CXX) ...`'tır. `.github/actions/ccache`
bu derlemeleri ccache'ten geçirir:

- **Kullanan işler:** `ci.yml` "Verilator lint", `metamorphic-nightly.yml`,
  `book.yml` "Book code examples", `difftest.yml`'nin parçaları (yalnız
  iş içinde, `persist: off`; aşağıda "Kaydetme"). Eylem iki kez çağrılır: `phase: start` Verilator eyleminin hemen
  ardından, `phase: end` (`if: always()`) Verilator derlemeleri bittikten
  sonra.
- **start:** ccache'in sabitlenmiş sürümünü (4.14.2, statik ikili; sürüm
  ve sha256 eylemin "ccache - pinned release" adımında) GitHub sürüm
  sayfasından indirir, özetini denetler ve `~/ccache-bin`'de önbelleğe
  alır. apt ile kurulum iş başına 14-42 s sürüyordu (`apt-get update` ve
  dpkg veritabanı). İndirme ya da özet tutmazsa, ya da ikili beklenen
  sürümü vermezse iş `::error` ile düşer: iş sessizce yavaş sürmez.
  `OBJCACHE=~/ccache-bin/ccache` (mutlak yol),
  `CCACHE_DIR=~/.cache/volt-ccache` ve `CCACHE_MAXSIZE=1G` iş ortamına
  yazılır; günlük ccache sürümünü, `verilated.mk`'nin `OBJCACHE` ve `CXX`
  satırlarını, derleyiciyi ve önbellek anahtarını gösterir. Ardından
  önceki koşunun önbelleğini geri yükler ve istatistikleri sıfırlar.
- **Anahtar:** `ccache-v1-<ImageOS>-<ImageVersion>-<CXX><sürüm>-<CXX --version
  ilk satırının özeti>-verilator<sürüm>-ccache<sürüm>-<iş>-<run_id>-<deneme>`. Derleyici
  `verilated.mk`'nin `CXX` satırından okunur. Geri yükleme önce aynı işin,
  yoksa aynı imaj, derleyici ve Verilator'lı herhangi bir işin en yeni
  önbelleğini alır. Koşucu imajı, derleyici ya da Verilator değişince
  önbellek boş başlar.
- **end:** `ccache --print-stats`'i `scripts/ci/ccache_report.py` okur;
  günlükte, bir `::notice`'ta ve iş özetinde `ccache [<iş>]: H hits (direct
  D, preprocessed P), M misses, hit rate R %; other non-zero counters: …;
  cache S MiB in F files` satırı çıkar. Bu koşunun kullanmadığı girdiler
  silinir (`ccache --evict-older-than`; isabet girdinin zamanını yeniler),
  önbellek işin çalışma kümesi kadar kalır. Başarılı işte ccache hiç
  derleme görmediyse adım düşer.
- **Kaydetme:** main'de, gecede ve elle koşuda her başarılı iş kaydeder;
  PR'da yalnız kendi işinin önbelleği geri yüklenmemişse (soğuk başlangıç)
  kaydeder, böylece PR koşuları önbellek kotasını doldurmaz. Düşen iş
  kaydetmez.
- **`persist: off`** (`difftest.yml`'nin parçaları, #255): önbellek
  yalnız iş içinde yaşar; eylem ne geri yükler ne kaydeder, günlük bunu
  yazar. Fark testinin rastgele tasarımları koşudan koşuya tekrar etmez;
  isabetler Verilator'ın çalışma zamanı kitaplığının aynı iş içinde her
  toplu koşuda yeniden derlenmesidir. Koşu 44112926993'te soğuk başlayan
  parça 159, önbelleği geri yüklenen parçalar 162 isabet aldı; her parça
  yaklaşık 1 GB (948-1114 MiB) kaydetti. Gecelik koşuda bu her gece ~4 GB
  olurdu; deponun önbellek kotası 10 GB'tır ve aşılınca en eski girdiler
  (main'in Rust ve araç önbellekleri de) silinir.
- **Önbellek yoksa ya da bozuksa** iş doğru çalışır: geri yükleme
  ıskalarsa ya da indirilemezse derlemeler ıskadır; ccache okuduğu her
  girdiyi denetler, bozuk girdiyi yeniden derler. ccache üretilen kodu
  değiştirmez.
- **ccache kapalı koşu:** `gh workflow run ci.yml --ref <dal> -f ccache=off`
  (ya da `metamorphic-nightly.yml`): aynı commit ccache'siz koşar, günlük
  bunu `::warning` ile yazar. ccache'ten şüphelenildiğinde ya da ölçüm için.
- **Kullanmayan işler:** "Docker backend" işi Verilator'ı her derlemede
  yeni bir konteynerde koşar (Docker köprüsü, ADR-0094); imajın kendi
  ccache'i konteynerle birlikte silinir ve iş üç derleme yapar. Önbelleği
  koşucuya bağlamak `volt`'un Docker köprüsünü değiştirmek olurdu.
  `mutants.yml`'nin parçaları henüz kullanmaz: mutant başına test süresi
  sınırı aynı parçada ölçülen temel koşudan gelir ve önbelleği dolu bir
  temel koşu sınırı, üretilen SV'yi değiştiren bir mutantın soğuk
  derleme süresinin altına çekebilir (missed bir mutant timeout olur);
  seçenekler ve gereken ölçüm #300'de.

## Coverage

- `ci.yml`'deki coverage işi ("Coverage (informational)") bilgilendirme
  amaçlıdır. Düşmesi PR'ı engellemez, çünkü dal korumasında zorunlu kontrol
  değildir.
- Hata gizlenmez: işte de adımlarında da `continue-on-error` yoktur. Codecov
  yüklemesi `fail_ci_if_error: true` ile koşar; yükleme düşerse iş kırmızı
  görünür.

## Fuzz

- PR/push'taki fuzz işi 60 saniyelik bir duman testidir; CI'nın toplam süresi
  artık en uzun iş (Verilator lint, ~2 dk) tarafından belirlenir. Önceden 300 sn
  fuzz tek başına ~6 dk sürüyordu.
- Gecelik iş corpus'u `actions/cache` ile saklar: en yeni
  `fuzz-corpus-parse_never_panics-*` girdisini geri yükler, üstüne fuzz'lar ve
  büyüyen corpus'u yeni bir anahtarla kaydeder (önbellek girdileri değişmez).
  Çökme olsa da corpus kaydedilir.
- PR fuzz işi aynı corpus'u yalnız okur (main'de kaydedilen önbellek PR'lara
  görünür); `tests/ui/` ve `tests/fuzz_regressions/` (eski fuzz bulguları,
  ADR-0067) her iki işte de salt okunur tohumdur.
- Çökme bulunursa girdi `fuzz-crash-parse_never_panics*` artifact'ı olarak
  yüklenir. Yerelde yeniden üretmek için (Linux/WSL, nightly):
  `cargo +nightly fuzz run parse_never_panics <indirilen-dosya>`.
- Elle tetikleme: `gh workflow run fuzz-nightly.yml -f seconds=1800`
  (`seconds` isteğe bağlı, varsayılan 1800).

## Sürüm yayımlama (ADR-0093, ADR-0105)

Sürüm günü komutları depo kökündeki [`RELEASING.md`](../../RELEASING.md)'dedir.
İş akışı yalnız **taslak** oluşturur; yayımlamak her zaman elle yapılır.

- **Kuru koşu** = sürüm etiketi push'u olmayan her koşu:
  `gh workflow run release.yml --ref <dal>`. Derler, her platformda duman
  testi (`volt X.Y.Z (<commit>)`) ve kurulum testi yapar, `SHA256SUMS`'ı ve
  sürüm notunu denetler; sonuçlar yalnız artifact'tır (`volt-<hedef>`,
  `volt-vsix`, `volt-release-X.Y.Z`, `volt-release-notes-X.Y.Z`). Son iş
  ("Dry run - nothing published") atlananları ve o sürümün Release'i ile
  etiketinin durumunu iş özetine yazar.
- **Etiket koşusu** aynı işleri koşar, ardından `publish` işi (koşulu
  `needs.version.outputs.dry_run == 'false'`, yazma izni olan tek iş):
  derleme kaynağı kaydı (Sigstore, herkese açık) ve
  `gh release create --draft --verify-tag`.
- Taslağın notu `docs/release-notes/vX.Y.Z.md`'dir (İngilizce; üst satır
  `# Volt X.Y.Z`, ilk bölüm `## Behavior changes`); dosya yoksa ya da
  `scripts/release/notes.py` denetiminden geçmezse koşu `publish`'ten önce
  düşer. `book.yml`'nin `release-notes` işi aynı denetimi her PR'da yapar.
- Sürüm numarası üç yerde aynıdır: `Cargo.toml` (`[workspace.package]
  version`), `crates/volt-sv-emit/src/lib.rs` (`VOLT_VERSION`) ve
  `editors/vscode/package.json`. İlk ikisini
  `cli_tests::version_flag_prints_the_cargo_package_version`, üçüncüsünü
  `version` işi denetler; tag ile Cargo sürümü uyuşmazsa ilk iş düşer.
- Taslağı **"Set as a pre-release" işaretlemeden**, "latest" olarak
  yayımla: kurulum betikleri ve kitaptaki doğrudan bağlantılar
  `releases/latest/download/<ad>` kullanır; GitHub'ın `latest`'i taslakları
  ve ön sürümleri görmez (ADR-0096). Yanlış tag'i geri alma:
  `RELEASING.md`, "Undo".

Denetim: `gh attestation verify <arşiv> -R volt-hdl/volt` ve
`sha256sum -c SHA256SUMS`. VS Code Marketplace ve crates.io yayını bu iş
akışında yoktur (hesap/jeton gerektirir).
