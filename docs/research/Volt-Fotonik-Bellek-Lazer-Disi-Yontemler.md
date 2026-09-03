> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Fotonik Bellek — Lazer Dışı Yazma ve Okuma Yöntemleri

> Lazer, fotonik bellekte hem yazma hem okumada tercih edilen araç
> ama zorunlu değil. Yazma ve okuma mekanizmaları birbirinden bağımsız
> seçilebilir — ve bu ayrım, hem enerji verimliliğini hem entegrasyon
> kolaylığını dramatik biçimde değiştirebilir. Bu belge her iki yön için
> lazer dışı alternatifleri, fiziksel temellerini ve ternary özel
> durumunu inceler.

---

## Bölüm 1 — Neden Lazer Dışı Yöntemler Önemli

```
Lazerin iki ayrı rolü var — ve her biri farklı sorun:

Yazma için lazer:
  Problem: termal süreç → yüksek enerji (~10-100 pJ/bit)
  Problem: gürültülü → kısmi kristalinleşme kontrol zor
  Problem: boyut sınırı → difraksiyon limiti (~λ/2)
  Problem: entegrasyon → ayrı lazer kaynağı gerekir

Okuma için lazer:
  Problem: read disturb → okuma ısıtıyor, durum bozuluyor
  Problem: enerji → zayıf da olsa foton tüketimi
  Problem: bant genişliği → lazer modülasyonu sınırlı
  Problem: gürültü → shot noise, RIN (intensity noise)

Ayrı mekanizma seçmenin kazancı:
  Yazma mekanizması: enerji verimliliği + hassas kontrol
  Okuma mekanizması: hız + non-destructive + düşük gürültü

  En iyi yazma ≠ En iyi okuma → ikisini ayır
```

---

## Bölüm 2 — Lazer Dışı YAZMA Yöntemleri

### Yöntem 1 — Joule Isıtma (Elektriksel Nano-Isıtıcı)

**Temel fizik:**

```
Lazer ısıtma:           Joule ısıtma:
  Foton → soğurma         Elektrik akımı → direnç ısısı
  → ısı → faz geçişi      → ısı → faz geçişi

  Isıtma mekanizması aynı (termal)
  Ama kaynak tamamen farklı

Yapı:
  ┌──────────────────────────────────────┐
  │     SiO₂ yalıtım                    │
  │  ┌─────────────────────┐             │
  │  │   GST film (5-20nm) │             │
  │  └─────────────────────┘             │
  │  ┌─────────────────────┐             │
  │  │ TiN nano-ısıtıcı    │←─── akım  │
  │  │ (10-50nm kalınlık)  │             │
  │  └─────────────────────┘             │
  │  ┌─────────────────────┐             │
  │  │ Si dalga kılavuzu   │←─── okuma  │
  │  └─────────────────────┘      ışığı │
  └──────────────────────────────────────┘

  Akım pulse → TiN ısınır → GST'ye ısı iletimi
  → SET: uzun/zayıf pulse → kristalin
  → RESET: kısa/güçlü pulse → amorf
```

**Enerji karşılaştırması:**

```
Lazer yazma (GST, mevcut):  10-100 pJ/bit
Joule yazma (TiN nano):      1-10 pJ/bit  → 10x daha iyi

Neden daha verimli:
  → Lazer: tüm ışık demeti → büyük hacim ısıtılıyor
  → TiN: yalnızca GST temas alanı ısıtılıyor
  → Daha az ısı kaybı → daha az enerji
  → Ölçek küçüldükçe verimlilik artar

Termal crosstalk karşılaştırması:
  Lazer: difraksiyon sınırı ~220nm → geniş ısıtma alanı
  TiN nano: ısıtıcı genişliği ~10-20nm → çok daha yerel
  → Komşu hücre etkisi dramatik azalır
```

**CMOS uyumluluğu:**

```
TiN (Titanyum Nitrür):
  ✓ Standart CMOS sürecinde mevcut
    (tungsten plug, barrier layer olarak kullanılıyor)
  ✓ 400°C altında depolama → BEOL uyumlu
  ✓ Yüksek erime noktası (~2930°C) → dayanıklı
  ✓ Elektriksel kontrol → DAC ile hassas akım kontrolü

Intel Optane (elektronik PCM) bu yaklaşımı kullanıyor:
  → Joule ısıtma + direnç okuma
  → Volt, aynı ısıtıcıyı optik okuma ile birleştirir
```

**Ternary için özel avantaj:**

```
Kısmi kristalinleşme kontrolü:
  Lazer: foton sayısı kontrolü zor (shot noise)
  Joule: akım seviyesi DAC ile çok hassas kontrol edilir

  Akım-kristallenme eğrisi:
  I < I₁  → amorf (trit -1)
  I₁ < I < I₂ → kısmi kristalin (trit 0)  ← hassas kontrol!
  I > I₂  → tam kristalin (trit +1)

  DAC çözünürlüğü 12-bit → kristallenme %0.02 hassasiyetle
  → Ternary drift kompansasyonu + yazma kolay
```

---

### Yöntem 2 — Elektrik Alan Anahtarlaması (Ferroelektrik/VO₂)

**Temel fikir:**

```
Bazı malzemeler termal değil, elektrik alanıyla değişir:

Vanadyum Dioksit (VO₂):
  Mott insülatör-metal geçişi (IMT):
  
  İzolasyon fazı:          Metal fazı:
  V-O ızgara bükük         V-O düz
  Yüksek direnç            Düşük direnç
  Yüksek optik saçılma     Düşük saçılma
  → trit -1                → trit +1

  Tetikleme:
  A) Isıl: T > 68°C → metal faz
  B) Elektrik alan: yeterince güçlü E → metal faz
  C) Gerinim: mekanik stres → faz geçişi
  D) Işık: yoğun lazer → ama termal değil elektronik
```

**Elektrik alan yazma mekanizması:**

```
VO₂ + kapasitör yapısı:

  ┌──────────────┐
  │  Üst elektrot│ ←─── +V (yüksek voltaj)
  ├──────────────┤
  │     VO₂      │  ← elektrik alan → faz geçişi
  ├──────────────┤
  │  Alt elektrot│ ←─── -V
  └──────────────┘

  Avantaj:
  ✓ Termal değil → daha hızlı (<1 ns)
  ✓ Daha az enerji (kapasitör şarjı: E = ½CV²)
  ✓ Endurance potansiyel olarak yüksek
     (termal yorgunluk yok)

  Dezavantaj:
  ✗ Voltaj yüksek (~10V) → CMOS düşük voltaj gerekir
  ✗ Tutma (retention): alan kaldırınca durum geri döner
     → Ferroelektrik kapı katmanı eklenmeli
  ✗ VO₂ faz geçişi sürekliliği sınırlı hücre
```

**Ferroelektrik + Elektro-Optik kombinasyonu:**

```
LiNbO₃ (Lityum Niobat):
  Güçlü Pockels etkisi: elektrik alan → kırılma indisi değişimi
  Δn = -(n³/2) × r₃₃ × E
  r₃₃ = 30 pm/V (çok büyük)

  Yazma: elektrik alan (CMOS DAC)
  Okuma: kırılma indisi değişimi → faz kayması → interferometri

  Ternary:
    E = 0 V/μm  → n değişmez → trit 0
    E = +E₀     → n artar → trit +1
    E = -E₀     → n azalır → trit -1

  Bu tam dengeli üçlü!
  Ve tamamen termal değil → endurance çok yüksek

  Sorun: kalıcı depolama yok (alan kaldırınca sıfırlanır)
  Çözüm: Ferroelektrik LiNbO₃ → kalıcı polarizasyon
```

---

### Yöntem 3 — İyon İnterkülasyonu (Elektrokromik Yazma)

**Temel fikir:**

```
İyonlar malzemeye girip çıkarak optik özelliği değiştirir:

WO₃ (Tungsten Trioksit) elektrokromik:
  H⁺ veya Li⁺ iyonu + elektron → renkli durum

  WO₃ (şeffaf)  +  H⁺  +  e⁻  →  H₀.₃WO₃ (mavi)
  şeffaf                              renkli
  trit -1                             trit +1

  Kısmi: 0-30% H⁺ doldurma → sürekli renk skalası
  → Ternary doğal: az H⁺ (trit -1), orta (trit 0), çok (trit +1)

Yazma:
  Elektrokimyasal hücre → elektroliz
  Voltaj uygula → H⁺ iyonları hareket eder
  Enerji: ~0.1-1 pJ/ion (çok düşük!)

Okuma:
  Optik geçirgenlik → lazer veya LED ile
```

**Enerji analizi:**

```
İyon hareketinin enerjisi:
  E = q × V_drive × n_ions
  q = 1.6×10⁻¹⁹ C (elektron yükü)
  V = 0.5 V (tipik elektrokimyasal)
  n = 100 iyon (küçük hücre için)
  E = 1.6×10⁻¹⁹ × 0.5 × 100 = 8×10⁻¹⁸ J = 8 aJ

  Teorik: 8 aJ/write (atto-joule!)
  Pratik: ~0.1-1 pJ (bağlantı + kayıplar dahil)

  Lazer ile karşılaştırma: 50-100 pJ
  İyon yazma: 0.1 pJ → 500-1000x daha verimli!

  Neden bu kadar ucuz:
  → Foton termal ısıtıyor → enerji büyük çoğunluğu ısı
  → İyon tam hedefli kimyasal reaksiyon → verimli
```

**Zorlukları:**

```
✗ Hız: ms-saniye ölçeğinde (termal PCM: ns)
   → Yavaş, ama bazı uygulamalar için yeterli
   → LLM ağırlıkları çıkarımda değişmez → yavaş yazma OK

✗ Endurance: iyon difüzyon hasarı
   → ~10⁴-10⁵ döngü (PCM ile benzer)

✗ Elektrolit entegrasyonu:
   → Katı elektrolit (LiPON gibi) → karmaşık süreç

✓ Retansiyon: iyonlar sıkıştırıldıktan sonra sabit
   → Yüksek sıcaklıkta bile iyi retansiyon
   → Termal kararlılık PCM'den çok iyi
```

---

### Yöntem 4 — Manyetik Yazma (Spintronik + Magneto-Optik)

**Temel fikir:**

```
Manyetik depolama + optik okuma kombinasyonu:

Yazma: Spin-Transfer Torque (STT) veya Spin-Orbit Torque (SOT)
  Elektrik akımı → spin polarize elektron → manyetizasyon değiştir

  ↑ manyetizasyon → trit +1
  → manyetizasyon → trit 0  (diyagonal)
  ↓ manyetizasyon → trit -1

Okuma: Magneto-Optik Kerr Etkisi (MOKE)
  Polarize ışık yansır → manyetizasyon yönüne göre polarizasyon döner
  Polarizasyon açısı ölç → manyetizasyon yönü = depolanan değer

Ternary:
  Üç manyetik durum: ↑, →, ↓ (üç açısal yönelim)
  MOKE rotasyon açısı: +θ, 0, -θ → trit +1, 0, -1
```

**STT-MRAM'ın olağanüstü endurance avantajı:**

```
Mevcut endurance karşılaştırması:
  GST fotonik PCM:    ~10⁵ döngü
  Elektronik PCM:     ~10⁷ döngü
  NAND Flash (SLC):   ~10⁵ döngü
  STT-MRAM:           >10¹⁵ döngü  ← devrimsel

  STT-MRAM neden bu kadar dayanıklı:
  → Manyetik geçiş: spin yönü değişir
  → Termal hasar yok (Joule ısıtma yok)
  → Atomik yapı değişmiyor

  Magneto-optik fotonik bellek:
  STT yazma + MOKE okuma
  → Endurance sorunu çözülüyor
  → Sadece okuma için ışık (veya bu da değiştirilebilir)
```

**Zorlukları:**

```
✗ Boyut: manyetik alan dağılımı → hücreler arası etkileşim
   Manyetik alan: uzun menzilli (1/r² düşüş)
   → Komşu hücreler etkileşir → yoğun paketleme zor

✗ Ternary manyetik durum kararlılığı:
   Üç kararlı manyetik yön → küçük boyutta kararsız
   Şekil anizotropi + malzeme anizotropi dengelenmeli

✗ Magneto-optik entegrasyon:
   YIG (Yttrium Iron Garnet): iyi MOKE ama Si uyumsuz
   → Heterogen entegrasyon gerekli

✓ Hız: ~ns yazma (manyetik geçiş hızlı)
✓ Enerji yazma: ~0.1-1 pJ/bit (STT ile)
✓ Termal kararlılık: manyetizasyon sıcaklığa daha dayanıklı
```

---

### Yöntem 5 — Fotokimyasal Anahtarlama (Moleküler Bellek)

**Temel fikir:**

```
Kimyasal reaksiyon optik özelliği değiştirir:

Diarylethene molekülü:
  
  Açık form (renksiz):    Kapalı form (renkli):
      S     S                  S───S
     ╱ ╲   ╱ ╲                ╱     ╲
    /   ╲ /   ╲              /       ╲
   /     X     ╲            /    ○    ╲
  UV ışık → kapatır
  Görünür ışık → açar
  
  veya elektrik alan ile tetiklenebilir (yeni araştırma)

Optik özellik:
  Açık form: UV soğurma (görünürde şeffaf)
  Kapalı form: görünür soğurma (renkli/opak)
  → Fotonik dalga kılavuzunda geçirgenlik farklı → okuma

Avantaj:
  ✓ Termal süreç değil → endurance yok denecek kadar yüksek
    (Diarylethene: >10⁷ döngü gösterildi)
  ✓ Oda sıcaklığında kararlı her iki durum
  ✓ Yazma enerjisi: ~0.01 pJ/molekül (düşük)
```

**Zorlukları:**

```
✗ Hız: pikosaniye-nanosaniye (termal ile benzer)
   Moleküler konformasyon değişimi: ~ns
   Bulk malzeme için: yavaş difüzyon sorunu

✗ Tek molekül vs film:
   Tek molekül = ideal
   Film = moleküller birbirini engeller, yavaş geçiş

✗ Si-fotonik entegrasyon:
   Organik molekül + inorganik Si → uyumluluk sorunu
   Self-assembled monolayer (SAM) → araştırma aşaması

✗ Ternary:
   İki durum doğal (açık/kapalı)
   Üçüncü durum için karışık film gerekir
   → Kontrol zor
```

---

### Yöntem 6 — Piezoelektrik / Mekanik Anahtarlama

**Temel fikir:**

```
Mekanik stres → kristal yapı değişimi → optik özellik değişimi

MEMS tabanlı fotonik bellek:
  Mikro-levye (cantilever) + dalga kılavuzu:
  
  Levye uzakta:   Levye yakında:
  ┌──────────┐    ┌──────────┐
  │          │    │  levye   │ ← evanesant bölge
  │dalga k.  │    │dalga k.  │   etkileşir
  └──────────┘    └──────────┘
  Az kayıp        Yüksek kayıp
  trit -1         trit +1

  Levye pozisyonu: elektrostatik kuvvetle kontrol
  → Yazma: voltaj → elektrostatik → mekanik hareket
  → Okuma: optik geçirgenlik

Alternatif: Piezoelektrik gerilim → kırılma indisi (elasto-optik)
  AlN (Alüminyum Nitrür): güçlü piezo ve Si uyumlu
  Voltaj → AlN gerilimi → Δn → faz kayması
```

**Değerlendirme:**

```
✓ Termal değil → endurance yüksek
✓ CMOS voltajlarında çalışır
✓ Geri alınabilir (reversible) → sonsuz döngü teorik

✗ Hız: mekanik rezonans → MHz ölçeği (yavaş)
✗ Boyut: MEMS elemanları büyük (μm ölçeği)
✗ Kalıcılık: voltaj kaldırınca durum döner
   → Sürekli güç gerekir (latching mekanizması şart)
✗ Vibrasyon hassasiyeti: dış titreşimden etkilenir

En iyi uygulama: yüksek döngü + düşük hız gereken özel bellek
               silikon fotonik optical switch (bellek değil)
```

---

## Bölüm 3 — Lazer Dışı OKUMA Yöntemleri

### Okuma Yöntemi 1 — Elektriksel Direnç (Hibrit Yaklaşım)

```
GST'nin kararlı elektriksel özelliği:
  Amorf GST:    R > 1 MΩ (yüksek direnç)
  Kristalin:    R < 10 kΩ (düşük direnç)
  Oran: >100:1 → net ayrım

Okuma:
  Küçük akım (sense current) uygula → gerilim ölç
  Yüksek gerilim → yüksek direnç → amorf → trit -1
  Düşük gerilim → düşük direnç → kristalin → trit +1

Enerji:
  V_sense = 0.1V, I_sense = 1μA, t = 10ns
  E = V × I × t = 0.1 × 10⁻⁶ × 10⁻⁸ = 10⁻¹⁵ J = 1 fJ
  → Lazer okuma ile karşılaştırılabilir

Avantaj:
  ✓ CMOS tamamen uyumlu — ayrı ışık kaynağı yok
  ✓ Hızlı: elektronik okuma < 1 ns
  ✓ Non-destructive: sense akımı düşük tutulursa
  ✓ Gürültü: elektronik amplifikatör olgunlaşmış

Dezavantaj:
  ✗ Fotonik hesaplama ile entegrasyon:
    Direnç okuma → dijital → optik dönüşüm gerekir
    → E-D-O (Elektrik-Dijital-Optik) dönüşüm zinciri

Ternary:
  R_amorf : R_kısmi : R_kristalin = 1000 : 10 : 1
  Üç direnç seviyesi → ADC ile üç trit değeri
  Joule yazma + direnç okuma → tam elektriksel ternary
```

---

### Okuma Yöntemi 2 — Kapasitans Ölçümü

```
GST dielektrik sabiti:
  Amorf:    ε ≈ 25
  Kristalin: ε ≈ 50
  Oran: 2:1 (dirençten küçük ama kullanılabilir)

Ternary:
  Kısmi kristalin: ε ≈ 35-40
  Üç seviye: 25, 37, 50 → oran 2:1.5:1

Okuma:
  Paralel plaka kapasitör yapısı
  C = ε₀ × ε × A / d
  Küçük AC sinyali → kapasitans köprüsü → ε ölç

Avantaj:
  ✓ Ultra-düşük enerji: kapasitör ölçme ~0.1 fJ
  ✓ Non-destructive: AC sinyali küçük
  ✓ Hız: GHz frekans → < 1 ns okuma

Dezavantaj:
  ✗ Küçük oran (2:1) → gürültüye hassas
  ✗ Parazit kapasitans: komşu devreler bozar
  ✗ Ternary için ayrım zor (1.5:1 oran)
```

---

### Okuma Yöntemi 3 — Kuantum Nokta / NV Merkezi (Yakın Alan)

```
NV merkezi (Nitrojen-Vakans, elmas içinde):
  Spin durumu optik olarak okunabilir
  Yakın çevredeki manyetik/elektrik alan → spin etkiler

  PCM'in yakınına NV merkezi koy:
  Amorf GST → farklı lokal alan → farklı spin rezonansı
  Kristalin GST → farklı lokal alan → farklı spin rezonansı

  Okuma: mikrodalga + yeşil lazer (ama bu "okuma lazeri" değil
          spin hazırlama lazeri — çok farklı enerji rejimi)

Avantaj:
  ✓ Atom ölçeğinde yerellik → sıfır crosstalk
  ✓ Çok hassas: tek bit tespit teorik mümkün
  ✓ Non-destructive: NV spin PCM'i etkilemez

Dezavantaj:
  ✗ Şu an kriyo gerektirir (4K-300K range, materyale bağlı)
  ✗ Entegrasyon: elmas + silikon fotonik → zor
  ✗ Okuma hızı: spin relaksasyon süresi ~μs
  ✗ Olgunluk: tamamen araştırma aşaması
```

---

### Okuma Yöntemi 4 — Termal İletkenlik Ölçümü

```
GST termal iletkenlik farkı:
  Amorf:    κ ≈ 0.2 W/mK
  Kristalin: κ ≈ 1.5 W/mK
  Oran: 7.5:1 → büyük fark

Okuma prensibi:
  Nano-ısıtıcı küçük güç ile ısıt
  Sıcaklık artışı ölç (termokupul veya direnç termometresi)
  Yüksek ΔT → düşük κ → amorf
  Düşük ΔT → yüksek κ → kristalin

Değerlendirme:
  ✓ Tamamen elektriksel — ışık yok
  ✗ Yazan ve okuyan aynı ısıtıcı → karışıklık
  ✗ Yavaş: termal denge ms ölçeği
  ✗ Crosstalk: ısı komşu hücreye yayılır
  → Gelecek araştırma; bugün pratik değil
```

---

### Okuma Yöntemi 5 — Evanescent Alan Sensörü (Lazer-Free)

```
Evanescent alan: dalga kılavuzu sınırından taşan elektrik alanı
  PCM evanescent alanla etkileşir → iletim değişir
  Bu mevcut yaklaşım ama lazer ile

Lazersiz evanescent okuma:
  LED (Light Emitting Diode): lazer değil
  → Tek frekanslı değil, geniş bantlı
  → Koherent değil ama okuma için yeterli

  LED avantajı:
  ✓ Lazerden 100x ucuz
  ✓ Daha basit sürücü devresi
  ✓ Titreşim ve gürültü az (koherent değil → speckle yok)
  ✓ Uzun ömür (VCSEL lazerin ömrünün 10x)

  Dezavantaj:
  ✗ Modülasyon hızı: LED ~100 MHz, lazer ~50 GHz
  ✗ Işık odaklama: ışın kalitesi düşük
  ✗ Sinyal-gürültü: koherent tespit yapılamaz

  Uygulama: düşük hız okuma (ağırlık yükleme) → LED yeterli
             yüksek hız veri akışı → lazer şart
```

---

## Bölüm 4 — En Umut Verici Hibrit Kombinasyonlar

Yazma ve okuma mekanizması bağımsız seçilebilir.
En iyi kombinasyon hedeflenen uygulamaya göre değişir:

### Kombinasyon A — Joule Yazma + Optik Okuma (En Olgun)

```
Joule yazma (TiN nano-ısıtıcı):
  Enerji: ~1-5 pJ/bit
  Hassasiyet: DAC kontrollü → ternary için ideal
  CMOS uyumu: tam
  Endurance: PCM sınırlı (~10⁶)

Optik okuma (zayıf lazer veya LED):
  Enerji: ~0.1 fJ/okuma
  Hız: < 1 ns
  Non-destructive: evet

Net kazanç:
  Yazma: 10x daha verimli (lazerden)
  Okuma: aynı hız ve enerji
  Ternary: çok daha güvenilir (DAC hassasiyet)
  CMOS: tam uyumlu

→ En gerçekçi kısa vadeli çözüm
→ Intel Optane yazma mekanizması + Si-fotonik okuma
→ 2-3 yıl içinde demo mümkün
```

### Kombinasyon B — İyon Yazma + Optik Okuma (En Verimli)

```
İyon yazma (elektrokimyasal, WO₃):
  Enerji: ~0.1-1 pJ/bit (lazerden 100x ucuz)
  Hız: ms-s (yavaş)
  Retansiyon: mükemmel (sıcaklığa dayanıklı)
  Ternary: doğal (kısmi iyon dolumu)

Optik okuma (dalga kılavuzu geçirgenlik):
  Enerji: ~0.1 fJ/okuma
  Hız: <1 ns

Net kazanç:
  Yazma: 100-1000x daha verimli
  Retansiyon: PCM'den çok iyi
  Ternary: doğal ve stabil
  
Dezavantaj:
  Yavaş yazma → sadece ağırlık yükleme için uygun
  (LLM çıkarımı: ağırlıklar çıkarımda değişmez → yavaş yazma OK)

→ LLM inference chip için ideal uzun vade çözüm
→ Bir kez yaz, milyarlarca kez oku
```

### Kombinasyon C — STT Yazma + MOKE Okuma (En Dayanıklı)

```
STT yazma (Spin-Transfer Torque):
  Enerji: ~0.1-1 pJ/bit
  Hız: < 1 ns
  Endurance: >10¹⁵ döngü  ← devrimsel
  CMOS uyumu: mevcut STT-MRAM süreciyle

MOKE okuma (Magneto-Optik Kerr Etkisi):
  Polarize ışık → yansıma polarizasyon analizi
  Enerji: ~0.01 pJ/okuma
  Hız: < 1 ns

Net kazanç:
  Endurance: PCM'in 10⁹ katı
  Hız: hem yazma hem okuma çok hızlı
  Enerji: dengeli

Dezavantaj:
  Manyetik ternary: üç yön → kararlılık zorluğu
  MOKE entegrasyonu: YIG malzeme Si uyumsuz
  
→ Yüksek endurance gerektiren uygulamalar
→ Online öğrenme (sürekli ağırlık güncelleme) için ideal
→ 5-7 yıl araştırma gerektiriyor
```

### Kombinasyon D — Elektrik Alan Yazma + Kapasitans Okuma (Tam Elektriksel)

```
Her şey elektriksel — foton hiç yok:

FeFET yazma:
  Ferroelektrik polarizasyon → eşik voltaj değişir
  Üç durum: yukarı/nötr/aşağı polarizasyon = ternary
  Enerji: ~1 fJ/bit (çok düşük!)

Kapasitans okuma:
  Polarizasyon durumu → kapasitans farkı
  Elektriksel ölçüm → trit değeri

Ama bu fotonik bellek değil — elektronik!
Optik entegrasyon:
  FeFET ağırlık → Pockels modülatör → optik sinyal
  → Yazma/okuma elektriksel, hesaplama optik

Net kazanç:
  Enerji yazma: 1 fJ (inanılmaz düşük)
  Endurance: FeFET ~10⁸ döngü
  Fotonik entegrasyon: Pockels etkisi ile

→ En radikal yaklaşım
→ Volt'ta: "elektriksel ağırlık, optik hesaplama"
```

---

## Bölüm 5 — Ternary için Hangi Kombinasyon?

```
Ternary zorluğu: üçüncü durum kararlılığı ve kontrolü

Yazma yöntemi × Ternary uyumluluk:

  Lazer (mevcut):     ❌ kötü  (stokastik nükleasyon, drift)
  Joule (TiN nano):   ✅ iyi   (DAC hassasiyeti, feedback loop)
  İyon (WO₃):         ✅ çok iyi (doğal sürekli → 3 seviye seçimi kolay)
  Manyetik (STT):     ⚠️ orta  (3 açısal yön zor)
  Elektrik alan:       ⚠️ orta  (FeFET 3 durum mümkün ama karmaşık)
  Fotokimyasal:       ❌ kötü  (doğal 2 durum)
  Mekanik:            ❌ kötü  (3 pozisyon zor)

Okuma yöntemi × Ternary ayrım kalitesi:

  Lazer (mevcut):      ✅ iyi   (sürekli soğurma ölçümü)
  Direnç:              ✅ iyi   (100:10:1 oran)
  Kapasitans:          ⚠️ orta  (2:1.5:1 oran, küçük)
  MOKE:                ✅ iyi   (+θ:0:-θ açı)
  Termal:              ❌ kötü  (yavaş, crosstalk)

En iyi ternary kombinasyonu:

  #1: İyon yazma + Direnç/Optik okuma
      → Doğal ternary, iyi retansiyon, ucuz yazma
      Zorluk: yavaş yazma (ms), ama LLM için OK

  #2: Joule yazma + Optik okuma
      → CMOS uyumlu, DAC kontrolü, bugün mümkün
      Zorluk: drift hâlâ var, ama daha az
```

---

## Bölüm 6 — Volt Tip Sistemi: Yeni Köprü Tipleri

```volt
// Lazer dışı yazma mekanizmaları için tip sistemi

// Joule ısıtma yazma köprüsü
bridge JouleWrite<const Bits: u8> {
    in  trit    : Trit       @DigitalControl
    in  current : u12        // DAC çıkışı (12-bit hassasiyet)
    out state   : PCMState   @PhotonicTernary

    @mechanism(JouleHeating)
    @heater(material=TiN, size=20.nm)
    @cost(energy=2.pJ, latency=100.ns)   // lazerin 50x iyisi
    @endurance(max_cycles=1_000_000)

    // DAC hassasiyet → ternary kontrol
    fn current_to_trit(i: u12) -> Trit {
        match i {
            0..=1365    => Trit::Neg,    // amorf bölge
            1366..=2730 => Trit::Zero,   // kısmi kristalin
            2731..=4095 => Trit::Pos     // tam kristalin
        }
    }
}

// İyon yazma köprüsü
bridge IonicWrite {
    in  trit    : Trit       @DigitalControl
    in  charge  : femtocoul  // yük miktarı
    out state   : ElectrochromicState @PhotonicTernary

    @mechanism(IonIntercalation)
    @material(WO3_electrolyte)
    @cost(energy=0.1.pJ, latency=1.ms)  // yavaş ama çok ucuz
    @endurance(max_cycles=100_000)
    @retention(temp=85.celsius, duration=10.years)  // iyi!

    // Yük miktarı → iyonizasyon yüzdesi → trit
    assert: absolute_error(achieved_trit, target_trit) < 0.1
}

// STT manyetik yazma + MOKE okuma
bridge MagneticWrite {
    in  trit    : Trit       @DigitalControl
    in  current : pA         // spin polarize akım
    out mag_state : MagneticState @MagnotoOptic

    @mechanism(SpinTransferTorque)
    @cost(energy=0.5.pJ, latency=1.ns)
    @endurance(max_cycles=1_000_000_000_000_000)  // 10¹⁵!
}

bridge MOKERead {
    in  mag_state : MagneticState @MagnotoOptic
    in  probe     : PolarizedLight @Photonic
    out trit      : Trit      @Digital

    @mechanism(MagnetoOpticKerrEffect)
    @cost(energy=0.01.pJ, latency=0.5.ns)
    // Non-destructive: manyetizasyon etkilenmez
    #[non_destructive]
}

// Adaptif yazma seçici (Mimari OS tarafından kullanılır)
module AdaptivePhotonicMemory<const N: usize> {
    in  addr      : bits<log2(N)>   @Digital
    in  data_w    : Trit             @Digital
    in  wen       : bool             @Digital
    in  write_mode: WriteMode        @ControlPlane
    out data_r    : Trit             @Digital

    // Mimari OS yazma modunu seçer
    match write_mode {
        WriteMode::Fast     => JouleWrite(data_w)
        WriteMode::Efficient => IonicWrite(data_w)
        WriteMode::Durable  => MagneticWrite(data_w)
    }

    // Okuma her zaman optik (hızlı, non-destructive)
    data_r = OpticalRead(addr)

    // Volt doğrulaması
    invariant: endurance_ok(write_mode, write_count)
    invariant: retention_ok(write_mode, temp, elapsed)
}
```

---

## Özet: Yöntem Seçim Kılavuzu

```
Uygulama              Yazma           Okuma          Neden
──────────────────────────────────────────────────────────────
LLM inference ağırlık İyon (WO₃)     Optik (LED)    Ucuz yaz, hızlı oku
Online öğrenme        STT manyetik    MOKE           Sonsuz döngü
Edge AI (pil)         Joule (TiN)    Optik           CMOS uyumlu, hızlı
Yüksek sıcaklık       İyon            Direnç         İyi retansiyon
Araştırma/demo        Joule (TiN)    Lazer           Bugün mümkün
Ternary (kısa vade)   Joule (TiN)    Optik           DAC kontrolü

Temel ilke:
  Lazer zorunlu değil — ne yazma ne okumada.
  Lazer en olgun çözüm, ama her boyut için en iyi değil.
  Yazma ve okumayı ayırmak:
    → Her birini en uygun fiziksel mekanizmayla çöz
    → Toplam sistem verimliliği dramatik artar
    → Ternary kontrolü çok daha güvenilir
```
