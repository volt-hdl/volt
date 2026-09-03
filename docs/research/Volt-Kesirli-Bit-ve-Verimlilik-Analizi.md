> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Bit Başına Verimlilik ve Kesirli Bit — Bilgi Teorisinden Fotoniğe

> Bilgi teorisi "bit" kavramını gerçek sayı (kesirli) olarak tanımlar.
> Donanımın tamsayı bitlere sıkışması bir fizik yasası değil,
> mühendislik seçimi. Bu seçimi aşmanın üç yolu var ve her biri
> 400B modeli farklı boyutlara sıkıştırıyor.

---

## Bölüm 1 — Bilgi Teorisi: Bit Zaten Kesirli

### 1.1 Shannon'ın Tanımı

```
Claude Shannon (1948), bilgi miktarını şöyle tanımladı:

  H = -Σ pᵢ × log₂(pᵢ)    (Shannon entropisi)

  pᵢ: her sembolün olasılığı

Eşit olasılıklı N sembol için:
  H = -N × (1/N) × log₂(1/N) = log₂(N)

  İkili (N=2):  H = log₂(2) = 1.000 bit/sembol ✓ (tam sayı)
  Üçlü (N=3):  H = log₂(3) = 1.585 bit/trit   ← kesirli!
  Beşli (N=5):  H = log₂(5) = 2.322 bit/quint  ← kesirli!

"Bit" tam sayı olmak zorunda değil.
Entropi doğası gereği gerçek sayıdır.
```

### 1.2 Neden Pratikte Tamsayıya Zorunlandık

```
Sorun: fiziksel iletim "tık tık" çalışıyor

  Saat kenarı 1: 0 veya 1 → tam sayı (2 durum)
  Saat kenarı 2: 0 veya 1 → tam sayı
  ...

  "1.585 bit iletmek" kelime olarak anlamsız görünüyor:
  Bir saat kenarında 1 bit, yarım saat kenarında 0.585 bit?

Ama bu yanılsama. Önemli olan:
  Sembol başına değil, UZUN VADEDE ortalama.
  Sonsuz sembol akışı → toplam bit / sembol sayısı → H

Benzetme:
  Zar: her atışta tam sayı çıkar (1-6)
  Ama beklenti = 3.5 → "kesirli ortalama"

Bilgi: her sembolde tam bit, ama çok sembol üzerinden kesirli.
```

---

## Bölüm 2 — Kesirli Bit Verimliliğine Ulaşmanın Üç Yolu

### Yol 1 — Sembol Gruplama (Blok Kodlama)

**Fikir:** Tek sembolü kodlamak yerine N sembolü birlikte kodla.

```
Tek üçlü (trit): 3 durum → 2 bit gerekir → 79% verimli

5 triti birlikte kodla:
  3⁵ = 243 farklı değer
  243'ü saklamak için: ⌈log₂(243)⌉ = 8 bit
  Verimlilik: (5 × 1.585) / 8 = 7.925 / 8 = 99.1%  ← çok iyi!

10 triti birlikte:
  3¹⁰ = 59,049 → ⌈log₂(59049)⌉ = 16 bit
  Verimlilik: (10 × 1.585) / 16 = 15.85 / 16 = 99.1%

100 triti birlikte:
  3¹⁰⁰ değer → ⌈100 × log₂(3)⌉ = 159 bit
  Verimlilik: (100 × 1.585) / 159 = 99.7%  ← mükemmele yakın

N → ∞:
  Verimlilik → 100%  ← tam sınır

Pratik tablo:
N (grup)    Gereken bit    Verimlilik
────────────────────────────────────
1           2              79.2%
2           4              79.2%  (aynı!)
5           8              99.1%
10          16             99.1%
13          21             98.0%
k×log₂(3)  k×1+ε          →100%
```

### Yol 2 — Aritmetik Kodlama / ANS (Asimetrik Sayısal Sistemler)

**Bu yöntem pratikte gerçekten kesirli bit kullanır:**

```
Aritmetik kodlama prensibi:
  Sembol akışı → gerçek sayı aralığına eşleme
  
  [0.0, 1.0) aralığından başla
  Her sembol aralığı daraltır:
  
  Sembol 0 (p=0.5): [0.0, 0.5)
  Sembol 1 (p=0.25): [0.25, 0.375)
  Sembol -1 (p=0.25): [0.3125, 0.34375)
  ...

  Sonunda: aralık içindeki herhangi bir sayı → kod
  Bu sayıyı binary'de yaz → sıkıştırılmış çıktı

  Uzun dizide:
  Gerçek bant genişliği → H = Σ pᵢ log₂(1/pᵢ) bit/sembol
  Tam olarak entropiye ulaşıyor!

ANS (Asymmetric Numeral Systems) — pratik versiyon:
  Jarek Duda (2009): aritmetik kodlamanın hızlı uygulaması
  Integer işlemler → donanım dostu
  Gerçek dünya: Zstandard, HEVC, AV1, LZFSE kullanıyor
```

**Nöral ağ ağırlıkları için:**

```
Ternary ağırlıkların GERÇEKDEKİ dağılımı:
  P(0)  = 0.50  ← modeldeki sıfırlar çok!
  P(+1) = 0.25
  P(-1) = 0.25

Entropi hesabı:
  H = -0.50×log₂(0.50) - 2×0.25×log₂(0.25)
    = 0.50 + 0.50 + 0.50
    = 1.500 bit/trit  ← uniform'dan daha az! (1.585 değil)

400B model:
  Uniform ternary: 400B × 2 bit / 8 = 100 GB
  ANS sıkıştırılmış: 400B × 1.5 bit / 8 = 75 GB  ← %25 tasarruf!

Eğer model fine-tune sonrası daha seyrekse (P(0)=0.70):
  H = 0.70×log₂(1/0.70) + 2×0.15×log₂(1/0.15)
    = 0.515 + 2×0.415
    = 1.346 bit/trit

  400B × 1.346 bit / 8 = 67.3 GB  ← %33 tasarruf!
```

**ANS'ın FE+EO CIM ile entegrasyonu:**

```
Geleneksel: her hücre = 1 trit (bağımsız, sabit)
ANS ile: hücreler ANS akışı olarak yorumlanır

  Okuma: fiziksel hücre → trit → ANS dekoder → ağırlık
  Bu küçük bir hesaplama ekler ama:
  Hücre sayısı %25-33 azalır → alan %25-33 küçülür

ANS dekoder donanım maliyeti:
  ~100-200 LUT (FPGA) veya ~1000 transistör (ASIC)
  Tradeoff: az alan ama hafif hesaplama ek yükü
```

---

### Yol 3 — Analog Depolama: Gerçekten Sonsuz Durum

**Fikir:** Sayısal değil, sürekli analog değer depola.

```
Sayısal: {-1, 0, +1} — 3 nokta
Analog:  [-1.0, +1.0] — sonsuz nokta!

Ama gürültü sonsuz çözünürlüğü öldürür.

Shannon-Hartley teoremi (analog kanal kapasitesi):
  C = log₂(1 + S/N) bit/sample

  S/N (sinyal-gürültü oranı):
  SNR = 10  (10 dB): C = log₂(11) ≈ 3.46 bit
  SNR = 100 (20 dB): C = log₂(101) ≈ 6.66 bit
  SNR = 1000(30 dB): C = log₂(1001) ≈ 9.97 bit
  SNR = 10⁴ (40 dB): C = log₂(10001) ≈ 13.3 bit

  FE+EO optik sistemi: SNR = 20-30 dB gerçekçi
  → 6-10 bit/hücre potansiyeli

Karşılaştırma:
  Binary:   1.00 bit/hücre
  Ternary:  1.585 bit/hücre
  Quinary:  2.322 bit/hücre
  Analog (20dB SNR): 6.66 bit/hücre
  Analog (30dB SNR): 9.97 bit/hücre
  INT8:     8 bit/hücre (referans)
```

---

## Bölüm 3 — e Sayısı: Teorik Optimum

### 3.1 e Neden Ortaya Çıkıyor

```
Soru: kaç durum kullanmak en verimli?

Birim "donanım karmaşıklığı" başına maksimum bilgi:
  N durum → log₂(N) bit
  Maliyet (karmaşıklık) ∝ N (durum sayısı)
  Verimlilik = log₂(N) / N

Bu fonksiyonu maksimize et:
  d/dN [log₂(N)/N] = 0

  d/dN [ln(N)/N] = 0
  [1/N × N - ln(N)] / N² = 0
  1 - ln(N) = 0
  N = e  ← Euler sayısı!

Sonuç: N = e ≈ 2.718 en verimli sayı tabanı
  log₂(e)/e = 1.4427/2.718 = 0.5306 bit/durum (maksimum)

Karşılaştırma:
  N=2:   log₂(2)/2   = 0.500  (e-optimumun %94.2'si)
  N=3:   log₂(3)/3   = 0.528  (e-optimumun %99.7'si!)  ← mükemmel
  N=5:   log₂(5)/5   = 0.464  (e-optimumun %87.5'i)
  N=8:   log₂(8)/8   = 0.375  (e-optimumun %70.7'si)
  N=16:  log₂(16)/16 = 0.250  (e-optimumun %47.1'si)

ÜÇLÜ e'nin en yakın tam sayısı olduğu için
verimlilikte e-optimumun %99.7'sine ulaşıyor.

Bu ternary'nin "Goldilocks" konumunun matematiksel kanıtı.
```

### 3.2 e-Tabanı Gerçekten Kullanılabilir mi

```
Teorik "e-tabanı" hesaplama:
  Her "e-it" (e-ary digit): log₂(e) = 1.4427 bit taşır
  
  Ama e irrasyonel → kaç durum? 2.718... durum → anlamsız!

  Pratik yaklaşım 1 — Fibonacci kodlama:
  Zeckendorf teoremine göre her pozitif tam sayı
  birbirini takip etmeyen Fibonacci sayılarının toplamı
  Fibonacci: 1, 2, 3, 5, 8, 13, 21...
  → base-φ (altın oran ≈ 1.618) kodlaması
  Hala irrasyonel, pratik değil

  Pratik yaklaşım 2 — ANS ile e'ye yaklaşma:
  Büyük bloklar kullanarak verimlilik e-optimuma yaklaşır
  3 durum, büyük blok → %99.7 verimlilik → e-optimumun %99.7'si

e'nin önemi: ternary'nin teorik üstünlüğünü açıklıyor
             pratik bir taban değil
```

---

## Bölüm 4 — Dağılım-Bilinçli Kodlama

### 4.1 Nöral Ağ Ağırlıkları Uniform Değil

```
Ağırlık dağılımının entropisi:
  P(0) arttıkça entropy azalır → daha az bit gerekir

  P(0)    P(±1)   Entropi    400B GB
  ─────────────────────────────────────
  0.333   0.333   1.585      100 GB   (uniform, maksimum)
  0.500   0.250   1.500       94 GB
  0.600   0.200   1.371       86 GB
  0.700   0.150   1.216       76 GB
  0.800   0.100   0.922       58 GB
  0.900   0.050   0.469       29 GB   (çok seyrek)
  1.000   0.000   0.000        0 GB   (tüm sıfır)

Modern TNN (Ternary Neural Networks) tipik:
  İnce ayar sonrası: P(0) ≈ 0.60-0.70
  → 76-86 GB (naive 100 GB'nin %76-86'sı)

Bu sıkıştırma ÜCRETSİZ:
  Ağırlıklar zaten bu dağılıma sahip
  ANS ile kodlanınca otomatik tasarruf sağlanır
```

### 4.2 Katmana Özgü Entropi

```
LLM'de farklı katmanların farklı ağırlık dağılımı:

Katman tipi      P(0)  Entropi  Bit/weight  GB (katman)
──────────────────────────────────────────────────────────
Embedding        0.30  1.521    2 bit       katmana göre
Attention Q/K    0.45  1.530    1.75 bit    uzun
Attention V/O    0.55  1.450    1.60 bit    uzun
FFN gate         0.65  1.300    1.45 bit    çok uzun
FFN up/down      0.70  1.216    1.35 bit    çok uzun
Output proj.     0.40  1.562    1.80 bit    kısa

Optimal strateji: her katman için ayrı ANS tablo
→ Katman entropisine göre otomatik sıkıştırma
→ Toplam: uniform'un %75-80'i
```

---

## Bölüm 5 — FE+EO CIM'de Kesirli Bit: Tam Analog

### 5.1 Pockels Etkisi Aslında Sürekli

```
FE+EO MZI transfer fonksiyonu:
  I_out = I_in × cos²(ΔΦ/2)
  ΔΦ = (2π/λ) × r₃₃ × E_FE × L

  E_FE: ferroelektrik iç alan → sürekli değer!
  Sadece {-E₀, 0, +E₀} değil
  [−E_max, +E_max] aralığında sürekli

Biz "ternary" diye 3 seviyeye kısıtladık.
Ama kısıtlamak zorunda değiliz.

Sürekli analog FE+EO:
  Ağırlık = E_FE ∈ [-E_max, +E_max] (sürekli gerçek sayı)
  Kayıt: ferroelektrik polarizasyon sürekliliği
  Okuma: Pockels faz kayması → sürekli çıkış
  Bu FP16 veya FP32 gibi gerçel sayı ağırlık!
```

### 5.2 Kaç Bit Saklanabilir

```
Optik sistem SNR (gürültü kaynakları):

  Shot noise:     σ_shot = √(η × P × τ) (fotodedektör)
  Johnson noise:  σ_J = √(4kTB/R)
  Laser RIN:      σ_RIN = RIN × P × τ
  Toplam:         σ_total = √(σ_shot² + σ_J² + σ_RIN²)

Pratik Si-fotonik parametrelerle:
  P = 1 mW lazer, τ = 1 ns entegrasyon
  SNR ≈ 25-35 dB (10³ - 3×10³)
  
  Bit kapasitesi: log₂(1 + SNR)
  25 dB: log₂(1 + 316) ≈ 8.3 bit
  30 dB: log₂(1 + 1000) ≈ 10.0 bit
  35 dB: log₂(1 + 3162) ≈ 11.6 bit

Yani tek bir FE+EO hücresi:
  Ternary (3 seviye): 1.585 bit (Shannon sınırının %19'u)
  Quinary (5 seviye): 2.322 bit (Shannon sınırının %28'i)
  Analog (30dB SNR):  10 bit  (Shannon sınırının %100'ü!)

Analog FE+EO: INT8 ile yarışıyor (8 bit)
```

### 5.3 Sürekli Ağırlık ile Hesaplama

```
En güzel özellik: hesaplama da kesirli!

Sayısal CIM (ternary):
  w = {-1, 0, +1}
  x = {-1, 0, +1} (veya INT8)
  y = w × x → {-1, 0, +1} veya yuvarlama

Analog CIM (sürekli):
  w ∈ [-1, +1]  (sürekli gerçek)
  x ∈ [-1, +1]  (sürekli giriş fotonu)
  y = w × x     → gerçek sayı sonuç, yuvarlama YOK!

  Analog çarpmanın avantajı:
  → Kuantizasyon hatası sıfır (teorik)
  → W × X doğrusal hesaplama (Pockels fiziksel)
  → FP32 doğruluğuna yakın!

Neden şu an yaygın değil:
  → Okuma: ADC ile kesirli değeri ölç (SNR sınırlı)
  → Drift: analog değer zamanla kayar
  → Kalibryon: her hücre ayrı ayar istiyor
```

---

## Bölüm 6 — Pratik Verimliliğin Tam Tablosu

### 6.1 400B Model için Her Yöntem

```
Yöntem                  Bit/weight   400B GB   Kısıtlama
──────────────────────────────────────────────────────────────
FP32                    32.00        1600 GB   —
FP16                    16.00         800 GB   —
BF16                    16.00         800 GB   —
INT8                     8.00         400 GB   kalibrasyon
INT4                     4.00         200 GB   doğruluk ↓
Ternary (2 bit/trit)    2.000         100 GB   doğruluk ↓↓
Ternary blok (N=5)      1.585*0.991   99.1 GB  karmaşıklık +
Ternary + ANS (P0=0.5) 1.500          75 GB   ANS overhead
Ternary + ANS (P0=0.7) 1.216          61 GB   seyreklik şart
Quinary (3 bit)         3.000         150 GB   daha az verimli
Quinary blok (N=13)     2.322*0.98   114 GB   büyük blok
İkili + ANS             1.000*H        ?       dağılıma bağlı
Analog (20dB SNR)       6.66          333 GB   drift/kalibrasyon
Analog (30dB SNR)       9.97          499 GB   drift/kalibrasyon
Sıkışık ternary+ANS    ~1.2-1.4     60-70 GB  en iyi pratik

* Teorik sınır; pratik ~99%
```

### 6.2 En İlginç Sonuç

```
Paradoks: analog daha fazla bit → daha büyük depolama?

  Ternary: 2 bit/hücre → 100 GB (FP16 doğrulukta değil)
  Analog (30dB): 10 bit/hücre → 500 GB (FP16 doğrulukta!)

  Ternary'nin avantajı depolama kapasitesinde değil,
  donanım basitliğinde ve sıfır (0) optimizasyonunda!

Gerçek karşılaştırma:
  Ternary CIM: 100 GB, %75 doğruluk yaklaşımı, çok ucuz hw
  Analog CIM:  500 GB, %100 FP16 doğruluk, karmaşık hw

  Doğruluk / GB oranı:
  Ternary: 100% (baz alınan)
  Analog:  çok daha iyi (aynı doğruluk için 5x fazla GB ama sıfır quantization error)

Sonuç: analog ve sayısal CIM farklı ödünleşimler → farklı uygulamalar
```

---

## Bölüm 7 — Yeni Kavram: Sürekli Trit (Soft Ternary)

### 7.1 Sert vs Yumuşak Kuantizasyon

```
Sert ternary (hard):
  w_real = 0.73  →  w_trit = +1  (kesip at)
  Bilgi kaybı: var, geri dönülemez

Yumuşak ternary (soft):
  w_real = 0.73  →  w_soft = +0.73  (koru)
  FE+EO'da: polarizasyon = 0.73 × P_max (sürekli)
  Okuma: Pockels faz kayması doğrusal → 0.73 × Δn_max

  Bu aslında analog ağırlık depolama!
  Ama ternary limitlerini referans alarak kalibre edilmiş.

Hibrit yaklaşım:
  Eğitim: FP16 (tam hassasiyet)
  İnce ayar: ternary hedef (0'a yakın ağırlık → 0)
  Depolama: analog (gerçel değer korunuyor!)
  Çıkarım: analog CIM (kuantizasyon hatası yok)

  Bu "en iyi her iki dünya":
  Ternary'nin seyreklik avantajı (P(0)≈0.5-0.7)
  + Analog'un hassasiyet avantajı (drift kontrol edilirse)
```

### 7.2 Kayan Nokta Analog: FP-Photonic

```
Analog CIM'i kayan noktalı (floating point) yapmak:

Stanart FP16: 1 bit işaret + 5 bit üs + 10 bit mantis
Analog FP: işaret + sayısal üs + ANALOG mantis!

  Analog mantis: [0, 1] aralığında sürekli değer
  Sayısal üs: tam sayı, birkaç bit
  İşaret: polarizasyon yönü

Tasarım:
  2 bit sayısal üs (FeFET) + 1 analog mantis (Pockels)
  → 4 üs değeri × sürekli [0,1] = FP4-analog

  Etkin hassasiyet: 4 × (SNR bit sayısı) ≈ 4 × 10 = 40 bit??

  Hayır. Daha doğru hesap:
  log₂(4 aralık × 1024 seviye) = log₂(4096) = 12 bit

  12 bit FP-analog: INT12'ye eşdeğer → excellent!
  Hücre sayısı: ternary ile aynı (2 bit üs + 1 analog)
  Depolama: 100 GB (ternary ile aynı!)
  Doğruluk: INT12 → FP16'ya çok yakın

Bu gerçekçi mi?
  2 bit FeFET: mümkün (bugün)
  1 analog Pockels (30dB): mümkün (araştırma aşaması)
  Birlikte: yakın vade araştırma → 5-7 yıl
```

---

## Bölüm 8 — Volt Tip Sistemi: Kesirli Bit Kavramları

```volt
// Bilgi entropisi – Volt'ta tip seviyesinde
type Entropy = f32  // bit/sembol (kesirli!)

// Analog ağırlık tipi – sürekli değer
type AnalogWeight<const SNR_dB: u32> = {
    value    : f32,         // sürekli gerçel
    capacity : Entropy,     // log₂(1 + 10^(SNR_dB/10))
    noise    : f32,         // standart sapma
}

// ANS sıkıştırılmış ternary bellek
module ANS_TernaryMemory<const N: usize> {
    in  weights_raw : [Trit; N]     // ham ternary ağırlıklar
    out weights_cmp : [u8; ?]       // sıkıştırılmış (boyut değişken)

    // Dağılımı öğren (ince ayar sonrası)
    @distribution(empirical)        // P(0), P(+1), P(-1) ölç
    @codec(ANS)                     // Asymmetric Numeral Systems

    // Gerçek bit/trit hesaplanır, tahmin değil
    let H = empirical_entropy(weights_raw)
    let compressed_size = N * H / 8  // byte

    // Volt doğrulaması
    assert: compressed_size < N * 2 / 8  // her zaman 2 bit'ten az
    @bits_per_weight(H)                  // gerçek (kesirli!) verimlilik
}

// Hibrit: sayısal üs + analog mantis
module HybridFP_FEO<const ExponentBits: u8> {
    in  optical_in   : OpticalSignal @Photonic
    in  exp_fe       : bits<ExponentBits>   @Ferroelectric  // sayısal
    in  mantis_phase : f32                   @Photonic       // analog!
    out optical_out  : OpticalSignal @Photonic

    // Etkin hassasiyet
    let snr_bits    = log2(1 + snr)        // analog kapasite
    let total_bits  = ExponentBits + snr_bits
    // 2 bit üs + 10 bit analog (30dB) = 12 bit etkin

    @effective_precision(total_bits)
    @cost(energy=0.02.fJ)  // analog okuma: saf ternary'nin 2x'i
}

// e-optimum referansı (teorik)
const E_OPTIMAL_STATES : f32 = std::f32::consts::E;  // 2.718
const TERNARY_EFFICIENCY : f32 = log2(3.0) / 3.0;    // 0.528
const E_EFFICIENCY      : f32 = log2(E) / E;          // 0.5306
const TERNARY_VS_E      : f32 = TERNARY_EFFICIENCY / E_EFFICIENCY;
// ≈ 0.9953 → ternary, teorik optimumun %99.5'i
```

---

## Özet: Bit Tam Sayı Olmak Zorunda Değil

```
Katman 1 — Bilgi teorisi:
  Entropi zaten kesirli: H = log₂(N) ∈ ℝ
  Bit = soyut ölçü birimi, fiziksel kısıt değil

Katman 2 — Pratik yöntemler:
  A) Sembol gruplama: N artınca verimlilik → %100
  B) ANS/aritmetik kodlama: akış üzerinde tam entropi
  C) Analog depolama: SNR ile sınırlı sürekli bit

Katman 3 — Nöral ağ özelliği:
  Ağırlıklar uniform değil: P(0) > 1/3
  → Gerçek entropi < log₂(3) = 1.585
  → ANS ile 75-85 GB (100 GB değil)

Katman 4 — FE+EO analog:
  Pockels etkisi sürekli
  30 dB SNR → 10 bit/hücre potansiyeli
  Hibrit FP-FEO: 2 bit sayısal + 10 bit analog → 12 bit etkin

Katman 5 — e sayısı:
  Teorik optimum: N = e ≈ 2.718 durum
  Ternary: e'nin en yakın tam sayısı → %99.7 verimli
  Bu, ternary'nin üstünlüğünün matematiksel kanıtı

400B model özet (iyi → en iyi):
  Naive ternary:       100 GB
  ANS ternary:          75 GB  (P(0)=0.5)
  ANS seyrek ternary:   60 GB  (P(0)=0.7, fine-tune sonrası)
  Hibrit FP-FEO:       100 GB  ama 12 bit etkin hassasiyet
                               (ternary alanı, INT12 doğruluğu!)

En büyük içgörü:
  "Kaç bit?" sorusu yanlış.
  "Kaç entropi birimi?" doğru soru.
  Cevap: her zaman kesirli, her zaman gerçel sayı.
```
