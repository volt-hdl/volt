# ADR-0094: Docker Köprüsü — Eksik Verilator/sby'yi Sabitlenmiş İmajda Otomatik Koşturmak

> Statü: Uygulandı
> İlgili: ADR-0084 (araç keşfi tek kaynak `volt-tools`, `volt doctor` — genişletme: yetenek Docker üzerinden), ADR-0093 (hazır ikililer — "indir → çalıştır" hedefinin ikinci yarısı), ADR-0082 (varsayılan çözücü boolector; imajda bitwuzla yok), ADR-0092 (dalga formu oturumu ve basılan yollar), ADR-0055 (tek sby süreci, `-j` içeride).
> Tarih: 2026-09-28
> Etkilenen: `volt-tools` (`docker`, `docker_paths` — YENİ; `VOLT_DOCKER`),
> volt-driver (`tool_backend.rs` YENİ, `sim/verilator.rs`, `sim/run_cmd.rs`,
> `sim/test_cmd.rs`, `verify.rs`, `verify_jobs.rs`, `doctor/`),
> volt-diagnostics (`simulation-setup`, `verify-setup` metinleri), testler
> (`docker_backend_tests`, `docker_e2e_tests`), CI (`docker-backend` işi),
> cli-contract.md §2, §8b (YENİ), §9a, §10; README "External tools",
> examples/README, CHANGELOG

## Sorun

ADR-0093 ile Volt tek dosya olarak indirilebiliyor; `build`, `check`,
`explain` hiçbir şey istemiyor. Ama `volt test` / `volt run` Verilator'u
(+ C++ derleyicisi, make), `volt verify` sby'yi (+ Yosys, çözücü) istiyor
ve Windows'ta bunların yerel ikilisi dağıtılmıyor. Bugün tek yol Docker
komutlarını elle yazmak: `examples/README.md` Verilator imajında cargo
kurup Volt'u konteynerde derletiyordu; formal için elle yazılmış bir
`build/sby-docker.cmd` sarmalayıcısı `VOLT_SBY`'ye veriliyordu. Hedef:
"indir → `volt new` → `volt test`", Docker Desktop dışında kurulum yok.

## 1. Tespit (ölçüm, 2026-09-28)

**a) Bugünkü Docker desteği.** Volt'ta otomatik Docker yolu YOK. Docker
yalnız dört yerde geçiyor: `volt doctor` daemon'u yokluyor ve "isteğe
bağlı" diye raporluyor; `explain` metinleri `docker pull` öneriyor;
ADR-0082'nin "sby Docker içinde koşunca" notu `VOLT_SBY`'nin elle yazılmış
bir sarmalayıcıyı göstermesi demek (`build/sby-docker.cmd`:
`docker run --rm -v "%CD%:/work" -w /work hdlc/formal sby %*`, deponun
izlenmeyen `build/` dizininde); `examples/README.md` elle tarifler.
Tetikleme tamamen kullanıcıda.

**b) İmajlar ve CI farkı.** CI Verilator/Yosys/sby'yi OSS CAD Suite
2026-09-21 tarballından alır (ADR-0079, ADR-0062). Genel erişimli OSS CAD
Suite imajı yok (`ghcr.io/yosyshq/oss-cad-suite`, `yosyshq/oss-cad-suite`:
erişim reddedildi / yok). Ölçülen sürümler:

| Kaynak | Verilator | Yosys | SBY | boolector | bitwuzla | yices | z3 |
|---|---|---|---|---|---|---|---|
| CI: OSS CAD Suite 2026-09-21 (`volt-eng`) | 5.053-devel (v5.052-159) | 0.69+77 | 0.69 | 3.2.4 | 0.9.1 | var | var |
| `verilator/verilator:v5.052` | **5.052** | — | — | — | — | — | — |
| `verilator/verilator:latest` (belgedeki tarif) | 5.050 (etiket kayar) | — | — | — | — | — | — |
| `hdlc/formal:all` | — | **0.66** | **0.69** | 3.2.4 | **yok** | 2.7.0 | 4.15.0 |
| `hdlc/formal:latest` (belgedeki tarif) | — | 0.36+42 | 0.68 | 3.2.4 | yok | 2.7.0 | 4.13.1 |

`hdlc/formal:latest` (ADR-0082'nin tablosundaki ve `sby-docker.cmd`'nin
imajı) Yosys 0.36'da kalmış: CI'dan 33 sürüm geride. `:all` etiketi aynı
ailenin güncel yapısı (Yosys 0.66; `ghcr.io/hdl/formal/all` ile aynı
içerik özeti). Verilator'da CI'nin geliştirme anlık görüntüsüne en yakın
sürümlü etiket v5.052.

**c) Geçmiş sorunlar.**

- *Kök sahipliği:* `docker run` varsayılan olarak root koşar; Linux'ta
  bağlanan dizine yazılan her dosya root'a ait olur (kullanıcı `build/`'i
  silemez). Bu depoda kayıtlı olay yok, çünkü geliştirme Windows'ta Docker
  Desktop'la yapıldı (orada bağlı dosyalar kullanıcıya yazılır) — sorun
  Linux'ta her elle tarifin gizli kusuru.
- *Windows yol çevirisi:* Git Bash `/work`'ü `C:/Program Files/Git/work`
  yapar (`MSYS_NO_PATHCONV=1` tarifleri); kısa `ALAR~1` yolları Docker
  Desktop'ta bağlanamıyor (bellek notu). Volt `docker`'ı doğrudan başlatır
  (MSYS yok), bağlanacak yolu kanonikleştirir (kısa ad açılır, `\\?\`
  soyulur).
- *`/work` sızıntısı:* ADR-0083 ölçüm kayıtlarında Verilator iletileri
  `/work/LBi/Alu.sv:38:22` gibi konteyner yollarıyla; ADR-0092 dalga formu
  oturumuna göreli yol yazdı, çünkü mutlak `/work/...` ana makinede yoktu.

## 2. Kararlar

### 2.1 Tetikleme: otomatik, tek satır bildirimle; `VOLT_TOOL_BACKEND` ile seçilebilir

Sıra: yerel araç (`volt-tools` araması, DEĞİŞMEDİ: `VOLT_VERILATOR` /
`VOLT_SBY` → PATH) → bulunamazsa çalışan Docker daemon'u → sabitlenmiş
imaj. `VOLT_TOOL_BACKEND=auto|local|docker` (varsayılan `auto`):
`local` Docker'ı hiç çağırmaz, `docker` yerel araç kurulu olsa da
konteyneri seçer, başka değer kullanım hatasıdır (çıkış 2).

Gerekçe:

- UX Anayasası İ4 "tek komut yeterli, kurulum/yapılandırma/ön adım yok".
  Açık seçim (bayrak, `Volt.toml`, ortam değişkeni) Windows kullanıcısına
  ikinci bir adım öğretir; üstelik kullanıcının yapabileceği tek şey
  Docker'ı açmaktır — seçim hiçbir bilgi eklemez.
- Yerel araç her zaman kazanır: aracı kurulu olan için davranış ve çıktı
  bayt bayt aynıdır (bu modül yerel yolda hiçbir şey basmaz). Otomatik
  geçiş yalnız, eskiden "araç yok, çıkış 3" olan durumu çalışır kılar.
- Sessiz geçiş YOK: Docker'a geçildiği her komut stderr'e bir satır basar
  (`note: Verilator not found locally; running it in Docker
  (verilator/verilator:v5.052)`; zorlanmışsa `note: VOLT_TOOL_BACKEND=docker;
  ...`). Komut başına bir kez (`volt test`'in bütün grupları için bir).
- Neden bayrak değil ortam değişkeni: seçim komut değil makine/CI
  ortamı özelliğidir (üç komutta aynı), `VOLT_VERILATOR`/`VOLT_SBY` ile
  aynı katmanda. `Volt.toml` reddedildi: proje dosyası makineye bağlı
  olmamalı (aynı proje Linux CI'da yerel, Windows'ta Docker koşar).
- CI kaygısı (ADR-0079 §3 — kurulum bozulursa sessizce yeşil kalmasın):
  araç bağımlı testler `VOLT_REQUIRE_TOOLS` ile aracı Volt'tan ÖNCE arar
  ve düşer; Docker geçişi bu kapıyı delmez. Kesinlik isteyen iş
  `VOLT_TOOL_BACKEND=local` koyar.

### 2.2 İmajlar: sürüm + özetle sabit, genel erişimli

| Komut | Başvuru | İçerik | İndirme (sıkıştırılmış, amd64) |
|---|---|---|---|
| test, run | `verilator/verilator:v5.052@sha256:a5b73e2f…7352120` (çok platformlu indeks: amd64 + arm64) | Verilator 5.052, g++ 13.3, make 4.3, Ubuntu 24.04 | 249 MB |
| verify | `hdlc/formal:all@sha256:d0852c89…f80f75c1bb3` (yalnız amd64) | Yosys 0.66, SBY 0.69, boolector 3.2.4, yices 2.7.0, z3 4.15.0 | 404 MB |

Özet etiketi kilitler: etiket taşınsa da aynı içerik çekilir; imaj
değişikliği bir kod değişikliğidir (`volt_tools::docker::{SIMULATION_IMAGE,
FORMAL_IMAGE}`, `contents` alanı ölçülen sürümleri `volt doctor`'a verir).

CI farkı (raporlanan, kabul edilen): Verilator 5.052 ↔ 5.053-devel (aynı
seri, 159 commit); Yosys 0.66 ↔ 0.69, SBY aynı (0.69); **bitwuzla yok** —
`--engine bitwuzla` Docker'da ADR-0082'nin mevcut iletisiyle ("SMT Solver
'bitwuzla' not found … install it, or pick an installed solver with
--engine") biter; `volt doctor` bunu satırda söyler. Varsayılan çözücü
boolector imajda var.

**Öneri (bu ADR'de YAPILMADI):** CI'ın kullandığı OSS CAD Suite tarballından
tek bir `ghcr.io/volt-hdl/tools:<tarih>` imajı yayımlamak (Verilator +
Yosys + sby + dört çözücü; CI ile birebir, bitwuzla dahil, amd64 + arm64).
Yayımlanınca yalnız iki sabit değişir; CI da aynı imajı kullanabilir.
Yayım bir sürüm/erişim kararıdır (ghcr ad alanı, bakım sorumluluğu).

### 2.3 Ne konteynerde koşar, yollar

Volt ana makinede kalır: derleme, SV/testbench/`.sby` yazımı, sonuç yorumu,
GTKWave oturumu, karşı örnek kopyası. Konteynerde YALNIZ araç koşar:

- `test`/`run`: grup başına iki kısa `docker run` — Verilator derlemesi
  (imajın giriş noktası, aynı argümanlar) ve üretilen simülasyon
  (`--entrypoint <yürütülebilir>`). Girdiler sim dizinine göreli olduğundan
  argümanlar yereldekiyle aynıdır.
- `verify`: tek `docker run … sby -j N -f <iş>.sby` (ADR-0055'in tek
  süreci; `-j` konteynerin içinde).

Bağlanan dizinler yalnız aracın çalışma dizinidir (`build/sim/<ad>/`,
`build/formal/`; `run --vcd` için VCD'nin dizini de). Eşleme belirlenimci
ve durumsuzdur (`volt_tools::docker_paths`):

- **Unix:** konteyner yolu = ana makine yolu. Aracın bastığı her yol zaten
  ana makine yoludur; çeviri gerekmez.
- **Windows:** `C:\Dev\demo\build\sim\x` → `/volt/c/Dev/demo/build/sim/x`.
  Aracın stdout/stderr'i basılmadan (ve sby satırları yorumlanmadan) önce
  bağlı öneklerin tersine çevrilir (`Mounts::host_text`: bileşen sınırında,
  en uzun önek önce, boşluklu yol doğru; kuyruk `\` ayraçlı).

Testbench'e gömülen VCD yolu Docker'da konteyner yoludur (dizini bağlı);
kullanıcıya basılan `Waveform` satırı ve oturum dosyası ana makinede,
eskisi gibi göreli yazılır (ADR-0092). Yeniden koşturma ipucu Docker'da
günlük dosyasını gösterir (`build/sim/<ad>/verilator.log` — Docker'da tam
günlük yazılır; `build/formal/<iş>_<görev>/logfile.txt`), çünkü tarif
edilen yerel komut o makinede yoktur. sby'nin konteynerde yazdığı
günlükler (`logfile.txt`, JUnit `.xml`) koşu bitince aynı çeviriden
geçirilir — ilk Windows koşusunda yakalanan sızıntı (bu dosyalarda
`/volt/c/...` kalıyordu). Konteyner ağsızdır (`--network
none`), `--init` sinyal/zombi yönetimi içindir, `HOME=/tmp`.

### 2.4 Dosya sahipliği

Linux'ta konteyner `--user <uid>:<gid>` ile koşar; kimlik, Volt'un az önce
oluşturduğu çıktı dizininin sahibidir (`MetadataExt`, `unsafe`/libc yok —
`getuid` gerekmez). Üretilen her dosya kullanıcıya aittir. Windows/macOS
Docker Desktop bağlı dosyaları zaten kullanıcıya yazar; bayrak yalnız
`cfg(unix)`.

### 2.5 İlk kullanım

`docker image inspect <başvuru>` yoksa önce boyut, sonra süre:

```
note: downloading verilator/verilator:v5.052 (~250 MB, first use only; this can take several minutes)
note: downloaded verilator/verilator:v5.052 in 8m 37s
```

İndirme `docker pull -q` (katman satırları basılmaz). Başarısızsa Docker'ın
son satırı + yeniden deneme komutu, çıkış 3.

### 2.6 `volt doctor`

Aynı karar (`tool_backend::resolve` ile özdeş koşullar): birincil araç
yerelde yok ya da `docker` zorlanmış, docker var, daemon çalışıyor →
yetenek TAMDIR ve satır yolu söyler:

```
✓ test, run — via Docker (verilator/verilator:v5.052: Verilator 5.052, g++ 13.3)
✓ verify — via Docker (hdlc/formal:all: Yosys 0.66, SBY 0.69, boolector 3.2.4, yices 2.7.0, z3 4.15.0)
    (bitwuzla is not in this image; --engine boolector|yices|z3, ADR-0082)
```

İmaj yoksa alt satır `image not downloaded yet: ~250 MB on first use`.
JSON'a (`volt-doctor/1`, eklemeli) `backend`, `image`, `image_present`.
`--strict` Docker üzerinden tam yeteneği tam sayar.

### 2.7 Kaynak sınırı

Aynı anda tek konteyner (CLAUDE.md bellek kuralı): `test` grupları ve
her grubun iki adımı sırayla; `verify -j` tek konteynerin içinde.
`--fail-fast` sby'yi keserken `docker run` istemcisini öldürmek konteyneri
durdurmaz — konteyner adlıdır (`volt-sby-<pid>-<ms>`) ve `docker rm -f`
ile kaldırılır.

### 2.8 Hata durumları (hepsi çıkış 3, geçersiz değer 2)

| Durum | Davranış |
|---|---|
| docker yok (`auto`) | eski "not found" kurulum yardımı; Windows satırı artık "install Docker Desktop (Volt then runs … in a container) or use WSL" |
| docker yok (`docker`) | `error: VOLT_TOOL_BACKEND=docker, but docker was not found` |
| daemon kapalı / 10 sn yanıtsız | `error: Verilator not found locally, and Docker is installed but its daemon is not running` + `= help: start Docker Desktop …` |
| imaj indirilemedi | `error: could not download the Docker image …` + neden + `docker pull` |
| bellek (137) | `error: the Docker container ran out of memory (exit code 137)` + Docker Desktop bellek ayarı / `-j` |
| konteyner başlamadı (125) | `error: docker could not start the container (exit code 125; …)` |
| bağlanamayan yol (UNC) | `error: cannot share '<yol>' with the Docker container: …` |

## 3. Doğrulama

**Windows (2026-09-28, Docker Desktop 29.7.2, PATH'te Verilator/sby YOK):**
yerel `cargo build --profile dist` ikilisi (sürüm arşiviyle aynı profil)
depo dışında boş bir klasöre kopyalandı; PATH yalnız o klasör + Docker +
System32:

- `volt doctor` → `✓ test, run — via Docker (…)`, `✓ verify — via Docker
  (…)` + `image not downloaded yet: ~404 MB on first use`.
- `volt new demo --template cdc`; `volt test` → tek bilgi satırı, 3/3 ok,
  cover özeti, çıkış 0, 9,9 sn (imaj hazır). Verilator imajının ilk
  indirmesi aynı senaryonun debug ikiliyle önceki koşusunda:
  `downloaded verilator/verilator:v5.052 in 8m 37s`.
- `volt verify event_counter.volt` (formal imajı önceden silindi) →
  `downloading hdlc/formal:all (~404 MB …)`, `downloaded hdlc/formal:all
  in 12m 03s`, 1/1 ok, çıkış 0; imaj varken 2,8 sn. (Bu bağlantı ~0,5
  MB/sn; CI runner'ında aynı indirmeler 10,3 sn ve 15,3 sn.)
- `examples/uart_tx.volt`: `volt run --cycles 60 --vcd waves/uart.vcd` →
  `Waveform gtkwave waves/uart.vcd waves/uart.gtkw`; iki dosya ana
  makinede, oturum `[dumpfile] "waves/uart.vcd"`.
- Bozulmuş kontrat (`bit_count_r <= 3`): E5001 "violated at cycle 23",
  `counterexample: build\formal\uarttx_cex.vcd`, `gtkwave
  build\formal\uarttx_cex.vcd build\formal\uarttx_cex.gtkw`; çıkış 6.
- Hiçbir çıktıda `/volt/` yok.
- Negatif (gerçek docker CLI, `DOCKER_HOST=tcp://127.0.0.1:1`): `volt test`
  ve `volt verify` 0,3 sn'de daemon iletisi, çıkış 3; `volt doctor` eski ✗
  satırları + `! docker — … daemon not running`.

**Sahte `docker` testleri** (`docker_backend_tests`, 15 test, her
platformda): tek bilgi satırı, bağlanan tek dizin ve `-w`, Verilator
argümanlarının yereldekiyle aynılığı, `--entrypoint` yürütülebilir, Linux'ta
`--user` = sim dizininin sahibi, konteyner yolu → ana makine yolu (tanı
ve `verilator.log`), daemon kapalı, `local` hiç docker çağırmaz, geçersiz
değer 2, zorlanmış `docker`, indirme boyutu→süre sırası, indirme hatası,
137, `run --vcd` (tb'deki konteyner yolu, bağlı VCD dizini), `verify` tek
adlı konteyner `sby -j 4`, Docker'da araç hatası günlük ipucu, doctor
(insan + JSON, imaj yokken boyut, `local`).

**Linux (CI `docker-backend` işi, PR #65 ilk koşu yeşil, 1 dk 36 sn):** araçsız runner'da
`docker_e2e_tests` (`VOLT_DOCKER_E2E=1`) gerçek imajlarla `new → test →
verify` ve uart_tx `run --vcd`; üretilen her dosyanın sahibi runner
kullanıcısı (test içinde ve ayrıca `find ! -user` adımı).

**Mutasyon (tek tek, sonra geri alındı):**

| Mutasyon | Yakalayan |
|---|---|
| `Mounts::host_text` kimlik | 2 birim testi + `verilator_errors_are_reported_with_host_paths` (Windows) |
| `--user` eklenmez (`volt-tools`) | `run_arguments_are_stable_and_carry_user_name_and_entrypoint` |
| `owner_of` sonucu atılır (sürücü) | Linux'ta `test_in_docker_runs_as_the_owner_of_the_output_dir` (volt-eng konteynerinde koşuldu; mutasyonsuz 16/16) |
| bilgi satırı basılmaz | 4 entegrasyon testi |
| sby günlük çevirisi kapalı | `verify_runs_sby_once_in_one_named_container` |

**Golden:** yerel araç bulunduğunda `tool_backend` hiçbir şey basmaz ve
`Command` yereldekiyle aynı kurulur (Verilator argümanları `args()`'tan,
sıra testi `command_puts_vlt_before_sv_and_keeps_argument_order`
değişmeden geçer); sahte `VOLT_VERILATOR`/`VOLT_SBY` ile koşan bütün
mevcut testler (sim_golden, verify_status, verify_parallel, extern,
contracts) değişmeden geçer.

## Sınırlar

- `hdlc/formal:all` yalnız amd64: Apple silicon'da emülasyonla (yavaş)
  koşar. Verilator imajı arm64'ü de taşır.
- bitwuzla Docker'da yok (§2.2 önerisi kapatır).
- Kullanıcının Ctrl-C'si `--init` ile konteynere iletilir; yine de Windows'ta
  öldürülen `docker.exe` konteyneri geride bırakabilir (`--fail-fast` yolu
  adlı konteyneri kaldırır, Ctrl-C yolu kaldırmaz). `--rm` konteyner
  bittiğinde temizler.
- Docker Desktop'ta bağlı dizin G/Ç'si yereldekinden yavaştır; Verilator
  derlemesi grup başına birkaç saniye uzar.
- Windows ağ paylaşımı (UNC) yolları bağlanamaz (açık hata).
- Podman/`nerdctl` sınanmadı (`VOLT_DOCKER` ile uyumlu bir CLI gösterilebilir).
- `volt lint`/Yosys sentez tarifleri (`examples/README.md` lint bölümü)
  Volt komutu olmadığından elle Docker tarifi olarak kaldı.
