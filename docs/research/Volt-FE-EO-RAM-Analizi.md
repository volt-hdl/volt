> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Ferroelektrik + Elektro-Optik RAM — Kapsamlı Analiz

> Ferroelektrik polarizasyon + Pockels etkisi kombinasyonu, mevcut
> bellek teknolojilerinin çözemediği yıkıcı okuma sorununu yapısal
> olarak ortadan kaldıran ve dengeli üçlüyü doğal olarak temsil eden
> yeni bir bellek paradigması sunuyor. Bu belge mekanizmayı, avantajları,
> zorlukları ve Volt ekosistemiyle ilişkisini derinlemesine inceler.

---

## Bölüm 1 — Temel Fikir: Neden Bu Kombinasyon Özel

### 1.1 Mevcut FeRAM'ın Ölümcül Sorunu

```
Klasik Ferroelektrik RAM (FeRAM) nasıl okur:

  ┌──────────────────────────────────┐
  │   FE kapasitör                   │
  │   Polarizasyon ↑ (= "1")        │
  └──────────────┬───────────────────┘
                 │
                 ▼
  Okuma voltajı uygula (−V)
                 │
                 ▼
  Polarizasyon ZOR YÖNDE ters çevrilmeye zorlanır
  → Yük akışı oluşur → sense amplifier okur
                 │
                 ▼
  Durum YOK EDİLDİ: ↑ → ↓

  Zorunlu yeniden yazma (restore):
  → Oku, sonra hemen geri yaz
  → Her okuma = 1 yazma döngüsü tüketimi
  → Endurance yarıya düşer
  → Hız: okuma + yeniden yazma = 2× gecikme
```

Bu problem FeRAM'ın DRAM ile rekabetini engelleyen temel etken.

### 1.2 FE + EO Kombinasyonunun Çözümü

```
Ferroelektrik polarizasyon → iç elektrik alan (kalıcı)
İç elektrik alan → kırılma indisi değişimi (Pockels etkisi)
Kırılma indisi → optik faz kayması → ışıkla ölçülür

Okuma için ferroelektrik durumu değiştirmeye GEREK YOK.
Polarizasyon aynen duruyor, ışık sadece etkisini okuyor.

Fiziksel analoji:
  Klasik FeRAM: "Yönünü anlamak için mıknatısı döndür"
  FE + EO:      "Mıknatısın yarattığı manyetik alanı uzaktan hisset"
```

---

## Bölüm 2 — Fiziksel Mekanizma

### 2.1 Ferroelektrik Polarizasyon ve İç Alan

```
Ferroelektrik malzeme:
  Dipol momentler kendiliğinden hizalanır → spontan polarizasyon (P)

  ↑↑↑↑↑↑   Yukarı polarizasyon:  P_s = +30 μC/cm²
  ↓↓↓↓↓↓   Aşağı polarizasyon:   P_s = −30 μC/cm²

  Dış alan kaldırıldıktan sonra polarizasyon KALIR:
  → Gerçek "remanent polarizasyon" (P_r)
  → Bu, depolanan bilgi

İç elektrik alan:
  P_r → depolarizasyon alanı: E_d = −P_r / ε₀ε_r
  LiNbO₃ için: P_r ≈ 70 μC/cm², ε_r ≈ 28
  E_d ≈ 70×10⁻⁴ / (8.85×10⁻¹²× 28) ≈ 2.8 MV/cm

  Bu alan dışarıdan voltaj uygulamadan mevcut
  → Pockels etkisi için kaynak: ücretsiz
```

### 2.2 Pockels Etkisi (Doğrusal Elektro-Optik)

```
Pockels etkisi:
  Δn = −(n³/2) × r_eff × E

  n: kırılma indisi (LiNbO₃: n_e = 2.14, n_o = 2.21)
  r_eff: etkin Pockels katsayısı
  E: uygulanan elektrik alan

LiNbO₃ Pockels katsayıları:
  r₁₃ = 8.6 pm/V
  r₃₃ = 30.9 pm/V  ← en büyük
  r₂₂ = 3.4 pm/V

Ferroelektrik iç alanla Δn:
  E_d = 2.8 MV/cm = 2.8×10⁸ V/m
  Δn = −(n³/2) × r₃₃ × E_d
  Δn = −(2.14³/2) × 30.9×10⁻¹² × 2.8×10⁸
  Δn ≈ −0.0095  (yaklaşık %0.44 değişim)

Faz kayması:
  ΔΦ = (2π/λ) × Δn × L
  L = 100 μm dalga kılavuzu uzunluğu
  λ = 1550 nm
  ΔΦ ≈ (2π/1550nm) × 0.0095 × 100μm ≈ 3.84 radyan

  ~π radyan (180°): yani tam anahtarlama için ~82 μm yeterli
  → Küçük hücre boyutu mümkün
```

### 2.3 Ternary İçin Doğal Eşleme

```
Üç ferroelektrik durum → üç Pockels kayması:

  P = +P_r   →   E_d = +E₀   →   Δn = +Δn₀   →   ΔΦ = +π/2
  P ≈ 0      →   E_d ≈ 0     →   Δn ≈ 0       →   ΔΦ ≈ 0
  P = −P_r   →   E_d = −E₀   →   Δn = −Δn₀   →   ΔΦ = −π/2

  MZI interferometre ile okuma:
  ΔΦ = +π/2  →  çıkış yüksek  →  trit +1
  ΔΦ ≈ 0     →  çıkış orta    →  trit  0
  ΔΦ = −π/2  →  çıkış düşük  →  trit −1

Bu dengeli üçlünün ferroelektrik fizikle mükemmel örtüşmesi.
Üçüncü durum için kısmi kristalinleşme gibi hassas kontrol yok:
polarizasyon ya tamamen yukarı, ya tamamen aşağı, ya da nötr.
→ PCM ternary'nin en büyük sorunu (drift) burada yok.
```

---

## Bölüm 3 — LiNbO₃: Her İkisi Birden

### 3.1 LiNbO₃'ün Benzersizliği

```
Çoğu malzeme ya ferroelektrik YA DA elektro-optik:
  PZT: güçlü ferroelektrik, zayıf EO
  Si:  EO yok, ferroelektrik yok
  LiNbO₃:  HEM güçlü ferroelektrik HEM güçlü EO  ← benzersiz

Bu, tek malzemenin hem depolama hem okuma yapması demek:
  → Ek EO katman gerekmez
  → Ara yüz sorunları yok
  → Basit fabrikasyon

LiNbO₃ parametreleri:
  Curie sıcaklığı:    ~1150°C → oda sıcaklığında kararlı
  Remanent polzarasyon: 70 μC/cm²
  Koersitif alan:      ~2 kV/mm (ince filmde çok daha az)
  Pockels r₃₃:         30.9 pm/V  (en yüksek şeffaf malzeme)
  Optik kayıp:         ~0.1 dB/cm (çok iyi)
  Bant genişliği:      >100 GHz (EO modülatör olarak)
```

### 3.2 İnce Film LiNbO₃ (TFLN) — Oyun Değiştirici

```
Standart LiNbO₃'ün sorunları:
  ✗ Kalın (mm ölçeği) → büyük cihaz
  ✗ Si ile entegrasyon zor
  ✗ Yüksek anahtarlama voltajı (Vπ ~ 5V)
  ✗ CMOS süreciyle uyumsuz

TFLN (Thin Film LiNbO₃ on Insulator):
  300-600 nm LiNbO₃ → SiO₂ → Si (wafer-bonded)
  
  ┌─────────────────────────┐
  │   LiNbO₃ (300-600 nm)  │ ← aktif katman
  ├─────────────────────────┤
  │   SiO₂ (2 μm)          │ ← alt kaplama
  ├─────────────────────────┤
  │   Si wafer              │ ← substrat
  └─────────────────────────┘

TFLN avantajları:
  ✓ Vπ × L ≈ 2 V·cm (standart: 20 V·cm) → 10x iyileşme
  ✓ Dalga kılavuzu kaybı: 0.027 dB/cm (rekord)
  ✓ EO modülatör: 100+ GHz bant genişliği
  ✓ Si-fotonik ile monolitik entegrasyon mümkün
  ✓ CMOS BEOL uyumlu (400°C altında işlem)

TFLN ferroelektrik bellek:
  2022: University of Washington demosu
  → TFLN'de ferroelektrik anahtarlama + EO okuma
  → İlk "non-volatile fotonik bellek" gösterimi
  → Endurance: >10⁴ döngü gösterildi
```

### 3.3 HZO + LiNbO₃ Hibrit Yaklaşım

```
Alternatif mimari: her malzemeyi güçlü olduğu işe kullan

HZO (Hafnium Zirconium Oxide) — yazma için:
  ✓ CMOS süreciyle tam uyumlu (Intel/Samsung üretiyor)
  ✓ Düşük anahtarlama voltajı: 1-3V
  ✓ Yüksek endurance: ~10⁸-10¹⁰ döngü
  ✗ EO etkisi zayıf

LiNbO₃ — okuma için:
  ✓ Güçlü EO (r₃₃ = 30.9 pm/V)
  ✓ Düşük optik kayıp
  ✗ Anahtarlama voltajı yüksek

Hibrit hücre:
  ┌─────────────────────────────┐
  │  Metal elektrot             │
  ├─────────────────────────────┤
  │  HZO (ferroelektrik, 10nm) │ ← depolama
  ├─────────────────────────────┤
  │  Ara yüz katmanı            │
  ├─────────────────────────────┤
  │  LiNbO₃ (EO, 300nm)       │ ← okuma
  ├─────────────────────────────┤
  │  Si dalga kılavuzu          │
  └─────────────────────────────┘

  HZO polarizasyon → iç alan → LiNbO₃ Δn → optik faz
  Yazma: HZO düşük voltajda (1-3V) → CMOS uyumlu
  Okuma: LiNbO₃ EO → çok hızlı ve non-destructive
```

---

## Bölüm 4 — RAM Olarak Kullanım Analizi

### 4.1 RAM Gereksinimleri vs FE+EO Özellikleri

```
Gereksinim          DRAM         SRAM         FE+EO RAM
──────────────────────────────────────────────────────────
Rastgele erişim     ✓            ✓            ✓
Okuma hızı         ~10-50 ns    ~1-5 ns      <1 ns (optik!) ✓
Yazma hızı         ~10-50 ns    ~1-5 ns      ~1-10 ns       ✓
Geçici mi?         Geçici       Geçici       KALICI         ✓✓
Yenileme gerekli?  Her 64ms     Yok          YOK            ✓✓
Yıkıcı okuma?      Hayır        Hayır        HAYIR (optik)  ✓✓
Bekleme gücü       ~mW/GB       ~mW/MB       ~0             ✓✓
Okuma enerjisi     ~pJ          ~fJ          ~0.01 fJ       ✓✓
Yazma enerjisi     ~pJ          ~fJ          ~fJ-pJ         ✓
Yoğunluk          Çok yüksek   Orta         Düşük          ✗
Endurance          Sınırsız     Sınırsız     ~10⁸-10¹⁰     ⚠️
Üretim olgunluğu   Tam olgun    Tam olgun    Araştırma      ✗
```

### 4.2 "Kalıcı RAM" Kavramı — Devrimsel Özellik

```
DRAM'ın en büyük gizli maliyeti: güç kaybı = veri kaybı

  Bilgisayar kapanır:
  → DRAM: tüm veri kaybolur
  → FE+EO RAM: polarizasyon kalır, veri kalır

  Bilgisayar açılır:
  → DRAM: işletim sistemi yükle, uygulamaları başlat (~dakikalar)
  → FE+EO RAM: tam olarak kaldığı yerden devam (~sıfır gecikme)

  "Anlık açılma" — her güç kapandığında RAM içeriği korunur

Enerji açısından:
  Veri merkezi DRAM yenileme:
  1TB DRAM × ~0.1W/GB = 100W sürekli yenileme gücü
  → Sadece veriyi korumak için!

  FE+EO RAM:
  Bekleme gücü = 0W (polarizasyon spontan)
  → 100W tasarruf, sürekli
```

### 4.3 Okuma Bant Genişliği: WDM Avantajı

```
Elektronik RAM bant genişliği:
  HBM3: ~900 GB/s → 128 veri pimi × 9.8 Gbps
  Sınır: pim sayısı × pin frekansı

FE+EO RAM bant genişliği:
  WDM (Dalga Boyu Çoğullama):
  λ₁, λ₂, ..., λ₆₄ → 64 kanal × tek dalga kılavuzu

  Her kanal 100 GHz modülasyon:
  64 kanal × 100 Gbps = 6.4 Tbps / dalga kılavuzu

  vs HBM3: 900 GB/s = 7.2 Tbps

  Benzer bant genişliği ama:
  → Tek fiber/dalga kılavuzu (çok az fiziksel kanal)
  → Düşük gecikme (ışık hızı)
  → Düşük enerji (foton taşıma ucuz)

Gerçekçi kısa vadeli hedef:
  16 WDM kanal × 25 GHz = 400 Gbps / mm²
  → HBM3'ün %50'si, ama çok daha az güç
```

---

## Bölüm 5 — Zorluklar ve Engeller

### 5.1 Anahtarlama Voltajı — Temel Gerilim

```
DRAM anahtarlama:         ~0.6V (CMOS standart)
HZO FeFET:               ~1-3V (kabul edilebilir)
LiNbO₃ standart:         ~200-500V (yok!)
LiNbO₃ TFLN:             ~5-20V (hâlâ yüksek)
LiNbO₃ TFLN (araştırma): ~2-5V (CMOS sınırında)

Sorun:
  Modern CMOS: 1.8V ve altı çalışır
  LiNbO₃ TFLN: 5V+ yazma voltajı
  → Özel yüksek voltajlı sürücü devresi şart
  → Bu ek alan ve karmaşıklık demek

Çözüm yolları:
  a) HZO ile yaz (1-3V), LiNbO₃ ile oku:
     → Voltaj sorunu HZO'da çözülmüş
     → LiNbO₃ sadece EO okuma (düşük voltaj)

  b) Kapasitif kuplaj + LiNbO₃:
     Küçük yüksek-κ dielektrik → alan artırma
     → Düşük voltaj → yüksek yerel alan → LiNbO₃ anahtarlar

  c) Yeni malzeme arayışı:
     BaTiO₃: CMOS uyumlu, Vπ < 1V (araştırma)
     PMN-PT: piezo+EO, düşük voltaj (Si entegrasyonu zor)
```

### 5.2 Yoğunluk Sınırı — Fiziksel Engel

```
Elektronik DRAM hücresi:
  1T1C: 1 transistör + 1 kapasitör
  Boyut: ~6F² (F = 10nm → hücre: 600 nm²)
  Yoğunluk: ~16 Gbit/cm²

FE+EO RAM hücresi minimumu:
  Dalga kılavuzu genişliği: ~400 nm
  Dalga kılavuzu adımı (pitch): ~800 nm
  Hücre boyutu: ~800 nm × 800 nm = 640,000 nm²
  Yoğunluk: ~1.5 Gbit/cm²

Oran: DRAM'ın ~10x daha az yoğun

Bu giderilebilir mi?
  Kısmen: WDM ile aynı waveguide'da çok bit
    16 WDM kanal × 1.5 Gbit/cm² = 24 Gbit/cm²
    → DRAM'a yaklaşıyor

  Ama asla aşılamayacak:
  Difraksiyon limiti (λ/2n) aşılamaz
  → Maksimum yoğunluk DRAM'ın altında kalır
```

### 5.3 Endurance — Ferroelektrik Yorgunluğu

```
Ferroelektrik yorgunluk mekanizması:
  Her polarizasyon geçişi: küçük dislokasyon
  Biriken hasar: Pr azalır, Ec artar
  Son durum: "takılı kalır" → yazılamaz

  LiNbO₃ TFLN endurance: ~10⁴-10⁵ döngü (zayıf)
  HZO endurance:          ~10⁸-10¹⁰ döngü (iyi)
  DRAM endurance:         pratik olarak sınırsız

  "Yoğun yazma" RAM uygulaması için zorluk:
  Örnek: işletim sistemi belleği sürekli yazılır
  10 ns × 10¹⁰ = 100 saniye → 100 saniyede hücre ölür!

Çözümler:
  a) Wear leveling: her zaman aynı hücreye yazma
  b) Tampon: sık yazılan veri SRAM'da, seyrek yazılan FE+EO
  c) Write-back: veriyi gruplayıp daha az yazma
  d) Yeni malzeme: FE+EO için 10¹² döngü → araştırma

Gerçekçi uygulama profili:
  Az yazılan ama çok okunan: AI ağırlıkları → mükemmel
  Sık yazılan çalışma belleği: SRAM ile hibrit
```

### 5.4 Ternary Kısmi Polarizasyon Kararlılığı

```
Trit 0 (nötr polarizasyon) nasıl elde edilir?

Seçenek 1 — Kısmi anahtarlama:
  +P_r'dan başla → belirli süre negatif alan uygula → P ≈ 0
  Sorun: ne kadar süre? Malzeme ve sıcaklığa bağlı
  → PCM'in kısmi kristalleşme sorununa benzer ama DAHA AZ ciddi
  → Çünkü polarizasyon drift yavaş, kristalleşme kadar hızlı değil

Seçenek 2 — Çok domainli yapı:
  Hücrenin yarısı yukarı, yarısı aşağı domain:
  Toplam: net polarizasyon ≈ 0
  Ortalama iç alan ≈ 0 → Δn ≈ 0 → trit 0
  
  Bu kararlı mı? Evet, eğer:
  → Hücre tasarımı domain sınırını sabitlerstabilize ederse
  → Domain duvarı enerji bariyeri yeterince büyük olursa
  → Tipik olarak: yüzlerce nm → düzinelerce yıl kararlı

Seçenek 3 — Üçüncü malzeme durumu (araştırma):
  Bazı ferroelektrikler üç kararlı polarizasyon durumu gösterir
  YMnO₃: +, 0, − → doğal ternary
  Ama CMOS entegrasyonu zor
```

### 5.5 Termal Kararlılık

```
Ferroelektrik malzemenin Curie sıcaklığı:
  LiNbO₃: ~1150°C → oda sıcaklığında çok kararlı ✓
  HZO:     ~350°C → 85°C datacenter'da kararlı ✓
  BaTiO₃:  ~130°C → bu düşük! 85°C yakın → sorun ✗

  PCM retansiyon sorunu burada yok:
  → PCM amorf durum: kristalleşmeye termodinamik eğilim
  → FE polarizasyon: enerji bariyeriyle korunur, üstesinden gelmek için çok fazla kT gerekir
  → 85°C'de retansiyon sorunu yok (HZO ve LiNbO₃ için)

Sonuç: Datacenter sıcaklık sorunu FE+EO'da yok
→ PCM fotonik belleğin en büyük dezavantajı burada ortadan kalkar
```

---

## Bölüm 6 — Hangi RAM Türüne Karşılık Geliyor

### 6.1 Bellek Hiyerarşisindeki Yeri

```
Geleneksel hiyerarşi:
  L1 cache (SRAM, 64KB, ~1 ns)
  L2 cache (SRAM, 1MB, ~5 ns)
  L3 cache (SRAM, 32MB, ~20 ns)
  DRAM (16GB, ~50 ns)
  NVMe SSD (1TB, ~100 μs)
  HDD (∞, ~ms)

FE+EO RAM nerede oturur?

  L1-L3 cache: hayır (SRAM daha hızlı, daha küçük)
  DRAM yerine: kısmen (daha az yoğun ama non-volatile)
  Yeni katman: "Storage-Class Memory" (SCM)
    → DRAM ve SSD arasında
    → Intel Optane'in hedeflediği yer
    → Ama FE+EO çok daha hızlı ve non-volatile

FE+EO RAM'ın ideal konumu:
  "Non-volatile near-memory cache"
  → AI ağırlıkları için kalıcı, hızlı, düşük enerjili depolama
  → "Bir kez yaz, milyar kez oku" profili
```

### 6.2 AI Çıkarım Belleği — Mükemmel Eşleşme

```
LLM çıkarım belleği profili:
  → Ağırlıklar: modeli yüklerken bir kez yazılır
  → Çıkarım: sonsuz kez okunur
  → Güncelleme: nadir (fine-tuning)
  → Bant genişliği: kritik darboğaz
  → Enerji: kritik kısıt (özellikle edge'de)

FE+EO RAM bu profile mükemmel uyar:
  ✓ Bir kez yaz → yüksek endurance yük yok
  ✓ Sonsuz non-destructive optik okuma
  ✓ WDM ile yüksek bant genişliği
  ✓ Bekleme gücü sıfır (non-volatile)
  ✓ Ternary depolama → 16x daha kompakt
  ✓ In-situ hesaplama: ağırlık × ışık = matris çarpım sonucu

70B model, ternary, FE+EO RAM:
  Konvansiyonel: 140 GB FP16 DRAM
  Ternary FE+EO: ~8.75 GB → 16x küçük
  Bekleme gücü: sıfır vs ~5W (DRAM yenileme)
  Okuma enerji: ~0.01 fJ vs ~pJ → 100x daha verimli
```

---

## Bölüm 7 — Hesaplama-İçi-Bellek (CIM) Boyutu

FE+EO RAM'ın en devrimsel potansiyeli bellek olarak değil,
**hesaplama yapan bellek** olarak:

### 7.1 Matris-Vektör Çarpımı Doğrudan Bellekte

```
Standart yaklaşım:
  Ağırlık (bellek) → veri yolu → çarpan (işlemci) → sonuç
  Her çıkarımda tüm model taşınır → bant genişliği sorunu

FE+EO CIM yaklaşımı:
  Ağırlık = FE polarizasyon (bellekte)
  Giriş = optik sinyal (dalga kılavuzuna enjekte)
  Çıkış = EO modülasyon sonucu (bellekten doğrudan)

  Fizik:
  E_out ∝ E_in × exp(i × Δn × k × L)
  Δn ∝ FE polarizasyon (ağırlık)
  
  → E_in × W doğrudan bellekte fizik yasasıyla hesaplanır
  → Veri hareketi yok
  → Von Neumann engeli tamamen aşıldı

Enerji kazancı:
  Konvansiyonel: veri taşıma ~pJ + hesaplama ~pJ = ~2 pJ/MAC
  FE+EO CIM:     sadece ışık enjeksiyonu ~0.01 pJ/MAC
  → 200x enerji tasarrufu
```

### 7.2 Sistolik Dizi Analog: Fotonik Ağırlık Matrisi

```
Sistolik dizi (Google TPU tarzı):
  Her PE: çarp + topla + ilet
  Veri dalga içinde akar

FE+EO fotonik analog:
  ┌────────────────────────────────────────┐
  │  W₁₁ W₁₂ W₁₃ ... W₁ₙ               │
  │   │    │    │         │               │
  x₁ ─┤────┤────┤─────────┤─ ΣW₁ᵢxᵢ     │
  x₂ ─┤────┤────┤─────────┤─ ΣW₂ᵢxᵢ     │
  x₃ ─┤────┤────┤─────────┤─ ΣW₃ᵢxᵢ     │
  ...                                     │
  │  Herbiri FE+EO hücresi               │
  │  Toplama: dalga süperpozisyonu        │
  └────────────────────────────────────────┘

  Matris çarpımı fizik yasasıyla anlık
  Enerji: yalnızca ışık enjeksiyonu
  Hız: ışık hızı × dalga kılavuzu uzunluğu
```

---

## Bölüm 8 — Volt Tip Sistemi

### 8.1 FE+EO RAM Tipleri

```volt
// Ferroelektrik domain tipi
domain Ferroelectric {
    storage   = PolarizationState
    switching = ElectricField
    retention = Permanent           // non-volatile!
    mechanism = DomainWallMotion
}

// Ferroelektrik durum — ternary doğal
enum FEState {
    Up,      // +P_r → trit +1
    Neutral, // P ≈ 0 → trit 0
    Down     // -P_r → trit -1
}

// FE+EO bellek hücresi
module FEOCell {
    // Yazma: elektriksel (HZO veya LiNbO₃)
    in  write_v : Voltage      @CMOS
    // Okuma: optik
    in  probe   : OpticalSignal @Photonic
    out readout : OpticalSignal @Photonic

    // Non-volatile durum — güç kesilince KAYBOLMAZ
    reg fe_state : FEState = FEState::Neutral
    // 'reg' burada farklı: saat yok, güç yok, hâlâ duruyor

    // Yazma: koersitif alanı aşınca geçiş
    on write_v {
        match write_v {
            v if v > +Ec => fe_state <= FEState::Up
            v if v < -Ec => fe_state <= FEState::Down
            _            => {}    // Ec altında: durum değişmez
        }
    }

    // Okuma: EO etkisi, non-destructive
    let delta_n = fe_state_to_delta_n(fe_state)
    readout = probe.pockels_shift(delta_n, length=100.um)

    // KRİTİK GARANTİ: okuma yazma ile eşdeğer değil
    #[non_destructive_read]
    invariant: read_operation does_not_modify fe_state

    // Retansiyon: güç kesilse de durum korunur
    #[power_independent_retention]
    @retention(duration=Forever)   // gerçekten kalıcı

    // Endurance
    @endurance(max_writes=1_000_000_000)   // HZO ile 10^9
}

// FE+EO RAM dizisi — WDM ile yüksek bant genişliği
module FEOMemory<const Rows: usize, const Cols: usize> {
    in  addr    : (bits<log2(Rows)>, bits<log2(Cols)>)
    in  data_w  : Trit           @CMOS
    in  wen     : bool
    out data_r  : Trit           @Photonic

    // WDM: her satır farklı dalga boyunda
    let wavelengths: [nm; Rows] = wdm_grid(1530, 1565, Rows)

    // Paralel okuma: tüm satırlar aynı anda okunabilir
    // Her dalga boyu kendi FE durumunu taşıyor
    #[wdm_parallel_read]
    @bandwidth(Rows * 100.GHz)     // WDM toplam bant genişliği

    // Yazma: tek hücre seçici
    // Okuma: WDM ile tüm satırlar paralel
    invariant: read_bandwidth > write_bandwidth * 1000
    // Oku → yaz asimetrisi: AI inference için ideal
}
```

### 8.2 Mimari OS Entegrasyonu

```volt
// Mimari OS FE+EO RAM'ı non-volatile kaynak olarak tanır
hardware_descriptor FEOMemory_0 {
    paradigm     = NonVolatilePhotonicRAM

    // Non-volatile → yenileme yok → bekleme gücü sıfır
    @standby_power(0.W)
    @refresh_required(false)

    // Okuma özellikleri
    read_latency   = 1.ns           // optik okuma hızı
    read_energy    = 0.01.fJ        // foton ölçüm
    read_bandwidth = 6.4.Tbps       // 64 WDM × 100 GHz
    #[non_destructive_read]         // endurance tüketmez

    // Yazma özellikleri
    write_latency  = 5.ns           // FE anahtarlama
    write_energy   = 1.fJ           // HZO yazma
    write_endurance = 1_000_000_000 // 10^9 döngü

    // Ternary depolama
    bits_per_cell  = log2(3)        // ~1.58 bit/hücre
    effective_density = baseline * 1.58  // binary'den iyi

    // AI profil optimizasyonu
    @optimized_for(WriteOnce_ReadMany)
    constraint: write_frequency < 1.Hz per_cell // ağırlık deposu
}
```

---

## Bölüm 9 — Güncel Araştırma ve Zaman Çizelgesi

### 9.1 Önemli Araştırma Sonuçları

```
2018 — Columbia University:
  TFLN elektro-optik modülatör: 100 GHz
  → EO altyapısı olgunlaşıyor

2020 — Harvard/MIT:
  TFLN dalga kılavuzu kaybı: 0.027 dB/cm (rekord)
  → Uzun dalga kılavuzları artık mümkün

2021 — ETH Zürich:
  HZO ferroelektrik + Si fotonik entegrasyon demo
  → Yazma: elektriksel, okuma: optik
  → İlk hibrit gösterim

2022 — University of Washington:
  TFLN non-volatile fotonik bellek
  → LiNbO₃ ferroelektrik anahtarlama + EO okuma
  → Endurance: 10⁴ döngü
  → "İlk gerçek FE+EO RAM" demonstrasyonu

2023 — EPFL + AIM Photonics:
  Ticari TFLN süreç platformu olgunlaşıyor
  → Foundry erişimi (Global Foundries, imec)
  → Özel AR-GE'den üretime geçiş başlıyor

2024 itibarıyla açık sorular:
  ✗ Endurance: 10⁴ → 10⁸ yolculuğu
  ✗ Ternary nötr durum kararlılığı: gösterilmedi
  ✗ 1 cm² ötesi yoğunluk
  ✗ Ticari ürün: henüz yok
```

### 9.2 Zaman Çizelgesi

```
2024-2026: Temel araştırma
  Endurance iyileştirmesi: 10⁴ → 10⁶
  Ternary demo: kısmi anahtarlama veya çok-domain
  TFLN foundry erişimi genişliyor

2026-2029: Sistem entegrasyonu
  HZO+TFLN hibrit hücre: düşük voltaj + güçlü EO
  CIM (hesaplama-içi-bellek) demo
  AI inference chip prototipi

2029-2032: Ürün aşaması
  Edge AI belleği: "yaz ve unut" profili
  Non-volatile AI accelerator
  Ticari TFLN foundry süreci

2032+: Olgunluk
  Storage-class memory kategorisi
  DRAM ile hibrit sistem
  FE+EO ternary: standart AI donanım
```

---

## Özet

```
"Ferroelektrik + EO kombinasyonu RAM olabilir mi?"
Cevap: Evet — ama "farklı bir RAM"

Geleneksel RAM'dan farkları:
  ✓ Non-volatile: güç kesilince veri kaybolmaz
  ✓ Non-destructive read: okuma döngü tüketmez
  ✓ Bekleme gücü sıfır (yenileme yok)
  ✓ Ternary doğal: dengeli üçlü = ferroelektrik durumlar
  ✓ CIM: bellek hesaplama yapıyor (von Neumann aşılıyor)
  ✗ Daha az yoğun (difraksiyon sınırı)
  ✗ Endurance sınırlı (sık yazma için)
  ✗ Anahtarlama voltajı yüksek (çözülüyor)

En uygun kullanım:
  AI ağırlık deposu — bir kez yaz, milyar kez oku
  Edge AI — pil ömrü kritik, bekleme gücü sıfır
  Compute-in-memory — ağırlık × ışık = hesaplama

Bu neden önemli:
  LLM'lerin bellek bant genişliği sorunu
  + Von Neumann veri taşıma maliyeti
  iki sorunun tek çözümü: FE+EO CIM
  Veri taşınmaz, hesaplama bellekte olur,
  sonuç ışıkta taşınır.
```
