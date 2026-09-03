> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# WDM Kanal Sayısı ve 3D Entegrasyon — Kapsamlı Açıklama

> 400B model hesabında ortaya çıkan iki kritik kavram: WDM kaç
> "paralel yol" olduğunu, 3D entegrasyon kaç "kat" olduğunu belirler.
> İkisi birlikte fotonik CIM'in yoğunluk sorununu çözen mekanizmadır.

---

## Bölüm 1 — WDM: Dalga Boyu Bölmeli Çoğullama

### 1.1 Sezgisel Başlangıç

```
Elektrik kablosu ile fiber optik karşılaştırması:

Elektrik kablosu:
  ─────────────────────────────────── (bir iletken)
  Bir sinyal taşır, diğeri geçemez

Fiber optik / dalga kılavuzu:
  ─────────────────────────────────── (bir dalga kılavuzu)
  λ₁ (kırmızı) ──────────────────────►
  λ₂ (turuncu) ──────────────────────►
  λ₃ (sarı)    ──────────────────────►  aynı anda, bağımsız!
  λ₄ (yeşil)   ──────────────────────►
  ...
  λ₆₄ (mor)    ──────────────────────►

  Neden mümkün: farklı renk ışıklar birbirini "görmez"
  Fizik: fotonlar arasında etkileşim yok (bozon özelliği)
  → Tek dalga kılavuzu = 64 bağımsız veri yolu
```

### 1.2 Dalga Boyu Aralığı ve Kanal Sayısı

```
Telekomünikasyondaki WDM standartları:

C-band (mevcut en yaygın):
  Aralık: 1530 nm – 1565 nm = 35 nm genişlik
  100 GHz aralık (≈0.8 nm): 44 kanal
  50 GHz aralık (≈0.4 nm):  88 kanal
  25 GHz aralık (≈0.2 nm): 175 kanal

L-band eklenirse (+35 nm):
  25 GHz: 350 kanal

O+C+L+U band (geniş pencere LiNbO₃):
  1260–1625 nm = 365 nm
  25 GHz aralık: ~1800 kanal (teorik)
  Pratik hedef 2030: 512 kanal

Silikon fotonik için pratik kısıt:
  AWG (Arrayed Waveguide Grating) boyutu büyür
  Sıcaklık hassasiyeti: kanal kayması yaşanır
  Bugün ticari: 16-128 kanal
  Araştırma: 512+ kanal
```

### 1.3 WDM Bileşenleri

**Multiplexer (MUX) — Birleştirici:**

```
λ₁ ──────────────────────────────┐
λ₂ ─────────────────────────┐   │
λ₃ ─────────────────────┐   │   │     ┌─────────────────────────
λ₄ ─────────────────┐   │   │   │     │
...                  └───┴───┴───┴─────► tek dalga kılavuzu
                         AWG / Ring    → hepsi bir arada
                         Resonatörler

AWG (Arrayed Waveguide Grating):
  ┌──────────────────────────────┐
  │  Giriş ─► dalga kılavuzu    │
  │           dizisi (farklı    │
  │           uzunlukta)        │
  │           ─► çıkış          │
  └──────────────────────────────┘
  Her dalga kılavuzu farklı gecikme → farklı λ seçilir
  Boyut: 100 μm – 1 mm² (kanal sayısına göre)
```

**Demultiplexer (DEMUX) — Ayırıcı:**

```
Tek dalga kılavuzu ─►  ┌─────┐  ─► λ₁
                        │DEMUX│  ─► λ₂
                        │(AWG)│  ─► λ₃
                        └─────┘  ─► ...

AWG'nin tersi → her λ kendi dalga kılavuzuna ayrılır
```

**Ring Resonatör (Kompakt Kanal Seçici):**

```
Ana dalga kılavuzu:
─────────────────────────────────────────────────
         ╭───────────╮
         │   Halka   │  ← yalnızca λ_res'e rezonans
         │ (resonatör│    Bu dalga boyunu çıkarır
         ╰───────────╯    veya ekler
─────────────────────────────────────────────────
                               │
                           yan kanal: λ_res burada

Boyut: çap ~5-20 μm (AWG'den çok küçük!)
Dezavantaj: sıcaklığa hassas (±1°C → ±10 GHz kayma)
Çözüm: her halka için mikro-ısıtıcı kontrol (enerji maliyeti)
```

**Lazer Kaynakları:**

```
Seçenek 1 — DFB Lazer Dizisi:
  Her kanal için ayrı lazer
  λ₁: lazer 1, λ₂: lazer 2, ...
  Avantaj: bağımsız kontrol, güvenilir
  Dezavantaj: N lazer → büyük alan, yüksek maliyet

Seçenek 2 — Optik Frekans Tarayı (Comb Laser):
  Tek lazer → çok sayıda eşit aralıklı dalga boyu
  Mikro-resonatör Kerr tarayı:
  Bir lazer → 100+ bağımsız kanal → mucizevi!
  
  ┌──────────────────────────┐
  │  Pompa lazeri (1550 nm) │
  │         ↓               │
  │  Mikro-ring resonatör   │
  │  (Kerr etkisi + SRS)   │
  │         ↓               │
  │  1549.2, 1550.0, 1550.8 │  ← eşit aralıklı taray
  │  1551.6, 1552.4, ...    │
  └──────────────────────────┘
  
  Bu yaklaşım CIM için devrimsel:
  Tek lazer → 128+ paralel kanal
  Maliyet ve alan dramatik azalır
```

### 1.4 WDM'in CIM'e Katkısı — Somut Hesap

```
WDM olmadan:
  Her FE+EO hücre: kendi fiziksel konumu gerekir
  400B hücre × 0.64 μm² = 256 m²  (imkânsız)

N=64 WDM ile:
  Aynı dalga kılavuzu konumunda 64 bağımsız hücre
  (her dalga boyunda farklı ağırlık)
  Alan: 256 m² / 64 = 4 m²  (hâlâ büyük)

N=512 WDM ile:
  Aynı konumda 512 hücre
  Alan: 256 m² / 512 = 0.5 m²  (iyileşiyor)

Fiziksel anlam:
  Tek bir dalga kılavuzu noktası:
  
  ┌──────────────────────────────┐
  │ FE hücre @λ₁ → ağırlık w₁  │
  │ FE hücre @λ₂ → ağırlık w₂  │ ← aynı fiziksel konum
  │ FE hücre @λ₃ → ağırlık w₃  │   farklı ağırlıklar
  │ ...                          │   farklı kanallarda
  │ FE hücre @λ₅₁₂ → ağırlık w₅₁₂│
  └──────────────────────────────┘
  
  512 ağırlık → tek fiziksel FE+EO hücre noktası
  WDM her kanalı bağımsız tutar → etkileşim yok
```

---

## Bölüm 2 — 3D Entegrasyon: Dikey Yığın

### 2.1 Sezgisel Başlangıç

```
Şehir büyümesi analojisi:

Yatay büyüme:                Dikey büyüme (3D):
  ┌──┐┌──┐┌──┐┌──┐            ┌──┐
  │  ││  ││  ││  │            │  │ kat 4
  └──┘└──┘└──┘└──┘            ├──┤
  ← arazi büyüdü →            │  │ kat 3
                              ├──┤
  Çip: wafer büyüdü           │  │ kat 2
  Maliyeti çok artar          ├──┤
  Bağlantılar uzar            │  │ kat 1
                              └──┘
                              ← aynı arazi →

Aynı yüzey alanı, daha fazla bilgi
```

### 2.2 3D Entegrasyon Türleri

**Tür 1 — 2.5D (Yatay, İnterposer Tabanlı):**

```
Bugünkü GPU paketi (NVIDIA H100):

┌──────────────────────────────────────────────┐
│              Paket (PCB üstü)                 │
│  ┌─────────┐     ┌──────────┐                │
│  │  GPU    │     │ HBM3     │                │
│  │  (çip)  │     │ bellek   │                │
│  └────┬────┘     └────┬─────┘                │
│       │Silicon İnterposer│                   │
│  ┌────┴──────────────┴───────────────────┐   │
│  │  Silikon İnterposer (2.5D)            │   │
│  │  Kısa bağlantılar (μm → mm ölçeği)   │   │
│  └───────────────────────────────────────┘   │
└──────────────────────────────────────────────┘

Özellik: çipler yan yana, interposer üzerinde
Bağlantı uzunluğu: ~mm (PCB'den kısa ama TSV değil)
Kullanım: HBM GPU entegrasyonu, mevcut teknoloji
```

**Tür 2 — 3D Yığın (Dikey, TSV Tabanlı):**

```
HBM (High Bandwidth Memory) yapısı:

     ┌─────────────┐
     │ DRAM Kat 8  │
     ├─────────────┤
     │ DRAM Kat 7  │
     ├─────────────┤
     │ DRAM Kat 6  │
     ├─────────────┤
     │ DRAM Kat 5  │  ← 8 DRAM katı üst üste
     ├─────────────┤
     │ DRAM Kat 4  │
     ├─────────────┤
     │ DRAM Kat 3  │
     ├─────────────┤
     │ DRAM Kat 2  │
     ├─────────────┤
     │ DRAM Kat 1  │
     ├─────────────┤
     │ Mantık Kat  │ ← kontrol devresi
     └─────────────┘
            │
          TSV (Through-Silicon Via)
          Dikey bağlantı, ~10μm çaplı

TSV nedir:
  Silikon wafer'ı delen dikey metal tünel
  Üst kat → alt kat sinyal geçirir
  Çap: 5-50 μm, adım: 50-200 μm
  Elektriksel: düşük gecikme (mm yoldan kısa)
```

**Tür 3 — Doğrudan Bakır Bağlantı (Hybrid Bonding):**

```
En yeni teknoloji (TSMC SoIC, Samsung X-Cube):

  ┌───────────────────────────┐
  │  Üst çip                  │  Cu-Cu bağ (atomik temas)
  ├──●──────────────────●─────┤  ← bağlantı yüzeyi
  ├──●──────────────────●─────┤
  │  Alt çip                  │
  └───────────────────────────┘

  ● : bakır pad (~1-10 μm aralık)

Geleneksel TSV vs Hybrid Bonding:
  TSV:    10,000 bağlantı/mm²
  Hybrid: 1,000,000 bağlantı/mm² → 100x yoğun!

  Gecikme: TSV ~1 ps, Hybrid ~0.1 ps
  Bant genişliği: Hybrid → TB/s ölçeği

Bu teknoloji fotonik 3D için kritik:
  Fotonik katman ↔ elektronik katman
  Ultra-kısa optik yol → düşük kayıp
```

### 2.3 Fotonik 3D Entegrasyon: Özel Zorluklar

**Işığı Dikey Taşıma:**

```
Elektronik 3D: elektron TSV içinden geçer (kolay)
Fotonik 3D:   foton dikey geçiş zor (dalga kılavuzu yatay!)

Çözüm 1 — Evanescent Kuplaj:
  İki dalga kılavuzu çok yakın → ışık tüneller
  
  Üst katman dalga kılavuzu:  ─────────────────
                                  ↕ ~100-200 nm
  Alt katman dalga kılavuzu:  ─────────────────

  Tünel mesafesi: 100-200 nm (nm hassasiyeti!)
  Verimlilik: %95+ (düzgün hizalamada)
  Bant genişliği: tüm optik bant genişliği

Çözüm 2 — Dikey Grating (Kırınım Izgarası):
  Dalga kılavuzunda periyodik yapı → ışığı yukarı/aşağı kırar
  
  Dalga kılavuzu: ────[||||||||]────
                          ↕
                      dikey ışık
  Verimlilik: %70-85
  Kullanım: chip-to-fiber arayüz (grating coupler)

Çözüm 3 — Optik Geçit (Optical Via):
  Dikey dalga kılavuzu (SiN veya polimer)
  Araştırma aşamasında, %50-80 verimlilik
```

**Sıcaklık Yönetimi:**

```
Fotonik 3D yığın sıcaklık sorunu:

Kat 1 (en altta): 85°C
Kat 2:            90°C
Kat 3:            95°C
Kat 4 (en üstte): 100°C

Ring resonatör sıcaklık hassasiyeti:
  ΔT = +1°C → Δλ = +0.1 nm → kanal kayması

Çözümler:
  → Her kat termal sensör + mikroısıtıcı
    (geri beslemeli sıcaklık kontrolü)
  → Termal tutundurma (athermal design):
    negatif ve pozitif sıcaklık katsayılı malzeme karıştır
    net sıcaklık hassasiyeti ≈ 0
  → Kısıtlı kat sayısı (8-16, 32 değil)

Pratik sınır:
  Termal yönetim göz önüne alınırsa: 8-16 kat gerçekçi
  32 kat: çok zor, ama imkânsız değil (2030+ teknoloji)
```

**Malzeme Uyumluluk:**

```
Her katın kendi malzemesi var:
  Kat 1: CMOS Si → 1000°C süreç
  Kat 2: Si fotonik → 400°C
  Kat 3: TFLN (LiNbO₃) → 200°C
  Kat 4: Dedektör (Ge) → 300°C

Sıralama kritik: yüksek sıcaklık önce!
Sonraki katlar daha az ısıya maruz kalır

Kirlililik (contamination):
  Li, Nb: Si'yı kirletir → ayrı fabrika bölümü
  Ge: Si uyumlu (mevcut CMOS sürecinde var)
  
Çözüm: wafer bonding
  Her katı ayrı fabrikada üret
  Sonra yapıştır (bonding)
  → CMOS kirlenmez, TFLN yüksek sıcaklığa maruz kalmaz
```

### 2.4 3D Entegrasyonun CIM'e Katkısı

```
WDM olmadan, 3D yok: 256 m²
WDM N=512: 0.5 m²
3D 8 kat: 0.5/8 = 625 cm² (1 wafer'dan az!)
3D 16 kat: 0.5/16 = 312 cm²
3D 32 kat: 0.5/32 = 156 cm²

Fiziksel anlam:
  Her kat = aynı xy konumunda farklı ağırlıklar

  Üstten bakış (xy):        Kesit (z):
  ┌──────────────┐          ─── kat 8 (ağırlık 401B-512B)
  │   1 cm²      │          ─── kat 7
  │  xy alanı    │          ─── kat 6
  └──────────────┘          ─── kat 5
                            ─── kat 4
                            ─── kat 3
                            ─── kat 2
                            ─── kat 1 (ağırlık 1-50B)
  
  Aynı xy'de 8 kat → 8× daha fazla ağırlık
```

---

## Bölüm 3 — WDM + 3D Birlikte: 400B Model Analizi

### 3.1 İki Boyutlu Çoğaltma

```
WDM: yatay kanal çoğaltma (dalga boyu ekseni)
3D:  dikey katman çoğaltma (z ekseni)

Toplam çoğaltma faktörü:
  WDM N=512 × 3D 16 kat = 8,192×

256 m² / 8,192 = 312 cm²

Karşılaştırma:
  A4 kağıt:          624 cm²
  iPhone 15:          148 cm²
  NVIDIA H100 paketi: ~900 cm²
  312 cm²:           H100 ile benzer boyut

→ 400B model, H100 büyüklüğünde bir FE+EO CIM çipine sığar
```

### 3.2 Gerçekçi Mimari

```
400B FE+EO CIM paketi (2035 hedefi):

Dışarıdan:
┌──────────────────────────────────────────────┐
│  ~30cm × 10cm paket (büyük ama mümkün)      │
│  ← GPU'ya benzer büyüklük                    │
└──────────────────────────────────────────────┘

İçeriden (katlar):

Kat 16 ─── TFLN FE+EO (ağırlıklar 351B-400B)
Kat 15 ─── TFLN FE+EO (ağırlıklar 301B-350B)
Kat 14 ─── TFLN FE+EO
...
Kat 9  ─── TFLN FE+EO (ağırlıklar 51B-100B)
Kat 8  ─── Si fotonik yönlendirme + AWG (WDM MUX/DEMUX)
Kat 7  ─── Ge fotodedektör dizisi
Kat 6  ─── ADC + dijital çıkış katmanı
Kat 5  ─── Optik kuplaj (evanescent)
Kat 4  ─── TFLN FE+EO (ağırlıklar 1B-50B)
...
Kat 2  ─── TFLN FE+EO
Kat 1  ─── CMOS kontrol elektronikleri + güç

Kenar:
  → Grating coupler: lazer girişi (512 WDM kanalı)
  → Elektriksel I/O: ağırlık yükleme, dijital çıkış
  → Güç beslemesi
  → Termal pad: ısı dağıtımı
```

### 3.3 Bant Genişliği Hesabı

```
WDM kanalları × modülasyon hızı:
  512 kanal × 100 Gbps = 51.2 Tbps giriş bant genişliği

3D 16 kat:
  Her kat ayrı giriş/çıkış
  16 × 51.2 Tbps = 819.2 Tbps toplam iç bant genişliği

Karşılaştırma:
  HBM3 (8 yığın): ~900 GB/s = 7.2 Tbps
  FE+EO CIM: 819.2 Tbps → 113× daha yüksek!

  Ama bu iç bant genişliği — çipin içinde
  Çip dışı (giriş): 51.2 Tbps hâlâ gerekli
  → Lazer ve fiber bağlantısı
```

---

## Bölüm 4 — Günümüz Teknoloji Olgunluk Tablosu

```
Bileşen              Bugün          2028 Hedef     2032 Hedef
────────────────────────────────────────────────────────────────
WDM kanal (CMOS)     16-128         256            512+
Comb lazer kanal     100+           256            512+
AWG boyutu           cm²            mm²            μm²
Ring resonatör T-ath Araştırma     Demo           Ürün
3D kat sayısı (gen.) 4-8 (HBM)     12-16          32+
Wafer bonding yoğ.   10K/mm²        100K/mm²       1M/mm²
TFLN endurance       10⁴ döngü      10⁶            10⁸
Dikey optik kuplaj   %70-85         %90+           %95+
Termal yönetim 3D    Araştırma      Demo           Ürün
400B FE+EO CIM       İmkânsız      Araştırma      Prototip mümkün
```

---

## Bölüm 5 — Kısa Vade Gerçekçi Hedef

400B hemen mümkün değil. Peki ne mümkün?

```
2026-2028: 7B-13B model (Llama sınıfı)

  WDM: 64 kanal (bugün mümkün)
  3D: 4 kat (HBM teknolojisiyle)
  Çoğaltma: 64 × 4 = 256

  7B model FE+EO alanı:
  7B × 0.64 μm² / 256 = 17.5 cm²

  → iPhone 15 boyutunda bir çip!
  → Laptop portuna takılan SoC için gerçekçi
  → Bu konuşmanın başındaki istek karşılanabilir

Bekleme gücü: sıfır
Okuma enerjisi: ~100× GPU'dan az
LLM çıkarım: gerçek zamanlı, yerel, özel

2030: 70B model
  WDM: 256 kanal
  3D: 8 kat
  Çoğaltma: 2,048

  70B × 0.64 μm² / 2,048 = 218 cm²
  → GPU paketi büyüklüğünde
  → Datacenter yerine masa üstü
```

---

## Bölüm 6 — Volt Tip Sisteminde WDM ve 3D

```volt
// WDM kanal tipi: dalga boyu tiptir
type WDMChannel<const λ_nm: u32, const bw_GHz: u32>
    = OpticalSignal @Photonic{λ=λ_nm}

// Comb lazer: tek kaynaktan çok kanal
module CombLaser<const N: usize, const λ_base: u32,
                 const spacing_GHz: u32> {
    out channels: [WDMChannel<λ_base + i*spacing_GHz>; N]
    @cost(energy=100.mW_total, per_channel=100.mW/N)
    // Önemli: N artar → kanal başına maliyet düşer!
}

// AWG: kanal ayırıcı
module AWG_Demux<const N: usize, const λ_base: u32> {
    in  wdm_in   : WDMSignal<N>   // hepsi birlikte
    out channels : [WDMChannel<λ_base + i*100GHz>; N]
    @cost(area=0.1.mm2 * N, insertion_loss=1.5.dB)
}

// 3D katmanlar: Volt'ta domain olarak
domain PhotonicLayer<const k: u8> {
    // Her kat farklı domain → aralarındaki geçiş açık
    layer_index = k
    z_position  = k * 5.um      // kat aralığı 5 μm
}

// Katlar arası evanescent kuplaj köprüsü
bridge EvanescentCouple<const k_from: u8, const k_to: u8,
                         const efficiency: f32> {
    in  light_in  : OpticalSignal @PhotonicLayer<k_from>
    out light_out : OpticalSignal @PhotonicLayer<k_to>

    @coupling_gap(150.nm)         // kritik hizalama mesafesi
    @efficiency(efficiency)        // %90-95 tipik
    @cost(energy=0.pJ)             // pasif eleman, enerji yok

    // Volt tipi: katlar arası geçiş açık ve kontrollü
    // Yanlış kat bağlantısı → derleme hatası
}

// 400B FE+EO CIM mimarisi (hedef)
module FEO_CIM_400B {
    // 16 kat, 512 WDM kanal
    let laser = CombLaser<512, λ_base=1530.nm, 25.GHz>()
    let mux   = AWG_Mux<512>()
    let demux = AWG_Demux<512>()

    // 16 TFLN katı, her katta 25B ağırlık
    let layer: [FEO_WeightLayer<25_000_000_000>; 16]

    // Katlar arası optik kuplaj
    for k in 0..15 {
        layer[k].output ──► EvanescentCouple<k, k+1, 0.93>
                        ──► layer[k+1].input
    }

    // Volt doğrulaması
    invariant: total_weights == 400_000_000_000
    invariant: total_area < 400.cm2
    invariant: standby_power == 0.W   // non-volatile!
    @cost(
        inference_energy = total_weights * 0.01.fJ,
        // 400B × 0.01 fJ = 4 μJ per token — GPU'nun 1000x altı
    )
}
```

---

## Özet: WDM ve 3D Ne Yapar, Neden Önemli

```
WDM (Dalga Boyu Çoğullama):
  "Bir dalga kılavuzunda N paralel veri yolu"
  
  Nasıl: farklı renk ışıklar birbirini görmez
  Kaç: 16-512 kanal (bugün→2032)
  CIM'e katkı: N× yoğunluk (yatay)
  Bileşen: AWG, ring resonatör, comb lazer
  Zorluk: sıcaklık hassasiyeti, kanal sayısı sınırı

3D Entegrasyon:
  "Birden fazla katı üst üste yığ"
  
  Nasıl: wafer bonding + evanescent kuplaj
  Kaç: 4-32 kat (bugün→2032)
  CIM'e katkı: K× yoğunluk (dikey)
  Bileşen: TSV, hybrid bonding, optical via
  Zorluk: sıcaklık yönetimi, dikey ışık iletimi

İkisi birlikte:
  N × K toplam çoğaltma faktörü
  512 × 32 = 16,384× → 400B model 156 cm²'ye sığar

Neden kritik:
  Fotonik'in en büyük zaafı: boyut
  (difraksiyon sınırı → elektroniğin 100,000× büyük hücresi)
  WDM + 3D bu zaafı "tasarım katmanında" çözüyor:
  Fizik sınırını değiştiremiyor ama mühendislik aşıyor
```
