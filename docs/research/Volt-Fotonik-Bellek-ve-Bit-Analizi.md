> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Fotonik Bellek ve Fotonik Bit — Temel Analiz

> Fotonik hesaplama tartışmasının en kritik eksik halkası bellek.
> Hesaplama ışık hızında yapılabiliyor ama bilgi hâlâ elektronik
> DRAM'de — bu yapısal uyumsuzluk çözülmeden fotonik'in tam
> potansiyeli gerçekleşemiyor. Bu belge fotonik bitin ne olduğunu,
> fotonik belleğin nasıl çalıştığını ve önündeki engelleri inceler.

---

## Bölüm 1 — Fotonik Bit Nedir

### 1.1 Elektronik Bitten Farkı

```
Elektronik bit:
  Fiziksel taşıyıcı: elektron
  Depolama mekanizması: kapasitör yükü (DRAM)
                        transistör durumu (SRAM)
                        ferroelektrik polarizasyon (FeFET)
  "0" = düşük gerilim, "1" = yüksek gerilim
  Kararlı: elektron kapasitörde oturur, bekler

Fotonik bit:
  Fiziksel taşıyıcı: foton
  Temel sorun: foton DURAMAZ
    → Işık hızında hareket eder
    → Durdurmak = yok etmek (soğurma)
    → "Bekleme" kavramı fotona yabancı

  Bu fotonik belleğin kök zorluğu:
  "Fotonu nasıl tutarsın?"
```

### 1.2 Fotonik Bilgi Kodlama Yöntemleri

Foton duramasa da bilgiyi ışığın **özelliklerinde** taşıyabilir:

```
Kodlama 1 — Genlik (On/Off):
  Işık var → 1
  Işık yok → 0
  En basit, en yaygın
  Enerji verimliliği düşük (kapalıyken foton yok)

Kodlama 2 — Faz:
  0° faz → 0
  180° faz → 1
  Ternary: 0°/120°/240° veya 0°/90°/180°
  Koherent tespit gerekir

Kodlama 3 — Polarizasyon:
  Yatay → 0
  Dikey → 1
  Diyagonal → ternary üçüncü durum
  Doğal Stokes parametresi

Kodlama 4 — Dalga Boyu (WDM):
  λ₁ → kanal 1 (farklı bilgi)
  λ₂ → kanal 2
  λ₃ → kanal 3
  Tek fiber/dalga kılavuzda çok bit

Kodlama 5 — Orbital Açısal Momentum:
  l = -1, 0, +1 → ternary doğal
  Vorteks ışın modları
  Yüksek kapasite ama karmaşık üretim

Dengeli ternary için en doğal:
  Faz kodlaması: {0°, 180°} binary → {0°, 90°, 180°} ternary değil
  En saf: {+E, 0, -E} amplitude+phase → +1, 0, -1
  MZI ile θ={0, π, 2π} → doğal üç nokta
```

### 1.3 Fotonik Bit ile Elektronik Bitin Karşılaştırması

```
Özellik           Elektronik Bit    Fotonik Bit
────────────────────────────────────────────────
Taşıyıcı          Elektron          Foton
Hız               ~elektrik hızı    Işık hızı
Enerji/bit-metre  yüksek (Joule/m)  çok düşük
Kararlılık        yüksek (bekler)   yok (hareket eder)
Depolama          kolay             çok zor
İletim            kısa mesafe verimli  uzun mesafe verimli
Boyut             ~2-5 nm (7nm node)   ~200-1000 nm (λ sınırı)
Termal kayıp      yüksek            çok düşük
Çoğullama         karmaşık          doğal (WDM)
```

---

## Bölüm 2 — Fotonik Bellek Türleri

### Tür 1 — Optik Gecikme Hatları (Geçici Bellek)

```
En basit fotonik bellek: ışığı bir döngüde tut

  ┌──────────────────────────────┐
  │   Optik dalga kılavuzu       │
  │   ╭──────────────────────╮   │
  │   │  ışık döngüde dolaşır│   │
  │   ╰──────────────────────╯   │
  └──────────────────────────────┘

  Bellek süresi = çevre uzunluğu / ışık hızı
  10cm döngü: 10cm / 2×10⁸ m/s ≈ 0.5 ns

  Kullanım: optik paket anahtarlama tamponu
  Sorun:
    → Kayıp: her turda güç azalır
    → Kalıcı değil: ışık durmaz, sadece döner
    → Yenileme gerekir (optik amplifikatör)
    → Yoğunluk düşük: 10cm × sinyal = büyük alan

Bu "gerçek" bellek değil — geçici tampon.
```

### Tür 2 — Faz Değişimli Malzeme (PCM) Bellek — Ana Yaklaşım

```
GST (Ge₂Sb₂Te₅) — en çok çalışılan malzeme:

İki kararlı durum (optik):

  AMORF durum:                KRISTALIN durum:
  Atomlar düzensiz            Atomlar düzenli ızgara
  
  ⊙⊙ ⊙ ⊙⊙                   ⊙ ⊙ ⊙ ⊙ ⊙
  ⊙  ⊙⊙ ⊙                   ⊙ ⊙ ⊙ ⊙ ⊙
  ⊙⊙  ⊙ ⊙                   ⊙ ⊙ ⊙ ⊙ ⊙

  Optik özellik:              Optik özellik:
  Yansıma düşük               Yansıma yüksek
  Geçirgenlik yüksek          Geçirgenlik düşük
  → fotonik bit = 0           → fotonik bit = 1
  (veya tersi, konvansiyona göre)

Yazma mekanizması:
  Amorf → Kristalin (SET):
    Uzun, düşük güçlü lazer darbesi
    Yavaş ısıtma + soğutma → kristalinleşme
    ~100 ns darbe süresi

  Kristalin → Amorf (RESET):
    Kısa, yüksek güçlü lazer darbesi
    Hızlı ısıtma (erime) + hızlı soğutma → amorf
    ~10 ns darbe süresi

Okuma mekanizması:
  Zayıf lazer darbesi → yansıma ölç
  Yüksek yansıma = kristalin = 1
  Düşük yansıma  = amorf    = 0
  Okuma: <1 ns, malzeme durumunu değiştirmez (non-destructive)
```

### Tür 3 — Kısmi Kristallenme ile Çok Seviyeli (Ternary)

```
PCM'in güzelliği: kısmi kristallenme kontrol edilebilirse
çok seviyeli depolama mümkün:

  0% kristalin   → amorf      → trit = -1
  50% kristalin  → ara durum  → trit =  0
  100% kristalin → kristalin  → trit = +1

  Optik ölçüm:
  Yansıma katsayısı 0 ile 1 arasında sürekli
  → 3 seviye = ternary
  → 8 seviye = 3-bit (laboratuvarda gösterildi)
  → 64 seviye = 6-bit (teorik, pratikte zor)

Oxford Üniversitesi demosu (2019):
  GST nano-disk + silikon dalga kılavuzu
  8 seviyeli (3-bit) optik depolama
  Okuma: 1 ns
  Yazma: ~100 ns
  Alan: ~1 μm²
```

### Tür 4 — Fotonik DRAM Konsepti

```
DRAM'ın optik karşılığı: yenile veya kaybet

  Optik rezonatör (ring resonator):
  Enerji depolanır, zamanla kaybolur (Q faktörü)
  Yenileme: optik darbe ile enerji tazele

  Benzer DRAM'a:
    Depolama: rezonatörde foton
    Yenileme: ~ns periyot (DRAM: ~64ms)
    Çok daha sık yenileme → enerji maliyeti yüksek

  Kullanım: yüksek hız tampon (L1 cache analog)
  Kalıcı depolama için uygun değil
```

### Tür 5 — Nadir Toprak Katkılı Dalga Kılavuzları

```
Erbiyum (Er³⁺) veya Praseodimyum (Pr³⁺) katkılı cam:
  Uyarılmış elektronik durum → uzun ömürlü
  Optik pompalama → enerji depolanır
  Uyarı: foton yayımlanır → okuma

  Teorik olarak çok ilgi çekici:
  ✓ Foton direkt depolanıyor (elektronik dönüşüm yok)
  ✓ Çok uzun ömür mümkün (teorik)

  Pratikte:
  ✗ Oda sıcaklığında çok kısa ömür (pikosaniyeler)
  ✗ Kriyo soğutma gerekir (birkaç Kelvin)
  ✗ Kuantum bellek bölgesine giriyor
  ✗ Genel bilgisayar için pratik değil (henüz)
```

---

## Bölüm 3 — PCM Fotonik Belleğin Mekanizması Derinlemesine

### 3.1 Termal Dinamik — Yazmanın Fiziksel Temeli

```
GST erime noktası: ~620°C
GST kristallenme sıcaklığı: ~150-200°C

SET işlemi (amorf → kristalin):
  T
  │    620°C ─────────────────── erime noktası
  │
  │    200°C ───────   ──────── kristallenme bölgesi
  │              ╲   ╱
  │    25°C ──────╲─╱────────── oda sıcaklığı
  └─────────────────────────────► zaman
                 ↑   ↑
              ısıt  yavaş soğu
              (lazer) → kristalin oluşur

RESET işlemi (kristalin → amorf):
  T
  │    620°C ────────────────── erime noktası
  │              ████
  │             █    █
  │    25°C ───█──────█──────── oda sıcaklığı
  └─────────────────────────────► zaman
                 ↑    ↑
              hızlı  çok hızlı soğu
              ısı    → amorf (dondu)

Kısmi kristallenme (ternary):
  SET darbesinin enerjisi/süresi kontrol edilir
  → Kısmen kristalin yapı
  → Ara optik durum → trit = 0
  → Kesin kontrol çok zor
```

### 3.2 Silikon Fotonik Entegrasyon

```
GST + Si dalga kılavuzu yapısı:

  ┌──────────────────────────────────────────┐
  │          SiO₂ üst kaplama                │
  │  ┌─────────────────────────────────┐     │
  │  │   GST film (~5-20 nm kalınlık)  │     │
  │  └─────────────────────────────────┘     │
  │  ┌─────────────────────────────────┐     │
  │  │   Si dalga kılavuzu             │     │
  │  └─────────────────────────────────┘     │
  │          SiO₂ alt kaplama                │
  └──────────────────────────────────────────┘

  Si dalga kılavuzu üzerindeki ışık GST ile etkileşir:
  → Amorf GST: az soğurma → ışık geçer (0)
  → Kristalin GST: çok soğurma → ışık azalır (1)

  Yazma lazeri: ayrı, güçlü, kısa dalga boyunda
  Okuma lazeri: zayıf, veri dalga boyunda

  CMOS uyumluluk:
  GST kaplama standart Si-fotonik sürecine eklenebilir
  → back-end-of-line (BEOL) adımı
  → Ön uç transistörler etkilenmez
```

---

## Bölüm 4 — Önündeki Engeller ve Zorluklar

### Engel 1 — Tutma Süresi (Retention)

```
Problem: Amorf durum termodinamik olarak KARARSIZ
  Kristalin durum enerji olarak daha düşük
  → Amorf kendiliğinden kristaline dönme eğiliminde

  Dönüşüm hızı sıcaklığa dağılım:
    25°C (oda): yıllar (kabul edilebilir)
    70°C (laptop):  haftalar (sorunlu)
    85°C (datacenter sunucu): günler (kabul edilemez)

  Arrhenius yasası:
  t_retention = A × e^(Eₐ/kT)
  Sıcaklık 20°C artar → tutma süresi ~10-100x azalır

  Sonuç:
  Mobil/soğuk ortam: GST çalışabilir
  Datacenter/sıcak ortam: çok ciddi problem

Çözüm denemeleri:
  → GSST (Ge₂Sb₂Se₄Te₁): daha yüksek amorf kararlılık
    Se atomunun Te'nin yerini alması → daha derin enerji çukuru
    85°C'de >10 yıl (teorik) — umut verici ama hâlâ araştırma
  → Kaplama katmanları: SiN veya Al₂O₃ ile kapla
    → Dış ortamdan yalıt → oksidasyon ve difüzyon engelle
  → Periyodik yenileme: DRAM gibi oku + yeniden yaz
    → Enerji maliyeti ekler, ama çalışır
```

### Engel 2 — Yazma Döngüsü Dayanıklılığı (Endurance)

```
Her SET/RESET döngüsü malzemeyi yıpratır:

  Termal döngü etkisi:
  ısıt → soğu → ısıt → soğu → ...
  → Malzeme genleşme/büzülme yorgunluğu
  → Ayrışma (demixing): Ge, Sb, Te birbirinden ayrılır
  → Elementel segregasyon → optik özellik değişir
  → Belirli döngü sonrası "takılı kalır" bir durumda

  GST endurance:
    Lab koşulları: ~10⁶ döngü (1 milyon)
    Elektronik PCM (Intel Optane): ~10⁷ döngü
    DRAM karşılaştırması: pratik olarak sınırsız
    NAND Flash: ~10⁴ döngü (SLC)

  Fotonik PCM neden elektronik PCM'den kötü:
    → Lazer darbesi daha küçük hacmi daha şiddetli ısıtır
    → Termal gradyan daha keskin → daha fazla stres
    → ~10⁵ döngü pratik hedef (şu an)

Çözüm denemeleri:
  → Aşınma dengeleme (wear leveling):
    Yazmaları hücrelere eşit dağıt (SSD'nin yaptığı gibi)
  → Yeni malzemeler: GeTe, Sb₂S₃, In₂Se₃
    → Daha yumuşak faz geçişi → daha az termal stres
  → Hibrit: az yazılan veriler için PCM, çok yazılan için sram
```

### Engel 3 — Yazma Enerjisi Paradoksu

```
Fotonik'in avantajı: okuma ve hesaplama çok ucuz
Fotonik'in paradoksu: yazma çok pahalı

  Okuma enerjisi:  ~0.1 fJ/bit (foton saymak ucuz)
  Yazma enerjisi:  ~10-100 pJ/bit (termal süreç pahalı)

  Asimetri: 10⁵ - 10⁶ kat fark

  Neden:
  Faz geçişi termal süreç → istenilen hacmi erimeleri için
  enerji = kütle × özgül ısı × ΔT + erime ısısı
  GST için minimal enerji: ~1 fJ/bit teorik
  Pratik: lazer verim + ısı kaybı = ~10-100 pJ/bit

  Hesaplama verimliliğini ne kadar bozuyor:
  Yazma nadirdir (ağırlıklar önceden yüklenir,
  çıkarımda değişmez) → kabul edilebilir
  Ama ağırlık güncellemesi (online öğrenme) gerekirse
  → PCM yazma enerjisi problematik

Çözüm denemeleri:
  → Daha küçük GST hacim: nano-boyut → daha az enerji
    ~(10nm)³ GST: teorik minimum ~0.01 pJ/bit
  → Optik rezonans: GST'yi rezonatör moduna getir
    → Lazer enerjisi seçici soğurulur → verimlilik artar
  → Yardımcı ısı: Joule ısıtma + lazer beraber
    → Toplam daha az enerji (sinerjik)
```

### Engel 4 — Ternary İçin Kısmi Kristallenme Kontrolü

```
En kritik ternary zorluğu:
"Tam olarak %50 kristalinleşmek istiyorum"

Problem 1 — Stokastik çekirdek oluşumu:
  Kristallenme çekirdek (nucleus) oluşumundan başlar
  Çekirdek oluşumu: stokastik kuantum süreç
  → Aynı lazer darbesi → farklı kristallenme yüzdesi
  → Trit = 0 durumu her seferinde biraz farklı

Problem 2 — Drift (kayma):
  Kısmi kristalin durum zamanla kayıyor:
  → %50 → %55 → %60 → sonunda tam kristalin
  RRAM'da aynı problem biliniyor (Müller ve ark. 2014)
  
  Drift modeli:
  C(t) = C₀ × (t/t₀)^α     α ≈ 0.1 (ampirik)
  
  1 μs sonra: %50 kristalin
  1 ms sonra: %52 kristalin
  1 s sonra:  %56 kristalin
  → Trit değeri zamanla değişiyor!

Problem 3 — Termal karışım:
  Hücre ısındığında komşu hücrenin sıcaklığı da artar
  → Komşu hücrenin kristallenme yüzdesi bozuluyor
  → Yakın paketlemede ciddi hata kaynağı

Çözüm denemeleri:
  → Kapalı döngü kontrol:
    Yaz → oku → kontrol et → fark varsa düzelt
    Converge algoritması: her yazma kalibre edilmiş
    Maliyet: 3-5x daha fazla yazma döngüsü

  → Drift kompansasyonu:
    Drift modeli biliniyorsa okuma zamanında düzelt
    C_corrected = C_measured / (t/t₀)^α
    Ama modelin doğruluğu sınırlı

  → Geniş pencere tasarımı:
    Trit -1: C < %20 kristalin
    Trit 0:  C = %40-60 kristalin (geniş pencere)
    Trit +1: C > %80 kristalin
    → Drift toleransı artar ama gürültü marjı azalır
```

### Engel 5 — Boyut Sınırı (Difraksiyon Limiti)

```
Fotonik boyutu sınırlayan temel fizik:
  Abbe difraksiyon limiti: d_min ≈ λ / (2n)
  λ = 1550 nm (telecom dalgası), n = 3.5 (Si)
  d_min ≈ 1550 / (2 × 3.5) ≈ 220 nm

  En küçük Si dalga kılavuzu: ~220 nm × 500 nm
  En küçük fotonik bellek hücresi: ~500 nm × 500 nm

  Elektronik ile karşılaştırma:
  SRAM hücresi (5nm node): ~30 nm × 30 nm → ~900 nm²
  Fotonik bellek hücresi:  ~500 nm × 500 nm → 250,000 nm²

  Yoğunluk farkı: 280x daha az yoğun (alan olarak)
  → Aynı yongada çok daha az bellek kapasitesi

Çözüm denemeleri:
  → Plazmonik dalga kılavuzu: metal-dielektrik ara yüz
    → Elektrik alan sıkıştırılır → sub-wavelength
    → Ama kayıp çok yüksek (metal soğurma)

  → Hybrid plasmonic-photonic: kısa mesafe plasmon + uzun mesafe foton
    → Denge: biraz daha küçük, biraz daha fazla kayıp

  → Nanofotonik rezonatör:
    → Yüksek-Q pikocavity: çok küçük mod hacmi
    → ~(λ/n)³ / 1000 = ~(70 nm)³ mod hacmi teorik
    → Pratikte kayıp problemi

  Gerçekçi sonuç:
  Fotonik bellek yoğunluğu elektronik DRAM'a ulaşamaz
  → Nişi: yüksek hız, düşük gecikme, cache benzeri kullanım
  → Ana depolama değil, hızlandırıcı tampon
```

### Engel 6 — Termal Çapraz Konuşma (Thermal Crosstalk)

```
Yazma sırasında ısı yayılımı:

  Hücre A yaz:
  ┌─────────────────────────────┐
  │ [A: 620°C] [B: 180°C] [C: 90°C] │
  │  (yazılıyor) (etkileniyor) (az etki)│
  └─────────────────────────────┘

  B'nin kristallenme durumu değişti!
  → B'nin saklandığı trit değeri bozuldu

  Isı yayılım uzunluğu GST'de:
  l_thermal = √(κ × τ) ≈ √(0.5 × 10⁻⁹) ≈ 20 nm
  
  Hücreler arası minimum mesafe: ~100 nm güvenli
  → Yoğunluğu daha da sınırlıyor

Çözüm denemeleri:
  → Termal bariyerler: SiO₂ yalıtım çukurları
    → Isı yayılımını engeller
    → Alan maliyeti ekler

  → Sıralı yazma: komşular aynı anda yazılmaz
    → Yazma hızı düşer

  → Düşük güç yazma + yüksek sensitivite okuma
    → Isı üretimi azalır → crosstalk azalır
```

### Engel 7 — CMOS Entegrasyon Zorlukları

```
GST neden standart CMOS'a zor:

Malzeme uyumsuzluğu:
  GST: Germanyum + Antimon + Tellür
  CMOS: Silikon + Oksit + Nitrür + Metal (Al, Cu, W)
  
  Sorunlar:
  → Tellür: Si'a diffüze eder → transistör özelliklerini bozar
  → Antimon: kontaminasyon riski → cleanroom politikaları
  → Yüksek fırın sıcaklığı gerektirmeyen düşük-T GST şart

  BEOL kısıtları:
  Transistörler yapıldıktan sonra ekleme (back-end):
  → Maksimum sıcaklık: ~400°C
  → GST kristallenme sıcaklığı: 150°C (güvenli)
  → GST depolama: ~250°C PVD sputtering (uyumlu)

  Neden halinden daha iyi değil:
  → Intel Optane (elektronik PCM): aynı entegrasyon sorununu
    çözdü → ticari başarı
  → Fotonik PCM: ek Si dalga kılavuzu + optik bileşen şart
  → Her ek bileşen = daha fazla süreç adımı = maliyet

CMOS-fotonik birleştirme seçenekleri:
  1. Monolitik entegrasyon: aynı yonga, yoğun (zor, pahalı)
  2. 3D yığın: fotonik yonga + CMOS yonga üst üste
     → TSV (Through-Silicon Via) bağlantı
     → IBM ve Intel bu yolda
  3. Chiplet: ayrı fotonik çip, yan yana, kısa bağlantı
     → En pratik yaklaşım şu an
```

### Engel 8 — Okuma Bozulması (Read Disturb)

```
Okuma lazeri de enerji taşır → malzemeyi etkiler?

Düşük güçlü okuma lazeri:
  → Teorik olarak non-destructive
  → Ama tekrar eden okumalar kümülatif ısı biriktirir
  → Özellikle kısmi kristalin (trit=0) durumda hassas

Deneysel gözlem:
  10⁶ kez oku → kristallenme yüzdesi değişiyor
  → "Read fatigue" sorunu
  → Kritik görev uygulamalarında sorun

Çözüm:
  → Daha düşük güçlü okuma (ama sinyal-gürültü azalır)
  → Dalga boyu ayarı: soğurma minimumda oku
  → Önceden yenile: uzun süre okunduysa yaz-yenile
```

---

## Bölüm 5 — Güncel Araştırma Durumu

### 5.1 Başarılar

```
2015 — Oxford / Cambridge landmark:
  GST + Si dalga kılavuzu: ilk fotonik bellekl gösterimi
  Yüksek hızlı multi-level (8 seviye) depolama
  Doğa dergisi Nature Photonics'te yayım

2019 — Oxford 8-seviyeli:
  Tek GST hücresinde 8 farklı optik durum
  3-bit eşdeğeri
  Okuma doğruluğu: %99.7

2021 — IBM Zürich:
  PCM + fotonik: çıkarım için in-memory hesaplama demo
  Matris-vektör çarpımı: optik okuma + PCM ağırlık
  10 TOPS/W (elektronik GPU'nun 10x iyi tarafı)

2022 — EPFL:
  GSST (yeni malzeme): daha iyi retansiyon
  Geniş bantlı şeffaflık (telecom penceresinde kayıp az)

2023 — MIT + Analog Devices:
  Entegre fotonik-elektronik PCM chip demo
  64 hücre × 8 seviye → 384 bit
  Küçük ama ilk "sistem" demonstrasyonu
```

### 5.2 Hâlâ Çözülmemiş

```
Açık problemler (2024 itibarıyla):
  ✗ >85°C'de uzun süreli retansiyon
  ✗ >10⁷ döngü dayanıklılık
  ✗ Ternary drift kompansasyonu (pratik)
  ✗ 1 cm²'de >1 MB kapasitesi
  ✗ Tekrarlanabilir üretim (hücre-hücre uyumsuzluğu)
  ✗ Datacenterde çalışma sıcaklığında kanıtlanmış sistem
```

---

## Bölüm 6 — Fotonik Ternary Bellek: Özel Zorluklar

Ternary bellek binary'nin tüm zorluklarını taşır üstüne özgün zorluklar ekler:

### 6.1 Üç Durumun Kararlılık Pencereleri

```
Binary PCM:
  Durum 0 (amorf):   C = 0-20%  → geniş pencere ✓
  Durum 1 (kristalin): C = 80-100% → geniş pencere ✓
  Aradaki boşluk büyük → gürültüye dayanıklı

Ternary PCM:
  Trit -1: C = 0-25%   → pencere: 25%
  Trit  0: C = 35-65%  → pencere: 30%  ← en hassas
  Trit +1: C = 75-100% → pencere: 25%

  Gürültü marjı binary'nin ~yarısı
  Drift bu pencereleri bozar:
    1 saniye sonra trit 0 → trit +1'e kayabilir
```

### 6.2 Kalibrasyonun Sürekliliği

```
Her hücre biraz farklı davranıyor:

  Hücre A: %50 kristalin için 50 pJ lazer
  Hücre B: %50 kristalin için 45 pJ lazer
  Hücre C: %50 kristalin için 58 pJ lazer

  Fabrikasyon sapması: ~±15%
  → Her hücre ayrı kalibre edilmeli

  1 MB fotonik ternary bellek:
  = 1,000,000 hücre × her biri için kalibrasyon
  = Uzun kalibrasyon süresi
  = Kalibrasyon verisi depolanmalı (meta-bellek sorunu)

  Çözüm: self-calibrating yazma döngüsü
    Yaz → oku → kontrol et → düzelt → tekrar
    Convergence: ~3-5 döngü
    Maliyet: yazma zamanı 3-5x uzar
```

---

## Bölüm 7 — Volt Ekosisteminde Fotonik Bellek

### 7.1 Tip Sistemi Gereksinimleri

```volt
// Fotonik bellek hücresinin Volt tipi
type PhotonicMemCell = {
    state        : PCMState,        // amorf/kısmi/kristalin
    crystallinity: Percentage,      // %0-100
    write_count  : u32,             // endurance takibi
    last_refresh : Timestamp,       // retention takibi
}

// PCM durumu — ternary doğal
enum PCMState { Amorphous, Partial, Crystalline }

// Ternary fotonik bellek dizisi
module PhotonicTernaryMemory<const N: usize> {
    in  addr   : bits<log2(N)>  @Digital
    in  data_w : Trit            @Digital
    in  wen    : bool            @Digital
    out data_r : Trit            @Digital

    // Okuma: fotonik (hızlı, ucuz)
    @read_cost(energy=0.1.fJ, latency=1.ns)

    // Yazma: termal (yavaş, pahalı)
    @write_cost(energy=50.pJ, latency=100.ns)

    // Endurance takibi
    @endurance(max_cycles=1_000_000)
    invariant: all_cells.write_count < 1_000_000

    // Retention uyarısı
    @retention(temp=25.celsius, duration=10.years)
    invariant: elapsed_since(last_refresh) < retention_limit(temp)

    // Ternary drift kompansasyonu
    @drift_model(alpha=0.1, reference_time=1.us)
    fn read_corrected(raw: Percentage, elapsed: Duration) -> Trit {
        let corrected = raw / pow(elapsed / 1.us, 0.1)
        classify_trit(corrected)
    }
}
```

### 7.2 Mimari OS'un Fotonik Bellek Yönetimi

```
Mimari OS fotonik bellek için özel görevler:

1. Endurance Yönetimi:
   Her yazma işlemini say
   Aşım yaklaşınca → wear leveling
   Kritik eşik → hücreyi devre dışı bırak

2. Retention Monitörü:
   Sıcaklık sensörü → retention ömrü hesapla
   Kritik veriler → periyodik yenileme zamanla
   Drift kompansasyonu → okuma zamanı düzelt

3. Termal Yönetim:
   Yazma işlemleri → komşu hücre koruma
   Termal harita → hangi hücreler son ısındı?
   Sıralama: komşu hücrelere sıralı yazma

4. Kalibrasyon Günlüğü:
   Her hücre için kalibrasyon parametreleri
   Sıcaklığa göre adaptif güncelleme
   Güç kaybında kurtarma: flash'ta yedek

Bu yönetim bugünkü SSD firmware'inin
fotonik ternary karşılığı — bilinen problem,
çözümler ölçeklenebilir.
```

---

## Bölüm 8 — Gerçekçi Zaman Çizelgesi

```
Bugün (2024-2025):
  Lab ortamı: GST fotonik bellek çalışıyor
  Kapasite: KB ölçeği demonstrasyonlar
  Endurance: ~10⁵ döngü (laboratuvar)
  Ternary: demo seviyesinde, drift problemi açık

Yakın vade (2026-2028):
  GSST olgunlaşması: daha iyi retansiyon
  İlk entegre PCM-fotonik çip ürünleri
  Kapasite: MB ölçeği
  Ternary drift: kısmen kompanse edilmiş

Orta vade (2028-2032):
  Si-fotonik CMOS entegrasyon (chiplet)
  Kapasite: GB ölçeği (özel uygulama)
  Fotonik nöral ağ ağırlık deposu
  Ternary: ürün seviyesi (sınırlı uygulama)

Uzun vade (2032+):
  Fotonik hesaplama + fotonik bellek birleşimi
  E-O-E dönüşümü minimumlarda
  Gerçek "all-optical" işlemci
  Fotonik ternary: yaygın uygulama
```

---

## Özet: Engellerin Ağırlık Matrisi

```
Engel                   Zorluk   Çözüm durumu   Volt etkisi
───────────────────────────────────────────────────────────────
Retansiyon (sıcaklık)   Yüksek   GSST umut ver.  @retention tip
Endurance (~10⁵)        Orta     Wear leveling   @endurance say.
Yazma enerjisi          Orta     Nano-boyut      @write_cost
Ternary drift           Çok yük. Kompansasyon    drift_model
Boyut sınırı (difraksiyon) Fizik  Aşılamaz (kısmen)  kapasite kısıt
Termal crosstalk        Orta     Bariyerler      sıralı yazma
CMOS entegrasyon        Orta     Chiplet         extern module
Read disturb            Düşük    Düşük güç       periyodik yenile
Ternary kalibrasyon     Yüksek   Self-calib.     kalibrasyon günlük

En kritik iki engel:
1. Retansiyon (datacenter sıcaklığında): fiziksel kimya problemi
2. Ternary drift: malzeme mühendisliği + algoritma problemi

Bu iki engel çözülürse:
   Fotonik ternary bellek → Von Neumann'ın "veri taşıma" sınırını
   kavramsal olarak ortadan kaldırır
   → Hesaplama nerede bellekte de orada
   → LLM çıkarımı için devrimsel
```

---

## Tek Cümle Özet

Fotonik bit ışığın genlik, faz veya polarizasyon özelliklerinde kodlanan bilgidir; fotonik bellek bu bilgiyi faz değişimli malzemelerin (özellikle GST) amorf/kristalin geçişinde kalıcı olarak saklayan yapıdır — ternary için üçüncü durum kısmi kristalinleşmeyle elde edilir; önündeki en büyük engeller yüksek sıcaklıkta retansiyon kaybı ve kısmi kristalinleşmenin zamanla kayması (drift) olup her ikisi de malzeme mühendisliği ve algoritma kompansasyonunun kesişiminde çözülmeyi bekleyen açık araştırma sorularıdır.
