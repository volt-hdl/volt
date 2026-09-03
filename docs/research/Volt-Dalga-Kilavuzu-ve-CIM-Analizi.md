> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Dalga Kılavuzu ve Hesaplama-İçi-Bellek — Temelden Gerçek Dünyaya

> İki kavramı sıfırdan, birbiriyle bağlantılı biçimde anlatmak
> gerekiyor. Dalga kılavuzu "hesaplamanın taşındığı yol",
> Hesaplama-İçi-Bellek ise "yolun kendisinin hesaplama yaptığı"
> paradigma. Birleşince Von Neumann'ın temel kısıtını aşan
> bir mimari ortaya çıkıyor.

---

## Bölüm 1 — Dalga Kılavuzu Nedir

### 1.1 Sezgisel Başlangıç

```
Elektrik kablosu ne yapar?
  → Elektronu bir yerden diğerine götürür
  → Elektron dışarı kaçmasın diye yalıtkan kaplama var

Dalga kılavuzu aynı işi ışık için yapar:
  → Fotonu bir yerden diğerine götürür
  → Foton dışarı kaçmasın diye farklı kırılma indisli katman var

Temel soru: Foton neden dışarı kaçmaz?
```

### 1.2 Toplam İç Yansıma — Işığı Hapseden Fizik

```
Işık iki malzeme sınırına çarptığında ne olur?

Hava (n=1.0) → Cam (n=1.5):
  Işık kırılarak girer (Snell yasası)

Cam (n=1.5) → Hava (n=1.0):
  Küçük açı: kırılarak çıkar
  Büyük açı: TAM YANSIR, dışarı çıkamaz!

Kritik açı:
  θ_c = arcsin(n₂/n₁) = arcsin(1.0/1.5) = 41.8°

  θ < θ_c: ışık çıkar (kırılma)
  θ > θ_c: ışık tamamen geri döner (hapsedildi!)

Dalga kılavuzu bu fizikten yararlanır:
  ┌──────────────────────────────────────┐
  │  Kaplama (n düşük, örn. SiO₂=1.44) │
  ├──────────────────────────────────────┤
  │  Çekirdek (n yüksek, örn. Si=3.48) │ ← Işık burada
  ├──────────────────────────────────────┤
  │  Kaplama (n düşük)                  │
  └──────────────────────────────────────┘

  Si/SiO₂ kritik açı: arcsin(1.44/3.48) = 24.5°
  Işık 24.5°'den dik açıyla çarparsa → tamamen yansır
  → Ileri doğru ilerler ama dışarı kaçamaz
  → Hapsedilmiş ışık dalga kılavuzunda ilerliyor
```

### 1.3 Dalga Kılavuzu Türleri

**Optik Fiber:**

```
Dünya çapındaki internet altyapısı:
  SiO₂ çekirdek (n≈1.468) + SiO₂ kaplama (n≈1.465)
  Çap: 8-50 μm çekirdek, 125 μm toplam

  Kayıp: 0.2 dB/km (1550 nm'de) → muhteşem düşük
  1000 km'de yalnızca %5 güç kaybı

  Kullanım: transatlantik kablo, internet omurgası
  İçerik: bu belgede "dalga kılavuzu" bu değil
```

**Silikon Fotonik Dalga Kılavuzu (konumuz):**

```
Çip üzerinde, mikron/nanometre ölçeğinde:
  Si çekirdek: 220 nm yüksek × 500 nm geniş
  SiO₂ kaplama üstte ve altta

  ┌─────────────────┐
  │   SiO₂ (üst)   │
  ├─────────────────┤
  │   Si ←  220nm  │ ← ışık buraya hapsedildi
  ├─────────────────┤
  │   SiO₂ (alt)   │
  └─────────────────┘
  ←    500 nm     →

  Kayıp: 0.5-3 dB/cm (fiber'dan çok kötü ama çip için yeterli)
  Bükülme yarıçapı: ~5 μm (fiber'ın 1000x daha sıkı)
  Üretim: standart CMOS fabrika → ucuz, kitlesel üretim

  Neden önemli:
  → Elektroniği bilgisayar çipine nasıl sığdırdıysak,
    fotonik bileşenleri de aynı şekilde sığdırıyoruz
  → Aynı silikon wafer: transistör + dalga kılavuzu birlikte
```

**LiNbO₃ İnce Film Dalga Kılavuzu (TFLN):**

```
FE+EO RAM için özel malzeme:
  LiNbO₃ çekirdek: 300-600 nm
  SiO₂ kaplama

  Kayıp: 0.027 dB/cm (rekor düşük!)
  EO katsayısı: r₃₃ = 30.9 pm/V (çok yüksek)
  Bant genişliği: 100+ GHz modülasyon

  Neden özel:
  → Ferroelektrik (depolar) + EO (okur) + dalga kılavuzu (taşır)
  → Üç işlev tek malzemede
```

### 1.4 Dalga Kılavuzunda Işık Nasıl Manipüle Edilir

```
Dalga kılavuzu bir "ışık yolu" — üzerinde çeşitli bileşenler:

Yarıklayıcı (Splitter/Coupler):
  ──────────────┐
                ├──── (50% güç)
  ──────────────┘
                └──── (50% güç)
  Tek yoldan iki yol → dalga kılavuzları birbirine yaklaştırılınca
  ışık tünelleme ile paylaşılır (evanescent coupling)

MZI (Mach-Zehnder İnterferometresi):
  Girdi──┬────────────────┬──Çıktı
         │ kol 1          │
         │ kol 2 (faz ±Δφ)│
         └────────────────┘
  İki kolun faz farkı → girişim → çıkış güç değişimi
  Δφ = 0   → yapıcı girişim → tam çıkış
  Δφ = π   → yokedici girişim → sıfır çıkış
  Δφ = π/2 → yarı çıkış
  → Bu FE+EO'nun okuma mekanizması!

Ring Resonatör:
  ──────────────────────────────
          ╭──────────╮
          │  halka   │  ← rezonant dalga boyu tutulur
          ╰──────────╯
  Belirli dalga boyu → halka içinde döner (depolanır)
  Diğer dalga boyları → geçer
  → Dalga boyu seçici filtre → WDM anahtarı

Grating Coupler (Izgara Kuplörü):
  Çipten dışarı ışık çıkarma noktası:
  Lazer veya fiber → çipe bağlantı
  → Dış dünya ile arayüz
```

---

## Bölüm 2 — Hesaplama-İçi-Bellek (CIM) Nedir

### 2.1 Von Neumann'ın Temel Sorunu — Görsel

```
Bugünkü bilgisayar:

Bellek (RAM):                    İşlemci (CPU/GPU):
┌──────────────┐                 ┌──────────────┐
│ Ağırlık w₁  │                 │              │
│ Ağırlık w₂  │ ←─── GELİYOR ──│  hesapla     │
│ Ağırlık w₃  │ ─── GİDİYOR ──►│  w × x       │
│ ...          │                 │              │
│ 140 GB LLM  │                 └──────────────┘
└──────────────┘
       ↑
   Her token üretiminde
   tüm model buradan okunur
   (bant genişliği darboğazı)

Enerji nereye gidiyor?
  %70: veriyi taşımak (veri yolu)
  %20: veriyi okumak (bellek)
  %10: gerçek hesaplama
  
→ Hesaplama için harcanan enerji yalnızca %10!
→ %90 sadece veri taşımak için
```

### 2.2 CIM'in Fikri — Hesaplamayı Veriye Götür

```
CIM yaklaşımı:

Bellek + İşlemci (aynı yerde):
┌─────────────────────────────────────┐
│ Ağırlık w₁  × Giriş x₁ = Sonuç    │
│ Ağırlık w₂  × Giriş x₂ = Sonuç    │
│ Ağırlık w₃  × Giriş x₃ = Sonuç    │
│ ...          ...          ...       │
│ Hepsi AYNI ANDA, AYNI YERDE        │
└─────────────────────────────────────┘

Enerji nereye gidiyor?
  %90: gerçek hesaplama
  %10: giriş/çıkış
  %0: veri taşıma (taşıma yok!)
```

### 2.3 Analog CIM'in Sihri — Ohm ve Kirchhoff

```
Analog crossbar dizisi (en yaygın CIM):

          x₁    x₂    x₃    (giriş voltajları)
          │     │     │
    ──────┼─────┼─────┼─────────
    │     │     │     │         │
    │  G₁₁│  G₁₂│  G₁₃│         │ ← ağırlıklar
    │     │     │     │         │   (iletkenlik)
    ├─────┴─────┴─────┴─────────┤
    │  G₂₁   G₂₂   G₂₃         │
    │                           │
    ├───────────────────────────┤
    │  G₃₁   G₃₂   G₃₃         │
    │                           │
    └───────────────────────────┘
          │     │     │
          I₁    I₂    I₃    (çıkış akımları)

Ohm yasası: I = V × G  (yani: giriş × ağırlık = çıkış)
Kirchhoff: I_toplam = Σᵢ (Vᵢ × Gᵢⱼ)

Bu formül TAM OLARAK matris-vektör çarpımıdır:
  y = W × x

  Yani fizik yasaları matris çarpımı yapıyor!
  Elektronik devre yok, saat döngüsü yok, hesaplama yok
  → Voltaj uygula → akım ölç → sonuç hazır

Hız: elektronların hareketi → nanosaniyeler
Enerji: sadece akım × voltaj × süre
  → ~0.01-0.1 fJ/işlem (dijital'in 1000x altında!)
```

### 2.4 CIM Türleri

```
Dijital CIM (D-CIM):
  SRAM hücresi yanına AND/OR kapı ekle
  Bitsel operasyonlar bellekte
  Kullanım: binary sinir ağları, arama

Analog CIM (A-CIM):
  Memristör/RRAM crossbar: Ohm+Kirchhoff MAC
  PCM crossbar: optik ağırlık okuma
  FE+EO: ferroelektrik + Pockels etkisi
  En güçlü: gerçek analog fizik hesaplama

Fotonik CIM (P-CIM):
  Işık ağırlıkla etkileşir
  Dalga süperpozisyonu toplama yapar
  MZI çarpma yapar
  WDM paralel kanallar
  En verimli: ışık hızında, minimum enerji
```

---

## Bölüm 3 — FE+EO CIM'in Tam Mekanizması

```
Her şeyin birleştiği yer:

Adım 1: Ağırlık yükleme (bir kez)
  Elektriksel voltaj → FE polarizasyon → dalga kılavuzunda saklandı
  LiNbO₃ dalga kılavuzu: ağırlık = iç elektrik alan

Adım 2: Giriş (her hesaplamada)
  Lazer → ızgara kuplörü → dalga kılavuzu
  Giriş değeri = optik genlik veya faz

Adım 3: Hesaplama (fizik yapıyor)
  Optik sinyal + FE iç alan:
  → Pockels etkisi: Δn = r₃₃ × E_FE
  → Faz kayması: ΔΦ = (2π/λ) × Δn × L
  → MZI çıkışı: I_out = I_in × cos²(ΔΦ/2)

  Bu tam olarak: çıkış = giriş × f(ağırlık)
  Yani: y = f(x × w)   ← matris çarpımı!

Adım 4: Çıkış okuma
  Fotodedektör → akım → ADC → dijital sonuç

WDM ile paralellik:
  λ₁: ağırlık satırı 1 × giriş → sonuç 1
  λ₂: ağırlık satırı 2 × giriş → sonuç 2
  ...
  λ₆₄: ağırlık satırı 64 × giriş → sonuç 64
  Hepsi AYNI ANDA, aynı dalga kılavuzunda!
```

---

## Bölüm 4 — Gerçek Dünyada Kullanım Alanları

### 4.1 Yapay Zeka Çıkarımı — Ana Kullanım

**Büyük Dil Modelleri (LLM):**

```
GPT-4 / Llama gibi modeller her token üretiminde:
  70 milyar ağırlık × giriş vektörü = matris çarpımı
  Bu işlem saniyede ~30 kez tekrarlanır

Bugünkü maliyet:
  GPU: ~300W, 140 GB DRAM bant genişliği şart
  Veri merkezi: binlerce GPU → megawatt güç

FE+EO CIM ile:
  Ağırlıklar dalga kılavuzlarına yüklü (ternary, 8.75 GB)
  Her token: giriş fotonu → çıkış fotonu → sonuç
  Güç: ~5-15W (GPU'nun 20-60x altında)
  Bant genişliği: WDM → veri taşıma yok

Gerçekçi senaryo (2030+):
  Laptop portuna takılan SoC (konuşmamızın başı!)
  Dahili: FE+EO CIM + nöromorfik ön işleme
  Yerel LLM: 7-13B model, gerçek zamanlı, pil ile günlerce
```

**Görüntü Tanıma (CNN):**

```
Gözetim kamerası örneği:
  24 saat/7 gün yüz tanıma, nesne tespiti
  GPU ile: sürekli yüksek güç → pahalı elektrik

FE+EO CIM ile:
  Nöromorfik ön işleme: değişiklik yoksa sıfır güç
  Değişiklik → CIM aktive: anlık matris çarpımı
  Güç profili: ortalama ~1mW (hareket olmadığında)
  vs GPU: sürekli 50W+

Gerçek ürün yolu:
  Bugün: Ambarella gibi şirketler ASIC ile yapıyor
  Gelecek: Fotonik CIM → 100x daha verimli
```

### 4.2 Tıp ve Biyomedikal

**Sürekli Sağlık İzleme:**

```
Wearable cihaz (akıllı saat, yama):
  24 saat EKG + SpO2 + EEG
  Şu an: ~100mW → 2 günde pil biter
  
FE+EO CIM entegrasyonuyla:
  Nöromorfik ön işleme: "anormal ritim var mı?"
  → Yok → 0.5mW (aylarca pil)
  → Var → CIM aktivasyon: "atrial fibrilasyon mu?"
  → Evet → alarm

Özel avantaj:
  Veri cihazdan çıkmaz → gizlilik
  Bulut bağlantısı gerekmez → güvenilirlik
  Gerçek zamanlı → gecikme yok

Klinik uygulamalar:
  Epilepsi tahmini: nöbet öncesi beyin sinyali kalıbı
  Diyabet: sürekli kan şekeri tahmini (CGM verileri)
  Parkinson: titreme kalıbı analizi
  Erken uyarı sistemleri → hayat kurtaran
```

**Tıbbi Görüntüleme:**

```
MRI/CT görüntü analizi:
  Radyolog başına ~50 görüntü/saat kapasitesi
  Dünyada radyolog açığı: milyonlarca

FE+EO CIM ile:
  Kanser tespit modeli → anlık analiz
  ~0.1ms/görüntü (GPU: ~10ms)
  10,000 görüntü/saat
  Doktora sadece şüpheli vakalar gelir

Implantable cihazlar:
  Beyin-bilgisayar arayüzü (BCI):
  1024 elektrod → anlık sinyal işleme
  Güç: <10mW (beyin içinde pil yok!)
  FE+EO CIM: pikojoul/işlem → ideal
```

### 4.3 Otonom Araçlar ve Robotik

**Otonom Araç Algılama:**

```
Gerçek zamanlı kısıt:
  Araç 100 km/h → 10 ms'de 28 cm ilerler
  Karar verme süresi: <5 ms şart

Bugünkü çözüm:
  NVIDIA Orin: 254 TOPS, ~60W
  HBM bant genişliği: ~204 GB/s
  Isı yönetimi: büyük soğutucu

FE+EO CIM ile:
  LiDAR + kamera → nöromorfik ön işleme → CIM çıkarım
  Gecikme: ~0.1 ms (ışık hızı sınırlı)
  Güç: ~5W
  Isı: minimal
  
Güvenlik boyutu:
  Volt formal doğrulama → "tepki süresi < 5ms" kanıtlanabilir
  ISO 26262 ASIL-D → sertifikasyon maliyeti düşer
```

**Endüstriyel Robotik:**

```
Fabrika robotu — gerçek zamanlı kavrama:
  Çeşitli nesneler → tanı → kavrama planı
  Şu an: kablolu bağlantı + merkezi bilgisayar

FE+EO CIM gömülü:
  Robot elinde → gecikme yok
  Dokunma geri bildirimi → nöromorfik → anlık tepki
  Güç: pil ile saatler → kabloya gerek yok
```

### 4.4 Enerji ve Altyapı

**Elektrik Şebekesi Anomali Tespiti:**

```
Akıllı şebeke: milyonlarca sensör
  Her sensör: akım, gerilim, harmonik → sürekli veri
  Arıza tespiti: ms içinde şalter açılmalı

Bugün: merkezi veri toplama → analiz → komut → ms gecikme
FE+EO CIM: her sensörde yerel AI → μs tepki

Örnek:
  Transformatör arızası → 500μs'de tespit
  Şalter açılır → kısa devre yayılmaz
  Bugün: ~50ms → kısa devre hasarı büyük
```

**Yenilenebilir Enerji Optimizasyonu:**

```
Rüzgar türbini:
  Rüzgar değişimi → kanat açısı optimizasyonu
  Her ms'de yeniden hesaplama
  
FE+EO CIM → türbinde gömülü:
  Hava akışı sensörleri → anlık optimizasyon
  Bulut bağlantısı gerekmez
  Enerji verimi +%5 → büyük parklarda milyonlarca dolarlık kazanç
```

### 4.5 Bilim ve Araştırma

**Genomik ve Biyoinformatik:**

```
DNA dizi eşleştirme (Smith-Waterman):
  İnsan genomu: 3 milyar baz çifti
  Veri tabanı: binlerce referans genom
  
  Matematiksel özü: dinamik programlama matrisleri
  → Matris çarpımına indirgenir

FE+EO CIM ile:
  Paralel dizi karşılaştırma: WDM kanal başına bir referans
  Hız: günler → saatler
  Nadir hastalık teşhisi → kişiselleştirilmiş ilaç

Protein yapı tahmini:
  AlphaFold tarzı model → sürekli çalışma
  FE+EO: protein-ilaç etkileşimi anlık simülasyon
```

**Parçacık Fiziği:**

```
LHC (CERN) tetikleyici sistemi:
  40 milyon çarpışma/saniye → hangisi ilginç?
  Anlık karar: <4μs (ışık zamanı!)
  
  Bugün: FPGAs + özel donanım, devasa güç
  FE+EO CIM:
  → Daha hızlı karar (~1μs)
  → Daha az güç (MW ölçeğinde tasarruf)
  → Daha hassas seçim → daha iyi fizik
```

### 4.6 Savunma ve Uzay

**Uydu Veri İşleme:**

```
Uzaktan algılama uydusu:
  Yeryüzü görüntüsü: TB/gün veri
  İndirme: bant genişliği sınırlı (GB/gün)
  
FE+EO CIM uydu içinde:
  Ham görüntü → anlık sınıflandırma
  "Değişiklik var mı?" → evet → indir
  "Değişiklik yok" → atla
  İndirilen veri: %99 azalır
  Güç: güneş enerjisi sınırlı → verimli şart
```

**Radar Sinyal İşleme:**

```
AESA radar (Active Electronically Scanned Array):
  1000+ anten elementi → anlık faz analizi
  Hedef tanıma: ms içinde

FE+EO CIM:
  Her anten elementi → yerel küçük CIM
  Paralel işleme → gecikme minimax
  Güç: kritik (uçak/gemi bütçesi)
```

---

## Bölüm 5 — Dalga Kılavuzu + CIM Birlikte: Tam Sistem

```
Gerçek bir FE+EO CIM çipinin içi:

  ┌────────────────────────────────────────────────────────┐
  │                    FE+EO CIM Çipi                      │
  │                                                        │
  │  Izgara kuplörler (lazer girişi):                      │
  │  λ₁ →─────┐  λ₂ →─────┐  ... λ₆₄ →─────┐           │
  │            │            │                 │            │
  │  ┌─────────▼────────────▼─────────────────▼────────┐  │
  │  │          Dalga Kılavuzu Ağı                       │  │
  │  │                                                   │  │
  │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐       │  │
  │  │  │ FE+EO    │  │ FE+EO    │  │ FE+EO    │  ...  │  │
  │  │  │ Hücre 1  │  │ Hücre 2  │  │ Hücre 3  │       │  │
  │  │  │ w₁₁      │  │ w₁₂      │  │ w₁₃      │       │  │
  │  │  └────┬─────┘  └────┬─────┘  └────┬─────┘       │  │
  │  │       │              │              │              │  │
  │  │  MZI (çarpma)  MZI         MZI                   │  │
  │  │       │              │              │              │  │
  │  │  Dalga kılavuzu süperpozisyonu (toplama)          │  │
  │  │       │              │              │              │  │
  │  └───────▼──────────────▼──────────────▼─────────────┘  │
  │          │              │              │                  │
  │  Fotodedektör  Fotodedektör  Fotodedektör                │
  │          │              │              │                  │
  │  ADC → dijital sonuç                                     │
  │                                                          │
  │  FE yazma (elektriksel, ayrı kontrol):                  │
  │  DAC → HZO/TFLN polarizasyon ayarı                      │
  └────────────────────────────────────────────────────────┘
```

---

## Bölüm 6 — Volt Ekosisteminde Dalga Kılavuzu ve CIM

```volt
// Dalga kılavuzu: Volt'ta bir "bağlantı tipi"
type Waveguide<const λ_nm: u32, const loss_dB_cm: f32> = {
    mode      : WaveguideMode,  // single/multi-mode
    width     : nm,
    height    : nm,
    material  : WGMaterial,     // Si, SiN, LiNbO3, ...
}

// WDM kanal: dalga kılavuzunda çoğullama
type WDMChannel<const λ_nm: u32> = OpticalSignal @Photonic{λ_nm}

// FE+EO CIM hücresi: tüm konseptin birleşimi
module FEO_CIM_Cell<const λ_nm: u32> {
    // Giriş: farklı dalga boyunda optik sinyal
    in  optical_in  : WDMChannel<λ_nm>
    // Ağırlık: ferroelektrik polarizasyon (kalıcı)
    in  fe_weight   : FEState   @Ferroelectric
    // Çıkış: modüle edilmiş optik sinyal
    out optical_out : WDMChannel<λ_nm>

    // Fizik hesaplamayı yapıyor:
    // optical_out = optical_in × f(fe_weight)
    let delta_n = pockels(fe_weight, r33=30.9.pm_per_V)
    let delta_phi = phase_shift(delta_n, λ_nm, length=100.um)
    optical_out = mzi_output(optical_in, delta_phi)

    // Maliyet: ışık hızı, sıfır dijital hesaplama
    @cost(energy=0.01.fJ, latency=0.5.ps)
    #[non_destructive_weight_read]  // ağırlık bozulmaz
}

// Tam bir matris satırı: WDM paralel
module FEO_CIM_Row<const N: usize, const λ_base: u32> {
    in  inputs   : [WDMChannel<λ_base + i*100GHz>; N]
    in  weights  : [FEState; N]
    out result   : PhotoCurrent  // dalga süperpozisyonu = toplama

    // N paralel çarpma + dalga süperpozisyonu ile toplama
    // Tüm N işlem AYNI ANDA, AYNI dalga kılavuzunda
    @cost(energy=N * 0.01.fJ, latency=0.5.ps)  // seri değil, paralel!
    @throughput(N.multiply_accumulate_per_halfps)
}

// Gerçek dünya uygulaması: LLM attention katmanı
module PhotonicAttentionLayer<const D: usize, const H: usize> {
    in  query  : Tensor<Trit, H, D>   @Digital
    in  key    : Tensor<Trit, H, D>   @Digital  (FE'ye yüklü)
    out scores : Tensor<f32, H, H>    @Digital

    // Dönüşüm: dijital → optik
    let q_optical = ElectroOptic.encode(query)

    // FE+EO CIM matris çarpımı
    // Key ağırlıkları zaten FE hücrelerinde!
    let s_optical = FEO_CIM_Matrix<H, D>(q_optical, key_fe)

    // Optik → dijital
    scores = PhotoDetector.decode(s_optical)

    // Volt garantisi
    @cost(energy = H * D * 0.01.fJ)  // enerji hesabı görünür
    invariant: latency < 1.ns          // ışık hızı sınırlı
}
```

---

## Özet: Neden Her Şey Birbirine Bağlı

```
Dalga kılavuzu:
  "Işığı bir yerden diğerine götüren yapı"
  → Toplam iç yansıma ile fotonu hapseder
  → Çipte: Si, SiN, LiNbO₃ nano-yapılar
  → Hesaplama için: hem taşıma yolu hem etkileşim yeri

CIM:
  "Hesaplamayı verinin yanına götür"
  → Veri taşıma = darboğaz → ortadan kaldır
  → Analog fizik (Ohm, Kirchhoff, Pockels) hesaplar
  → Matris çarpımı = elektrik/optik yasaları

FE+EO CIM:
  "Ferroelektrik ağırlık × Işık girişi = Pockels çarpma"
  → Ağırlık: kalıcı, enerji-serbest
  → Çarpma: fizik yasası, ışık hızında
  → Toplama: dalga süperpozisyonu, ücretsiz
  → Von Neumann tamamen aşıldı

Gerçek dünya etkisi:
  LLM: 60W GPU → 5W FE+EO CIM (12x verimli)
  Wearable AI: günler pil → aylarca pil
  Otonom araç: ms karar → μs karar
  Tıp: yavaş analiz → anlık, güvenilir, gizli
  Bilim: günler simülasyon → saatler

Tek cümleyle:
  Dalga kılavuzu ışığı taşır;
  FE+EO CIM'de ışık hem taşınır hem hesaplar;
  sonuç: veri hareket etmeden matris çarpımı —
  bu Von Neumann'ın 75 yıllık darboğazının
  fizik yasalarıyla aşılmasıdır.
```
