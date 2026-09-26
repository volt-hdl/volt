# ADR-0042: Çoklu Dosya Derleme ve Import Sistemi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-13
> Etkilenen: name-resolution.md §3.2/§10, cli-contract.md §5, ADR-0024,
> volt-syntax, volt-ast, volt-hir, volt-sv-emit, volt-driver
> Uygulama aşaması: F5 (bu ADR ile birlikte uygulandı)

## Sorun

`examples/soc/` keşfi (rapor 7a) şunu gösterdi: `package` ve `use`
ayrıştırılıyor ama isim çözümlemesi yok; `volt build` tek dosya alıyor.
Sonuç: 1 067 satırlık SoC tek dosyada yazılmak zorunda kaldı, `UartTx` ve
`Axi4LiteSlave` yeniden kullanılamayıp kopyalandı (`flatten.sh` ile
metinsel birleştirme). Hiçbir ciddi tasarım tek dosyada yaşamaz.

İkinci, bağlı sorun: ADR-0024 (modül başına SV dosyası) uygulanmamıştı;
Verilator lint için `awk` ile bölme gerekiyordu (DECLFILENAME).

## Karar

### 1. Dosya keşfi

`volt build <dosya>` bağımlılıklarını `use` bildirimlerinden kendisi
bulur. `use soc::gpio::Gpio;` için arama sırası:

1. `./soc/gpio.volt` — import eden dosyanın dizinine göre
2. `<kök>/<src>/soc/gpio.volt` — `<kök>` import eden dosyadan yukarı
   doğru bulunan ilk `Volt.toml`'un dizini; `[package] src` anahtarı
   (varsayılan `src`) kaynak kökünü verir
3. `std::` ön eki — yerleşik kütüphane, dosya yüklenmez

`Volt.toml [package]` bölümü: `name` (paket adı) ve `src` (kaynak kök
dizini). Mevcut `[ui] lang` okuyucusu gibi satır tarayıcıyla okunur;
TOML bağımlılığı alınmaz.

Keşif derinlik-öncelikli yapılır; bir dosya bir kez yüklenir (kanonik
yol). Bulunan dosyalar **bağımlılık sırasıyla** (önce bağımlılıklar, ana
dosya sonda) tek derleme birimine ayrıştırılır.

### 2. Paket bildirimi

```volt
// examples/soc/gpio.volt
package soc::gpio;

pub module Gpio { ... }      // dışa açık
    module Internal { ... }  // dosyaya özel
```

Bir dosya en çok bir `package` bildirir (aksi E0001, mevcut). Bildirim
yoksa dosyaya varılan `use` yolu paket adı olur (`use bus::X` → `bus`).
Dosya hem bildirdiği paket adıyla hem de varılan yolla kayıt edilir; iki
biçim birlikte çalışır.

### 3. `use` çözümlemesi

Desteklenen biçimler (ayrıştırıcı zaten tanıyordu):

```volt
use soc::gpio::Gpio;
use soc::{gpio::Gpio, timer::Timer};
use soc::gpio::*;                 // hedefin tüm pub öğeleri
use soc::gpio::Gpio as G;
```

name-resolution.md §3.2'deki nitelikli yol mantığı dosya sınırını aşar:
yolun son bölümü dışındaki kısım **paket yolu**, son bölüm **öğe**dir.
Öğe hedef dosyanın üst düzey öğelerinde aranır.

| Durum | Kod |
|---|---|
| Paketi sağlayan dosya yok | **E1011** "modül bulunamadı" — aranan yollar `note` ile listelenir |
| Dosya var, öğe yok | **E1011** — yardım metni paketin `pub` öğelerini sıralar |
| Öğe `pub` değil | **E1004** (mevcut kod, ilk kez üretiliyor) |
| Döngüsel import (`a` → `b` → `a`) | **E1006** (mevcut kod) — import zinciri `note` ile |
| Aynı yerel ad iki kaynaktan | **E1010** (mevcut kod) |

Görünürlük kuralı: bir dosya başka dosyanın öğesini yalnız import
etmişse (adıyla, takma adıyla veya glob ile) yalın adla kullanabilir.
Import edilmemiş dış öğeye başvuru E1001'dir. Kendi dosyasının öğeleri
her zaman görünür. Monomorfize örnekler (`Delay_4_8`) şablon adının
(`Delay`) görünürlüğünü devralır.

### 4. Derleme birimi

Tüm dosyalar tek `SourceFile` arena'sında toplanır (`parse_unit`); her
düğümün `Span.file` alanı kendi dosyasını taşır, tanılar doğru dosyayı
gösterir. Bundle düzleştirmesi (ADR-0039) ve monomorfizasyon (ADR-0041)
tüm birim üzerinde **bir kez** koşar — bundle tipi ve modülü, generic
tanım ve örneklemesi farklı dosyalarda olabilir.

- Geçiş 1: tüm dosyaların öğe seviyesi toplanır (ileri referans serbest)
- Geçiş 2: gövdeler çözümlenir; kök kapsam aramaları dosya başına
  görünürlük kümesiyle süzülür

Bilinen sınır: kök kapsam birim genelinde tektir — iki dosya aynı adla
öğe tanımlayamaz (E1003). Paket başına ad alanı ileriki bir ADR konusu.

`use` çözümlemesi artık dosya yüklediği için tek dosyalı akış da birim
akışından geçer: hedefi olmayan bir `use` eskiden sessizce kabul
edilirken (W1005 ile) artık E1011'dir.

Artımlı derleme YOK — her build sıfırdan. Ölçüm (debug, Windows):
8 dosyalık SoC 0,02–0,09 s; süreç başlatma baskın.

### 5. Modül başına SV dosyası (ADR-0024 uygulaması)

`volt build` varsayılan olarak her modülü `build/rtl/<Modül>.sv`'ye
yazar; başlığa `// Module:  <Modül>` ve modülün **kendi** kaynak dosyası
(`// Source:  uart_tx.volt`) yazılır. DECLFILENAME uyarısı kalkar;
Verilator tüm dosyaları birlikte lint edebilir.

`--single-file` bayrağı eski düzeni (`build/rtl/<kaynak>.sv`, tüm
modüller) korur. `volt run/test/verify` birleşik metni kullanmaya devam
eder. JSON zarfının `artifacts` alanı üretilen her dosyayı listeler.

### 6. `examples/soc/`

1 005 satırlık `soc.volt` ve `flatten.sh` silindi. Düzen:

| Dosya | Paket | İçerik |
|---|---|---|
| `top.volt` | `soc::top` | `SocTop` |
| `bus.volt` | `soc::bus` | `BusDecoder` (+ dosyaya özel `AxiSel`) |
| `axi.volt` | `soc::axi` | `RegBus`, `AxiReadCtl`, `AxiToReg` — `../axi4lite_slave.volt`'tan import |
| `gpio.volt` | `soc::gpio` | `Gpio` |
| `timer.volt` | `soc::timer` | `Timer` |
| `uart.volt` | `soc::uart` | `UartCtrl` — `../uart_tx.volt`'tan `UartTx` import |

`examples/Volt.toml` (`src = "."`) `examples/` dizinini kök yapar;
`use axi4lite_slave::...` ve `use soc::gpio::Gpio` oradan çözülür.
Kopyalanan kod yok: `UartTx` ve `Axi4LiteSlave` referansla kullanılır.

## Reddedilen Seçenekler

- **`Volt.toml` modül listesi**: her dosyayı elle saymak gerekir;
  `use`'dan keşif hem daha az yazım hem hata kaynağı yok.
- **Dosya başına ayrı arena + indeks yeniden eşleme**: her `Idx` alanına
  dokunmak gerekir; tek arena değişikliği ayrıştırıcıda 20 satır.
- **Paket başına tam ad alanı**: kök kapsamı dosya bazlı süzmek aynı
  hataları verir; ad alanı ayrımı (aynı adlı iki özel öğe) ertelendi.

## Sonuçlar

- `tests/ui/multifile/` dizini: `basic/`, `pubpriv/` (E1004),
  `notfound/` (E1011), `cyclic/` (E1006); UI harness çoklu dosya
  fixture'larını birim olarak analiz eder.
- `volt explain E1011` iki dilde.
- name-resolution.md §3.2'ye çoklu dosya notu, §10 tabloya E1011
  eklenir; cli-contract.md §5 çıktı adlandırması ADR-0024'e uyar.

## Ek — Çıktı kümesi: ana dosyadan erişilebilen modüller (2026-09-26, ADR-0081 Aşama 3 bulgusu)

### Sorun

ADR-0081 Aşama 3 iki belirti gördü: (1) `use riscv_core::imm_i_of`
diyen `riscv_pipeline`'ın `volt build` çıktısına `RiscvCore.sv` ve
`UartTx.sv` de ekleniyordu; (2) yalnız fn içeren `riscv_imm.volt` tek
başına derlenince modülsüz, boş bir `riscv_imm.sv` yazılıyordu ve çıktı
ağının (ADR-0079) Verilator adımı reddediyordu (`%Error: Specified
--top-module 'riscv_imm' was not found in design.`).

### Kural (önceki durum, kodda)

`volt_sv_emit::emit_unit` birimdeki **her** `ItemKind::Module`'ü üretiyordu
— `use` ile yüklenen dosyalardaki modüller dahil, örneklensin ya da
örneklenmesin. Birim ne istediğinden bağımsız dosyanın tamamını yükler
(`use a::f` → `a.volt`'un tüm öğeleri). Sürücü modül listesi boşsa
`--single-file` düzenine düşüp `build/rtl/<kök>.sv`'ye yalnız başlık
yazıyordu. SVA (`.sva`), `verify` görevleri, SDC/XDC ve `@mmio`
sürücüleri de aynı "birimdeki her modül" kümesinden üretiliyordu.

Tüm korpus (`tests/ui/pass`, `examples/`, `tests/fixtures/`,
`tests/ui/multifile/`; `--emit=sva,sdc,xdc,rust,c,regmap` ve
`--single-file`) önce/sonra dökümünde fazla yazılan dosyalar:

| Tasarım | Fazla yazılan (artık yok) | Neden yüklenmişti |
|---|---|---|
| `examples/soc/axi.volt` | `Axi4LiteSlave` .sv/.sva/.sdc/.xdc | `axi4lite_slave`'den yalnız struct tipleri |
| `examples/soc/timer.volt` | aynı | aynı |
| `examples/soc/uart.volt` | aynı | aynı |
| `examples/vga/frame_buffer.volt` | `VgaTiming` .sv/.sva/.sdc/.xdc | `vga_timing`'den yalnız `domain`'ler |
| `tests/ui/multifile/basic/main.volt` | `Hidden` .sv/.sdc/.xdc | `lib.volt`'ta örneklenmeyen özel modül |
| `tests/ui/multifile/fn/fnlib.volt` | boş `fnlib.sv` (iki düzende) | modülsüz dosya |

Kalan her dosyanın içeriği `// Source:` satırı dahil aynı; yalnız
`--single-file` birleşik metinlerinden kalkan modüller düştü ve `.sva`
yorumlarındaki kaynak satır numaraları taşınan fn'ler kadar kaydı
(`riscv_core`, `riscv_pipeline`, `hello_soc`). Çıkış kodu değişen
tasarım yok.

### Karar

**Çıktı kümesi = ana dosyanın modülleri + örnekleme kapanışı.** Ana dosya
`FileId(0)`'dır (birim yükleyici onu ilk ziyaret eder); `use` ile yüklenen
her dosya kütüphanedir. Parser'ın ürettiği sentetik kaynaklar (`@mmio`,
ADR-0044) kütüphane sayılmaz. Kapanış emitter'ın örnek hedefiyle aynı
kuralı izler: modül gövdesinin üst düzey `Instance` deyimleri, yerleşik
primitifler hariç (`volt_sv_emit::reachable_modules`). Monomorfize örnek
somut adı taşıdığından `Delay_4_8` gibi modüller kendiliğinden girer.

Süzgeç tek noktada, sürücünün `Compiled::retain_reachable`'ında (yeni
`reach.rs`) ve aynı küme şu çıktıların hepsine uygulanır:

| Çıktı | Süzülür |
|---|---|
| `build/rtl/<Modül>.sv` ve `--single-file` birleşik metni | evet |
| `--emit=sva` `.sva` dosyaları | evet |
| `--emit=sdc,xdc` kısıtları (ve W0022 uyarıları) | evet |
| `--emit=rust,c,regmap,regmap-md` `@mmio` sürücüleri, `--check-regmap` | evet |
| `volt verify` görevleri ve `build/formal/<iş>.sv` | evet |
| `volt run` / `volt test` | hayır — test dosyası modül tanımlamaz, test ettiği modülleri `use` ile alır |
| Tanılar (`check`, `build`, LSP) | hayır — kütüphane dosyasının tamamı yine denetlenir (ADR-0070 paritesi) |

**Modülsüz birim SV üretmez.** Yalnız fn/const/tip içeren dosya
kütüphanedir: `volt build` çıkış 0, `0 SV file(s)`, JSON `artifacts: []`
ve insan çıktısında bilgi notu (`volt run/verify` önerisi yerine):

```text
    Finished 0.00s (1 source file(s), 0 SV file(s))
        Note no module in 'riscv_imm.volt' — no SystemVerilog written
        Help this is a library file: call its pub fn from a module with 'use riscv_imm::<name>;'
```

Uyarı ya da hata DEĞİL: kütüphane dosyasını tek başına derlemek (örneğin
çıktı ağının yaptığı gibi) geçerli bir kullanımdır.

### Reddedilen

- **emit_unit'e kök parametresi**: dört çağıran (build, check, LSP,
  tek dosya testleri) var ve `check`/LSP'nin çıktıya ihtiyacı yok; üstelik
  süzgeç SDC ve sürücü modellerini de kapsamalı — bunlar emitter'ın
  dışında üretilir. Sürücüde tek nokta hepsini birden bağlar.
- **Emit'i erişilemeyen modüller için atlamak**: kütüphanedeki bir
  modülün E0003 gibi emitter tanıları `build`'de kaybolurdu (ADR-0070
  paritesi bozulur).
- **Modülsüz dosyaya boş bir sarmalayıcı modül yazmak**: Verilator'u
  susturur ama kullanıcıya var olmayan bir üst modül sunar.

### Kanıt

- `riscv_pipeline.volt` immediate'leri artık `examples/riscv_imm.volt`
  ortak dosyasından; `riscv_core.volt` da aynı dosyayı kullanır.
  `RiscvPipeline.sv`, `RiscvCore.sv`, `UartTx.sv` **tamamen byte-aynı**
  (`cmp`, `// Source:` satırı dahil). `volt test` riscv_core 59/59,
  riscv_pipeline 15/15, diğer 7 örnek geçti (Docker, OSS CAD Suite
  2026-09-21 Verilator).
- Çıktı ağı: yeni `tests/fixtures/fn_library/` (`checksum.volt` modülsüz
  kütüphane, `sum_top.volt` kullanıcısı) ve `examples/riscv_imm.volt`
  korpusta. Eski ikiliyle `checksum.volt` → `%Error: Specified
  --top-module 'checksum' was not found in design.`; yenisiyle SV yok,
  koşu yok, bulgu yok.
- Testler: `volt-sv-emit` `reach` birim testleri (4), `volt-driver`
  `unit_output_tests` (6: modülsüz dosya iki düzende + JSON, yalnız fn
  alan birimde her çıktı türü, örneklenen kütüphane modülü kalır,
  `verify` görev listesi); `cli_tests`'in iki çoklu dosya testi yeni
  kümeye güncellendi (`Hidden.sv` artık yazılmıyor, 3 → 2 SV).
- Mutasyon (`build/fnlib/mutate.py`, tek tek): 14/14 öldü — kapanışı
  izlememek, her dosyayı kök saymak, ana dosyayı kütüphane saymak,
  modül/sva/prop/regmap/kısıt süzgeçlerinden her biri, birleşik SV'yi
  yeniden kurmamak, hatalı derlemede erken dönmemek, build'de ya da
  verify'da süzmemek, modülsüz birimde eski düzen, bilgi notunu atlamak.
