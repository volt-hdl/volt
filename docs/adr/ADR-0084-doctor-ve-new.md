# ADR-0084: `volt doctor` ve `volt new` / `volt init` — Kurulum Teşhisi, Tek Kaynaklı Araç Keşfi, Doğrulanmış Şablonlar

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-27
> Etkilenen: yeni crate `volt-tools` (araç keşfi + zaman sınırlı sürüm
> sorgusu), volt-driver (`doctor/`, `new/`, `sim/verilator.rs`,
> `verify.rs`, `tests/tools/mod.rs`), volt-diagnostics
> (`explain::topics::install_steps`), `templates/` (yeni), CI
> (`.github/workflows/ci.yml` şablon adımları), cli-contract.md §1, §2,
> §9a, §9b, §10, §14; README "Getting started"
> Uygulama: Bölüm 1 → PR `feat/doctor`, Bölüm 2 → PR `feat/new` (ikisi de
> main'den, zincir değil)

## Sorun

Dil temelleri tamam (enum, struct, fn, match, blok `let`); darboğaz ilk
izlenim. İki boşluk:

1. **Eksik araç geç öğreniliyor.** Kullanıcı Verilator'ın, sby'nin ya da
   varsayılan çözücünün (ADR-0082) eksik olduğunu ancak `volt test` /
   `volt verify` hata verince görüyor — her komut kendi aracını ayrı
   ayrı, ilk çağrıda arıyor.
2. **Proje başlatma yolu yok.** `volt new`/`volt init` cli-contract.md
   §1'de F0'dan beri listeli ama uygulanmamış. Kullanıcı `Volt.toml`'u,
   dosya düzenini (`volt test` yalnız çalışma dizinindeki `*_test.volt`
   dosyalarını tarar), test dosyası biçimini ve kontrat sözdizimini
   örneklerden çıkarmak zorunda.

Kitle (r/FPGA gözlemi): çoğunluğu Windows kullanan öğrenciler ve yeni
başlayanlar; ilk beş dakika belirleyici.

## Tespit — araç keşfi kaç yerde?

ADR-0084 öncesinde **üç bağımsız uygulama** vardı (iki kopya daha
bunlardan birine yalnız yönlendiriyordu):

| Yer | Araçlar | Ortam değişkeni | Ekler |
|---|---|---|---|
| `volt-driver/src/sim/verilator.rs` `find_verilator` | Verilator | yalnız dosya yolu | `""`, `.exe`, `.bat`, `.cmd` |
| `volt-driver/src/verify.rs` `find_sby` | sby | yalnız dosya yolu | `""`, `.exe`, `.bat`, `.cmd` |
| `volt-driver/tests/tools/mod.rs` `find` (`VOLT_REQUIRE_TOOLS`, ADR-0079 §3) | 6 araç | dosya yolu **ya da PATH'teki ad** | `""`, `.exe`, `.cmd` |
| `tests/extern_source_tests.rs` `on_path`, `tests/sw_emit_tests.rs` `find_cc` | — | — | `tools::require`'a yönlendirir |

Kopyalar ayrışmıştı (ADR-0070 dersi): sürücü `VOLT_SBY=sby-wrapper`
gibi çıplak adı tanımıyor, testler `.bat` sahte aracını PATH'te
görmüyordu. CI'daki "araç var, testler koştu" iddiası komutların
gerçekte kullandığı aramayla aynı koddan gelmiyordu.

## Karar

### §1 Tek kaynak: `volt-tools`

Yeni, bağımlılıksız crate `volt-tools`: `Tool` (13 araç: verilator, sby,
yosys, boolector, bitwuzla, yices, z3, opensta, cc, cxx, rustc, make,
docker), `find`/`find_in(Tool, &ToolEnv)` ve `probe_version`. `volt test`,
`volt run`, `volt verify`, `volt doctor` ve araç bağımlı testler
(`VOLT_REQUIRE_TOOLS`, dev-dependency) aynı fonksiyonu çağırır. Sürücü
ikili crate olduğundan testler ondan içe aktaramaz; ayrı crate bunun
için.

Birleşik arama iki kopyanın BİRLEŞİMİDİR — mevcut komut çıktısı
değişmez: önce aracın değişkeni (`VOLT_VERILATOR`, `VOLT_SBY`, `CC`,
`CXX`; dosya yolu ya da PATH'teki ad, boş değer yok sayılır; bulunamayan
yol PATH'e düşer — eski davranış), sonra adaylar PATH'te, ekler `""`,
`.exe`, `.bat`, `.cmd`. Yeni olan tek durum: çıplak adlı `VOLT_SBY`
artık o adı PATH'te arar (önceden yok sayılıp `sby` aranıyordu).
`VOLT_REQUIRE_TOOLS` 13 adı da kabul eder (`all` hepsini zorunlu kılar;
CI açık liste kullanır).

### §2 `volt doctor` — yetenek grupları

Rapor araç listesi değil, **hangi komutun çalışacağının** listesidir:

| Yetenek | Komutlar | Zorunlu araçlar | Not |
|---|---|---|---|
| core | build, check, explain | — | Volt tek başına yeterli |
| simulation | test, run | Verilator ≥ 5.0, C++ derleyicisi, make | Verilator `--build` ikisini çağırır |
| verify | verify | sby, Yosys, bir SMT çözücü | boolector varsayılan (ADR-0082); yalnız bitwuzla/yices/z3 varsa "!" + `--engine <ad>` |
| timing (isteğe bağlı) | — | OpenSTA (`sta`) | üretilen `.sdc`'nin ikinci ağı (ADR-0065) |
| driver checks (isteğe bağlı) | — | cc, rustc | `--emit=c,rust` sürücülerini derler (ADR-0053) |
| docker (isteğe bağlı) | — | docker + çalışan daemon | Windows/macOS'ta araçları konteynerde çalıştırma yolu |

Proje bağlamı: `Volt.toml`'un kökü ya da aramanın durduğu yer (git kökü,
ev dizini, `VOLT_MANIFEST_DIR`; ADR-0061 araması — `find_manifest_dir`
aynen), WSL içinde `/mnt/<sürücü>` altında çalışma uyarısı (9P G/Ç'si
Verilator derlemesini ve `build/` yazımını kat kat yavaşlatır).

**Kurulum ipucu kopyalanmaz:** eksik yeteneğin altına `volt explain
simulation-setup` / `verify-setup` konusunun INSTALL bölümünden bu
işletim sistemine düşen paragraf basılır (`topics::install_steps`) ve
konu adı verilir. Metin tek yerde yaşar; konu güncellenince doctor da
güncellenir. OpenSTA ve sürücü derleyicileri için konu olmadığından
yalnız amaç yazılır.

**Asgari sürüm yalnız kanıtla:** Verilator 5.0 (Volt'un testbench'i, CI
ve `simulation-setup` 5.x; ADR-0079 5.020'nin bile geçerli SV'yi
derleyemediği durumu belgeler). Altı "too_old": yetenek "!" (çalışabilir
ama desteklenmez). sby/Yosys için ölçülmüş taban yok; sürüm yalnız
raporlanır — tahmini taban yanlış alarm üretir.

**Zaman sınırı:** her sürüm sorgusu ayrı iş parçacığında, varsayılan 5
sn (`--timeout 1..60`); süre dolunca süreç öldürülür, araç
"unresponsive". Sorgular paralel olduğundan rapor en kötü durumda bir
zaman sınırı sürer (bu makinede, Windows, araçsız + Docker açık: 1,7 sn;
yavaş olan `docker info`). stdin kapalıdır (REPL'e düşen araç takılmaz).

### §3 Çıkış kodu

- **0** — rapor üretildi; eksik araç yalnız bildirilir. Volt tek başına
  build/check/explain yapar; doctor'ın varsayılan işi bilgi vermek, bir
  öğrencinin ilk komutu kırmızı çıkış koduyla bitmemeli.
- **`--strict` → 3** — zorunlu bir yetenek (simulation, verify) tam
  değilse (eksik, eski, yanıtsız ya da varsayılan çözücü yok). 3,
  `volt test`/`volt verify`'ın eksik araç kodudur (cli-contract.md §2
  G/Ç hatası); CI kurulum adımı `volt doctor --strict` ile "araçlar gerçekten
  yerinde mi"yi tek komutla sorar. İsteğe bağlı yetenekler `--strict`'i
  etkilemez.
- 2 — kullanım hatası (clap).

Reddedilen: eksik araçta 1 (1 derleme hatası demek, CI'da yanlış
sınıflanır); "hangi yetenek zorunlu" listesi (`--require test,verify`) —
YAGNI, ihtiyaç doğunca eklenir.

**`--format=json`** (`schema: "volt-doctor/1"`): `capabilities[]`
(id, commands, optional, status ok|degraded|missing, tools,
setup_topic), `tools[]` (name, status ok|missing|unresponsive|broken|
too_old, path, version, min_version), `solver`, `docker_daemon`,
`project` (manifest_dir, search_stop, stop_dir, wsl_mount),
`required_ok`. JSON metin içermez — dil bağımsız, CI ve LSP için. İnsan
biçimi stdout'tadır (rapor veridir, cli-contract.md §11) ve `--lang`
izler.

### §4 `volt new <ad>` / `volt init`

- `volt new <ad> [--template <t>]` — `<ad>` dizinini oluşturur. Dizin
  varsa ve boş değilse **hata** (çıkış 2).
- `volt init [--template <t>] [--name <ad>]` — çalışma dizinine yazar;
  ad varsayılan olarak dizin adıdır. `Volt.toml` zaten varsa ya da
  yazılacak bir dosya mevcutsa **hata**, çakışan dosyalar listelenir;
  diğer dosyalara dokunulmaz (`.git`, README vb. sorun değil).
- **`--force` YOK.** Üzerine yazma geri alınamaz ve kullanıcının
  dosyasını şablonla değiştirir; çakışmada kullanıcı dosyayı kendisi
  taşır/siler. Hata iletisi hangi dosyaların çakıştığını söyler.
- **Ad doğrulaması:** geçerli Volt tanımlayıcısı (`[A-Za-z_][A-Za-z0-9_]*`
  ve lexer'ın `Ident` saydığı — Volt anahtar sözcüğü değil) ve
  ADR-0078'in tek tablosunda ayrılmış değil (SV, Rust, C/C++). Ad
  paketin adıdır (`[package] name`); `use` yolunda ve üretilen yazılım
  sürücüsünde görünebilir. Tire önerisi: `my-design` → "`my_design`".
- Çıkış kodları: 0 başarı, 2 geçersiz ad/şablon/dolu dizin/çakışma, 3
  yazma hatası.
- Sonrasında `Next:` satırları (mevcut CLI kuralı), stderr'e:
  `cd <ad>`, `volt check <ana>.volt`, `volt test`.

### §5 Şablonlar — az ve doğru

| Şablon | İçerik | Dil yapıları | Kontrat |
|---|---|---|---|
| `minimal` (varsayılan) | sayaç + test | `const`, `fn` | invariant + cover |
| `cdc` | iki saat alanı, ham reset, `sync()` köprüsü (toggle), test | `domain`, `wire`, `sync()` | cover |
| `fifo` | stdlib `SyncFifo` üstünde paket tamponu, test | `struct` + `as u12`/`as Packet`, `SyncFifo` | invariant + cover |
| `mmio` | `@mmio` register haritası + sürücü üretimi, AXI4-Lite testi | `@mmio`/`@reg`, `match` | üretilen kontratlar + cover |

Her şablon üç kipte (bmc, prove, cover; derinlik 20) kanıtlanır; bir
şablon cover'ı erişilemez olsaydı kullanıcının ilk `--mode cover`
denemesi E5001 verirdi.

**Şablonları yazarken çıkan bulgular** (şablonlar etrafından dolaşır,
düzeltme ayrı iş):

1. `SyncFifo<Packet, 8>` — struct öğe tipi E0003 ("struct type as a
   signal type"); şablon `as u12` ile paketler, `as Packet` ile açar.
2. `match` kolunda `const` adı sabitle karşılaştırılmaz, yeni bağlama
   olur (W1002 "shadows"); şablon literal desen kullanır.
3. `wire x : bool @Slow` — alan ek açıklaması W0020 "unknown attribute";
   tel alanı çıkarımla bulunur, şablon açıklamasız yazar.
4. `let x = sync(...)` E0003 (bilinen; yalnız atamanın tüm sağ tarafı);
   şablon `wire` + atama kullanır.
5. Doygun sayaç (`if c != 255 { c <= c + 1 }`) ADR-0066'nın otomatik
   "counter wrap" cover'ını erişilemez kılar → `--mode cover` E5001.
   Şablon sayaç yerine yapışkan bayrak kullanır.

Dörtten fazlası reddedildi: UART/VGA/RISC-V `examples/`'ta zaten var ve
proje başlangıcı değil, örnek; her şablon CI'da dört araçla doğrulanır,
sayı bakım yüküdür. `AsyncFifo` ayrı şablon değil: `fifo` README'si iki
saatli kullanımı `cdc` ile birlikte anlatır.

**Yapı kararı — DÜZ:** `Volt.toml` (`src = "."`), `<ana>.volt`,
`<ana>_test.volt`, `README.md`, `.gitignore` (`build/`) proje kökünde.
`src/` REDDEDİLDİ: `volt test` argümansız yalnız çalışma dizinindeki
`*_test.volt`'u tarar (`sim/test_files.rs`) ve `X_test.volt` kardeşi
`X.volt`'u aynı dizinde arar; `src/` ile "cd <ad>; volt test" hiçbir
test bulamazdı. `examples/` de düzdür (`src = "."`).

Yorumlar İngilizce (ADR-0026 — üretilen/dağıtılan kod). Şablonlar
`templates/<ad>/` altında gerçek dosyalardır ve ikiliye `include_str!`
ile gömülür: kurulum dizininden bağımsız çalışır, repo'daki dosya ile
gömülen bayt bayt aynıdır. `Volt.toml` ve `README.md` içindeki
`{{name}}` tek yer tutucudur; `.volt` dosyaları aynen yazılır (modül
adları sabit), böylece repo'daki şablon dosyaları doğrudan derlenebilir.

### §6 Kalıcı doğrulama

Bayat şablon = kullanıcının ilk deneyimi hata. Her şablon CI'da:

1. `volt new` ile üretilir (tests/`template_tests.rs`); üretilen
   dosyalar gömülü kaynakla bayt aynı.
2. `volt check` + `volt build` temiz (uyarı dahil — şablon uyarı
   üretmemeli), mmio için `--emit=c,rust,regmap`.
3. Çıktı doğrulama ağı: `templates/` ADR-0079 korpusuna eklenir —
   Verilator `-Wall`, Yosys elaborasyonu, C/C++/Rust sürücü derlemesi,
   regmap şeması; işaretsiz (LINT-ALLOW vb. YOK).
4. `volt test` geçer (integration işi, `VOLT_REQUIRE_TOOLS=verilator`).
5. `volt verify` (her şablon kontratlı) geçer (verify işi,
   `VOLT_REQUIRE_TOOLS=sby`).

Bir dil değişikliği şablonu bozarsa bu adımlardan biri düşer.

## Sonuçlar

- (+) Kurulum sonrası tek komut hangi komutların çalışacağını, eksik
  için işletim sistemine göre kurulum satırını söyler.
- (+) Araç keşfi tek yerde; doctor'ın "bulundu" dediğini komut da bulur,
  CI'nın zorunlu kıldığı araç komutun aradığı araçtır.
- (+) Yeni proje 3 komut: `volt new`, `cd`, `volt test`; şablonlar
  güncel dil yapılarıyla en iyi uygulama örneği ve CI'da canlı tutulur.
- (−) Yeni crate (bağımlılıksız, iç).
- (−) Şablon sayısı × dört araç CI süresine eklenir (küçük tasarımlar,
  saniyeler).

## Gelecek İş

1. `volt doctor --fix` (Linux'ta kurulum komutunu önermek yerine
   çalıştırmak) — izin ve paket yöneticisi çeşitliliği nedeniyle yok.
2. LSP'nin `volt-doctor/1` JSON'unu okuyup editörde "Verilator yok"
   bildirimi.
3. Kullanıcı şablon dizini (`--template ./my-template`) — ihtiyaç doğunca.
