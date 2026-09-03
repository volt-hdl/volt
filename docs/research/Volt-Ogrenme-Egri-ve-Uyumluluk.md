> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Volt Zayıf Yönleri — Basit Açıklamalar ve Çözüm Stratejileri

---

## Bölüm 1 — Altı Kavramın Basit Açıklaması

### 1.1 Verilog/SV Sözdizimi Farklılığı → Alışma Maliyeti

**Ne demek:**

```
Verilog'da yıllarca yazan birinin alışkanlığı var:

  always @(posedge clk) begin
      if (reset) count <= 0;
      else count <= count + 1;
  end

Volt'ta aynı şey:

  on Sys {
      count <= count + 1
  }

Teknik olarak daha temiz — ama beyin "always @" yazmak istiyor.
```

Bu bir zorluk değil, **kas hafızası** sorunudur. Verilog mühendisi doğru kodu biliyor ama yanlış yazmaya eğilimli çünkü 10 yıllık alışkanlık var. Tıpkı solak birinin sağ elle yazmayı öğrenmesi gibi — imkânsız değil ama alışma süreci var.

**Pratikte ne kadar sürer:**

```
Verilog deneyimi     Volt'a alışma süresi
──────────────────────────────────────────
< 1 yıl              1-2 hafta (alışkanlık az)
1-5 yıl              3-4 hafta
5-10 yıl             1-2 ay (derin refleks var)
10+ yıl ASIC         2-3 ay (ve direnç!)
```

---

### 1.2 L2 Timeline Tipleri Karmaşık → Bazı Kullanıcılar

**Ne demek:**

L2, bir sinyalin tam olarak **hangi saat döngüsünde geçerli** olduğunu tip sisteminde ifade etmenin yolu. Şöyle görünüyor:

```volt
#[timeline]
module Multiplier {
    in  a : u8  @['G+[0,1])    // G döngüsünden başlayıp G+1'e kadar geçerli
    in  b : u8  @['G+[0,1])
    out p : u16 @['G+[1,2))   // bir döngü sonra çıkış hazır
}
```

`@['G+[0,1])` ifadesi matematiksel olarak mükemmel ama sezgisel değil.

**Kimin için sorun:**

```
Etkilenmeyenler:
  → Öğrenci (L2 öğrenmek zorunda değil, L0/L1 yeterli)
  → FPGA hobicisi (pipeline hizalama genelde yeterli)
  → Çoğu RTL mühendisi (L1 Delayed<T,N> yeterli)

Etkilenenler:
  → ASIC timing analisti (L2'yi kullanmak istiyor ama öğrenmesi zaman alır)
  → Yüksek verimli boru hattı (HPC, AI hızlandırıcı) tasarımcısı

İyi haber:
  L2 tamamen opsiyonel — #[timeline] yazmadan hiç karşılaşmazsın.
  90% kullanıcı asla ihtiyaç duymaz.
```

---

### 1.3 Orta Soyutlama, Bağımsız, Güçlü Tip

**Soyutlama seviyeleri şöyle sıralanır:**

```
ÇOK DÜŞÜK (kapı seviyesi):
  AND, OR, NOT — transistör kadar yakın
  Kim kullanır: dijital tasarım öğrencisi (öğrenmek için)

DÜŞÜK (Verilog/VHDL — register seviyesi):
  reg, wire, always, clock — donanım görünür
  Kim kullanır: mevcut endüstri standardı

ORTA (Volt — RTL soyutlaması):
  reg(Sys), on D, fsm, pipeline — donanım görünür ama temiz
  Kim kullanır: Volt hedef kitlesi

YÜKSEK (Chisel/SpinalHDL — meta-donanım):
  Scala'da donanım üretici yazıyorsunuz
  Parametrik, jeneratör-tabanlı, güçlü
  Kim kullanır: platform/kütüphane geliştiricisi

ÇOK YÜKSEK (HLS — davranışsal):
  C/C++ yazıyorsunuz, araç donanım üretiyor
  Kim kullanır: algoritma geliştiricisi, DSP
```

**Volt "orta" — ne anlama geliyor:**

```
Volt'ta hâlâ:
  → Register düşünürsünüz (reg(Sys))
  → Saat kenarı düşünürsünüz (on Sys)
  → Pipeline aşaması düşünürsünüz (stage)

Ama Volt'ta ARTIK:
  → Sensitivity list yok (implicit)
  → blocking/non-blocking karışıklığı yok
  → CDC el ile kontrol yok (tip sistemi yapıyor)
  → Latch endişesi yok (derleme zamanı yakalanır)

Yani: donanım düşünce modeli korunuyor,
      ama kötü sürprizler ortadan kalkıyor.
```

**"Bağımsız"** → Volt çalışmak için Scala, Haskell, JVM, Python yüklemenizi istemiyor. Tek bir `volt` ikili dosyası.

**"Güçlü tip"** → Genişlik, işaret, saat alanı, birim hepsi tip sisteminde. Yanlış bağlantı derleme hatası, sessiz değil.

---

### 1.4 MLIR Öğrenme Eğrisi Ekip İçin Dik

**MLIR nedir:**

Google'ın geliştirdiği derleyici altyapısı. Volt'un CIRCT/MLIR'i "arka uç" olarak kullandığını konuştuk. Bu altyapıyı kullanmak için Volt ekibinin öğrenmesi gerekenler:

```
Normal Rust geliştiricisi bilmez:
  → "Dialect" (lehçe): MLIR'deki işlem kümeleri
  → "Pass": IR üzerinde dönüşüm
  → "TableGen": operasyon tanımlama DSL'i
  → "Pattern rewriting": IR eşleme ve değiştirme
  → C++ ile Rust arasında FFI güvenliği

Öğrenme süresi:
  Hevesli, deneyimli Rust geliştirici: 2-3 ay
  Ortalama backend geliştirici: 4-6 ay
  Yeni mezun: 6-12 ay

Neden önemli:
  Hatalı CIRCT entegrasyonu = üretilen SV yanlış
  = müşterinin tasarımı sentezde başarısız
  = güven kaybı
```

**Pratik risk:**

Ekipte hiç MLIR/CIRCT deneyimi yoksa, `volt-lower` crate'ini yazmak projede en uzun ve en riskli adım. Bu yüzden F3 (CIRCT lowering) fazı için 1-2 aylık öğrenme tamponu planlandı.

---

### 1.5 Büyük Tasarımda Verilator/VCS'den Yavaş

**Simülatör hız gerçekliği:**

```
Simülatörler nasıl çalışır:
  Volt tasarımı → simülatör kodu → çalıştır

Verilator (açık kaynak hız şampiyonu):
  20+ yıllık optimizasyon
  RTL → C++ → çok iş parçacığı
  Büyük çip: milyarlarca saat döngüsü/saniye

Volt yerleşik simülatör (yeni):
  IR → native kod → çalıştır
  Hedef: Verilator hızında
  Gerçekçi beklenti: %50-80 hızda başlar

Pratik fark:
  Küçük tasarım (UART, sayaç, FSM):
    Volt: 0.1 saniye — fark edilmez
    Verilator: 0.08 saniye — fark edilmez

  Büyük tasarım (işlemci çekirdeği, önbellek):
    Volt: 10 dakika
    Verilator: 6 dakika
    VCS: 4 dakika
    Fark burada hissedilir
```

**Neden önemli ama çözülebilir:**

Volt yerleşik simülatör büyük tasarım için yavaş kalabilir. Çözüm: Volt `--simulator=verilator` seçeneği ile Verilator'a yönlendirme (zaten planlandı). Kullanıcı farkı hissetmez.

---

### 1.6 Büyük Durum Uzayı → Bounded (Sınırlı) Kalan

**Model kontrol nedir, neden sınırlı:**

```
Formal doğrulama sorusu:
  "Bu tasarım HİÇBİR ZAMAN hata yapar mı?"

Cevap vermek için:
  Tasarımın alabileceği TÜM durumları kontrol et

Küçük tasarım (4-bit sayaç):
  16 olası durum → kolay kontrol → KESİN cevap ✓

Büyük tasarım (önbellek tutarlılık protokolü):
  Durum sayısı = milyarlarca × milyarlarca
  → Tamamını kontrol etmek yıllar alır

"Bounded" çözüm:
  "İlk N adımda hata yok mu?" → EVET kontrol edilir
  "N+1. adımda ne olur?" → bilinmiyor

Örnek:
  "Bu sayaç 10 döngüde taşmaz" → kanıtlandı ✓
  "Bu sayaç hiçbir zaman taşmaz" → sınırlı değil → kanıtlanamaz
```

**Basit benzetme:**

Bounded formal = "Bu yolda ilk 10 km'de kaza olmuyor" kanıtı.
Tam formal = "Bu yolda hiçbir zaman kaza olmaz" kanıtı (çok daha zor).

Tam formal için Coq, Lean gibi teorem ispatlayıcılar gerekir — bunları kullanmak doktora seviyesi bilgi ister.

---

## Bölüm 2 — Öğrenme Eğrisi Düşürülebilir mi?

**Kısa cevap: Evet — ama sihirli değnek yok, somut araçlar gerekiyor.**

### 2.1 Verilog → Volt Geçişi İçin Somut Araçlar

**Araç 1 — Kısmi Otomatik Çevirici:**

```
volt migrate counter.v → counter.volt

Çevirici ne yapar:
  → always @(posedge clk) → on Sys { }
  → reg[7:0] → reg(Sys) x : u8 = 0
  → wire → wire veya direkt bağlantı
  → Basit assign → kombinasyonel bağlantı

Çevirici ne YAPAMAZ:
  → Saat alanı tipini otomatik çıkaramaz (bilmek gerekir)
  → CDC geçişlerini tanıyamaz (sen açıklamalısın)
  → Karmaşık generate yapıları

Gerçekçi beklenti:
  %60-70 otomatik, %30-40 elle tamamlama
  → Sıfırdan yazmaktan çok daha hızlı
  → Alışma sürecini 1 aydan 2 haftaya indirir
```

**Araç 2 — "Paralel Öğretici" Hata Mesajları:**

```
Kullanıcı Verilog alışkanlığıyla yazıyor:
  always @(posedge clk) begin...

Volt derleyicisi:
  Sözdizimi hatası: 'always' Volt'ta geçersiz.
  
  Demek istediğiniz muhtemelen:
  ┌─────────────────────────────────────────┐
  │  on Sys {                               │
  │      // buraya yazdığınız kodu koyun    │
  │  }                                      │
  └─────────────────────────────────────────┘
  
  Volt'ta saat alanı 'Sys' olarak tanımlanmalı:
  domain Sys { clock = posedge, reset = sync active_high }
  
  [Belgelere bak: volt-lang.org/verilog-to-volt]
```

Bu "eğitici hata mesajı" yaklaşımı öğrenme eğrisini yarıya indirir — hata yaptığın anda doğru yolu öğretir.

**Araç 3 — Kademeli Öğrenme Yolu:**

```
Seviye 1 (Hafta 1-2): Temel RTL
  → Sayaç, shift register, basit FSM
  → L0 güvenlik: pipeline hizalama otomatik
  → CDC: henüz yok, tek alan

Seviye 2 (Hafta 3-4): İletişim ve Bellek
  → UART, SPI, I2C
  → Stream/Flow primitifleri
  → AXI-Lite bağlantısı

Seviye 3 (Ay 2): Çok Alan Tasarım
  → İki saat alanı, CDC köprüsü
  → CDC tip sistemi anlaşılıyor
  → L1 Delayed<T,N> tanıtımı

Seviye 4 (Ay 3+): İleri Özellikler
  → Parametrik tasarım, const-generics
  → Formal doğrulama (assert/invariant)
  → L2 timeline (opsiyonel, isteyen için)

Kural: Her seviyede önceki seviyenin bilgisi yeterli.
       Kimse zorla L2'ye itilmez.
```

**Araç 4 — "Tanıdık" Sözdizimi Seçenekleri:**

Tartışmaya değer tasarım kararı: Verilog kullanıcıları için **sözdizimi uyum modu** (`--compat-verilog`):

```volt
// Normal Volt
on Sys {
    count <= count + 1
}

// Uyumluluk modu (tartışmalı, ama geçiş kolaylaştırır)
always_ff @(posedge clk) {   // Volt yorumluyor, always_ff kabul
    count <= count + 1        // ama latch kontrolü Volt yapıyor
}
```

Bu tartışmalı — "yanlış alışkanlıkları korur mu?" riski var. Ama geçiş kolaylaşır.

### 2.2 L2 Öğrenme Eğrisi — Neredeyse Problem Değil

L2 zaten opsiyonel. Öğrenme eğrisini düşürmek için ek bir şey gerekmez — sadece dokümantasyonda net yazmalı:

```
"L2 timeline tipleri çoğu tasarım için gerekmez.
 Kullanmak zorunda değilsiniz. Ancak:
 - Yüksek verimli boru hatları tasarlıyorsanız
 - Kaynak çakışmasını derleme zamanında yakalamak istiyorsanız
 L2'ye bakın: [bağlantı]"
```

---

## Bölüm 3 — Orta Soyutlama Yeterli mi? Güçlü Soyutlama Faydalı Olur mu?

### 3.1 Chisel'in Güçlü Soyutlaması Ne Sağlıyor

Chisel (Scala üzerine kurulu), Volt'tan daha yüksek soyutlama sunar:

```scala
// Chisel: tip seviyesinde vektör hesaplama
class Adder[T <: Data : Num](gen: T, n: Int) extends Module {
    val io = IO(new Bundle {
        val in = Input(Vec(n, gen))
        val out = Output(gen)
    })
    io.out := io.in.reduce(_ + _)
}
// Hem UInt hem SInt hem FixedPoint için çalışır
// Tip parametresi değişince otomatik uyarlanır
```

Bu güçlü soyutlama şunları mümkün kılar:
- Bir kod → yüzlerce farklı konfigürasyon
- Kütüphane yazıcıları çok güçlü araçlara sahip
- Rocket chip gibi büyük projeler bu olmadan zor

### 3.2 Güçlü Soyutlamanın Bedeli

```
Chisel'in güçlü soyutlaması ne kaybettirir:

1. Üretilen donanım öngörülemez:
   "Bu Scala kodu ne üretiyor?" — bazen bilinmiyor
   Volt: şeffaf lowering → her yapının ne ürettiği belgelenmiş

2. Hata mesajları kriptik:
   Scala tip hatası + donanım bağlamı = anlaşılmaz
   Volt: donanım bağlamında, Rust/Elm kalitesinde mesaj

3. Performans tuzakları gizlenir:
   Yüksek soyutlama bazen istenmeyen donanım üretir
   Farkında olmadan büyük alan/güç harcıyor olabilirsin

4. Ev sahibi dil yükü:
   Chisel için Scala öğrenmek şart → 3-6 ay
   Volt: kendi dili → Scala gerektirmez
```

### 3.3 Volt İçin Doğru Karar: "Stratejik Orta"

```
Volt'un soyutlaması neden kasıtlı olarak "orta":

1. Donanım düşünce modeli korunuyor
   → Kullanıcı ne üretildiğini biliyor
   → "HLS kara kutusu" problemi yok

2. Python API yüksek soyutlamayı dışarıdan sağlıyor
   → Parametrik jeneratörler Python'da
   → Volt tipi güvenli tutuluyor, Python üretiyor

3. Const-generics + tip parametreleri zaten güçlü
   → Volt'ta parametrik tasarım mümkün
   → Ama Scala'nın tüm gücü yok (kasıtlı)

En doğru çerçeve:
  Volt DILI = orta soyutlama (kasıtlı, değiştirme)
  Volt EKOSİSTEMİ = yüksek soyutlama (Python API, NeuroCompiler)

Bu ikisinin ayrılması kritik:
  Dil: "donanım ne yapacak" → net, öngörülür
  Araç: "bu donanımı nasıl üretiriz" → güçlü soyutlama OK
```

### 3.4 Güçlendirilebilecek Soyutlamalar (Dili Değiştirmeden)

Volt dilini değiştirmeden, kütüphane katmanında yüksek soyutlama mümkün:

```volt
// Bugün (elle yazılmış):
module Adder {
    in  a, b : u8
    out sum  : u9
    sum = a + b
}

// Gelecekte stdlib'de:
// GenericAdder<T: Numeric> → tek satır
let adder = GenericAdder<u8>()

// Veya Python API ile üret:
# Python (NeuroLang):
adder_8bit = generate_adder(width=8, signed=False)
adder_16bit = generate_adder(width=16, signed=True)
# Volt kodu otomatik üretilir, tip-güvenli
```

---

## Bölüm 4 — Sektör Araçlarıyla Uyumluluk

**Temel strateji:** Volt resmi destek olmadan çalışır çünkü **çıktı** uyumludur, kendisi değil.

### 4.1 "Şeffaf Çıktı" Prensibi

```
Kullanıcı perspektifi:

Vivado (Xilinx) ne bilmek ister?
  → SystemVerilog dosyası
  → Volt'u bilmek ZORUNDA DEĞİL

Volt ne üretir?
  → Temiz, okunabilir, standart SystemVerilog-2017

Sonuç:
  Vivado → SystemVerilog → Volt'un ürettiği
  Vivado "Volt ile tasarlandı" bilmeden çalışıyor
  → UYUMLULUK OTOMATİK ✓
```

### 4.2 Araç Araç Uyumluluk Tablosu

```
ARAÇ                   UYUMLULUK        NASIL ÇALIŞIR
────────────────────────────────────────────────────────────────────
Vivado (Xilinx/AMD)    ✓ Tam            SV çıktısı → doğrudan add
Quartus (Intel/Altera) ✓ Tam            SV çıktısı → doğrudan add
Yosys (açık kaynak)    ✓ Tam            SV → synthesis
OpenROAD               ✓ Tam            SV → P&R
Design Compiler        ✓ Tam            SV → synthesis
Genus (Cadence)        ✓ Tam            SV → synthesis
Innovus                ✓ Tam            Netlist → P&R
Calibre (Mentor)       ✓ Tam            GDSII → DRC/LVS
PrimeTime (Synopsys)   ✓ Tam            Netlist → timing
VCS (Synopsys)         ✓ Tam            SV testbench (Volt tests → SV)
Xcelium (Cadence)      ✓ Tam            SV testbench
Questa (Siemens)       ✓ Tam            SV testbench
GTKWave                ✓ Tam            VCD/FST → dalga formu
SymbiYosys             ✓ Tam            SVA assertions
JasperGold             ✓ Tam            SVA assertions (büyük formal)
Verilator              ✓ Tam            SV → hızlı simülasyon
SpyGlass               ✓ Tam            SV → lint
────────────────────────────────────────────────────────────────────

Özel konfigürasyon gerekenler:
  ─ Vendor-özel IP: extern module ile sarılır (Volt tarafı)
  ─ UCF/XDC kısıt dosyaları: Volt signal isimleri korunur (şeffaf)
  ─ Timing constraint: SV sinyal adları Volt'ta korunur
```

### 4.3 Sinyal Adı Tutarlılığı — Kritik Detay

Bir tasarım araçlara girdiğinde sinyal adları önemli. Voltun ürettiği SV'de:

```volt
// Volt kaynağı:
module Counter {
    in  enable : bool
    out count  : u8
    reg(Sys) value : u8 = 0
    ...
}
```

```systemverilog
// Üretilen SV (sinyal isimleri KORUNUYOR):
module Counter (
    input  logic        enable,  // ← aynı isim
    output logic [7:0]  count    // ← aynı isim
);
    logic [7:0] value;           // ← aynı isim
    ...
endmodule
```

Vivado constraints dosyasında:
```tcl
set_input_delay -clock clk 2.0 [get_ports enable]  # Volt'taki isim
set_output_delay -clock clk 1.0 [get_ports count]  # Volt'taki isim
```

Sinyal isimleri değişmezse constraints dosyası Volt ile yazılmış tasarıma da çalışır. Bu kasıtlı tasarım kararı.

### 4.4 Resmi Destek Olmadan UVM ile Uyum

UVM (Universal Verification Methodology) büyük ASIC ekiplerinin standardı. Volt doğrudan UVM yazmıyor ama köprü mümkün:

```
Volt assert/test → SV'ye çevrilir → UVM testbench ile karıştırılır

// Volt testbench:
test "veri doğru geçiyor" {
    let dut = MyModule()
    tick(Sys, 10)
    expect dut.output == 42
}
// → SV'ye çevrilir → UVM driver/monitor ile aynı DUT'u test eder
// → UVM framework Volt çıktısını bilmeden kullanır
```

Orta vade (v1): cocotb köprüsü Python tabanlı UVM-benzeri test yazmayı sağlar. Bu büyük ASIC ekiplerinin Volt'u benimsemesinde kritik.

### 4.5 "Volt-Farkında" Araç Entegrasyonu (Kademeli)

Resmi destek olmadan çalışır, ama daha iyi olabilir:

```
Seviye 0 (bugün): SV çıktısı → her araç çalışır (MEVCUT)

Seviye 1 (v1, topluluk): Vivado eklentisi
  → Volt kaynağını doğrudan projeye ekle
  → Vivado arka planda "volt build" çağırır
  → Resmi Xilinx desteği GEREKMEZ (script yeterli)

Seviye 2 (v2, ortaklık):
  → Xilinx/Intel resmi Volt plugin'i (isteğe bağlı)
  → Cadence resmi cocotb entegrasyonu

Seviye 3 (uzun vade):
  → Volt dahili P&R optimizasyon ipuçları (@bram, @dsp)
  → Araç-farkında sentez direktifleri
```

---

## Bölüm 5 — En Önemli Sonuçlar

```
Öğrenme eğrisi azaltmak için:
  1. Kısmi otomatik Verilog → Volt çevirici (öncelik ver)
  2. Eğitici hata mesajları ("Demek istediğiniz: ...")
  3. Kademeli öğrenme yolu (7 seviye, her biri bağımsız)
  4. L2 tamamen opsiyonel ve dokümantasyonda net
  5. Playground: "30 saniyede ilk tasarım" deneyimi

Orta soyutlama yeterli mi:
  Dil seviyesinde: EVET, değiştirme (şeffaflık önemli)
  Ekosistem seviyesinde: HAYIR tek başına (Python API ekle)
  Kütüphane seviyesinde: Güçlendirilebilir (GenericAdder tarzı)

Sektör araçları uyumluluğu:
  Resmi destek beklemeden çalışır — SV çıktısı evrensel
  Sinyal ismi tutarlılığı kasıtlı korunmalı
  Vivado/Quartus script entegrasyonu Volt ekibi yazabilir
  (resmi Xilinx/Intel onayı gerekmez)

Büyük tasarım simülasyonu yavaş:
  Çözüm: --simulator=verilator bayrağı
  Verilator köprüsü gün-1'den itibaren mevcut olmalı
  (yerleşik simülatör küçük/orta için, Verilator büyük için)

Bounded formal:
  %80 kullanıcı için yeterli
  Geri %20 için: JasperGold SVA entegrasyonu (Volt SV üretir, JG okur)
  Tam formal (Coq/Lean): uzak vadede K-framework ile köprü
```
