> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Optik Gürültü — Kaynaklar ve Azaltma Yöntemleri

> Optik gürültü tek bir şey değil: birbirinden farklı fiziksel
> kökenli en az yedi ayrı gürültü türü var. Bir kısmı temel
> fizik yasaları gereği kaçınılmaz, bir kısmı mühendislik
> ile sıfıra indirilebilir, bir kısmı ise kuantum tekniklerle
> teorik sınırın altına itilebilir.

---

## Bölüm 1 — Gürültüyü Anlamak: Temel Çerçeve

### 1.1 Neden Gürültü Var — Fiziksel Köken

```
Her ölçüm sisteminde üç temel kısıt:

1. Kuantum belirsizliği:
   "Foton tam olarak ne zaman gelecek?" — bilinemez
   Heisenberg: ΔE × Δt ≥ ħ/2

2. Termal salınım:
   Sıcaklık > 0 K ise elektronlar rastgele hareket eder
   Bu hareketi tamamen durdurmak için T = 0 K gerekir

3. Malzeme kusurları:
   Atom dizilimindeki düzensizlik → ışık saçılır
   Teorik mükemmel malzeme = pratikte imkânsız

Bu üçünden gürültüler kaçınılmaz ama
"ne kadar gürültü" mühendislikle çok değiştirilebilir.
```

### 1.2 Gürültü Türleri Haritası

```
OPTİK GÜRÜLTÜ
│
├── TEMEL (fizik yasası, tam ortadan kaldırılamaz)
│   ├── Atım gürültüsü (shot noise) ← kuantum
│   └── Faz gürültüsü (phase noise) ← kuantum
│
├── KLASİK (mühendislikle azaltılabilir/yokedilebilir)
│   ├── RIN (lazer yoğunluk gürültüsü)
│   ├── Termal gürültü (Johnson noise)
│   ├── Karanlık akım gürültüsü
│   ├── Dalga kılavuzu saçılma gürültüsü
│   └── WDM kanal çapraz konuşması
│
└── KUANTUM TEKNİKLERLE AŞILABİLİR
    ├── Sıkıştırılmış ışık (squeezed light)
    └── Dolanık foton durumları
```

---

## Bölüm 2 — Her Gürültü Türünün Fiziği

### Gürültü 1 — Atım Gürültüsü (Shot Noise) — En Temel

**Fiziksel köken:** Işığın parçacık doğası.

```
Klasik dalga anlayışıyla:
  Işık sürekli akış → her an sabit güç → sorunsuz

Gerçek (kuantum mekanik):
  Işık fotonlardan oluşuyor
  Fotonlar ayrık parçacıklar → rastgele geliyorlar

  Ortalama: saniyede 1000 foton
  Gerçek: 998, 1003, 1001, 997, 1005...  (Poisson dağılımı)

  Sapma: σ = √(ortalama) = √1000 ≈ 31.6 foton
  SNR = ortalama / sapma = 1000 / 31.6 ≈ 31.6

Formül:
  σ²_shot = 2eIB
  I: fotodedektör akımı, B: bant genişliği, e: elektron yükü

  SNR_shot = √(η P τ / hν)
  η: kuantum verimlilik, P: güç, τ: ölçüm süresi
```

**Önemli özellik:** P arttıkça SNR artar, ama √P olarak:

```
P × 4 → SNR × 2 (4x güç için 2x iyileşme)
P × 100 → SNR × 10

Bu kök ilişkisi atım gürültüsünün parmak izi:
  Daha güçlü lazer kullan → SNR iyileşir, ama yavaş
```

**Kaçınılabilir mi:** **Hayır** — fotonların rastgele gelişi kuantum mekaniğin temeli. Ama aşılabilir (Bölüm 4'te).

---

### Gürültü 2 — Lazer Yoğunluk Gürültüsü (RIN)

**Fiziksel köken:** Lazer içindeki kendiliğinden yayım (spontaneous emission).

```
Lazer nasıl çalışır:
  Uyarılmış yayım (stimulated emission): kontrollü, lazer ışığı üretir
  Kendiliğinden yayım (spontaneous emission): rastgele, her yöne

  Problem: kendiliğinden yayım ana lazer moduna karışır
  → Anlık güç dalgalanması

  Ev analojisi:
  Kalabalık salonda konuşuyorsun (stimulated = lazer modu)
  Arka planda rastgele gürültü var (spontaneous = arka plan)
  Arka plan ana sese karışıyor

RIN (Relative Intensity Noise) formülü:
  RIN = ΔP²/P² / B   [dB/Hz]

Tipik değerler:
  DFB lazer: -150 dB/Hz    (iyi)
  VCSEL:     -140 dB/Hz    (kabul edilebilir)
  LED:       -120 dB/Hz    (kötü, lazer değil)
```

**Kaçınılabilir mi:** **Büyük ölçüde evet** — balans algılama (Bölüm 3) ile neredeyse tamamen yokedilebilir.

---

### Gürültü 3 — Termal Gürültü (Johnson Noise)

**Fiziksel köken:** Elektronların termal hareketi.

```
Her iletken malzemede:
  Sıcaklık > 0K → elektronlar rastgele salınım yapıyor
  Bu salınım → küçük akım → gürültü

Formül:
  σ²_thermal = 4kTB/R
  k: Boltzmann sabiti, T: sıcaklık (Kelvin), R: direnç

  T = 300K (oda sıcaklığı), R = 1kΩ, B = 1GHz:
  σ_thermal = √(4 × 1.38×10⁻²³ × 300 × 10⁹ / 1000)
            ≈ 4 μV

Optik sistemde:
  Fotodedektör okuma devresi → termal gürültü ekler
  Düşük optik güçlerde termal gürültü domineer
  Yüksek optik güçlerde atım gürültüsü domineer
```

**Kaçınılabilir mi:** **Kısmen** — soğutmayla azaltılabilir. T = 4K (sıvı helyum): oda sıcaklığının 1/75'i kadar.

---

### Gürültü 4 — Faz Gürültüsü (Phase Noise / Laser Linewidth)

**Fiziksel köken:** Lazerin tam frekansı sabit değil.

```
İdeal lazer:         E(t) = A × cos(2πf₀t)   (sabit frekans f₀)
Gerçek lazer:        E(t) = A × cos(2πf₀t + φ(t))
                     φ(t): rastgele faz yürüyüşü

  Foton frekansı her zaman tam f₀ değil
  ±Δf civarında salınım → "lazer genişliği" (linewidth)

DFB lazer linewidth:       ~100 kHz - 1 MHz
Harici boşluklu lazer:     ~1 kHz
Kalıp kilitli lazer:       ~1 Hz seviyesinde

Koherent deteksiyonda önemli:
  Alınan sinyal × yerel osilatör (LO) lazer
  LO'nun faz gürültüsü → ölçüm hatasına dönüşür
```

**Kaçınılabilir mi:** **Büyük ölçüde** — dar çizgili lazer seçimi (harici boşluklu) ile 1000x azaltılabilir.

---

### Gürültü 5 — Dalga Kılavuzu Saçılma Gürültüsü

**Fiziksel köken:** Fabrikasyon kusurları.

```
Mükemmel dalga kılavuzu (teorik):
  ─────────────────────────────
  Düzgün yüzey → ışık sorunsuz ilerler

Gerçek dalga kılavuzu:
  ─╲──╱─╲──╱───╲──╱──
  Dalgalı yüzey → ışık saçılır, kaybolur

Yüzey pürüzlülüğü kaynakları:
  Litografi: EUV bile ±0.5 nm sapma
  Kimyasal aşındırma: homojen değil
  Kaplama: atom-atom düzensizlik

Etkisi:
  Si dalga kılavuzu, 220nm × 500nm:
  İyi fabrikasyon: 0.5 dB/cm kayıp
  Kötü fabrikasyon: 5 dB/cm kayıp
  → 10× fark mümkün
```

**Kaçınılabilir mi:** **Büyük ölçüde** — daha iyi litografi (EUV), kimyasal-mekanik parlatma (CMP), rıhtım tabanlı dalga kılavuzu ile azaltılabilir.

---

### Gürültü 6 — WDM Kanal Çapraz Konuşması (Crosstalk)

**Fiziksel köken:** Kanallar arasında sızan ışık.

```
İdeal WDM demultiplexer:
  λ₁ → tam olarak çıkış 1
  λ₂ → tam olarak çıkış 2  (sıfır karışım)

Gerçek durum:
  λ₁ → %99.9 çıkış 1 + %0.1 çıkış 2 (kaçak!)
  λ₂ → %0.1 çıkış 1 + %99.9 çıkış 2

  %0.1 = -30 dB izolasyon: kabul edilebilir
  %1.0 = -20 dB izolasyon: sorunlu

Kaynaklar:
  AWG üretim hatası → kanal ayırma kusurlu
  Ring resonatör sıcaklık kayması → yanlış λ'yı seçiyor
  Dalga kılavuzu evanescent sızıntı → komşu kanala geçiş
```

**Kaçınılabilir mi:** **Büyük ölçüde** — daha iyi AWG tasarımı, ring resonatör sıcaklık kontrolü, geniş kanal aralığı ile azaltılabilir.

---

### Gürültü 7 — Karanlık Akım (Dark Current)

**Fiziksel köken:** Fotodedektörde ışıksız akım.

```
Fotodedektör (fotodiode):
  Foton gelir → elektron-hole çifti oluşur → akım
  Bu istenen sinyal

Ama:
  Isıl enerji de elektron-hole çifti oluşturabilir
  Işık olmasa da küçük akım var: "karanlık akım"

Tipik değerler:
  Silikon fotodiyot (oda sıcaklığı): ~1 nA
  InGaAs fotodiyot (oda sıcaklığı): ~10 nA
  Soğutulmuş (−40°C): ~0.01 nA → 1000× daha az

  Düşük optik güçlerde karanlık akım sinyal/gürültü oranını bozar
```

**Kaçınılabilir mi:** **Büyük ölçüde** — soğutma ile veya daha iyi malzeme (InGaAs, Ge, SNSPD) ile azaltılabilir.

---

## Bölüm 3 — Klasik Azaltma Teknikleri

### Teknik 1 — Balans Algılama (Balanced Detection)

Bu tek teknikle RIN gürültüsü neredeyse tamamen yokediliyor.

```
Tek dedektör (normal):
  Işık → dedektör → akım
  RIN dahil: dalgalanma gözlemlenir

Balans algılama:
  ışık
   │
   ├──50%──► Dedektör 1 → akım I₁
   │          (sinyal + gürültü + RIN)
   │
   └──50%──► Dedektör 2 → akım I₂
              (sinyal + gürültü + RIN)
              (ama ters faz referans)

  Çıkış: I₁ - I₂

  RIN her iki dedektörde aynı:
  I₁_RIN ≈ I₂_RIN  →  I₁_RIN - I₂_RIN ≈ 0 (iptal!)

  Sinyal ters fazda:
  I₁_signal = +A,  I₂_signal = -A
  I₁ - I₂ = 2A  (2× sinyal güçlenme)

Sonuç:
  RIN: -150 dB/Hz → -180+ dB/Hz (30 dB iyileşme)
  Atım gürültüsü: aynı kalır (temel sınır)
  Sinyal: 2× güçlenir (bonus)
```

**FE+EO CIM'de kullanım:** MZI çıkışı zaten iki port üretiyor — balans algılama doğal olarak entegre edilebilir.

---

### Teknik 2 — Koherent Algılama (Homodyne / Heterodyne)

```
Doğrudan algılama:
  Işık → fotodedektör → |E|² ölçülür (güç)
  Faz bilgisi KAYBOLUR

Koherent algılama:
  Sinyal ışık + Yerel osilatör (LO) lazer → birlikte algıla

  ┌──────────┐
  │ Sinyal   │──┐
  └──────────┘  │   90° hibrit    ┌──────────────┐
                ├────────────────►│I₁, I₂, Q₁, Q₂│→ I, Q değerleri
  ┌──────────┐  │   bağlaşıcı     └──────────────┘
  │ LO lazer │──┘
  └──────────┘

  Sonuç:
  → Faz da amplitude de ölçülür
  → Gürültü: LO gücüyle sinyali "amplify" eder
  → Termal gürültü bastırılır, atım gürültüsü limitine ulaşılır
  → SNR maksimize edilir

  Faydası:
  10 μW sinyal, doğrudan: termal gürültü domine
  10 μW sinyal, koherent (1 mW LO): atım gürültüsüne ulaşır
  → 100× iyileşme!
```

---

### Teknik 3 — Kilitleme Amplifikasyonu (Lock-in Detection)

```
Fikir: sinyali belirli frekansta modüle et, gürültüyü filtrele

  Sinyal modülasyonu: f_mod = 1 kHz
  ─────────┐ ┌───────┐ ┌───────
           └─┘       └─┘

  Gürültü: her frekansda (beyaz gürültü)
  ──────────────────────────────

  Lock-in amplifier:
  Yalnızca f_mod etrafındaki dar bant seçer → gürültü %99 azalır

  Bant genişliği: 1 Hz (f_mod etrafında)
  Gürültü: B ∝ bant genişliği → √(1Hz / 1GHz) = 31,623× azalma!

  Dezavantaj: ölçüm çok yavaşlar (1/bant genişliği = 1 sn/örnek)
  → Yüksek doğruluk gereken kalibrasyonda kullanılır
```

---

### Teknik 4 — Sinyal Entegrasyonu (Averaging)

```
Fizik: atım gürültüsü ∝ √N (N = foton sayısı)
       SNR ∝ √N

Pratik: daha uzun ölç → daha fazla foton → daha iyi SNR

  τ = 1 ns:  N = 1000 foton,  SNR = 31.6
  τ = 100 ns: N = 100,000,   SNR = 316   (10× iyileşme)
  τ = 10 μs:  N = 10,000,000, SNR = 3162 (100× iyileşme)

CIM'de ne anlama gelir:
  Hızlı okuma (1 ns): düşük SNR → az bit/hücre
  Yavaş okuma (100 ns): yüksek SNR → çok bit/hücre

  Hız-hassasiyet ödünleşimi: daha yüksek hassasiyet için daha yavaş
```

---

### Teknik 5 — Dedektör Soğutma

```
Termal ve karanlık akım gürültüsü sıcaklığa bağlı:
  σ_thermal ∝ √T
  I_dark ∝ exp(-E_g / kT)

Soğutma seviyeleri ve kazanımlar:
  +25°C (oda):  referans
  −40°C (TEC):  karanlık akım ÷ 100
  −196°C (N₂):  karanlık akım ÷ 10⁷
  −269°C (He):  karanlık akım ÷ ∞ (pratik sıfır)

SNSPD (Superconducting Nanowire Single Photon Detector):
  Çalışma sıcaklığı: 0.8 - 4 K
  Karanlık sayım hızı: < 1 Hz
  Kuantum verimliliği: %95+
  Tek foton algıla: evet!
  
  Dezavantaj: kriyojenik soğutma sistemi gerekli
              → büyük, pahalı, güç harcayan
              → veri merkezi uygulamasında uygulanabilir
              → cep telefonunda değil
```

---

## Bölüm 4 — Kuantum Teknikler: Temel Sınırı Aşmak

Bu bölüm, atım gürültüsünün bile geçilebileceğini gösteriyor.

### 4.1 Heisenberg Belirsizliği ve Optik

```
Kuantum mekaniğinde ışık alanı iki bileşenden oluşur:
  X₁ (genlik kuadratür): I_out'u belirler
  X₂ (faz kuadratür):   φ'yi belirler

Heisenberg:
  ΔX₁ × ΔX₂ ≥ 1/4

Koherent durum (normal lazer):
  ΔX₁ = ΔX₂ = 1/2 (belirsizlik eşit paylaşılmış)
  Bu STANDART KUANTUM LİMİTİ (SQL) = atım gürültüsü

Soru: belirsizliği eşit paylaştırmak ZORUNDA mıyız?
Cevap: HAYIR!

ΔX₁ × ΔX₂ = 1/4 kuralı korunduğu sürece:
  ΔX₁ = 0.1 (çok az gürültü), ΔX₂ = 2.5 (çok fazla gürültü)
  X₁'i ölçüyorsak: gürültü 5× azaldı!
  (ΔX₂ çok büyüdü ama onu ölçmüyoruz)

Bu: SIKIŞTIRMANIN (SQUEEZING) TEMELİ
```

### 4.2 Sıkıştırılmış Işık (Squeezed Light)

```
Normal koherent durum:        Sıkıştırılmış durum:
  X₂                           X₂
  │ ●  ●  ●                    │ ●●●●●
  │●   ●   ●                   │ ●●●●●
  │  ●  ●  ●   → X₁            │   ●●●●● → X₁
  │●   ●   ●                   │ ●●●●●
  │ ●  ●  ●                    │ ●●●●●
  
  Daire şeklinde               Elips: X₁'de sıkışmış
  belirsizlik                  X₂'de genişlemiş

Ölçüm: X₁ kuadratürü ölçüyoruz (genlik/yoğunluk)
  Normal: ΔX₁ = 0.5 (SQL)
  Sıkıştırılmış: ΔX₁ = 0.1 (SQL'in 5× altı!)
  SNR: 5× iyileşme = 7 dB → 1 bit daha fazla!
```

**LIGO'da kullanım — kanıtlanmış teknoloji:**

```
LIGO (gravitasyonel dalga dedektörü):
  4 km boyunda dev interferometre
  10⁻¹⁸ m mesafe değişimi ölçüyor (protonun yarıçapının 1/1000'i!)

  Atım gürültüsü bu kadar küçük ölçümde problem
  Çözüm: sıkıştırılmış ışık enjeksiyonu

  LIGO güncel sonuç:
  15 dB sıkıştırma → SNR 5.6× iyileşme
  → Daha küçük sinyaller algılanabiliyor

Fotonik CIM'e uygulanabilir mi?
  Teorik: evet → her MZI hesaplaması 6+ dB iyileşebilir
  Pratik zorluk:
  → Sıkıştırma kaynağı: komplike (optik parametrik osilatör)
  → Sıkıştırma dalga kılavuzunda korunmalı (kayıp bozar)
  → Her hesaplama noktasına sıkıştırılmış ışık gerekli
  → Şu an: araştırma aşamasında, 10-15 yıl uzak
```

### 4.3 Foton Sayısı Durumu (Fock State) — Ultimate Sınır

```
N fotonla mümkün olan en iyi ölçüm:

  Standart kuantum limiti (koherent ışık):
  Δφ_SQL = 1/√N

  Heisenberg limiti (maksimum dolanık durum):
  Δφ_Heisenberg = 1/N

  Fark: √N faktörü
  N = 1000 foton: SQL = 1/31.6, Heisenberg = 1/1000
  → 31.6× daha iyi!

Gerekli: NOON durumu |N,0⟩ + |0,N⟩
  N foton ya hep bir kolda ya hep diğerinde
  → Kuantum dolanık durum

Pratik zorluk:
  N=2: gösterildi (basit)
  N=100: son derece zor (dekoherans)
  N=10⁶: şu an imkânsız
  → Fotonik CIM için uzak gelecek
```

---

## Bölüm 5 — FE+EO CIM'de Gürültü Analizi

### 5.1 Hangi Gürültüler Ne Kadar Önemli

```
FE+EO CIM okuma zinciri:
  LO lazer + sinyal lazer
       │
  MZI (FE polarizasyon etkisi)
       │
  Fotodedektör (balans)
       │
  Okuma devresi

Her aşamada gürültü:

1. Lazer RIN:           -150 dB/Hz başlangıç
   Balans dedeksiyonu ile: -180 dB/Hz → neredeyse sıfır

2. Atım gürültüsü:      SNR = √(η P τ / hν)
   1mW, 1ns:            SNR ≈ 5000 → ~12 bit etkin
   Bu TEMELsınır!

3. Faz gürültüsü:       DFB lazer ~100kHz linewidth
   Koherent algılamada: hesaplamalar arasında önemli değil
   (her okuma çok kısa)

4. Termal drift (EO):   LiNbO₃ dn/dT ≈ 10⁻⁴/°C
   1°C sapma → Δn ≈ 10⁻⁴ → faz kayması ~0.04π
   → Ternary için tolere edilebilir, analog için sorun
   Çözüm: sıcaklık kontrolü (TEC) + aktif kalibrasyon

5. Dalga kılavuzu kayıp: 0.1-1 dB/cm
   MZI kolu 100μm: ~0.01 dB → ihmal edilebilir

6. WDM crosstalk:       -30 dB izolasyon tipik
   512 kanal sistemi: önemli (kanal sayısı artınca zorunlu)
   Çözüm: AWG kalitesi + geniş kanal aralığı
```

### 5.2 Pratik SNR Bütçesi

```
FE+EO CIM için SNR bütçesi (1mW lazer, 1ns okuma):

  Başlangıç (atım gürültüsü sınırı):  +35 dB
  Detektor kuantum verimliliği (85%):  -0.7 dB
  Balans algılama (RIN iptali):        +3 dB
  Dalga kılavuzu ek kaybı:             -1 dB
  WDM demux ek kaybı:                  -1.5 dB
  Elektriksel okuma gürültüsü:         -1 dB
  Sıcaklık sapması (±0.1°C):          -0.5 dB
  Fabrikasyon uyumsuzluğu:             -1 dB
  ─────────────────────────────────────────────
  Net SNR:                             ~32 dB
  Etkin bit/hücre: log₂(1+1585) ≈     10.6 bit

  Hedef: 8-10 bit → mevcut sistem yeterli!
```

---

## Bölüm 6 — Azaltma Stratejilerinin Karşılaştırması

```
Teknik              Hangi gürültü   Azaltma    Maliyet
────────────────────────────────────────────────────────────
Balans algılama     RIN             30-40 dB   Düşük (1 ekstra diyot)
Koherent algılama   Termal          20-30 dB   Orta (LO lazer)
Soğutma (-40°C)     Karanlık akım   20 dB      Düşük (TEC)
Soğutma (4K)        Termal+dark     40+ dB     Yüksek (kriyostat)
SNSPD               Her şey         50+ dB     Çok yüksek
Dar çizgi lazer     Faz gürültüsü   30 dB      Orta
Sıkıştırılmış ışık  Atım gürültüsü  6-15 dB    Çok yüksek, araştırma
Heisenberg limiti   Atım gürültüsü  √N → N     Pratik değil (şimdi)
Daha uzun entegr.   Atım gürültüsü  10dB/100×τ Hız ödünü
EUV litografi       Saçılma         5-10 dB    Fabrika maliyeti
Sıcaklık kontrolü   Termal drift    10-20 dB   Orta (TEC+kontrol)
────────────────────────────────────────────────────────────
```

---

## Bölüm 7 — 400B Model için Pratik Öneriler

```
FE+EO CIM'de gürültü yönetimi önerileri:

Kısa vade (şimdi uygulanabilir):
  ✓ Balans algılama → RIN'i yoket (ucuz, etkili)
  ✓ DFB lazer (dar çizgi) → faz gürültüsü azalt
  ✓ TEC soğutma (+25°C → -10°C) → dark current ÷ 30
  ✓ AWG kanal ayrımı ≥ 200 GHz → crosstalk -40 dB
  ✓ EUV litografi → dalga kılavuzu pürüzü ÷ 3

  Net SNR: ~32 dB → 10 bit/hücre
  400B × 10 bit / 8 = 500 GB ama saf analog

  Ternary için: 10 bit >> gerekli 1.585 bit
  → Ternary CIM'de gürültü SORUN DEĞİL
  → Analog CIM'de drift ana sorun (gürültü değil!)

Orta vade (2028-2032):
  ✓ Entegre optik sıkıştırma → +6 dB
  ✓ SNSPD (seçici kanallar için) → +20 dB
  ✓ Photon number resolving detector → +3 dB
  
  Net SNR: ~40 dB → 13 bit/hücre
  Hibrit FP-FEO: bu SNR'da mükemmel çalışır

Uzun vade (2035+):
  ✓ On-chip squeezed light → +10-15 dB
  ✓ Kriyo-CMOS entegrasyonu → 4K'da çalışma
  
  Net SNR: ~50 dB → 16 bit/hücre
  → FP16 hassasiyetini tek hücrede yakala!

En kritik gürültü FE+EO CIM için:
  GÜRÜLTÜ değil → DRIFT!
  Ferroelektrik polarizasyon kayması zamanla
  Analog değer bozuluyor → periyodik kalibrasyon şart
  Gürültü mühendislikle %95 çözüldü
  Drift hâlâ açık araştırma sorusu
```

---

## Özet: Gürültü Türleri ve Çözüm Yolu

```
Gürültü türü        Köken              Azaltılabilir mi?
────────────────────────────────────────────────────────
Atım gürültüsü      Foton kuantumu     Evet (sıkıştırmayla)
                                       Hayır (klasik teknikle)
RIN                 Lazer spontan em.  Evet → balans ile sıfıra yakın
Termal (Johnson)    Elektron hareketi  Evet → soğutmayla
Faz gürültüsü       Lazer linewidth    Evet → dar çizgi lazer
Karanlık akım       Termal üretim      Evet → soğutma, SNSPD
Saçılma gürültüsü   Fabrikasyon        Evet → daha iyi litografi
WDM crosstalk       Kanal sızıntısı    Evet → AWG kalitesi

Temel mesaj:
  Gürültü fizik yasası = atım gürültüsü
  Geri kalan hepsi mühendislik sorunu

  FE+EO CIM için asıl sorun:
  Gürültü değil → drift (analog değerin zamanla kayması)
  Bu ayrım kritik: sorunlar farklı, çözümler farklı.
```
