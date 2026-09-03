> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# 400B Model / 30 Token/s — Özel Çıkarım Cihazı Teknik Spesifikasyonu

> Mac mini/Studio boyutunda, 400B parametreli modeli saniyede 30 token
> hızında çalıştıracak özel donanım ve yazılım tasarımı.
> Bu belge "bugün yapılabilir" ile "2027-2028 hedefi"ni
> net ayrımla ortaya koyuyor.

---

## Bölüm 1 — Gereksinim Analizi: Sayılar Önce

### 1.1 Temel Kısıt Denklemi

```
Gerekli bellek bant genişliği = Model boyutu × Token hızı

Senaryo A — Ternary + ANS (bugünün en iyisi):
  75 GB × 30 tok/s = 2,250 GB/s

Senaryo B — Hybrid SSM (%75 Mamba):
  KV cache sadece Attention katmanlarında:
  Efektif bant genişliği ≈ 2,250 GB/s (model)
                          + 100 GB/s (kV + Mamba durum)
  Toplam: ~2,350 GB/s

Hesaplama (TOPS):
  400B MACs/token × %50 sıfır (ternary seyreklik)
  = 200B efektif MAC/token × 30 tok/s = 6 TOPS minimum
  Güvenlik payı (2×): 12-15 TOPS yeterli
  → Hesaplama KOLAY, bant genişliği ZOR

Bellek kapasitesi:
  Model ağırlıkları (ternary+ANS): 75 GB
  KV cache (hybrid, 4K bağlam):   8 GB
  Mamba durumu:                   0.4 GB
  Aktivasyonlar + buffer:         5 GB
  ─────────────────────────────────────
  Toplam: ~89 GB minimum, 128 GB konforlu
```

### 1.2 Hangi Teknoloji Bu Sayıları Karşılar

```
HBM4 (2025'te piyasaya çıkıyor):
  Bant genişliği: 1,500 GB/s / yığın
  Kapasite:       48-64 GB / yığın
  
  2 yığın: 3,000 GB/s, 96-128 GB → YETERLI ✓
  3 yığın: 4,500 GB/s, 144-192 GB → KONFORLU ✓✓

LPDDR6 (2026+):
  Bant genişliği: 256 GB/s / paket
  → 9 paket gerekir → çok geniş alan, değil

GDDR7:
  Bant genişliği: ~900 GB/s toplam (64-bit × 16 chip)
  → Yetmez (2,250 GB/s gerekiyor)

KARAR: 2-3× HBM4 yığını, silikon interposer üzerinde
```

---

## Bölüm 2 — Donanım Mimarisi

### 2.1 Ana Hesaplama Çipi (Custom ASIC)

```
Süreç teknolojisi: TSMC 3nm veya Samsung 3GAP
Çip alanı: ~600-800 mm²  (Apple M4 Ultra: ~260 mm², bu daha büyük)

İç yapı:

┌────────────────────────────────────────────────────────┐
│                   Ana Hesaplama Çipi                   │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │    Ternary Sistolic Dizi (Compute Fabric)        │  │
│  │    512 × 512 PE = 262,144 işlem birimi           │  │
│  │    Her PE: 4 ternary MAC (INT8 aktivasyon)       │  │
│  │    Toplam: ~1 TOPS @ 1GHz                        │  │
│  │    Hedef: 200 TOPS efektif (sıfır atlama ile)    │  │
│  └──────────────────────────────────────────────────┘  │
│                                                         │
│  ┌────────────────┐   ┌────────────────────────────┐   │
│  │  Mamba SSM     │   │  Attention Engine          │   │
│  │  Donanım Hızl. │   │  (hybrid %25 katman için)  │   │
│  │  Seçici tarama │   │  FlashAttention-3 tarzı    │   │
│  └────────────────┘   └────────────────────────────┘   │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │  On-chip SRAM: 512 MB                           │   │
│  │  Mamba durumu (200 MB) + sıcak aktivasyon (312) │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │  HBM4 Denetleyici × 3 kanal                    │   │
│  │  Her kanal: 1,500 GB/s → Toplam: 4,500 GB/s    │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  ┌────────────┐  ┌──────────┐  ┌───────────────────┐  │
│  │ PCIe 5.0   │  │ USB4     │  │  Güç Yönetimi     │  │
│  │ × 16 host  │  │ Gen3×2   │  │  (DVFS, güç adası)│  │
│  └────────────┘  └──────────┘  └───────────────────┘  │
└────────────────────────────────────────────────────────┘
```

**Ternary Sistolic Dizi Tasarımı:**

```
Her PE (İşlem Birimi):
  ┌──────────────────────────────┐
  │  Ağırlık kaydı: Trit (2 bit) │
  │  Aktivasyon girişi: INT8      │
  │                               │
  │  Trit = +1: sonuç += aktiasyon│
  │  Trit =  0: atla (0 enerji!)  │
  │  Trit = -1: sonuç -= aktivasyon│
  │                               │
  │  Çarpan YOK — sadece MUX+ADD  │
  └──────────────────────────────┘

Sıfır atlama (zero-skip) donanımı:
  Ağırlık = 0 → PE hiç aktive olmaz
  %50 ağırlık sıfır → %50 enerji tasarrufu
  %50 ağırlık sıfır → efektif bant genişliği yarıya

200 TOPS efektif hesabı:
  262,144 PE × 4 MAC × 2 GHz clock = 2 TOPS raw
  Sıfır atlama (%50): efektif 1 TOPS
  
  (Yetmiyor! Daha büyük dizi gerekiyor)
  
Gerçekçi hedef için:
  4096 × 4096 PE = 16,777,216 PE
  × 2 GHz = 33 TOPS efektif
  Çip alanı: ~400 mm² sadece compute
  
  Veya: daha küçük dizi + daha yüksek saat
  1024 × 1024 PE @ 3 GHz + pipeline = ~10 TOPS
  
  NOT: 400B / 30 tok/s = 6 TOPS yeterli
  12 TOPS (güvenlik payıyla) hedef → 1024×1024 @ 3GHz makul
```

### 2.2 Bellek Alt Sistemi (HBM4 + İnterposer)

```
Fiziksel yapı (üstten görünüm):

  ┌───────────────────────────────────────────┐
  │         Silicon İnterposer (CoWoS-L)      │
  │  ~130mm × 100mm                           │
  │                                           │
  │   ┌──────────┐  ┌──────┐  ┌──────┐       │
  │   │ Ana Çip  │  │ HBM4 │  │ HBM4 │       │
  │   │ (~600mm²)│  │  #1  │  │  #2  │       │
  │   │          │  │ 48GB │  │ 48GB │       │
  │   │          │  │1.5T  │  │1.5T  │       │
  │   └──────────┘  └──────┘  └──────┘       │
  │                    ┌──────┐               │
  │                    │ HBM4 │               │
  │                    │  #3  │ (opsiyonel)   │
  │                    │ 48GB │               │
  │                    │1.5T  │               │
  │                    └──────┘               │
  └───────────────────────────────────────────┘
  
  Toplam (3 yığın):
  Kapasite: 144 GB
  Bant genişliği: 4,500 GB/s
  
  Model (75 GB) + KV + Mamba + buffer = ~90 GB → sığıyor ✓
  Bant genişliği: 4,500 > 2,250 GB/s → 2× fazla → hız tampon ✓
```

**İnterposer Teknoloji Seçimi:**

```
Seçenek A — TSMC CoWoS-L (Chip on Wafer on Substrate Large):
  ✓ Olgun teknoloji (NVIDIA H100/H200 kullanıyor)
  ✓ Ana çip + 4 HBM yığını destekliyor
  ✓ TSMC'de yerleşik süreç
  ✗ Pahalı (başına ~$30K-50K non-recurring engineering)
  ✗ Teslim süresi: 6-12 ay

Seçenek B — Custom Silikon İnterposer:
  ✓ Tam kontrol (routing optimize edilebilir)
  ✗ Daha pahalı (özel maske seti)
  ✗ Daha uzun geliştirme süresi

Seçenek C — Organik İnterposer (EMIB tarzı, Intel):
  ✓ Daha ucuz
  ✗ Daha yüksek sinyal kaybı
  ✗ HBM bant genişliği kısıtlı

ÖNERİ: CoWoS-L (kanıtlanmış, TSMC desteği mevcut)
```

### 2.3 Soğutma Sistemi

```
Güç bütçesi:
  Ana çip:              120-150W
  3× HBM4:              15-25W
  PCIe/USB kontrolcü:    5W
  Anakart + VRM:        10W
  Fan:                   5-10W
  ─────────────────────────────
  Toplam:              155-200W

Mac Studio boyutu (200mm × 200mm × 95mm):
  Bu güç için yeterli ısıl kütleye sahip

Soğutma tasarımı:
  ┌─────────────────────────────────────────┐
  │                                         │
  │  [Fan] →  [Radyatör]  ← [Isı Borusu]  │
  │               ↑               ↑         │
  │          [Buhar Odası] ─────────────── │
  │               ↑                         │
  │          [Ana Çip + HBM]               │
  │                                         │
  └─────────────────────────────────────────┘
  
  Buhar odası (vapor chamber):
  200mm × 150mm → 150W iletim kapasitesi → yeterli
  
  Fan: 120mm × 2 adet (Mac Studio tarzı)
  Gürültü: ~35 dB(A) yükte (Mac Studio ile benzer)
  
  Çip junction sıcaklığı hedefi: < 80°C @ 150W ✓
```

### 2.4 Host Bağlantısı

```
Thunderbolt 5 (USB4 80Gbps):
  Gerçek bant genişliği: ~15 GB/s PCIe tüneli
  Kullanım: aktivasyon I/O + model yükleme

  Token başına aktivasyon transfer:
  400B model giriş/çıkış: ~4 MB/token
  30 tok/s: 120 MB/s → Thunderbolt yeterli ✓

  Model yükleme (75 GB):
  @ 15 GB/s: 5 saniye → kabul edilebilir

PCIe 5.0 × 8 (alternatif):
  Bant genişliği: 32 GB/s
  Daha hızlı model yükleme (2.3 sn)
  Kablo gerektirmez (PCIe yuvası)
  Masaüstü PC için ideal

ÖNERİ: İki bağlantı seçeneği:
  - Thunderbolt 5 portu × 2 (bağımsız kullanım, laptop uyumlu)
  - PCIe 5.0 × 8 konnektör (masaüstü entegrasyon için)
```

---

## Bölüm 3 — Yazılım Yığını

### 3.1 Katman Mimarisi

```
┌────────────────────────────────────────────────────────┐
│               KULLANICI KATMANI                         │
│  Python API / OpenAI uyumlu HTTP / CLI                 │
└────────────────────────────────────────────────────────┘
                         │
┌────────────────────────────────────────────────────────┐
│               ÇIKARIM MOTORU (Rust)                    │
│                                                         │
│  Model yükleyici    Tokenizer    Örnekleyici            │
│  SafeTensors/GGUF   BPE/SentPiece  Top-p/Temp          │
│                                                         │
│  Ternary kuantizasyon pipeline                         │
│  ANS kodek (sıkıştır/aç)                              │
│  Mamba SSM operatörü                                   │
│  Hybrid dikkat zamanlayıcı                             │
└────────────────────────────────────────────────────────┘
                         │
┌────────────────────────────────────────────────────────┐
│               DONANIM SOYUTLAMA KATMANI (Rust + C FFI) │
│                                                         │
│  Ternary sistolic dizi sürücüsü                        │
│  HBM4 bellek yöneticisi                                │
│  Mamba donanım birimi arabirimi                         │
│  PCIe/Thunderbolt I/O yöneticisi                       │
│  Güç ve termal yönetici (DVFS)                         │
└────────────────────────────────────────────────────────┘
                         │
┌────────────────────────────────────────────────────────┐
│               DONANIM (Custom ASIC)                    │
└────────────────────────────────────────────────────────┘
```

### 3.2 Çıkarım Motoru (Rust)

```rust
// Temel çıkarım döngüsü (pseudocode)
pub struct InferenceEngine {
    model: TernaryModel,          // ternary ağırlıklar, HBM4'te
    mamba_state: MambaState,      // on-chip SRAM'de
    kv_cache: Option<KVCache>,    // DRAM'de (hybrid attention için)
    hardware: HardwareAbstraction,
}

impl InferenceEngine {
    pub fn generate(&mut self, tokens: &[u32]) -> Vec<u32> {
        let mut output = vec![];
        
        for &token in tokens.iter() {
            // 1. Token embedding (lookup)
            let x = self.model.embed(token);
            
            // 2. Her katmanı işle (hybrid: Mamba veya Attention)
            let mut hidden = x;
            for (i, layer) in self.model.layers.iter().enumerate() {
                hidden = match layer.kind {
                    LayerKind::Mamba => {
                        // Sabit bellekli SSM adımı
                        self.hardware.mamba_step(
                            &hidden, 
                            &mut self.mamba_state.layers[i],
                            &layer.weights
                        )
                    }
                    LayerKind::Attention => {
                        // KV cache ekle ve dikkat hesapla
                        self.kv_cache.as_mut().map(|cache| {
                            cache.append(i, &hidden);
                            self.hardware.attention(
                                &hidden,
                                cache.get(i),
                                &layer.weights
                            )
                        }).unwrap()
                    }
                };
            }
            
            // 3. Sonraki token tahmin
            let logits = self.model.lm_head(&hidden);
            let next_token = sample(logits, self.config.temperature);
            output.push(next_token);
        }
        output
    }
}
```

### 3.3 Ternary Kuantizasyon Pipeline

```
Model giriş formatı: HuggingFace SafeTensors (FP16/BF16)
Hedef format: Özel Ternary Binary

Kuantizasyon adımları:
  1. Ağırlık istatistikleri hesapla (her katman için)
     - Ortalama, std, kurtosis ölçümü
     
  2. Eşik (threshold) hesapla:
     δ = 0.7 × mean(|W|)  ← TWN (Ternary Weight Networks) kuralı
     Veya katmana özgü kalibrasyon (daha iyi doğruluk)
     
  3. Trit atama:
     W[i] > +δ  → +1
     W[i] < -δ  → -1
     aksi halde →  0
     
  4. Ölçek faktörü kaydet (FP16):
     α = mean(|W[W≠0]|)  ← sıfır olmayanların ortalaması
     
  5. ANS kodlama:
     Dağılımı ölç: P(0), P(+1), P(-1)
     ANS tablosu oluştur
     Trit akışını kodla → binary

  6. Doğruluk değerlendirme:
     Kalibrasyon seti ile PPL (perplexity) ölç
     Kabul edilemezse: eşik ayarla, tekrar dene

  Araç: Python + Rust karışımı
  Python: istatistik + karar
  Rust: ANS kodlama (hız kritik)
  
  Süre: 400B model → ~24 saat (8× A100 ile)
  Çıktı: ~75 GB .tbin dosyası
```

### 3.4 Mamba SSM Donanım Arayüzü

```
Mamba'nın seçici tarama (selective scan) operasyonu:
  h_t = A_t × h_{t-1} + B_t × x_t

Donanım gereksinimleri:
  - Matris vektör çarpımı: A_t (64×64) × h (64×4096) → GPU gibi
  - Ama A_t inputa bağlı → her adımda farklı
  - Paralel tarama eğitimde, sıralı çıkarımda

Özel donanım birimi:
  ┌────────────────────────────────────────┐
  │         Mamba SSM Hızlandırıcı         │
  │                                         │
  │  h_prev (64×4096) ← on-chip SRAM'den  │
  │       ↓                                 │
  │  A_t hesapla (inputa bağlı matris)     │
  │       ↓                                 │
  │  A_t × h_prev + B_t × x_t             │
  │       ↓                                 │
  │  h_next → on-chip SRAM'e geri yaz     │
  │                                         │
  │  Gecikme: 1-2 saat döngüsü hedefi     │
  └────────────────────────────────────────┘

  h her katman için SRAM'de tutulur:
  400 katman × 64 × 4096 × 2 byte = 200 MB
  → 512 MB on-chip SRAM yeterli
```

### 3.5 Bellek Yöneticisi

```
HBM4 bellek haritası (128 GB toplam):

┌─────────────────────────────────────────────────────┐
│ 0x00000000  Model ağırlıkları (ternary, ANS açık)  │
│             75 GB                                    │
├─────────────────────────────────────────────────────┤
│ 0x12C00000  KV Cache (Attention katmanları için)    │
│             20 GB (elastik: bağlama göre büyür)     │
├─────────────────────────────────────────────────────┤
│ 0x1DC00000  Aktivasyon tampon (çift arabellek)      │
│             4 GB                                     │
├─────────────────────────────────────────────────────┤
│ 0x1FC00000  İşletim sistemi + sürücü               │
│             4 GB                                     │
│ 0x1FFFFFFF                                          │
└─────────────────────────────────────────────────────┘

Bellek bant genişliği zamanlayıcı:
  Ağırlık okuma: yüksek öncelik, zaman-kritik
  KV cache: orta öncelik (dikkat hesaplarında)
  Aktivasyon: düşük öncelik (geçici)
  
  Çifte tamponlama (double buffering):
  Katman N hesaplanırken katman N+1 ağırlıkları ön yüklenir
  → Bellek gecikmesi gizlenir
  → Gerçek verim teorik maksimuma yaklaşır
```

---

## Bölüm 4 — Desteklenmesi Gereken Teknolojiler

### 4.1 Model Formatları

```
Zorunlu:
  ✓ SafeTensors (HuggingFace standardı)
  ✓ GGUF (llama.cpp ekosistemi)
  ✓ PyTorch checkpoint (.pt, .bin)

İyi olur:
  ✓ ONNX (geniş model desteği)
  ✓ MLX format (Apple ekosistemi)

Çıkış formatı (cihaza özgü):
  ✓ .tbin (Ternary Binary) — kendi geliştirilen format
     - Başlık: model meta verisi, katman yapısı
     - Ağırlıklar: ANS kodlu ternary bloklar
     - Ölçek faktörleri: FP16
```

### 4.2 Model Mimarisi Desteği

```
Zorunlu:
  ✓ Transformer (GPT, Llama, Mistral tarzı)
  ✓ Grouped Query Attention (GQA) — 70B+ modellerde yaygın
  ✓ RoPE (Rotary Positional Embedding)
  ✓ SwiGLU aktivasyon

Önemli:
  ✓ Mixture of Experts (MoE) — verimli 400B için kritik
  ✓ Mamba / SSM (hybrid mimari için)
  ✓ Sliding window attention (uzun bağlam için)

Gelecek (v2):
  ○ RWKV
  ○ Griffin
  ○ RetNet
```

### 4.3 Kuantizasyon Desteği

```
Cihazda çalışabilecek:
  ✓ Ternary (TWN tarzı)
  ✓ INT4 (fallback, ternary yoksa)
  ✓ INT8 (hassasiyet gerektiren katmanlar)
  ✓ FP16 (layer norm, embedding, output projeksiyon)
  ✓ Karma hassasiyet (per-layer)

Gelecek:
  ○ NF4 (Normal Float 4 — QLoRA)
  ○ Ternary + ANS (sıkıştırılmış, daha az alan)
  ○ Analog (FE+EO CIM gelince)
```

### 4.4 Çıkarım Özellikleri

```
Zorunlu:
  ✓ Greedy decoding
  ✓ Top-p (nucleus) örnekleme
  ✓ Temperature
  ✓ Tekrar cezası (repetition penalty)
  ✓ Bağlam penceresi: 4K minimum, 32K hedef

Önemli:
  ✓ Speculative decoding (küçük taslak model ile)
     → Gerçek hız 2-4× artabilir!
  ✓ Continuous batching (çok kullanıcı)
  ✓ Prompt caching (önek tekrarında hız)
  ✓ Function calling / tool use

İsteğe bağlı:
  ○ Multi-modal (görüntü + metin)
  ○ Embedding oluşturma
  ○ Reranking
```

---

## Bölüm 5 — Dikkat Edilmesi Gerekenler

### 5.1 Ternary Kuantizasyon Tuzakları

```
SORUN 1: Outlier ağırlıklar
  Birkaç çok büyük ağırlık tüm ölçek faktörünü etkiler
  → Geri kalan ağırlıkların ayrımı bozulur
  
  ÇÖZÜM: LLM.int8() tarzı outlier ayrımı
  Büyük ağırlıklar: FP16 (ayrı saklanan)
  Normal ağırlıklar: ternary
  %0.1 FP16, %99.9 ternary → doğruluk çok artar

SORUN 2: Dikkat katmanları hassas
  Q, K matrislerinde ternary → dikkat kalitesi düşer
  Özellikle uzun bağlamda belirgin
  
  ÇÖZÜM: Q, K → INT4 veya INT8 kullan (ternary değil)
  V, O → ternary (daha toleranslı)
  Bellek maliyeti: ~%15 artar ama kalite korunur

SORUN 3: İlk ve son katmanlar kritik
  Embedding katmanı → mutlaka FP16/INT8
  LM head (output) → mutlaka FP16
  İlk 2-3 ve son 2-3 katman → INT4 minimum
  
  GENEL KURAL: Katman ne kadar "sınırda" (giriş/çıkış)
               olursa o kadar hassas davran
```

### 5.2 Donanım Tasarım Tuzakları

```
SORUN 1: HBM4 + interposer termal yönetimi
  HBM yığını doğrudan güç dağılımı yapar
  Interposer üzerinde sıcak noktalar oluşabilir
  
  ÇÖZÜM: Termal simülasyon (ANSYS Icepak) erken aşamada
  HBM'leri compute die'dan uzağa koy
  Buhar odası HBM bölgesini de kaplamalı

SORUN 2: PCIe sinyal bütünlüğü
  PCIe 5.0 @ 32 GT/s: 
  Konnektör + kablo + trace uzunluğu kritik
  
  ÇÖZÜM: Kısa mesafe (< 5 cm) + düşük kayıplı PCB
  Thunderbolt alternatifi: zaten dahili kablo, sorun yok

SORUN 3: Güç dağıtımı
  150W + anlık zirve (burst): 200W+
  VRM (Voltage Regulator Module) kapasitesi kritik
  
  ÇÖZÜM: 12V girişte VRM, board üzerinde çok fazlı
  Kapasitör tamponu anlık zirveleri yumuşatır

SORUN 4: Saat dağıtımı
  3 GHz'de saat ağacı dağılımı kritik (skew < 10 ps)
  
  ÇÖZÜM: H-tree veya mesh saat dağıtımı
  PLL bütçesinin %10'u maximum skew
```

### 5.3 Yazılım Geliştirme Tuzakları

```
SORUN 1: ANS sıkıştırma/açma darboğazı
  Model yüklenirken 75 GB ANS açma işlemi
  CPU bound olabilir
  
  ÇÖZÜM: Donanım ANS dekoder (ASIC içine ekle)
  Alternatif: Çoklu çekirdek paralel açma
  Hedef: < 2 saniye model yükleme

SORUN 2: Mamba durum yönetimi
  Uzun konuşmada Mamba durumu birikir
  Farklı "konuşmalar" için ayrı durum gerekir
  
  ÇÖZÜM: Durum sözlüğü (session_id → state)
  Bellek: 400 MB/oturum → 10 eş zamanlı = 4 GB

SORUN 3: Speculative decoding entegrasyonu
  Taslak model (7B) ana modelden (400B) farklı cihazda
  
  ÇÖZÜM: 7B modeli host CPU/GPU'da çalıştır
  Token önerileri Thunderbolt üzerinden gel
  Kabul/ret 400B cihazda karar ver
  2-3× hız artışı mümkün

SORUN 4: Türkçe dahil çok dilli destek
  Tokenizer: BPE, SentencePiece, tiktoken
  Türkçe için: yeterli vocabulary gerekli
  
  ÇÖZÜM: Model seçiminde vocabulary büyüklüğüne dikkat
  LLaMA tokenizer: Türkçe için yetersiz (çok fazla split)
  Özel Türkçe tokenizer veya çok dilli model tercih et
```

### 5.4 Üretim ve Tedarik Zinciri

```
KRİTİK: TSMC CoWoS-L kapasitesi kısıtlı
  NVIDIA, AMD, Apple TSMC kapasitesini doluyor
  Küçük hacim (< 1000 adet) için yer bulmak zor
  
  ÇÖZÜM: Samsung Foundry (alternatif)
  2.5D packaging: Samsung FOWLP veya EMIB benzeri
  Ödün: biraz düşük performans, ama erişilebilir

HBM4 tedariki:
  SK Hynix, Samsung, Micron → sınırlı 2025'te
  Büyük müşteriler (NVIDIA) öncelikli
  
  ÇÖZÜM: Erken NDA + satın alma anlaşması
  Alternatif: HBM3e (şimdi mevcut, biraz daha az bant)
  HBM3e 2 yığın: 1,800 GB/s → 30 tok/s için yeterli
```

---

## Bölüm 6 — Performans Bütçesi: Adım Adım

### 6.1 Tek Token Üretim Süresi

```
Hedef: 1/30 saniye = 33 ms/token

Zaman bütçesi (33 ms = 33,000 μs):

[1] Ağırlık okuma (HBM4'ten):
    75 GB (sıkıştırılmamış) / 4,500 GB/s = 16.7 ms
    Çift tamponlama ile örtüşme: ~10 ms efektif
    
[2] Ternary hesaplama:
    200B efektif MAC / (12 TOPS) = 16.7 ms
    Ağırlık okuma ile örtüşme (pipeline): ~0 ms ekstra

[3] Mamba durum güncellemesi:
    200 MB SRAM okuma/yazma @ 10 TB/s on-chip: ~0.02 ms
    Neredeyse ücretsiz

[4] Attention (hybrid, %25 katman):
    Sadece 100 attention katmanı (400'ün %25'i)
    KV cache okuma (8 GB @ 4,500 GB/s): 1.8 ms

[5] Overhead (tokenizer, sampling, I/O):
    ~1-2 ms

TOPLAM: 10 + 0 + 0.02 + 1.8 + 1.5 = ~13 ms
Güvenlik payıyla: ~20 ms → 50 tok/s potansiyeli!

30 tok/s: rahat karşılanıyor ✓
50 tok/s: mümkün (iyi tasarımla) ✓
```

### 6.2 Bant Genişliği Kullanım Analizi

```
Token başına HBM4 erişimi:

Ağırlık okuma:
  Sıkıştırılmış (ANS): 75 GB
  Sıkıştırma oranı: 75 / 100 = %75
  Ham ağırlık: 100 GB / token okunur
  
  UYARI: Her token TÜM ağırlıklar okunuyor
  Bu doğru — LLM çıkarımının temel özelliği
  Bant genişliği = model boyutu × hız

Bant genişliği kullanımı:
  100 GB × 30 tok/s = 3,000 GB/s (ham)
  Mevcut: 4,500 GB/s
  Kullanım: %67 → iyi (tüm bant değil, margin var)

KV cache:
  8 GB @ 30 tok/s: 240 GB/s
  Mevcut bant genişliğinin %5'i → önemsiz
```

---

## Bölüm 7 — Zaman Çizelgesi ve Geliştirme Aşamaları

### 7.1 Gerçekçi Geliştirme Planı

```
AŞAMA 0 — Araştırma ve Prototipleme (6 ay):
  Yazılım: Ternary çıkarım motoru (FPGA üzerinde)
  Hedef: 400B model doğruluğunu doğrula
  Çıktı: Ternary kalibrasyon metodolojisi
  Ekip: 3-4 kişi (ML + yazılım)

AŞAMA 1 — FPGA Prototipi (12 ay):
  Donanım: AMD Alveo U250 veya Xilinx VCU1525
  HBM2e: 32 GB (kısıtlı ama prototip için)
  Hedef: 70B model @ 10 tok/s (ölçeklenebilirlik testi)
  Çıktı: RTL kod tabanı, yazılım yığını
  Ekip: +2 donanım mühendisi (RTL/FPGA)
  
  NEDEN FPGA ÖNCE:
  ASIC mask seti: $5M-15M (bir denemede yapamazsın)
  FPGA: $50K-200K, tekrar programlanabilir
  Tasarım hatalarını FPGA'da bul, ASIC'te düzelme

AŞAMA 2 — ASIC Tape-out (18-24 ay):
  RTL → GDS II: TSMC 3nm
  CoWoS-L interposer: HBM4 × 3
  Mask seti: ~$10-15M (3nm için)
  Teslim: 4-6 ay sonra ilk silikon
  İlk silikon başarısız olabilir (spin gerekebilir)
  Ekip: +4-6 ASIC mühendisi

AŞAMA 3 — Ürün (6-12 ay sonra):
  Silikon doğrulama + yazılım olgunlaştırma
  Paketleme (Mac Studio tarzı kutu)
  Beta kullanıcıları
  
TOPLAM: 3-4 yıl, $20-50M yatırım

BÜTÇE DAĞILIMI (kabaca):
  ASIC mask + üretim: %40
  Ekip (4 yıl): %35
  FPGA + ekipman: %10
  Paketleme + sertifikasyon: %10
  Beklenmedik: %5
```

### 7.2 Kısa Vadeli Alternatif (1 yıl, ~$2M)

```
"TODAY'S BEST" yaklaşımı (ASIC olmadan):

Mevcut çip + özel paketleme:
  AMD Instinct MI300X: 192 GB HBM3, 5.2 TB/s
  Ya da: 2× H100 NVLink bridge
  
  Boyut: masaüstü/sunucu kutu (~30cm × 30cm × 20cm)
  Güç: 300-700W (Mac Studio değil ama çalışıyor)
  
  70B model @ 60 tok/s: ✓ (bugün mümkün)
  400B model @ 15 tok/s: ✓ (INT4 ile)
  400B model @ 30 tok/s: ⚠️ (sıkıştırma + optimizasyon ile)

  Yazılım yatırımı daha değerli:
  Çıkarım motorunu geliştir, model optimizasyonunu öğren
  ASIC tasarımı için hazırlık yap
```

---

## Bölüm 8 — Tam Teknik Spesifikasyon Tablosu

```
KATEGORİ             ÖZELLİK                 DEĞER
─────────────────────────────────────────────────────────────

DONANIM
Ana çip              Süreç                  TSMC 3nm
                     Çip alanı              600-800 mm²
                     Güç                    120-150W TDP
                     Compute                12 TOPS efektif (ternary)
                     On-chip SRAM           512 MB
                     
Bellek               Teknoloji              HBM4
                     Yığın sayısı           3
                     Toplam kapasite        144 GB
                     Toplam bant genişliği  4,500 GB/s
                     
Paketleme            Teknoloji              CoWoS-L
                     Boyut                  130mm × 100mm
                     
Soğutma             Yöntem                 Vapor chamber + 2× 120mm fan
                     TDP kapasitesi         200W
                     Gürültü                ≤35 dB(A) tam yükte
                     
Boyut               Kasa                   200mm × 200mm × 95mm (Mac Studio)
Ağırlık                                    ~2.5 kg

BAĞLANTI
Host               Thunderbolt 5            2 port (15 GB/s efektif)
                   PCIe 5.0 × 8            (opsiyonel, masaüstü için)
Güç               DC giriş                 19.5V @ 10A = 195W adaptör

YAZILIM
Çıkarım motoru    Dil                     Rust
                  API                     Python (PyO3), REST (OpenAI uyumlu)
                  
Model desteği     Format                  SafeTensors, GGUF, ONNX
                  Mimari                  Transformer, MoE, Mamba, Hybrid
                  Maksimum parametre      500B (tasarım hedefi)
                  
Kuantizasyon      Desteklenen             Ternary, INT4, INT8, FP16, karma
                  Varsayılan              Ternary + ANS sıkıştırma
                  
Çıkarım           Bağlam penceresi        32K (hybrid ile; 4K tam attention)
                  Batch                   1-8 (sürekli batching)
                  Speculative decoding    Evet (host CPU taslak model ile)

PERFORMANS (400B Ternary Hybrid)
Token hızı                               ≥30 tok/s (hedef: 50 tok/s)
İlk token gecikmesi                      <500 ms (4K prompt)
Model yükleme süresi                     <10 saniye
Bellek kullanımı (400B)                  ~90 GB

GÜÇ
Boşta                                   25W
Ortalama yükte                          120W
Maksimum                                200W
```

---

## Özet: En Kritik Kararlar

```
1. HBM4 seçimi — pazarlık edilemez:
   LPDDR6 veya GDDR7 ile 30 tok/s 400B mümkün değil.
   HBM4'ün bant genişliği bu cihazın temel taşı.

2. Ternary kuantizasyon + Hybrid SSM — birlikte:
   Sadece ternary: 30 tok/s ile KV cache sorunu biter
   Sadece hybrid: bant genişliği yeterli olmaz
   İkisi birlikte: hem bant genişliği hem KV cache çözülür

3. FPGA önce, ASIC sonra:
   $10M+ mask setini "ilk denemede doğru" yapmak imkânsız.
   FPGA protipi hem tasarımı doğrular hem yazılımı geliştirir.

4. Speculative decoding — ücretsiz 2-3× hız:
   7B taslak model host CPU'da, 400B donanımda.
   Thunderbolt bant genişliği bu koordinasyona yeterli.
   Ek donanım maliyeti yok, yazılım görevi.

5. Doğruluk vs alan ödünleşimi erkenden karar ver:
   Ternary, 400B modelde %1-3 doğruluk kaybı yaşatır.
   Bu kullanıcıya kabul ettirilebilir mi?
   Değilse INT4 gerekir → 200 GB → 3-4 HBM4 yığını → daha büyük/pahalı.

2025-2026 için gerçekçi beklenti:
  Mac Studio boyutu: 70B @ 60 tok/s ✓ (mümkün, FPGA+HBM3e ile)
  Mac Studio boyutu: 400B @ 30 tok/s ✓ (ASIC+HBM4 gerekli, 2027-2028)
```
