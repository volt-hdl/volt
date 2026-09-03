> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Yeni HDL Yazanlar — Ekosistem, Motivasyon ve Sürtünme Azaltma

---

## Bölüm 1 — Günümüzde Yeni HDL Yazanlar

### Aktif Projeler Haritası

Düşündüğünüzden çok daha kalabalık bir alan:

```
DİL          TEMEL        YAŞ    EKİP     DURUM
──────────────────────────────────────────────────────────────────
Chisel       Berkeley     2012   Akademi  Aktif, büyük topluluk
SpinalHDL    C.Papon      2014   Bireysel Aktif, tutarlı geliştirme
Amaranth     C.Whitequark 2016   Açık K.  Aktif, FPGA odaklı
ROHD         Intel        2021   Şirket   Aktif (Dart tabanlı)
Filament     Cornell      2022   Akademi  Araştırma (timeline tipler)
Veryl        Açık K.      2022   Topluluk Aktif, Rust sentaksı
Silice       Inria        2020   Araştırma Aktif (FPGA odaklı)
Blarney      Cambridge    2018   Akademi  Aktif (Haskell)
Clash        —            2010   Akademi  Aktif (Haskell → HDL)
HardCaml     Jane Street  2014   Şirket   İç kullanım + açık
Magma        Stanford     2018   Akademi  Python, AI çip odaklı
PyRTL        UCSB         2015   Akademi  Eğitim odaklı
Lava         Chalmers     2000   Akademi  Ata, hâlâ referans
Bluespec BSV Bluespec Inc 2000   Ticari   Açık kaynak oldu (2020)
```

**Veryl özellikle dikkat çekici:**

```
Veryl (2022):
  Rust benzeri sözdizimi + modern özellikler
  Ancak: Verilog semantiği korunuyor
         (Verilog'un modernize edilmiş versiyonu)
  
  Volt'tan farkı:
  Veryl: "Verilog'u daha güzel yazmak"
  Volt:  "Verilog'un çözemediği sorunları çözmek"
         (CDC tipte, tek semantik model, nöromorfik...)
  
  Örtüşen alan var — dikkat edilmeli.
  Volt'un farklılaşması net olmalı.
```

### Kaç Kişi

```
Aktif HDL projesi geliştiren:        ~50-100 dünyada
Akademik HDL/donanım araştırmacısı:  ~500-1000
FPGA/ASIC araçlarına katkı yapan:    ~2000-5000
"Yeni HDL fikri" üzerinde düşünen:  ~10,000+

Sonuç: "yeni HDL" alanı kalabalık ama
       çoğu proje 1-3 yılda ölüyor.
       Hayatta kalanlar: Chisel, Amaranth, SpinalHDL
```

---

## Bölüm 2 — Motivasyonlar

### Motivasyon 1 — Kişisel Hayal Kırıklığı (En Yaygın)

```
"Bu hatayı saatler arayıp kaynağın
 sensitivity list olduğunu bulunca
 sinir krizine girdim."
 
 → SpinalHDL böyle başladı (Charles Papon, kendi projesi)
 → Amaranth böyle başladı (Catherine'in frustration'ı)
 → Çoğu kişisel proje bu motivasyondan

Güçlü yön: gerçek acı → gerçek çözüm
Zayıf yön: tek kişinin acısı → evrensel mi?
```

### Motivasyon 2 — Akademik Yayın

```
"Bu yeni tip sistemini HDL'e uygularsam
 PLDI/ICCAD/ASPLOS makale çıkar."
 
 → Filament (timeline tipler) bu motivasyondan
 → Blarney (Haskell kategorik tipler)
 → Çoğu akademik proje
 
Güçlü yön: teorik derinlik, inovasyon
Zayıf yön: makale çıkınca proje durur
            kullanıcı değil, hakem hedefi
```

### Motivasyon 3 — Şirket İhtiyacı

```
"Chisel ya da Amaranth istediğimizi vermiyor,
 şirkete özgü araç yapalım."
 
 → Google'ın iç araçları (XLS, PyHDL)
 → Jane Street HardCaml
 → Intel ROHD (Dart tabanlı)
 
Güçlü yön: gerçek kullanım, sürekli bakım
Zayıf yön: şirket ihtiyacına göre şekillenir
            dışarıya açılması yavaş
```

### Motivasyon 4 — Demokratikleştirme

```
"ASIC tasarımı sadece büyük şirketlerin elinde.
 Açık araçlarla herkese açalım."
 
 → Yosys, OpenROAD bu ruhla
 → Amaranth kısmen bu motivasyonla
 → Tiny Tapeout bu hareketi hızlandırdı
 
Güçlü yön: topluluk bağlılığı, geniş kitle
Zayıf yön: ticari model kurmak zor
```

### Motivasyon 5 — AI Çip Dalgası (Yeni, Büyüyen)

```
"LLM ve özel AI donanımı patladı.
 Mevcut araçlar bu hızda yetmiyor."
 
 → Magma (Stanford, AI chip odaklı)
 → CIRCT (LLVM, modern IR ihtiyacı)
 → Veryl (modern sözdizimi, temiz çıktı)
 
Güçlü yön: pazar büyüyor, finansman var
Zayıf yön: çok rekabetçi, büyük oyuncular giriyor
```

---

## Bölüm 3 — Neden Çoğu Proje Ölüyor

Bunları bilmek Volt'u farklı konumlandırmak için kritik:

```
ÖLÜM NEDENİ 1 — "Tek kişi tükenmesi":
  Başlangıç: heyecan + hızlı ilerleme
  6. ay: gerçek zorluk başlar
  12. ay: topluluk yok, feedback yok
  18. ay: proje ölür
  
  Korunma: ikinci kişi erkenden, topluluk önce

ÖLÜM NEDENİ 2 — "Makale sonrası terk":
  ICCAD'a kabul edildi → dil bitti!
  Kod GitHub'da duruyor, bakım yok
  
  Korunma: akademi ile ilişki ama bağımlı olma

ÖLÜM NEDENİ 3 — "Yanlış soyutlama seviyesi":
  Ya çok yüksek (HLS gibi, ne çıkacağı belirsiz)
  Ya çok düşük (Verilog'dan farkı yok)
  Kullanıcı "neden bu?" sorusunu sorar
  
  Korunma: değer önerisi net ve somut

ÖLÜM NEDENİ 4 — "Ekosistem olmadan dil ölür":
  Güzel dil + sıfır kütüphane = kullanılamaz
  Stdlib, örnekler, IDE desteği olmadan kimse kalmaz
  
  Korunma: stdlib F5'te değil F2'de başlar

ÖLÜM NEDENİ 5 — "Çok geniş hedef":
  "Hem hobici hem ASIC hem nöromorfik"
  Hiçbirini iyi yapamamak
  
  Korunma: aşama 1'de FPGA + öğrenci SADECE
```

---

## Bölüm 4 — Dört Sürtünme Noktası: Somut Çözümler

### 4.1 "CIRCT'i Rust'tan Çağırmak Karmaşık"

**Sorunun gerçek boyutu:**

```
CIRCT C++ ile yazılmış.
Rust'tan C++ çağrısı: C++ ABI stabil değil
  → Symbol mangling değişir
  → Sürüm geçişlerinde kırılır
  → Güvenli değil

Ama CIRCT bir çözüm sunuyor: MLIR C-API
  → C ABI stabil (C dili garanti)
  → CIRCT ekibi bunu korumayı taahhüt etti
  → Rust'tan güvenle çağrılabilir
```

**Sürtünme Azaltma Stratejisi:**

**Adım 1 — `melior` crateni değerlendir:**

```rust
// melior: MLIR için Rust wrapper (hazır kütüphane)
// Sıfırdan FFI yazmak yerine bu kullanılabilir

use melior::{
    Context,
    dialect::{arith, func},
    ir::{Block, Location, Module, Region, Type, Value},
};

// MLIR context oluştur
let context = Context::new();
context.append_dialect_registry(&registry);
context.load_all_available_dialects();

// Volt HIR → MLIR bu kütüphane üzerinden
// FFI kod yazmak gerekmez
```

**Adım 2 — `volt-lower` crate'i tamamen izole et:**

```
workspace/
├── volt-span/
├── volt-diagnostics/
├── volt-syntax/
├── volt-ast/
├── volt-hir/
├── volt-lower/    ← CIRCT burada izole
│   ├── Cargo.toml (melior bağımlılığı sadece burada)
│   └── src/
│       ├── circt_backend.rs
│       └── firrtl_fallback.rs   ← Plan B buraya
└── volt-driver/
```

Sonuç: CIRCT API kırılırsa sadece `volt-lower` değişir. Geri kalan 26,000 satır etkilenmez.

**Adım 3 — Plan B'yi baştan yaz:**

```rust
// volt-lower/src/firrtl_fallback.rs
// CIRCT olmadan doğrudan FIRRTL metin üretimi
// Karmaşık değil: FIRRTL basit bir formatı

// CIRCT çalışıyorsa: CIRCT kullan
// CIRCT bozulduysa: bu kod devreye girer

pub fn emit_firrtl(module: &HirModule) -> String {
    // FIRRTL text formatı elle üretilebilir
    // "circuit Counter :\n  module Counter :\n..."
    // F3 aşamasında bu Plan B olarak hazır olur
}
```

**Adım 4 — CIRCT topluluğuyla erkenden temas:**

```
LLVM Discourse → CIRCT alt forumu
GitHub: llvm/circt → Discussions

Sormak istenen:
"Veri akışı semantiği olan yeni HDL üzerinde
 çalışıyorum. reg + clock domain semantiğini
 hangi dialect en iyi karşılar: seq mi FIRRTL mi?
 Küçük örnek: [kod]"

Bu soruyu soran biri daha önce CIRCT ekibini
aldatmış değil — bu tür sorular hoş karşılanır.
Sonuç: doğru karar + toplulukla ilk temas.
```

---

### 4.2 "Ternary Fotonik Stabil Değil"

**Sorunun gerçek boyutu:**

```
GST PCM drift problemi: gerçek
MZI sıcaklık hassasiyeti: gerçek
Si-fotonik foundry olgunluğu: henüz sınırlı
2024'te ticari fotonik ternary: YOK

Bu itiraz teknik olarak DOĞRU.
```

**Ama yanlış soruya yanıt:**

```
Volt'un fotonik ternary desteği = V3 = 5+ yıl sonra

MVP için fotonik ternary:
  → Gerekli mi? HAYIR
  → V3'e kadar donanım yok, ama tip sistemi tasarlanabilir
  → "Donanım olmadan dil tasarımı" olağan bir pratik
     (RISC-V 2010'da tasarlandı, çipleri yıllar sonra geldi)

Sürtünme azaltma:
  Bu itiraza verilecek cevap:
  "Haklısınız — fotonik ternary şu an V3 hedefimizde.
   MVP'de SV çıktısı, FPGA hedefi var.
   PTrit tipi teorik olarak tanımlı, donanım gelince
   lowering eklenecek. Bugünkü değer:
   [CDC, latch, tip güvenliği, single semantics]"
   
  Yani: bu itirazı kabul et, MVP'yi savun.
  MVP'yi fotonik üzerine kurmaya çalışma.
```

**Pratik: Fotonik'i "opsiyonel uzantı" olarak konumlandır:**

```
Volt ana gövdesi → doğrulanabilir, çalışır, bugün faydalı
Volt-photonic    → experimental crate, V3 yol haritasında

Bu ayrım şeffaf olunca itiraz moot olur:
"Fotonik stabil değil" → "evet, o yüzden experimental"
```

---

### 4.3 "Bounded Formal Yetmez"

**Sorunun gerçek boyutu:**

```
Bounded model checking ne der:
  "Bu tasarım ilk N adımda güvenli"
  
Unbounded ne der:
  "Bu tasarım sonsuza kadar güvenli"

Büyük ASIC şirketleri unbounded ister:
  JasperGold, Cadence Jasper → unbounded BMC
  Synopsys VC Formal → unbounded
  Bu araçlar lisansı: yıllık $500K+
  
Bu itiraz büyük ASIC şirketleri için DOĞRU.
```

**Ama bu itirazın hedef kitlesi kim:**

```
r/FPGA kullanıcısı:
  "Bounded formal yetmez" demiyor.
  Formal doğrulamayı hiç kullanmıyor.
  Volt'un bounded formal'i devrimsel.

AI chip startup'ı (20 kişi):
  JasperGold lisansı alamaz ($500K/yıl).
  SymbiYosys ile bounded → büyük kazanım.
  Volt onlar için yeterli.

Büyük ASIC şirketi:
  JasperGold zaten var.
  Volt SVA assertion üretiyor → JasperGold okur.
  Volt onların formal aracını KAPSAMIYOR,
  RTL kalitesini artırıyor.
  Bu itiraz aslında geçersiz.
```

**Sürtünme azaltma — üç katman:**

```
Katman 1 (bugün): SymbiYosys entegrasyonu
  assert not(state == DEADLOCK)
  → SymbiYosys BMC → N adımda kanıtlanır
  → Hobi + startup için yeterli

Katman 2 (V1): SVA export
  Volt assertion → SystemVerilog Assertion (SVA)
  → JasperGold okur → unbounded kanıt
  → Büyük ASIC şirketleri mevcut araçlarını kullanır
  → Volt onların aracını bilmeden entegre olur

Katman 3 (araştırma): K-framework
  Tam semantik kanıt → uzak vade
  → Akademik katkı olarak değerli
  → Ürün gereksinimi değil

Bu üç katman şeffaf belgelenince:
"Bounded yetmez" → "JasperGold'u SVA ile kullanın"
→ itiraz yanıtlanmış
```

---

### 4.4 "UVM Olmadan Büyük ASIC Olmaz"

**Sorunun gerçek boyutu:**

```
UVM (Universal Verification Methodology):
  SystemVerilog üzerinde kurulu test framework
  Sequence, Driver, Monitor, Scoreboard, Agent...
  Büyük ASIC şirketlerinin standardı
  Öğrenmesi 6-12 ay
  VCS/Xcelium gerektirir (pahalı)
  
Bu itiraz tamamen DOĞRU: büyük ASIC = UVM.
```

**Ama büyük ASIC MVP hedefi değil:**

```
Hedef kitle önceliği (tekrar):
  Aşama 1: FPGA hobi + öğrenci → UVM bilmiyor, kullanmıyor
  Aşama 2: AI chip startup → UVM kullanmak istiyor ama
                              lisans parası yok, ekip küçük
  Aşama 3: Büyük ASIC → UVM şart (Yıl 4+)

Aşama 1-2 için gerçekçi alternatifler:
```

**Sürtünme azaltma — kademeli:**

```
Seviye 1 (bugün): cocotb köprüsü
  Python'da test yaz → Volt SV çıktısını test et
  cocotb: UVM'nin açık kaynak alternatifi
  "UVM bilmeden doğrulama" → hobi + startup yeterli

Seviye 2 (V1): Volt'tan cocotb jeneratörü
  module Counter { ... }
  → volt test generate --cocotb
  → counter_tb.py otomatik üretilir
  → Temel test kodu hazır
  
Seviye 3 (V2): SV UVM testbench uyumu
  Volt SV üretir → UVM testbench bu SV'yi test eder
  UVM kodu Volt bilmeden çalışır
  Büyük ASIC şirketi: UVM'yi değiştirmez, Volt RTL kullanır

Seviye 4 (uzun vade): Volt UVM kütüphanesi
  UVM class hiyerarşisini Volt'ta modelle
  → "Volt-native UVM" — iddialı ama mümkün
```

**İtirazı nasıl karşılarsın:**

```
"UVM olmadan büyük ASIC olmaz" diyene:

"Haklısınız — büyük ASIC için UVM standart.
 Volt SV ürettiği için mevcut UVM testbenchiniz
 Volt ile yazılmış RTL'i sorunsuz test eder.
 
 Volt'un değeri UVM'yi değiştirmek değil:
 RTL yazarken CDC hatalarını önlemek,
 formal assertion'ları dilde tutmak,
 sentez öncesi PPA tahmin etmek.
 
 Bunlar UVM'den önce geliyor — RTL kalitesini
 UVM'ye girmeden artırıyoruz."

Bu cevap:
  → İtirazı kabul ediyor (dürüst)
  → Volt'un değerini başka yerde gösteriyor
  → Çatışmıyor, tamamlayıcı konumlanıyor
```

---

## Bölüm 5 — Volt'u Ölüm Tuzaklarından Ayıran Şeyler

```
DİĞERLERİ                    VOLT'UN FARKI
──────────────────────────────────────────────────────────────
Tek motivasyon                Çok katmanlı:
(akademi VEYA hobi)           teknik + stratejik + vizyon

Dil = araç                    Dil = ekosistem kapısı:
                              nöromorfik, fotonik, CIM...

Mevcut sorunları çözer        Gelecek paradigmaları da kapsar:
                              Spike<T>, PTrit, mimari OS

"Makale çıktı, bitti"         Açık çekirdek iş modeli:
                              sürdürülebilirlik planlandı

Topluluk sonradan             Topluluk stratejisi önceden:
                              FPGA → startup → ASIC sırası

SV çıktısı değil              SV çıktısı → araç uyumu otomatik:
(veya yorumlanmış)            Vivado, DC, Calibre bilmeden çalışır

Tek paradigma                 Çok paradigma:
(klasik dijital)              ternary, nöromorfik, fotonik hazır

Zayıflıklar gizlenir         Zayıflıklar belgelendi, çözüm planı var:
                              bounded formal → SVA export
                              UVM yok → cocotb, tamamlayıcı konum
```

---

## Özet

```
Yeni HDL yazanlar:
  ~50-100 aktif proje, çoğu 1-3 yılda ölüyor
  Motivasyon: hayal kırıklığı, akademi, şirket ihtiyacı, AI dalgası
  Hayatta kalanlar: Chisel, Amaranth, SpinalHDL (güçlü topluluk)
  
  Veryl dikkat çekici — sözdizimi benzer, semantik farklı.
  Volt'un farklılaşması net olmalı.

Dört sürtünme noktası:

  CIRCT karmaşık:
  → melior crate + volt-lower izolasyonu + FIRRTL fallback
  → CIRCT topluluğuyla erken temas → doğru kararlar

  Fotonik stabil değil:
  → V3 hedefi, experimental etiket, MVP'yi savun
  → Bu itirazı kabul et, MVP'nin değerini göster

  Bounded yetmez:
  → Hedef kitleye göre: hobi için yeter, büyük ASIC için SVA export
  → JasperGold SVA okur — Volt onun yerine geçmiyor, besliyor

  UVM yok:
  → Tamamlayıcı konum: RTL kalitesi UVM'den önce gelir
  → Cocotb şimdi, SVA uyum V1, Volt-native V2+
  → "Rakip değil, tamamlayıcı" çerçevesi

Hepsinin ortak çözümü:
  Her itirazı kendi hedef kitlesinde değerlendir.
  Hobi kullanıcısı UVM sormaz.
  Büyük ASIC zaten JasperGold'u var.
  Startup ikisi arasında — Volt tam orada.
```
