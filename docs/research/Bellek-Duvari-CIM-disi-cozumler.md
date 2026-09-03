> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Bellek Duvarı — CIM/PIM Dışındaki Çözümler

> Kaynaklar: FMS 2026, IEEE Spectrum, HPCwire, The Register,
> arXiv, Google araştırma ekibi (Ma & Patterson), TrendForce
> Tarih: Ağustos 2026

---

## Bölüm 1 — Sorunun Güncel Boyutu

### 1.1 Rakamlar Kötüleşiyor

```
2023-2025 arası (Google araştırmacılarının verisi):
  HBM maliyeti: kapasite ve bant genişliğinde %35 ARTIŞ
  Standart DDR maliyeti: yaklaşık YARIYA düştü
  → Makas açılıyor

DRAM kapasite ölçekleme:
  Tarihsel: 4× kapasite artışı 3-6 yılda
  Şimdi:    aynı artış 10+ yıl alacak
  → Ölçekleme duvara çarptı
```

### 1.2 Aritmetik Yoğunluk Krizi

```
Batch=1 decode aşamasında:
  Aritmetik yoğunluk ≈ 0.75 FLOP/byte (FP16)

  Yani: her byte için 0.75 işlem
  Modern GPU: ~100+ FLOP/byte için tasarlandı
  → Hesaplama birimleri %1 kullanımda

Enerji tabanı:
  7B FP16 model → 2.24 J/token
  Bunun büyük kısmı BELLEK ERİŞİMİ, hesaplama değil
```

### 1.3 Ekonomik Sonuç

```
2026 bellek krizi:
  Bellek, PC ve telefonda toplam malzeme maliyetinin %35'i
  (2024'te %17 idi — iki katına çıktı)

  Gartner tahmini: $500 altı PC segmenti 2028'de kaybolacak

"GPU açlığı" (GPU Starvation):
  Pahalı GPU'lar veri beklerken boşta duruyor
```

---

## Bölüm 2 — CIM/PIM Tek Çözüm mü? HAYIR

Araştırma en az **sekiz farklı yaklaşım** gösteriyor. CIM bunlardan sadece biri — ve şu an en olgunu bile değil.

```
YAKLAŞIM                    DURUM        ZAMAN
─────────────────────────────────────────────────────
1. HBF (NAND tabanlı)       Standart     2027
2. SRAM-only (wafer-scale)  ÜRETİMDE     Bugün
3. Ağırlıkları kazıma       Satın alındı Bugün
4. CXL bellek havuzu        Ürün var     Bugün
5. Katmanlı bellek          Ürün var     Bugün
6. Yazılım (kuantizasyon)   Ürün var     Bugün
7. Mimari (MoE, SSM)        Ürün var     Bugün
8. CIM/PIM                  Araştırma    2028+
9. Fotonik interconnect     Araştırma    2030+
```

**Kritik gözlem:** CIM listenin en olgunlaşmamış maddelerinden.
Bugün çalışan çözümlerin çoğu CIM değil.

---

## Bölüm 3 — Çözüm 1: HBF (High Bandwidth Flash)

En yeni ve en çok konuşulan alternatif.

### 3.1 Nedir

```
Fikir: HBM gibi istifle, ama DRAM yerine NAND flash kullan

Sandisk 1. nesil:
  16 NAND çipi istifi
  512 GB / istif kapasite
  1.6 TB/s okuma bant genişliği

  2. nesil: 2 TB/s
  3. nesil: 3.2 TB/s

Karşılaştırma:
  HBM3e:  ~1.2 TB/s, 24-36 GB
  HBM4:   ~2.5 TB/s (12-high), 48-64 GB
  HBF:    1.6-3.2 TB/s, 512 GB  ← 8-16× KAPASİTE
```

### 3.2 Neden Devrimsel

```
HBM'in gerçek sorunu bant genişliği değil, KAPASİTE:
  400B model → 100 GB (ternary) veya 800 GB (FP16)
  HBM4 tek istif: 64 GB
  → Çok çipli sistem zorunlu → yazılım parçalama →
    sık iletişim

HBF ile:
  Tek istif 512 GB
  → 1 trilyon FP4 ağırlık tek cihazda
  → 59 token/s (HBF bant genişliği sınırlı)
  → Mobil edge cihazda rack ölçeği performansı
```

### 3.3 Zayıf Yön ve Zekice Çözüm

```
NAND'ın sorunu: YAZMA çok yavaş

Ama:
  KV cache okuma ağırlıklı bir iş
  Model ağırlıkları bir kez yazılıyor, milyar kez okunuyor
  → HBF bu iş yükü için mükemmel uyum

Sandisk'in disaggregated (ayrıştırılmış) mimarisi:
  Prefill (hesaplama ağır)  → HBM
  Decode (bellek ağır)      → HBF
  → Farklı bellek türü, farklı aşama
```

### 3.4 Durum

```
Şubat 2026: Sandisk + SK Hynix, OCP altında standart çalışması
            Google ve Tenstorrent katıldı
2026 H2:    İlk örnekler
2027 başı:  İlk AI çıkarım cihazları

UCIe protokolü ile bağlanıyor → chiplet ekosistemine uyumlu
```

---

## Bölüm 4 — Çözüm 2: SRAM-Only (Radikal Basitlik)

DRAM'i tamamen atma yaklaşımı.

```
Fikir: HBM yerine sadece on-chip SRAM kullan

Groq LPU:
  Nvidia GTC 2026'da öne çıkan duyuru
  HBM'in SRAM lehine yerinden edilmesi
  Nvidia $20 milyara Groq teknolojisini lisansladı

Cerebras (wafer-scale):
  Tek wafer boyutunda çip
  44 GB on-chip SRAM
  AWS ve OpenAI ile anlaşmalar

Avantaj:
  SRAM bant genişliği ~10-100 TB/s (on-chip)
  DRAM'e hiç gitmiyorsun

Dezavantaj:
  Kapasite küçük (Cerebras: 44 GB)
  Alan pahalı (SRAM 6 transistör/bit)
  → Büyük model için çok çip gerekiyor
```

---

## Bölüm 5 — Çözüm 3: Ağırlıkları Silikona Kazıma

Konuşmada detaylı incelendi. Özet:

```
Taalas modeli:
  Ağırlıklar transistör düzeninde kalıcı
  Bellek okuma SIFIR

  HC1: Llama 3.1 8B, 17,000 token/s, H200'ün 1/10 gücü
  AMD tarafından satın alındı (Ağustos 2026)

Sınır: model değişince çip çöp
```

---

## Bölüm 6 — Çözüm 4: CXL Bellek Havuzu

```
Fikir: belleği çipten ayır, havuzda topla, paylaş

CXL (Compute Express Link):
  PCIe üzerinde önbellek-tutarlı bellek protokolü
  Bellek genişletme, havuzlama, paylaşım

GTC 2026'da:
  Penguin — CXL tabanlı MemoryAI KV cache sunucusu
  → GPU kümelerine düşük gecikme, yüksek verim

Avantaj:
  Kapasite esnek (TB seviyesi)
  Kullanılmayan bellek başka işleme verilebiliyor
  Maliyet DDR seviyesinde

Dezavantaj:
  Gecikme HBM'den yüksek (~200-400 ns vs ~100 ns)
  Bant genişliği PCIe ile sınırlı
  → Model ağırlığı için yavaş, KV cache için uygun
```

---

## Bölüm 7 — Çözüm 5: Katmanlı Bellek Mimarisi

FMS 2026'nın ana teması:

```
Katman        Teknoloji    Bant Gen.    Kapasite    Maliyet
──────────────────────────────────────────────────────────────
L0  On-chip   SRAM         10+ TB/s     ~50 MB      çok yüksek
L1  Paket içi HBM4         2.5 TB/s     64 GB       yüksek
L2  Paket içi HBF          1.6 TB/s     512 GB      orta
L3  CXL       DDR5         100 GB/s     TB'lar      düşük
L4  Depolama  NVMe SSD     7 GB/s       10+ TB      çok düşük

Strateji:
  Sıcak veri → yukarı katmanlar
  Soğuk veri → aşağı katmanlar
  MoE ile mükemmel uyum: nadir uzmanlar SSD'de
```

SK hynix'in FMS 2026 açılış konuşması tam bu konuda:
"katmanlı bellek mimarisi ile AI altyapı verimliliği"

---

## Bölüm 8 — Çözüm 6: Yazılım Tarafı (En Ucuz)

Donanım değişmeden yapılabilecekler:

### 8.1 Kuantizasyon

```
Google TurboQuant (2026):
  KV bellek sıkıştırma → değer başına 3.5 bit
  → Kapasite darboğazını doğrudan hedefliyor

Ternary (BitNet b1.58):
  16 bit → 1.58 bit → 10× az veri okuma

Karma hassasiyet:
  Dikkat Q/K: INT4-8
  FFN: ternary
  → Doğruluk korunuyor, bant genişliği düşüyor
```

### 8.2 Paged Attention ve Sürekli Batching

```
vLLM'in getirdiği teknikler:
  Paged attention: KV cache sayfalama (işletim sistemi gibi)
  Continuous batching: istekler dinamik gruplanıyor
  → Bellek kullanımı %50-80 azalıyor
  → Donanım değişmeden
```

### 8.3 Speculative Decoding

```
Küçük model tahmin eder, büyük model onaylar
→ Aynı bant genişliğinde 2-3× hız
→ Ek donanım gerekmiyor
```

---

## Bölüm 9 — Çözüm 7: Mimari Değişimi (En Etkili)

Modelin kendisini değiştirmek:

### 9.1 MoE (Mixture of Experts)

```
400B parametre, ama her token için sadece ~100B aktif
→ Bant genişliği ihtiyacı 4× azalıyor

Bugün üretimde: Mixtral, DeepSeek, Llama 4
→ En yaygın kabul gören çözüm
```

### 9.2 SSM / Hibrit (Mamba)

```
Transformer: KV cache O(N) büyüyor
Mamba: sabit durum O(1)

128K bağlam:
  Saf Transformer: 1 TB KV cache
  %75 Mamba hibrit: 64 GB
  → 16× azalma
```

### 9.3 Bağlam Yönetimi

```
Sorun: kurumsal bağlam pencereleri 24 ayda
       100K token'dan 100M token'a çıktı

Çözümler:
  Kayan pencere
  Hiyerarşik özet
  Retrieval (RAG) — tüm bağlamı bellekte tutmama
```

---

## Bölüm 10 — Çözüm 8: Ağ Yeniden Düşünme

Google araştırmacılarının (Ma & Patterson) vurgusu:

```
"Büyük ağırlıklar nedeniyle LLM çıkarımı artık çok çipli
 sistem gerektiriyor; yazılım parçalama (sharding) sık
 iletişim demek."

Yani sorun sadece bellek değil, AĞ:
  Çok çip → parçalama → çipler arası iletişim
  → Ağ gecikmesi yeni darboğaz

Öneri:
  Veri merkezi tasarımına hakim olan
  gecikme-bant genişliği ödünleşimini temelden yeniden düşün
```

**Yan fayda:** Daha yüksek kapasiteli bellek (HBF gibi)
sistem boyutunu küçültüyor → iletişim yükü azalıyor.
Yani bellek çözümü aynı zamanda ağ çözümü.

---

## Bölüm 11 — Patent Verisinden Görülen Yönelimler

PatSnap analizi (2022-2025 başvuruları) beş yön gösteriyor:

```
1. Model-güdümlü DMA bant genişliği optimizasyonu
2. Ölçekli analog bellek-içi hesaplama
3. AIMC tile'ları için kablosuz çip-içi iletişim  ← ilginç
4. Fotonik bellek interconnect ve fotonik AI çipleri
5. ROM tabanlı CIM (büyük ölçekli sinir ağları için)

Küme büyüklüğü:
  SRAM tabanlı CIM → en fazla başvuru
  Dirençli NVM CIM → ikinci
  3D istifleme → üçüncü
```

**Dikkat çekici:** "ROM tabanlı CIM" — Taalas'ın yaklaşımının
patent literatüründeki karşılığı. Ağırlık değişmiyorsa
ROM yeterli, çok daha yoğun.

---

## Bölüm 12 — Karşılaştırmalı Değerlendirme

```
ÇÖZÜM              BANT GEN.  KAPASİTE  ESNEKLİK  OLGUNLUK  MALİYET
──────────────────────────────────────────────────────────────────────
HBM4               ★★★★★     ★★☆       ★★★★★    ★★★★★    ★☆
HBF                ★★★★      ★★★★★    ★★★★★    ★★☆      ★★★★
SRAM-only          ★★★★★     ★☆        ★★★★★    ★★★★     ★☆
Ağırlık kazıma     ★★★★★     ★★★★      ☆         ★★★★     ★★★
CXL havuz          ★★         ★★★★★    ★★★★★    ★★★★     ★★★★★
Katmanlı bellek    ★★★★      ★★★★★    ★★★★★    ★★★★     ★★★★
Kuantizasyon       —          ★★★★     ★★★★★    ★★★★★    ★★★★★
MoE/SSM            —          ★★★★★    ★★★★     ★★★★★    ★★★★★
CIM/PIM            ★★★★★     ★★★       ★★★       ★★        ★★
Fotonik            ★★★★★     ★★        ★★★       ★         ★
```

**En yüksek getiri/maliyet oranı:** Kuantizasyon + MoE.
Donanım değişmeden, bugün, ücretsiz.

---

## Bölüm 13 — Neden CIM Tek Çözüm Değil

```
CIM'in yapısal sorunları:

1. Analog non-idealiteler:
   Uyumsuzluk (mismatch), drift, kuantizasyon hatası
   → Eğitim ve ince ayar ile telafi gerekiyor
   → Her çip biraz farklı davranıyor

2. Yazma dayanıklılığı:
   ReRAM/PCM: 10⁶-10⁸ döngü
   → Ağırlık güncellemesi sınırlı

3. Programlanabilirlik:
   Model değişince ağırlıkları yeniden yazmak pahalı
   → Esneklik kaybı

4. Ölçek:
   Analog toplama gürültüsü dizi boyutuyla artıyor
   → Büyük dizi zor

5. Standart yokluğu:
   Her üretici kendi arayüzü
   → Yazılım taşınamıyor

6. ADC maliyeti:
   Analog CIM'de ADC sistem enerjisinin büyük kısmı
   → Çözüm arayışı sürüyor (şarj tabanlı toplama vb.)
```

Bu yüzden endüstri CIM'i beklemiyor — paralel yollar deniyor.

---

## Bölüm 14 — Gerçekçi Yol Haritası

```
BUGÜN (uygulanabilir):
  ✓ Kuantizasyon (ternary, INT4, karma)
  ✓ MoE mimarisi
  ✓ SSM/hibrit (Mamba)
  ✓ Paged attention, continuous batching
  ✓ Speculative decoding
  ✓ Katmanlı bellek (HBM + DDR + SSD)
  → Bunlar birleşince 10-50× iyileşme

2027:
  ✓ HBF örnekleri → 512 GB/istif
  ✓ HBM4 yaygınlaşması
  ✓ CXL 3.0 bellek havuzları

2028-2030:
  △ CIM ilk ticari ürünler (dar alanlarda)
  △ 3D istiflenmiş bellek+lojik
  △ ROM tabanlı ağırlık depolama yaygınlaşması

2030+:
  △ Fotonik interconnect
  △ Fotonik CIM (araştırma)
```

---

## Bölüm 15 — Volt İçin Anlamı

Bu çeşitlilik Volt'un domain sistemini daha da değerli kılıyor:

```volt
// Katmanlı bellek = farklı domain'ler
domain SramTier    { latency = 1.ns,   bandwidth = 10.tbps }
domain Hbm4Tier    { latency = 100.ns, bandwidth = 2.5.tbps }
domain HbfTier     { latency = 500.ns, bandwidth = 1.6.tbps,
                     write_endurance = 100_000 }
domain CxlTier     { latency = 300.ns, bandwidth = 100.gbps }

module TieredInference {
    in  hot_weights  : [Trit; N] @SramTier
    in  warm_weights : [Trit; M] @Hbm4Tier
    in  cold_experts : [Trit; K] @HbfTier

    // Katman geçişi → tip sistemi denetliyor
    // Yanlış katmandan okuma → derleme hatası
    // HBF'ye sık yazma → dayanıklılık uyarısı

    @budget(hbf_writes_per_second = 100)  // endurance koruması
}
```

**Volt'un katkısı:** Bu heterojen bellek hiyerarşisini
tip-güvenli yönetmek. Bugün her tasarımcı elle yapıyor,
yanlış katmandan erişim sessiz performans kaybı yaratıyor.

---

## Özet

```
CIM/PIM tek çözüm mü?  KESİNLİKLE HAYIR

Bugün çalışan çözümler (CIM değil):
  Kuantizasyon    → 8-10× veri azalması, ÜCRETSİZ
  MoE             → 4× aktif parametre azalması
  SSM/hibrit      → 16× KV cache azalması
  Katmanlı bellek → kapasite/maliyet optimizasyonu
  Speculative     → 2-3× hız

Yakında gelecekler:
  HBF (2027)      → 512 GB/istif, 8-16× kapasite
  HBM4 (2026)     → 2.5 TB/s
  CXL havuzları   → esnek kapasite

CIM'in yeri:
  Araştırma aşamasında, 2028+ dar alanlarda
  Analog non-idealiteler, dayanıklılık, standart yokluğu
  → Endüstri beklemiyor, paralel yollar deniyor

En büyük içgörü:
  Bellek duvarı TEK bir çözümle aşılmıyor
  On farklı tekniğin ÇARPIMI ile aşılıyor
  
  Örnek: ternary (8×) + MoE (4×) + speculative (2.5×)
         = 80× efektif iyileşme
         → 400B model bugünkü donanımda çalışabilir hale geliyor
```
