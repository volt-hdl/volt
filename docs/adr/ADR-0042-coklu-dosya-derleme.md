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
