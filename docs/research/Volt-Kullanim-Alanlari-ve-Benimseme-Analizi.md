> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Volt Ekosistemi — Kullanım Alanları ve Benimseme Analizi

> Bu belge, Volt ekosisteminin parça parça ve bütünsel kullanım senaryolarını,
> ilk benimseyenlerin kimler olduğunu ve neden onların ilk benimseyeceğini analiz eder.
> Katman bağımsız olarak değerlendirilebilir — her katmanın kendi benimseme eşiği var.

---

## Bölüm 1 — Katmana Göre Kullanım Alanları

### Katman 1 — Volt MVP (Dijital RTL, Bugün Kullanılabilir)

Bu katman mevcut Verilog/VHDL/Chisel'ın doğrudan ikamesi. Sentezlenebilir
SystemVerilog ürettiği için mevcut araç zincirine sürtünmesiz girer.

**Kullanım alanı 1 — FPGA Geliştirme**

```
Kim: Bağımsız geliştiriciler, hobiciler, küçük ekipler
Ne: DSP, motor kontrolü, protokol implementasyonu,
    retro bilgisayar emülasyonu, SDR
Neden Volt: CDC ve latch hatalarını derleme zamanında yakalar
            → Haftalar süren hata ayıklamayı önler
            → Öğrenme eğrisi Verilog'dan düşük
Örnek: SDR alıcısı tasarlayan bir radyo amatörü
       AXI-Lite slave yazan bir gömülü geliştirici
```

**Kullanım alanı 2 — Üniversite Eğitimi**

```
Kim: Bilgisayar mühendisliği, elektrik mühendisliği bölümleri
Ne: Dijital tasarım dersi, bilgisayar mimarisi laboratuvarı
Neden Volt: Öğrencinin en çok yaptığı hataları (latch, CDC,
            blocking/non-blocking karışıklığı) derleme zamanında
            öğretici mesajlarla gösterir
            → Asistan saatlerini azaltır
            → "Neden çalışmıyor?" yerine "Ne öğrendim?" sorusu
Örnek: RISC-V işlemci yazan öğrenci ekibi
       Pipelined ALU tasarlayan lisans öğrencisi
```

**Kullanım alanı 3 — Açık Kaynak Donanım**

```
Kim: RISC-V topluluğu, OpenTitan, eFabless, Chips Alliance
Ne: Açık kaynak işlemci çekirdeği, güvenlik primitifleri,
    platform bağımsız IP blokları
Neden Volt: Açık kaynak derleyici + standart SV çıktısı
            → Satıcı bağımsız, araç bağımsız
            → Katkı süreci temiz (tip sistemi PR kalitesini artırır)
Örnek: PULP Platform (ETH Zürich RISC-V ekibi) benzeri gruplar
       OpenHW Group üyeleri
```

**Kullanım alanı 4 — ASIC Doğrulama Ekipleri**

```
Kim: Orta ölçekli ASIC şirketleri (chip startup'ları)
Ne: RTL doğrulama, formal property checking,
    CDC analizi, lint
Neden Volt: assert/invariant dilde birinci sınıf
            → Doğrulama mühendisi ayrı araç öğrenmez
            → Formal motorlara (SymbiYosys) doğrudan köprü
            → Sim/sentez uyumsuzluğu yapısal olarak imkânsız
Örnek: AI çip startup'ı (Tenstorrent, Untether benzeri)
       Automotive chip (radar, LiDAR işlemcisi)
```

---

### Katman 2 — Volt v1 (ML PPA, Python API, Timeline)

**Kullanım alanı 5 — AI Hızlandırıcı Tasarımı**

```
Kim: AI donanım startup'ları, büyük şirketlerin custom chip ekipleri
Ne: Sistolik dizi, attention mekanizması donanımı,
    mixed-precision hesaplama birimi
Neden Volt v1: ML PPA motoru → sentez beklemeden anlık alan/güç tahmin
               Python API → parametrik mimari keşfi (NAS için)
               Timeline L2 → pipeline tehlikesi derleme zamanı
Örnek: Özel LLM çıkarım çipi tasarlayan startup
       Datacenter inference accelerator
```

**Kullanım alanı 6 — Akademik Araştırma**

```
Kim: Bilgisayar mimarisi araştırmacıları, EDA araştırmacıları
Ne: Yeni mimari prototipleme, donanım-yazılım co-design,
    tasarım uzayı keşfi (design space exploration)
Neden Volt v1: Parametrik jeneratörler + derleme zamanı hesaplama
               → Binlerce tasarım noktasını otomatik süpür
               → Her nokta için PPA raporu
Örnek: MICRO/ISCA/HPCA bildirisi için yeni bellek hiyerarşisi
       FPGA optimizasyon makalesi
```

---

### Katman 3 — Volt v2 (Nöromorfik, Spike<T>)

**Kullanım alanı 7 — Edge AI ve IoT**

```
Kim: Akıllı sensör şirketleri, giyilebilir teknoloji,
     endüstriyel IoT
Ne: Sürekli izleme, anomali tespiti, uyandırma kelimesi,
    gerçek zamanlı duyusal işleme
Neden Volt v2: Spike<Trit> + on spike semantiği
               → Olay olmadığında güç sıfır
               → mW seviyesinde AI pil ömrü
Örnek: Fabrika makinesi titreşim anomali dedektörü
       Kalp ritmi izleme bandı (günlerce pil)
       Akıllı kapı güvenlik sistemi
```

**Kullanım alanı 8 — Robotik**

```
Kim: Otonom araç şirketleri, endüstriyel robot üreticileri,
     drone şirketleri
Ne: Gerçek zamanlı sensör füzyonu, refleks kontrolü,
    engel kaçınma, propriosepsiyon
Neden Volt v2: Spike zamanlaması tip sisteminde
               → Sensör gecikmeleri formal doğrulanabilir
               → Nöromorfik refleks + dijital planlama ayrımı
               CDC köprüsüyle güvenli
Örnek: Drone'un engel kaçınma refleksi (mikrosaniye)
       Robot el parmak dokunma geri bildirimi
```

**Kullanım alanı 9 — Nörobilim Araştırması**

```
Kim: Üniversite nörobilim laboratuvarları, beyin-bilgisayar
     arayüzü şirketleri (Neuralink, Synchron benzeri)
Ne: Nöral kayıt işleme, spike sıralama, BCI sinyal zinciri
Neden Volt v2: Spike<T> biyolojik sinyali doğrudan modeller
               → Nörobilimci HDL bilgisine gerek duymaz
               NeuroLang → Volt → FPGA boru hattı
Örnek: 1024 kanallı nöral kayıt işlemcisi
       Epilepsi uyarı sistemi (implantable)
```

---

### Katman 4 — Volt v3 (Fotonik Ternary)

**Kullanım alanı 10 — Datacenter AI Çıkarımı**

```
Kim: Büyük bulut sağlayıcıları (AWS, Google, Microsoft, Meta),
     fotonik çip startup'ları (Lightmatter, Luminous)
Ne: LLM attention katmanı, büyük embedding matrisi,
    yüksek throughput düşük gecikme çıkarım
Neden Volt v3: PTrit + TernaryMVM → 3000x enerji verimliliği
               E-O-E köprüsü tip-güvenli → sessiz hata yok
               Fotonik EDA araç zinciri → tasarım otomasyonu
Örnek: GPT-4 ölçeğinde modeli 10x daha az güçle çalıştırma
       100 TOPS @ 5W (konuşmanın başındaki SoC isteği)
```

**Kullanım alanı 11 — Yüksek Performanslı Hesaplama**

```
Kim: HPC merkezleri, ulusal laboratuvarlar,
     fizik simülasyonu yapan kurumlar
Ne: Büyük matris işlemleri, kuantum kimya,
    iklim simülasyonu
Neden Volt v3: Fotonik matris çarpımı ışık hızında
               → Latency-critical hesaplama
               Ternary quantization: bellek 16x az
               → Daha büyük problem aynı bellekte
```

---

### Katman 5 — Mimari İşletim Sistemi + Hibrit SoC

**Kullanım alanı 12 — Taşınabilir AI Cihazı**

```
Kim: Bağımsız geliştiriciler, araştırmacılar, "mühendis olmayan uzmanlar"
Ne: Laptop'a takılan, yerel LLM çalıştıran SoC
    (konuşmanın başındaki istek)
Neden hibrit SoC: Nöromorfik ön filtre (0.5mW sürekli)
                  + Fotonik ternary matris (yoğun hesaplama)
                  + Scalar kontrolcü
                  → 5-15W'ta 7-13B model gerçekçi
```

**Kullanım alanı 13 — Otomotiv ve Güvenlik-Kritik**

```
Kim: Tier-1 otomotiv tedarikçileri (Bosch, Continental),
     uçuş kontrol sistemleri, medikal implant
Ne: ISO 26262 / DO-178C uyumlu çip tasarımı
Neden: Volt'un formal doğrulaması → güvenlik kanıtı
       Tek semantik model → sim = sentez = gerçek davranış
       assert/invariant → güvenlik özellikleri dilde
Örnek: ADAS sensör füzyonu çipi (ISO 26262 ASIL-D)
       Pacemaker kontrolcüsü
```

**Kullanım alanı 14 — Savunma ve Uzay**

```
Kim: Savunma sanayii, uzay ajansları, uydu şirketleri
Ne: Radyasyona dayanıklı FPGA tasarımı, güvenli iletişim,
    sinyal işleme, otonom sistem çipi
Neden: Formal doğrulama zorunlu (DO-254 gibi standartlar)
       Volt bunu dilde yapıyor → sertifikasyon maliyeti düşer
       Açık kaynak derleyici → tedarik zinciri bağımsızlığı
```

---

## Bölüm 2 — Bütünsel Kullanım: Her Şeyin Birleştiği Senaryolar

### Senaryo A — Otonom Araç Beyin Çipi

```
Görev: Kamera + LiDAR + Radar → anlık karar → kontrol sinyali

Volt ekosistemi uçtan uca:

  Kamera ham veri (sürekli, yüksek bant)
      │
      ▼ Volt v2 — Nöromorfik ön işleme
  Olay kamerası spike akışı
  "Engel var mı?" → 0.1ms, 0.5mW
      │
      ▼ Volt v3 — Fotonik ternary
  Derin ağ çıkarımı (nesne tanıma)
  PTrit matris → 1ns gecikme
      │
      ▼ Volt MVP — Dijital RTL
  Planlama ve kontrol algoritması
  CGRA dataflow → deterministik zamanlama
      │
      ▼ Mimari OS
  "Fren kararı" → hangi aktüatöre?
  Volt invariant: tepki_süresi < 5ms
      │
      ▼ ISO 26262 ASIL-D formal kanıt
  Sertifikasyon: Volt assert'leri kanıt üretiyor
```

### Senaryo B — Tıbbi Yapay Zeka Cihazı

```
Wearable sürekli sağlık izleme:

  EKG / SpO2 / EEG sensörler
      │ Volt v2: Spike kodlaması
      ▼
  Nöromorfik anomali dedektörü (mikrowatt)
  "Normal mi?" → sessiz
  "Anormal!" → dijital çekirdeği uyandr
      │ Volt MVP: CDC köprüsü tip-güvenli
      ▼
  Dijital sinyal işleme
  "Atrial fibrilasyon mu?"
      │ Volt v1: formal doğrulama
      ▼
  Karar + alarm
  invariant: yanlış_pozitif_oranı < 0.001

  Pil ömrü: haftalarca (nöromorfik çekirdek baskın)
  Sertifikasyon: IEC 62304, FDA approval
  Volt formal kanıtı sertifikasyonu hızlandırır
```

### Senaryo C — Araştırma Platformu (Üniversite + Endüstri)

```
"Volt Playground":

  Alan uzmanı (biyolog, fizikçi, ekonomist)
      │ NeuroLang yüksek seviye API
      ▼
  Topoloji tanımı (hangi nöron, hangi bağlantı)
      │ NeuroCompiler
      ▼
  Volt RTL + tip doğrulama
      │ CIRCT
      ▼
  FPGA bitstream (saatler içinde)
      │
      ▼
  Kendi özel nöromorfik hızlandırıcısı

  Bugün: Python'da yavaş simülasyon
  Volt ile: FPGA'da gerçek zamanlı, alan uzmanı HDL bilmez
```

---

## Bölüm 3 — İlk Benimseyenler: Kim, Neden, Ne Zaman

### 3.1 Benimseme Eğrisi Genel Bakış

```
                    Katman 1      Katman 2      Katman 3-5
                    (Volt MVP)    (Nöromorfik)  (Fotonik/OS)
Zaman              Bugün+12ay    2-4 yıl       5-10 yıl
─────────────────────────────────────────────────────────
İnovatörler        FPGA hobi     Loihi ekipleri Lightmatter
(%2.5)             Açık kaynak   BCI araştırma  Si-fotonik lab

Erken Benimseyenler Üniversiteler Edge AI startup Datacenter AI
(%13.5)            ASIC startup  Robotik        HPC merkezleri

Erken Çoğunluk    Orta FPGA     IoT şirketleri Bulut sağlayıcı
(%34)             ASIC ekipleri Otomotiv        (3-5 yıl sonra)

Geç Çoğunluk      Büyük FPGA    Tüketici       Geniş endüstri
(%34)             şirketleri    elektroniği     (7-10 yıl)

Geç Benimseyenler Enterprise    Medikal         Standart haline
(%16)             ASIC          Savunma         geldiğinde
```

---

### 3.2 İnovatörler — İlk 12–18 Ay

#### Grup 1: FPGA Hobici ve Bağımsız Geliştiriciler

```
Kim:
  - Amateur radio / SDR topluluğu
  - Retro donanım (MiSTer FPGA projesi gibi)
  - Bağımsız chip tasarımcıları (Tiny Tapeout katılımcıları)
  - Açık kaynak donanım katkıcıları

Neden ilk benimseyenler:
  ✓ Mevcut araç zincirine (Vivado, Quartus) zaten küskenler
  ✓ "Latch hatasını 3 günde bulduk" acısını yaşadılar
  ✓ Yeni araç denemekten çekinmiyorlar
  ✓ LSP ve playground = anında değer görüyorlar
  ✓ Topluluk olarak birbirlerine yayıyorlar (Reddit r/FPGA)

Nasıl kazanılır:
  Playground (WASM derleyici): "Volt dene, hiç kurmadan"
  r/FPGA, HackerNews: "Sensitivity list'siz HDL" başlığı
  Gerçek bir tasarım (Ethernet MAC) Volt ile yazılmış örnek
  "Verilog'dan Volt'a" geçiş rehberi

Beklenti: 500-2000 aktif kullanıcı, 12. ayda
```

#### Grup 2: Akademik Araştırmacılar

```
Kim:
  - Bilgisayar mimarisi (MICRO/ISCA/HPCA topluluğu)
  - EDA araştırmacıları (DAC/ICCAD topluluğu)
  - Nörobilim + donanım kesişimi (araştırma grupları)

Neden ilk benimseyenler:
  ✓ Yeni araç = yayın fırsatı
  ✓ Tasarım uzayı keşfi için parametrik jeneratörler şart
  ✓ Formal doğrulama → güvenilir sonuçlar → güçlü makaleler
  ✓ Öğrencilere "temiz" bir dil öğretmek istiyorlar

Nasıl kazanılır:
  DAC 2026: Volt'u tanıtan paper + tutorial
  CARRV (RISC-V mimarisi workshopu): demo
  Araştırma lisansı: ücretsiz, erken erişim
  "Volt ile X yeniden tasarlandı: Y% daha az hata" makalesi
```

#### Grup 3: AI Donanım Startup'ları

```
Kim:
  - Özel AI çipi geliştiren startup'lar
    (Etched, d-Matrix, Groq benzeri erken aşama)
  - RISC-V tabanlı AI core geliştiren ekipler
  - Tiny Tapeout / eFabless MPW katılımcıları

Neden ilk benimseyenler:
  ✓ Küçük ekip, her hata pahalı → latch/CDC garantileri kritik
  ✓ ML PPA motoru: sentez beklemeden anlık feedback
  ✓ Python API: algoritmacı ile donanım mühendisi aynı dilde
  ✓ Açık kaynak derleyici → satıcı bağımsızlığı
  ✓ Rakipten farklılaşma: "Volt ile yaptık" güvenilirlik sinyali

Nasıl kazanılır:
  YC/a16z AI donanım cohort'larıyla erken temas
  "Volt ile ilk ASIC" case study
  Hibrit broker entegrasyonu: Volt → eFabless → üretim
```

---

### 3.3 Erken Benimseyenler — 18 Ay–4 Yıl

#### Grup 4: Üniversite Müfredatı

```
Kim:
  MIT, Stanford, CMU, ETH Zürich, TU Delft
  (bilgisayar mimarisi / dijital tasarım bölümleri)

Neden kritik:
  Üniversite = 4-10 yıl sonraki endüstri benimsemesi
  Her yıl mezun olan mühendis Volt'la başladıysa
  endüstri "mevcut araçta çalışıyoruz" direncini kırar

Nasıl kazanılır:
  Ücretsiz akademik lisans (zaten açık kaynak)
  "Dijital Tasarım" ders materyali paketi
  Online playground: sınıfta kurulum gerektirmez
  Hata mesajları = öğretmen → asistan saati azalır
  İlk hedef: 5-10 üniversite, "pilot müfredat"
```

#### Grup 5: Edge AI ve IoT Şirketleri

```
Kim:
  - Akıllı kamera şirketleri (Ambarella benzeri)
  - Endüstriyel IoT (Siemens, Bosch küçük birimleri)
  - Giyilebilir teknoloji (Garmin, Polar, startup'lar)
  - Tarım teknolojisi (drone, sensör)

Neden Volt v2 (nöromorfik uzantısıyla):
  ✓ "Pil ömrü = ürün kalitesi" bu segmentte
  ✓ Nöromorfik ön filtre → standby'da mikrowatt
  ✓ Spike kodlaması dilde → özel arka uç gerekmez

Ne zaman: Volt v2 hazır olduğunda (2-3 yıl)
```

#### Grup 6: Otomotiv Tedarikçileri

```
Kim:
  Tier-1: Bosch, Continental, Aptiv, Magna
  ADAS çip startup'ları: Hailo, Mobileye rakipleri

Neden:
  ✓ ISO 26262 formal kanıt gerektiriyor
  ✓ Volt formal doğrulaması = sertifikasyon maliyeti düşer
  ✓ Sim = sentez garantisi = "güvenli davranış belirsiz değil"
  ✓ CDC güvenliği kritik (çok çekirdekli ADAS SoC)

Benimseme engeli: "araç zinciri değişikliği riski"
Aşma yolu: extern module ile mevcut IP korunur
            Volt yalnızca yeni bloklar için
            → kademeli geçiş → risk azalır
```

---

### 3.4 Erken Çoğunluk — 4–7 Yıl

Bu noktada Volt "bilinenlerden biri" olmaya başlar.

#### Grup 7: Büyük FPGA Ekipleri

```
Kim: Xilinx/AMD, Intel FPGA kullanıcı ekipleri
     Telecom (Ericsson, Nokia donanım ekipleri)
     Finansal hesaplama (HFT için FPGA)

Tetikleyici: Üniversiteden gelen yeni mühendisler
             "Verilog değil Volt öğrendim" diyor
             → Şirket adaptasyonu başlıyor
```

#### Grup 8: Nöromorfik Ekosistem

```
Kim: Intel Loihi partner ekosistemi
     IBM TrueNorth kullanıcıları
     Yeni nöromorfik chip startup'ları

Tetikleyici: NeuroCompiler Loihi 2 + FPGA hedeflerinde
             stabil olduğunda → araç eksiği kapanıyor
             → benimseme hızlanıyor
```

---

### 3.5 Geç Çoğunluk — 7–10 Yıl

```
Büyük ASIC şirketleri (Qualcomm, MediaTek, Samsung LSI)
Savunma ve uzay (Lockheed, Airbus, ESA)
Medikal cihaz (FDA sertifikasyonu gerektiren implantlar)
Fotonik chip (Si-fotonik CMOS olgunlaşınca)

Bu grubun beklentisi:
  "Herkes kullanıyor, araç eksiği yok, risk kabul edilebilir"
```

---

## Bölüm 4 — Coğrafi ve Sektörel Dağılım

### İlk Dalga Coğrafyası

```
ABD:
  Silicon Valley: AI chip startup'ları, VC fonlamalı
  Boston: medikal cihaz, biyoteknoloji
  Austin/Portland: FPGA endüstrisi

Avrupa:
  ETH Zürich, TU Delft: akademik inovatörler
  Eindhoven (ASML ekosisstemi): EDA araçları
  Berlin: savunma, otomotiv startup'ları
  Fraunhofer: endüstriyel IoT

Asya:
  Japonya: robotik, otomotiv (Toyota, Sony)
  Güney Kore: Samsung, SK Hynix → bellek+hesaplama
  Tayvan: TSMC ekosistemi, IC design house'lar
  Hindistan: VLSI mühendislik topluluğu (büyük, aktif)

Türkiye (özel not):
  Aselsan, Roketsan, STM: savunma formal doğrulama
  Üniversiteler: Bilkent, ODTÜ, İTÜ EE bölümleri
  Akinon, Peak Games: gömülü + donanım yakın start-up'lar
  EPFL/MIT mezunları dönen araştırmacılar
  → Yerel ekosistem kurulursa ilk benimseyici olabilir
```

---

## Bölüm 5 — Benimsemeyi Hızlandıran ve Yavaşlatan Faktörler

### Hızlandıranlar

```
1. Silikon hata maliyeti arttıkça:
   3nm, 2nm re-spin = 10M+ dolar
   Volt'un derleme zamanı garantileri = doğrudan ROI

2. AI donanımı demokratikleşme baskısı:
   "Mühendis olmadan tasarım" talebi büyüyor
   Volt + AI ajan = bu talebin karşısında en iyi araç

3. Enerji krizi / yeşil bilişim:
   Nöromorfik + fotonik ternary = 1000x enerji verimliliği
   Düzenleyici baskı → zorunlu geçiş

4. RISC-V + açık donanım hareketi:
   "Donanımda da açık kaynak" kültürü büyüyor
   Volt bu kültürle doğal uyumlu

5. LLM kod üretimi:
   Volt'un düzenli grameri ve güçlü tip sistemi
   → LLM daha az hata yapıyor
   → AI destekli tasarım ilk Volt'ta işe yarıyor
```

### Yavaşlatanlar

```
1. Ekosistem olgunluğu:
   Onyıllık Verilog/VHDL IP birikimi
   → "Extern module ile sarıyoruz" kısmen çözer
   → Ama tam geçiş yıllar alır

2. "Eğitim maliyeti":
   "Ekip Verilog biliyor, Volt öğreniriz ama risk var"
   → Üniversite müfredatı bu direnci 5-10 yılda kırar

3. CIRCT/MLIR bağımlılığı:
   LLVM ekosistemi hızlı değişiyor
   → Sürüm sabitleme + FFI izolasyonu kritik

4. Fotonik donanım olgunluğu:
   Si-fotonik ticari üretim 2028+ gerçekçi
   → Volt v3 donanım olmadan havada kalır
   → Lab partneri ile "co-evolution" şart

5. Büyük şirketlerin araç zinciri kilidi:
   Synopsys/Cadence bağımlılığı
   → Açık kaynak EDA + hibrit broker bu kilidi açıyor
   → Ama yavaş süreç
```

---

## Bölüm 6 — Benimseme Stratejisi: Sıralama Önemli

### Doğru Sıra

```
YANLIŞ:       ASIC ekiplerini önce ikna et
              → "Araç zinciri değişikliği riski kabul edilemez"
              → Kapı kapanır

DOĞRU:        1. FPGA hobi + açık kaynak → topluluk + kanıt
              2. Üniversite → gelecek mühendisler
              3. AI chip startup → erken referans müşteri
              4. Orta ASIC → startup başarısına bakarak
              5. Büyük ASIC → müfredattan gelen baskıyla
              6. Fotonik/Nöromorfik → donanım olgunlaşınca

Moore yasası analog:
  İlk transistör = araştırmacı oyuncağı
  10 yıl sonra = ticari bilgisayar
  20 yıl sonra = herkesin masasında
  Volt: benzer yol, daha hızlı (internet çağı)
```

### Kritik İlk Referans Noktaları

```
Her katman için "ilk önemli referans":

Volt MVP:
  → "X üniversitesi dijital tasarım dersinde Volt kullanıyor"
  → "Y açık kaynak RISC-V çekirdeği Volt'a geçti"

Volt v1:
  → "Z AI chip startup'ı Volt ile ilk silikon tapeout'unu yaptı"

Volt v2:
  → "W robotik şirketi nöromorfik FPGA prototipinde Volt kullandı"

Volt v3:
  → "V fotonik startup'ı ternary attention hızlandırıcısını
     Volt ile tasarladı, 100x enerji verimliliği gösterdi"

Her referans bir sonraki kapıyı açar.
İlk referans en değerlisidir.
```

---

## Özet: Kim, Ne Zaman, Neden

```
BUGÜN hazır, FPGA/Açık kaynak/Hobici:
  → Volt MVP: "Latch olmadan HDL" değer önerisi net
  → Araç: playground + VS Code eklentisi + geçiş rehberi

1-2 YIL içinde, Üniversite/AI startup:
  → Volt v1 + ML PPA: "Sentez beklemeden anında feedback"
  → Araç: akademik lisans + tutorial + referans tasarımlar

2-4 YIL içinde, Edge AI/Robotik:
  → Volt v2: "mW seviyesinde AI" değer önerisi net
  → Araç: NeuroCompiler + FPGA demo

4-7 YIL içinde, Otomotiv/Medikal:
  → Formal kanıt → sertifikasyon maliyeti düşer
  → Araç: formal köprü + ISO 26262 uyumluluk kılavuzu

7-10 YIL, Fotonik/Büyük ASIC/Endüstri:
  → Volt v3: Si-fotonik CMOS ile birlikte olgunlaşır
  → "Standart" haline gelmiş, aktif direnç kalmamış

TÜM SÜREÇ BOYUNCA geçerli olan tek şey:
  Volt'un tek semantik modeli ve tip sistemi
  her katmanda aynı — bu süreklilik sinerjinin garantisi.
```
