> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# 400B Model — Taşınabilir SoC'ta Çalışmamasının Engelleri

> Bu soruyu doğru yanıtlamak için önce neden 7B modelin çalıştığını,
> sonra 400B'nin neden çalışmadığını katman katman görmek gerekiyor.
> Engeller birbirinden bağımsız değil — birbiriyle etkileşen ve
> birbirini ağırlaştıran bir sistem. Her birini tek tek çözmek
> yetmiyor; hepsinin aynı anda çözülmesi gerekiyor.

---

## Bölüm 1 — Neden 7B Çalışıyor Ama 400B Çalışmıyor

```
Apple M4 Ultra (bugün):
  Birleşik bellek: 192 GB
  Bellek bant genişliği: 800 GB/s
  Güç: ~60-100W
  
  7B model (INT4): 3.5 GB → sığıyor ✓
  70B model (INT4): 35 GB  → sığıyor ✓, yavaş
  400B model (INT4): 200 GB → SIĞMIYOR ✗

Sorun boyutsal değil, çok boyutlu:
  Bellek kapasitesi: 200 GB > 192 GB → sığmıyor
  Bellek bant genişliği: 200 GB × 30 tok/s = 6,000 GB/s gerekli
                         M4 Ultra: 800 GB/s → 7.5× yetersiz
  
  Yani kapasite çözülse bile bant genişliği engeli kalır.
  Bant genişliği çözülse bile kapasite engeli kalır.
  İkisi birlikte çözülmeli.
```

---

## Bölüm 2 — Beş Ana Engel (Şiddet Sırasıyla)

```
ENGEL 1 — Bellek Bant Genişliği (EN KRİTİK)
ENGEL 2 — Bellek Kapasitesi
ENGEL 3 — Güç Tüketimi
ENGEL 4 — Termal Yönetim
ENGEL 5 — KV Cache (Bağlam Belleği)

Bunlara ek: ENGEL 6 — Hesaplama Gücü (görece küçük sorun)
```

---

## Bölüm 3 — Engel 1: Bellek Bant Genişliği (En Kritik)

### Neden Bant Genişliği Bu Kadar Önemli

```
LLM çıkarımının bottleneck denklemi:

Gerekli bant genişliği = Model boyutu × Token hızı

  FP16 (800 GB) × 30 tok/s = 24,000 GB/s  ← imkânsız
  INT4 (200 GB) × 30 tok/s =  6,000 GB/s  ← imkânsız
  Ternary (100 GB) × 30    =  3,000 GB/s  ← imkânsız
  ANS Ternary (75 GB) × 30 =  2,250 GB/s  ← imkânsız

  Mevcut teknolojiler:
  LPDDR5X (en iyi mobil): 136 GB/s → 75 GB × 1.8 tok/s
  HBM3 (en iyi sunucu):   900 GB/s → 75 GB × 12 tok/s
  
  Konuşma hızı için gereken: ~30 tok/s
  HBM3 bile: 12 tok/s → yavaş ama kabul edilebilir
  Ama HBM3 taşınabilir SoC'a sığmaz (boyut + güç)
```

### Neden Bu Engel Başka Engelleri Bağımsız Kılıyor

```
Bant genişliği = model boyutu × hız olduğundan:
  Modeli küçült → bant genişliği sorunu azalır
  Hızı düşür   → bant genişliği sorunu azalır

  Token hızı 1'e düşürülürse:
  ANS Ternary: 75 GB × 1 tok/s = 75 GB/s
  LPDDR5X: 136 GB/s → yeterli!
  
  Ama 1 tok/s: bir cümle ~10 kelime → 10 saniye bekle
  → Kullanılabilir değil (80s/saniye konuşma hızının altında)

  "Gerçek zamanlı" için en az 10-20 tok/s
  75 GB × 15 tok/s = 1,125 GB/s → hâlâ LPDDR5X'in 8× üstü
```

### FE+EO CIM Bu Engeli Nasıl Çözüyor

```
Geleneksel: Ağırlık bellekte → veri yolu → işlemci → hesapla
FE+EO CIM:  Ağırlık bellekte → ışık geçer → hesap orada olur

  "Hesap için veri taşıma" yok
  → Bant genişliği darboğazı kavramsal olarak ortadan kalkar

  WDM 512 kanal × 100 GHz = 51.2 Tbps (iç, bellek içinde)
  Bu bant genişliği sorununun çözümü değil, sorunun yokedilmesi

Ama FE+EO CIM 2032+ gerçekçi → bugün çözüm değil
```

---

## Bölüm 4 — Engel 2: Bellek Kapasitesi

### Sayılar

```
400B parametreli model depolama ihtiyacı:

Kesinlik          Boyut        Durum
──────────────────────────────────────────────────────
FP16              800 GB       İmkânsız (hiçbir SoC)
INT8              400 GB       İmkânsız
INT4              200 GB       İmkânsız
Ternary (2 bit)   100 GB       Zor (1-2 wafer HBM3)
ANS Ternary       60-75 GB     Zor (büyük HBM yığını)
İkili (1 bit)      50 GB       Mümkün ama doğruluk?
```

### Bugünkü SoC Bellek Gerçekliği

```
Apple M4 Ultra:       192 GB birleşik bellek (en iyisi)
Qualcomm 8 Gen 3:      16 GB LPDDR5X
NVIDIA Jetson Orin:    64 GB
Intel Gaudi 3:        128 GB HBM2e per chip

400B model için gereken minimum (ANS ternary):
  Model ağırlıkları: ~75 GB
  KV cache (4K bağlam): ~12 GB
  Aktivasyonlar: ~2 GB
  ─────────────────────
  Toplam: ~89 GB

  M4 Ultra: 192 GB → sığar! (bellek kapasitesi)
  Ama bant genişliği: 800 GB/s < 1,125 GB/s gerekli
  
→ Kapasite sorunu M4 Ultra ile çözülüyor
→ Bant genişliği sorunu hâlâ devam ediyor
```

### Bellek Hiyerarşisi Çözümü

```
"Katmanlı bellek" yaklaşımı:
  Sık kullanılan ağırlıklar → SRAM (hızlı, küçük)
  Az kullanılan ağırlıklar → DRAM (yavaş, büyük)
  Çok az kullanılanlar → Flash (çok yavaş, çok büyük)

  Transformer'da dikkat katmanları → sık erişim
  FFN ağırlıkları → daha az erişim

  Spekülatif önbellekleme:
  "Sonraki hangi ağırlığa erişeceğiz?" tahmin et
  → Önceden yükle → gecikme gizle

  Apple bu stratejiyi unified memory ile yapıyor
  400B için aynı prensip ama çok daha büyük ölçek
```

---

## Bölüm 5 — Engel 3: Güç Tüketimi

### Mevcut Durumun Gerçeği

```
400B model çalıştırmak için bugün gereken güç:

  NVIDIA A100 × 8 kart: 8 × 400W = 3,200W
  (bu kadar kartla 400B model BM çalışır)
  
  Optimize edilmiş sistemler:
  Apple M4 Ultra × 2: 2 × 100W = 200W (70B için yeterli)
  400B için: kabaca 6-8× daha fazla → 600-800W

  Thunderbolt 4 maksimum güç beslemesi: 100W
  USB4 (PD3.1) maksimum: 240W (nadir)
  
  400B model için gereken: 200W+
  Thunderbolt'tan alınabilecek: 100W
  
  AÇIK: en az 100W ek güç kaynağı şart
```

### Enerji Tüketiminin Kaynakları

```
Bir token üretmek için enerji harcayan noktalar:

  1. Bellek okuma (ağırlık erişimi):
     400B × INT4 × her token = 200 GB okunuyor
     DRAM enerji: ~10 pJ/bit = 200 GB × 8 × 10 pJ ≈ 16 kJ/token
     → Bu başlı başına SoC'u yakar!
     
     Gerçekte daha az (cache hit var, enerji daha az)
     Ama büyük ölçek kabaca doğru

  2. Matris çarpımı:
     400B × INT4: 400B × 0.5 pJ = 200 J/token (çok fazla!)
     
  3. Aktivasyon belleği, softmax, vb.: küçük

Ternary + FE+EO CIM ile:
  Bellek okuma: FE+EO → ışık geçer → 0 pJ (!)
  Matris çarpımı: Pockels etkisi → 0.01 pJ/MAC
  400B × 0.01 pJ = 4 J/token (GPU'nun 50×'i daha az)
  
  30 tok/s → 4 J/s = 4W ← taşınabilir SoC için gerçekçi!
  (ama FE+EO CIM 2032+ gerçekçi)
```

### Güç-Doğruluk-Hız Üçlüsü

```
Seçim yapılmalı — üçünü birden optimize edemezsin:

  Yüksek doğruluk → büyük model → çok güç
  Yüksek hız     → çok TOPS → çok güç
  Düşük güç      → küçük model veya yavaş → düşük doğruluk/hız

  "İyi" tanımı uygulamaya göre değişir:
  Kodlama asistanı: doğruluk > hız > güç
  Ses tanıma:       hız > doğruluk > güç
  Wearable izleme:  güç > doğruluk > hız
```

---

## Bölüm 6 — Engel 4: Termal Yönetim

### Fizik Sınırları

```
Pasif soğutma kapasitesi (kural):
  Radyasyon + iletim + konveksiyon (natural air):
  Küçük cihaz (~100 cm²): 5-10W pasif
  Orta cihaz (~400 cm²): 15-25W pasif
  Büyük pasif soğutucu: 30-40W pasif (sınır)
  
  Fan eklenirse: 50-150W aktif soğutma
  Sıvı soğutma: 200-500W (ama taşınabilir değil)

  Taşınabilir SoC hedefi:
  Thunderbolt kutusu boyutu: ~200 cm²
  Pasif soğutma kapasitesi: ~15-20W
  
  400B bugünkü teknoloji: ~200W+ → 10× fazla!
  400B FE+EO CIM (gelecek): ~5-15W → pasif mümkün!
```

### Termal Throttling Sorunu

```
SoC ısınınca ne olur:
  Çip 80°C üstü → işlemciyi yavaşlat (throttling)
  → Token hızı düşer (30 tok/s → 5 tok/s)
  → Kullanım deneyimi kötüleşir

  Güvenilir performans için:
  Çip < 70°C tutulmalı
  Bu da güç bütçesini daha da kısıtlar

400B için termal senaryo (bugün):
  200W güç → ~170°C chip sıcaklığı (çip yanar!)
  Gerçekte: throttle açılır → 20W'a iner → 2 tok/s
  → Kullanılamaz
```

---

## Bölüm 7 — Engel 5: KV Cache (Bağlam Belleği)

### KV Cache Nedir ve Neden Büyür

```
Transformer'da dikkat mekanizması:
  Her yeni token üretmek için:
  Önceki TÜM tokenlerin K (anahtar) ve V (değer) vektörlerine bak

  Bu vektörler "KV cache" olarak saklanır:
  Yeniden hesaplamak yerine hafızada tut → hız

  400B model KV cache boyutu (tahmin):
  Model özellikleri: ~128 katman, 128 kafa, kafa_boyut=128
  
  1 token KV:
  2 × 128 katman × 128 kafa × 128 boyut × 2 byte (FP16)
  = 8 MB per token

  Bağlam uzunluğuna göre:
  1K token:   8 MB   (önemsiz)
  4K token:  32 GB   (büyük ama yönetilebilir)
  32K token: 256 GB  (SIĞMIYOR)
  128K token:  1 TB  (kesinlikle SIĞMIYOR)
```

### KV Cache Özeldir: Sıkıştırılamaz

```
Model ağırlıkları: bir kez yükle, hep kullan
  → Ternary, ANS, FE+EO ile sıkıştırılabilir
  → Zamanla değişmez

KV cache: her token için YENİ veriler
  → Sürekli değişiyor
  → Sıkıştırmak mümkün ama hassasiyeti bozar
  → FE+EO ile saklayamazsın (sürekli yazma → endurance biter)

KV cache çözümleri:
  
  1. Kayan pencere (Mistral Mixture of Experts):
     Yalnızca son N tokeni tut, eskisini at
     4K pencere: 32 GB → sabit boyut ✓
     Dezavantaj: uzun bağlamı unutur
  
  2. KV cache kuantizasyonu:
     FP16 → INT4: 4× küçülme
     32K bağlam: 256 GB → 64 GB → hâlâ büyük
  
  3. Alternatif mimari (Mamba / SSM):
     Transformer değil, durum uzayı modeli
     Bağlamı O(1) bellekte sıkıştırır!
     Sonsuz bağlam → sabit bellek
     Dezavantaj: Transformer kadar iyi değil (henüz)
  
  4. Hibrit: Transformer + SSM
     Kısa bağlam: Transformer (hassas)
     Uzun bağlam: SSM (verimli)
     Araştırma aşamasında
```

---

## Bölüm 8 — Engel 6: Hesaplama Gücü (Küçük Sorun)

```
400B model, 30 tok/s için gereken TOPS:
  Her token: ~800B MAC işlemi (2 × parametre)
  30 tok/s: 800B × 30 = 24 TOPS

  Mevcut SoC TOPS değerleri:
  Apple M4 Ultra NPU: 38 TOPS
  Qualcomm 8 Gen 3:   75 TOPS
  Samsung Exynos 2500: 34 TOPS
  NVIDIA Orin:        275 TOPS

  → Hesaplama gücü YETERLI! 24 TOPS < 38 TOPS
  
Hesaplama SoC için sorun değil.
Sorun hesaplamayı beslemek için yeterli veriyi getirememek.
(Şef iyi ama hammadde yolda tıkanıyor.)
```

---

## Bölüm 9 — Engellerin Etkileşimi

Engeller birbirinden bağımsız değil — birbiriyle etkileşiyor:

```
                    BANT GENİŞLİĞİ
                         │
              ┌──────────┼──────────┐
              │          │          │
           Model      Token      Bant gen.
           boyutu     hızı       donanımı
              │          │          │
              ▼          ▼          ▼
         KAPASITE    TERMAl     ARALIK
              │          │
         Sıkıştır    Güç düşür
              │          │
         Doğruluk   Hız düşür
            kaybı        │
                    Kullanışsız

"Bant genişliğini artır" dersen:
  → Daha geniş bellek veri yolu → daha büyük güç
  → Güç artar → termal sorun
  → Termal çözümü → büyük soğutucu → taşınabilir değil

"Modeli küçült" dersen:
  → Daha az kapasite gerekli → bant genişliği azalır
  → Ama doğruluk düşer
  → 400B'den 7B'ye: %10-15 doğruluk kaybı

"Hızı düşür" dersen:
  → Bant genişliği sorunu küçülür
  → Ama 1 tok/s = kullanılamaz deneyim

Her "çözüm" başka sorun yaratıyor.
FE+EO CIM bu döngüyü kıran tek yaklaşım.
```

---

## Bölüm 10 — FE+EO CIM Hangi Engelleri Çözüyor

```
Engel                  Bugün         FE+EO CIM (2032+)
───────────────────────────────────────────────────────────
Bant genişliği         HAYIR         ÇÖZÜLDÜ (taşıma yok)
Bellek kapasitesi      Kısmen        ÇÖZÜLDÜ (100 GB ternary)
Güç tüketimi           HAYIR         BÜYÜK ÖLÇÜDE (4W!)
Termal yönetim         HAYIR         ÇÖZÜLDÜ (4W pasif)
KV cache               HAYIR         HÂLÂ SORUN
Hesaplama gücü         Zaten yeterli  Daha da iyi

FE+EO CIM çözmediği:
  KV cache → mimari değişikliği gerekiyor (SSM/hibrit)
  Model doğruluğu → ternary doğruluk kaybı (küçük ama var)
  Üretim olgunluğu → 2032+ gerçekçi
```

---

## Bölüm 11 — Gerçekçi Zaman Çizelgesi

```
BUGÜN (2024-2025):
  7B model: Apple M4 Pro, M4 Max → mümkün, iyi
  13B model: Apple M4 Ultra → mümkün, yavaş
  70B model: M4 Ultra → zar zor, çok yavaş
  400B model: İMKÂNSIZ (bant genişliği + kapasite)

2026-2028 (HBM4 + gelişmiş kuantizasyon):
  70B: rahat çalışıyor (ternary + HBM4)
  400B: hâlâ imkânsız SoC'ta, sunucu klüster gerekiyor

2029-2031 (PIM bellek + NMC):
  70B: Thunderbolt SoC ile gerçekçi
  400B: özel büyük SoC ile mümkün (masaüstü boyutu)

2032+ (FE+EO CIM + SSM hibrit):
  400B ternary + SSM bağlam:
  - 100 GB ağırlık depolama
  - 5-15W güç
  - Pasif soğutma
  - Sınırsız bağlam (SSM)
  - Thunderbolt SoC: GERÇEKÇİ!
```

---

## Bölüm 12 — Bugün Ne Yapılabilir: Gerçekçi Alternatiflere

```
400B beklenirken:

Seçenek 1 — Daha küçük model:
  70B ternary → bugün ~35 GB → M4 Ultra ile çalışır
  Doğruluk: 400B'nin %85-90'ı
  Deneyim: 70B büyük çoğunluk için yeterli

Seçenek 2 — Bulut hibrit:
  SoC: yerel küçük model (7B) → gizlilik korunan görevler
  Bulut: 400B → hassas olmayan görevler
  Karma routing → en iyi ikisi bir arada

Seçenek 3 — Mixture of Experts (MoE):
  400B parametreli MoE modeli:
  Her token için yalnızca ~50B aktif parametre
  Efektif bant genişliği: 50B/token → 400B × 30 değil!
  50B × 30 tok/s = 1,500 GB/s → HBM3 ile yapılabilir

  Mixtral 8×7B: bu mantıkla çalışıyor
  400B sınıfı MoE: araştırma aşamasında

Seçenek 4 — Uzun vadeli (2028+):
  Neuromorphic ön işleme + küçük model
  Edge AI için optimize: belirli görevler çok iyi
  Genel amaçlı 400B: bekle
```

---

## Özet: Engellerin Hiyerarşisi

```
EN KRİTİK (şimdi çözülemiyor):
  Bellek bant genişliği: 2,250 GB/s gerekli, 900 GB/s mevcut
  → Bunu çözmek için FE+EO CIM gerekiyor (2032+)

BÜYÜK AMA ÇÖZÜLEBILIR (kısmen):
  Bellek kapasitesi: ANS ternary ile 75 GB → M4 Ultra sığıyor
  Güç: FE+EO CIM ile 4W → bugün 200W+

KALICI SORUN (mimari değişikliği gerekiyor):
  KV cache: SSM veya hibrit mimari ile çözülmeli
  Uzun bağlam (128K+): FE+EO CIM de çözmüyor

SORUN DEĞİL:
  Hesaplama TOPS: zaten yeterli (24 TOPS gerekli, 38+ mevcut)
  Thunderbolt bant genişliği: model yükleme için yeterli

Tek cümle özet:
  Bant genişliği engeli tüm diğerlerini geçersiz kılıyor.
  Model sığsa bile, yeterli hızda okuyamıyoruz.
  FE+EO CIM bu engeli fizik yasasıyla aşıyor:
  okuma yok → bant genişliği sorunu yok.
  2032'ye kadar 70B gerçekçi hedef, 400B değil.
```
