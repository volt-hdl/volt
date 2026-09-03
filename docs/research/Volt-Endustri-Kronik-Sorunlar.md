> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Donanım Tasarım Endüstrisinin Kronik Sorunları

> Araştırma tabanlı analiz. Kaynaklar: Wilson Research Group 2024,
> Siemens EDA Verification Study, ESDA Workshop serisi, arXiv.

---

## Bölüm 1 — Sayılarla Endüstrinin Durumu

Bunlar pazarlama rakamları değil, bağımsız araştırma sonuçları:

```
2024 Wilson Research Group Fonksiyonel Doğrulama Raporu:

%14   → ASIC projelerinin ilk silisyumda başarı oranı (tarihin en düşüğü)
%13   → FPGA projelerinin üretimde hata kaçışı olmadan tamamlanma oranı
%60-70 → Doğrulama aşamasının toplam proje süresindeki payı
%63   → LLM'lerin ürettiği donanım assertion'larındaki semantik hata oranı
%50   → Tasarım mühendislerinin doğrulamaya harcadığı zaman

Yani: 100 ASIC projesinden 86'sı ilk silisyumda başarısız.
Bu "normal" kabul edilen bir durum — korkunç ama normalleşmiş.
```

---

## Bölüm 2 — Yedi Kronik Sorun

### Sorun 1 — Doğrulama Verimliliği Uçurumu (En Büyük)

<cite index="16-1">Doğrulama görevleri tek başına tüm IC proje süresinin %60-70'ini tüketiyor. Tasarımcıların neredeyse yarısı da doğrulmaya yardım etmek zorunda kalıyor.</cite>

```
Nasıl böyle oldu:
  1980'ler: Birkaç bin kapı → bir mühendis doğrulayabilir
  2000'ler: Milyonlarca kapı → UVM gerekli
  2020'ler: Milyarlarca transistör → otomasyon yetersiz kalıyor

Temel sorun:
  Tasarım karmaşıklığı ÜS ile büyüyor
  Doğrulama verimliliği DOĞRUSAL büyüyor
  Bu makas kapanmıyor — açılıyor

Kök neden:
  "Bu tasarım doğru mu?" sorusunun cevabı
  RTL'in ne yaptığından çok
  ne yapması GEREKTİĞİNE bağlı.
  Ama "ne yapması gerektiği" çoğunlukla:
  → Excel tabloları
  → Word belgeleri
  → Mühendisin kafasında

  Makine okuyamıyor → otomatize edilemiyor.
```

**Volt'un bu sorunla ilişkisi:**

```
assert/invariant → formal doğrulama dilde
Tasarım ile spec aynı dosyada → boşluk azalır

Ama tam çözüm değil:
  UVM düzeyinde verification methodology yok
  Coverage-driven verification yok
  Büyük SOC için yeterli değil (MVP'de)
```

---

### Sorun 2 — Spesifikasyon Uçurumu (Sessiz Kök Neden)

<cite index="14-1">Synopsys'ten Frank Schirrmeister: "Yeni darboğaz spesifikasyonun netliği. Bu, doğrulama mühendislerini tasarım mühendislerinden ayıran eski düşünceye kadar 30 yıl öncesine gidiyor. Sektör bir tarafı düzeltti — RTL'i belirli bir noktadaki donanım örneği olarak modelledi — ama her şeyi soyutlamadı. Her şeyin türetilebildiği, donanım/yazılım ve altın olan yürütülebilir bir spesifikasyon şimdi mümkün mü?"</cite>

```
Sorun şu:
  Mühendis isterleri okur (Word/Excel)
  RTL yazar
  Doğrulama mühendisi isterlerden testbench yazar
  Yazılım ekibi isterlerden sürücü yazar

  Üç ekip aynı belgeyi farklı anlıyor.
  Hiç kimsenin referans aldığı tek "altın" yok.
  Silisyumda hata çıkınca: "Ama isterler öyle demiyordu ki..."

Mevcut çözüm girişimleri:
  SystemRDL: register haritaları için
  IP-XACT: IP meta verisi için
  UVM Sequences: davranışsal spec
  → Hepsi parçalı, bağlantısız

İdeal (henüz yok):
  Tek makine-okunabilir spec →
    RTL türetilir
    Testbench türetilir
    Yazılım sürücüsü türetilir
    Formal özellikler türetilir
```

**Volt'un bu sorunla ilişkisi (en büyük fırsat):**

```
Volt'un tip sistemi + formal assertion → yürütülebilir spec

module UART_TX {
    // Bu spec hem RTL hem formal hem dokümantasyon:
    invariant: baud_counter < baud_rate
    invariant: tx_valid → tx_data kullanılabilir
    assert: her START biti sonrası 8 data biti gelir
}

HW-SW köprüsü:
  register_map → Rust driver otomatik üretilir
  Bu "altın spec" fikrinin küçük ama somut adımı

Uzun vadeli vizyon:
  Volt dosyası = donanım + doğrulama + yazılım arabirimi spec'i
  Bu Synopsys'in 30 yıldır çözmediği problem
  Volt bu yönde mi gidecek?
  → Eğer giderse endüstride transformatif etki
```

---

### Sorun 3 — Sıfırlama Alanı Geçişleri (RDC) — CDC'nin Kardeşi

```
CDC nasıl bilinen bir sorunsa, RDC de aynı derecede
kritik ama çok daha az araç desteği var.

RDC nedir:
  Farklı sıfırlama alanları arasında veri geçişi
  Sıfırlama zamanlaması farklı → metastabilite
  Çıkış davranışı belirsizleşir

  Örnek:
  domain A: senkron sıfırlama
  domain B: asenkron sıfırlama, ters polarite
  A → B geçişinde sinyal → öngörülemez başlangıç durumu

Bugün RDC nasıl çözülüyor:
  Ticari araçlar (Synopsys SpyGlass RDC, Cadence JasperGold RDC)
  Elle yazılmış kısıt dosyaları
  Deneyim ve sezgi ("bunu biliyordum")

Neden kötü:
  CDC gibi derleme zamanında değil
  Ayrı araç çalıştırmak gerekiyor
  Hata maliyeti: silisyum yeniden üretim

Volt'un fırsatı:
  Reset<S, P, D?> tipi zaten Arch'ta var!
  (Arch bunu yaptı — Volt bunu ÖNCE yapmalıydı)
  
  domain SysDomain { reset = sync, active_high }
  domain MemDomain { reset = async, active_low }
  
  RDC ihlali → CDC gibi derleme zamanı hatası
  
  Bu MVP'de olmalı mıydı?
  → Evet, ama ADR-0002 buna yer ayırmamış
  → Acil revizyon gerekiyor
```

---

### Sorun 4 — X-Yayılımı (Simülasyon Kör Noktası)

```
Nedir:
  VHDL ve Verilog simülasyonunda "X" (bilinmeyen) değeri var
  Başlatılmamış register → X
  Metastabilite → X
  Hata koşulu → X

Sorun:
  X'i kullanan bir sinyal de X olur
  X AND 0 = 0 (Verilog kuralı!)
  X AND 1 = X
  
  Yani: bazı X değerleri 0 ile "yutulabilir"
  Simülasyonda: tasarım çalışıyor gibi görünür
  Silisyumda: farklı davranır (0 veya 1 ama hangisi belirsiz)

  Buna "X-iyimserlik" deniyor.
  2024 verisi: mantık/fonksiyon hataları hâlâ #1 re-spin nedeni
  Bunların önemli kısmı X kaynaklı.

Volt'un yaklaşımı:
  Tek semantik model → simülasyon = sentez
  Başlatılmamış register → derleme hatası veya zorunlu başlangıç değeri:
    reg(Sys) count : u8 = 0  ← zorunlu başlangıç
  
  Ama 4-state simülasyon (X/Z) henüz planlanmamış
  Bu açık bir gap.

Tam çözüm için:
  Volt'ta 4-state semantik opsiyonu
  "X-güvenli" modu: tüm başlatılmamışlar uyarı
```

---

### Sorun 5 — EDA Araç Maliyeti ve Oligopolü

<cite index="20-1">Üç büyük EDA şirketi, dramatik şekilde yüksek bir yüzdeye sahip oldukları 4 milyar dolarlık piyasayı elinde tutarken araştırma yatırımlarını da dramatik biçimde azaltmıştır. Dahası, EDA alanında startup şirketler artık gelişemiyor. Kullanışlı teknolojilere sahip az sayıdaki şirket, yeni fikirleri desteklemeden bunları top-3 şirketlerinden birine emmek zorunda kalıyor.</cite>

```
Araç maliyeti gerçeği:
  Synopsys tam lisans paketi: $1-5M/yıl
  Cadence benzer: $1-5M/yıl
  Küçük startup: bu parayla 5-10 mühendis tutabilir

  Sonuç:
  Küçük şirket → kısıtlı araç → rekabet dezavantajı
  Üniversite → araç yok → mezun hazırsız
  Mezun şirkete giriyor → sıfırdan öğreniyor → maliyet

Volt'un bu sorunla ilişkisi:
  Açık kaynak ASIC akışı:
  Volt → Yosys (açık) → OpenROAD (açık) → Sky130/GF180

  Bu tam akış bugün çalışıyor!
  (Tiny Tapeout bu akışı kullanıyor)
  
  Ticari araç olmadan ASIC tape-out → mümkün
  Doğrulama: SymbiYosys (açık formal) + Verilator (açık sim)
  
  Bu özellikle başlangıç şirketleri ve akademi için kritik.
```

---

### Sorun 6 — Uzun Geri Bildirim Döngüleri

```
Mühendis kodu yazar, ne kadar bekler?

Aktivite              Bekleme süresi
─────────────────────────────────────────
Derleme               dakika
RTL lint              dakika
Simülasyon (küçük)    dakika
Simülasyon (büyük)    saat-gün
Sentez                saat-gün
P&R                   gün-hafta
Timing sign-off       hafta
Silisyum              ay

Sonuç:
  Hata ne kadar geç bulunursa maliyeti o kadar yüksek
  Silisyumda bulunan hata = $1M+ re-spin
  
  "Shift left" — hatayı akışta öne taşı
  Bu endüstrinin son 10 yılın ana teması

Volt'un yaklaşımı:
  Tam shift left: CDC hatası derleme zamanında
  (gün → saniye)
  
  ML PPA tahmini: sentez olmadan alan/güç tahmini
  (gün → dakika)
  
  Formal doğrulama: simulation loop olmadan kanıt
  (saat → dakika kararı için)
  
  Bu "shift left" değer önerisi güçlü ve somut.
```

---

### Sorun 7 — IP Güven ve Kalite Sorunu

```
Endüstri gerçeği:
  Modern SoC'un %80'i satın alınan IP
  (ARM çekirdeği, PHY, USB, PCIe, DSP bloğu...)
  
  IP'nin kalitesi nasıl bilinir?
  → Sağlayıcının söylediğine güven
  → Kendi doğrulama → çok pahalı
  
  Sorunlar:
  IP belgesi hatalı → entegrasyon başarısız
  IP'nin CDC açıkları → entegrasyon sonrası belirsiz
  IP güvenlik açıkları → kasıtlı backdoor riski (nadir ama gerçek)
  IP sürüm yönetimi → "hangi versiyonu kullandım?"

Volt'un fırsatı:
  "Volt Certified" IP:
  □ CDC formal olarak kanıtlanmış
  □ Tüm portlar tip-güvenli
  □ Assertion kapsamı >%90
  □ Deterministik build (sürüm kilitli)
  
  Bu sertifikasyon bugün sektörde yok.
  Volt registry + sertifikasyon = IP güven altyapısı
  
  Donanım dünyasında npm + semver eşdeğeri.
```

---

## Bölüm 3 — Volt'un Değemeyeceği Sorunlar (Dürüst Sınırlar)

```
FIZIKSEL TASARIM — Volt'un kapsamı dışında:

Timing Closure:
  P&R sonrası setup/hold ihlalleri
  ECO döngüleri (Engineering Change Orders)
  Kritik yol optimizasyonu
  → Primetime, Tempus: Volt'un girmediği alan

IR Drop:
  Güç şebekesi analizi
  Voltaj düşüşü → lojik hız değişimi
  → Apache RedHawk: tamamen farklı domain

Signal Integrity:
  Crosstalk, EMI, return path
  → Cadence Clarity: Volt'un alamayacağı alan

DRC/LVS:
  Dökümhane fiziksel kurallar
  Calibre: Volt çıktısı SV, Calibre bunu GDSII'den kontrol eder
  → Bu her zaman ayrı araç

ANALOG TASARIM:
  Op-amp, PLL, ADC/DAC
  → Virtuoso/SPICE: Volt tamamen dijital

ÜRETIM VARYASYON:
  Monte Carlo simülasyon
  Corner analizi
  → Volt bu katmana dokunmuyor
```

---

## Bölüm 4 — Çözülmemiş ve Büyüyen Sorunlar

Bu sorunlar endüstrinin henüz çözemediği, büyüyen problemler:

### 4.1 Chiplet/Multi-Die Entegrasyon

```
Gelişen trend:
  Tek büyük çip yerine birden fazla küçük chiplet
  (AMD EPYC, Apple M-serisi, Intel Meteor Lake)

Yeni sorunlar:
  UCIe/AIB arayüzleri → yeni protokol standardı
  Die-to-die timing → çip sınırını geçen zamanlama
  Isı yönetimi → chipletler arasında
  
Volt'un fırsatı:
  UCIe arayüzü Volt'ta tip-güvenli tanımlanabilir
  Die sınırı = özel domain geçişi
  Bu henüz hiçbir HDL'de iyi çözülmemiş
```

### 4.2 Donanım Güvenliği

```
Büyüyen sorun:
  IoT cihazları → milyarlarca güvensiz donanım
  Yan kanal saldırıları (timing, power)
  Hardware Trojan (supply chain attack)
  
Volt'un güvenlik akış denetimi (v2):
  @Secure domain → @Public domain geçişi yasak
  Güç tüketimi örüntüsü analizi (araştırma)
  Formal güvenlik kanıtı
  
Bu alan hızla büyüyor ve araç desteği yetersiz.
```

### 4.3 Fonksiyonel Güvenlik Belgesi Yükü

```
ISO 26262, DO-254, IEC 61508:
  ASIL-D, DAL-A sertifikasyonu
  Her karar için iz belgesi şart
  "Bu assertion neden yazıldı?" → belge
  
Bugün:
  Elle yazılan dokümanlar
  RTL ile senkronizasyon hataları
  
Volt'un fırsatı:
  assert, invariant → otomatik traceability belgesi
  "Bu formal özellik şu gereksinim maddesini karşılıyor"
  ADR → gereksinim bağlantısı
  
  Bu, otomotiv ve havacılık için çok değerli.
  Bugün kimse bunu iyi yapmıyor.
```

### 4.4 LLM Üretilen RTL Güvensizliği

<cite index="16-1">LLM tabanlı doğrulamadaki kritik darboğaz, üretilen assertion'lardaki yüksek semantik hata oranı. AssertEval gibi benchmark'lar, ticari LLM'lerin %63 oranında semantik hatalı assertion ürettiğini ortaya koyuyor.</cite>

```
Sorun:
  LLM'ler RTL ve assertion yazıyor
  Ama %63 semantik hata oranı kabul edilemez
  "Sözdizimi doğru ama anlambilim yanlış"

Neden:
  Verilog/SV sözdizimi öğrenilebilir ama semantik ince
  LLM eğitimi kod gördü ama donanım çalıştırmadı
  Feedback loop yok (doğru mu derleyemez)

Volt'un avantajı (bu konuşmada geçti):
  LLM Volt kodu üretiyor
  Tip sistemi semantik hataları yakalıyor
  LLM hata mesajından öğreniyor
  Döngü: her turda daha az hata

  Arch da bunu gördü (LL(1), todo!)
  Ama Volt'un tip sistemi daha güçlü semantik garantisi
  → LLM + Volt = LLM + Arch'tan daha güvenilir RTL
```

---

## Bölüm 5 — Sorunların Volt'la İlişkisi Özeti

```
SORUN                        VOLT POTANSİYELİ    ZAMAN
──────────────────────────────────────────────────────────────
Doğrulama verimliliği        Orta (formal dilde)  V1-V2
Spesifikasyon uçurumu        Yüksek (yür. spec)   V1
RDC (reset domain)           Yüksek (CDC gibi)    MVP REVIZYONU
X-yayılımı                   Orta (tek semantik)  V1
EDA araç maliyeti            Yüksek (açık kaynak) Bugün
Uzun geri bildirim döngüsü   Yüksek (shift left)  MVP
IP güven/kalite              Yüksek (registry+)   V1
Chiplet entegrasyon          Orta (domain model)  V2
Donanım güvenliği            Yüksek (@Secure)     V2
Fonksiyonel güvenlik         Yüksek (traceability)V1-V2
LLM RTL güvensizliği         Yüksek (tip sistemi) MVP
──────────────────────────────────────────────────────────────
Fiziksel tasarım             HAYIR (kapsam dışı)  —
Analog tasarım               HAYIR                —
Üretim varyasyon             HAYIR                —
```

---

## Bölüm 6 — Acil Aksiyon Önerisi: RDC

```
Bu analiz kritik bir eksikliği ortaya çıkardı:

RDC (Reset Domain Crossing) MVP'de YOK.

Ama:
  - CDC'nin doğal kardeşi
  - Arch bunu Clock<D>, Reset<S,P,D?> ile yaptı
  - Volt CDC'yi yapıyorsa RDC'yi yapmamak tutarsız
  - Otomotiv/havacılık müşterileri her ikisini de sorar

Önerilen ADR revizyonu:
  ADR-0002-cdc-semantik.md → ADR-0002-cdc-rdc-semantik.md
  
  Reset tipi:
  domain SysDomain {
      clock = posedge
      reset = sync, active_high    ← reset semantiği
  }
  
  domain MemDomain {
      clock = posedge
      reset = async, active_low   ← farklı reset
  }
  
  RDC ihlali:
  // E3002: Sıfırlama alanı uyumsuzluğu
  reg_mem @MemDomain = sig_sys @SysDomain  ← hata

Bu değişiklik Volt'u Arch'tan öne geçirir:
  Arch: Clock<D> + Reset<S,P,D?> → ayrı parametreler
  Volt: @Domain içinde hem clock hem reset → daha sade
```

---

## Özet

```
Endüstrinin en kronik sorunları:

1. Doğrulama verimliliği (%70 süre, %14 başarı)
   → Volt kısmen: formal assertion dilde
   → Tam çözüm sektörün en büyük açık problemi

2. Spesifikasyon uçurumu
   → Volt fırsatı: yürütülebilir spec
   → HW-SW köprüsü bu yönde ilk adım

3. RDC (sıfırlama alanı geçişi)
   → Volt'ta OLMAMALI DEĞİL, OLMALI
   → ADR revizyonu acil

4. X-yayılımı
   → Tek semantik model kısmen çözüyor
   → 4-state seçeneği ileride

5. EDA oligopolü / araç maliyeti
   → Volt açık kaynak → demokratikleştirme

6. IP güveni
   → Volt registry + sertifikasyon → çözüm yolu

7. LLM RTL güvensizliği
   → Volt tip sistemi → semantik garantisi

Volt'un en güçlü ek değer önerisi:
  "Spesifikasyon, RTL ve doğrulama arasındaki boşluğu
   tek Volt dosyasına kapatıyoruz.
   30 yıllık 'altın spec' problemi için küçük ama somut adım."

Bu CDC'den bile daha büyük bir iddia.
Ama somut olmak için HW-SW köprüsü ve formal dilde
çalışan örnekler gerekiyor — belgeden önce.
```
