> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Dengeli Beşli (Balanced Quinary) — Ternary ile Karşılaştırma

> {-1, 0, +1} yerine {-2, -1, 0, +1, +2} kullanmak ne sağlar,
> ne kaybettirir? Cevap beklentinin tersine ilginç bir yerde bitiyor:
> 5 durum, 3 durumdan hem fiziksel hem bilgi-teorik açıdan
> bazı boyutlarda daha az verimli — ama doğru uygulamada
> daha iyi hassasiyet sunabiliyor. Fotonik için özel bir durum var.

---

## Bölüm 1 — Dengeli Beşli Nedir

### 1.1 Temel Tanım

```
Dengeli üçlü (ternary):   {-1,  0, +1}    → 3 durum
Dengeli beşli (quinary):  {-2, -1,  0, +1, +2}  → 5 durum

Her "quint" (beşli basamak) kaç bit bilgi taşır?

  İkili:    log₂(2)  = 1.000 bit/sembol
  Üçlü:     log₂(3)  = 1.585 bit/trit
  Beşli:    log₂(5)  = 2.322 bit/quint
  Dörtlü:   log₂(4)  = 2.000 bit/sembol (= 2 bit tam)
  Sekizli:  log₂(8)  = 3.000 bit/sembol (= 3 bit tam)

Beşli: ikiden fazla ama üçün tam katı değil.
```

### 1.2 Neden "Dengeli"

```
Normal beşli: {0, 1, 2, 3, 4}
  → İşaret biti ayrı gerekir
  → Asimetrik aralık

Dengeli beşli: {-2, -1, 0, +1, +2}
  → İşaret biti yok (negatif doğal)
  → Simetrik aralık
  → Sıfır ortada (yuvarlama noktası)
  → Negasyon ücretsiz: tüm rakamları çevir

Dengeli üçlü ile ortak özellikler:
  ✓ İşaret biti yok
  ✓ Negasyon: -2→+2, -1→+1, 0→0, +1→-1, +2→-2
  ✓ Sıfır doğal
  Ama: +2 ve -2 → ek karmaşıklık
```

---

## Bölüm 2 — Depolama Verimliliği: Beklentinin Tersi

### 2.1 Bit Başına Verimlilik

Fiziksel depolama tamsayı bit sayısı kullanır.
5 durum için minimum 3 bit gerekir (2² = 4 < 5 ≤ 8 = 2³):

```
Durum sayısı   Gereken bit   Verimlilik
─────────────────────────────────────────────
2  (ikili)     1 bit         2/2 = 100%  ← mükemmel
3  (üçlü)      2 bit         1.585/2 =  79.2%
4  (dörtlü)    2 bit         2.000/2 = 100%  ← mükemmel
5  (beşli)     3 bit         2.322/3 =  77.4%
7              3 bit         2.807/3 =  93.6%
8  (sekizli)   3 bit         3.000/3 = 100%  ← mükemmel
9  (dokuzlu)   4 bit         3.170/4 =  79.2%
16             4 bit         4.000/4 = 100%  ← mükemmel

Kritik gözlem:
  Üçlü: 79.2%
  Beşli: 77.4%  ← üçlüden DAHA az verimli!

2ⁿ durumlar her zaman 100% verimli (bit tam oturur)
3 = 2² - 1: iyi ama tam değil (79%)
5 = 2³ - 3: daha da kötü (77%)
```

### 2.2 400B Model Depolama Karşılaştırması

Her ağırlık bir sembol olarak saklanırsa:

```
Kesinlik       Durum  Bit/ağırlık  400B model  Alan (WDM512×3D32)
──────────────────────────────────────────────────────────────────
FP16           65536    16         800 GB        —
INT8           256       8         400 GB        —
INT4           16        4         200 GB        —
Beşli (5)       5        3         150 GB       234 cm²
Üçlü (3)        3        2         100 GB       156 cm²
İkili (2)        2        1          50 GB        78 cm²
──────────────────────────────────────────────────────────────────

Beşli: üçlüden 50 GB daha fazla → %50 büyük!
```

### 2.3 Hücre Başına Verimlilik (FE+EO Perspektifi)

```
Ama hikâye burada bitmiyor. Farklı bir bakış açısı:

"Aynı sayıda fiziksel hücre kaç ağırlık saklar?"

FE+EO hücresi 5 durumu destekliyorsa:
  Ternary hücre:  1.585 bit kapasitesi
  Quinary hücre:  2.322 bit kapasitesi
  Quinary/Ternary = 2.322 / 1.585 = 1.465x

Yani quinary hücre, ternary hücrenin 1.465x bilgisi depolar.

400B ağırlık için gereken HücRE SAYISI:
  Ternary:  400B hücre (1 trit = 1 ağırlık)
  Quinary:  400B × (log₂3 / log₂5)
           = 400B × 0.682 = 273B hücre

Quinary: %32 daha az hücre! → %32 daha az alan!

Ama fiziksel gerçeklik:
  Her hücrenin 5 kararlı durumu olması gerekiyor
  → Gürültü marjı daralıyor
  → Fabrikasyon çok daha zor

Bu ödünleşim kritik: daha az alan ama daha az güvenilir.
```

---

## Bölüm 3 — Doğruluk: Quinary Ne Kadar İyi?

### 3.1 Ağırlık Dağılımı ve Kuantizasyon Hatası

```
Nöral ağ ağırlıklarının dağılımı (tipik):

Frekans
  │         ████████
  │       ████████████
  │     ████████████████
  │   ████████████████████
  │ ████████████████████████___
  └──────────────────────────────► ağırlık değeri
    -2   -1    0    +1   +2

Gauss benzeri: çoğu ağırlık sıfıra yakın

Ternary {-1,0,+1}: ağırlık aralığını 3 bölgeye böler
  [<-0.5] → -1,  [-0.5, +0.5] → 0,  [>+0.5] → +1
  Büyük ağırlıklar (-2,-3...) hepsi -1 veya +1'e yuvarlanır
  Bilgi kaybı: yüksek

Quinary {-2,-1,0,+1,+2}: 5 bölge
  [<-1.5]→-2, [-1.5,-0.5]→-1, [-0.5,+0.5]→0,
  [+0.5,+1.5]→+1, [>+1.5]→+2
  Büyük ağırlıkları ayrıştırıyor → daha az bilgi kaybı
```

### 3.2 Doğruluk Karşılaştırması

```
ImageNet ResNet-50 doğruluğu (yaklaşık değerler):
  FP32:       76.1%  (referans)
  FP16:       76.0%  (neredeyse aynı)
  INT8:       75.8%  (-0.3%)
  INT4:       74.5%  (-1.6%)
  Quinary:    74.9%  (-1.2%)  ← INT4'ten iyi!
  Ternary:    73.5%  (-2.6%)
  Binary:     67.0%  (-9.1%)

LLM (dil modeli) için daha kritik:
  Perplexity artışı (düşük = iyi):
  FP16:     20.0 (referans)
  INT8:     20.1 (+0.5%)
  Quinary:  21.0 (+5.0%)
  Ternary:  21.5 (+7.5%)
  Binary:   45.0 (+125%)

Quinary ternaryden ~2× daha az hata üretiyor.
Ama INT4'ten hâlâ kötü (INT4: +2% ≈ 20.4 PPL)
```

### 3.3 NF-Quinary: Daha Akıllı Yerleştirme

```
NF4 (Normal Float 4) gibi, NF-quinary da mümkün:

Düzgün aralıklı quinary:
  -2, -1, 0, +1, +2  (eşit boşluk: 1.0)

Normal dağılıma göre quinary:
  5 seviye → Gauss dağılımının 1/10, 3/10, 5/10, 7/10, 9/10 yüzdeliklerinde
  ≈ {-1.28, -0.39, 0, +0.39, +1.28}

  Neden daha iyi:
  Ağırlıkların %60'ı [-0.5, +0.5] aralığında
  → Bu bölgeye daha ince ayrım → daha az hata

  Düzgün vs NF-quinary karşılaştırması:
  Düzgün: PPL = 21.0
  NF-5:   PPL = 20.6  (daha iyi)
  INT4:   PPL = 20.4  (hâlâ biraz önde)
```

---

## Bölüm 4 — Fiziksel Uygulama: Her Malzeme İçin

### 4.1 PCM (Faz Değişimli Malzeme) — En Uyumlu

```
GST'de 5 kristalinleşme seviyesi:

  %0   kristalin → amorf   → quint = -2
  %25  kristalin → az kr.  → quint = -1
  %50  kristalin → yarı kr.→ quint =  0
  %75  kristalin → çok kr. → quint = +1
  %100 kristalin → tam kr. → quint = +2

Oxford gösterimi (2019): GST'de 8 seviye başarılı
5 seviye: 8'den daha kolay → mümkün

Gürültü marjı karşılaştırması:
  100% aralık / (N-1) pencere sayısı:
  Ternary:  100 / 2 = 50% pencere → 25% marj
  Quinary:  100 / 4 = 25% pencere → 12.5% marj ← yarısı!

Drift etkisi (kısmi kristalinleşme kayması):
  Ternary trit=0: %50 ± 15% tolerans (güvenli pencere 30%)
  Quinary quint=0: %50 ± 6% tolerans (çok dar!)

PCM quinary için kritik sorun: drift toleransı çok az
Çözüm: aktif kalibrasyon + geniş pencere tasarımı
```

### 4.2 FeFET — En Zor

```
Standart ferroelektrik: 2 kararlı polarizasyon (↑ ve ↓)
Kısmi polarizasyon = 3. durum (zor ama mümkün)
5 durum = çok zor

Seçenek 1 — Kademeli polarizasyon:
  ↑↑↑↑↑ = +2 (tam yukarı)
  ↑↑↑↓↓ = +1 (çoğu yukarı)
  ↑↑↓↓↓ = 0  (dengeli)
  ↑↓↓↓↓ = -1 (çoğu aşağı)
  ↓↓↓↓↓ = -2 (tam aşağı)
  → Kısmi domain anahtarlaması → çok hassas kontrol gerekir

Seçenek 2 — Çok domainli ferroelektrik:
  YMnO₃, BiFeO₃: birden fazla polarizasyon oryantasyonu
  → Doğal 6 domain tipi → bazı kombinasyonlar 5 seviye verebilir
  → CMOS entegrasyonu çok zor

Seçenek 3 — Çift FeFET:
  İki ternary FeFET → kombinasyon
  3 × 3 = 9 durum → 5 seçilir
  → Alan iki kat artıyor → avantaj yok

Sonuç: FeFET için quinary ternaryden çok daha zor.
Ternary FeFET'in en büyük avantajı 3 doğal durumu.
```

### 4.3 Fotonik MZI — Özel Bir Fırsat

```
MZI transfer fonksiyonu (kompleks alan):
  E_out = E_in × exp(i × ΔΦ)

Şiddet (intensity):
  I_out = I_in × cos²(ΔΦ/2)

5 şiddet seviyesi için (PAM-5 — Pulse Amplitude Modulation):
  Seviye -2: I = 0       ← ΔΦ = π
  Seviye -1: I = I₀/4
  Seviye  0: I = I₀/2
  Seviye +1: I = 3I₀/4
  Seviye +2: I = I₀      ← ΔΦ = 0

Bu telekomda kullanılıyor (PAM-4 standart, PAM-5 araştırma)!

Ama balanced quinary için işaret (sign) gerekiyor:
  +E ve -E farkı → koherent tespit
  E_out = {-2A, -A, 0, +A, +2A}

MZI koherent quinary:
  ΔΦ = {-π, -π/2, 0, +π/2, +π}
  → Field: {-E, -E/√2, 0, +E/√2, +E}
  → 5 farklı alan değeri → balanced benzeri

Gürültü marjı (optik):
  Shot noise limit: σ_shot ∝ √I
  I yüksekken gürültü az → +2 ve -2 daha güvenilir
  I düşükken gürültü görece yüksek → 0 seviyesi hassas

PAM-5 vs PAM-3 (ternary MZI):
  PAM-3: SNR gereksinimi referans (1×)
  PAM-5: ~2.5× daha iyi SNR gerekir (kaçınılmaz)
  Fotonik avantajı: shot noise'u azaltmak için güç artır
  → Lazer gücü 2.5× artarsa PAM-5 çalışır

Sonuç: Fotonik MZI'de quinary mümkün ve araştırılıyor.
       Telekom PAM-5 bu yolun kanıtı.
```

### 4.4 Memristör Crossbar — Mümkün Ama Zorlu

```
Memristör direnç seviyeleri:
  R_min (düşük direnç):    quint = +2
  R_low:                   quint = +1
  R_mid:                   quint =  0
  R_high:                  quint = -1
  R_max (yüksek direnç):  quint = -2

5 dirençli memristör: gösterildi
Ama:
  Direnç drift: her seviye zamanla kayar
  Ternary (3 seviye): 50% pencere → drift toleranslı
  Quinary (5 seviye): 25% pencere → drift sorunlu

  Cycle-to-cycle variability: ±5% típik
  Ternary: 50% - 2×5% = 40% net marj → güvenli
  Quinary: 25% - 2×5% = 15% net marj → sınırda
```

---

## Bölüm 5 — Aritmetik: Quinary Ne Kadar Zor

### 5.1 Toplama Tablosu

```
Dengeli üçlü toplama (3×3 = 9 durum):
  +  | -1   0  +1
  ───┼──────────────
  -1 | T̄1  -1   0
   0 | -1    0  +1
  +1 |  0   +1  1̄1   (T̄1 = taşımalı, 1̄1 = taşımalı)

Dengeli beşli toplama (5×5 = 25 durum, seçimler):
  +  | -2  -1   0  +1  +2
  ───┼─────────────────────
  -2 | T̄4  -3  -2  -1   0
  -1 | -3  -2  -1   0  +1
   0 | -2  -1   0  +1  +2
  +1 | -1   0  +1  +2  +3
  +2 |  0  +1  +2  +3  T4   (T4 = taşıma gerekli)

  Ternary: taşma {-2, +2} → taşıma seyrek
  Quinary: taşma {-5..-3, +3..+5} → daha sık taşıma

Donanım karmaşıklığı:
  Ternary adder:  ~3-4 LUT (FPGA'da)
  Quinary adder:  ~8-10 LUT (ternary'nin 2.5x)
```

### 5.2 Çarpma

```
Ternary çarpma: {-1,0,+1} × {-1,0,+1} = 9 durum
  Sonuç aralığı: {-1, 0, +1} → taşma yok!
  
  Temel işlemler:
  +1 × +1 = +1   (kopyala)
   0 ×  ? =  0   (sıfırla — en verimli!)
  -1 × +1 = -1   (ters çevir)
  
  Donanım: sadece MUX ve negasyon → çok ucuz

Quinary çarpma: {-2,-1,0,+1,+2} × {-2,-1,0,+1,+2} = 25 durum
  Sonuç aralığı: {-4,-3,-2,-1,0,+1,+2,+3,+4} → 9 farklı değer!
  → Sonuç quinary'ye sığmıyor: +2×+2 = +4 (yok)
  → Geniş sonuç tipi veya kırpma gerekli

  Temel işlemler (ternary gibi basit değil):
  +2 × +2 = +4   (bit kaydırma değil — gerçek çarpma)
  +2 × +1 = +2   (kopyala)
  +2 ×  0 =  0   (sıfırla)
  +1 × +1 = +1   (kopyala)
  
  Sıfır optimizasyonu:
  Ternary: %33 ağırlık sıfır → %33 operasyon atlanır
  Quinary: %20 ağırlık sıfır → %20 operasyon atlanır
  
  Donanım: tam çarpan gerekiyor için bazı kombinasyonlar
```

---

## Bölüm 6 — Neden 3 Özeldir: "Goldilocks Sayısı"

```
N durum kullanmak için verimlilik ve pratiklik dengesi:

N=2 (ikili):    100% verimli, çok basit ama az bilgi/sembol
N=3 (üçlü):    79% verimli, {-1,0,+1} fizikle örtüşüyor
N=4 (dörtlü):  100% verimli ama {0,1,2,3} simetrik değil
N=5 (beşli):   77% verimli, fiziksel gerçekleşimi zor
N=7 (yedili):  93% verimli, ama 7 doğal durum yok
N=8 (sekizli): 100% verimli, ama çok durum → zor

3'ün özel olmasının nedenleri:
  1. En küçük impair (tek) sayı > 1 → simetrik {-1,0,+1}
  2. Sıfır dahil → 1/3 oranında sıfır → %33 hesaplama tasarrufu
  3. Çarpma kapalı: {-1,0,+1} × {-1,0,+1} ⊆ {-1,0,+1}
  4. Fiziksel eşleme:
     PCM: amorf/kısmi/kristalin
     FeFET: aşağı/nötr/yukarı
     MZI: θ=0/π/2π (doğal 3 nokta)
     Sinaps: zayıfla/değiştirme/güçlendir
     Önyargı/oy: karşı/çekimser/lehte
  5. Bilgi teorisi: e'ye (2.718) en yakın taban → teorik optimum

e (Euler sayısı) ve sayı tabanı ilişkisi:
  e ≈ 2.718 → teorik olarak en verimli sayı tabanı e'ye yakın
  Bu yüzden 3, 2'den bilgi-teorik olarak daha verimli
  Ve 5'ten fiziksel olarak daha uygun
  
  "e tabanı" gerçek dünyada kullanılamaz (irrasyonel)
  3: e'nin en yakın tam sayısı → pratikte en iyi seçim
```

---

## Bölüm 7 — Ne Zaman Quinary Mantıklı

### 7.1 Fotonikte PAM-5 Uygulaması

```
Optik iletişim: PAM-5 aktif araştırma alanı
  800G ethernet: PAM-4 kullanıyor (4 seviye)
  1.6T ethernet (taslak): PAM-4 veya PAM-8 tartışılıyor
  
FE+EO CIM'e uyarlandığında:
  Her MZI = 1 ağırlık × 1 giriş fotonu = 1 sonuç
  PAM-3 (ternary):  3 ağırlık değeri × 1 giriş = 3 seviye çıkış
  PAM-5 (quinary):  5 ağırlık değeri × 1 giriş = 5 seviye çıkış

  PAM-5'in avantajı: aynı MZI, daha fazla bilgi
  MZI sayısı %32 azalır (hücre sayısı hesabından)
  
  Zorunlu koşul: koherent dedektör (fotodedektör + LO lazer)
  Maliyet: ~2x daha pahalı okuma devresi
  Enerji: ~2x daha fazla lazer gücü
```

### 7.2 Karma Hassasiyet (Mixed Precision) — Gerçek Çözüm

```
400B modelde tüm katmanlar aynı değil:
  Kritik katmanlar (dikkat Q/K):   hassasiyet şart
  Orta katmanlar (FFN):            orta hassasiyet
  Az kritik (embedding):           kaba yeterli

Optimal karma:
  Dikkat Q,K: Quinary (5 seviye) → hassas ayrım
  Dikkat V,O: Ternary (3 seviye) → kabul edilebilir
  FFN gate:   Ternary (3 seviye)
  FFN up/down: Ternary (3 seviye)
  Embedding:  Binary (2 seviye) → çok katman → ortalama iyi
  Layer norm:  FP16             → küçük, hassas şart

Karışık model boyutu (400B):
  Quinary (~50B):  50B × 3 bit = 18.75 GB
  Ternary (~330B): 330B × 2 bit = 82.5 GB
  Binary (~5B):    5B × 1 bit = 0.625 GB
  FP16 (~15B):     15B × 16 bit = 30 GB
  ─────────────────────────────────────────
  Toplam: 131.875 GB  ← saf ternary'nin (100 GB) biraz üstü
  Ama doğruluk: saf quinary'ye yakın

Bu "doğruluk-verimlilik dengesi"nin gerçek optimumu.
```

### 7.3 Sonuç Tablosu

```
Ölçüt              Binary   Ternary   Quinary   INT4
──────────────────────────────────────────────────────
Bit/sembol            1.0     1.585     2.322    4.0
Depolama (400B)      50 GB   100 GB   150 GB   200 GB
Doğruluk kaybı      ~9%      ~2.5%    ~1.2%    ~1.6%
PCM uyumu            ✓✓      ✓✓✓     ✓✓       ✓
FeFET uyumu          ✓✓      ✓✓✓     ✓        ✓✓
MZI uyumu            ✓✓      ✓✓✓     ✓✓       ✗
Sıfır optimizasyon  —       %33 at.  %20 at.   ✗
Çarpma karmaşıklığı En az   Az       Orta      Tam
Donanım olgunluğu   ✓✓✓     ✓✓      ✓         ✓✓✓
```

---

## Bölüm 8 — Volt Tip Sisteminde Beşli

```volt
// Dengeli beşli tipi
type Quint = i4  // -2 ile +2, i4 yeterli (4 bit, ama 5 değer)
// Daha doğru: özel kısıtlı tip
type Quint = restrict i4 in {-2, -1, 0, +1, +2}

// Quinary nöral ağ katmanı
module QuinaryLinear<const In: usize, const Out: usize> {
    in  x      : [i8; In]                   // aktivasyon INT8
    in  weights: Matrix<Quint, Out, In>      // quinary ağırlıklar
    out y      : [i16; Out]                  // genişletilmiş çıkış

    // Her ağırlık operasyonu:
    // +2: 2x kopyala  (bit kaydırma + kopyala)
    // +1: kopyala
    //  0: sıfırla     (en hızlı)
    // -1: ters çevir
    // -2: 2x ters çevir
    
    @cost(energy = In * Out * 0.15.pJ)  // ternary 0.1 pJ'den fazla
    // (binary: 0.05 pJ, ternary: 0.1 pJ, quinary: 0.15 pJ)

    // Sıfır optimizasyonu (ternary'den az ama hâlâ var)
    @zero_skip_rate(0.20)  // %20 (ternary'de %33)
}

// PAM-5 fotonik quinary
module PAM5_FEO_Cell<const λ_nm: u32> {
    in  optical_in  : WDMChannel<λ_nm>
    in  fe_weight   : Quint   @Ferroelectric5Level
    out optical_out : WDMChannel<λ_nm>

    // 5 faz değeri
    let phi = quint_to_phase(fe_weight)
    // phi ∈ {-π, -π/2, 0, +π/2, +π}

    optical_out = mzi_coherent(optical_in, phi)

    @cost(energy=0.015.fJ)  // ternary 0.01 fJ'den biraz fazla

    // Koherent dedektör gereksinimi
    #[requires(coherent_detection)]  // ternary'den farklı!

    // Dar gürültü marjı uyarısı
    @snr_requirement(2.5x_baseline)  // ternary'nin 2.5 katı SNR
}

// Karma hassasiyet LLM bloğu (gerçekçi)
module MixedPrecisionBlock<const D: usize> {
    // Q ve K: quinary (hassasiyet kritik)
    let Q = QuinaryLinear<D, D>(x, W_Q)
    let K = QuinaryLinear<D, D>(x, W_K)

    // V ve O: ternary (yeterli)
    let V = TernaryLinear<D, D>(x, W_V)
    let O = TernaryLinear<D, D>(attn, W_O)

    // Volt doğrulaması: toplam enerji bütçesi
    invariant: layer_energy < 50.uJ
    // Karma hassasiyet: saf quinary'den verimli, saf ternary'den doğru
}
```

---

## Özet: Üçlü mü, Beşli mi?

```
Sorunun cevabı kullanım senaryosuna göre değişiyor:

Ternary kazanıyor:
  → Depolama verimliliği (%50 daha az alan vs quinary)
  → Fiziksel uygulama kolaylığı (PCM, FeFET, MZI)
  → Sıfır optimizasyonu (%33 vs %20)
  → Donanım sadeliği (çarpma kapalı)
  → "Goldilocks" pozisyonu: e'ye en yakın tam sayı

Quinary kazanıyor:
  → Daha yüksek doğruluk (%1.2 vs %2.5 kayıp)
  → Aynı hücre sayısında %46 daha fazla bilgi
  → MZI'nin 5 doğal faz noktasından yararlanma (PAM-5)
  → Kritik katmanlar için

Gerçek optimum:
  Karma hassasiyet:
  Kritik katmanlar → quinary (veya INT4)
  Diğer katmanlar → ternary
  → En iyi doğruluk-depolama dengesi

Tek cümle:
  Dengeli beşli, dengeli üçlünün "daha hassas ama daha az verimli"
  versiyonu; fiziksel uygulama açısından üçlünün doğal
  avantajlarını yitiriyor, ama fotonik PAM-5'te ve karma
  hassasiyet senaryolarında gerçek bir değer sunuyor.
  3, e'ye en yakın tam sayı olduğu için teorik optimum;
  5, pratik ödünleşimlerde ikincil ama gerekli bir seçenek.
```
