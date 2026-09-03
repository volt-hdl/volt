> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# SSM ve Hibrit Bağlam Mimarisi — Sıfırdan Açıklama

> Transformer'ın KV cache problemi 400B modelin taşınabilir SoC'ta
> çalışmamasının kalıcı engellerinden biri. SSM bu engeli mimari
> düzeyde çözüyor: bağlam ne kadar uzun olursa olsun sabit boyutlu
> bellek kullanıyor. Ama bedeli var. Hibrit mimari iki dünyanın
> en iyisini birleştiriyor.

---

## Bölüm 1 — Sorun: Transformer Neden Sonsuz Bellek İstiyor

### 1.1 Dikkat Mekanizması (Attention) Nedir

```
Transformer'da her yeni kelime üretilirken şu soru sorulur:
"Önceki hangi kelimeler bu yeni kelimeyle ilgili?"

Örnek: "Fransa'nın başkenti ________"
  "Fransa" → çok ilgili
  "'nın"   → az ilgili
  "başkenti" → ilgili
  
Her önceki kelimeye "dikkat ağırlığı" atanır.
Bu ağırlıklar hesaplanırken K (Anahtar) ve V (Değer) vektörleri kullanılır.
```

### 1.2 KV Cache Neden Büyüyor

```
Her token üretiminde:
  Tüm önceki tokenların K ve V vektörleri gerekli
  → Hafızada tutulması şart (her seferinde yeniden hesaplamak çok pahalı)

Bu "KV Cache":
  Token 1:  K₁, V₁
  Token 2:  K₁, V₁, K₂, V₂
  Token 100: K₁, V₁, K₂, V₂, ..., K₁₀₀, V₁₀₀
  Token N:  N × (K + V) vektörü

  N arttıkça cache doğrusal büyüyor: O(N)

400B model, 128K bağlam:
  128,000 token × 8 MB/token = 1 TB  ← imkânsız!
```

### 1.3 Sorunun Özü

```
Transformer mantığı: "Şu ana kadar her şeyi hatırla ve her şeye bak"
  
  Her yeni kelime için: TÜM geçmiş yeniden okunuyor
  Bağlam 10× uzarsa: bellek 10× büyüyor, hesaplama 100× artıyor

  Kütüphane analojisi:
  "1 milyon kitaptan hangisi bu soruyu yanıtlar?"
  Transformer: her sorudan önce 1 milyon kitabın tamamını tara
  → Kitap sayısı arttıkça süre çarpımsal artıyor
```

---

## Bölüm 2 — SSM Nedir: Kontrol Teorisinden Gelen Fikir

### 2.1 Durum Uzayı Modeli (State Space Model)

SSM aslında 1960'lardan gelen bir kontrol teorisi kavramı. Kalman filtresi de bir SSM.

**Temel fikir:** Sistemin tüm geçmişi yerine, geçmişin **sıkıştırılmış özeti**ni tut.

```
İnsan hafızası analojisi:

Transformer gibi hafıza:
  "Dün sabah 7:02'de kahve makinesinin düğmesine bastım,
   7:03'te su ısındı, 7:04'te fincanı aldım..."
  → Her detay kaydedilmiş, hafıza sonsuza büyüyor

SSM gibi hafıza:
  "Kahve içtim, iyi hissediyorum, bugün toplantı var"
  → Özet çıkarıldı, sabit boyutlu bilgi
  → Yeni gün bilgisi eklenince güncelleniyor
```

### 2.2 Matematiksel Mekanizma

```
SSM'in çalışması (basitleştirilmiş):

DURUM (h): sistemin "özet hafızası" — sabit boyutlu vektör
           örn: 64 boyutlu vektör

Giriş (x): yeni token geldi
           örn: "Paris" kelimesi

Güncelleme:
  h_yeni = A × h_eski + B × x_yeni
  
  A: "eski hafızadan ne kadar tut?" matris
  B: "yeni bilgiyi nasıl ekle?" matris

Çıkış (y):
  y = C × h_yeni + D × x_yeni
  
  C: "hafızadan ne çıkar?" matris
  D: "doğrudan geçiş"

Kritik nokta:
  h her zaman aynı boyutta (64) → sabit!
  1 token sonra: h = 64 boyut
  1000 token sonra: h = 64 boyut (hâlâ!)
  1,000,000 token sonra: h = 64 boyut (hâlâ!)
  
  → Sonsuz bağlam, sabit bellek!
```

### 2.3 Sezgisel Karşılaştırma

```
                Transformer        SSM
Hafıza türü     Her şeyi kaydet    Özet tut
Bellek boyutu   O(N) — büyüyor     O(1) — sabit!
Hesaplama       O(N²) — çarpımsal  O(N) — doğrusal
Kesinlik        Mükemmel (her şey) Sıkıştırılmış (özet)
"Geri bak"      Kolayca            Zor (silinmiş olabilir)
```

---

## Bölüm 3 — Mamba: Modern SSM'in Atılımı

### 3.1 Klasik SSM'in Sorunu

```
Eski SSM'ler (S4, 2021):
  A, B, C matrisleri SABİT (inputa bağlı değil)
  
  Problem: Her token için aynı "unutma oranı"
  "Türkiye" de "ve" de aynı şekilde işleniyor
  → Önemli bilgiyi seçemez, her şeyi eşit "sıkıştırır"
  → Dikkat gerektiren görevlerde zayıf
```

### 3.2 Mamba'nın Seçici Mekanizması

```
Gu & Dao (2023) - Mamba:

  A, B, C matrisleri INPUTA BAĞLI!
  
  Her yeni token gelince:
  "Bu token hafızamı ne kadar etkilesin?"
  "Eski bilginin ne kadarını tutalım?"
  kararları O TOKEN'A GÖRE VERİLİR

Örnek:
  "Fransa'nın başkenti Paris'tir. Çin'in başkenti Beijing'dir.
   Japonya'nın başkenti ______"
  
  Transformer: "Paris" ve "Beijing"e bak, Japonya ile ilişkisine bak
  Mamba: "başkent" bilgisi önemliydi → hafızaya güçlü kodlandı
         "tır" gibi kelimeler → zayıf kodlandı, etkili biçimde silindi
  
  Seçicilik: neyin önemli, neyin değersiz olduğunu öğreniyor
```

### 3.3 Mamba'nın Performansı

```
Benchmark karşılaştırması (yaklaşık değerler):

Görev              Transformer   Mamba
──────────────────────────────────────────
Dil modelleme       ✓✓✓          ✓✓✓  (eşit!)
In-context öğrenme  ✓✓✓          ✓✓   (transformer iyi)
Kesin geri çağırma  ✓✓✓          ✓    (transformer çok iyi)
Uzun bağlam anlama  ✓✓           ✓✓✓  (Mamba iyi!)
Bellek kullanımı    ✗✗✗ (büyür)  ✓✓✓  (sabit)
Çıkarım hızı       orta         hızlı

Mamba iyi ama "iğneyi demet samanın içinde bul" 
türü görevlerde transformer hâlâ öne geçiyor.
```

---

## Bölüm 4 — SSM Çıkarımı Neden Gerçekten O(1) Bellek

### 4.1 Adım Adım Gösterim

```
400B Mamba modeli, 1 milyon token bağlam:

  Token 1 geldi:
    h_1 = A × h_0 + B × x_1   → h boyutu: 64×4096 = 262,144 değer
    
  Token 2 geldi:
    h_2 = A × h_1 + B × x_2   → h boyutu: hâlâ 262,144 değer
    (h_1 artık gerekmiyor, atıldı!)
    
  Token 1,000,000 geldi:
    h_1M = A × h_999999 + B × x_1M
    → h boyutu: hâlâ 262,144 değer
    (yalnızca bir önceki durum tutuldu, geri kalan atıldı)

Bellek kullanımı:
  Transformer: 1M token × 8 MB = 8 TB  ← imkânsız
  Mamba:       262,144 × 4 byte = ~1 MB ← her zaman sabit!
```

### 4.2 Eğitim Nasıl Paralel Olabiliyor

```
Bir çelişki var gibi görünüyor:
  Çıkarımda: sıralı (önceki duruma bak, sonrakini hesapla)
  Eğitimde:  paralel (tüm sequence aynı anda işlemek istiyoruz)

  Transformer eğitim paralelliği: kolay (tüm tokenlar birbirini görüyor)
  Mamba eğitim paralelliği: "Parallel Scan" algoritması ile mümkün!

Paralel Scan:
  [a₁, a₂, a₃, a₄] → [a₁, a₁a₂, a₁a₂a₃, a₁a₂a₃a₄]
  (önekler çarpımı — paralel hesaplanabilir!)
  
  O(N log N) karmaşıklık → transformer O(N²)'den daha iyi!
  → Eğitim hızı: Mamba kazanıyor
  → Çıkarım hafızası: Mamba kazanıyor
```

---

## Bölüm 5 — Hibrit Mimari: İki Dünyanın En İyisi

### 5.1 Neden Hibrit

```
Saf Transformer:
  ✓ Her şeyi hatırlar (mükemmel geri çağırma)
  ✗ Sonsuz bellek gerekir
  ✗ Uzun bağlamda hesaplama patlar

Saf Mamba:
  ✓ Sabit bellek (sonsuz bağlam)
  ✓ Hızlı çıkarım
  ✗ "İğneyi samanın içinde bul" gibi görevlerde zayıf
  ✗ In-context öğrenme daha zayıf

Hibrit:
  Transformer katmanları → kesin geri çağırma gereken yerler
  Mamba katmanları → uzun bağlam sıkıştırma
  İkisinin birlikte kullanımı her ikisinin zayıflıklarını örtüyor
```

### 5.2 Hibrit Mimarinin Yapısı

```
Hibrit model katman sırası (örnek Jamba tarzı):

Katman 1:  [Mamba] → uzun bağlam sıkıştırma
Katman 2:  [Mamba] → devam
Katman 3:  [Mamba] → devam
Katman 4:  [Attention] ← bağımlılık yakalamak için
Katman 5:  [Mamba]
Katman 6:  [Mamba]
Katman 7:  [Mamba]
Katman 8:  [Attention] ←
...

Tipik oran: 3-7 Mamba katmanı başına 1 Attention katmanı

Neden bu oran:
  Attention: kesin bilgi erişimi ama pahalı (KV cache)
  Mamba: sıkıştırma ama ucuz (sabit durum)
  Nadir attention → KV cache küçük kalır
  Çok Mamba → uzun bağlam mümkün
```

### 5.3 Gerçek Hibrit Modeller

```
Jamba (AI21 Labs, Mart 2024):
  52B toplam parametre (12B aktif, MoE)
  Mamba + Transformer karışımı
  256K bağlam — TEK GPU'da!
  (Saf Llama 70B: 256K için 8 GPU gerekir)
  Hız: Llama 2 70B'nin 3×'i
  Kalite: Mixtral 8×7B ile rekabet ediyor

Griffin (Google DeepMind, Şubat 2024):
  Gated Linear Recurrence + yerel Attention
  2B ve 14B boyutları
  Gemma 2B ile karşılaştırılabilir kalite

Falcon Mamba (Technology Innovation Institute, 2024):
  7B — saf Mamba (attention yok!)
  Llama 3 8B ile karşılaştırılabilir performans
  
RWKV (Peng Bo vd.):
  "Attention-free Transformer"
  Linear attention = SSM analog
  7B-14B boyutları, topluluk tarafından geliştiriliyor
```

---

## Bölüm 6 — 400B Model İçin Somut Kazanım

### 6.1 KV Cache Karşılaştırması

```
128K token bağlam, 400B model:

Saf Transformer:
  Her katman KV cache
  128,000 × 8 MB = 1 TB  ← imkânsız

Hibrit (75% Mamba, 25% Attention):
  Mamba katmanları: sabit durum ~1 MB/katman
  Attention katmanları: KV cache
  
  Attention katman sayısı: 128 katman × %25 = 32 katman
  32 katman × 128K token × KV boyutu:
  32 × 128,000 × 512KB = 2 TB / 32 = 64 GB
  
  (64 GB: hâlâ büyük ama 1 TB'nin 16'da biri!)

Saf Mamba:
  Tüm durum: 400 katman × 1 MB = 400 MB  ← çok küçük!
  KV cache: 0 GB
  Dezavantaj: kesin geri çağırma zayıf
```

### 6.2 Hız Karşılaştırması

```
128K bağlam için token başına süre (yaklaşık):

Saf Transformer:
  Her token: 128,000 token × 400B hesaplama = astronomik
  → Pratikte 128K mümkün değil (hafıza + hesaplama)

Hibrit (%25 Attention, %75 Mamba):
  Attention katmanları: 32 katman × 128K = 4M işlem
  Mamba katmanları: 96 katman × sabit = 96K işlem
  → ~20-40× daha hızlı
  → 128K bağlam artık pratik!

Saf Mamba:
  Tüm katmanlar sabit işlem: çok hızlı
  → En hızlı, ama kalite ödünü var
```

### 6.3 FE+EO CIM + Hibrit = Mükemmel Eşleşme

```
İki teknoloji birbirini mükemmel tamamlıyor:

FE+EO CIM:
  MODEL AĞIRLIKLARI için ideal
  → Bir kez yaz, milyar kez oku
  → Non-volatile, bant genişliği yok
  → Endurance: 10⁸ döngü (az yazma iyidir)

Hibrit SSM:
  KV CACHE sorununu çözüyor
  → Attention'ı azalt → KV cache küçük
  → Mamba'nın durumu: küçük, hızlı SRAM'da tutulabilir

Birlikte 400B model taşınabilir SoC:
  Ağırlıklar (100 GB ternary): FE+EO CIM'de
  Mamba durumu (~400 MB): hızlı SRAM'da
  Attention KV cache (küçük, hibrit): LPDDR5X'de
  
  Toplam aktif bellek: ~10-20 GB (yönetilebilir!)
  Güç: FE+EO kısmı ~4W + SRAM ~1W + DRAM ~2W ≈ 7W
  → Pasif soğutma + Thunderbolt SoC = MÜMKÜN!
```

---

## Bölüm 7 — Volt Ekosistemiyle Bağlantı

### 7.1 SSM Donanım Gereksinimleri

```
Mamba'nın çalışması için gereken donanım:

1. Matris çarpımı (A × h):
   Durum: 64×4096 matris
   Her token: 64×4096 × 4096×1 = 262,144 MAC
   → Küçük! FE+EO CIM burada da kullanılabilir

2. Durum güncellemesi (h_yeni = A×h_eski + B×x):
   h sürekli yazılıyor (her token)
   FE+EO endurance: 10⁸ döngü (yeterli ama dikkatli kullanım)
   Alternatif: SRAM için durum, FE+EO için ağırlıklar

3. Seçici tarama (Parallel Scan):
   Eğitimde gerekli, çıkarımda değil
   → Eğitim: GPU (standart), Çıkarım: SoC

Volt'ta SSM modülü:
```

```volt
// Mamba SSM bloğu — Volt'ta nasıl ifade edilir
module MambaBlock<const D: usize, const N: usize> {
    // D: model boyutu, N: durum boyutu (64 tipik)

    in  x       : Vector<i8, D>      // giriş token (INT8)
    in  h_prev  : Matrix<f16, N, D>  // önceki durum (FP16)
    out y       : Vector<i8, D>      // çıkış
    out h_next  : Matrix<f16, N, D>  // yeni durum

    // Seçici parametreler (inputa bağlı!)
    let delta = linear(x, W_delta)   // ne kadar güncelle
    let B_sel = linear(x, W_B)       // giriş projeksiyonu
    let C_sel = linear(x, W_C)       // çıkış projeksiyonu

    // Durum güncellemesi (sürekli zaman diskretizasyonu)
    let A_bar = exp(-exp(A) * delta)  // ayrık A
    let B_bar = delta * B_sel        // ayrık B

    h_next = A_bar * h_prev + B_bar * x  // SSM adımı
    y      = C_sel * h_next + D_skip * x  // çıkış

    // h boyutu sabit: N×D = 64×4096 = 262,144 değer
    // Token sayısından bağımsız!
    invariant: sizeof(h_next) == N * D * sizeof(f16)
    #[context_independent_memory]  // sonsuz bağlam, sabit bellek
}

// Hibrit Transformer + Mamba katmanları
module HybridBlock<const D: usize, const Ratio: u8> {
    // Ratio: Mamba/Attention oranı (örn. 3 = 3 Mamba + 1 Attention)

    in  x       : Vector<i8, D>
    in  h_prev  : Matrix<f16, 64, D>   // Mamba durumu
    in  kv_cache: Option<KVCache>       // Attention cache (opsiyonel)
    out y       : Vector<i8, D>
    out h_next  : Matrix<f16, 64, D>

    // Mamba veya Attention seç (katmana göre)
    match layer_type(Ratio) {
        LayerType::Mamba     => MambaBlock<D, 64>(x, h_prev)
        LayerType::Attention => AttentionBlock<D>(x, kv_cache)
    }

    // Mamba katmanında: KV cache büyümez
    // Attention katmanında: küçük KV cache
    @memory_analysis:
        mamba_state = 64 * D * sizeof(f16)  // sabit
        attention_kv = context_len * D * sizeof(f16)  // büyüyor ama az
}
```

---

## Bölüm 8 — Gelecek Mimariler

### 8.1 Tamamen Farklı Yaklaşımlar

```
SSM/Mamba dışında da O(1) bellek araştırmaları:

Linear Attention:
  Standart attention: O(N²) hesaplama, O(N) bellek
  Linear attention: O(N) hesaplama, O(D²) bellek (sabit!)
  Nasıl: çekirdek yaklaşımı (kernel approximation)
  Kalite: standart attention'dan biraz düşük

RetNet (Microsoft, 2023):
  "Retention": SSM benzeri ama farklı formülasyon
  Eğitim: parallel mod (transformer gibi)
  Çıkarım: recurrent mod (SSM gibi)
  O(1) çıkarım belleği

HGRN (Hierarchical Gated Recurrent Network):
  Hiyerarşik gated recurrence
  Farklı katamanlar farklı "zaman ölçeği"nde çalışıyor
  Kısa dönem → ince detay, Uzun dönem → büyük örüntü

xLSTM (2024):
  LSTM'in modern versiyonu
  Uzun kısa süreli bellek → modern ölçeğe getirildi
  Mamba ile rekabet ediyor
```

### 8.2 Biyolojik Beyin ile Bağlantı

```
Nöromorfik hesaplama tartışmasına geri dönüş:

İnsan beyni nasıl çalışıyor?
  Sınırsız bağlam: hayatın tamamını "hatırlıyor"
  Ama sabit bellek: 86 milyar nöron, sabit sayı

Beyin de aslında SSM benzeri:
  Kısa süreli hafıza (working memory): dikkat katmanı gibi
  Uzun süreli hafıza (episodic): SSM gibi sıkıştırılmış
  Hipokampus: yeniden konsolidasyon — SSM ↔ attention köprüsü

Mamba'nın seçici mekanizması:
  "Neyin önemli, neyin önemsiz olduğuna karar ver"
  → Biyolojik dikkat (attention) ile örtüşüyor
  → Nöromorfik donanım ile doğal eşleşme!

Nöromorfik + SSM + FE+EO CIM:
  SSM durumu → nöron membran potansiyeli gibi
  Seçici gate → nöromorfik ateşleme eşiği gibi
  FE+EO ağırlık → sinaptik plastisity gibi

Bu üç paradigma birbirine yakınsıyor —
tesadüf değil, beyin bu problemi çoktan "çözmüş".
```

---

## Özet

```
Sorun:
  Transformer, bağlam büyüdükçe bellek büyütüyor (O(N))
  128K token → 1 TB KV cache → SoC'a sığmıyor

SSM çözümü:
  Geçmişi sıkıştırılmış sabit boyutlu "durum" olarak tut
  N ne kadar büyük olursa olsun: bellek sabit (O(1))
  Ama: kesin "geri bak" zayıflar

Mamba inovasyonu:
  Seçici SSM: "neyi hatırla, neyi unut" inputa bağlı
  Transformer kalitesine yaklaşıyor ama O(1) bellekle

Hibrit mimari:
  %75 Mamba (uzun bağlam) + %25 Attention (kesin erişim)
  KV cache: 1 TB → 64 GB (16× küçülme)
  Kalite: transformer'a yakın, maliyet: dramatik düşük

400B + FE+EO CIM + Hibrit:
  Ağırlıklar (100 GB): FE+EO'da, non-volatile, bant yok
  Mamba durumu (400 MB): hızlı SRAM'da
  KV cache (64 GB, hibrit): LPDDR5X'de
  
  Toplam güç: ~7W → pasif soğutma → Thunderbolt SoC
  
  Bu "2032+ gerçekçi hedef" tablo:
  400B model + sonsuz bağlam + taşınabilir SoC = MÜMKÜN
```
