> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Komut Setleri (ISA) — Mevcut Durum, Sorunlar ve Yeni Arayışlar

> Kaynaklar: ISCA 2026, ASPLOS, MICRO, arXiv, TechInsights,
> The Register, Intel/AMD duyuruları. Tarih: Ağustos 2026

---

## Bölüm 1 — Üç Büyük Oyuncu: Güncel Durum

### 1.1 x86 — 48 Yıllık Miras

```
Doğuş:    1978 (Intel 8086)
Sahip:    Intel + AMD (çapraz lisans)
Pazar:    Masaüstü, dizüstü, kurumsal sunucu
Durum:    Baskın ama sıkışıyor
```

**2023-2026 arasında yaşananlar — bir reform denemesi ve kısmi başarısızlık:**

```
APX (Advanced Performance Extensions):
  Genel amaçlı yazmaç sayısı 16 → 32
  Tam sayı komutlarına ayrı hedef yazmaç
  Sonuç: derlenmiş kodda %10 daha az load, %20 daha az store
  Durum: GCC 14'te destek var, Diamond Rapids ile geliyor
  Değerlendirme: "40 yıllık RISC mimarilerinde bulunan
                  yetenekleri ekliyor, çoktan gecikmiş"

AVX10:
  AVX-512'nin karmaşasını temizleme girişimi
  P-core 512 bit, E-core 256 bit → birleşik ISA
  AVX-512 donduruluyor, gelecek AVX10 üzerinden

x86S:
  64-bit-only x86, eski modları (real mode, 16-bit) atma
  Amaç: saldırı yüzeyini küçültmek + verimlilik
  DURUM: İPTAL EDİLDİ (Gelsinger'in ayrılışından sonra)
  → x86'nın temizlenme şansı kaybedildi
```

**x86'nın kırılganlıkları:**

```
1. Yasal ikilik:
   Sadece iki şirket üretebiliyor (Intel, AMD)
   Çapraz lisans anlaşması → üçüncü oyuncu giremiyor
   → İnovasyon iki şirketin kararına bağlı

2. Karmaşıklık kaynaklı güvenlik açığı:
   Derin pipeline + spekülatif yürütme → geniş saldırı yüzeyi
   Spectre/Meltdown bunun sonucu
   Her yeni optimizasyon → yeni yan kanal riski

3. Decode maliyeti:
   Değişken uzunlukta komut (1-15 bayt)
   → Paralel decode zor, transistör pahalı
   → ARM/RISC-V'de bu maliyet yok

4. Geriye uyumluluk yükü:
   1978'den kalan modlar hâlâ destekleniyor
   x86S iptal edilince bu yük kalıcılaştı

5. Fragmentasyon (kendi içinde):
   AVX-512 bazı çiplerde var, bazılarında yok
   E-core/P-core farklı yetenekler
   → Yazılım hangi komutu kullanacağını bilemiyor
```

### 1.2 ARM — Lisans Modeli, Mobil Hakimiyeti

```
Doğuş:    1985 (Acorn)
Sahip:    Arm Holdings (SoftBank çoğunluk)
Pazar:    Mobil (%99), gömülü, giderek sunucu
Durum:    Güçlü ama RISC-V baskısı altında
```

**Güçlü yönler:**

```
✓ Olgun ekosistem (Linux, Android, iOS, derleyiciler)
✓ Temiz ISA (ARM64'te eski yükler atıldı)
✓ Güç verimliliği (Apple M serisi kanıtı)
✓ Geniş IP portföyü (Cortex-M'den Neoverse'e)
✓ SVE/SVE2 ile ölçeklenebilir vektör
```

**Kırılganlıklar:**

```
1. Telif maliyeti:
   Çip fiyatının %1-2 + milyonlarca dolar peşin lisans
   Yüksek hacimli üretici için ciddi maliyet
   → RISC-V'nin ana satış argümanı

2. Tek şirket riski:
   ARM Holdings'in kararları tüm ekosistemi etkiliyor
   Nvidia satın alma girişimi (başarısız) bu riski gösterdi
   SoftBank sahipliği → finansal baskı → fiyat artışı

3. Lisans kısıtları:
   Mimari lisans çok az şirkete veriliyor
   Özel komut eklemek zor/imkânsız
   → AI hızlandırıcı için esneklik yok

4. Qualcomm davası (2022-2025):
   ARM'ın lisans şartlarını değiştirme girişimi
   → Müşteri güveni sarsıldı
   → RISC-V'ye kaçış hızlandı
```

### 1.3 RISC-V — Açık Alternatif, Hızlı Olgunlaşma

```
Doğuş:    2010 (UC Berkeley)
Sahip:    RISC-V International (İsviçre, kâr amaçsız)
Pazar:    IoT, gömülü, edge AI, giderek sunucu
Durum:    2025-2026'da eşik atladı
```

**2025-2026'daki kritik gelişmeler:**

```
Performans paritesi:
  Tenstorrent Ascalon-X (Jim Keller ekibi)
  ~22 SPECint2006/GHz
  → AMD Zen 5 ve ARM Neoverse V3 ile aynı seviyede
  → "RISC-V yüksek performans yapamaz" iddiası çürütüldü

RVA23 Profili:
  Zorunlu vektör uzantısı (RVV 1.0)
  FP8 ve BF16 native desteği (generative AI için)
  → Fragmentasyon sorununu büyük ölçüde çözdü
  → Ubuntu 26.04 LTS ilk olarak RVA23'ü hedefliyor

Ticari benimseme:
  Qualcomm "Snapdragon Data Center" (Ventana Veyron V2)
  32 çekirdek/chiplet, 3.8+ GHz
  → ARM'a göre %30-40 daha iyi PPA (bulut iş yükleri)

  Alibaba T-Head XuanTie C930 → sunucu sınıfı
  Çin Bilimler Akademisi projesi → veri merkezi hedefi
```

**Kalan sorunlar:**

```
1. Fragmentasyon riski:
   Özel uzantıların çokluğu → uyumsuzluk
   RVA23 baseline'ı çözdü ama custom uzantılar sorun
   "Her satıcı kendi komutlarını ekliyor"

2. Doğrulama yükü:
   Esneklik → her uygulama farklı
   → Uyumluluk testi kritik ama karmaşık
   → Gelişmiş metodoloji ve uzman ekip gerekiyor

3. Yazılım ekosistemi:
   x86/ARM'a göre hâlâ geride
   Araç desteği olgunlaşıyor ama tam değil

4. Jeopolitik risk:
   Büyük hükümetler ihracat kontrolü uygularsa
   → ISA'nın açıklığı ekosistemi ulusal hatlar boyunca
     bölünmekten tam koruyamaz
   → Çin RISC-V'yi bağımsızlık aracı olarak kullanıyor
   → ABD kısıtlama tartışıyor

5. IP endişeleri:
   ISA açık ve telifsiz, ama bazı uzantılar patentli olabilir
```

---

## Bölüm 2 — Çözülemeyen Temel Sorunlar

Bu sorunlar üç ISA'da da var ve mimari değişiklik gerektiriyor.

### Sorun 1 — Bellek Duvarı (En Kritik)

```
Sorun:
  İşlemci hızı yılda ~%50 arttı (tarihsel)
  Bellek gecikmesi yılda ~%7 iyileşti
  → Makas kapanmıyor, açılıyor

Bugünkü durum (LLM çıkarımı):
  Hesaplama: yeterli
  Bellek bant genişliği: darboğaz

  Konuşmada hesaplandı:
    400B model × 30 token/s = 2,250 GB/s gerekli
    HBM3: 900 GB/s → yetmiyor

ISA bu sorunu ÇÖZEMİYOR:
  Load/store komutu ne kadar iyi tasarlanırsa tasarlansın
  veri hâlâ DRAM'den gelmek zorunda
```

**Denenen çözümler ve sonuçları:**

```
Daha büyük cache      → alan pahalı, getiri azalan
Prefetch komutları    → erişim örüntüsü tahmin edilemiyorsa işe yaramaz
Non-temporal store    → yardımcı ama marjinal
Vektör/SIMD           → hesaplamayı hızlandırıyor, belleği değil
HBM                   → pahalı, güç aç, hâlâ DRAM

Radikal çözümler (ISA dışı):
  Taalas: ağırlıkları transistöre kazı → bellek okuma sıfır
  CIM: hesaplamayı belleğe taşı
  Ternary: veri boyutunu 8× küçült
```

### Sorun 2 — Enerji Verimliliği Duvarı

```
Dennard ölçekleme 2005'te bitti:
  Transistör küçülüyor ama güç yoğunluğu artıyor
  → "Dark silicon": çipin bir kısmı sürekli kapalı kalmalı

ISA seviyesinde çözüm yok:
  Komut çözme (decode) enerjisi sabit
  Yazmaç erişimi enerjisi sabit
  Kontrol mantığı enerjisi sabit

x86'da decode enerjisi toplam güçün %10-20'si
→ Bu tamamen "işi yapmayan" enerji
```

### Sorun 3 — Spekülatif Yürütme Güvenlik Açıkları

```
Spectre (2018) → hâlâ tam çözülmedi

Kök neden:
  Performans için spekülasyon şart
  Spekülasyon → mikroarchitectural durum değişiyor
  → Yan kanal → bilgi sızıntısı

Mevcut yaklaşım: yama üstüne yama
  Her yeni varyant → yeni azaltma
  Her azaltma → performans kaybı (%5-30)

Temel çözüm ISA'da değil, mikroarchitecture'da
Ama ISA "spekülasyon güvenli" garantisi veremiyor
```

### Sorun 4 — Kontrol Akışı Paralelliği Sınırı

```
Von Neumann modeli:
  Program = sıralı komut dizisi
  Paralellik → donanım komut sırasını yeniden düzenliyor (OoO)

Sınır:
  ILP (instruction-level parallelism) tavanı ~4-8
  Daha geniş issue → getiri azalan, karmaşıklık artan
  Dal tahmini %95+ ama kalan %5 pahalı

Sonuç:
  Tek çekirdek performansı 2010'dan beri yavaş artıyor
  Çözüm: daha fazla çekirdek → ama Amdahl yasası
```

---

## Bölüm 3 — Yeni Arayışlar

Akademi ve endüstri bu duvarları aşmak için farklı yönlerde çalışıyor.

### 3.1 Dataflow (Veri Akışı) Mimarileri

```
Fikir: komut sırası değil, veri hazır olunca yürüt

  Von Neumann: "1. komut, sonra 2., sonra 3."
  Dataflow:    "verisi hazır olan her komut çalışabilir"

Avantaj: doğal paralellik, kontrol akışı darboğazı yok
Sorun:   kontrol akışı yönetimi, veri yapıları, spekülasyon eksik

Bu sorunlar dataflow mimarilerinin
yaygınlaşmasını engelledi.
```

**Hibrit yaklaşım (en umut verici):**

```
Aynı işlemcide hem OoO hem explicit-dataflow motoru:
  → Uygulama fazına göre dinamik geçiş
  → Analiz: ideal dataflow motoru komutların
    yarısından fazlası için kârlı olabilir

Intel CSA (Configurable Spatial Accelerator):
  Temel kontrol von Neumann
  Hesaplamanın dataflow kısmı yapılandırılabilir
```

### 3.2 CGRA (Coarse-Grained Reconfigurable Array)

```
Nedir:
  Yapılandırılabilir ağla bağlı işlem elemanları (PE) dizisi
  Her PE konfigürasyonuna göre hesaplama yapıyor
  Kernel → Data Flow Graph → PE'lere eşleniyor

Konum:
  FPGA'dan kaba taneli (bit değil, kelime seviyesi)
  ASIC'ten esnek
  → Güç, performans, esneklik dengesi

Aktif araştırma alanları (2025-2026):
  Bellek darboğazı (SPM kapasitesi yetersiz kalıyor)
  Kontrol akışı eşleme zorluğu (Amdahl darboğazı)
  ISA standardizasyonu YOK → derleyici kod üretimi büyük sorun
```

**Kritik gözlem:** CGRA'lar arasında ISA standardizasyonunun
olmaması, mevcut derleyici teknolojisiyle kod üretimini
büyük bir zorluk haline getiriyor.

Bu tam olarak Volt'un adresleyebileceği bir boşluk.

### 3.3 Bellek-Hesaplama Birleşimi (CIM/PIM)

```
Von Neumann'ın temel sorunu:
  Bellek ve hesaplama ayrı → veri taşıma enerjisi

Çözüm: hesaplamayı belleğe taşı

Yaklaşımlar:
  Analog CIM   → memristör, flash (Mythic)
  Dijital CIM  → SRAM içinde hesaplama (Axelera)
  MRAM CIM     → Everspin, DoD CHEETA programı
  Fotonik CIM  → FE+EO (araştırma)

ISA sorunu:
  Bu mimariler için standart komut seti YOK
  Her üretici kendi arayüzünü tanımlıyor
  → Yazılım taşınabilirliği sıfır
```

### 3.4 Vektör-Matris Uzantıları

```
RISC-V VME (Vector-Matrix Extension):
  AI/ML iş yükleri için matris komutları
  Topluluk tarafından geliştiriliyor

ARM SME (Scalable Matrix Extension):
  Matris çarpımı native
  Apple M4'te var

Intel AMX (Advanced Matrix Extensions):
  Tile tabanlı matris işlemleri
  Sapphire Rapids'ten beri

Ortak yön: ISA'ya matris seviyesi soyutlama ekleniyor
Sorun: her biri farklı → yazılım fragmentasyonu
```

### 3.5 Model-Specific ISA (Radikal Yaklaşım)

```
Taalas modeli (AMD tarafından satın alındı, Ağustos 2026):
  ISA yok — model doğrudan devreye kazınıyor
  Llama 3.1 8B: 17,000 token/s, H200'ün 1/10 gücü

Bu bir "ISA'sızlık" yaklaşımı:
  Genel amaçlılık tamamen feda ediliyor
  Karşılığında 73× hız

Sınır: model değişince çip çöp
```

---

## Bölüm 4 — Mevcut ISA'lar İhtiyacı Karşılıyor mu?

Cevap alan bazında değişiyor:

```
ALAN                  DURUM              DEĞERLENDİRME
──────────────────────────────────────────────────────────────
Genel amaçlı CPU      ✓ Karşılıyor       x86/ARM/RISC-V yeterli
                                          Sorun ISA'da değil,
                                          bellek duvarında

Mobil/gömülü          ✓ Karşılıyor       ARM + RISC-V güçlü
                                          Güç verimliliği iyi

Sunucu                ✓ Karşılıyor       x86 + ARM + RISC-V
                                          Rekabet sağlıklı

AI eğitim             △ Kısmen           GPU ISA'ları (PTX, GCN)
                                          Standart yok, satıcıya bağlı

AI çıkarım            ✗ KARŞILAMIYOR     Bellek duvarı çözülmedi
                                          Her hızlandırıcı kendi
                                          arayüzünü tanımlıyor

Nöromorfik            ✗ KARŞILAMIYOR     Standart ISA YOK
                                          Loihi, Akida, SpiNNaker
                                          hepsi farklı

CIM/PIM               ✗ KARŞILAMIYOR     Standart YOK
                                          Yazılım taşınamıyor

CGRA                  ✗ KARŞILAMIYOR     ISA standardizasyonu yok
                                          Derleyici sorunu

Fotonik               ✗ KARŞILAMIYOR     Kavram bile yok
```

**Örüntü net:** Klasik hesaplamada ISA sorunu çözülmüş.
Yeni paradigmalarda standart bile yok.

---

## Bölüm 5 — Kırılganlık Analizi

### 5.1 Jeopolitik Kırılganlık (En Yeni ve En Ciddi)

```
x86:  ABD şirketleri (Intel, AMD) → ihracat kontrolüne tabi
ARM:  İngiltere merkezli, SoftBank (Japonya) sahipliği
      → ABD baskısına açık
RISC-V: İsviçre merkezli vakıf
      → Ama ABD kısıtlama tartışıyor

Çin'in stratejisi:
  RISC-V'yi teknolojik bağımsızlık aracı olarak kullanıyor
  XuanTie, Zhihe, Bilimler Akademisi projeleri

Risk:
  ISA'nın açık olması, ekosistemi ulusal hatlar boyunca
  bölünmekten tam olarak korumuyor
  → İki ayrı RISC-V ekosistemi (Batı/Çin) oluşabilir
```

### 5.2 Tek Nokta Bağımlılığı

```
x86:   2 şirket
ARM:   1 şirket
RISC-V: dağıtık ama RVA23 profili kritik

Her birinde farklı risk:
  x86 → oligopol, inovasyon yavaş
  ARM → sahiplik değişimi riski (Nvidia denemesi)
  RISC-V → fragmentasyon riski
```

### 5.3 Yazılım Kilitlenmesi

```
40 yıllık x86 yazılımı:
  Kaynak kodu olmayan ticari uygulamalar
  Eski derleyicilerle üretilmiş binary'ler
  → Taşınamıyor

Bu ARM ve RISC-V'nin masaüstünde yayılmasını engelliyor
Emülasyon (Rosetta 2, Prism) çözüm ama %20-30 performans kaybı
```

---

## Bölüm 6 — Volt İçin Fırsat Analizi

Bu araştırma net bir boşluk gösteriyor:

```
KLASİK ISA'LAR:            YENİ PARADİGMALAR:
  Standart var               Standart YOK
  Derleyici var              Derleyici sorunu
  Doğrulama metodolojisi var Doğrulama ad-hoc
  → Volt'a ihtiyaç az        → Volt'a ihtiyaç YÜKSEK
```

**Somut fırsatlar:**

```
1. CGRA ISA standardizasyonu:
   Literatür açıkça "ISA standardizasyonu yok, derleyici
   kod üretimi büyük zorluk" diyor
   → Volt CGRA konfigürasyonunu tip-güvenli tanımlayabilir

2. Nöromorfik ISA:
   Loihi, Akida, SpiNNaker → üçü de farklı
   → Spike<T> tipi ortak soyutlama olabilir

3. CIM/PIM arayüzü:
   Her üretici kendi komutunu tanımlıyor
   → Volt domain sistemi analog/dijital sınırı yönetebilir

4. RISC-V özel uzantı prototipi:
   custom-0..3 opcode alanları var
   → Volt tip-güvenli decoder + formal doğrulama
   → Uzantı tasarımı hızlanır

5. Fragmentasyon yönetimi:
   RISC-V'nin en büyük riski uzantı çokluğu
   → Volt uyumluluk kontrolü yapabilir
   → "Bu tasarım RVA23 uyumlu mu?" formal kanıt
```

---

## Özet

```
MEVCUT DURUM:

x86:     Reform denemesi kısmen başarısız (x86S iptal)
         APX/AVX10 devam ediyor ama gecikmiş
         Karmaşıklık + güvenlik + oligopol kırılganlıkları

ARM:     Güçlü ama telif maliyeti ve tek şirket riski
         Qualcomm davası güveni sarstı
         RISC-V baskısı artıyor

RISC-V:  2025-2026'da performans paritesi kanıtlandı
         RVA23 fragmentasyonu büyük ölçüde çözdü
         Jeopolitik risk yeni ve ciddi

ÇÖZÜLEMEYEN SORUNLAR:
  Bellek duvarı        → ISA çözemez, mimari değişimi gerekli
  Enerji verimliliği   → Dennard bitti, ISA seviyesinde çare yok
  Spekülasyon güvenliği → yama üstüne yama, temel çözüm yok
  ILP tavanı           → 4-8 sınırı aşılamıyor

YENİ ARAYIŞLAR:
  Dataflow/hibrit      → araştırma aktif, ürün az
  CGRA                 → ISA standardı YOK ← fırsat
  CIM/PIM              → standart YOK ← fırsat
  Matris uzantıları    → her satıcı farklı ← fragmentasyon
  Model-specific       → Taalas, radikal ama katı

İHTİYACI KARŞILIYOR MU:
  Klasik hesaplama:    EVET
  AI çıkarım:          HAYIR (bellek duvarı)
  Nöromorfik/CIM/CGRA: HAYIR (standart bile yok)

VOLT İÇİN SONUÇ:
  Klasik ISA alanında yer yok (çözülmüş)
  Yeni paradigmalarda standart boşluğu büyük
  → Volt'un çok paradigma vizyonu bu boşluğa denk geliyor
```
