# CI iş akışları

| Dosya | Tetik | İçerik |
|---|---|---|
| `ci.yml` | push (main), PR | biçim + clippy + test, Verilator, tutarlılık, formal, OpenSTA, coverage, **60 sn fuzz** |
| `fuzz-nightly.yml` | her gün 03:00 UTC, elle (`workflow_dispatch`) | **30 dk fuzz**, corpus geceden geceye taşınır |
| `release.yml` | yalnız tam `vX.Y.Z` tag'i (`-rc`, aşama ve önekli tag'ler tetiklemez; tutarlılık kontrolü 18), elle (`workflow_dispatch` = kuru koşu, herhangi bir daldan) | 4 platform arşivi + her platformda duman testi + kurulum betikleri bu arşivlerle + `.vsix` + `SHA256SUMS` + sürüm notu (`docs/release-notes/vX.Y.Z.md`); yalnız tag'de `publish` işi: derleme kaynağı kaydı ve **taslak** Release (ADR-0093, ADR-0105). Kuru koşu yalnız artifact yükler. PR'da koşmaz |
| `book.yml` | push (main), `vX.Y.Z` tag'i, PR, elle | kitap (`book/`, mdBook): denetleyicinin öz-testi, her ```` ```volt ```` bloğu `volt check`'ten, her test bloğu Verilator'dan geçer; mdbook uyarısız derlenir; site kökü ve `/dev/` birleştirilir (aşağıda "Kitap ve Pages düzeni"). Yalnız main'de GitHub Pages'e yayımlar (https://volt-hdl.github.io/volt/; depo ayarı Settings → Pages → Source: GitHub Actions gerekir). Tag'de yayımlamaz, main'de yeni bir koşu başlatır |
| `mutants.yml` | yalnız elle (`workflow_dispatch`); PR'da ve push'ta koşmaz, zorunlu kontrol değildir | **mutasyon testi** (ADR-0109): cargo-mutants, kapsam `.cargo/mutants.toml`; `MUTANTS_SHARDS` parça (`--shard k/N`; N ilk işte ADR-0109 §3'ün hesabıyla koşunun mutant sayısından bulunur, `shards` girdisi onu ezer), her parçada araçlı işin OSS CAD Suite'i ve `VOLT_REQUIRE_TOOLS=all`, testler nextest ile; ilk iş her parçanın mutant listesini `mutants-list` artifact'ı olarak yükler, her parça `mutants-out-<k>` artifact'ını kesilse de yükler (`if: always()`), son iş toplam/caught/missed/timeout/unviable ve missed listesini iş özetine yazar (`mutants-summary` artifact'ı). Missed mutant işi düşürmez; tamamlanmayan parça (artifact'ı yok ya da 300 dakikada bitmedi; iş sınırı 330 dakika) özetin başında test edilmeyen mutantlarıyla "NOT COMPLETED" diye adlandırılır ve özet işini düşürür. Tetik: `gh workflow run mutants.yml --ref <dal>`. Hedefli koşu (ADR-0109, "Hedefli koşu"): `-f re=<regex>` yalnız adı eşleşen mutantları dener; cargo-mutants `--re` ile alan silme mutantlarını süzmediğinden liste `scripts/ci/mutants_target.py`'nin yazdığı satır diff'iyle (`--in-diff`) ikinci kez daraltılır ve son listede eşleşmeyen bir mutant varsa ilk iş düşer; `-f shards=<1-20>` parça sayısı |
| `install.yml` | PR/push (yalnız `scripts/install/`, `scripts/release/`, `install.yml`, `release.yml` değişince), pazartesi 05:17 UTC, elle | kurulum betikleri (ADR-0096): shellcheck + PSScriptAnalyzer; üç platformda derle → `scripts/release/package.sh` → `VOLT_ARCHIVE` ile kur, yeni kabukta `volt --version`, yeniden kur, kaldır (dosya ve PATH izi kalmaz), bozuk `SHA256SUMS`; sahte GitHub'a (`test/fake-github.py`) karşı ağ senaryoları (sürüm yok, varlık 404, kopan indirme, API hız sınırı → yönlendirme, geçici hata → yeniden deneme, yanıt yok); Windows'ta PowerShell 7 ve 5.1 ayrı adım. Haftalık/elle: gerçek tek satırlık komutlar; sürüm yokken "henüz sürüm yok" iletisini, sürüm varsa kurulumu sınar |

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

## Verilator: sabit hakem (ADR-0107, "Hakem sürümü"; #145)

- Simülasyon yapan her iş (`ci.yml` "Verilator lint", `book.yml` "Book code
  examples", `difftest.yml`, `mutants.yml`'nin parçaları) Verilator'ı
  `.github/actions/verilator` eyleminden alır. Sürüm tek bir yerde
  tanımlıdır: `scripts/ci/verilator.env` (sürüm, imaj, özet).
- Eylem Verilator'ı Docker arka ucunun özetle sabitlenmiş imajından
  (`verilator/verilator:v5.052`) kopyalar, `~/verilator` altında önbelleğe
  alır (anahtar: sürüm + özet), OSS CAD Suite'in kendi Verilator'ını
  (geliştirme derlemesi) siler ve `verilator --version` beklenen sürümü
  vermezse ya da PATH'te birden çok `verilator` varsa işi düşürür.
- Yosys, sby ve çözücüler OSS CAD Suite'ten gelmeye devam eder.
- `crates/volt-tools/tests/verilator_pin_tests.rs` CI'ın sürümüyle Docker
  arka ucunun imajını (`SIMULATION_IMAGE`) karşılaştırır.
- Sürüm güncellemesi ayrı bir PR'dır: `verilator.env` ve `docker.rs`
  birlikte değişir, kanarya testi
  (`crates/volt-driver/tests/verilator_canary_tests.rs`) yeni sürümle
  geçer, fark testinin büyük koşusu yeni hakemle yeniden koşar.
- `verilated.mk`'nin `OBJCACHE ?= ccache` satırı kopyada boşaltılır:
  imaj ccache ile yapılandırılmış, koşucuda ccache yok.
- Sınır: 5.052'nin de aynı sınıftan bir DFG yanlış derlemesi var (#148);
  kanarya yalnız #145'in tetikleyicisini kapsar.

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
