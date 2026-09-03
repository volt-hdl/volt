> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Volt Ekosistemi — Bileşen Analizi, Hedef Kitle ve Gelecek Stratejisi

> Bu belge Volt ekosisteminin tüm bileşenlerini güçlü/zayıf yönleriyle
> değerlendirir, çip tasarımında dış yazılım bağımlılığını açıklar,
> hedef kitle stratejisini belirler ve uzun vadeli ekosistem
> güçlendirme yolunu çizer.

---

## Bölüm 1 — Ekosistem Bileşen Haritası

### 1.1 Tam Bileşen Listesi (Geliştirme Önceliğine Göre)

```
TEMEL KATMAN (Yıl 1 — MVP)
├── Volt Dili (HDL çekirdeği)
├── Volt Derleyici (Rust + CIRCT)
├── LSP Sunucusu + volt fmt/lint
├── Yerleşik Simülatör
├── Formal Doğrulama Köprüsü
├── Standart Kütüphane (temel)
└── Paket Yöneticisi

GENİŞLEME KATMANI (Yıl 2-3 — v1)
├── Python API (jeneratör + cocotb)
├── ML PPA Tahmin Motoru
├── Playground (WASM tarayıcı)
├── Paket Kayıt (registry)
└── Eğitim platformu

UZANTI KATMANI (Yıl 3-5 — v2)
├── NeuroTorch (spike-uyumlu eğitim)
├── NeuroCompiler (topoloji → çip)
├── Spike<T> + Olay Semantiği
└── Profiler + Dalga Formu İzleyici

ARAŞTIRMA KATMANI (Yıl 5+ — v3)
├── Fotonik Ternary Uzantısı (PTrit)
├── Analog/Mixed-Signal Arayüz
└── Mimari İşletim Sistemi
```

---

## Bölüm 2 — Her Bileşenin Detaylı Analizi

### 2.1 Volt Dili (HDL Çekirdeği)

**Görev:** Donanım davranışını tanımlayan bağımsız, amaca özel dil.

```
Güçlü Yönler:
  ✓ Saat alanı tip sisteminde → CDC ihlali derlenmez
  ✓ Tek semantik model → sim = sentez garantisi
  ✓ Kademeli zamanlama (L0/L1/L2) → öğrenme eğrisi yönetimi
  ✓ Dengeli üçlü (Trit) → nöromorfik + fotonik için doğal
  ✓ Rust/Elm sınıfı hata mesajları → öğretici
  ✓ Bağımsız dil → Scala/Haskell/JVM yükü yok
  ✓ Standart SV çıktısı → mevcut araçlarla sürtünmesiz

Zayıf Yönler:
  ✗ Sıfır ekosistem (Yıl 1'de) → "kim kullanıyor?" sorusu
  ✗ Analog desteği yok (v3'e kadar)
  ✗ Verilog/SV'den sözdizimi farklılığı → alışma maliyeti
  ✗ L2 timeline tipleri karmaşık → bazı kullanıcılar zorlanır

Kıyaslama:
  Verilog/SV: olgun araç, zayıf tip güvenliği
  Chisel:     güçlü soyutlama, Scala bağımlılığı
  Volt:       orta soyutlama, bağımsız, güçlü tip
```

---

### 2.2 Volt Derleyici (Rust + CIRCT)

**Görev:** Kaynak kod → tip kontrolü → HIR → CIRCT → SystemVerilog.

```
Güçlü Yönler:
  ✓ Rust: bellek güvenliği, hızlı derleme (geliştirme)
  ✓ CIRCT: olgun arka uç, LLVM ekosistemi
  ✓ Hata-toleranslı parser (cstree) → LSP için şart
  ✓ Makine-okunur JSON tanılar → AI ajan desteği
  ✓ Tek araç zinciri: build/test/fmt tek komut

Zayıf Yönler:
  ✗ CIRCT bağımlılığı: hızlı değişen upstream
    → Sürüm sabitleme + FFI izolasyonu gerekli
  ✗ Büyük tasarımda derleme süresi uzayabilir
    → Workspace bölme + incremental derleme (v1)
  ✗ MLIR öğrenme eğrisi ekip için dik
    → İlk 2-3 ay öğrenme tamponu planlanmalı
  ✗ C++ arka uç (CIRCT) → Rust-C++ FFI nazik olmayan

Kritik Risk:
  CIRCT API kırılırsa güncelleme maliyeti yüksek.
  Çözüm: volt-lower crate'i tampon katman olarak izole.
```

---

### 2.3 LSP Sunucusu + Araçlar

**Görev:** IDE entegrasyonu, anlık tanı, biçimlendirme, lint.

```
Güçlü Yönler:
  ✓ tower-lsp (Rust): olgun, async, geniş IDE desteği
  ✓ Anlık tip/alan tanısı → hata aylarca beklemez
  ✓ Otomatik kablolama (AXI, APB) → boilerplate sıfır
  ✓ volt fmt: round-trip garantili (kayıpsız CST)
  ✓ Canlı PPA öngörüsü (v1) → erken mimari karar

Zayıf Yönler:
  ✗ F5 aşamasında (MVP sonunda) → ilk yıl yok
    Bu ciddi eksik: LSP olmadan geliştirici deneyimi kötü
  ✗ Görsel FSM izleyici (v1'e ertelendi)
  ✗ Büyük tasarımda LSP gecikme riski
    → Incremental analiz gerekli (salsa v1+)
```

---

### 2.4 Yerleşik Simülatör

**Görev:** `volt test` → derleme + simülasyon + kapsama tek komutla.

```
Güçlü Yönler:
  ✓ Sıfır kurulum → playground ile uyumlu
  ✓ Volt assert/invariant doğrudan izleme noktası
  ✓ VCD/FST dalga formu otomatik → GTKWave uyumlu
  ✓ Verilator hızında hedef (IR → native)

Zayıf Yönler:
  ✗ Büyük tasarımda Verilator/VCS'den yavaş
    → "Hız gerekirse Verilator'a yönlendir" politikası
  ✗ UVM (Universal Verification Methodology) desteği yok
    → Büyük ASIC ekipleri bunu sorar
    → Uzun vade: cocotb köprüsü bu açığı kapar

Gerçekçi değerlendirme:
  Hobi/öğrenci: yerleşik yeterli
  ASIC endüstrisi: VCS/Questa + UVM talep eder
  Bu hedef kitle ayrımını belirler (Bölüm 5)
```

---

### 2.5 Formal Doğrulama Köprüsü

**Görev:** `assert`/`invariant` → SymbiYosys BMC + K-framework semantik.

```
Güçlü Yönler:
  ✓ Dil içinde formal → sonradan ekleme değil
  ✓ Tek semantik model → formal = gerçek davranış
  ✓ Hazır özellik kütüphanesi (FIFO, handshake)
  ✓ Karşı-örnek otomatik dalga formu gösterimi

Zayıf Yönler:
  ✗ Büyük durum uzayı → bounded (sınırlı) kalan
    → Sonsuz kanıt için Coq/Lean gerekir (araştırma)
  ✗ K-framework entegrasyonu v1'e kadar yok
    → Anlambilim açığı korunuyor başta
  ✗ Öğrenme eğrisi: temporal mantık yabancı
    → Hazır şablon kütüphanesi bu açığı kapatmalı
```

---

### 2.6 Standart Kütüphane

**Görev:** Yerleşik FIFO, arbiter, CDC primitifleri, AXI bundle.

```
Güçlü Yönler:
  ✓ Tip-güvenli arayüzler (master/slave yönleri tipte)
  ✓ Yanlış bağlantı derleme hatası
  ✓ Dogfooding: stdlib, Volt ile yazılır → kalite kanıtlar

Zayıf Yönler:
  ✗ MVP'de ince (sadece temel bileşenler)
  ✗ AXI4-Full, CHI gibi karmaşık protokoller eksik başta
  ✗ Sektör standardı IP (AMBA, RISC-V) zaman alır
    → Topluluk katkısı kritik
```

---

### 2.7 Python API + NeuroTorch + NeuroCompiler

**Görev:** Jeneratör arayüzü, spike-uyumlu eğitim, topoloji → çip.

```
Güçlü Yönler:
  ✓ Donanım tasımaz Python kodu → donanım IR üretir
  ✓ NeuroTorch: HW kısıtlarını eğitime geri besler
  ✓ NeuroCompiler: FPGA hedefi ile ilk Volt RTL

Zayıf Yönler:
  ✗ Hepsi v2+ → MVP'de yok
  ✗ NeuroCompiler eşleme NP-zor → sezgisel yaklaşım
  ✗ Mamba/SSM donanım desteği araştırma aşaması

Kritik soru:
  NeuroTorch PyTorch'u geçmelidir?
  Hayır — PyTorch üzerine inşa edin, rekabet etmeyin.
```

---

### 2.8 Mimari İşletim Sistemi

**Görev:** Görev → paradigma eşleme, SoC + host katmanı.

```
Güçlü Yönler:
  ✓ hardware_descriptor Volt tipinde → tip güvenli eşleme
  ✓ SoC ve host için net sorumluluk ayrımı
  ✓ Volt assert → çalışma zamanı izleme

Zayıf Yönler:
  ✗ V3+ → 5+ yıl uzakta
  ✗ Çok iddialı → pratik benimseme belirsiz
  ✗ Fotonik/nöromorfik donanım olmadan test edilemez

Gerçekçi değerlendirme:
  Bu bileşen "vizyon" — ürün planı değil.
  İlk 5 yılda karar verilmeli: gerçek ürün mü araştırma mı?
```

---

## Bölüm 3 — Çip Tasarımında Volt Dışı Yazılım Gereksinimleri

### 3.1 Volt'un Kapsamadığı Alanlar

Volt RTL tanımı ve doğrulamasında güçlüdür. Ama tam çip tasarım akışında şunlara DOKUNMAZ:

```
SENTEZ:
  Volt → SystemVerilog üretir
  SystemVerilog → Yosys (açık) veya DC/Genus (ticari)
  VOLT YERİNE GEÇMEZ → sentez aracı zorunlu

YERLEŞTIRME & YÖNLENDIRME:
  Kapı ağı → fiziksel layout
  OpenROAD (açık) veya Innovus/Fusion (ticari)
  VOLT DOKUNMAZ

SIGN-OFF:
  DRC/LVS: Calibre (ticari, dökümhane şartı)
  Statik zamanlama: PrimeTime (ticari)
  VOLT DOKUNMAZ

ANALOG TASARIM:
  Cadence Virtuoso / Synopsys Custom Compiler
  Volt tamamen dijital RTL odaklı
  Mixed-signal: V3'e kadar extern module ile sınır

IP DOĞRULAMA (UVM):
  Büyük ASIC için VCS/Xcelium + UVM zorunlu
  Volt'un cocotb köprüsü (v1) alternatif sunar
  Tam UVM yerine geçmez başlangıçta
```

### 3.2 Tam Akış Tablosu

```
Aşama              Volt Katkısı           Dış Araç (zorunlu)
──────────────────────────────────────────────────────────────
RTL tasarım         ANA araç               —
Linting             volt lint              SpyGlass (ticari, opsiyonel)
CDC analizi         DİL SEVİYESİ           — (volt çözdü!)
Simülasyon          yerleşik + Verilator   VCS/Questa (büyük ASIC)
Formal              SymbiYosys             JasperGold (tam güç için)
Sentez              SV üretir              Yosys / DC / Genus
P&R                 —                      OpenROAD / Innovus
Zamanlama           —                      PrimeTime (ticari)
DRC/LVS             —                      Calibre (ticari, şart)
Analog              extern module sınırı   Virtuoso / Custom Compiler

SONUÇ:
  FPGA için: Volt + Yosys + Vivado/Quartus = YETERLI ✓
  ASIC open-source: Volt + Yosys + OpenROAD + hibrit broker = YETERLİ ✓
  ASIC büyük ticari: Volt + DC + Innovus + Calibre = şirket kurulur ✓✓
```

---

## Bölüm 4 — Hedef Kitle Stratejisi

### 4.1 "Herkese Her Şey" Yanılgısı

```
Yaygın hata: "hem hobiciye hem ASIC mühendisine hizmet et"
Gerçekte: bu iki kitle birbirinden çok farklı

Hobici/öğrenci ister:
  → Sıfır kurulum playground
  → Anında çalışan örnek
  → Nazik hata mesajı
  → Ücretsiz

ASIC mühendisi ister:
  → UVM desteği
  → Formal kesinlik (bounded değil)
  → Akış entegrasyonu (DC, Innovus)
  → Kurumsal destek (SLA, sorumluluk)
  → Ticari lisans

Her ikisini birden hedeflemek = hiçbirini iyi yapamamak.
```

### 4.2 Önerilen Strateji: Kademeli Genişleme

```
AŞAMA 1 (Yıl 1-2): FPGA Topluluğu + Öğrenciler
  Kim: hobi geliştiriciler, üniversite, küçük ekip
  Ne ister: kolay başlangıç, iyi hata mesajı, ücretsiz
  Volt değeri: "Sensitivity list yok, CDC garantili"
  Benimseme kanalı: r/FPGA, HackerNews, GitHub
  Ücret: tamamen ücretsiz ve açık kaynak

AŞAMA 2 (Yıl 2-4): AI Chip Startupları
  Kim: 3-20 kişilik ASIC ekibi, özel AI hızlandırıcı
  Ne ister: hız, CDC güvenliği, ML PPA, Python API
  Volt değeri: "Sentez öncesi PPA + formal güvence"
  Benimseme kanalı: YC/a16z network, DAC/MICRO bildiri
  Ücret: Açık kaynak + opsiyonel ticari destek

AŞAMA 3 (Yıl 4-7): Orta ASIC Şirketleri
  Kim: 50-500 kişi, telecom, otomotiv, tüketici elektroniği
  Ne ister: UVM benzeri doğrulama, akış entegrasyonu
  Volt değeri: "Formal kanıt + tip güvenli CDC"
  Benimseme kanalı: üniversiteden gelen mühendisler
  Ücret: ticari lisans + kurumsal destek

NEDEN BU SIRA:
  Hobi topluluğu = kullanıcı sayısı + kanıt
  Startup = ilk ticari referans + katkı
  Büyük şirket = direktif değil, çekilen pazar
```

### 4.3 İki Ürün Hattı Modeli

```
Volt Open (ücretsiz, açık kaynak):
  Temel derleyici, LSP, simülatör, stdlib
  FPGA hedefi, Yosys entegrasyonu
  Topluluk forumu, GitHub Issues
  Hedef: hobi, öğrenci, araştırmacı, startup

Volt Pro (ticari, abonelik):
  Gelişmiş formal doğrulama (sınırsız bounded)
  ASIC akış entegrasyonları (DC, Innovus)
  PPA optimizasyon motoru (ML tabanlı)
  Kurumsal destek (SLA, güvenlik, audit)
  IP sertifikasyon hizmeti
  Hedef: orta/büyük ASIC ekipleri

"Açık çekirdek" modeli (open core):
  Dil spesifikasyonu ve çekirdek derleyici: DAIMA açık
  Gelişmiş araçlar: ticari
  → Topluluk güveni + iş modeli dengesi
```

---

## Bölüm 5 — Uzun Vadede Ekosistemi Güçlendirme

### 5.1 Teknik Güçlendirme

```
KİSA VADE (Yıl 1-2):
  Playground kalitesi: "30 saniyede ilk tasarım"
  Örnek kütüphanesi: 50+ referans tasarım (sayaç'tan RISC-V'e)
  "Verilog'dan Volt'a" otomatik çevirici (kısmi)
    → Mevcut projeleri kolaylaştırır
  IDE eklentileri: VS Code (gün-1), Neovim, JetBrains (v1)

ORTA VADE (Yıl 2-4):
  Incremental derleme (salsa tabanlı)
    → 1M+ satır tasarımda hız kritik
  Görsel şema görünümü (LSP içinden)
    → Bağlantı grafiği canlı üretimi
  Gerçek zamanlı PPA göstergesi (ML SOG motoru)
  Çapraz platform: Linux, macOS, Windows (MSYS2)

UZUN VADE (Yıl 4+):
  Tam analog/mixed-signal (V3)
    → Cadence Virtuoso rakibi değil, köprüsü
  AI-destekli otomatik tamamlama
    → Volt tiplerine uygun LLM önerileri
  Donanım-farkında LLM (Volt üretiyor, derleyici düzeltiyor)
  Paralel simülasyon (çok çekirdekli, bulut)
```

### 5.2 Ekosistem Büyümesi

```
TOPLULUK:
  Volt Foundation kurulması (Linux Foundation modeli)
    → Vendor-nötr yönetim → kurumsal güven
  RFC (Request for Comments) süreci
    → Dil değişiklikleri şeffaf, topluluk onaylı
  Yıllık Volt Conference (DAC/ICCAD yanında satellite)
  Açık kaynak katkı rehberi + mentörlük programı

EĞİTİM:
  MIT/Stanford/ETH Zürich ortaklıkları
    → Müfredata girmek 5-10 yıllık benimseme garantisi
  Online kurs platformu (Coursera/edX)
    → "Volt ile Dijital Tasarım" sertifikası
  Türkçe dahil çok dilli dokümantasyon
  Etkileşimli Jupyter tarzı ders defteri (Volt + cocotb)

IP EKOSİSTEMİ:
  Volt Package Registry (npm/PyPI benzeri)
    → Sürüm kilitli, lisans denetimli
  IP Marketplace (şirketler Volt IP satabilir)
    → Gelir paylaşım modeli → katkıyı teşvik eder
  "Volt Certified" IP sertifikasyon programı
    → Kalite güvencesi → kurumsal alım kolaylaşır

ARAÇ ENTEGRASYONLARİ:
  Vivado/Quartus eklentisi (Volt → bitstream tek tık)
  GitHub Actions resmi Volt aksiyonu
    → CI/CD pipeline'a entegrasyon kolaylaşır
  Tinygo/Rust embedded ile ko-entegrasyon
    → Gömülü yazılım + donanım aynı repo
```

### 5.3 İş Modeli Sürdürülebilirliği

```
GELİR KAYNAKLARI:

1. Volt Pro Aboneliği:
   Küçük takım: $500/ay
   Orta şirket: $5K/ay
   Büyük şirket: $50K+/ay
   → Araç olgunlaştıkça değer artar

2. Eğitim ve Danışmanlık:
   "Volt ile ASIC tasarımı" kursu
   Şirket geçiş danışmanlığı
   → İlk yıl ana gelir kaynağı

3. IP Marketplace Komisyonu:
   Her IP satışından %10-20 komisyon
   → Ekosistem büyüdükçe kendiliğinden büyür

4. Araştırma Hibeleri:
   NSF, DARPA, EU Horizon
   → Nöromorfik + fotonik uzantılar için mükemmel uyum
   → "Donanım güvenliği" temasında hibe imkânı

5. Bulut Simülasyon:
   Büyük tasarım → bulut paralel simülasyon
   Kullanım başına ücret (AWS benzeri)
   → Tek seferlik büyük maliyet yerine akışkan

YAKILACAK KARA YOL (kaçınılacaklar):
  ✗ "Hepsi ücretsiz" → sürdürülebilir değil
  ✗ "Hepsi ücretli" → topluluk oluşmaz
  ✗ Vendor'a satmak → vendor kilidi → topluluk güveni yok
  ✗ Akademi odaklı kalmak → endüstri benimsemesi olmaz
```

---

## Bölüm 6 — Geleceğe Yönelik Adımlar

### 6.1 Teknik Vizyon (2025-2035)

```
2025-2026 — TEMEL KURULUM:
  Volt MVP beta çıkışı
  İlk 1,000 aktif kullanıcı (FPGA topluluğu)
  5 referans ASIC tasarımı (Tiny Tapeout üzerinden)
  DAC 2026 bildirisi: "Volt: Tip-Güvenli HDL"

2027-2028 — BÜYÜME:
  Volt v1: Python API + NeuroTorch + Timeline L2
  İlk ticari ASIC: bir startup Volt ile tape-out
  100+ Volt IP paketi (topluluk + firma)
  3 üniversite müfredatında Volt

2029-2031 — OLGUNLUK:
  Volt v2: Spike<Trit> + Olay semantiği
  Nöromorfik FPGA çıkarım demosu (Loihi 2 + FPGA)
  NeuroCompiler: FPGA + Loihi hedefleri
  10,000+ aktif kullanıcı, 1M+ indir

2032-2035 — PARADIGMA DÖNÜŞÜMÜ:
  Volt v3: PTrit + fotonik + analog
  Fotonik ternary CIM prototipi (Volt ile tasarlandı)
  Mimari OS: ilk SoC demosu
  "Yeni donanım çağının HDL'i" olarak kabul
```

### 6.2 Stratejik Ortaklıklar

```
AÇIK KAYNAK TOPLULUKLAR:
  LLVM/CIRCT: doğrudan katkı → upstream güvenilirliği
  OpenHW Group: RISC-V standart kütüphane ortak geliştirme
  SymbiYosys/Yosys: resmi entegrasyon → araç olgunluğu

AKADEMİ:
  MIT CSAIL, Stanford EE, ETH Zürich: müfredat ortaklığı
  CMU ECE: doğrulama araştırması (CoWoS + Volt)
  Bilkent, ODTÜ, İTÜ: Türkiye'den güçlü başlangıç

ENDÜSTRİ:
  eFabless/Tiny Tapeout: kolay ASIC yolu = topluluk çekim
  TSMC Open Innovation Platform: erken entegrasyon
  Xilinx/AMD: Vivado eklentisi resmi desteği
  Arm: Volt'ta Arm IP örnek implementasyonu

YATIRIM:
  Seed: açık kaynak araç şirketlerine ilgi artıyor
    (Zed, Warp, Modal, Vercel tarzı)
  YC/a16z: "AI donanım altyapısı" temasına uyuyor
  DARPA ERI (Electronics Resurgence Initiative): tam uyum
```

### 6.3 Volt'un Rakip Olmadığı Yer

```
Volt şu alanda RAKIP DEĞİL:
  Cadence, Synopsys: sentez ve P&R → Volt tamamlayıcısı
  Verilog/SV: Volt çıktısı SV → birlikte çalışır
  MATLAB/Simulink: sistem seviyesi → Volt daha aşağı
  HLS araçları: davranışsal sentez → farklı soyutlama

Volt şu alanda DOĞRUDAN RAKIP:
  Chisel/SpinalHDL: Volt bağımsız dil, bunlar gömülü
  PyRTL/Amaranth: Python gömülü, Volt bağımsız + tip-güvenli
  Bluespec: sektörde az benimsendi, Volt daha geniş hedef

Volt'un kazanma şansı nerede:
  → Chisel'in "Scala yükünden kaçınmak isteyenler"
  → Amaranth'ın "Python dinamik tip sorunun farkındakiler"
  → Yeni başlayanlar (Verilog tuzaklarını bilmeyenler)
  → AI donanım startup'ları (hız + güvenlik dengesi)
```

---

## Bölüm 7 — Güçlü ve Zayıf Yönler: Ekosistem Geneli

### 7.1 Ekosistem Güç Analizi (SWOT)

```
GÜÇLÜ YÖNLER:
  ✓ Benzersiz konum: bağımsız + tip-güvenli + formal dahil
  ✓ Tek semantik model → en pahalı hataları önlüyor
  ✓ Rust ekibi kültürü ile uyumlu (güvenlik-önce)
  ✓ Nöromorfik + fotonik → gelecek paradigmalar için hazır
  ✓ CIRCT: LLVM ekosistemi desteği
  ✓ LLM-dostu yapı → AI destekli geliştirme avantajı

ZAYIF YÖNLER:
  ✗ Sıfır ekosistem başlangıçta
  ✗ CIRCT bağımlılığı → upstream riski
  ✗ UVM yok → büyük ASIC ekiplerinden direnç
  ✗ Analog desteği yok (uzun vadeli kısıt)
  ✗ Farkındalık sıfır → uzun benimseme yolu
  ✗ Küçük ekip → her şeyi yapmak imkânsız

FIRSATLAR:
  ○ AI çip patlaması: yeni donanım ekipleri araç arıyor
  ○ Açık kaynak EDA olgunlaşıyor (Yosys, OpenROAD)
  ○ RISC-V topluluğu: açık donanım kültürü büyüyor
  ○ Nöromorfik donanım geliyor: araç yokluğu boşluk
  ○ LLM + donanım co-design: yeni iş akışları

TEHDİTLER:
  ⚠ Büyük firma (Google, Intel) kendi açık HDL çıkarırsa
  ⚠ Chisel topluluğu güçlenirse (JVM sorununu çözerlerse)
  ⚠ CIRCT API kırılırsa geçiş maliyeti yüksek
  ⚠ Ticari model bulunamazsa sürdürülebilirlik sorunu
  ⚠ İlk önemli referans müşteri gelişmezse momentum kaybı
```

---

## Bölüm 8 — Öncelikli Sonraki Adımlar

### Acil (Önümüzdeki 6 Ay)

```
1. F0-F1 kodlamaya başla (bu hafta)
   → Workspace, CI, ADR-0001..5, Counter → SV → Verilator

2. Topluluk kurulumu (Ay 1-2)
   → GitHub org, Discord/Matrix, RF proses, katkı rehberi

3. İlk gerçek tasarım (Ay 3)
   → Bir RISC-V ALU veya UART Volt'ta yazılmış, README + video

4. CIRCT topluluğuyla temas (Ay 1)
   → LLVM Discourse, RFC okuma, mini spike

5. Akademi ortaklığı ön görüşmeler (Ay 2-3)
   → Bir üniversite, "pilot müfredat" anlaşması

6. Ticari model kararı (Ay 4)
   → "Açık çekirdek" mi, "tamamen açık + hibe" mi?
   → Bu karar her şeyi belirliyor
```

### Orta Vade (6-18 Ay)

```
1. Playground: Ay 6 önce canlı olmalı
   → "Volt dene" = topluluk büyümesinin motoru

2. İlk Tiny Tapeout: Ay 12 hedef
   → Volt ile tasarlanan çip üretimden çıkmış = referans

3. NeuroTorch Aşama 0: Ay 12
   → PyTorch üzerinde Spike<T> tip anotasyonu prototipi

4. Volt Foundation kurulumu: Ay 18
   → Yönetim modeli, sektör üyeleri
   → "Volt'a katkıda bulunmak güvenli" mesajı
```

---

## Özet Tablosu

```
Bileşen              Hazır Yıl  Hedef Kitle        Kritik Risk
────────────────────────────────────────────────────────────────────
Volt Dili            MVP (Y1)   Herkes             Ekosistem yokluğu
Derleyici            MVP (Y1)   Herkes             CIRCT bağımlılığı
LSP                  MVP (Y1)   Hobi+Startup       F5'e erteleme riski
Simülatör            MVP (Y1)   Hobi+Öğrenci       Büyük tasarımda hız
Formal               MVP (Y1)   Startup+ASIC       K-framework gecikmesi
Stdlib               MVP (Y1)   Herkes             İncelik başta
Pkg Yöneticisi       MVP (Y1)   Startup+Büyük      Registry yokluğu
Playground           v1 (Y2)    Hobi+Öğrenci       Geliştirme önceliği
Python API           v1 (Y2)    Araştırmacı        Ternary ile uyum
NeuroTorch           v2 (Y3-4)  Nörobilim+Edge     Donanım beklentisi
NeuroCompiler        v2 (Y3-4)  Araştırmacı        NP-zor eşleme
Spike<T>             v2 (Y3-4)  Nöromorfik         Anlambilim tasarımı
PTrit+Fotonik        v3 (Y5+)   Araştırma          Donanım hazır değil
Mimari OS            v3 (Y5+)   İleri ASIC         Çok iddialı
────────────────────────────────────────────────────────────────────

Tek cümle özet:
Volt'un ekosistem stratejisi "FPGA topluluğundan başla,
startup'a geç, büyük ASIC'e ulaş" sırasını izler;
teknik üstünlüğü (tip güvenliği, tek semantik, formal dahil)
önce güven, sonra araç olgunluğu olarak sunar;
ve nöromorfik + fotonik uzantılarla 10 yıl sonrasının
donanım paradigmalarına hazırlıklı konumlanır.
```
