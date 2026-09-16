# ADR-0055: Paralel Formal Doğrulama — `volt verify -j`, Modül Başına sby Görevi, Kaynak Sıralı Rapor

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-16
> Etkilenen: volt-sv-emit (`sby.rs`: `SbyTask`, `sby_config_tasks`;
> `lib.rs` dışa aktarım), volt-driver (`verify.rs` yeniden düzenlendi;
> `verify_jobs.rs` YENİ: görev koşucusu, `-j` ayrıştırma, `--fail-fast`;
> `verify_report.rs` YENİ: ilerleme/özet/JSON; `main.rs`: `-j/--jobs`,
> `--fail-fast`, `json_envelope`), docs/spec/cli-contract.md §3/§8a/§16
> (bu ADR kaynaklı), .github/workflows/ci.yml (`verify` işine SoC `-j 4`
> adımı), testler: crates/volt-driver/tests/verify_parallel_tests.rs
> (YENİ, 21 test), cli_tests.rs sahte sby güncellemesi, birim testleri.
> DOKUNULMADI: README.md, examples/.

## Sorun

`volt verify` kontratlı her modül için ayrı bir `.sby` yazıp `sby -f`
çağrısını SIRAYLA yapıyordu. `examples/soc` (8 modül, 137 kontrat) için
bu 21,1 s sürüyordu (bmc 12) ve `cover 48` kipinde dakikalara çıkıyordu.
Çok çekirdekli makinede işin büyük kısmı boşta duran çekirdeklerle
yapılıyordu; Windows'ta sby Docker sarmalayıcısıyla (`build/sby-docker.cmd`)
koştuğundan her modül ayrıca ~1,3 s konteyner başlatma bedeli ödüyordu.

Goal iki strateji sundu: (A) property başına ayrı sby süreci, volt'un
kendi iş havuzu; (B) sby'nin kendi görev paralelliği (`[tasks]` +
`sby -j N`). "B mümkünse tercih edilsin" dendi; önce ölçüldü.

### Durum tespiti (bu ADR'nin 1. adımı)

| Soru | Cevap |
|---|---|
| sby çağrısı | Modül başına bir `sby -f <modül>.sby`, sırayla; her `.sby` aynı SV'nin ayrı kopyasını okur |
| `.sby` üretimi | `volt_sv_emit::sby_config(top, sv, opts)` — `[options]/[engines]/[script]/[files]` |
| Süre nerede | SoC bmc 12: sby toplamı ≈ 10 s (SocTop tek başına 9 s, diğer 7 modül ≈ 1 s), 8 Docker başlatması ≈ 9–10 s |
| hdlc/formal sby | `-j <N>`, `--sequential`, `[tasks]`, görev-koşullu satırlar (`ad: satır`), `--live`, `yosys-smtbmc --keep-going` VAR |
| `read -formal` | Modülleri `$abstract\` olarak saklar; seçim/`chformal` ancak `prep -top` SONRASI çalışır |

Property başına ölçüm, elle üretilen 62 görevli `.sby` ile (bmc kipinde
yalnız `assert` türü kontratlar denetlenir: 62; `assume` 54 ve `cover`
30 görev olmaz), izolasyon `prep -top M; select -set keep a:src=…;
chformal -assert -cover -remove … @keep %d` ile:

| Birim (sby-only, tek konteyner, bmc 12) | -j 1 | -j 4 | -j 8 | -j 16 |
|---|---|---|---|---|
| Modül (8 görev) | 15,5 s | 13,7 s | 13,4 s | — |
| Property (62 görev) | 46,0 s | 17,5 s | 14,2 s | 14,2 s |

Belirleyici bulgu: **tek bir SocTop property'sinin görevi 9–10 s sürüyor,
yani SocTop'un 137 hiyerarşik kontratını birlikte denetleyen görevle
aynı.** BMC maliyeti tasarımın `depth` kadar açılmasıdır; SMT sorgusuna
ek bir assertion neredeyse bedava. Property başına bölme toplam işi
~3× büyütüyor (`-j 1`: 46 s ↔ 15,5 s), duvar süresini kısaltmıyor (`-j 8`:
14,2 s ↔ 13,4 s) ve CI'nın 2–4 çekirdeğinde ciddi gerileme olurdu.
Ayrıca `prove` (k-tümevarım) kipinde diğer assertion'lar tümevarım
hipotezini GÜÇLENDİRİR; tek tek kanıtlamak bugün geçen property'leri
düşürebilirdi. Property başına bölme bu nedenle REDDEDİLDİ.

## Karar

### 1. B seçeneği, birim = modül

Birimdeki kontratlı modüller kaynak sırasında tek `build/formal/<iş>.sby`
dosyasına `[tasks]` olarak yazılır (`<iş>` = girdi dosyasının
`[A-Za-z0-9_]`'ye indirgenmiş kök adı, `top.volt` → `top`; görev = küçük
harf modül adı, çakışırsa `_2`). Ortak satırlar önekisiz, görevi ayıran
satırlar görev-koşullu:

```
[tasks]
soctop
gpio

[options]
mode bmc
depth 12
fifobridge: multiclock on        # yalnız iki+ saatli modülde (ADR-0027)

[engines]
smtbmc z3

[script]
read -formal top.sv
soctop: prep -top SocTop
gpio: prep -top Gpio

[files]
top.sv
```

Tek `sby -j N -f <iş>.sby` süreci koşar; sby görev döngüsü işleri
dağıtır, `-j` sby'ye olduğu gibi geçer. Çalışma dizini `<iş>_<görev>/`,
karşı örnek `<görev>_cex.vcd`. Tek görevi yinelemek: `sby -f <iş>.sby
<görev>`. B'nin goal'deki "netlist bir kez okunur" gerekçesi DOĞRU DEĞİL
(her görev kendi Yosys sürecinde SV'yi yeniden okur); gerçek kazanımı tek
süreç/tek konteyner ve volt tarafında iş havuzu kodunun olmamasıdır.

### 2. CLI (cli-contract.md §3, §8a)

`-j, --jobs <N|auto>` (varsayılan `auto` = `available_parallelism`, `1`
= sıralı), `--fail-fast`. `0` ya da sayı/`auto` dışı değer clap üzerinden
kullanım hatasıdır (çıkış 2). `-j` §3'te zaten genel bayrak olarak
tanımlıydı; bugün yalnız `verify` uygular.

### 3. İlerleme gösterimi

sby stdout/stderr'i iki ayrık iş parçacığıyla satır satır kanala
kopyalanır; her satır ilk `[<çalışma dizini>]` etiketinden görevine
ayrılır (`verify_jobs::task_index_of`). `DONE (` satırı geldiğinde
görevin kendi log'u `interpret_sby_output` ile yorumlanır ve
`[ k/N] <Modül> (<n> properties) ... ok|FAIL|error (<süre>)` basılır —
`k` tamamlanan sayısı, süre görevin ilk `starting process` satırından
DONE'a kadar (sby'nin tam-saniye özeti yerine). Özet: `137 properties
verified in 12.3s (8 jobs; bmc, depth 12)`.

### 4. Hata durumu

Varsayılan: her görev tamamlanır, tüm başarısızlıklar sonda kaynak
sırasında `Failures:` altında listelenir (`Beta.inv_0  E5001 contract
violated at cycle 7`), çıkış 6. `--fail-fast`: ilk `DONE (FAIL)`'de sby
süreci `kill` edilir, kalan görevler `skipped`. DONE satırı olmayan
görev araç hatasıdır (çıkış 3; karşı örnek varsa 6 baskın), yeniden
koşturma ipucu görevi seçer; göreve ait olmayan sby satırları (yapılandırma
hatası) en fazla 20 satır gösterilir.

### 5. Determinizm

Sonuç dizisi `run_sby_tasks` içinde girdi indeksine yazılır; rapor
(E5001 tanıları, `Failures:`, JSON `modules`/`properties`) bu diziyi
kaynak sırasında yürür. Tamamlanma sırası yalnız ilerleme satırlarına
yansır. Testler: sahte sby karışık sırada (Gamma, Alpha, Beta) bitirir;
`-j 1` ve `-j 4` JSON'ları süre alanları maskelenince birebir aynı; iki
ardışık koşu aynı; gerçek sby ile `examples/soc` `-j 1/4/8` JSON'ları
birebir aynı (137 `pass`).

### 6. JSON

Zarfa `verify` nesnesi eklenir (`json_envelope` ayrıştırıldı):
`mode, depth, engine, jobs, fail_fast, modules[{module, task, status,
properties, duration_ms}], properties[{module, name, keyword, status,
duration_ms}]`. Kontrat durumu `pass | fail | unproven | error |
skipped`; `unproven` = aynı modülde başka ihlal, BMC o döngüde durdu.
`duration_ms` kontratın GÖREVİNİN süresidir (kontratlar birlikte
kanıtlanır) — belgede açıkça söylenir.

## Ölçüm (examples/soc, 137 kontrat, Docker hdlc/formal, 16 mantıksal çekirdek)

Uçtan uca `volt verify --mode bmc --depth 12` (derleme + Docker + sby):

| Akış | Süre | Hızlanma |
|---|---|---|
| Eski: modül başına `sby -f`, sırayla, 8 Docker başlatması | 21,1 s | 1,0× |
| Yeni `-j 1` (tek süreç, görevler sırayla) | 14,4 s | 1,47× |
| Yeni `-j 4` | 12,9 s | 1,64× |
| Yeni `-j 8` | 13,1 s | 1,61× |

`cover --depth 48` (yalnız sby, modül görevleri): `-j 1` 439 s, `-j 8`
376 s (1,17×); SocTop görevi 375 s ile kritik yol, UartCtrl 177 s.

Yorum: bu tasarımda duvar süresi SocTop'un TEK BMC koşusuyla sınırlı
(bmc 12'de ~9–11 s, cover 48'de ~6 dk); paralellik yalnız diğer 7 modülü
ve Windows'ta Docker başlatmalarını gizler. `-j 4` ile `-j 8` arasındaki
fark ölçüm gürültüsü içindedir. Modül sayısı ve dengesi arttıkça kazanım
büyür; tek dev modülde kazanım yoktur — bu, algoritmik (BMC) sınırdır,
orkestrasyon sınırı değil.

## Sınırlar

- Property başına süre/ilerleme YOK — kontrat ölçeğinde bölme reddedildi
  (yukarıdaki ölçüm). "En yavaş property" sorusunun cevabı modül
  ölçeğindedir (`modules[].duration_ms`).
- Bir modülde ilk ihlalde BMC durur; aynı modülün diğer kontratları
  `unproven`. hdlc/formal'daki `yosys-smtbmc --keep-going` denemede
  çalıştı (iki ayrı `Assert failed`, `trace0/trace1.vcd`); çoklu E5001 +
  iz eşlemesi bu ADR'nin dışında bırakıldı.
- `--fail-fast` sby'yi `kill` eder; sby'nin alt süreçleri (yosys/smtbmc)
  ve Windows'ta Docker konteyneri kendi adımlarını bitirene kadar
  yaşayabilir (sınırlı, `build/formal/` dışına yazmaz).
- `-j`'nin anlamı sby'nin süreç bütçesidir (görev sayısı değil); sby bir
  görevde aynı anda en fazla bir-iki süreç koşturduğundan pratikte
  eş zamanlı görev sayısına yakındır.
- sby `-j`/`[tasks]` gerektirir (hdlc/formal ve OSS CAD Suite'te var);
  daha eski sby'de `-j` bilinmez → araç hatası (çıkış 3) ve sby çıktısı
  gösterilir.
- Alt modüllerin kontratları üst modülün görevinde de denetlenir (bugünkü
  davranış korunur); kontrat sayımı ve `properties[]` yine kontrat başına
  tek satırdır.

## Ölçütler

- `volt verify -j 8 --depth 12 examples/soc/top.volt` (Docker) → 137
  `pass`, 8 görev, `-j 1` ile birebir aynı JSON (süreler hariç), 21,1 s →
  ~13 s.
- `cargo test --all` yeşil; +38 test (sv-emit 4, verify_jobs 5,
  verify_report 5, verify 3, verify_parallel_tests 21); baseline 1879 →
  1917.
- `just check`, `just consistency`, `just clippy-strict` temiz; CI `verify`
  işi SoC'yi `-j 4` ile koşturur.
