> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Clash HDL ve Veryl HDL — Detaylı İnceleme Raporu

> Kaynak: clash-lang.org, github.com/veryl-lang/veryl, akademik makaleler
> ve birincil belgeler taranarak hazırlanmıştır.

---

# BÖLÜM A — CLASH HDL

---

## A.1 — Temel Kimlik

```
Adı:      CλaSH (Clash)
Çıkış:    2009 (16 yıllık en eski aktif alternatif HDL!)
Kurum:    Twente Üniversitesi → QBayLogic (ticari şirket)
Lisans:   BSD-2-Clause (çok permissive)
Çıktı:    VHDL, Verilog, SystemVerilog
Dil:      Haskell (ev sahibi dil olarak kullanılıyor)
Derleyici: GHC (Glasgow Haskell Compiler) arka ucu
GitHub:   clash-lang/clash-compiler (aktif, onlarca katkıcı)
İş modeli: QBayLogic ticari destek satıyor
Afilyasyon: Haskell Foundation affiliate projesi
```

---

## A.2 — Temel Çalışma Prensibi: Signal Tipi

Clash'i anlamak için Signal tipini anlamak şart.

### A.2.1 Signal Nedir

<cite index="98">Signal tipi donanım çevirisinde çalışıyor çünkü implementasyonu public API'den gizlenmiş. Datayı hâlâ kullanışlı yapmak için primitifler sunuluyor: pure (sinyal oluştur), fmap (saf fonksiyon/kombinasyonel devre uygula), liftA2 (birden fazla sinyali birleştir), register (durum ekle).</cite>

```haskell
-- Signal: saat alanıyla etiketlenmiş sonsuz liste
data Signal (dom :: Domain) (a :: Type) = a :- Signal dom a
--                ^^^                              ^^^
--          fantom tip parametresi         cons operatörü
--          (derleme zamanı alan etiketi)  (lazy liste)

-- Kullanım:
>>> signal = pure 1 :: Signal SystemDomain Int
>>> signal
1 :- 1 :- 1 :- 1 :- 1 :- ^C

-- Combinational (alan etiketi aynı olmalı):
>>> liftA2 (+) signal signal
2 :- 2 :- 2 :- 2 :- 2 :-
```

### A.2.2 CDC Derleme Zamanında

<cite index="98">Farklı alan sinyalleri birleştirilmeye çalışıldığında Haskell derleyicisi engeller. Bu yanlışlıkla saat alanlarını asla geçemeyeceğiniz anlamına gelir.</cite>

```haskell
-- CDC HATASININ DERLEME ZAMANINDA YAKALANMASI:

>>> a = pure 3 :: Signal A Int   -- A alanında sinyal
>>> b = pure 5 :: Signal B Int   -- B alanında sinyal
>>> liftA2 (+) a b
*** Type error: could not deduce A ~ B
--                                    ^^^
--                   A ve B'nin aynı alan olduğunu çıkaramadı
--                   → Derleme hatası, runtime değil!

-- Bilinçli CDC için unsafeSynchronizer:
unsafeSynchronizer :: Clock dom1 -> Clock dom2 -> Signal dom1 a -> Signal dom2 a
-- "unsafe" ön eki: bilerek yapılan geçiş, otomatik değil
```

### A.2.3 Auto-Pipelining

<cite index="90">Clash tip seviyesinde gecikmeleri takip ederek sinyalleri hizalamak için gereken register sayısını otomatik eklüyor. a*b + c ifadesinde c, çarpmanın hesaplanması kadar gecikmeli olmalı. delayI bu kayıt eklemeyi otomatik yapıyor.</cite>

```haskell
-- Auto-pipelining örneği:
-- a * b → 3 saat döngüsü süriyor (çarpan gecikme)
-- a + b → 1 saat döngüsü

macResult = fst (result, delay) where
    product  = mul a b          -- 3 döngü gecikme
    delayed_c = delayI (delayI (delayI c))  -- c'yi 3 gecik
    result   = add product delayed_c

-- delayI: tip bilgisinden kaç delay gerektiğini biliyor!
-- Manuel pipeline hesabı YOK
```

---

## A.3 — Tip Sistemi: Tam Haskell Gücü

### A.3.1 Mevcut Özellikler

```haskell
-- 1. Sabit uzunluklu vektörler
type Vec (n :: Nat) a -- n: tip seviyesinde doğal sayı

-- 2. Tip seviyesinde hesaplama
type TotalWidth n m = n + m  -- derleme zamanı aritmetik

-- 3. Sınıflar (type class) ile kısıtlama
class NFDataX a => SomeHardwareClass a where
    encode :: a -> BitVector 8

-- 4. Monad/Functor hiyerarşisi
-- Sinyal işlemleri Applicative ve Functor kullanıyor

-- 5. Yüksek mertebeli fonksiyonlar
map :: (a -> b) -> Vec n a -> Vec n b
fold :: (a -> a -> a) -> Vec (n+1) a -> a
```

### A.3.2 GHC Eklentileri

Clash GHC'yi arka uç olarak kullandığı için tüm modern Haskell uzantıları erişilebilir:

```haskell
{-# LANGUAGE DataKinds #-}       -- tip seviyesinde literal
{-# LANGUAGE TypeFamilies #-}    -- tip ailesi hesaplamaları
{-# LANGUAGE GADTs #-}           -- genelleştirilmiş ADT
{-# LANGUAGE TypeOperators #-}   -- tip operatörleri (:+:, .=.)

-- Bu uzantılarla donanım seviyesinde olmayan kısıtlamalar
-- bile ifade edilebilir (tip teorisi seviyesinde)
```

---

## A.4 — Araçlar ve Ekosistem

### A.4.1 REPL (GHCi)

```
En güçlü özelliklerden biri:
Haskell'in interaktif ortamı (GHCi) donanım simülasyonu için kullanılabilir.

>>> :load my_design.hs
>>> simulate counter []
[0, 1, 2, 3, 4, 5, ...]

Test bench yazılmadan hızlı keşif mümkün.
Bu hiçbir Verilog/VHDL aracında yok!
```

### A.4.2 Çıktı Kalitesi

```haskell
-- Clash kodu:
counter :: HiddenClockResetEnable System
        => Signal System (Unsigned 8)
counter = register 0 (counter + 1)
```

```systemverilog
// Üretilen SV (Clash çıktısı):
module counter
  ( input  wire       clk
  , input  wire       rst
  , output wire [7:0] result
  );
  reg [7:0] count_r;
  assign result = count_r;
  always @(posedge clk) begin
    if (rst) count_r <= 8'd0;
    else     count_r <= count_r + 1;
  end
endmodule
```

Çıktı okunabilir ve makul ama Arch/Veryl kadar "insan yazmış gibi" değil.

### A.4.3 QBayLogic Desteği

<cite index="92">Clash sisteminin yaratıcısı ve birincil bakımcısı olarak QBayLogic, bu güçlü teknolojiden yararlanmak isteyen kuruluşlara uzmanlık ve Clash desteği sunuyor: mevcut HDL bileşenleriyle Clash tasarımlarını entegre etme, en iyi pratik konusunda rehberlik.</cite>

---

## A.5 — Gerçek Dünya Kullanımı: Google Bittide

<cite index="90">Clash'i birincil donanım tanımlama dili olarak kullanıyoruz. Clash'in bu tasarım için özellikle yararlı olan üç özelliği: saat alanı geçişi güvenliği, kayan nokta işlemlerinin otomatik pipeline'ı ve genel amaçlı programlama — donanım tanımı, deney üretimi, veri işleme ve diyagram oluşturma hepsi aynı Haskell kodu içinde.</cite>

```
Google Bittide nedir:
  Küresel deterministik zamanlama altyapısı
  Dağıtık sistem saat senkronizasyonu
  Google üretim altyapısında çalışıyor

Clash kullanım motivasyonları:
  - CDC güvenliği: transceiver mantığında kritik
  - Auto-pipelining: kayan nokta senkronizasyonu
  - Tek dil: hem donanım hem analiz aynı Haskell
```

---

## A.6 — Clash'in Zayıf Yönleri (Dürüst Değerlendirme)

```
1. Haskell öğrenme eğrisi — EN BÜYÜK ENGEL:
   - Monad, Functor, Applicative anlamak şart
   - Tembel değerlendirme (lazy evaluation) anlamak şart
   - Tip sınıfları (type classes) anlamak şart
   - GHC uzantıları anlamak gerekebilir
   - Tahmini öğrenme süresi: 3-12 ay (deneyime bağlı)

2. Hata mesajları:
   - Haskell tip hataları → kriptik
   - "could not deduce" mesajları donanım hatalarını
     gizleyebilir
   - Spade/Arch kalitesine kıyasla çok zayıf

3. Sentez çıktısı:
   - Üretilen kod büyük ve karmaşık olabilir
   - FF seviyesinde elle düzenleme zor
   - ASIC timing ECO workflows için uygun değil
   - Veryl bu sorunu açıkça çözdü

4. Donanım-özgü sözdizimi yok:
   - Haskell sözdizimi → donanım sezgisine yabancı
   - always_ff benzeri yok
   - Yeni başlayanlar için karmaşık

5. Simülasyon vs sentez tutarsızlığı:
   - unsafeSynchronizer simülasyonda "zaman yolculuğu" yapar
   - Signal soyutlaması bazen sızdıran soyutlama

6. UVM/cocotb entegrasyonu yok:
   - Doğrulama Haskell'de veya ayrı araçlarla
   - Büyük ASIC doğrulama akışlarıyla entegrasyon zor
```

---

## A.7 — Volt için Ders

```
Clash'ten öğrenilecekler:

1. Signal<dom, a> tipi → Volt @Domain anotasyonu
   İkisi aynı problemi farklı sözdizimle çözüyor
   Volt'un yaklaşımı: @Domain anotasyonu, daha açık

2. unsafeSynchronizer isimlendirme:
   "unsafe" ön eki → "bu bilinçli, dikkatli ol" mesajı
   Volt'ta CDC köprüsü explicit gerekiyor — benzer mantık

3. Auto-pipelining delay bilgisi:
   Clash: delayI ile tip bilgisinden delay ekle
   Volt L1: Delayed<T,N> benzer fikir
   Aynı problem, farklı çözüm

4. REPL:
   Clash GHCi ile test bench olmadan test ediyor
   Volt'ta benzer: volt sim --interactive (gelecek?)

Clash'ten ALINMAYACAK:
   Haskell bağımlılığı → Volt'un en güçlü rekabet noktası:
   "Clash'in CDC güvenliği, Haskell öğrenmeden"
```

---

---

# BÖLÜM B — VERYL HDL

---

## B.1 — Temel Kimlik

```
Adı:     Veryl (Verilog + Beryl minerali)
Çıkış:   2022 (ilk commit), aktif geliştirme
Yazar:   Naoya Hatta (baş geliştirici), Taichi Ishitani,
         Ryota Shioya, Nathan Bleier (katkıcı)
Lisans:  Apache-2.0 OR MIT (çift lisans, çok permissive)
Çıktı:   Okunabilir IEEE 1800-2017 SystemVerilog
Dil:     Bağımsız (Rust ile yazılmış derleyici)
GitHub:  938 yıldız, 62 fork (Mayıs 2026)
         v0.20.0 en son sürüm (Mayıs 2026)
         4,456 commit — çok aktif!
Yayınlar:DVCon Japan 2024, ISCA 2025, VLSI 2025, DSF 2025
```

---

## B.2 — Tasarım Felsefesi: "SV'nin Daha İyi Hali"

Veryl'in temel kararı diğer tüm HDL'lerden farklı:

<cite index="106">Veryl, SV uzmanları için tanıdık temel sözdizimini korurken mantık tasarımı için optimize edilmiş sözdizimi benimsiyor. Mevcut alt-HDL'lerin çoğu bir programlama dilinin iç DSL'i. Bu yaklaşımın avantajları var ama sözdizimi donanım tanımı için tam uygun olamıyor. Ayrıca bu dillerden kısa ve sofistike koddan çok büyük Verilog kodu üretiliyor. Bu durum zamanlama iyileştirme, ön/son-mask ECO gibi genel ASIC iş akışlarını engelliyor.</cite>

```
Veryl'in BILINÇLI kararları:

1. Neden otomatik pipeline yok?
   "Veryl, dil semantiği açısından SystemVerilog ile
    eşdeğerliğe odaklanıyor. Bu, Veryl kodundaki
    değişikliklerin üretilen SV'yi nasıl değiştireceğini
    tahmin etmeyi kolaylaştırıyor — ASIC timing ECO
    için şart."

2. Neden off-side rule (girinti bazlı) yok?
   "Sözdizimi basitliğine odaklanıyoruz — araç
    implementasyon çabasını azaltmak için."

3. Neden Chisel gibi değil?
   "Mevcut SV ile birlikte çalışabilirlik kritik."
```

---

## B.3 — Sözdizimi: Veryl vs SV Karşılaştırma

```veryl
/// Markdown formatında dokümantasyon yorumu
/// * liste öğesi 1
/// * liste öğesi 2
pub module Delay #(
    param WIDTH: u32 = 1,  // trailing comma izni
) (
    i_clk : input  clock       ,  // saat tipi
    i_rst : input  reset       ,  // reset tipi
    i_data: input  logic<WIDTH>,  // generic genişlik
    o_data: output logic<WIDTH>,
) {
    var _unused_variable: logic;  // _ önekli → uyarı yok

    // saat ve reset sinyalleri çıkarımlı (yazılmıyor!)
    always_ff {
        // reset polaritesi ve senkronluğu soyutlandı
        if_reset {
            o_data = '0;
        } else {
            o_data = i_data;
        }
    }
}
```

```systemverilog
// Veryl'in ürettiği okunabilir SV:
module Delay #(
    parameter int WIDTH = 1
) (
    input              i_clk ,
    input              i_rst ,
    input  [WIDTH-1:0] i_data,
    output [WIDTH-1:0] o_data
);
    logic unused_variable;
    always_ff @ (posedge i_clk or negedge i_rst) begin
        if (!i_rst) begin
            o_data <= '0;
        end else begin
            o_data <= i_data;
        end
    end
endmodule
```

**Kritik fark:** Üretilen SV tamamen okunabilir ve FF seviyesinde düzenlenebilir. ASIC timing ECO mümkün.

---

## B.4 — Dil Özellikleri Detay

### B.4.1 Tip Sistemi: SV-Uyumlu

```veryl
// Temel tipler (SV'ye karşılık geliyor):
var a: logic<8>;        // logic [7:0] a
var b: bit<16>;         // bit  [15:0] b
var c: uint<32>;        // u32 eşdeğeri
var d: sint<16>;        // s16 eşdeğeri

// Enum tipler:
enum State: logic<2> {
    Idle  = 2'b00,
    Run   = 2'b01,
    Done  = 2'b10,
}

// Struct tipler:
struct Packet {
    data: logic<8>,
    valid: logic,
    ready: logic,
}

// Interface (SV benzeri):
interface AXI_Lite #(
    param DATA_WIDTH: u32 = 32,
    param ADDR_WIDTH: u32 = 32,
) {
    var awaddr: logic<ADDR_WIDTH>;
    var awvalid: logic;
    var awready: logic;
    ...
}
```

### B.4.2 Modport (SV Birlikte Çalışma)

```veryl
// SV interface ve modport ile doğrudan uyum
// Mevcut SV IP ile sorunsuz entegrasyon
interface BusIf {
    var data: logic<32>;
    var valid: logic;

    modport Master {
        data  : output,
        valid : output,
    }

    modport Slave {
        data  : input,
        valid : input,
    }
}

module MyModule (
    bus: modport BusIf::Master,  // tip-güvenli yön
) { ... }
```

### B.4.3 Always Blokları (SV Soyutlaması)

```veryl
// if_reset → reset polaritesi ve senkronluğu otomatik
// Veryl.toml'da belirleniyor:
// reset_type = "sync_low"  // negedge senkron
// reset_type = "async_high" // posedge asenkron

always_ff {
    if_reset {           // hangi reset olursa olsun
        count = '0;
    } else if enable {
        count = count + 1;
    }
}

// always_comb → latch önleme
always_comb {
    case state {
        State::Idle: out = '0;
        State::Run:  out = data;
        State::Done: out = '1;
        // eksik durum → derleme uyarısı/hatası
    }
}
```

---

## B.5 — Araç Ekosistemi (Çok Güçlü)

### B.5.1 Paket Yöneticisi (Veryl.toml)

```toml
# Veryl.toml
[package]
name = "my_project"
version = "0.1.0"

[dependencies]
# Git tabanlı bağımlılık
axi_lib = { git = "https://github.com/example/axi_lib", version = "^1.0" }
# Local bağımlılık
utils = { path = "../utils" }

[build]
target = "xilinx"  # Vivado hedefi

[format]
indent_width = 4
```

```bash
veryl build    # derleme → SV üret
veryl check    # lint ve tip kontrol
veryl fmt      # otomatik biçimlendirme
veryl test     # test çalıştır
veryl doc      # dokümantasyon üret
veryl publish  # paket registry'ye yayımla
```

### B.5.2 LSP (Real-time Checker)

```
Desteklenen editörler (şu an):
  ✓ VS Code (resmi eklenti)
  ✓ Vim/Neovim
  ✓ Emacs
  ✓ Helix

Özellikler:
  ✓ Real-time hata gösterimi
  ✓ Otomatik tamamlama
  ✓ Tanıma git (Go to Definition)
  ✓ Hover bilgisi
  ✓ Otomatik biçimlendirme (on save)
```

### B.5.3 Playground

```
https://doc.veryl-lang.org/playground/

WebAssembly ile tam derleyici tarayıcıda
Dil öğrenmek için sıfır kurulum
Nightly sürüm de mevcut
```

### B.5.4 Docker Desteği

```bash
# Docker ile hızlı kurulum
docker pull veryllang/veryl
docker run veryllang/veryl veryl --version
```

---

## B.6 — Veryl'in Açık Reddettiği Özellikler

<cite index="106">Veryl, dil semantiği açısından SystemVerilog ile eşdeğerliğe odaklanıyor. Bu nedenle FF üreten bazı özellikler benimsenmedi çünkü bunlar üretilen SV kodunun öngörülebilirliğini engelliyor.</cite>

```
Veryl BILINÇLI şunları YAPMAZ:

1. Otomatik pipeline → SV tahmin edilemez olur
   → Spade/Arch/Volt'tan fark

2. CDC tip kontrolü → SV semantiği dışı
   → Volt'un fırsatı

3. Formal doğrulama → kapsam dışı

4. Fonksiyonel programlama → SV paradigması dışı

5. Şiddet değişken uzunlukta vektörler (dinamik) → sentez dışı

Bu kararlar Veryl'i "SV transpiler" kategorisine koyuyor.
Anlambilimsel yenilik değil, sözdizimsel iyileştirme.
```

---

## B.7 — Yayın Geçmişi (Hızlı Büyüme)

```
Ağustos 2024: DVCon Japan 2024 → ilk büyük yayın
Kasım 2024:   arXiv:2411.12983 (uluslararası görünürlük)
Haziran 2025: VLSI Technology and Circuits Workshop
Haziran 2025: OSCAR @ ISCA 2025 (çok prestijli!)
Ekim 2025:    Design Solution Forum 2025
Mayıs 2026:   v0.20.0, 938 GitHub yıldızı
```

---

## B.8 — RgGen Entegrasyonu

<cite index="99">RgGen, Veryl dilinde yazılmış CSR modülleri üretebilen açık kaynak bir CSR otomasyon aracı. Okunabilir register map spesifikasyonlarından CSR üretimini otomatikleştiriyor.</cite>

```
RgGen → Veryl bağlantısı:
  Register map YAML/TOML → Veryl CSR modülü → SV

Bu HW-SW köprüsünün bir versiyonu:
  Volt'un sinerjik özellik listesinde
  "Donanım-Yazılım Köprüsü" bu yaklaşım Veryl'de mevcut
```

---

## B.9 — Veryl'in Güçlü ve Zayıf Yönleri

```
GÜÇLÜ:
  ✓ 938 GitHub yıldızı → gerçek topluluk ilgisi
  ✓ Çok aktif (v0.20, 4456 commit, Mayıs 2026)
  ✓ Mükemmel SV birlikte çalışabilirliği
  ✓ ASIC timing ECO uyumlu (FF seviyesi düzenlenebilir)
  ✓ Kapsamlı araç seti (LSP, fmt, test, doc, publish)
  ✓ Playground mevcut
  ✓ Çift lisans (Apache+MIT) → kurumsal güven
  ✓ Prestijli yayınlar (ISCA 2025)
  ✓ Japonca belgeleme → Japon yarı iletken endüstrisi hedef

ZAYIF:
  ✗ SV semantiği korunuyor → gerçek yenilik yok
  ✗ CDC kontrol yok
  ✗ Formal doğrulama yok
  ✗ Tip güvenliği sınırlı (SV tip sistemi)
  ✗ Pipeline güvenliği yok (bilinçli karar ama eksiklik)
  ✗ Ternary/nöromorfik/fotonik yok
  ✗ "Yeni dil" değil → "SV'nin güzel versiyonu"
```

---

# BÖLÜM C — CLASH ve VERYL KARŞILAŞTIRMA TABLOLARI

## C.1 — Clash vs Veryl vs Volt Özellik Matrisi

```
ÖZELLİK               Clash    Veryl    Volt
──────────────────────────────────────────────────────────────
CDC tip sisteminde      ✓✓       ✗        ✓(plan)
Bağımsız dil            ✗(H)    ✓        ✓
SV birlikte çalışma     ✓       ✓✓       ✓(SV çıktı)
Tip güvenliği           ✓✓✓     ✓        ✓✓
Pipeline güvenliği      ✓✓      ✗        ✓(L0-L2)
Auto-pipelining         ✓✓      ✗        ✓(L1/L2)
Formal doğrulama        ✓(SAT)  ✗        ✓(SymbiYosys)
REPL                    ✓✓✓    ✗        ✗(plan)
Paket yöneticisi        ✗       ✓✓       ✓(plan)
LSP                     ✗       ✓✓       ✓(plan)
Playground              ✗       ✓✓       ✓(plan)
Aktif topluluk          ✓       ✓✓       ✗(plan)
Ticari destek           ✓✓      ✗        ✓(plan)
ASIC timing ECO         ✗       ✓✓       ✗(CIRCT)
Sentez çıktı kalitesi   ✓       ✓✓✓     ✓✓(plan)
Ternary/Trit            ✗       ✗        ✓
Nöromorfik              ✗       ✗        ✓(v2)
Fotonik                 ✗       ✗        ✓(v3)
Çalışan uygulama        ✓✓✓    ✓✓       ✗(plan)
Öğrenme kolaylığı       ✗✗✗    ✓✓       ✓✓(plan)
──────────────────────────────────────────────────────────────
H = Haskell bağımlı
```

## C.2 — Her Projenin Hedef Kitlesi

```
Clash:
  → Haskell bilen akademisyen ve araştırmacı
  → "Doğruluk her şeyden önce" yaklaşımı
  → Google gibi büyük teknoloji firmaları (Bittide)
  → Kriptografi, güvenlik-kritik tasarım

Veryl:
  → Mevcut SV/Verilog bilgisi olan FPGA/ASIC mühendisi
  → "Geçiş acısız olsun" isteyen ekipler
  → Japonya'daki yarı iletken endüstrisi
  → Büyük mevcut SV kod tabanı olan şirketler
  → Açık kaynak donanım topluluğu

Volt:
  → CDC güvenliği öncelik olan yeni başlayanlar ve startup
  → AI hızlandırıcı tasarımcısı
  → Ternary/nöromorfik/fotonik araştırmacı
  → "Temiz sayfa" yapan ekipler
```

## C.3 — Volt İçin Stratejik Sonuçlar

```
Clash'ten:
  "CDC güvenliği var ama Haskell öğrenmek gerekiyor"
  → Volt mesajı: "Clash'in CDC güvenliği, Haskell olmadan"
  → Clash kullanıcılarını çekme fırsatı

Veryl'den:
  "SV mühendisleri için kolay geçiş ama yeni semantik yok"
  → Volt mesajı: "Mevcut SV ile uyumlu çıktı + gerçek yenilik"
  → Veryl kullanıcıları ileride Volt'a geçebilir

İkisinden birlikte:
  Clash: derin ama Haskell engeli
  Veryl: geniş ama sığ (SV semantiği)
  Volt: derin + erişilebilir + gelecek paradigmaları
```

---

# ÖZET

```
CLASH:
  ✓ En olgun ve kanıtlanmış CDC çözümü
  ✓ Google üretimde (Bittide)
  ✓ 16 yıllık sürdürülebilirlik
  ✗ Haskell engeli → geniş benimseme imkânsız
  Volt mesajı: "Clash'i Haskell olmadan"

VERYL:
  ✓ En aktif GitHub topluluğu (938 ★)
  ✓ En iyi araç ekosistemi (bugün!)
  ✓ ASIC timing ECO uyumlu
  ✓ SV ile mükemmel uyum
  ✗ Gerçek semantik yenilik yok (SV transpiler)
  ✗ CDC, formal, pipeline güvenliği yok
  Volt mesajı: "Veryl'in araç olgunluğu + gerçek semantik"

VOLT:
  Clash + Veryl + Arch + Spade'in en iyilerini alıyor:
  → CDC (Clash'ten fikir)
  → Araç ekosistemi (Veryl'den model)
  → AI-native (Arch'tan fikir)
  → Pipeline güvenliği (Spade'den fikir)
  + Ternary/Nöromorfik/Fotonik (hiçbirinde yok)

  Bugünkü zayıflık: HİÇBİRİ ÇALIŞMIYOR HENÜZ
  → F0 öncelik #1
```
