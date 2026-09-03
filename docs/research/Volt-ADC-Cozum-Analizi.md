> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# ADC Sorunu — Kapsamlı Çözüm Analizi

> Analog-Dijital dönüşüm sorunu tek bir problem değil, farklı fiziksel
> sınırların farklı bağlamlarda ortaya çıkmasıdır. Bu belge mevcut üç
> çözümü derinlemesine analiz eder, yeni yaklaşımlar ekler ve hangi
> durumda hangi çözümün kullanılacağını belirler.

---

## Bölüm 1 — Sorunun Gerçek Anatomisi

ADC sorunu üç ayrı sınırda ortaya çıkar ve her biri farklı karakterdedir:

```
Sınır 1 — Nöromorfik → Dijital:
  Analog membran potansiyeli → sayısal değer
  Karakter: seyrek, zamansal, düşük hassasiyet yeterli
  Enerji bütçesi: mikrowatt-miliwatt

Sınır 2 — Fotonik → Dijital:
  Optik alan (genlik + faz) → sayısal değer
  Karakter: yüksek bant genişliği, koherent
  Enerji bütçesi: pikojoul/bit

Sınır 3 — Analog CAM → Dijital:
  Benzerlik skoru (sürekli) → en yakın indeks
  Karakter: tek karşılaştırma sonucu, düşük çözünürlük yeterli
  Enerji bütçesi: femtojoul

Her sınır için ayrı bir ADC felsefesi gerekir.
```

---

## Bölüm 2 — Mevcut Üç Çözümün Derin Analizi

### Çözüm 1 — Seyrek Çıkış (Spike Tabanlı)

**Nasıl çalışır:**

```
Nöron ateşlemediğinde:    ADC devresi kapalı → 0 enerji
Nöron ateşlediğinde:      Spike olayı → dijital darbe → zaman damgası

Dönüştürülen şey amplitude DEĞİL, zamanlama:
  "Bu nöron t=47.3ms'de ateşledi"
  → Sayısal değil, olay (event)
  → ADC'ye gerek yok — dijital sayaç yeterli

Enerji hesabı:
  Geleneksel: 10,000 nöron × 100fJ × 1GHz = 1W
  Spike: 10,000 × %5 × 100fJ × ateşleme_hızı = 50mW ✓
```

**Gerçek sınırları:**

```
✓ Güçlü: Zamansal kodlama (timing-based) için ideal
✓ Güçlü: %99 seyreklikte neredeyse sıfır ADC maliyeti
✗ Zayıf: Rate coding için hâlâ yüksek ateşleme sayılması gerekir
✗ Zayıf: Çok sayıda nöron eş zamanlı ateşlerse (senkronizasyon)
          spike çakışması → dijital event queue dolar
✗ Zayıf: Düşük SNR — gürültülü analog → yanlış ateşleme
          → yanlış spike → yanlış bilgi
```

**Ne zaman kullan:**

```
→ Temporal coding baskın (zaman = bilgi)
→ Ateşleme oranı gerçekten düşük (%1-5)
→ Duyusal ön işleme (ses, dokunma, hareket)
→ Anomali tespiti (normal = sessiz, anomali = spike)
```

---

### Çözüm 2 — Yerel Dijitalleştirme

**Nasıl çalışır:**

```
Her nöronun yanında küçük, özel ADC:
  Merkezi ADC: 10,000 nöronu sırayla dönüştürür → yavaş
  Yerel ADC: her nöron kendi değerini anında dönüştürür

Hassasiyet: 4-bit yeterli
  Neden: nöral aktivasyonlar kaba taneli dağılır
         16 seviye yeterince ayrıştırıcı
  Alan: 4-bit ADC ~0.001mm² (7nm'de) → 10,000 nöron = 10mm²

Mimari:
  ┌──────────────────────────────────┐
  │  Nöron 1  │ ADC │ → dijital     │
  │  Nöron 2  │ ADC │ → dijital     │
  │  ...       │ ... │               │
  │  Nöron N  │ ADC │ → dijital     │
  └──────────────────────────────────┘
  Paralel dönüşüm → tek saat döngüsünde hepsi
```

**Gerçek sınırları:**

```
✓ Güçlü: Her cycle tam nüfus vektörü → rate coding için ideal
✓ Güçlü: Merkezi ADC darboğazı yok
✗ Zayıf: Alan maliyeti yüksek — N nöron × ADC alanı
✗ Zayıf: Her ADC güç tüketiyor — nöron suskunken bile
          → Seyrek durumda Çözüm 1'den çok daha pahalı
✗ Zayıf: 4-bit hassasiyet bazı görevler için yetersiz
          (özellikle keskin karar sınırı gereken sınıflandırma)
```

**Ne zaman kullan:**

```
→ Rate coding baskın (ateşleme frekansı = bilgi)
→ Ateşleme oranı yüksek (>%30)
→ Nüfus vektörü her cycle gerekli
→ Downstream dijital işlem yoğun (transformer dikkat katmanı)
→ Alan bütçesi bol, güç bütçesi orta
```

---

### Çözüm 3 — Tam Analog Zincir

**Nasıl çalışır:**

```
Analog giriş → Analog nöromorfik → Analog çıkış → Actuator
                                                 (motor, ses, ışık)

ADC hiç yok — fiziksel dünya zaten analog:
  Motor kontrol: akım → tork → hareket (hepsi analog)
  Ses çıkışı: voltaj → hoparlör → ses dalgası
  Işık kontrolü: gerilim → LED parlaklığı

Özel durum: Analog-Analog amplifikasyon
  Zayıf nöral sinyal → analog yükselteç → güçlü actuator sinyali
  Hiçbir dijital dönüşüm yok
```

**Gerçek sınırları:**

```
✓ Güçlü: ADC enerjisi sıfır (kavramsal olarak)
✓ Güçlü: Gecikme minimax — en hızlı tepki
✓ Güçlü: Robotik refleks için biyolojik nöral devreye en yakın
✗ Zayıf: Programlanabilirlik çok düşük
          Tasarım değiştirilince fiziksel devre değişmeli
✗ Zayıf: Gürültü birikimi — uzun analog zincirde SNR düşer
✗ Zayıf: LLM, NLP, görüntü işleme — imkânsız
          Bu görevler sayısal hassasiyet gerektiriyor
✗ Zayıf: Kalibrasyonu zor — sıcaklık, yaşlanma etkisi
```

**Ne zaman kullan:**

```
→ Refleks döngüsü (< 1ms tepki zorunlu)
→ Sabit, önceden bilinen görev (yeniden programlama yok)
→ Güç bütçesi kritik sınırda (implantable cihaz)
→ Basit duyusal-motor döngü (denge, yürüyüş)
```

---

## Bölüm 3 — Yeni Çözümler: Üç Mevcut Yaklaşımın Ötesi

Mevcut üç çözüm iyi bir başlangıç ama eksik kalan dört kritik yaklaşım var:

---

### Çözüm 4 — Zaman-Dijital Dönüşüm (TDC — Time-to-Digital Converter)

**Temel fikir:** Amplitüdü değil, zamanı dijitalleştir.

```
Geleneksel ADC:          TDC:
  "Bu sinyalin         "Bu olay ne zaman oldu?"
   değeri kaç?"
  → Analog → sayı      → Zaman → sayı

  Voltaj ölçmek         Gecikme saymak
  hassas → pahalı       basit → ucuz

Nasıl çalışır:
  Referans saat:  ████████████████  (yüksek frekanslı)
  Spike olayı:         ↑
  Sayaç:          0123456789...
  TDC çıkışı:    "7. saat kenarında oldu"

  Sonuç: spike zamanı sayısal olarak temsil edildi
  Amplitude ölçüldü mü? → Hayır, gerek yok (temporal coding'de)
```

**Enerji karşılaştırması:**

```
SAR ADC (12-bit, 1MS/s):    ~10 pJ/dönüşüm
Sigma-Delta ADC:             ~5 pJ/dönüşüm
TDC (10-bit, pikosaniye):   ~0.1-0.5 pJ/dönüşüm

TDC neden daha ucuz:
  → Analog devre yok (sadece dijital sayaç)
  → Üretim sapması minimal etkiler
  → Ölçeklenme mükemmel (teknoloji küçüldükçe daha hızlı)
```

**Sınırları:**

```
✓ Spike zamanlaması için birebir doğal eşleme
✓ Çok düşük enerji
✓ Tam dijital — analog kalibrasyon gerektirmez
✗ Yalnızca temporal coding için — amplitude bilgisi yok
✗ Çok sayıda eş zamanlı spike → TDC arbitrasyon gerekir
✗ Jitter (saat titremesi) → hassasiyet sınırı
```

**Volt'ta ifadesi:**

```volt
bridge SpikeToTDC {
    in  spike    : Spike<bool>    @NeuromorphicDomain
    out timestamp: u32            @DigitalDomain

    // Amplitude ölçülmüyor — yalnızca zaman
    @tdc_resolution(10.ps)       // 10 pikosaniye hassasiyet
    @cost(energy=0.2.pJ, latency=1.ns)

    // Volt tip sistemi garantisi:
    // Bu dönüşüm yalnızca temporal coding için geçerli
    #[requires(encoding = TemporalCode)]
}
```

**Ne zaman kullan:**

```
→ Spike zamanlaması kritik (STDP, temporal pattern)
→ Çok düşük güç bütçesi (wearable, implant)
→ Amplitude bilgisine ihtiyaç yok
→ Yüksek frekans gerekmiyor (< 100MHz spike hızı)
```

---

### Çözüm 5 — Stokastik Hesaplama (Olasılıksal Kodlama)

**Temel fikir:** Değeri bit akışının olasılığıyla temsil et.

```
Geleneksel:  0.75 → 0011 (4-bit ikili)
Stokastik:   0.75 → 1101 1011 1110... (bit akışı, %75 '1')

Nasıl çalışır:
  Nöron aktivasyonu = 0.75
  Stokastik üreteç: her cycle %75 olasılıkla '1' üretir
  → Dijital işlemci yalnızca 1-bit sinyaller görür

Avantaj:
  ADC karmaşıklığı yok — sadece comparator (1-bit ADC)
  Çarpma: A × B = AND kapısı (1-bit AND)
  Toplama: A + B = OR kapısı (yaklaşık)
  → Çarpma 1 AND kapısı → 0.01 pJ
  → Geleneksel INT8 çarpma → 0.5 pJ → 50x daha verimli

Dezavantaj:
  Hassasiyet için uzun bit akışı gerekir:
    8-bit hassasiyet → 256 cycle bekle
    16-bit hassasiyet → 65,536 cycle bekle
  → Yavaş: yüksek hassasiyet pahalı
```

**Doğal kullanım alanı:**

```
Nöromorfik + stokastik:
  Nöron ateşleme oranı = stokastik değer zaten
  %75 ateşleme → 0.75 olasılıklı bit akışı → doğrudan

  Biyoloji bu yöntemi kullanıyor:
  Beyin nöronları rate code ile çalışıyor
  → Stokastik bilgisayar beynin doğal karşılığı

Gerekli hassasiyet düşük olduğunda:
  Anomali tespiti: "normal mi değil mi?" → 1-bit yeterli
  Ses sınıflandırma: 16 sınıf → 4-bit yeterli → hızlı
```

**Volt'ta ifadesi:**

```volt
type StochasticTrit = Probability<Trit>  // olasılıklı trit

bridge SpikeToStochastic<const StreamLen: usize> {
    in  spikes      : SpikeStream<StreamLen>  @NeuromorphicDomain
    out probability : f8                      @DigitalDomain

    // Bit akışı sayılır → olasılık
    @cost(energy=0.05.pJ_per_bit, latency=StreamLen.cycles)

    // Hassasiyet-hız ödünleşimi görünür
    // StreamLen = 256 → 8-bit hassasiyet
    // StreamLen = 16  → 4-bit hassasiyet (daha hızlı)
    assert: accuracy_bits == log2(StreamLen)
}
```

**Ne zaman kullan:**

```
→ Hassasiyet gereksinimi düşük (4-8 bit yeterli)
→ Rate coding baskın
→ Donanım basitliği öncelikli (AND/OR kapıları)
→ Güç çok kısıtlı (harvesting-based IoT)
```

---

### Çözüm 6 — Faz Alanı Hesaplama (Fotonik İçin Kritik)

**Temel fikir:** Amplitude'ü değil, fazı ölç — çok daha ucuz ve hızlı.

```
Geleneksel fotonik ADC:
  Optik güç → fotodedektör → analog akım → SAR ADC → dijital
  Her adım enerji ve gecikme

Faz tabanlı ölçüm:
  Referans ışık kaynağı (lokal osilatör)
  ↓
  Optik mikser (90° hibrit)
  ↓
  Dört çıkış: I+ I- Q+ Q-
  ↓
  Basit diferansiyel amplifikatör (analog)
  ↓
  Tek comparator (1-bit ADC yeterli faz tespiti için)
  ↓
  Dijital: +1, 0, -1 (ternary için mükemmel!)

Neden ternary fotonik için birebir:
  MZI çıkışı: 0°, 90°, 180° faz
  → Faz dedektörü: doğrudan +1, 0, -1 → ADC gerekmez
  → Üç durum doğal olarak tespit edilir
```

**Enerji karşılaştırması:**

```
Geleneksel fotonik ADC (8-bit):  10-50 pJ/dönüşüm
Koherent faz dedektörü:          0.1-1 pJ/dönüşüm
Faz tabanlı ternary tespit:      0.01-0.1 pJ/trit

Neden daha ucuz:
  → Faz karşılaştırma basit interferans = fizik yapıyor işi
  → ADC devresinin tamamı yerine tek comparator
  → Referans osilatör paylaşılabilir (tüm nöronlar için tek)
```

**Volt'ta ifadesi:**

```volt
bridge CoherentDetection {
    in  optical : PTrit       @PhotonicTernary
    in  lo      : LocalOscillator  // referans ışık
    out trit    : Trit        @DigitalDomain

    // Faz dedektörü — amplitude ADC değil
    @detector(type=coherent_90deg_hybrid)
    @lo_power(1.mW)                    // referans güç
    @cost(energy=0.05.pJ_per_trit)    // çok düşük

    // Ternary doğal tespit:
    // 0° → +1, 180° → -1, güç=0 → 0
    assert: detection_error < 0.001   // %0.1'den az hata
}
```

**Ne zaman kullan:**

```
→ Fotonik ternary (doğal eşleme)
→ Yüksek bant genişliği fotonik (>100GHz)
→ Düşük gecikme gerektiren optik hesaplama
→ WDM çok kanallı (her kanal için ayrı LO fazı)
```

---

### Çözüm 7 — Olay Güdümlü ADC (Event-Driven / Level-Crossing ADC)

**Temel fikir:** Sürekli örnekleme değil, sinyal eşiği aştığında örnekle.

```
Geleneksel ADC (Nyquist):
  t=0: ölç → 0.3
  t=1: ölç → 0.3  (değişmedi — boşa harcandı)
  t=2: ölç → 0.3  (değişmedi — boşa harcandı)
  t=3: ölç → 0.7  (değişti!)
  Her cycle enerji harcandı, çoğu boşa

Olay güdümlü ADC:
  "Sinyal Δ = 0.1'den fazla değiştiyse örnekle"
  t=0: 0.3 (başlangıç)
  t=3: 0.7 (+0.4 değişti → örnekle!)
  t=1,2: hiçbir şey → sıfır enerji

Enerji kazancı:
  Sinyalin %90'ı "sessiz" ise → %90 ADC enerjisi tasarruf
  Doğal nöromorfik sinyallerle mükemmel uyum
```

**Delta-Sigma varyantı:**

```
Normal Level-Crossing:  eşik aşılınca örnekle
Delta-Sigma:            değişim büyüklüğünü de kodla

  Sinyal: 0.3 → 0.7 (+0.4 değişim)
  Çıkış: (zaman=3, delta=+4) [4-bit çözünürlükte]

  Avantaj: Birikimli hata yok
  Dezavantaj: Kodlama daha karmaşık
```

**Nöromorfik ile doğal uyum:**

```
Biyolojik sensör (retina, koklea):
  Değişim olmadığında sessiz
  Değişim olduğunda spike
  → Zaten olay güdümlü!

Olay kamerası (Dynamic Vision Sensor):
  Her piksel bağımsız, parlaklık değişince spike
  → Olay güdümlü ADC'nin silikon uygulaması
  → Foveation: hızlı hareket olan bölge çok veri, durağan az

Volt'ta bu bilgi:
  in  retina_events : EventStream<DVSPixel>  @NeuromorphicDomain
  → Zaten event güdümlü — geleneksel ADC gerekmez
```

**Volt'ta ifadesi:**

```volt
bridge LevelCrossingADC<const Threshold: f32, const Bits: u8> {
    in  analog  : AnalogSignal   @AnalogDomain
    out events  : EventStream<i8> @DigitalDomain

    @threshold(Threshold)
    @resolution(Bits)
    @cost(energy_idle=0.pJ, energy_event=1.pJ)

    // Sessiz dönemde sıfır güç
    invariant on AnalogDomain:
        |delta(analog)| < Threshold → no_output
}
```

**Ne zaman kullan:**

```
→ Doğal olay tabanlı sensörler (DVS kamera, mikrofon)
→ Analog sinyal büyük ölçüde "sessiz" (> %80 zaman sabit)
→ Değişim bilgisi yeterli (mutlak değer gerekmiyor)
→ Enerji harvesting sistemler
```

---

## Bölüm 4 — Tüm Çözümlerin Karşılaştırmalı Matrisi

```
                    Spike   Yerel   Tam     TDC    Stok.  Faz    Olay
                    Tab.    Dijit.  Analog         Hes.   Alan   Güd.
────────────────────────────────────────────────────────────────────────
Enerji/dönüşüm     ~1pJ    ~5pJ    ~0pJ   ~0.2pJ ~0.1pJ ~0.05pJ ~0.5pJ
Hız                orta    yüksek  anlık  yüksek  düşük  çok yük. var.
Hassasiyet         düşük   orta    n/a    orta    düşük  yüksek  orta
Alan maliyeti      düşük   yüksek  düşük  düşük   düşük  orta    düşük
Programlanabilirlik orta   yüksek  çok dş yüksek  orta   yüksek  orta
Seyreklik avantajı ✅      ❌      ✅     ✅      ❌     ❌      ✅
Temporal coding     ✅      ❌      ✅     ✅      ❌     ❌      ✅
Rate coding         ❌      ✅      ❌     ❌      ✅     ❌      ❌
Fotonik uyum       ❌      ❌      ❌     ❌      ❌     ✅      ❌
Ternary uyum        ⚠️      ❌      ❌     ❌      ❌     ✅      ❌
────────────────────────────────────────────────────────────────────────
En iyi             Duyusal Edge AI  Robotik STDP   IoT    Fotonik Sensör
kullanım           önişlem  çıkarım refleks timing        ternary  DVS
```

---

## Bölüm 5 — Her Durum İçin Doğru Çözüm

### Durum Matrisi

```
Görev                          En İyi Çözüm        Yedek
───────────────────────────────────────────────────────────────────
Duyusal ön işleme (ses, hareket)  Spike tabanlı (1)  Olay güdümlü (7)
Anomali tespiti (fabrika)          Spike tabanlı (1)  Stokastik (5)
Wearable sürekli izleme           Spike tabanlı (1)  TDC (4)
BCI (beyin-bilgisayar arayüzü)   TDC (4)            Spike (1)
STDP öğrenme                      TDC (4)            Spike (1)
Edge AI çıkarım (yoğun)           Yerel dijital (2)  —
Rate coding ağlar                  Yerel dijital (2)  Stokastik (5)
Robotik refleks (<1ms)             Tam analog (3)     Olay güdümlü (7)
Implantable medikal               Tam analog (3)     Spike (1)
LLM attention (fotonik)           Faz alan (6)       Yerel dijital (2)
Fotonik ternary çıkarım           Faz alan (6)       —
DVS kamera işleme                  Olay güdümlü (7)  Spike (1)
Yüksek hassasiyet sinyal işleme   Sigma-Delta        —
Düşük maliyet IoT                  Stokastik (5)     Olay güdümlü (7)
Analog CAM sonucu                  Tek comparator     —
```

### Karar Ağacı

```
ADC çözümü seçmek için:

Sinyalin doğası ne?
├── Fotonik (optik)
│   ├── Ternary kodlanmış → Faz alan tespiti (6)
│   └── Binary/analog → Koherent alıcı + ADC
│
├── Nöromorfik (elektrik/analog)
│   ├── Temporal coding baskın?
│   │   ├── Evet → TDC (4) veya Spike tabanlı (1)
│   │   └── Hayır → Rate coding
│   │       ├── Seyreklik > %80?
│   │       │   ├── Evet → Spike tabanlı (1)
│   │       │   └── Hayır → Yerel dijital (2)
│   │       └── Hassasiyet < 4-bit yeterli?
│   │           ├── Evet → Stokastik (5)
│   │           └── Hayır → Yerel dijital (2)
│   │
│   └── Sinyal büyük ölçüde statik?
│       ├── Evet → Olay güdümlü (7)
│       └── Hayır → Duruma göre yukarı bak
│
└── Actuator doğrudan sürülüyor?
    ├── Evet → Tam analog (3)
    └── Hayır → Yukarı bak
```

---

## Bölüm 6 — Çözüm 8: Bağlamsal Adaptif ADC (Yeni Sentez)

Tek bir çözümün yetmediği yerde gerçek güç hibrit ve adaptif yaklaşımda:

**Fikir:** Mimari OS çalışma zamanında ADC yöntemini görev bağlamına göre seçer.

```
Görev analizi:
  "Bu ses tanıma görevi temporal mi rate mi?"
  "Bu veri akışı ne kadar seyrek?"
  "Downstream hassasiyet ihtiyacı nedir?"
        │
        ▼
ADC konfigurasyon kararı:
  Seyreklik > %80 → Spike tabanlı aktif, yerel ADC kapalı
  Seyreklik < %30 → Yerel ADC aktif, spike çıkışı kapalı
  Fotonik giriş  → Faz dedektörü aktif
  Robotik mod    → Tam analog yol, ADC tamamen by-pass

Donanım:
  Yeniden yapılandırılabilir ADC bloğu
  Her çözüm fiziksel olarak mevcut
  Mimari OS hangisini aktive edeceğine karar veriyor
  Kullanılmayan bloklar güç kapısıyla kapalı → sıfır enerji
```

**Volt'ta ifadesi:**

```volt
// Adaptif ADC — mimari OS tarafından yapılandırılır
module AdaptiveADC {
    in  analog_in  : AnalogSignal  @AnalogDomain
    in  config     : ADCConfig     @ControlPlane
    out digital_out: AdaptiveValue @DigitalDomain

    // Tüm ADC modları fiziksel olarak mevcut
    let spike_path  = SpikeBased()
    let local_path  = LocalDigital<4>()
    let tdc_path    = TimeToDigital<10>()
    let phase_path  = PhaseDetector()
    let event_path  = LevelCrossing<0.1>()

    // Mimari OS seçiyor
    on config.update {
        match config.mode {
            ADCMode::SpikeBased  => route(analog_in → spike_path → digital_out)
            ADCMode::LocalDigit  => route(analog_in → local_path → digital_out)
            ADCMode::TDC         => route(analog_in → tdc_path  → digital_out)
            ADCMode::Phase       => route(analog_in → phase_path → digital_out)
            ADCMode::Event       => route(analog_in → event_path → digital_out)
            ADCMode::FullAnalog  => bypass(analog_in → analog_out)
        }
    }

    // Volt doğrulaması: seçilen mod ve sinyal uyumlu mu?
    invariant: compatible(config.mode, analog_in.characteristics)
    // Örn: temporal coding sinyaline yerel dijital seçilirse hata
}
```

**Kazanç:**

```
Statik seçim (tek çözüm):
  Ortalama durum için optimize
  En kötü durum: kötü uyum → %10x fazla enerji veya düşük kalite

Adaptif seçim:
  Her görev için optimal
  Enerji: teorik minimuma yakın
  Kalite: her görev için yeterli hassasiyet

Örnek kazanım:
  Ses tanıma:
    Sessiz: %80 zaman → spike tabanlı → 50mW
    Konuşma: %20 zaman → yerel dijital → 200mW
    Ortalama: 0.8×50 + 0.2×200 = 80mW

  Statik yerel dijital seçilseydi: 200mW sürekli
  Adaptif: 80mW → 2.5x enerji tasarrufu
```

---

## Bölüm 7 — Volt Ekosisteminde ADC Katmanının Yeri

```
ADC çözümü seçimi iki yerde yaşar:

1. Tasarım zamanı (Volt tip sistemi):
   Bridge tipindeki @cost anotasyonu
   Hardware descriptor'daki ADC kabiliyeti
   Tip uyumluluk kontrolü (temporal → TDC zorunlu)

2. Çalışma zamanı (Mimari OS):
   Görev profiline göre ADC modu seçimi
   Gerçek enerji ölçümü → model güncelleme
   Volt invariant ihlali → ADC modu değiştir

Volt'un katkısı:
   ADC dönüşüm maliyetini görünür kılmak:
   bridge SpikeToDijital { @cost(5.pJ) }
   → Derleyici toplam enerji bütçesini hesaplar
   → "Bu tasarım 10W bütçeyi aşıyor" → derleme uyarısı

   Tip uyumsuzluğunu engellemek:
   Temporal coded signal → rate-coded ADC → derleme hatası
   "E0501: signal encoding mismatch:
    input is TemporalCode, but ADCMode::LocalDigit
    expects RateCode. Use TDC or SpikeBased instead."
```

---

## Özet

```
Soru: "Her durum için tek çözüm yeterli mi?"
Cevap: Hayır — ve bu beklenenden daha derin bir neden:

ADC sorunu tek bir problem değil:
  → Nöromorfik-Dijital sınırı: temporal/rate ayrımı
  → Fotonik-Dijital sınırı: koherent algılama
  → Analog CAM-Dijital: tek comparator yeterli

Her sınır farklı fizik:
  → Elektriksel seyrek sinyal: Spike tabanlı veya TDC
  → Optik faz bilgisi: Koherent faz dedektörü
  → Stokastik nöral: Olasılıksal kodlama

Doğru yaklaşım:
  Statik tek çözüm → ortalama performans
  Adaptif Çözüm 8 → her bağlamda optimal
  Volt tip sistemi → hangi çözümün uygun olduğunu derleme zamanında garantiler
  Mimari OS → çalışma zamanında doğru çözümü seçer

Son söz:
  ADC sorunu çözülmüş bir problem değil.
  Enerji bütçesi sıkıştıkça, paradigmalar karıştıkça,
  Volt'un tip sistemindeki @cost anotasyonları ve
  bridge tip güvenliği bu kararları mühendisten
  derleyiciye taşıyacak — bu geçiş Volt'un
  uzun vadede en büyük değer katkısı olabilir.
```
