# RDC + Doğrulama Verimliliği + Spesifikasyon Uçurumu
## Kapsamlı ve Bütünleşik Çözüm

> Bu üç sorun bağımsız görünüyor ama aynı kök nedenden kaynaklanıyor.
> Kök nedeni çözen tek bir mimari hamle üçünü aynı anda çözebilir.

---

## Bölüm 1 — Kök Nedenin Tespiti

### Üç Sorunun Ortak Kökü

```
RDC:
  Reset semantiği mühendisinin kafasında.
  Araç bilmiyor → yakalayamıyor.

Doğrulama verimliliği:
  "Bu tasarım doğru mu?" sorusu için
  "Doğru ne demek?" yanıtı mühendisinin kafasında.
  Araç bilmiyor → otomatize edemiyor.

Spesifikasyon uçurumu:
  Spec Word belgesi → mühendis yorumluyor.
  Araç okuyamıyor → RTL'den kopuyor.

ORTAK KÖK:
  "Örtük bilgi mühendiste, araçta değil."

Çözüm prensip olarak basit:
  Örtük olanı açık yap — ama aynı artifact içinde.
  Ayrı belge değil, kodun kendisi.
```

### Mevcut Endüstri Yaklaşımı Neden Başarısız

```
Bugün:

  Spec.docx ──────────────────────────────────┐
                                               ↓
  RTL.v ──────────────────────── Mühendis yorumluyor
                                               ↓
  Testbench.sv ──────────────── başka mühendis yorumluyor
                                               ↓
  Formal.tcl ─────────────────── başka mühendis yazıyor

Dört ayrı artifact → dört ayrı bilgi kaynağı
Her kopya → kayma → hata → re-spin

Yeni yaklaşım hedefi:

  design.volt ─────────────────────────────────
  (tek kaynak)                                 ↓
                    Spec         otomatik üretilir
                    RTL          derleyici yazar
                    Testbench    araç üretir
                    Formal       araç üretir
                    SW Driver    araç üretir
                    Belgeler     araç üretir
                    RDC kuralları tip sistemi uygular
```

---

## Bölüm 2 — Bütünleşik Mimari: "Yaşayan Spesifikasyon"

### Temel Fikir: Hoare Mantığının Donanıma Uyarlanması

```
1969'dan beri bilinen yazılım prensip:

  {P} C {Q}
  
  P: Ön koşul (ne geçerli olmalı öncesinde)
  C: Komut (implementasyon)
  Q: Son koşul (ne geçerli olmalı sonrasında)

Donanıma uyarlama:

module Divider {
    requires: divisor != 0              // P: ön koşul
    in  dividend, divisor : u32
    out quotient, remainder : u32       // C: implementasyon
    ensures: quotient * divisor + remainder == dividend  // Q: son koşul
    ensures: remainder < divisor
}

"Bu modülü kullanan herkes ön koşulun sağlandığından emin olmalı."
"Bu modül son koşulları garanti ediyor."
→ Kontrat programlama donanıma taşındı.
```

### Üç Katmanlı Çözüm

```
KATMAN 1 — TİP SİSTEMİ (statik, derleme zamanı)
  @Domain: saat + sıfırlama semantiği
  Linear tipler: kaynak kullanımı
  Timeline tipler: zamanlama güvenliği

KATMAN 2 — KONTRAT SİSTEMİ (dinamik-statik karma)
  requires: ön koşul
  ensures:  son koşul
  invariant: her zaman doğru olmalı
  cover:    bu duruma ulaşılabilmeli (doğrulama hedefi)

KATMAN 3 — ÜRETİM KATMANI (otomatik türetim)
  Tip sistemi + Kontratlar → her şey üretilir
```

---

## Bölüm 3 — RDC Çözümü: Domain Semantiğinin Tamamlanması

### 3.1 Mevcut CDC Yaklaşımını Genişletme

```
Volt şu an:
  domain Sys { clock = posedge }
  @Sys anotasyonu CDC için kullanılıyor

Eksik:
  Reset semantiği yok
  Reset polaritesi yok
  Reset senkronluğu yok
  Reset sırası yok

Tamamlanmış domain tanımı:
```

```volt
// Genişletilmiş domain tanımı
domain SysDomain {
    // Saat özellikleri
    clock     = posedge
    frequency = 100.mhz

    // Sıfırlama özellikleri (YENİ)
    reset_sync        = sync         // sync | async
    reset_polarity    = active_high  // active_high | active_low
    reset_cycles      = 4            // kaç döngü sıfırlamada kalmalı
    reset_sequence_id = 1            // sıfırlama sırası (1 = önce)
}

domain UsbDomain {
    clock             = posedge
    frequency         = 48.mhz

    reset_sync        = async
    reset_polarity    = active_low
    reset_cycles      = 0            // asenkron: hemen
    reset_sequence_id = 2            // Sys'ten sonra
}

domain AnalogDomain {
    clock             = none         // saatten bağımsız
    reset_sync        = async
    reset_polarity    = active_high
    reset_sequence_id = 0            // en önce (power domain)
    reset_requires    = [PLL_locked] // koşullu sıfırlama
}
```

### 3.2 RDC Hata Tespiti

```volt
module CrossDomainReg {
    in  sys_signal  : u8 @SysDomain
    out usb_signal  : u8 @UsbDomain

    reg(UsbDomain) buffer : u8 = 0

    on UsbDomain {
        // E3001: CDC ihlali (saat farklı) → Volt zaten yakalıyor
        // E3003: RDC ihlali (reset semantiği uyumsuz) → YENİ
        buffer <= sys_signal   // hata: hem CDC hem RDC ihlali
    }
}

// Hata mesajı:
// error[E3003]: Sıfırlama alanı uyumsuzluğu
//  --> design.volt:9:19
//   |
// 9 |     buffer <= sys_signal
//   |                ^^^^^^^^^^
//   | SysDomain: sync, active_high, sequence=1
//   | UsbDomain: async, active_low, sequence=2
//   |
// help: ResetSync<SysDomain, UsbDomain> kullanın
// note: Sıfırlama sırası da farklı — sekans bağımlılığı riski
```

### 3.3 Stdlib RDC Primitifleri

```volt
// Sıfırlama senkronizatörü
module ResetSync<SrcDomain, DstDomain> {
    in  rst_src : Reset @SrcDomain
    out rst_dst : Reset @DstDomain

    // İki flip-flop senkronizasyon (reset için)
    reg(DstDomain) stage1 : bool = true   // reset'te 1 (active high)
    reg(DstDomain) stage2 : bool = true

    on DstDomain.reset_deassert {
        stage1 <= false
        stage2 <= stage1
    }

    rst_dst = stage2

    // Formal özellik: sıfırlama yayılımı garantisi
    assert: always (rst_src == active → stage2 == active)
    assert: eventually (rst_src == inactive → stage2 == inactive)
}

// Sıralı sıfırlama sekansörü
module ResetSequencer<domains: [Domain]> {
    // Reset sequence_id'ye göre sıralı sıfırlama
    // sequence_id = 0 önce, sonra 1, sonra 2...
    
    @auto_generate(from = domain_sequence_ids)
    // Araç domain tanımlarından sıralama üretiyor
}

// Koşullu sıfırlama köprüsü
module ConditionalReset<SrcDomain, DstDomain, Cond: Signal<bool>> {
    // reset_requires = [PLL_locked] gibi koşulları uygular
    in  rst_src  : Reset @SrcDomain
    in  pll_lock : bool
    out rst_dst  : Reset @DstDomain

    invariant: !pll_lock → rst_dst == active  // kilit yoksa sıfırda tut
}
```

### 3.4 RDC Analizi Otomasyonu

```volt
// Tasarım seviyesinde sıfırlama sekansı belgesi

module SoC_Top {
    // Volt araçları domain tanımlarından üretiyor:
    
    // volt analyze --rdc design.volt çıktısı:
    // ═══════════════════════════════════════════
    // SIFIRLAMA SEKANS ANALİZİ
    // ═══════════════════════════════════════════
    // 
    // Sıra 0: AnalogDomain (async, active_high)
    //   Koşul: PLL_locked gerekli değil
    //   Süre: anında
    //
    // Sıra 1: SysDomain (sync, active_high, 4 döngü)
    //   Koşul: AnalogDomain.rst == inactive
    //   Süre: 4 × 10ns = 40ns
    //
    // Sıra 2: UsbDomain (async, active_low)
    //   Koşul: SysDomain.rst == inactive
    //   Süre: anında
    //
    // RDC GEÇİŞLERİ:
    //   SysDomain → UsbDomain: 3 köprü bulundu
    //     - uart_tx_module: ResetSync ✓
    //     - usb_fifo: ResetSync ✓
    //     - config_reg: KÖPRÜ YOK → E3003
    //
    // ONAYLANMAYAN GEÇİŞLER: 1
    // ═══════════════════════════════════════════
}
```

---

## Bölüm 4 — Doğrulama Verimliliği: Kontrat Sistemi

### 4.1 Kontrat Sistemi Tasarımı

```volt
module AXI_Slave #(ADDR_WIDTH: u32 = 32) {
    in  bus : AXI_Lite::Slave

    // ═══ KONTRAT KATMANI ═══

    // Ön koşullar (bus kullanıcısı sağlamalı):
    requires: bus.awaddr[1:0] == 0b00   // 4-byte hizalama
    requires: bus.wstrb != 0b0000       // en az 1 byte yazılıyor

    // Son koşullar (bu modül garanti ediyor):
    ensures: bus.awready → (bus.awaddr < ADDR_WIDTH)
    ensures: bus.bvalid → bus.bresp in {0b00, 0b10}  // OKAY veya SLVERR

    // Her zaman doğru olmalı (invariant):
    invariant: !(bus.awvalid && bus.arvalid)  // eşzamanlı okuma/yazma yok
    invariant: bus.bvalid → prev(bus.bvalid || bus.bready)  // protokol

    // Ulaşılmalı koşullar (coverage spec):
    cover: bus.awvalid && !bus.awready  // geri basınç senaryosu
    cover: bus.bresp == 0b10           // slave error senaryosu
    cover: bus.awaddr == ADDR_WIDTH-4  // son adrese erişim

    // ═══ İMPLEMENTASYON KATMANI ═══
    // (RTL kodu buraya)
}
```

### 4.2 Kontratlardan Otomatik Üretim

```
Kontrat katmanı → araç dört şey üretiyor:

1. Testbench (cocotb):
   requires → stimulus koşulları
   ensures  → assertion kontrolleri
   cover    → coverage hedefleri (önce bu senaryolar)

   # Otomatik üretilen cocotb:
   @cocotb.test()
   async def test_backpressure_scenario(dut):  # cover hedefi
       # "geri basınç" senaryosu → otomatik stimulus
       dut.awvalid.value = 1
       dut.awready.value = 0  # geri basınç
       await RisingEdge(dut.clk)
       assert dut.awvalid.value == 1  # ensures kontrolü

2. Formal özellikler (SymbiYosys):
   invariant → assert property
   ensures   → assert property (gecikme ile)
   requires  → assume property

   # Otomatik SVA:
   property no_simultaneous_rw;
       @(posedge clk) not (awvalid && arvalid);
   endproperty
   assert property (no_simultaneous_rw);

3. İnsan-okunabilir spec:
   volt doc axi_slave.volt → axi_slave_spec.md
   Kontratlar → doğal dil açıklaması
   
4. Hız testi:
   requires → input constraint
   cover    → öncelikli test senaryosu
   → coverage hedeflerine yönlendirilmiş random test
```

### 4.3 Coverage'ı Tasarımdan Türetme

```volt
// Geleneksel yaklaşım:
// 1. Mühendis RTL yazar
// 2. Doğrulama mühendisi RTL'i okur
// 3. "Hangi durumlar var?" manuel analizi
// 4. Coverage grubu elle yazar
// 5. Simülasyon → hangi duruma girildi?

// Volt yaklaşımı:
// cover ifadesi → araç otomatik coverage grubu üretir

module UART_RX {
    // Coverage hedefleri RTL ile birlikte:
    cover: (state == IDLE && start_bit_detected)     // başlangıç
    cover: (data_count == 7 && stop_bit_valid)       // normal tamamlanma
    cover: (data_count == 7 && stop_bit_invalid)     // frame error
    cover: (state == RECEIVING && buffer_full)        // overrun error
    cover: (consecutive_idle_frames > 10)             // idle hatası

    // Volt araçları "hangi senaryo gerekli?" sorusuna hazır cevap:
    // "Buffer full + receiving: bu test durumu eksik, 3 senaryo daha gerekli"
}
```

### 4.4 Regresyon Analizi Otomasyonu

```
Geleneksel regresyon:
  Değişiklik yapıldı → tüm testbench çalıştır → saatler
  "Hangi test etkilendi?" elle analiz

Volt regresyon:
  volt analyze --impact-of uart_rx.volt

  Değişen: state makinesi geçiş koşulu (satır 47)
  Etkilenen kontratlar: 3 invariant, 2 ensures
  Kritik cover senaryolar: 2 (RECEIVING + buffer_full koşulu)
  
  Çalıştırılması gereken testler:
    test_buffer_overflow.py (önce)
    test_frame_error.py (önce)
    test_normal_reception.py (sonra)
  
  Atlanan testler (değişiklikten etkilenmiyor):
    test_baud_rate_detection.py ← atlayabilirsiniz
    test_parity_check.py       ← atlayabilirsiniz
  
  Tahmini simülasyon süresi: 12 dakika (tümü: 4 saat)
  Tasarruf: %95
```

---

## Bölüm 5 — Spesifikasyon Uçurumu: Tek Kaynak Mimarisi

### 5.1 "Yaşayan Spesifikasyon" Kavramı

```
Synopsys'in 30 yıldır aradığı "altın spec":
  "Donanım, yazılım ve doğrulama ekiplerinin
   aynı anda referans alabileceği tek kaynak"

Volt'un önerisi:
  design.volt = altın spec

Proje yöneticisi okur → ne yaptığını anlar (kontratlar)
Donanım mühendisi okur → nasıl yapıldığını anlar (RTL)
Yazılım mühendisi okur → nasıl kullanacağını anlar (@mmio)
Doğrulama mühendisi okur → ne test edeceğini anlar (cover)
```

### 5.2 Tek Dosyadan Her Şeyi Üretme

```volt
// ═══════════════════════════════════════════════════
// Bu tek Volt dosyası tüm artifact'ların kaynağı
// ═══════════════════════════════════════════════════

@version("1.2.3")
@author("Volt Ekibi")
@standard("AXI4-Lite Spec Rev 2.0")
module I2C_Controller {

    // ─── ARABIRIM SPECİ (SW sürücüsü buradan türer) ───
    @mmio(base = 0x4000_0000)
    regs : {
        @offset(0x00) @access(ReadWrite)
        control : {
            enable   : bool,   // [0]
            speed    : bits<2> // [2:1] 0=100kHz, 1=400kHz, 2=1MHz
        }

        @offset(0x04) @access(ReadOnly)
        status : {
            busy     : bool,
            ack_error: bool,
            arb_lost : bool
        }

        @offset(0x08) @access(ReadWrite)
        tx_data : u8

        @offset(0x0C) @access(ReadOnly)
        rx_data : u8
    }

    // ─── DAVRANIŞSAL SPEC (formal ve testbench buradan türer) ───
    requires: regs.control.speed <= 2   // geçerli hız değeri
    requires: regs.tx_data != X         // başlatılmamış veri yok

    ensures: always (regs.status.busy → !@mmio.writable(tx_data))
    ensures: ack_error → (regs.status.busy within 3.us)

    invariant: !(regs.status.busy && regs.status.arb_lost)
    invariant: i2c_scl_out → i2c_mode == open_drain

    cover: regs.control.speed == 2 && transfer_success  // 1MHz başarılı
    cover: ack_error && retry_count < 3                   // hata kurtarma
    cover: arb_lost && recovery                           // arbitration

    // ─── ZAMANLAMA SPEC ───
    @timing: scl_high_time >= 600.ns  when speed == 0  // 100kHz spec
    @timing: scl_high_time >= 60.ns   when speed == 1  // 400kHz spec
    @timing: scl_high_time >= 26.ns   when speed == 2  // 1MHz spec

    // ─── SIFIRLAMA SPEC (RDC buradan türer) ───
    @domain(I2CDomain) {
        reset_sync = sync, active_high, sequence = 2
        reset_requires = [sys_domain_stable]
    }

    // ─── İMPLEMENTASYON (RTL) ───
    // ...
}
```

**Bu dosyadan araç üretimi:**

```bash
# Rust/C yazılım sürücüsü
volt codegen --target rust  i2c_controller.volt
# → i2c_controller_driver.rs (register map + tip-güvenli erişim)

# SystemVerilog (ASIC/FPGA)
volt build --target asic-tsmc7  i2c_controller.volt
# → i2c_controller.sv

# Testbench (cocotb)
volt testbench --framework cocotb  i2c_controller.volt
# → test_i2c_controller.py (requires → stimulus, cover → senaryolar)

# Formal özellikler
volt formal  i2c_controller.volt
# → i2c_controller.sby (SymbiYosys yapılandırması)
# → i2c_controller.sva (SVA özellikler)

# RDC analiz raporu
volt analyze --rdc  i2c_controller.volt
# → reset_analysis.html

# İnsan-okunabilir spec
volt doc  i2c_controller.volt
# → i2c_controller_spec.md (proje yöneticisi okuyabilir)

# Zamanlama kısıtları
volt constraints  i2c_controller.volt
# → i2c_controller.sdc (SDC dosyası)
```

### 5.3 Spec-RTL Uyumsuzluk Tespiti

```
Bugün: spec değişiyor, RTL eski kalıyor → kimse bilmiyor

Volt ile:
  spec (kontrat) + RTL aynı dosyada
  → Kontrat değişince RTL derlenemiyor (tip kontrolü)
  → RTL kontratı ihlal edince formal yakalıyor

  "Spec ile RTL uyumsuzluğu" — endüstrinin korkusu —
  Volt'ta yapısal olarak imkânsız.

Örnek:
  ensures: baud_counter < baud_rate
  RTL: baud_counter için sınır kontrolü eklenmiyor
  
  → volt formal --depth 10 design.volt
  → Karşı örnek: baud_counter == baud_rate durumu bulundu
  → E5001: Kontrat ihlali: ensures 4. satır
  → Mühendise anında bildirim: spec ile RTL uyumsuz
```

---

## Bölüm 6 — Üçünü Birleştiren Mimari

### 6.1 Tek Formal Model

```
Üç sorun tek formal modelde birleşiyor:

Σ (sistem durumu):
  = Register değerleri (RTL)
  + Saat alan durumları (CDC)
  + Sıfırlama alan durumları (RDC)
  + Kontrat durumları (requires/ensures karşılandı mı?)

Geçiş ilişkisi T(s, s'):
  = RTL semantiği (her döngü nasıl ilerliyor)
  + Domain geçiş kuralları (CDC/RDC)
  + Kontrat geçiş kuralları (koşullar korunuyor mu?)

Özellikler P:
  = invariantlar (her durumda doğru)
  + ensures (belirli bir giriş sonrası)
  + cover (bu duruma ulaşılabilmeli)

Tek model → tek araç → tek rapor
```

### 6.2 Araç Mimarisi

```
                    design.volt
                        │
           ┌────────────┴──────────────┐
           │       Volt Derleyici       │
           │  (Parser + Tip Kontrolü)  │
           │  CDC/RDC kontrolü burada  │
           └────────────┬──────────────┘
                        │ HIR (yüksek seviye IR)
           ┌────────────┼──────────────────────┐
           │            │                      │
           ▼            ▼                      ▼
     CIRCT/MLIR    Kontrat IR           Domain IR
    (RTL üretim)  (Formal üretim)    (RDC analizi)
           │            │                      │
           ▼            ▼                      ▼
     .sv dosyası   .sby + .sva         rdc_report.html
     (sentez)      (SymbiYosys)
           │            │
           ▼            ▼
     Verilator      Formal
     (simülasyon)   (kanıt)
           │
           ▼
     Testbench     SW Driver    Spec Belgesi    SDC
     (.py)         (.rs / .h)   (.md)           (.sdc)
```

### 6.3 Kullanıcı Deneyimi (Birleşik)

```bash
# Tek komut → her şeyi kontrol
volt check design.volt

# Çıktı:
# ✓ Sözdizimi: temiz
# ✓ Tip kontrolü: 0 hata
# ✓ CDC analizi: 3 geçiş, tümü köprülü
# ✓ RDC analizi: 2 geçiş, tümü sekanslanmış
# ⚠ Formal: 2 invariant bounded (depth=20), 1 kanıtlanamadı
#   → E5001: ensures 47. satır: baud_counter sınırı
# ✓ Coverage: 5/7 cover hedefi ulaşılabilir
# ⚠ Coverage: 2 hedef ulaşılamaz olabilir
#   → W3001: cover 89. satır: ack_error && !busy eşzamanlı?
# ✓ Zamanlama: tüm @timing kısıtları SDC'de
# ✓ SW arabirimi: 4 register, tip-güvenli erişim

# Sorunları gider, sonra:
volt build --all-artifacts design.volt
# → design.sv, design_driver.rs, test_design.py,
#    design.sby, design_spec.md, design.sdc,
#    reset_analysis.html
```

---

## Bölüm 7 — Volt İçin Uygulama Yol Haritası

### MVP (F2-F5 revizyonu) — RDC Eklenmesi

```
F2 revizyonu — Domain semantiği genişletme:
  domain { } bloğuna reset_sync, reset_polarity,
  reset_cycles, reset_sequence_id ekle
  
  ADR-0002 revizyonu: "CDC + RDC birleşik semantik"

F4 revizyonu — RDC stdlib primitifleri:
  ResetSync<SrcDomain, DstDomain>
  ResetSequencer otomatik üretimi
  ConditionalReset koşullu köprü

Hata kodları:
  E3003: RDC ihlali (alan uyumsuzluğu)
  E3004: Sıfırlama sekans ihlali (sıra yanlış)
  E3005: Koşullu sıfırlama ihlali (koşul karşılanmıyor)
```

### V1 — Kontrat Sistemi Temeli

```
Dil eklemeleri:
  requires { ... }
  ensures { ... }
  cover:   ifadesi

Araç eklemeleri:
  volt testbench: kontratlardan cocotb üretimi
  volt formal: kontratlardan SVA üretimi
  volt doc: kontratlardan spec belgesi üretimi
  volt impact: etki analizi (hangi test yeniden çalışmalı?)

ADR: ADR-0011-kontrat-sistemi.md
```

### V2 — Tam Tek Kaynak Mimarisi

```
Dil eklemeleri:
  @mmio, @offset, @access anotasyonları
  @timing zamanlama kısıtları
  @standard referans etiketi

Araç eklemeleri:
  volt codegen --target rust/c: SW sürücü üretimi
  volt constraints: SDC/XDC üretimi
  volt impact: regresyon analizi
  volt analyze --rdc: RDC raporu

Sonuç:
  Tek Volt dosyasından:
  RTL + Testbench + Formal + SW Driver + Spec + SDC + RDC
  "Altın spec" problem çözüldü
```

---

## Bölüm 8 — Endüstriye Etkisi (Gerçekçi Tahmin)

```
%14 ilk silisyum başarısı için ne değişir:

CDC/RDC hataları:
  Bugün: %20-30 re-spin nedeni
  Volt ile: yakın sıfır (derleme zamanı)
  → İlk silisyum başarısı: +5-10 puan

Spec-RTL uyumsuzluğu:
  Bugün: %15-25 re-spin nedeni
  Volt ile: structural olarak azaltılmış
  → İlk silisyum başarısı: +5-8 puan

Doğrulama kapsamı:
  Bugün: cover hedefleri elle, eksik
  Volt ile: kontratlardan otomatik, sistematik
  → Kaçan bug azalır: +3-5 puan

Toplam tahmini etki:
  %14 → %27-37 ilk silisyum başarısı (2-3× iyileşme)

Doğrulama süresi:
  %60-70 toplam süreden → %40-50 hedefi
  Kontrat-driven testbench + regresyon analizi ile
  → Mühendis çalışmayı azaltmak değil, yanlış yerlerde
    çalışmayı azaltmak (etkilenmeyen testi çalıştırmamak)

Uyarı:
  Bu sayılar teorik.
  Gerçek etki: uygulama kalitesine bağlı.
  Kanıt için: gerçek vaka çalışması şart.
```

---

## Bölüm 9 — Neden Bu Bir HDL Problemi

```
Neden EDA araçları (SpyGlass, JasperGold) bu sorunu çözmedi:

SpyGlass RDC:
  RTL'i analiz eder
  Ama RTL'de reset semantiği YOK — engineer kafasında
  Araç tahmin eder, kesin bilemez
  Yanlış pozitif → güven kaybı
  
JasperGold Formal:
  SVA assertion gerektirir
  SVA ayrı dosyada → spec-RTL boşluğu kalır
  Mühendis SVA yazar → hata yapabilir
  Araç yazılan SVA'yı kanıtlar → yanlış spec kanıtlanır

Volt'un farkı:
  Reset semantiği dil içinde (RTL'den ayrı değil)
  → Araç "tahmin" etmiyor, "biliyor"
  
  Kontrat RTL ile birlikte (ayrı dosyada değil)
  → Spec-RTL uyumsuzluğu yapısal olarak imkânsız

  "Araç RTL'e baktıktan sonra analiz yapar"
  yerine
  "Dil tasarım sırasında semantiği kodlar"
  
  Bu fark küçük görünüyor ama sonucu büyük:
  Araç analizi = sonradan → hata kaçabilir
  Dil kısıtı = tasarım sırasında → hata kaçamaz
```

---

## Özet

```
Üç kronik sorunun tek çözüm prensibi:
  "Örtük bilgiyi açık yap — aynı artifact içinde"

RDC çözümü:
  Domain tanımına reset semantiği ekle
  Aynı @Domain mekanizması → E3003 RDC hatası
  ResetSync, ResetSequencer stdlib primitifleri

Doğrulama verimliliği:
  requires/ensures/invariant/cover kontrat sistemi
  Kontratlardan testbench, formal, coverage otomatik
  Regresyon analizi: sadece etkilenen testler

Spesifikasyon uçurumu:
  design.volt = altın spec
  Tek dosyadan: RTL + SW driver + testbench + formal + belgeler
  Spec-RTL uyumsuzluğu → yapısal olarak imkânsız

Üçü birden:
  Tek formal model (Σ, T, P)
  Volt Derleyici → HIR → üç alt sistem
  Tek komut → tüm artifact'lar

Beklenen etki (gerçekçi):
  İlk silisyum başarısı: %14 → %27-37
  Doğrulama süresi: %65 → %45
  RDC hataları: sıfır (derleme zamanı)
  Spec-RTL boşluğu: yapısal olarak yok

Bugün yapılması gereken:
  ADR-0002 revizyonu: domain = saat + sıfırlama semantiği
  F2'ye RDC tip kontrolü ekle
  Bu tek karar üç kronik sorunu aynı anda ele almaya başlar.
```
