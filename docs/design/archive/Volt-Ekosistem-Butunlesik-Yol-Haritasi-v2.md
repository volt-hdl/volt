> UYARI: Bu belge geçersizdir.
> Güncel sürüm: docs/design/ altında.

# Volt HDL Ekosistemi — Kapsamlı Yol Haritası

> Bu belge, bu oturumda üretilen 30 belgeyi ve yapılan araştırmaları
> sentezler. Rekabet analizi, teknoloji vizyonu, teknik kararlar
> ve pratik adımları bir arada sunar.

---

## Bölüm 1 — Gerçekliği Görmek: Dürüst Değerlendirme

### 1.1 Bugünün Durumu

```
Volt HDL:                           Rakipler:
  ✓ Vizyon kapsamlı                  Arch: CDC'yi 3 ay önce yaptı (çalışıyor)
  ✓ Teknik tasarım detaylı           Spade: 300 kullanıcı, araç ekosistemi tam
  ✓ Rekabet analizi yapıldı          Veryl: 938 ★, paket yöneticisi/LSP mevcut
  ✓ Teknoloji trendleri bilinir      Clash: 16 yıldır CDC yapıyor
  ✓ Strateji belgelenmiş
  ✗ Tek satır kod yok
  ✗ F0 bile tamamlanmadı
  ✗ Ekip yok
  ✗ Finansman yok

Sonuç: Güçlü bir spesifikasyon belgesi, henüz proje değil.
Bunu değiştirmek için gereken: F0.
```

### 1.2 Pencere Daralıyor

```
Zaman çizelgesi baskısı:

Nisan 2026: Arch CDC'yi tip sistemine ekledi (ÇALIŞIYOR)
Haziran 2026: ??? (bu aylar içinde Spade CDC ekleyebilir)

Volt'un CDC avantajı rekabet penceresi:
  Şu an: açık (Spade yapmadı, Veryl yapmadı)
  6 ay içinde: daralabilir (Spade CDC'yi bitirirse)
  12 ay içinde: kapanmış olabilir

Bu pencereyi değerlendirmek için
F0 → F2 arası hız kritik.
```

### 1.3 Volt'un Gerçek Farklılaşma Noktaları

```
Bugün mevcut ve rakiplerde YOK:
  1. CDC + bağımsız dil + CIRCT üçlüsü (tam olarak)
  2. Ternary/Trit tipi donanım HDL'de
  3. Nöromorfik uzantı vizyonu (v2)
  4. Fotonik ternary vizyonu (v3)
  5. Çok paradigma tek çatı (dijital + ternary + spike + fotonik)

Yakında kapanacak avantaj:
  CDC: Spade 6-12 ay içinde ekleyebilir
  CIRCT: Arch da planlıyor

Uzun vadeli korunan avantaj:
  Ternary + nöromorfik + fotonik vizyon
  → Hiçbir rakip bu yönde gitmiyor
```

---

## Bölüm 2 — Stratejik Çerçeve

### 2.1 Volt Nedir, Ne Değildir

```
Volt NEDİR:
  "Donanım mühendisleri için CDC garantili,
   bağımsız, CIRCT tabanlı, çok paradigmalı HDL.
   Bugün FPGA ve ASIC, yarın ternary, nöromorfik, fotonik."

Volt NE DEĞİLDİR:
  "Verilog'u güzel yazan transpiler" (Veryl bu)
  "Haskell veya Scala gömülü DSL" (Chisel/Clash bu)
  "Sadece akademik araştırma dili" (Filament bu)
  "Sadece AI donanımı için" (dar, kaçınılacak)
```

### 2.2 Hedef Kitle Hiyerarşisi

```
Aşama 1 (Yıl 1-2): FPGA topluluğu + öğrenciler
  Kim: Rust bilen, Verilog öğrenen veya bıkan
  Neden Volt: "Sensitivity list yok, CDC garantili, iyi hata mesajı"
  Kanal: r/FPGA, HackerNews Show HN, GitHub

Aşama 2 (Yıl 2-4): AI chip startupları
  Kim: 3-20 kişilik ASIC ekibi, özel hızlandırıcı
  Neden Volt: "Ternary NPU + formal doğrulama + ML PPA tahmini"
  Kanal: YC/a16z network, DAC/MICRO

Aşama 3 (Yıl 4-7): Orta ASIC şirketleri
  Kim: 50-500 kişi, otomotiv, tıp, tüketici
  Neden Volt: "Kanıtlanmış CDC + ASIC akış entegrasyonu"
  Kanal: Üniversiteden gelen mühendisler

Aşama 4 (Yıl 7+): Nöromorfik araştırma kuruluşları
  Kim: Intel Loihi, IBM, BrainChip ekipleri
  Neden Volt: "Spike<T> + nöromorfik + fotonik tek çatı"
```

### 2.3 İş Modeli

```
Açık Çekirdek (Open Core) Modeli:

Volt Open (ücretsiz, daima):
  Dil spesifikasyonu, çekirdek derleyici, LSP temel
  FPGA hedefleme, Yosys entegrasyonu
  Temel stdlib, community forum

Volt Pro (ticari):
  Gelişmiş CDC analizi (yeniden yakınsama)
  ASIC akış entegrasyonları (DC, Innovus, Calibre)
  ML tabanlı PPA optimizasyon motoru
  SLA + kurumsal destek + audit

Gelir takvimi:
  Yıl 1: 0 TL (sadece yatırım)
  Yıl 2: İlk danışmanlık + eğitim geliri
  Yıl 3: İlk Volt Pro lisansı
  Yıl 5: Sürdürülebilir iş modeli

Alternatif finansman:
  DARPA ERI (Electronics Resurgence Initiative): tam uyum
  EU Horizon: nöromorfik + AI donanım teması
  Üniversite araştırma hibeleri: akademik ortaklık
```

---

## Bölüm 3 — Teknik Yol Haritası

### Aşama 0 — F0: "Çalışan İlk Şey" (Bugünden 4-6 Hafta)

**Hedef:** Tek çalışan pipeline. Bir counter, SystemVerilog'a, Verilator'a, CI'a.

```
F0 Çıktıları:
  volt/
  ├── Cargo.toml          (workspace kurulumu)
  ├── volt-syntax/        (lexer + parser — minimal)
  │   └── src/
  │       ├── lexer.rs    (logos ile token'lar)
  │       └── parser.rs   (el yazısı özyinelemeli)
  ├── volt-hir/           (yüksek seviye IR — basit)
  │   └── src/lib.rs      (5-10 düğüm türü)
  ├── volt-sv-emit/       (doğrudan SV üretici — CIRCT yok!)
  │   └── src/lib.rs      (string template ile SV)
  └── volt-driver/        (CLI — "volt build counter.volt")
      └── src/main.rs

  Counter örneği (counter.volt):
    module Counter {
        in  enable : bool
        out count  : u8
        reg(Sys) value : u8 = 0
        on Sys { if enable { value <= value + 1 } }
        count = value
    }

  Üretilen SV:
    module Counter (
        input  logic       enable,
        output logic [7:0] count
    );
        logic [7:0] value = 8'h0;
        always_ff @(posedge clk or posedge rst) begin
            if (rst) value <= 8'h0;
            else if (enable) value <= value + 1;
        end
        assign count = value;
    endmodule

  CI (GitHub Actions):
    volt build → SV üret → Verilator derle → test çalıştır
    Her commit'te otomatik → "Yesil ✓"

NEDEN CIRCT YOK F0'DA:
  CIRCT öğrenmek 2-3 ay
  F0 için CIRCT gerekmez
  SV'yi string template ile üret
  CIRCT F3'te eklenir (sonra)

Bu aşama için gerekli Rust bilgisi:
  logos: lexer kuralları (2-3 saat öğrenme)
  winnow/nom: parser (1-2 hafta pratik)
  Temel Rust: yeterli

F0 bitmeden HİÇBİR ŞEYDE İLERLEME YOK.
```

---

### Aşama 1 — F1-F2: "Dil Temeli" (Ay 1-6)

**Hedef:** Parser + Tip Sistemi + İsim Çözümü

```
F1 — Parser ve CST (Ay 1-2):
  cstree entegrasyonu (hata-toleranslı CST)
  EBNF grameri tam uygulanması
  Volt dilinin tüm yapıları parse edilebilmeli
  "volt parse" komutu çalışmalı

F2 — Tip Sistemi (Ay 2-6) — KRİTİK YOL:
  Bu aşama en uzun ve en önemli.
  6 ayın 4'ü burada harcanabilir.

  F2a: Temel tipler
    u8, u16, u32, u64, i8... + bool + bits<N>
    Bit genişliği çıkarımı
    İşaret uyumluluk kuralları

  F2b: Trit tipi (erken!)
    Trit = i2 ile kısıtlı {-1, 0, +1}
    Volt'un benzersiz katkısı
    Basit ama anlambilim açısından önemli

  F2c: Saat alanı tipleri (KRİTİK)
    domain Sys { } tanımı
    @Domain anotasyonu
    CDC ihlali → derleme hatası
    Bu Volt'un kalbi

  F2d: Port yönü güvenliği
    in/out tipte
    Spade'in lineer tipleri → Volt'a uyarla
    Çift sürücü derleme hatası

  F2e: İsim çözümü
    Scope analizi
    Modül referansları
    Generic parametreler

Test stratejisi F1-F2 için:
  "Volt comptest" klasörü
  Her özellik için: geçmeli .volt + hata vermeli .volt
  100+ test → güven inşa eder
```

---

### Aşama 2 — F3: "CIRCT Entegrasyonu" (Ay 6-9)

**Hedef:** HIR → CIRCT → Okunabilir SV

```
Bu aşama en zor teknik adım.
2-3 aylık CIRCT öğrenmesi gerekiyor.

Mimari:
  volt-hir → volt-lower (CIRCT) → SystemVerilog

volt-lower crate tasarımı:
  - CIRCT sadece bu crate'te
  - Değişirse sadece burayı değiştir
  - melior crate kullan (Rust MLIR wrapper)
  - FIRRTL fallback hazır tut (Plan B)

Hangi CIRCT dialect:
  seq dialect: register'lar için (seq.firreg)
  hw dialect: modüller için (hw.module)
  comb dialect: kombinasyonel mantık
  sv dialect: SV özellikli yapılar

Hedef:
  Counter örneği → CIRCT → okunabilir SV
  "Okunabilir" önemli: ASIC mühendisi anlayabilmeli
  Veryl'in çıktı kalitesi referans alınabilir

CIRCT topluluğuyla temas (bu aşama öncesi):
  LLVM Discourse → "Volt yeni HDL, şu soruyu soruyorum"
  Spesifik teknik soru → ciddi yanıt alınır
```

---

### Aşama 3 — F4-F5: "Kullanılabilir Ürün" (Ay 9-12)

**Hedef:** Simülatör + Formal + LSP + Beta

```
F4 — Simülatör ve Formal (Ay 9-11):

  Simülatör:
    Seçenek A: Verilator köprüsü (hızlı, güvenilir)
      volt build --simulator=verilator → Verilator çağır
      Bugün bunu yap
    Seçenek B: Yerleşik simülatör (uzun vadeli hedef)
      F4'te Seçenek A yeterli

  SymbiYosys formal:
    assert/invariant → SVA'ya çevir → SymbiYosys çağır
    "volt verify --depth 20 counter.volt"
    Bounded BMC: hobi ve startup için yeterli

  SVA export:
    JasperGold/Formal Verification: SVA okur
    Volt SVA üretiyorsa büyük ASIC için de çalışır
    Resmi destek beklemeden!

F5 — LSP + Stdlib + Docs + Beta (Ay 11-12):

  LSP sunucusu (tower-lsp):
    Inline diagnostics: ŞART (F5'e bırakma!)
    Hover: tip bilgisi göster
    Go to definition: modül gezinme
    Volt fmt: biçimlendirici

  Standart kütüphane (başlangıç):
    TwoFlop<T>: CDC senkronizatör (Volt'un kalbi)
    FIFO<T, N>: basit FIFO
    AXI_Lite::Master/Slave: protokol bundle
    Bundle<...>: port grup yönetimi

  Dokümantasyon:
    "Merhaba Donanım" (30 dakika ile ilk tasarım)
    "Verilog'dan Volt'a" geçiş rehberi (ŞART)
    API referans

  Beta:
    GitHub'da public, açık kaynak
    Discord kanalı
    r/FPGA duyurusu

  Beta hedef sayısı: 50-100 aktif kullanıcı
  Bu sayı olmadan v1'e geçmek anlamsız.

Tiny Tapeout (Ay 12):
  Volt ile tasarlanan çip → submit
  "Volt ile tasarlanan çip üretimden çıktı"
  → En güçlü itibar kanıtı
```

---

### Aşama 4 — V1: "İlk Gerçek Versiyon" (Yıl 2)

**Hedef:** Ekosistem tamamlanması + İlk ticari kullanım

```
V1 öncelik listesi:

1. Playground (WASM) — AY 1-3
   "30 saniyede ilk Volt tasarımı"
   Spade ve Veryl'de zaten var — fark kapanmadan yap
   WebAssembly derleyici + basit editör

2. Paket Kayıt (Registry) — AY 2-4
   volt.toml → bağımlılık yönetimi
   registry.volt-lang.org
   İlk 20-30 paket (stdlib genişletmesi)

3. Python API — AY 3-6
   volt_python: parametrik jeneratörler
   Ternary NPU katmanı üretici
   cocotb entegrasyonu (tip-farkında)

4. ML PPA Tahmini (temel) — AY 4-8
   Xgboost/basit model: alan + güç tahmin
   "Sentez öncesi 'bu tasarım ne kadar büyük?'"
   Spade'de yok, Veryl'de yok → farklılaşma

5. HW-SW Köprüsü — AY 6-9
   Register haritası Volt → Rust/C sürücü otomatik
   En sinerjik eklenti (tüm projelerde var)

6. SDC Otomatik Üretimi — AY 8-12
   Volt pipeline bilgisi → Synopsys SDC
   ASIC ekipleri için kritik

7. Verilog'dan Volt'a Çevirici — AY 9-12
   %60-70 otomatik, rest el ile
   Benimseme engelini azaltır

V1 hedef metrikleri:
  500+ aktif kullanıcı
  1 ticari müşteri (küçük ASIC startup)
  10+ topluluk katkıcısı
  Üniversite müfredat ortaklığı (1 üniversite)
```

---

### Aşama 5 — V2: "Nöromorfik + AI Destekli" (Yıl 3-4)

**Hedef:** Paradigma genişlemesi + İlk ticari traction

```
V2 teknik genişleme:

1. Spike<T> ve Olay Semantiği
   Nöromorfik tile tasarımı için
   FeFET tabanlı sinaptik ağırlık tipi
   always_on { on spike { } } semantiği

2. NeuroTorch
   PyTorch üzerine spike anotasyonları
   Donanım kısıtlarını eğitime geri besle
   "Bu modeli Volt ile tasarlanan çipe çalıştır"

3. NeuroCompiler (temel)
   Küçük ağ topolojisi → FPGA mapping
   NP-zor → sezgisel yaklaşım, mükemmel değil

4. AI Destekli Tasarım
   LLM + Volt tip sistemi döngüsü
   Volt tiplerini anlayan LLM fine-tuning
   "todo!" mekanizması (Arch'tan ders)
   LLM hata mesajından öğreniyor

5. Güvenlik Akış Denetimi
   @Secure / @Public domain ayrımı
   Güvenlik çipi pazarı için kritik
   "Gizli sinyal asla açık alana akamaz"

V2 organizasyonel hedefler:
  Volt Foundation kurulumu
  (Linux Foundation modeli, vendor-nötr)
  5+ şirket katkıcı üye
  DARPA/EU Horizon hibesi

V2 hedef metrikleri:
  2000+ aktif kullanıcı
  5+ ticari müşteri
  3+ üniversite müfredat
  İlk nöromorfik tile Volt ile tasarlandı
```

---

### Aşama 6 — V3+: "Fotonik Vizyon" (Yıl 5+)

**Hedef:** Yeni donanım paradigmalarının HDL'i olmak

```
V3 teknik kapsam (araştırma + vizyon):

PTrit Tipi ve Fotonik Primitifleri:
  MZI ağırlık tipi: faz noktaları {0, π, 2π}
  WDM<λ_nm> kanal tipi
  OpticalSignal + Photonic domain anotasyonu
  FE+EO CIM hücre modeli

Mimari İşletim Sistemi:
  hardware_descriptor Volt tipinde
  Görev → paradigma yönlendirme
  Nöromorfik + dijital + fotonik hibrit

Analog/Mixed-Signal Arayüzü:
  extern module ile sınır
  Cadence Virtuoso köprüsü
  "Analog bloğunu sarmala, içinden geçme"

FE+EO CIM Desteği:
  2032+ gerçekçi
  Donanım hazır olduğunda tip sistemi hazır olacak
  Bugünden tasarlanıyor, donanım gelince çalışıyor

V3 gerçeklik kontrolü:
  Bu bileşenler 2025'te ürün değil, araştırma
  Akademik ortaklık ile geliştirme
  EPFL, MIT, Stanford Si-fotonik grupları
  Deneysel crate olarak etiketleme
```

---

## Bölüm 4 — Ekip ve Kaynak Planı

### 4.1 Gerekli Roller

```
HEMEN GEREKEN (F0 için):
  Rust/Derleyici Mühendisi (1 kişi veya kendiniz)
    → Lexer, parser, tip sistemi, CIRCT
    → Bu olmadan F0 bile olmuyor
    → Bulma yolu: CIRCT GitHub katkıcıları,
      rust-analyzer ekibi, akademik PL araştırmacıları

AY 2-3 (F1-F2 için):
  Donanım Tasarım Mühendisi (1 kişi)
    → SV doğrulama, stdlib, Vivado/Quartus test
    → "Bu SV sentezleniyor mu?" sorusunu cevaplar
    → Bulma yolu: Tiny Tapeout topluluğu, r/FPGA

YIL 2 (V1 için):
  ML/Sistem Mühendisi (1 kişi)
    → Python API, cocotb, PPA motoru
  DevRel (1 kişi)
    → Topluluk, belgeleme, blog, Twitter/X
    → Olmadan kullanıcı gelmez

DANIŞMAN (proje bazlı, tam zamanlı değil):
  Formal Doğrulama: SymbiYosys tasarımı için (Ay 9)
  ASIC Sign-off: İlk tape-out için
  Nöromorfik Araştırmacı: V2 Spike<T> tasarımı
  Si-Fotonik Araştırmacı: V3 PTrit fiziksel geçerlilik
```

### 4.2 Bütçe Gerçekçiliği

```
Minimal senaryo (1 kişi, kendi başına):
  F0: 0 maliyet (kendi zamanı)
  F1-F5: 6-12 ay, ciddi zaman yatırımı
  Toplam: zaman maliyeti → sonra iş modeli

Küçük ekip (2-3 kişi, erken aşama):
  Yıl 1: $200-500K (maaş + altyapı)
  Finansman: angel, DARPA, kendi sermaye

Startup senaryo (5+ kişi):
  Yıl 1: $500K-1M
  Yıl 2-3: $2-5M Series A
  Finansman: YC, a16z, Sequoia (donanım AI teması)

NOT: Açık kaynak geliştirme maliyeti düşürebilir.
1 yetenekli geliştirici + topluluk = başlangıç için yeterli.
```

---

## Bölüm 5 — Topluluk ve Ekosistem Büyüme Stratejisi

### 5.1 İlk 100 Kullanıcı

```
Adım 1 — "Show HN" (F0 tamamlanınca):
  Başlık: "Volt: CDC'yi derleme zamanında yakalayan HDL"
  İçerik: Counter → SV pipeline, çalışıyor
  CDC ihlali örneği + derleme hatası
  → HackerNews front page hedefi

Adım 2 — r/FPGA duyurusu:
  "Sensitivity list olmayan, CDC garantili HDL"
  Kod örneği + GitHub linki
  FPGA topluluğu: yeni araçlara açık

Adım 3 — CIRCT topluluğu:
  LLVM Discourse'da teknik soru sor
  "Volt bu şekilde lowering yapıyor, görüş?"
  Teknik topluluk = kaliteli ilk kullanıcılar

Adım 4 — Akademi:
  1 üniversite ile ön görüşme
  "Müfredatınızda Volt kullanabilir miyiz?"
  → Öğrenci kitlesi + feedback döngüsü
```

### 5.2 Topluluk Sağlığı Metrikleri

```
Sağlıklı topluluk için gerekli minimum:
  Discord/Matrix: aktif kanal, sorular yanıtlanıyor
  GitHub Issues: 24-48 saat içinde yanıt
  Katkı rehberi: net, dostane, giriş noktaları
  İlk katkı kolaylığı: "good first issue" etiketler

SPADE'İN YAKLAŞIMINDAN KAÇINILACAKLAR:
  "LLM katkısı kabul edilmez" politikası
  → Volt: LLM yardımcı olabilir, kalite kontrolü insanda
  → Bu yaklaşım farklı hedef kitleye hitap eder
```

### 5.3 Akademi İlişkileri

```
Öncelikli üniversiteler:
  Bilkent, ODTÜ, İTÜ (Türkiye, güçlü başlangıç)
  MIT, Stanford, ETH Zürich (uzun vade, itibar)
  Linköping, TU Delft (Spade/donanım HDL topluluğu)

Yaklaşım:
  "Müfredat ortağı" → ücretsiz + ortak geliştirme
  "Araştırma hibesi" → Volt + akademik araştırma
  "Tez konusu" → NeuroCompiler, K-framework köprüsü
```

---

## Bölüm 6 — Rekabet Stratejisi

### 6.1 "Neden Volt Değil X?" Sorularına Hazır Yanıtlar

```
"Neden Arch değil?":
  "Arch CDC'yi yapıyor (iyi!) ama:
   CIRCT yok, formal yok, ternary yok,
   nöromorfik yok, fotonik yok.
   Arch dijital RTL için, Volt geleceğin
   donanım paradigmaları için de."

"Neden Spade değil?":
  "Spade bağımsız dil, güçlü tip sistemi (iyi!)
   ama CDC henüz yok.
   Spade yakında ekleyecek ama Volt bunu
   başından itibaren dilde garanti ediyor."

"Neden Clash değil?":
  "Clash CDC'yi 16 yıldır yapıyor (doğru fikir!)
   ama Haskell öğrenmek 3-12 ay.
   Volt: Clash'in CDC güvenliği, Haskell olmadan."

"Neden Veryl değil?":
  "Veryl SV'yi güzel yazmanın yolu (faydalı!)
   ama yeni anlambilim yok.
   CDC, formal, pipeline güvenliği, ternary — yok.
   Veryl'den gelen mühendisler Volt'a geçecek."
```

### 6.2 Arch ile İlişki

```
Arch en yakın rakip ve aynı zamanda potansiyel köprü.

Nisan 2026'da çıktı — 3 ay önce.
Tek geliştirici (Shuqing Zhao).
Çalışan derleyici + vaka çalışmaları (güçlü kanıt).

Fırsatlar:
  CIRCT: Arch planlıyor, Volt yapıyor
  → Volt CIRCT lowering yayımlarsa Arch geliştiricisi ilgilenir

  todo! mekanizması: Arch'tan al, Volt'a ekle
  → Bu hafta tasarım kararına eklenebilir

  Topluluk: Arch + Volt aynı problem farklı kapsam
  → İşbirliği mümkün

Tehdit:
  Arch kapsam genişlirse (ternary ekler mi?)
  → Şu an genişletme planı yok, izle
```

### 6.3 Spade ile İlişki

```
Spade en olgun rakip.
Frans Skarman ile iletişim şart.

İletişim için zamanlama: F0 tamamlanınca
"Volt'ta CDC'yi şöyle yapıyoruz: [kod örneği]
 Spade'deki CDC planlarınız ne zaman?
 Surfer'ı Volt'ta da destekleyebilir miyiz?"

Olası sonuçlar:
  A) İşbirliği: Surfer Volt desteği, CDC fikir alışverişi
  B) Yarışma: CDC yarışı → Volt önce yaparsa kazanır
  C) Nişleşme: Spade FPGA, Volt ASIC + çok paradigma
```

---

## Bölüm 7 — Teknoloji Trendleriyle Uyum

### 7.1 Bugün Geçerli (Volt MVP'de)

```
TNN (Ternary Neural Networks) — BUGÜN ÇALIŞIYOR:
  BitNet b1.58: 7B model 1.4 GB, CPU'da çalışır
  Volt'un Trit tipi bu trendi doğrudan karşılıyor
  "Ternary NPU Volt ile tasarlandı" → güçlü mesaj

FPGA prototipi olarak AI hızlandırıcı:
  Volt → sistolic dizi RTL → Vivado → FPGA
  Bu bugün mümkün ve talep var

RISC-V uzantısı:
  Özel ternary komut → RISC-V custom-0 opcode
  Volt ile tip-güvenli decoder
  ISA prototipi için mükemmel
```

### 7.2 Yakın Vadede Geçerli (2026-2028, V1-V2)

```
Nöromorfik tile telefon SoC'larında (2028-2030):
  Volt v2 Spike<T> → bu chiplet'leri tanımlar
  Qualcomm/Apple NPU + nöromorfik tile birlikte
  Volt bu geçişi modelleyen tek araç

SSM/Hibrit mimari (Mamba + Transformer):
  KV cache problemini çözen mimariler
  Volt Hardware OS → task routing
  Mamba donanım hızlandırıcısı → Volt ile tasarlanır

HBM4 + Ternary ASIC (2027-2028):
  Mac Studio boyutu 400B cihazı
  Volt ternary sistolic dizi → ASIC tape-out
  Bu konuşmanın teknik spesifikasyonu hazır
```

### 7.3 Uzun Vadede (2030+, V3)

```
FE+EO CIM (2032+):
  LiNbO₃ ferroelektrik + Pockels etkisi
  Volt PTrit tipi bu donanım gelince hazır
  "Volt ile tasarlanan ilk fotonik çip"

Nöromorfik + Fotonik hibrit:
  Spike tabanlı alan + optik ağırlık
  Mimari OS her iki paradigmayı yönetiyor
  Akademik araştırma, ürün değil

Sürekli analog FE+EO:
  Analog ağırlık depolama → FP-Photonic tipi
  2 bit sayısal üs + analog mantis
  12 bit etkin hassasiyet, ternary alanı
```

---

## Bölüm 8 — En Kritik Riskler ve Azaltma

### 8.1 Teknik Riskler

```
Risk 1: CIRCT API kırılması
  Olasılık: Yüksek (upstream hızlı değişiyor)
  Etki: volt-lower crate yeniden yazılır
  Azaltma: volt-lower izolasyonu + FIRRTL fallback

Risk 2: Tip çıkarım algoritması yanlış tasarım
  Olasılık: Orta (karmaşık problem)
  Etki: F2 yeniden yazılır, aylarca kayıp
  Azaltma: Filament PLDI 2023 + Hindley-Milner referans
           CDC semantik test suite F2 öncesi

Risk 3: Simülasyon semantiği hatası
  Olasılık: Yüksek (ince anlambilim sorunu)
  Etki: Kullanıcı güveni kaybı
  Azaltma: "Tek semantik model" → formal spec önce
           K-framework bağlantısı uzun vade

Risk 4: Topluluk oluşmaması
  Olasılık: Orta (yaygın sorun)
  Etki: Proje ölür
  Azaltma: F0 olmadan duyuru yapma
           İlk 50 kullanıcıyı elle kazan
```

### 8.2 Rekabet Riskleri

```
Risk 5: Spade CDC ekler (6-12 ay içinde)
  Olasılık: Yüksek (açıkça planlandı)
  Etki: Volt'un ana farklılaşması zayıflar
  Azaltma: CDC Volt'ta F2'de (önce bitir!)
           Ternary + çok paradigma korunan fark

Risk 6: Büyük firma (Google/Intel) kendi açık HDL çıkarır
  Olasılık: Düşük-orta
  Etki: Çok ciddi
  Azaltma: Niş (ternary + nöromorfik) korun
           Topluluk önce kur → büyük firma zor geçer

Risk 7: Chisel topluluğu güçlenir (JVM sorununu çözer)
  Olasılık: Düşük
  Etki: Orta
  Azaltma: Volt'un bağımsız dil avantajı korunur
```

### 8.3 İnsan Kaynağı Riskleri

```
Risk 8: Tek kişi tükenmesi
  Olasılık: Çok Yüksek (en yaygın ölüm nedeni)
  Etki: Proje durur
  Azaltma: F0 tamamlanınca hemen ortak ara
           Topluluk katkısını erken aç
           Küçük tutulabilir hedefler

Risk 9: Yanlış ortak seçimi
  Olasılık: Orta
  Etki: Kültürel uyumsuzluk, enerji kaybı
  Azaltma: F0 kodu ile ortağı değerlendir
           Teknik yetkinlik + uzun vade vizyonu uyumu
```

---

## Bölüm 9 — Bu Haftadan Başlayacaklar

### 9.1 24 Saat İçinde

```
1. Bu konuşmanın özetini bir GitHub Gist veya Not'a kaydet
   (uzun oturum — kaybetme)

2. Volt dili spesifikasyonunu tek Markdown dosyasına topla
   (birden fazla belgede dağınık)

3. F0 için gerekli araçları yükle:
   rustup, cargo, git, Verilator (brew/apt)

4. Boş GitHub repository aç:
   volt-lang/volt (veya kişisel)
   README: tek paragraf vizyon
   .gitignore + MIT/Apache lisans
```

### 9.2 Bu Hafta

```
5. logos ve winnow/nom ile mini lexer yaz:
   Hedef: "module" anahtar kelimesini tanı
   Test et: "module Counter" → TOKEN_MODULE TOKEN_IDENT

6. F0 gramer alt kümesini tanımla:
   Sadece module, in, out, reg, on, u8, bool
   Tam gramer değil, minimal çalışan subset

7. Bir Rust/CIRCT topluluğuna katıl:
   LLVM Discord veya CIRCT GitHub Discussions
   "Yeni HDL üzerinde çalışıyorum" → sessiz tanıtım
   Sormak için değil, ortamı anlamak için
```

### 9.3 Bu Ay

```
8. Counter örneği → SV üretimi (F0):
   Hedef: minimal, çirkin ama çalışan SV
   Test: Verilator bu SV'yi derliyor mu?
   CI: GitHub Actions yeşil ✓

9. İlk "CIRCT spike":
   melior crate'i kur
   CIRCT'te bir "hello world" hw.module üret
   Volt'tan değil, doğrudan Rust'tan
   Amac: CIRCT'i anlamak

10. CDC semantik kararı belgele:
    ADR-0001: "CDC domain anotasyonu nasıl çalışır"
    Kod yokken karar yaz → kod yazınca referans
```

---

## Bölüm 10 — Başarı Kriterleri

### Her Aşama İçin

```
F0 tamamlandı mı?
  ✓ Counter Volt'ta yazıldı
  ✓ SV üretildi
  ✓ Verilator derledi
  ✓ GitHub CI yeşil
  ✓ README okunan biri çalıştırabilir

V0.1 hazır mı?
  ✓ F0-F5 tamamlandı
  ✓ 50+ kullanıcı beta test yapıyor
  ✓ Tiny Tapeout submit edildi
  ✓ 1 blog yazısı veya konferans bildirisi

V1 hazır mı?
  ✓ 500+ aktif kullanıcı
  ✓ Playground çalışıyor
  ✓ Paket registry 10+ paket
  ✓ 1 ticari müşteri referansı
  ✓ Üniversite müfredat ortaklığı

Uzun vade (5 yıl):
  ✓ "AI hızlandırıcı tasarımında standart HDL" konumu
  ✓ İlk nöromorfik tile Volt ile üretimde
  ✓ Volt Foundation kurulmuş
  ✓ Sürdürülebilir iş modeli
```

---

## Özet: 9 Kritik Karar

```
Bu oturumda netleşen ve değişmemesi gereken kararlar:

1. YENİ DİL: Volt bağımsız HDL — Python/Scala/Haskell gömülü değil
2. RUST DERLEYİCİ: Bellek güvenli, ekosistemi güçlü
3. CIRCT ARKA UCU: F3'ten itibaren — sağlam, çok hedef
4. CDC DİLDE: Volt'un kalbi — F2'de, başka hiçbir şeyden önce
5. TRIT TİPİ ERKEN: F2'de minimal — benzersiz farklılaşma
6. SV ÇIKTISI: Araç uyumu bedava gelir
7. AÇIK ÇEKİRDEK: Dil açık, gelişmiş araçlar ticari
8. FPGA'DAN BAŞLA: Hobi → startup → büyük ASIC sırası
9. F0 ÖNCE: Başka hiçbir şeyde ilerleme F0 olmadan anlamsız

Ve bir hatırlatma:
  Bu konuşma bir spesifikasyon belgesi üretti.
  Artık bunun ötesine geçme zamanı.
  İlk satır kodu bu hafta yazılabilir.
```
