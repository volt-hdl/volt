# Tarayıcıda Volt oyun alanı: fizibilite raporu

- **Deneme dalı:** `claude/keen-euler-vlen7r` (main'e PR yok, merge yok).
- **Tarih:** 2026-10-05. **Taban:** `main` @ `754ab30`.
- **Araç zinciri:** rustc 1.97.0, cargo 1.97.0, `wasm32-unknown-unknown`,
  wasm-bindgen 0.2.129 (crate ve CLI aynı sürüm), binaryen wasm-opt 132,
  Node 22.22.0, Chromium 1194 (Playwright 1.56.1, headless).
- **Yol haritası maddesi:** [Browser playground](../../docs/roadmap.md#browser-playground).

**Kısa sonuç:** Yapılabilir, ve önündeki engel küçük. İstenen yedi crate
wasm32'ye değişiklik yapmadan derleniyor. Mevcut genel API'lerle kurulan
bellek içi bir `compile()` fonksiyonu, dört örnekte ve iki modülsüz
girdide `volt build --single-file` ile **aynı tanı kodlarını ve bayt bayt aynı SV'yi** üretiyor.
Bu fonksiyon tarayıcıda da çalışıyor. wasm çekirdeği gzip ile 556 KiB.
Sayfa 4G bağlantıda yaklaşık 1 saniyede hazır oluyor; sonraki her
derleme 1–50 ms sürüyor. Sunucu gerekmiyor, bu yüzden GitHub Pages'te
statik `/play/` dizini olarak barınabilir. MVP için tahmin 10–13 iş günü
(6. bölüm).

Denemedeki dosyalar (`experiments/volt-play/`, kök çalışma alanının üyesi
değil; `cargo test`, `just check` ve CI onu görmüyor):

| Dosya | Ne |
|---|---|
| `src/lib.rs` | `compile(source, lang) -> Output`: bellekteki metinden tanılar + SV + JSON zarfı |
| `src/web.rs` | wasm-bindgen arayüzü: `compile_json(source, lang)`, `version()` |
| `tests/parity.rs` | Çekirdek ile gerçek `volt build` karşılaştırması (6 test) |
| `web/index.html` | Statik sayfa: düzenleyici, örnek seçici, dil seçici, tanılar, SV |
| `web/build.sh` | `web/dist/` kurar (cargo + wasm-bindgen) |
| `web/serve.mjs` | gzip'li yerel statik sunucu (ölçüm için) |
| `web/node-smoke.mjs`, `web/stack.mjs` | Node'da wasm testi: örnekler, süre, yığın derinliği |
| `web/browser-measure.cjs`, `web/sizes.cjs`, `web/sections.cjs` | Tarayıcı yükleme ölçümü, boyut ölçümü |
| `examples_src/*.volt` | blinky, cdc_error (kitabın `crossing.volt`'u), counter (`templates/minimal`), mmio_blinker (`templates/mmio`) |
| `results/` | Komut çıktıları ve ekran görüntüleri |

Yeniden üretmek için (depo kökünden):

```
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cargo build -p volt-driver --bin volt                  # eşlik testi gerçek volt'u koşar
cd experiments/volt-play
cargo test -- --test-threads=1                         # 6 eşlik testi
web/build.sh                                           # web/dist/
node web/serve.mjs web/dist 8765                       # http://127.0.0.1:8765/
```

---

## 1. Crate'lerin wasm32-unknown-unknown'a derlenmesi

Komut (depo kökünde, her crate için ayrı):

```
rustup target add wasm32-unknown-unknown
cargo build -p <crate> --target wasm32-unknown-unknown --release
```

Çıktının tamamı `results/wasm-build.log` dosyasında. Özeti (debug derleme
de aynı sonucu verdi):

```text
$ cargo build -p volt-span --target wasm32-unknown-unknown --release
    Finished `release` profile [optimized] target(s) in 0.27s
exit=0
$ cargo build -p volt-diagnostics --target wasm32-unknown-unknown --release
    Finished `release` profile [optimized] target(s) in 9.13s
exit=0
$ cargo build -p volt-ast --target wasm32-unknown-unknown --release
    Finished `release` profile [optimized] target(s) in 1.31s
exit=0
$ cargo build -p volt-syntax --target wasm32-unknown-unknown --release
    Finished `release` profile [optimized] target(s) in 19.53s
exit=0
$ cargo build -p volt-hir --target wasm32-unknown-unknown --release
    Finished `release` profile [optimized] target(s) in 15.38s
exit=0
$ cargo build -p volt-lower --target wasm32-unknown-unknown --release
    Finished `release` profile [optimized] target(s) in 0.08s
exit=0
$ cargo build -p volt-sv-emit --target wasm32-unknown-unknown --release
    Finished `release` profile [optimized] target(s) in 11.92s
exit=0
```

Karşılaştırma için (istenmedi): `volt-sdc-emit`, `volt-sw-emit` ve
`volt-tools` de derleniyor. `volt-lsp` ve `volt-driver` derlenmiyor:
`error: Only features sync,macros,io-util,rt,time are supported on wasm.`
(tokio `rt-multi-thread`). Sürücü ayrıca clap, ctrlc ve `std::process`
kullanıyor. Oyun alanı bu ikisine ihtiyaç duymuyor.

wasm32-unknown-unknown'da `std::fs`, `std::env`, `thread::spawn` ve
`Instant::now` **derlenir**, ama çalışma zamanında hata döndürür ya da
panik eder. Bu yüzden "derleniyor" yetmez. Kaynakta bu çağrıları aradım
(`grep std::fs|env::|thread::|Instant|SystemTime|canonicalize|...`) ve
oyun alanının yolundaki her birini Node ile Chromium'da çalıştırarak
denedim:

| Crate | wasm32 | Çalışma zamanı engeli | Kaldırma zorluğu |
|---|---|---|---|
| `volt-span` | derleniyor | Yok. `PathBuf` yalnız ad olarak tutuluyor, dosya açılmıyor. | — |
| `volt-diagnostics` | derleniyor | Yok. codespan-reporting `NoColor` arabelleğine yazıyor, terminal yok; serde_json saf. Dil seçimi süreç geneli `AtomicU8` (`set_lang`), tek iş parçacıklı wasm'da sorun değil. | — |
| `volt-ast` | derleniyor | Yok. | — |
| `volt-syntax` | derleniyor | **İş parçacığı:** `with_compiler_stack` (ADR-0080) derleyiciyi 64 MiB yığınlı bir iş parçacığında koşturuyor. wasm'da `spawn` `Err` dönüyor, kod bu durumda zaten çağıranın yığınına düşüyor (panik yok). wasm'ın varsayılan yığını 1 MiB. Release derlemede parser sınırına kadar (`if` iç içe 250, ikili işlem, `if/else` ifadesi; sınırda E0018) **tuzak oluşmadı** (`web/stack.mjs`). | Kolay. Gerekirse `-C link-arg=-zstack-size=8388608` (kod değişikliği yok). Debug wasm'da ağır geçitlerde 1 MiB yetmeyebilir; yayın derlemesi her zaman release. |
| `volt-hir` | derleniyor | **Dosya sistemi ve ortam:** `unit_load` (`load_unit[_with_text]`, `Manifest::lookup`, `manifest_warnings_into`, `test_sibling_path`), `manifest_search` (`env::var_os`, `current_dir`, `canonicalize`, `.git` arama), `attrs::UnenforcedLint::discover`, `extern_source::FsSourceLocator`/`project_root`. Hepsi hata döndürüyor, panik etmiyor; ama oyun alanı bunları **hiç çağırmıyor**: birim bellekte kuruluyor, `SourceLocator`/`TestFileLoader` trait'leri oyun alanında dosyasız uygulanıyor. Statik `Mutex` (`CHECKED_MANIFESTS`) wasm'da çalışıyor. | MVP için yok. Çok dosyalı oyun alanı için: yükleyicideki dosya erişimini bir trait arkasına almak, 1–2 gün. |
| `volt-lower` | derleniyor | Yok: crate boş (4 satır). Bu kasıtlı: ADR-0005 ve ADR-0010'a göre CIRCT arka ucu için ayrılmış bir yer tutucu; CIRCT/melior yalnız burada görünebilir (tutarlılık kontrolü 4). Hiçbir crate kodunu kullanmıyor, `volt-driver` bağımlılık olarak listeliyor. | — |
| `volt-sv-emit` | derleniyor | Yok. G/Ç yok, SV metin olarak dönüyor. | — |

Zaman (`Instant`, `SystemTime`) bu yedi crate'te hiç kullanılmıyor: süre
ölçümü sürücüde (`duration_ms`). `HashMap`'in `RandomState`'i
wasm32-unknown-unknown'da rastgelelik kaynağı olmadan çalışıyor; denemede
panik görülmedi.

## 2. Tek fonksiyonla bellekten tanılar + SV

**Kurulabiliyor, mevcut genel API'lerle.** `src/lib.rs` içindeki
`compile(source: &str, lang: Lang) -> Output` (~190 satır, belgeleriyle) ve sürücünün
`compile_all` aşama sırasının tek dosyalık kopyası:

1. `SourceMap::add_file("main.volt", …)`, `volt_syntax::parse_unit`,
   `volt_hir::unit_load::register_generated` (`@mmio` sentetik kaynakları)
2. `UnitInfo::add` (paket adı: `package` bildirimi ya da `main`)
3. `volt_hir::pre_resolve_checks(…, UnenforcedLint::Warn)`
4. `volt_hir::check_imports`. Olmayan paket için E1011 burada da üretiliyor.
5. `volt_hir::resolve_extern_sources(&ast, &NoFiles)`: `@source` → E1012
   ("the playground has no files")
6. `volt_hir::resolve_unit` + `volt_hir::run_semantic_stages(…, Some(&NoFiles), &TestTarget::Partial(None), …)`
7. `volt_sv_emit::unit_source_texts` + `emit_unit(…, SvaMode::None, ConstArrayStyle::default())`
8. `volt_hir::annotate_generate`, `volt_diagnostics::cap_diagnostics`
9. JSON: `volt_diagnostics::to_json_value`, metin: `render_human`

Hepsi `with_compiler_stack` içinde koşuyor. Kod `#![forbid(unsafe_code)]`
ile derleniyor (wasm-bindgen makrosu dahil).

**Doğrulama** (`tests/parity.rs`): her örnek geçici bir dizinde
`main.volt` olarak `volt build --single-file --format json` ile derlendi.
Tanı kodları ve `build/rtl/main.sv` çekirdeğin çıktısıyla karşılaştırıldı.

```text
$ cd experiments/volt-play && cargo test -- --test-threads=1
test blinky_matches_volt_build ... ok
test cdc_error_matches_volt_build ... ok
test counter_matches_volt_build ... ok
test mmio_blinker_matches_volt_build ... ok
test file_without_modules_has_no_sv ... ok
test turkish_diagnostics ... ok
test result: ok. 6 passed; 0 failed
```

Ayrıca `cargo fmt --check` ve `cargo clippy --all-targets -- -D warnings`
(yerel hedef ve `--target wasm32-unknown-unknown`) temiz.

**Sürücüye (volt-driver) bağımlı kalınan yerler:**

| Ne | Nerede | Sonuç |
|---|---|---|
| Aşama sırası ve kapıları (parse hatasında dur, import/extern, anlamsal, emit, `annotate_generate`, üst sınır) | `volt-driver/src/main.rs` `compile_all` (özel fonksiyon) | Oyun alanı sırayı **kopyalıyor**. Kopya zamanla sapar; ADR-0070'in anlamsal aşamalar için çözdüğü sorunun aynısı. Çözüm: `compile_all`'ı dosya erişimini trait ile alan bir kütüphane fonksiyonuna taşımak (`volt-hir` ya da yeni bir `volt-compile`); sürücü, LSP ve oyun alanı aynı fonksiyonu çağırır. Yeni ADR gerekir. |
| Birim yükleyicisi | `volt_hir::unit_load::load_unit_with_text` | Ana metin bellekten verilebiliyor; Volt.toml ve bağımlılık dosyaları ise diskten okunuyor. Oyun alanı yükleyiciyi atlayıp birimi kendisi kuruyor (tek dosya). |
| JSON zarfı | `volt-driver` `json_envelope` (özel) | Kopyalandı (`duration_ms`/`artifacts` yok, `sv` ve `rendered` eklendi). Tanı nesnesinin kendisi kütüphanede (`to_json_value`), kopya gerekmiyor. |
| Varsayılanlar | `DEFAULT_MAX_DIAGNOSTICS = 1000`, `SvaMode` seçimi, dil önceliği (`--lang` > `VOLT_LANG` > Volt.toml) | Oyun alanında sabit: 1000, `SvaMode::None` (`volt build`'in `--emit=sva` olmadan varsayılanı), dil sayfadan seçiliyor. |
| Test veri dosyaları | `volt-driver/src/sim_lower.rs` `FsTestFiles` | Oyun alanında `NoFiles` (her yol "dosya yok"). |
| Modülsüz dosya | `volt-driver` `build()`: SV yazmaz, "no module … no SystemVerilog written" notu ve `use main::<name>` yardımı verir | `emit_unit` yine de başlıktan oluşan 11 satırlık bir SV döndürüyor; karar sürücüde. Deneme bunu ilk sürümde kaçırdı ve boş girdide başlığı gösterdi. Düzeltildi (`emitted.modules` boşsa SV yok), `file_without_modules_has_no_sv` testi eklendi. Not/yardım metni ise sürücüde kaldı. Bu da 6. bölümün 1. maddesindeki ortak girişe taşınmalı. |
| `volt explain` metni | `volt_diagnostics::render_explanation` | Sürücüye bağlı değil; oyun alanı kullanabilir (MVP'de yok). |

**Bilinçli farklar** (MVP'de oyun alanının kapsamı dışında): çok dosyalı
birim (`use` ile başka dosya), Volt.toml (`[lint]`, `[ui] lang`), `@source`
(E1012 verir), `*_test.volt` kardeş tasarımı, `--emit=sva/sdc/c/rust`.

## 3. Tanı çıktısının biçimi ve #84

**Yapısal biçim var, yeni bir şey gerekmiyor.** `volt-diagnostics/src/json.rs`
cli-contract.md §5 şemasını üretiyor (`to_json_value`): `code`,
`severity`, `message`, `spans[]` (dosya, `start`/`end` içinde
`line`/`col`/`byte`, `label`, `primary`), `notes[]` (`kind`, `text`),
`help`, `suggestions[]` (`span`, `replacement`, `applicability`,
`additional_edits`), `explain_url`. Oyun alanı bunu olduğu gibi kullanıyor:
listedeki konuma tıklayınca düzenleyicide o aralık seçiliyor. `suggestions`
ileride düzenleyicide "düzelt" düğmesine dönüşebilir (öneriler ADR-0099
ile tur testinden geçiyor).

İnsan metni (`render_human`) de var, ve **renksiz**: codespan-reporting
`NoColor` arabelleğine yazıyor, ANSI kaçış dizisi yok. Sayfada `<pre>`
içinde doğrudan gösteriliyor ("details"). Türkçe çıktı da çalışıyor
(`neden:`, `çözüm:`; `results/node-smoke.txt`).

`explain_url` şu an hep `null`. Bu bir hata değil, bugünkü tasarım:
`crates/volt-diagnostics/src/code.rs:147-150` (`ErrorCode::explain_url`),
kitapta kod başına sayfa olmadığı için `None` döndürüyor; açıklama
`volt explain <KOD>`'da. Kod başına bir sayfa üretilirse oyun alanı ve kitap
tanıdan oraya bağlanabilir (iyileştirme, #97).

**#84 ile ilişkisi:** [volt-hdl/volt#84](https://github.com/volt-hdl/volt/issues/84),
`volt test` ve `volt run` için `--format json` istiyor: test başına kayıt
ve `volt run` çevrim tablosu, şeması ADR ister. **MVP buna bağlı değil:**
MVP `check`/`build` tanılarını gösteriyor ve onların JSON'u zaten var.
#84 simülasyonlu bir sonraki adımda önem kazanıyor ("Çalıştır" sekmesi):
oyun alanının göstereceği test kaydı ve çevrim tablosu, #84'ün
tanımlayacağı nesnenin aynısı olmalı. Öneri: #84'ün ADR'si, kaydın sürücüde
`println!` ile değil bir kütüphane fonksiyonunda üretilmesini istesin;
sürücü, CI ve wasm o zaman aynı kodu kullanır. Simülasyonun kendisi
(Verilator) tarayıcıda yok. Bunun için `volt-hir/src/sim*.rs`'deki
yorumlayıcı benzeri parçalar ayrıca incelenmeli (bu denemenin kapsamı
dışında).

## 4. Boyut ve ilk yükleme

Ölçüm komutu: `node web/sizes.cjs <dosya>` (zlib gzip -9, brotli -11).
Çıktısı `results/sizes.txt` dosyasında.

| Yapı | Ham | gzip | brotli |
|---|---|---|---|
| `opt-level="z"`, fat LTO, `panic=abort`, wasm-bindgen sonrası (**seçilen**) | 1719 KiB | **556 KiB** | 443 KiB |
| aynısı + `wasm-opt -Oz` | 1539 KiB | 608 KiB | 474 KiB |
| `opt-level=3` | 2483 KiB | 874 KiB | 645 KiB |
| `opt-level=3` + `wasm-opt -O3` | 2348 KiB | 879 KiB | 650 KiB |
| JS yapıştırıcı (`volt_play.js`) | 8 KiB | 2 KiB | 2 KiB |

Bölümler: kod 1443 KiB, veri 268 KiB (iki dilli ileti metinleri dahil).
`wasm-opt -Oz` ham boyutu küçültüyor ama sıkıştırılmış boyutu büyütüyor;
yayında kullanılmamalı. `opt-level="z"` derleme hızını hissedilir biçimde
düşürmüyor: Node'da ısınmış blinky ~1 ms, 200 modüllük dosya ~49 ms
(`results/node-smoke.txt`).

**İlk yükleme** (gerçek tarayıcı: Chromium headless, gzip'li yerel sunucu,
önbellek kapalı, CDP ile ağ kısıtlaması; sayfa açılışından ilk örneğin
derlenip gösterilmesine kadar; `results/browser.txt`):

| Bağlantı | Aktarılan | `init()` (indirme + derleme + başlatma) | Sayfa hazır |
|---|---|---|---|
| kısıtlama yok (yerel) | 563 KiB | 90 ms | 167 ms |
| 20 Mbps, 40 ms RTT | 563 KiB | 309 ms | 513 ms |
| 4G: 9 Mbps, 85 ms RTT | 563 KiB | 612 ms | 930 ms |
| yavaş 4G: 1.6 Mbps, 150 ms RTT | 563 KiB | 3017 ms | 3563 ms |

Hazır olduktan sonra: ilk derleme ~30 ms (ısınma), sonraki her düzenleme
1–5 ms (blinky, cdc, mmio). Düzenleyicide 250 ms gecikmeyle her tuşta
yeniden derleme rahatça yetişiyor. Sayfada CDC örneğine `sync()` köprüsü
eklenince E3001'in kalktığı da denendi (`errors: 0`,
`results/cdc_fixed.png`).

## 5. Barındırma: GitHub Pages, `/play/`, sürümlü site

**Sunucusuz çalışabilir.** Sayfa, bir HTML, bir JS modülü, bir `.wasm` ve
örnek `.volt` dosyalarından oluşuyor. Bütün yollar göreli (`./pkg/…`,
`examples/…`), bu yüzden sayfa `/volt/play/` altında da `/volt/dev/play/`
altında da aynı biçimde çalışır. İş parçacığı ve `SharedArrayBuffer`
kullanılmıyor, bu yüzden COOP/COEP başlığı gerekmiyor. Bu önemli, çünkü
GitHub Pages özel başlık ayarlatmıyor.

**Bu oturumda doğrulanamayanlar:** github.io bu ortamın ağ politikasında
kapalı (`curl https://volt-hdl.github.io/volt/` → 403). GitHub Pages'in
`.wasm` dosyasını `application/wasm` türüyle ve gzip ile sunduğu bilinen
davranış, ama burada denenmedi. Tür yanlış gelirse wasm-bindgen'in
`init()`'i akışsız derlemeye düşer; yine çalışır, yalnız biraz yavaşlar.
İlk yayında tarayıcının ağ sekmesinde kontrol edilmeli.

**Sürümlü siteyle uyum.** `book.yml` bugün kökü (`/`) en son `vX.Y.Z`
etiketinin `git worktree`'sinden, `/dev/`'i main'den kuruyor
(`assemble_site.py`, ADR-0100 §6). Oyun alanı aynı kalıba oturuyor:

- `/play/`: kökteki kitapla **aynı etiketin** worktree'sinden derlenir.
  Kitaptaki bir örnek ile oyun alanındaki derleyici aynı sürümdür;
  sayfa `volt_sv_emit::VOLT_VERSION`'ı gösterir (SV başlığındakiyle aynı).
- `/dev/play/`: main'den, kitabın `/dev/` sayfaları gibi "development
  version" şeridiyle.
- Oyun alanı kodunu henüz içermeyen bir etiket kökteyken `/play/`, kitabın
  yayın öncesi davranışı gibi main'den kurulur ve şerit taşır.
  `assemble_site.py`'deki mantık bunu zaten yapıyor; yalnız `play/` dizinini
  de taşıması gerekir.
- Kitaptaki "Try it" bağlantısı göreli olur (`../play/?example=…`).
  Böylece yayın kitabı yayın oyun alanını, dev kitabı dev oyun alanını açar.

CI için gerekenler: `build` işinde Rust, `wasm32-unknown-unknown` hedefi ve
`Cargo.lock`'taki sürüme **sabitlenmiş** `wasm-bindgen-cli` (sürüm
uyuşmazlığında wasm-bindgen hata verir; kurulum önbelleğe alınmalı, çünkü
kaynaktan birkaç dakika sürüyor). Tam wasm derlemesi bu makinede 4 çekirdek
ve `CARGO_BUILD_JOBS=2` ile ~40 saniye sürdü. `/play/` site düzenini
değiştirdiği için ADR-0100'ün karar metni düzenlenemez, yeni bir ADR
gerekir.

## 6. Sonuç: MVP iş listesi ve süre

MVP kapsamı: düzenleyici, örnek seçici (blinky, CDC hatası), tanılar ve
üretilen SV; simülasyon yok. Bu deneme, kapsamın ilk sürümünü
(düz `<textarea>`) zaten çalıştırıyor (`results/blinky.png`,
`results/cdc_error_tr.png`). Kalan iş, üretime hazır hale getirmek:

| # | İş | Gün |
|---|---|---|
| 1 | **Ortak derleme girişi.** `compile_all`'ın aşama sırasını dosya erişimini trait ile alan bir kütüphane fonksiyonuna taşımak; sürücü, LSP ve oyun alanı onu çağırır. Sürücü çıktısı bayt bayt aynı kalır (mevcut testler + `parity.rs`). Yeni ADR (ADR-0070'in devamı). | 2–3 |
| 2 | **`crates/volt-play`.** Bu denemeyi çalışma alanına almak. `wasm-bindgen`'i `[workspace.dependencies]` üzerinden, gerekçeli PR ile eklemek. `compile_json` API'si, testler (eşlik testleri, Türkçe/İngilizce). | 1 |
| 3 | **Dayanıklılık.** Derleyiciyi bir Web Worker'da koşmak. Panikte (`panic=abort` → wasm tuzağı, örnek ölür) örneği yeniden kurmak ve "iç derleyici hatası" iletisi göstermek. Uzun süren derlemede worker'ı sonlandırmak. | 1 |
| 4 | **Düzenleyici.** CodeMirror 6 (sürümü sabitlenmiş ESM, depoya alınmış). Volt sözdizimi renklendirme (`book/theme/volt-highlight.js` kurallarından), tanıların satır içi işaretleri (`spans`), `suggestions` ile hızlı düzeltme. | 2–3 |
| 5 | **Örnekler ve paylaşım.** Örnekleri depodaki kaynaktan kurmak (`templates/minimal/counter.volt`, kitabın `crossing.volt`'u; kopya değil). `?example=` parametresi. İsteğe bağlı: kaynağı URL'de paylaşma. | 0.5–1 |
| 6 | **Sonuç paneli.** Tanı listesi + ayrıntı (`rendered`), SV renklendirme, dil seçimi, `volt explain` metni (`render_explanation`). | 1 |
| 7 | **CI ve barındırma.** `book.yml`: wasm hedefi, sabit `wasm-bindgen-cli`, önbellek. Yayın etiketinden `/play/`, main'den `/dev/play/`. `assemble_site.py`'de şerit/yerleşim. Boyut bütçesi denetimi (ör. gzip > 700 KiB ise kırmızı). Headless tarayıcıda duman testi (örnek yüklenir, E3001 görünür). | 1.5–2 |
| 8 | **Kitap.** Tur sayfalarında "Try it" bağlantıları. Kitap kuralları (yasak sözcükler, yönlendirmeli sayfa kuralları) ve `check_book.py` uyumu. | 0.5 |
| 9 | **Belgeler.** Oyun alanı ADR'si (barındırma, sürümleme, desteklenmeyenler: çok dosya, Volt.toml, `@source`, simülasyon). CHANGELOG, yol haritası maddesinin durumu. | 0.5–1 |
| | **Toplam** | **10–13 iş günü** |

Riskler: (a) wasm-bindgen sürüm sabitleme ve CI süresi; (b) GitHub Pages
MIME/sıkıştırma davranışı (yukarıda, doğrulanmadı); (c) aşama sırası
kopyası. 1. madde yapılmazsa oyun alanı zamanla sürücüden sapar. Bu
yüzden 1. madde ilk sırada.

---

## Açık bulgular

| Bulgu | Sınıf | Yeniden üretme | Nereye |
|---|---|---|---|
| Olmayan bir paket için `volt check` aynı `use` satırında **iki** E1011 veriyor: biri yükleyiciden (sütun 5, "searched:" notuyla), biri `check_imports`'tan (sütun 16). Oyun alanında (yalnız `check_imports`) bir tane. | açık hata (yinelenen tanı), iyileştirme | `printf 'use soc::gpio::Gpio;\n\npub module Top {\n  in clk : clock\n  out q : bool\n  q = false\n}\n' > main.volt && volt check --format short main.volt` → iki satır `error[E1011]` | Issue açılmadı (görev yalnız rapordu); açılması önerilir. |
| `explain_url` JSON'da hep `null`. Bu bilinçli: kitapta kod başına sayfa yok (`code.rs:147-150`). Sayfalar üretilip alanın doldurulması bir iyileştirme. | iyileştirme | `volt check --format json` herhangi bir hatalı dosyada | #97 |
| `volt-lower` boş bir crate (4 satır) ve kodunu hiçbir crate kullanmıyor; `volt-driver` bağımlılık olarak listeliyor. ADR-0005 ve ADR-0010'da CIRCT için ayrılmış yer tutucu olarak belgeli. Soru: kaldırılsın mı (ADR-0005/0010'un ilgili kısımlarının yerini alan yeni bir ADR ister), yoksa ayrılmış olarak mı kalsın? | karar verilecek | `wc -l crates/volt-lower/src/lib.rs` | #98 |

## Atlananlar ve doğrulanmayanlar

- GitHub Pages'te gerçek yayın ve başlık kontrolü yapılmadı: github.io bu
  ortamdan erişilemiyor (403), ve görev main'e/yayına dokunmamayı istiyor.
- Firefox ve Safari denenmedi; yalnız Chromium (headless) ve Node 22.
- Mobil cihazda gerçek ölçüm yok; ağ, CDP ile taklit edildi. CPU
  kısıtlaması uygulanmadı, bu yüzden yavaş bir telefonda `init()` daha uzun
  sürer.
- Panik davranışı denenmedi: panik ettiren bilinen bir girdi bulunmadı.
  `panic=abort` ile panik, wasm örneğini kullanılmaz bırakır; çözüm 6.
  bölümün 3. maddesinde.
- `brotli` boyutu yalnız bilgi için verildi: GitHub Pages'in brotli sunup
  sunmadığı doğrulanmadı. Hesap gzip üzerinden yapıldı.
- Simülasyon/formal araçları (Verilator, Yosys, SymbiYosys) bu ortamda
  yok. Kök testlerdeki araca bağlı testler ADR-0079 kuralıyla atlandı
  (`VOLT_TOOL_BACKEND=local`, `VOLT_REQUIRE_TOOLS` yok). Bu deneme o
  kodlara dokunmuyor.
- Nightly araç zinciri kurulu değil, `just clippy-strict`'in nightly
  yarısı koşulmadı.
- CI koşusu yok: deneme dalı için PR açılmadı (istenen buydu). Bu yüzden
  okunacak bir CI günlüğü de yok.

## Doğrulama (bu dalda, yerel)

`just` kurulu değil; tarifler doğrudan koşuldu (`CARGO_BUILD_JOBS=2`,
`line-tables-only`, `VOLT_TOOL_BACKEND=local`). Kök çalışma alanı
değişmedi; deneme onun dışında.

| Komut | Çıkış | Not |
|---|---|---|
| `cargo fmt --all --check` (`just check`) | 0 | |
| `cargo clippy --all-targets -- -D warnings` (`just check`) | 0 | |
| `cargo test --all` (`just check`) | 0 | 3464 geçti, 0 düştü, 0 ignored |
| `bash scripts/check-consistency.sh` (`just consistency`) | 0 | "Tutarlılık denetimi temiz: 158 kod, 3775 test." |
| `cargo clippy --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` (`just clippy-strict`, 2. yarı) | 0 | |
| `git status --porcelain` (testlerden sonra) | — | yalnız `?? experiments/`; testler checkout'a yazmadı |
| `cd experiments/volt-play && cargo test -- --test-threads=1` | 0 | 6/6 (`results/parity-tests.txt`) |
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --target wasm32-unknown-unknown --profile web -- -D warnings` (deneme crate'i) | 0 | |
| `web/build.sh`, `node web/node-smoke.mjs`, `node web/stack.mjs`, `node web/browser-measure.cjs` | 0 | `results/` |
