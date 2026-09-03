> UYARI: Bu belge geçersizdir.
> Güncel sürüm: docs/design/ altında.

# Volt Ekosistemi — Bütünleşik Vizyon ve Yol Haritası

> Bu belge, HDL tasarımından başlayıp nöromorfik hesaplama, fotonik ternary,
> Von Neumann alternatifleri ve mimari işletim sistemine uzanan bir konuşmanın
> sentezi ve eylem planıdır. Her katman bir sonrakinin zeminini hazırlar;
> sinerji araçların yan yana durmasından değil, **aynı tip sistemini, aynı
> semantiği ve aynı doğrulama modelini paylaşmasından** gelir.

---

## Bölüm 1 — Büyük Resim: Nerede Duruyoruz

### 1.1 Beş Bağımsız Kriz Aynı Anda Olgunlaştı

```
Kriz 1 — HDL yetersizliği:
  Verilog/SV: gizli davranışlar, zayıf tip güvenliği,
              sim/sentez uyumsuzluğu
  Chisel/SpinalHDL: ev sahibi dil yükü (Scala/JVM)
  Sonuç: en pahalı donanım hataları derleme sonrasına
         kalıyor — silikon dönüşü milyonlarca dolar

Kriz 2 — Von Neumann duvarı:
  Bellek ve işlemci ayrı → veri taşıma = enerji
  LLM çıkarımı: hesaplama değil, DRAM erişimi darboğaz
  GPU 100+ TOPS ama 70B model için 140 GB/s bant şart
  Fizik: bu duvar daha da büyüyecek, küçülmeyecek

Kriz 3 — Sayısal format sınırı:
  FP32: çok pahalı; FP16: taşma; INT4: 16 değer
  Fiziksel sınır (Landauer): N bit → 2ᴺ değer, aşılamaz
  Ama hangi 2ᴺ değeri seçeceğin = tasarım özgürlüğü
  NF4, MX formatları bu özgürlüğü henüz tam kullanmıyor

Kriz 4 — AI donanımı demokratikleşemiyor:
  Tasarım: yalnızca mühendisler → yavaş üretim
  Doğrulama: özel araçlar, uzun döngüler
  Üretim: TSMC sign-off için Calibre → milyonlarca dolar
  Sonuç: beyin gücü dar boğazda

Kriz 5 — Yeni paradigmalar kör:
  Nöromorfik: spike zamanlaması ifade edilemiyor
  Fotonik: ternary doğal ama binary zorunlu tutuluyor
  Analog/Dijital sınır: tip sisteminde yok
  Heterojen SoC: paradigmalar arası geçiş "elle"
```

### 1.2 Konuşmanın Keşfettiği Birleşim Noktası

Bu beş kriz bağımsız değil — **aynı kökteki sorunun farklı tezahürleri:**

> Mevcut donanım soyutlama katmanı, yalnızca tek bir hesaplama paradigması
> (senkron dijital RTL, binary, Von Neumann) için tasarlandı. Gerçek dünya
> artık birden fazla paradigmayı aynı çipte istiyor — ve bunu ifade edecek,
> doğrulayacak, sentezleyecek, birleştirecek bir araç yok.

```
Çözüm: Volt — tek semantik modeli olan,
        çok-paradigmalı, tip-güvenli,
        formal doğrulanabilir bir donanım tanımlama dili
        ve onun üzerine kurulu ekosistem.
```

---

## Bölüm 2 — Konuşmanın Katman Katman Özeti

### Katman A — Volt HDL: Temel

**Ne tasarlandı:**
- Bağımsız, amaca özel HDL (Scala/Python ev sahibi dil yükü yok)
- Tip sistemi: genişlik + işaretlilik + saat alanı + birim + zamanlama
- Tek semantik model: sim = sentez (sim'de çalıştı, çipte çalışmadı imkânsız)
- Kademeli zamanlama tipleri: L0 (pipeline hizalama) / L1 (Delayed<T,N>) / L2 (timeline)
- CDC tip sisteminde: `bool @Fast` → `bool @Slow` doğrudan bağlantı derleme hatası
- Birinci sınıf yapılar: `fsm`, `pipeline`, `Stream`/`Flow`, `rule`
- Rust/Elm sınıfı hata mesajları: konum + neden + somut düzeltme

**Derleyici mimarisi:**
```
Rust önyüz (cstree CST, error-recovery)
    → CIRCT/MLIR (Calyx/Handshake/FIRRTL)
    → SystemVerilog (sentez akışlarına)
    + Native simülatör + SymbiYosys formal
    + LSP + volt fmt + paket yöneticisi
```

**Resmî spesifikasyon:** EBNF gramer + çıkarım-kuralı formatında tip sistemi hazır.
**MVP yol haritası:** 12 ay, 6 faz, ~6-7 FTE.
**F0-F1 teknik tasarım:** Crate mimarisi, CST, parser, hata kurtarma, HIR, isim çözünümü somutlaştırıldı.

---

### Katman B — Nöromorfik Hesaplama: İlk Uzantı

**Temel içgörü:** LLM darboğazı TOPS değil GB/s — bellek bant genişliği. Nöromorfik çip bellek bant genişliği *ihtiyacını* ortadan kaldırır (memristör crossbar: ağırlık = fiziksel direnç, hesaplama = Ohm yasası).

**Volt ile mevcut:** Dijital LIF nöron, spike yönlendirici, FIFO — bugün yazılabilir.

**Volt v2 uzantıları gerekiyor:**
```volt
Spike<T>           — spike zamanlaması birinci sınıf tip
SpikeStream        — seyrek, olay-tabanlı akış
on spike(x) { }   — yalnızca spike gelince çalış (%99 güç tasarrufu)
rule stdp when fire { weight <= weight + delta(timing) }
```

**Setun bağlantısı:** Dengeli üçlü (-1, 0, +1) STDP sinaptik durumlarla (zayıflat/değiştirme/güçlendir) doğal örtüşüyor. 1958'deki matematiksel zarafet, 2024'te donanım verimliliğine dönüşüyor.

**Ekosistem ihtiyaçları:** NeuroTorch (spike-uyumlu eğitim), NeuroCompiler (topoloji→çip eşleme), Spike kodlama kütüphanesi, Profiler (spike raster + enerji), Doğrulama (Volt assert/invariant).

---

### Katman C — Fotonik Hesaplama: Derin Uzantı

**Temel içgörü:** Işık analog — sürekli genlik ve faz. Binary fotonik, analog sinyali iki duruma *zorla* indirgiyor. MZI interferometrenin **üç doğal çalışma noktası** var:
- θ=0 → +E (yapıcı girişim) = **+1**
- θ=π → 0 (yokedici girişim) = **0**
- θ=2π → -E (ters fazda yapıcı) = **-1**

Bu, dengeli üçlünün fiziksel karşılığı.

**Ternary nöral ağ + fotonik:**
```
Ağırlık +1 → ışığı geçir         (0 enerji)
Ağırlık  0 → ışığı engelle       (minimal enerji)
Ağırlık -1 → π faz ekle + geçir  (0.001 pJ)

FP32 çarpma: ~3 pJ
Fotonik ternary çarpma: ~0.001 pJ
→ 3,000x enerji verimliliği
```

**E-O-E sorunu:** Her dijital↔fotonik geçiş 5-10 pJ/bit. Derin ağlarda bu domine eder. Ternary kodlama bu geçişi minimuma indirir (aynı güç seviyesinde +1/-1 ayrımı faz ölçümüyle yapılır).

**Volt v3 uzantıları:**
```volt
domain PhotonicTernary { carrier=photon, encoding=BalancedTernary }
type PTrit = Trit @PhotonicTernary
module TernaryMVM<N>: dalga süperpozisyonu = ücretsiz toplama
bridge ElectroOptic: tip-güvenli, maliyeti görünür
```

---

### Katman D — Von Neumann Alternatifleri ve Hibrit SoC

**İçgörü:** Tek mimari Von Neumann'ın yerini alamaz. Her paradigma farklı kısıtı çözer:

| Paradigma | Veri Hareketi | Sıralı Kontrol | En İyi Görev |
|---|---|---|---|
| Nöromorfik (memristör) | ✅ Çözer | ✅ Çözer | Seyrek, temporal |
| Fotonik ternary | ✅ Kısmen | ✗ | Yoğun matris |
| Uzaysal/CGRA | ✅ Çözer | ✅ Çözer | Pipeline, akış |
| Analog CAM | ✅ Çözer | — | Benzerlik arama |
| PIM/Memristör | ✅ Çözer | — | Ağırlık depolama |
| Scalar CPU | ✗ | ✅ Çözer | Kontrol akışı |

**Doğru yaklaşım:** Heterojen kompozisyon — görev paradigmayı belirler, geçişler tip-güvenlidir.

---

### Katman E — Mimari İşletim Sistemi

**İki katmanlı yapı (karar süresi yeri belirler):**

```
SoC İç Katmanı (nanosaniye-mikrosaniye):
  Spike yönlendirme, ADC/DAC köprü,
  Volt assert çalışma zamanı, anlık güç kontrolü

Host Dış Katmanı (milisaniye-saniye):
  Görev→paradigma eşleme, güç politikası,
  uzun dönem kalibrasyon, kullanıcı arayüzü
```

**Volt'un rolü:** Donanım kendini Volt `hardware_descriptor` tipleriyle tanımlar. Eşleme kısıtları Volt tiplerinden türer — yanlış eşleme derleme hatası. Aynı Volt tipi hem tasarım zamanında hem çalışma zamanında aynı anlama gelir (tek semantik model).

---

## Bölüm 3 — Sinerjik Bağlantı Haritası

Her ok "bu olmadan o çalışmaz" ilişkisini gösterir:

```
┌────────────────────────────────────────────────────────────────┐
│                    VOLT TİP SİSTEMİ                            │
│              (tek semantik model — tüm katmanların zemini)     │
└──────────┬───────────────┬──────────────┬──────────────────────┘
           │               │              │
    ┌──────▼──────┐ ┌──────▼──────┐ ┌────▼────────────────┐
    │  Dijital    │ │ Nöromorfik  │ │  Fotonik Ternary    │
    │  RTL Volt   │ │ Spike<T>    │ │  PTrit @Photonic    │
    │  (v0.1 MVP) │ │ (v2)        │ │  (v3)              │
    └──────┬──────┘ └──────┬──────┘ └────┬────────────────┘
           │               │              │
           └───────────────┴──────────────┘
                           │
                    ┌──────▼──────┐
                    │  CIRCT/MLIR │
                    │  (ortak IR) │
                    └──────┬──────┘
                           │
          ┌────────────────┼────────────────┐
          │                │                │
   ┌──────▼──────┐ ┌───────▼──────┐ ┌──────▼──────┐
   │  SV/FPGA   │ │ Nöromorfik   │ │  Fotonik    │
   │  Sentez     │ │ Çip Eşleme  │ │  Chip Fab   │
   └─────────────┘ └──────────────┘ └─────────────┘
           │                │                │
           └────────────────┴────────────────┘
                           │
                   ┌───────▼───────┐
                   │  Hibrit SoC   │
                   │  Donanım      │
                   └───────┬───────┘
                           │
                   ┌───────▼───────┐
                   │ Mimari OS     │
                   │ (SoC + Host) │
                   └───────┬───────┘
                           │
                   ┌───────▼───────┐
                   │ Kullanıcı /   │
                   │ AI Ajan       │
                   └───────────────┘
```

**Sinerjinin somut örnekleri:**

- Volt'un CDC tipi → mimari OS'ta paradigmalar arası geçişin tip güvenliği
- Volt'un `assert`/`invariant` → profiler'a otomatik izleme noktası enjeksiyonu
- Volt'un `hardware_descriptor` → eğitim çerçevesine donanım kısıtı geri beslemesi
- Volt'un tek semantik modeli → sim = sentez = çalışma zamanı doğrulama
- Dengeli üçlü tipler → nöromorfik STDP + fotonik MZI aynı `Trit` tipiyle

---

## Bölüm 4 — Bütünleşik Yol Haritası

### Faz Yapısı Genel Bakış

```
YIL 1: TEMEL           YIL 2-3: UZANTI        YIL 4-5: OLGUNLUK
───────────────────    ────────────────────    ──────────────────
Volt MVP               Volt v1                 Volt v2/v3
(Core uyumluluk)       (Python API,            (Spike<T>,
                        ML PPA, Timeline)       PTrit, Analog)

F0-F1 Önyüz            NeuroTorch              Fotonik Ternary
F2 Tip Sistemi          NeuroCompiler           Derleyici
F3 CIRCT Lowering       Spike Kodlama           Fotonik EDA
F4 Simülatör            Profiler                Formal Semantik
F5 LSP + Stdlib         Doğrulama               Architecture OS
                                                Hibrit SoC Proto
```

---

### Faz 0 — Temel Kararlar (Ay 0, önce tamamlanmalı)

Bu faz kod yazmadan önce, **tasarım kararlarının kitaplaştırılmasıdır:**

```
ADR-0001: cstree vs rowan → cstree (string interning)
ADR-0002: Lexer yaklaşımı → logos + post-process
ADR-0003: Hata modeli → codespan-reporting + JSON
ADR-0004: HIR ayrımı → ayrı arena-tabanlı HIR
ADR-0005: Incremental → MVP'de salsa yok, geçişe uygun sınır

Kritik mimarî sözleşme:
  "Volt tipi = doğrulama birimi"
  Tip sistemine girmeyen hiçbir donanım özelliği
  derleyici tarafından denetlenemez.
  Bu ilke tüm uzantılar için kılavuz prensip.

Ekosistem sözleşmesi:
  Tüm katmanlar (NeuroTorch, NeuroCompiler, Architecture OS)
  aynı Volt IR'ı konuşur — kendi veri formatını icat etmez.
  Bu kural yazılı, bağlayıcı, gün-1'den itibaren geçerli.
```

---

### Faz 1 — Volt MVP (Ay 1–12)

Yol haritası detayı daha önce hazırlandı. Özet:

```
F0 (Ay 1):  Yürüyen iskelet — Counter → SV → Verilator
F1 (Ay 2-3): Parser + CST + HIR + isim çözünümü
F2 (Ay 4-6): TİP SİSTEMİ — projenin kritik yolu
             Genişlik + işaret + saat alanı + CDC + latch + L0/L1
F3 (Ay 6-8): CIRCT lowering → okunabilir SV
F4 (Ay 9-11): Simülatör + formal doğrulama (SymbiYosys)
F5 (Ay 11-12): LSP + stdlib + doküman + beta

Kilometre taşı M3 (Ay 6): Tip sistemi Core tamam
→ Projenin başarı/başarısızlık eşiği burada
```

**Bu fazın teslim etmesi gereken en kritik şey:**

```
1. CDC ihlali derlenmez
2. Gizli latch derlenmez
3. Genişlik kesmesi sessiz olmaz
4. Sim = sentez (tek semantik model kanıtlanmış)
5. Rust/Elm sınıfı hata mesajları

Bu beş garanti olmadan sonraki hiçbir katman güvenilir değil.
```

---

### Faz 2 — Volt v1 ve Ekosistem Temeli (Ay 13–24)

**Volt v1 özellikleri:**
```
Python API:    jeneratör + cocotb doğrulama
               (donanım tanımı değil, IR üreticisi)
ML PPA:        SOG tabanlı tahmin motoru → LSP canlı PPA
Timeline L2:   #[timeline] ile full Filament/Anvil
Paket registry: açık, vendor-bağımsız IP kataloğu
```

**NeuroTorch (paralel, bağımsız ekip):**
```
Volt kısıt-bilinçli eğitim:
  constraints = VoltTarget.load("soc0.volt")
  Population(LIF, precision=constraints.weight_bits)

Spike-uyumlu geri yayılım:
  Surrogate gradient + STBP
  STDP + backprop karma

Cocotb entegrasyonu:
  Python referans modeli = donanım testbench
```

**NeuroCompiler iskeleti:**
```
Topoloji IR (hedef-bağımsız) tanımı
FPGA hedefi: Volt RTL üretimi (ilk hedef)
Loihi 2 hedefi: Intel Lava API köprüsü
```

---

### Faz 3 — Nöromorfik Uzantı (Ay 24–36)

**Volt v2 çekirdek uzantıları:**
```volt
// Bu üç yapı Volt v2'nin özü
type Spike<T> = { value: T, time: Timestamp, source: NeuronId }
type SpikeStream         // seyrek, olay-tabanlı
on spike(x) { ... }     // yalnızca spike gelince çalış
```

**Kritik tip genişlemesi — Trit:**
```volt
// Setun'dan gelen matematiksel zarfet
// Nöromorfik ve fotonik için ortak taban
type Trit = i2   // -1, 0, +1 dengeli üçlü

// STDP doğal Trit işlemi olarak
rule stdp when fire {
    weight <= weight + Trit::from_timing(pre, post)
}
```

**NeuroCompiler olgunlaşması:**
```
Topoloji → Volt RTL → FPGA: tamamlanmış
Topoloji → Loihi 2: çalışır prototip
Topoloji → Memristör crossbar: araştırma
```

**Olay-tabanlı anlambilim:**
```
Mevcut: tüm bloklar her cycle çalışır
Yeni:   on spike bloğu yalnızca spike gelince
→ %99 güç tasarrufu potansiyeli
→ Semantic garanti: Volt'un tek semantik modeline eklenir
```

---

### Faz 4 — Fotonik Ternary Uzantısı (Ay 36–60)

**Volt v3 fotonik katmanı:**
```volt
domain PhotonicTernary {
    carrier  = photon
    encoding = BalancedTernary    // dengeli üçlü
    states   = {
        Pos: (amplitude=1.0, phase=0.rad),    // +1
        Zer: (amplitude=0.0),                  //  0
        Neg: (amplitude=1.0, phase=PI.rad)     // -1
    }
}

type PTrit = Trit @PhotonicTernary

// MZI: üç doğal çalışma noktası
// θ=0 → +1, θ=π → 0, θ=2π → -1
extern module MZI {
    in  input : PTrit @PhotonicTernary
    in  theta : f32   @Electronic    // faz kontrolü elektronik
    out output: PTrit @PhotonicTernary
    @cost(energy=0.001.pJ, latency=1.ps)
}

// E-O-E köprüsü: tip-güvenli, maliyeti görünür
bridge ElectroOptic { @cost(energy=5.pJ_per_trit) }
bridge OptoElectronic { @cost(energy=2.pJ_per_trit) }
```

**Fotonik EDA araç zinciri:**
```
Si-fotonik CMOS süreci için Volt hedefi
Waveguide yönlendirme (PCB routing'in fotonik karşılığı)
Termal sapma modeli (0.01nm/°C)
Faz kalibrasyon aracı
```

**Ternary-bilinçli CIRCT lehçesi:**
```
Mevcut CIRCT: binary varsayımı
Uzantı:       TernaryHW lehçesi
              Trit aritmetiği → MZI primitifleri
              Dengeli üçlü optimizasyon geçişleri
```

---

### Faz 5 — Mimari İşletim Sistemi (Ay 48–72)

**Hardware Descriptor Standardı:**
```volt
hardware_descriptor NeuromorphicCore {
    paradigm  = Neuromorphic
    capacity  = Population<LIF, max=1_000_000>
    precision = Spike<Trit>       // FeFET üçlü
    energy    = 1.mW @ full_load
    constraint: weight_bits == 2
}

hardware_descriptor PhotonicCore {
    paradigm   = Photonic
    precision  = PTrit
    throughput = 1.TOPS_per_pJ
    constraint: operation == TernaryMVM
    constraint: matrix_size <= 64
}
```

**Eşleme motoru:**
```
Kısıt: Volt tip uyumluluğu (zorunlu, derleme zamanı)
Hedef: enerji × gecikme minimize (çalışma zamanı)
Öğrenme: Bayesian güncelleme (gerçek ölçüm → model)
```

**SoC ↔ Host protokolü:**
```
Host → SoC: TaskPacket (VoltIR + kısıtlar)
SoC → Host: StatusUpdate (güç, invariant ihlalleri)
SoC → Host: TaskResult (enerji, gecikme, doğruluk)
```

---

### Faz 6 — Hibrit SoC Prototipi (Ay 60–84)

```
Hedef donanım:
  Nöromorfik çekirdek (FeFET/Memristör, Trit ağırlıklar)
  + Fotonik ternary matris (Si-fotonik + MZI)
  + Uzaysal CGRA (orta karmaşıklık)
  + Scalar kontrolcü (ARM Cortex-M)
  TB4/TB5 arayüzü (laptop bağlantısı)

Üretim stratejisi:
  Açık kaynak EDA (Yosys + OpenROAD)
  + Hibrit broker (Calibre DRC/LVS kiralama)
  → TSMC 28nm MPW shuttle ($52-64K)

Volt'un bu çipta rolü:
  Her çekirdeğin RTL'i Volt ile tasarlandı
  Tüm paradigmalar aynı tip sistemini paylaşıyor
  CDC + domain geçişleri tip-güvenli
  Formal doğrulama her katmanda
```

---

## Bölüm 5 — İlk Nereden Başlamak

### Cevap: F0-F1'den — Ama Doğru Stratejiyle

Her katman Volt'a bağımlı. Volt derleyicisi olmadan:
- NeuroTorch'un donanım kısıtı geri beslemesi çalışmaz
- NeuroCompiler'ın Volt RTL üretimi mümkün değil
- Mimari OS'un tip-güvenli descriptor'ları yoktur
- Fotonik ternary için EDA araç zinciri temelsiz kalır

**Ama "sadece HDL yazayım" değil — Volt'u doğru konumlandırarak başlamak gerekir.**

### Başlangıç Anı: Üç Eş Zamanlı Hamle

```
Hamle 1 — Volt MVP kodu (Ay 1+):
  F0-F1 teknik tasarım dokümanı hazır
  Başla: cargo new, workspace, ADR-0001..5
  Odak: tip sistemi (F2) kritik yol
  Risk: CIRCT yetkinliği (erken spike gerekli)

Hamle 2 — Trit tipinin çekirdeğe girmesi (Ay 1+, paralel):
  MVP içinde Trit = i2 tanımı yap
  newtype FeSynapse = Trit (FeFET için)
  Spike<Trit> gelecek uzantı için alan bırak
  → Saat sona erdiğinde ternary ekleme değil,
    baştan dahil etme farkı yaratır

Hamle 3 — Ekosistem sözleşmesi (Ay 0):
  "Volt IR ortak dil" kararını yazılı yap
  NeuroTorch, NeuroCompiler, Architecture OS —
  hepsi bu sözleşmeyi imzalar
  → Sonraki ekipler tutarlı zemine basar
```

### Başlangıç Anının Çıktısı (Ay 1, M1)

```
volt build counter.volt → SV → Verilator → CI yeşil

Bu küçük çıktının anlamı:
  ✓ Rust önyüz çalışıyor
  ✓ CIRCT lowering spike'ı başarılı
  ✓ Sim = sentez tutarlılığı kanıtlandı (bu modelde)
  ✓ CI altyapısı kurulu
  ✓ Trit tipi çekirdekte

Bir sonraki 11 ay bu temeli genişletir.
```

---

## Bölüm 6 — Risk ve Gerçekçilik

### Kritik Yollar (Projeyi Öldürebilecek Şeyler)

```
Risk 1 — F2 tip sistemi patlaması:
  CDC + latch + genişlik + zamanlama hepsini birden
  yapma isteği → kapsam kayması → geç teslim
  Önlem: F2a/b/c/d alt-fazları, sıkı çıkış kriterleri

Risk 2 — CIRCT yetkinliği:
  LLVM konferansı → CIRCT topluluğuyla erken temas
  İlk ayda CIRCT spike: "Counter lowering CIRCT ile"
  Plan B: doğrudan FIRRTL üretimi

Risk 3 — Ekosistem önce dil sonra:
  NeuroTorch ekibi Volt bitmeden başlarsa
  kendi veri formatını icat eder → sinerji kırılır
  Önlem: Ay 0'da ekosistem sözleşmesi

Risk 4 — Fotonik ternary prematüre optimizasyon:
  Fotonik gerçeklik doğrulanmadan Volt'a ekleme
  → Yanlış soyutlama → kaldırmak daha pahalı
  Önlem: Lab partneri (üniversite Si-fotonik grubu)
          Ay 24'te "fotonik ternary gerçeklik kontrolü"
          geçmeden v3'e başlama
```

### Dürüst Zaman Çerçevesi

```
"CUDA benzeri sezgisel nöromorfik programlama":
  CUDA 2007 → gerçek benimseme 2012 = 5 yıl
  Nöromorfik daha karmaşık → 8-10 yıl gerçekçi

"Fotonik ternary ticari çip":
  Si-fotonik CMOS olgunluğu: 2028-2030
  Ternary EDA araçları: 2030+
  Ticari ürün: 2032+ gerçekçi

"Volt MVP beta":
  12 ay — ekip varsa gerçekçi
  Risk: CIRCT öğrenme ±2 ay

Bu zaman çerçeveleri uzun görünür ama:
  GPU'nun dominant olması 30 yıl sürdü
  CMOS ölçeklenmesi 50 yıldır devam ediyor
  Doğru temeli erken atmak = uzun vadeli kazanç
```

---

## Bölüm 7 — Tek Cümle Özeti

Her katmanı tek cümlede:

```
Volt MVP:
  "Donanım anlambiliminden ödün vermeden en pahalı
   donanım hatalarını derleme zamanında imkânsız kılan HDL."

Nöromorfik uzantı:
  "Spike zamanlaması tipin parçası olduğunda,
   beyin gibi hesaplayan donanım güvenle tanımlanabilir."

Fotonik ternary:
  "MZI'nin üç doğal çalışma noktası dengeli üçlünün
   üç değeriyle örtüşür — binary fotonik zorlanmış,
   ternary fotonik fiziğin kendisi."

Mimari OS:
  "Görev donanımı seçtiğinde değil, donanım kendini
   tip-güvenli tanımladığında ve eşleme kısıtları
   derleyiciden geldiğinde mimari OS mümkün olur."

Tüm vizyon tek cümlede:
  "Volt, Von Neumann'ın ötesinde her hesaplama
   paradigmasını — dijital RTL, nöromorfik spike,
   fotonik ternary matris, analog CAM — aynı tip
   sisteminde, aynı semantikle, aynı formal
   doğrulamayla ifade eden birleştirici zemin;
   ve bu zeminin üzerine kurulu ekosistem, donanım
   tasarımını mühendislik tekelinden çıkarıp
   alan uzmanlarının eline verir."
```

---

## Ekler

### Ek A — Tüm Dokümanlar

1. Volt HDL Tasarım Dokümanı (birleşik)
2. Volt Resmî Dil Spesifikasyonu (EBNF + tip kuralları)
3. Volt MVP Yol Haritası (6-12 ay)
4. F0-F1 Teknik Tasarım Dokümanı (crate mimarisi + parser)

### Ek B — Bağımlılık Grafiği (Özet)

```
Fotonik Ternary SoC
    └── Mimari OS
        └── Hibrit SoC Donanımı
            ├── Nöromorfik Çekirdek (FeFET/Memristör)
            │   └── Volt v2 (Spike<Trit>)
            │       └── Volt v1 (Timeline, Python API)
            │           └── Volt MVP (Core tip sistemi)
            │               └── F0-F1 (Önyüz, bugün)
            └── Fotonik Ternary Çekirdek
                └── Volt v3 (PTrit, MZI primitif)
                    └── Volt v2 (Trit tipi)
                        └── Volt MVP

Her şey F0-F1'e bağlı.
F0-F1, bugün başlanabilecek tek somut adım.
```

### Ek C — İlk Hafta Eylem Listesi

```
Gün 1:
  [ ] cargo new volt --workspace
  [ ] Crate yapısı: volt-span, volt-diagnostics,
      volt-syntax, volt-ast, volt-hir, volt-lower,
      volt-driver, volt-cli
  [ ] GitHub Actions CI iskeleti
  [ ] ADR klasörü + ADR-0001 taslağı

Gün 2-3:
  [ ] volt-span: TextSize, TextRange, FileId, SourceMap
  [ ] volt-diagnostics: Diagnostic, Label, TtyEmitter
  [ ] İlk birim testleri

Gün 4-5:
  [ ] SyntaxKind enum (tam liste, spesifikasyondan)
  [ ] Minimal lexer (keyword + ident + literal)
  [ ] İlk corpus dosyası: counter.volt

Gün 6-7:
  [ ] Counter için şablon lowering (F0 stub)
  [ ] volt build counter.volt → SystemVerilog
  [ ] Verilator CI entegrasyonu
  [ ] M1 kapanış: "çalışıyor" kanıtı

Paralel (haftadan bağımsız):
  [ ] CIRCT topluluğuyla temas (LLVM Discourse)
  [ ] Ekosistem sözleşmesi taslağı
  [ ] Trit tipinin çekirdek planı (ADR-0006)
```
