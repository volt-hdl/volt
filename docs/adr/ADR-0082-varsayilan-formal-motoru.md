# ADR-0082: Varsayılan Formal Motoru — boolector; bitwuzla Seçeneği, Portföy ve Otomatik Seçim Reddedildi

> Statü: Uygulandı
> İlgili: ADR-0094 (Docker köprüsü: `hdlc/formal:all` imajında boolector var, bitwuzla yok).
> Tarih: 2026-09-26
> Etkilenen: volt-sv-emit (`sby.rs` — `SbyEngine` varsayılanı `Boolector`,
> yeni `Bitwuzla`), volt-driver (`main.rs` `--engine` varsayılanı ve
> `bitwuzla` değeri; `verify.rs` eksik çözücü yardımı, zaman aşımı ve
> kurulum önerisi), volt-diagnostics (`explain verify-setup` çözücüler
> bölümü), cli-contract.md §8a, ADR-0055 (paralel görevler — değişmedi)

## Sorun

`volt verify` bugüne dek `smtbmc z3` ile koşuyordu (F4b varsayılanı).
Üç kez aynı şey görüldü: RV32IM keşfinde z3 22 dakika takıldı; ADR-0081
Aşama 3'te `riscv_core` `bmc --depth 10` varsayılan motorla 21 dakikada
bitmedi, boolector ile 11 saniye sürdü; kullanıcı her seferinde
`--engine boolector` yazmak zorunda. Zaman aşımı yardımı bile "boolector
çoğu zaman daha hızlı" diyordu.

## Ölçüm

**Düzen:** CI'nin sabitlediği araç zinciri — OSS CAD Suite 2026-09-21
(Yosys 0.69+77, sby, z3, boolector, yices, bitwuzla) — `volt-eng` Docker
imajında; volt release ikilisi. `examples/` (test dosyaları hariç) ve
`tests/ui/pass/` içindeki her tasarım `volt verify --mode {bmc,prove,cover}
--depth 20 --timeout 120 -j 4` ile; 4 tasarım paralel (16 mantıksal
çekirdek). Kontratsız tasarımlar atlandı: **62 kontratlı tasarım × 3 kip
× 4 çözücü = 744 koşu**. Motor, üretilen `.sby`'nin `[engines]` satırı
değiştirilerek seçildi (`build/eng/sby_eng.py`). Araçlar:
`build/eng/measure.py`, `analyze.py`, `analyze_props.py`, `pf_probe.sh`.

### Toplam süre ve zaman aşımı (120 sn sınır; TO = zaman aşımı)

| Kip | z3 | boolector | yices | bitwuzla |
|---|---|---|---|---|
| bmc toplam / TO | 1026 s / 6 | 435 s / 2 | 502 s / 3 | **330 s / 1** |
| prove toplam / TO | 1028 s / 6 | 529 s / 2 | 523 s / 3 | **383 s / 1** |
| cover toplam / TO | 550 s / 3 | **133 s / 0** | 357 s / 1 | 149 s / 0 |
| en hızlı olduğu tasarım (bmc/prove/cover) | 3 / 6 / 14 | 22 / 12 / 25 | 33 / 33 / 21 | 14 / 19 / 14 |

Medyan her çözücüde 0,4–0,6 s: tasarımların çoğu küçüktür ve orada
çözücü fark etmez (yices küçüklerde en sık birinci). Fark büyük
tasarımlarda açılır:

| Tasarım (kip) | z3 | boolector | yices | bitwuzla |
|---|---|---|---|---|
| `riscv_pipeline` (bmc) | TO | 98,7 | TO | **6,4** |
| `riscv_pipeline` (prove) | TO | 107,3 | TO | **5,5** |
| `riscv_core` (prove) | TO | TO | TO | **98,1** |
| `riscv_core` (cover) | TO | **4,0** | TO | 6,6 |
| `riscv_sw/hello_soc` (bmc) | TO | TO | TO | **113,6** |
| `riscv_sw/hello_soc` (prove) | TO | 107,5 | TO | **87,5** |
| `hybrid_accel/ternary_array` (bmc) | TO | **8,2** | 35,9 | 9,5 |
| `hybrid_accel/hybrid_top` (prove) | TO | TO | **72,5** | TO |
| `soc/top` (bmc, 8 görev) | 63,9 | **8,4** | 9,1 | 8,9 |
| `tests/ui/pass/31_ram` (bmc) | 86,5 | **0,4** | 0,4 | 0,5 |
| `tests/ui/pass/46_pipeline_forwarding` (bmc) | TO | 0,7 | **0,4** | 0,5 |
| `riscv_core` (bmc d20) | TO | TO | TO | TO |

**Sorunun kendisi — `riscv_core` `bmc --depth 10`, 600 sn sınır, tek
tasarım sırayla:**

| | z3 | boolector | yices | bitwuzla |
|---|---|---|---|---|
| duvar süresi | **600 s zaman aşımı** (RiscvCore TIMEOUT) | **9,3 s** | 600 s zaman aşımı | **5,3 s** |
| sonuç | UartTx PASS, RiscvCore sonuç yok | 2/2 görev PASS | UartTx PASS, RiscvCore sonuç yok | 2/2 görev PASS |

Aşama 3'teki "21 dakikada bitmedi / boolector 11 s" gözlemi aynı araç
zincirinde yeniden üretildi.

### Sonuçlar değişmedi mi? (ADIM 3)

İmza, volt'un kullanıcıya gösterdiği sonuçtur: çıkış kodu, **her
kontratın** durumu (`pass`/`fail`/`unknown`/`unproven`), FAIL'de karşı
örnek adımı, cover'da ulaşılan adımlar. Karşılaştırma yalnız zaman aşımına
uğramayan koşular arasında (zaman aşımı sonuç değil, sonuç yokluğudur).
Başarısız, kanıtlanamayan ya da erişilemeyen sonucu olan 23 tasarım ve
`tests/ui/fail/` + `tests/fixtures/` (CI'daki 24_violated, 71 cover,
deep_violation, extern_source dahil) kontrat düzeyinde ayrıca koşuldu.

- **87 tasarım×kip karşılaştırıldı, 85'i dört çözücüde birebir aynı**:
  aynı PASS/FAIL/UNKNOWN, aynı karşı örnek döngüsü (ör. `24_violated` adım
  7, `54_inout_unsynchronized` adım 3, `58_mmio_access_control` adım 5),
  aynı ulaşılamayan cover'lar (ör. `riscv_core` 2 cover — boolector ve
  bitwuzla bitirdi, z3 ve yices zaman aşımı).
- **2 fark — RAPOR:** ikisi de `prove` kipinde UNKNOWN (çıkış 7, modülün
  hiçbir kontratı kanıtlanmış sayılmaz — dört çözücüde aynı). Farklı olan,
  E5002'nin "tümevarımsal değil" diye **adlandırdığı** kontrat:

  | Tasarım | z3 | boolector | yices | bitwuzla |
  |---|---|---|---|---|
  | `examples/vga/vga_top.volt` | `VgaTop.inv_0` | `inv_2` | `inv_1` | `inv_2` |
  | `tests/ui/pass/87_sdc_targeted_all_bridges.volt` | `fifo_inv_0` | `ps_inv_0` | `fifo_inv_0` | `fifo_inv_0` |

  Neden: tümevarım adımı keyfi (belki erişilemez) bir durumdan başlar;
  birden çok kontrat tümevarımsal değilse çözücünün bulduğu ilk
  karşı-model hangisini bozarsa smtbmc onu yazar (yices logunda iki satır
  görüldü: SV 215 ve 367). Volt ilk satırı adlandırır. Karar değil, seçim
  farkıdır; E5002'nin kendisi "diğerleri de kanıtlanmış sayılmaz" der.
  Kalıcı çözüm (tüm tümevarım ihlallerini listelemek) bu ADR'nin kapsamı
  dışında — Gelecek iş.
- prove UNKNOWN'daki tümevarım **adım** numarası da çözücüye göre değişti
  (0 ve 19), ama volt bunu zaten kullanıcıya göstermez
  (`interpret_sby_output`: "Tümevarım adım numarası kullanıcı döngüsü
  değildir").

### Erişilebilirlik (varsayılanın kurulu olması)

| Kaynak | z3 | boolector | yices | bitwuzla |
|---|---|---|---|---|
| Ubuntu 24.04 apt | var | var | yok | yok |
| Debian trixie apt | var | var | yok | yok |
| `hdlc/formal` (belgedeki Docker tarifi) | var | var | var | **yok** |
| OSS CAD Suite (CI) | var | var | var | var |

Kurulu olmayan çözücüyle sby'nin yazdığı (hdlc/formal'da bitwuzla ile
ölçüldü): `SMT Solver 'bitwuzla' not found in path.` → `DONE (ERROR,
rc=16)`.

## Seçenekler

### A) Varsayılanı en iyi ölçülen motora çevir — SEÇİLDİ: boolector

Ham hızda bitwuzla birinci (bmc/prove toplamı en düşük, en az zaman
aşımı, büyük tasarımlarda açık ara). Ama varsayılan **her kurulumda
çalışmalıdır**: bitwuzla apt'de ve `hdlc/formal`'da yok; varsayılan
olsaydı belgedeki iki kurulum yolunun ikisinde de `volt verify` ilk
denemede araç hatası verirdi. boolector:

- z3'e göre toplam süre bmc 2,4×, prove 1,9×, cover 4,1× kısa; zaman
  aşımı 15 → 4;
- cover'da birinci, bmc/prove'da bitwuzla'dan sonra ikinci;
- apt, hdlc/formal ve OSS CAD Suite'in hepsinde var;
- sorunun kendisini çözer: `riscv_core` bmc d10 (yukarıdaki tablo).

bitwuzla `--engine bitwuzla` olarak eklendi ve zaman aşımı yardımı onu
önerir ("büyük tasarımlarda çoğu zaman en hızlısı").

### B) Portföy — sby çok motoru paralel koşturur mu? ÖLÇÜLDÜ: REDDEDİLDİ

sby `[engines]` bölümünde birden çok satırı destekler: motorlar paralel
başlar, ilk sonuç görevi bitirir, diğerleri sonlandırılır (tek görevli
küçük tasarımda ölçüldü: `engine_0 (smtbmc boolector) returned FAIL`,
`engine_1: terminating process`). Ancak volt görevleri tek sby sürecinde
`-j N` ile koşturur (ADR-0055) ve sby'nin iş sunucusu jetonları motor
başına dağıtır. Portföy (boolector + yices + bitwuzla) ile
(`build/eng/pf_probe.sh`, `--init`'li konteyner, 200 sn bekçi):

| Tasarım | görev × motor | `-j 1` | `-j 4` | `-j 16` |
|---|---|---|---|---|
| `fir_filter` | 3 × 3 = 9 | ASKIDA | ASKIDA (4 motor başladı) | tamam |
| `24_violated_invariant` | 1 × 3 = 3 | ASKIDA (1 motor başladı) | tamam | tamam |

`-j` görev × motor sayısından küçükken sby, biten görevin sonlandırılan
motorlarının jetonlarını geri almıyor gibi davranır: bekleyen motorlar hiç
başlamaz, görev `--timeout` dolunca TIMEOUT olur ve sby süreci %0 CPU'da
sonsuza dek bekler (tam matriste 4 portföy koşusunun 4'ü böyle takıldı).
`volt verify -j 1` (sıralı, cli-contract §8a) bile askıda kalıyor. Güvenli
kullanım `-j ≥ görev × motor` ister: SoC'de 8 × 3 = 24 süreç — CPU'yu 3×
aşırı yükler ve `-j` anlamını bozar. Volt'un kendi portföyü (her motor
için ayrı sby süreci, ilk sonuçta diğerlerini öldürmek) mümkün ama A'nın
getirisinin çoğunu zaten veriyor; gerekirse ayrı ADR.

### C) Tasarıma göre otomatik seçim — REDDEDİLDİ

- Derleme zamanında güvenilir bir öngörücü yok: kazanan tasarıma ve kipe
  göre değişiyor (küçüklerde yices, `soc/top`'ta boolector, işlemci
  çekirdeklerinde bitwuzla, `hybrid_top` prove'da yalnız yices bitiriyor).
- Kurulu çözücüyü yoklamak güvenilmez: sby çoğu zaman başka bir yerde
  koşar (`VOLT_SBY` Docker sarmalayıcısı — konak PATH'i konteyneri
  göstermez).
- Sessiz motor değişimi raporları ortama bağlı yapar (prove UNKNOWN'da
  adlandırılan kontrat çözücüye bağlı — yukarıdaki RAPOR).

## Karar

1. `volt verify` varsayılan motoru **boolector** (`SbyEngine::default()`,
   `--engine` varsayılanı). `.sby` `[engines]` bölümü tek satır kalır:
   `smtbmc boolector`.
2. `--engine bitwuzla` eklendi (`boolector | bitwuzla | yices | z3`).
3. Seçilen çözücü kurulu değilse (`SMT Solver '<ad>' not found in path.`)
   volt araç hatasına şunu ekler — eski belgeye göre yalnız z3 kuran
   kullanıcı için:

   ```text
     = reason: the SMT solver 'boolector' is not installed where sby runs
     = help: install it, or pick an installed solver with --engine boolector|bitwuzla|yices|z3
     = for more: volt explain verify-setup
   ```
4. Kurulum önerisi `apt install yosys boolector`; `volt explain
   verify-setup` çözücüler bölümü ölçümü özetler; zaman aşımı yardımı
   bitwuzla'yı önerir.
5. Çok motorlu portföy kullanılmaz (B); otomatik seçim yok (C).

## Sonuçlar

- CI formal işi (`volt verify` varsayılanla) artık boolector ile koşar;
  OSS CAD Suite'te mevcut.
- JSON `verify.engine` varsayılanı `"boolector"`.
- Testler: `sby.rs` varsayılan + dört çözücü yazımı; `cli_tests`
  `verify_engine_flag_writes_one_smtbmc_line_per_solver` (her `--engine`
  değeri tek `smtbmc` satırı), kurulum yardımı, `explain verify-setup` iki
  dil; `verify_status_tests` eksik çözücü yardımı (yalnız eksik görevde);
  `verify.rs` birim testi `missing_solver`.
- Mutasyon (`build/eng1/mutate.py`, tek tek): 9/9 öldü — enum ve
  `SbyOptions` varsayılanı z3, bitwuzla yanlış yazım, CLI varsayılanı z3,
  CLI bitwuzla → boolector, eksik çözücü okunmaz, not basılmaz, kurulum
  önerisi z3, explain varsayılanı z3.

## Gelecek iş

- prove UNKNOWN'da smtbmc'nin raporladığı **tüm** tümevarım ihlallerini
  listelemek (adlandırılan kontratın çözücüden bağımsız olması için).
- Volt düzeyinde portföy (motor başına ayrı sby süreci) — büyük
  tasarımlarda bitwuzla/yices'in avantajını kurulum şartı olmadan almak
  için; önce sby iş sunucusu sorununun yukarı akışa bildirilmesi.
