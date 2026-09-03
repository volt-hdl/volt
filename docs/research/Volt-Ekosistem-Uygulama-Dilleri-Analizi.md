> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Volt Ekosistemi — Uygulama Dilleri Analizi

> Ekosistemi inşa eden diller (implementation languages) ile ekosistemi
> kullanan dil (Volt) birbirinden farklı sorulardır. Bu belge ikisini de
> inceler: her bileşen hangi dille yazılmalı, mevcut diller nerede yeterli
> nerede yetersiz, ve neden Volt'un kendisi yeni bir dil olmak zorunda.

---

## Bölüm 1 — Bileşen Haritası ve Dil Kararları

### Genel Mimari

```
┌─────────────────────────────────────────────────────────────┐
│  Kullanıcı Katmanı (alan uzmanı, donanım mühendisi)         │
│  Dil: Volt (yeni) + Python API                             │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│  Araç Katmanı                                               │
│  LSP, fmt, lint, playground, paket yöneticisi              │
│  Dil: Rust                                                  │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│  Derleyici Katmanı                                          │
│  Önyüz + tip sistemi + HIR + CIRCT lowering                │
│  Dil: Rust + MLIR C-API (FFI)                              │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│  Ekosistem Katmanı                                          │
│  NeuroTorch, NeuroCompiler, Spike lib, Profiler             │
│  Dil: Python + Rust (performans kritik çekirdek)           │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│  Formal Katman                                              │
│  K-framework semantik, SymbiYosys                          │
│  Dil: K dili + SMT-LIB                                     │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│  Mimari OS                                                  │
│  SoC firmware (iç) + Host runtime (dış)                   │
│  Dil: Rust (her iki katman)                                │
└─────────────────────────────────────────────────────────────┘
```

---

## Bölüm 2 — Bileşen Bileşen Karar ve Gerekçe

### 2.1 Volt Derleyici Önyüzü (Lexer, Parser, CST, AST, HIR)

**Seçilen dil: Rust**

```
Neden Rust:
  ✓ Bellek güvenliği zorunlu (devasa AST'ler işlenirken)
    → C++: use-after-free, double-free kaçınılmaz
    → GC'li diller: latency + bellek kontrolü kaybı
    → Rust: ownership modeli bu riski sıfırlar

  ✓ cstree, logos, tower-lsp — ekosistem hazır
    → Tekerlek icat etmeye gerek yok
    → rust-analyzer'ın kanıtlanmış desenleri takip edilir

  ✓ FFI: MLIR C-API'ye temiz bağlanır
    → `extern "C"` blokları + bindgen

  ✓ Performans: büyük tasarımda derleme süresi
    → Arena tahsisi + Idx<T> handle modeli

Dezavantajı:
  ✗ Borrow checker, döngüsel IR yapılarında savaş
    → Çözüm: arena + indeks deseni (Idx<T>)
    → HIR'de Raw pointer değil, Idx<NodeType>

  ✗ Compile süresi uzun (büyük crate'lerde)
    → Çözüm: workspace'i küçük crate'lere böl
    → cargo nextest ile paralel test

  ✗ Öğrenme eğrisi yüksek
    → Ama derleyici mühendisi için zorunlu yatırım
```

**Neden diğerleri değil:**

```
OCaml (GHC alternatifi, derleyici için popüler):
  ✓ Pattern matching mükemmel (AST dönüşümü için)
  ✓ Haskell/OCaml derleyici geleneği güçlü
  ✗ MLIR/CIRCT ekosistemi Rust/C++ odaklı
  ✗ FFI Rust kadar temiz değil
  ✗ Topluluk küçük → CIRCT yetkinliği bulmak zor

Haskell:
  ✓ Tip sistemi mükemmel (Volt'un tip sistemini modellemek için)
  ✗ Lazy evaluation → bellek davranışı tahmin edilemez
  ✗ GHC runtime → gömülü veya WASM hedefi zor
  ✗ Ekosistem izolasyonu

Go:
  ✓ Basit, hızlı derleme
  ✗ Generics hâlâ olgunlaşıyor
  ✗ GC → latency spikes
  ✗ Donanım tanımlama araçları topluluğu yok

C++:
  ✓ LLVM/CIRCT bu dilde yazılı → natif entegrasyon
  ✗ Bellek güvenliği garantisi yok
  ✗ Hata mesajı kalitesi için ilave çaba
  ✗ Volt'un "güvenlilik kültürü" C++ ile çelişir
```

---

### 2.2 CIRCT / MLIR Köprüsü (Arka Uç)

**Seçilen dil: Rust + MLIR C-API**

```
CIRCT/MLIR C++ ile yazılı — doğrudan Rust↔C++ FFI tehlikeli:
  → C++ ABI stabil değil (name mangling, vtable)
  → LLVM sürüm geçişlerinde kırılma garantili

Güvenli yol: MLIR C-API
  → LLVM ekibi C-API'yi stabil tutmayı taahhüt ediyor
  → `bindgen` ile Rust binding otomatik üretilir
  → Tüm CIRCT işlemleri C-API üzerinden

Alternatif: `melior` (Rust'tan MLIR için wrapper)
  → Daha yüksek seviye abstraction
  → Ama CIRCT'nin tüm lehçelerini kapsamıyor
  → MVP için pragmatik başlangıç: melior + elle yazım

Mimari karar:
  CIRCT bağımlılığı tek bir crate'te izole edilir (volt-lower)
  Değişim maliyeti kapsüllenir
  Geleceğe not: CIRCT yerine doğrudan FIRRTL veya özel IR
  gerekirse yalnızca volt-lower değişir, geri kalan etkilenmez
```

---

### 2.3 LSP Sunucusu ve Araçlar

**Seçilen dil: Rust**

```
tower-lsp: Rust'ta olgun, async LSP framework
  → VS Code, Neovim, Emacs eklentileri için yeterli
  → LSP protokolü JSON-RPC → tower'ın async runtime'ı

volt fmt: Rust
  → Derleyici CST'sini kullanır → round-trip garantisi
  → Bağımsız çalışır, hata durumunda bile format eder

volt lint: Rust
  → HIR üzerinden donanım anti-pattern tespiti

Playground (WASM hedefi):
  → Rust → wasm-bindgen → WASM
  → Tarayıcıda `volt build` çalışır, kurulum yok
  → React/TypeScript ön yüz (editör: Monaco)
  → Bu Typescript → gerekçe: geniş web geliştirici kitlesi
     tarayıcı eklenti ekosistemi

Paket yöneticisi (volt.toml):
  → Rust — cargo'nun tasarım kararlarından öğrenilmiş
  → Sürüm çözümleme: PubGrub algoritması
     (cargo'nun kullandığı, SAT'ten daha iyi hata mesajları)
```

---

### 2.4 NeuroTorch (Spike-Uyumlu Eğitim Çerçevesi)

**Seçilen dil: Python + Rust çekirdek**

```
Neden Python önyüz:
  ✓ PyTorch ekosistemi — araştırmacı alışkanlığı
  ✓ NumPy, matplotlib, Jupyter — veri bilimi araçları
  ✓ Hızlı prototipleme — araştırmacı verimliliği

Neden Rust çekirdek:
  ✓ Spike simülasyonu performans kritik
  ✓ Donanım kısıtı denetimi tip-güvenli olmalı
  ✓ PyO3: Python↔Rust bindings temiz

Mimari:
  Python API:
    Population(), connect(), SpikeEncoder — kullanıcıya açık
    Volt hardware_descriptor okuma → donanım kısıtı Python'a taşınır

  Rust çekirdek:
    Spike event queue (öncelikli kuyruk, nanosaniye hassas)
    STDP ağırlık güncelleme (sıkı döngü)
    Volt IR üretici (Rust → çıktı: topoloji IR)

Neden TensorFlow veya JAX değil:
  → PyTorch: araştırma standardı, esnek
  → JAX: fonksiyonel, ama spike zamanlaması state gerektiriyor
  → TF: fazla ağır, araştırma kitlesi uzak

Eksik olan ve inşa edilecek:
  → Spike-uyumlu geri yayılım (surrogate gradient)
     Python'da prototip, Rust'ta optimize
  → Volt donanım kısıtı → eğitim döngüsü geri besleme
     Bu köprü tamamen yeni, mevcut frameworklerde yok
```

---

### 2.5 NeuroCompiler (Topoloji → Çip Eşleme)

**Seçilen dil: Rust + Python API**

```
Rust çekirdeği:
  Kısıt çözücü (constraint satisfaction):
    Topoloji düğümü → paradigma eşleme (NP-zor)
    Çözüm: greedy + yerel arama heuristik
    Rust: performans kritik, büyük ağlarda yavaş Python kabul edilemez

  Volt IR okuyucu:
    Topoloji IR → dahili graf yapısı
    Hardware descriptor → kısıt listesi

  Hedef arka uçlar:
    → Volt RTL üretici (Rust, volt-lower'ı çağırır)
    → Loihi 2 eşleyici (Intel Lava API, Python)
    → Memristör crossbar (araştırma, Python/C++)

Python API:
  Kullanıcıya açık: compile(network, target="fpga")
  Görselleştirme: eşleme grafiği, kaynak kullanımı
  Loihi 2 Lava API köprüsü (Intel'in Python SDK'sı)

ML tabanlı eşleme iyileştirme:
  → PyTorch ile öğrenilen heuristik
  → Tarihsel eşleme verisi → daha iyi tahmin
  → Python katmanında, Rust çekirdeğine öneri verir
```

---

### 2.6 Spike Kodlama Kütüphanesi

**Seçilen dil: Rust + Python binding**

```
Rust çekirdeği:
  Rate encoder, temporal encoder, delta encoder
  Nanosaniye hassasiyetli timestamp
  Volt tip sistemi: RateCode<max_hz, window_ms>
  → Kodlama şeması tip olarak taşınır

Python binding (PyO3):
  encode(image_data, method="rate", max_hz=100)
  → Araştırmacı dostu API

Neden C/C++ değil:
  → Rust: bellek güvenliği + Python binding kolaylığı (PyO3)
  → Spike event queue lock-free implementation: Rust ideal

Neden sadece Python değil:
  → Büyük ölçekli simülasyonda (1M nöron) Python yavaş
  → Kritik döngü mutlaka Rust veya C'de
```

---

### 2.7 Profiler ve Hata Ayıklama

**Seçilen dil: Rust çekirdek + Python görselleştirme**

```
Rust çekirdeği:
  Spike raster toplayıcı (lock-free ring buffer)
  Volt assert çalışma zamanı monitörü
  Enerji ölçüm modülü
  VCD/FST dalga formu üretici

Python görselleştirme:
  Matplotlib / Plotly: spike raster, enerji profili
  Jupyter Notebook entegrasyonu
  Pandas: zaman serisi analizi

Web tabanlı görselleştirme:
  TypeScript + D3.js: interaktif spike raster
  WebSocket: gerçek zamanlı donanım akışı
  → WASM Rust çekirdeği ile tarayıcıda
```

---

### 2.8 Formal Doğrulama Katmanı

**Seçilen dil: K framework + SMT-LIB + Rust köprüsü**

```
K framework:
  Volt semantiğini resmîleştirmek için
  K dili: rewriting mantığı tabanlı
  → Volt'un "tek semantik modeli" burada kanıtlanır
  → CIRCT lowering doğruluğu: kaynak = hedef anlam

  Neden K:
  ✓ LLVM, EVM, Java semantiği K ile resmîleştirilmiş
  ✓ Model checker otomatik türetilir
  ✓ Mevcut donanım HDL araştırması K kullanıyor

SMT-LIB (SymbiYosys köprüsü):
  Volt assert/invariant → SMT sorgusu
  Z3, Bitwuzla çözücüler: bounded model checking
  Rust köprüsü: SMT-LIB üretimi otomatik

Coq/Lean (opsiyonel, uzun vade):
  Tam kanıt için (bounded değil, sonsuz)
  → Akademik araştırma için uygun
  → MVP için gerekli değil
```

---

### 2.9 Mimari İşletim Sistemi

**Seçilen dil: Rust (her iki katman)**

**SoC iç katmanı (firmware):**

```
Rust (no_std + embedded-hal):
  ✓ Bare-metal: işletim sistemi yok, doğrudan donanım
  ✓ Bellek güvenliği: heap allocation olmadan
  ✓ RTIC (Real-Time Interrupt-driven Concurrency):
    spike yönlendirme için interrupt-driven model
  ✓ ARM Cortex-M hedefi: probe-rs ile flash

Neden C değil:
  → Bellek güvenliği kritik (güvenlik-kritik SoC)
  → Rust embedded ekosistemi C ile rekabet edecek olgunlukta
  → MISRA-C'nin Rust karşılığı: ownership modeli

Neden Zephyr/FreeRTOS değil:
  → Genel RTOS fazla ağır
  → RTIC: lightweight, zero-cost abstractions
  → Özel SoC'a özel firmware daha iyi kontrol
```

**Host dış katmanı:**

```
Rust (std, async tokio):
  ✓ Thunderbolt/PCIe sürücü katmanıyla iletişim
  ✓ Async: birden fazla SoC'u eş zamanlı yönet
  ✓ Volt IR parse + görev paketleme
  ✓ ML tabanlı eşleme öğrenmesi için Python FFI

Python arayüzü (opsiyonel):
  → Kullanıcıya açık high-level API
  → "schedule_task(model, constraints)" gibi
  → PyO3 Rust↔Python
```

---

### 2.10 Fotonik EDA (Volt v3, Uzun Vade)

**Seçilen dil: Rust + CIRCT fotonik lehçesi**

```
Waveguide yönlendirme:
  Rust: özel yönlendirme algoritması
  → Elektriksel PCB routing'den farklı kısıtlar:
    eğrilik yarıçapı, çapraz geçiş kayıpları,
    termal sapma modeli
  → Mevcut EDA araçları (Cadence, Synopsys) yok
  → Sıfırdan inşa: Rust ideal

WDM kanal yönetimi:
  Rust: dalga boyu atama, çarpışma tespiti

Termal sapma simülasyonu:
  Python + SciPy: ısı denklemleri, FEM

CIRCT fotonik lehçesi (yeni):
  MLIR'e katkı: `photonic` dialect
  PTrit operasyonları, MZI primitifleri
  → CIRCT topluluğuyla işbirliği gerekli
```

---

## Bölüm 3 — Mevcut Diller Nerede Yetersiz Kalıyor

Bu analizi yapmak projenin en derin sorusuna yanıt veriyor:
**Volt neden var olmak zorunda?**

### Yetersizlik 1 — Donanım Paradigmaları İfade Edilemiyor

```
Python, Rust, C++, Java — hepsi Von Neumann varsayımı üzerine:
  → Değişken = bellekte bir yer
  → Fonksiyon = sıralı komutlar
  → Tip = veri yapısı

Donanım gerçekliği farklı:
  → Sinyal = saat alanında bir değer @ClockDomain
  → Blok = eş zamanlı çalışan donanım
  → Tip = bit genişliği + işaretlilik + saat alanı

Hiçbir mevcut dil şunu yapamaz:
  bool @FastDomain + bool @SlowDomain → derleme hatası
  u8 + u8 → u9 (taşma korunur)
  reg(Sys) value: u8 = 0 (reset değeri zorunlu)

→ Bu ifade gücü için yeni bir dil gerekir
→ Bu Volt'un var olma nedenidir
```

### Yetersizlik 2 — Spike Zamanlaması Hiçbir Dilde Birinci Sınıf Değil

```
Mevcut yaklaşım:
  Python: spike_time = float (sadece sayı)
  C++: struct Spike { double time; float value; }
  Rust: struct Spike { time: Duration, value: f32 }

Sorun:
  Hangi saat alanında? → bilinmiyor
  Gecikme hizalaması? → çalışma zamanı hatası
  CDC güvenliği? → yok

Volt v2 ile:
  Spike<T> @NeuromorphicDomain
  Delayed<Spike<T>, 3>
  → Tip sistemi hizalamayı garanti eder
  → Mevcut hiçbir dil bunu yapamıyor
```

### Yetersizlik 3 — Dengeli Üçlü Hiçbir Dilde Yerel Değil

```
Her modern dil binary varsayar:
  bool: true/false
  int8: -128..127
  Ternary: kütüphaneyle simüle edilebilir, ama...

Simülasyonun sorunu:
  type Trit = i8 // -1, 0, 1 değerleri kullanılır
  Ama derleyici -128..127 aralığının tamamını allocate eder
  Tip sistemi: "Trit + Trit → Trit" doğrulanamaz
  Hardware eşleme: derleyici FeFET primitifini bilmez

Volt'ta:
  type Trit = i2 // gerçekten 2 bit
  Trit + Trit → tanımlı, taşma korunur
  @PhotonicTernary ile MZI primitifine eşlenir
  → Mevcut hiçbir dil bu eşlemeyi yapamıyor
```

### Yetersizlik 4 — Fotonik Sinyaller Hiçbir Dilde Yoktur

```
Mevcut:
  Elektriksel sinyal: float/int (gerilim/akım sayısı)
  Dijital: bool/bit

Fotonik gerçeklik:
  E(t) = A × e^(iφ) — kompleks alan
  Dalga boyu: 1310nm veya 1550nm (WDM kanalı)
  Domain: @PhotonicTernary ayrı bir fiziksel ortam

  WDM çoğullama → bir "kablo"da birden fazla sinyal tipi
  E-O-E dönüşümü → enerji maliyeti varolan bir gerçek

Hiçbir dil bunu ifade etmiyor:
  OpticalSignal @Photonic{λ=1550.nm}
  → "Bu sinyal 1550nm kanalında"
  bridge ElectroOptic → tip sistemi zorlar

→ Volt v3 bu boşluğu dolduracak
→ Mevcut EDA araçları fotonik için kör
```

### Yetersizlik 5 — Paradigmalar Arası Geçiş Güvensiz

```
Mevcut durumda:
  Nöromorfik → Dijital geçişi: "ne döndüreceğiz?"
  Fotonik → Dijital: ADC mı? Faz tespiti mi?
  Hepsi elle, belgeleme yok, hata sessiz

Volt'ta:
  bridge SpikeToDigital {
    in  : SpikeStream @Neuromorphic
    out : Tensor<f32> @Digital
    method = PopulationVector
    @cost(latency=10.us, energy=50.nJ)
    assert: information_loss < 0.05
  }

  → Geçiş stratejisi tip sisteminde
  → Maliyet görünür
  → Bilgi kaybı formal doğrulanabilir
  → Mevcut hiçbir dil bunu yapamıyor
```

---

## Bölüm 4 — Hangi Mevcut Dil Neyi İyi Yapıyor (Özet)

```
Dil        Güçlü olduğu alan               Volt ekosistemindeki rolü
──────────────────────────────────────────────────────────────────────
Rust       Derleyici, LSP, firmware,        Temel uygulama dili
           performans-kritik çekirdek       (çoğunluk burası)

Python     Araştırma API'si, görselleştirme, NeuroTorch, NeuroCompiler
           ML eğitim çerçevesi             API, profiler görselleştirme

TypeScript Web ön yüz, IDE eklentisi        Playground UI, VS Code ext

K dili    Resmî semantik, model checking   Volt semantik kanıtı

SMT-LIB   Formal çözücü sorgu dili         assert/invariant → BMC

C-API     Stabil FFI sınırı               Rust↔MLIR köprüsü

Volt      Donanım tanımı                  Kullanıcının yazdığı dil
(yeni)    (RTL, nöromorfik, fotonik)      (tüm donanım bloklarını tanımlar)
```

---

## Bölüm 5 — Kritik Mimari Karar: Rust Neden Merkezde

Proje için tek bir uygulama dili seçmek zorunda olsaydık Rust olurdu.
Neden:

```
1. Derleyici + LSP + firmware + arka uç → tek dil ekibi
   → Farklı bileşenler arasında geçiş sürtünmesi az
   → Ortak araçlar (clippy, cargo, nextest)
   → Ortak kültür (hata mesajı kalitesi, güvenlik)

2. Bellek güvenliği kritik → Rust
   → Derleyici AST'leri büyük ve karmaşık
   → Firmware bare-metal → heap yönetimi hassas
   → C++ bu garantiyi veremiyor

3. FFI her yönde çalışıyor:
   → Python ↔ Rust: PyO3 (NeuroTorch, profiler)
   → C-API ↔ Rust: bindgen (CIRCT)
   → WASM ↔ Rust: wasm-bindgen (playground)
   → Rust merkez, diğerleri çevre

4. Ekosistem Volt'un değerleriyle örtüşüyor:
   → "Yanlış kod derlenmez" kültürü
   → Mükemmel hata mesajları (Elm'den ilham)
   → Zero-cost abstractions (donanım için kritik)
   → Güvenlik önce, sonra kolaylık
```

---

## Bölüm 6 — Açık Sorular ve Gelecek Dil Gereksinimleri

### Soru 1 — Ternary Hesaplama için Native Dil Gerekiyor mu?

```
Bugün: Trit tipi Volt'ta i2 olarak ifade ediliyor
Sorun: Rust simülasyonu binary makinede çalışır
       → FeFET üçlü simülasyonu yazılımda pahalı

Gelecek olasılık:
  Ternary bir üst dil → Volt'a compile
  (Setun BESM dilinin modern karşılığı)
  → Matematiksel ifadeleri dengeli üçlü aritmetiğiyle yaz
  → Volt fotonik ternary primitiflerine lowering

Bu henüz araştırma sorusu — MVP'de gerekli değil.
```

### Soru 2 — Nöromorfik Spesifikasyon için Özel Notasyon Gerekiyor mu?

```
Mevcut: NeuroLang → Python API → Volt RTL
Sorun: Python tip-güvenli değil →
       topoloji hatası çalışma zamanında

Gelecek olasılık:
  Volt'un üstünde (veya içinde) formal topoloji notasyonu:
  population V1 : LIF[1000] @Spike<Trit>
  connect V1 → V2 : STDP, weight=Trit
  → Derleme zamanı topoloji doğrulama

Bu Volt v2'nin bir parçası olabilir.
Ayrı dil değil, Volt uzantısı.
```

### Soru 3 — Fotonik EDA için Özel Dil Gerekiyor mu?

```
Mevcut EDA dilleri (SKILL, Ocean, SKILL++):
  → Analog tasarım için (SPICE odaklı)
  → Fotonik için hiçbir standard yok

Volt v3 fotonik domain'i ekliyor ama:
  → Waveguide yönlendirme kompleks geometri problemi
  → Termal sapma: diferansiyel denklemler
  → WDM kanal yönetimi: frekans alanı

Büyük olasılık:
  Volt, fotonik tanım için yeterli (domain + primitif)
  Waveguide physical layout → ayrı araç (Volt entegrasyonlu)
  → Yeni bir dil değil, Volt'a bağlanan araç
```

---

## Özet: Dil Kararları Tablosu

```
Bileşen                    Dil               Gerekçe
────────────────────────────────────────────────────────────────
Volt derleyici önyüzü      Rust              Bellek güvenliği + ekosistem
CIRCT köprüsü              Rust + C-API      Stabil FFI sınırı
LSP sunucusu               Rust (tower-lsp)  Async, olgun framework
Playground UI              TypeScript + WASM  Web standardı
volt fmt / lint            Rust              Derleyici CST'si paylaşımı
NeuroTorch API             Python + Rust     Ekosistem + performans
NeuroCompiler çekirdeği    Rust              Kısıt çözücü performansı
Spike kodlama kütüphane    Rust + PyO3       Hız + Python erişim
Profiler çekirdeği         Rust              Lock-free veri toplama
Profiler görselleştirme    Python + D3.js    Araştırmacı arayüzü
Formal semantik            K framework       Rewriting mantığı
Formal çözücü              SMT-LIB + Z3      Standart BMC
SoC firmware               Rust (no_std)     Bare-metal güvenliği
Host mimari OS             Rust (async)      Performans + güvenlik
Fotonik EDA (v3)           Rust + yeni CIRCT Sıfırdan inşa

Kullanıcının yazdığı dil:  Volt (yeni)       Mevcut diller yetersiz
```

**En önemli sonuç:**

Volt ekosistemini *inşa eden* diller için mevcut diller yeterli —
Rust merkeze, Python araştırma katmanına, TypeScript web katmanına,
K framework formal katmana yerleşiyor.

Ama Volt'un *kendisi* — kullanıcının donanımı tanımladığı dil —
yeni olmak zorunda. Saat alanı tipte, spike zamanlaması tipte,
dengeli üçlü tipte, fotonik sinyal tipte — bunların hiçbiri
mevcut hiçbir dilde birinci sınıf. Bu boşluk Volt'u zorunlu kılar,
tercih değil.
