# Volt Ekosistemi — Bütünleşik Mimari v3.0
## 45 Özelliğin Sinerjik Konsolidasyonu

> Bu belge, oturumda tanımlanan tüm özellikleri tek bir tutarlı
> mimariye oturtur. Özellikler bağımsız eklentiler olarak değil,
> birbirini besleyen katmanlar olarak tasarlanmıştır.
>
> ÖNCEKİ BELGELERİ GEÇERSİZ KILAR:
>   - Volt-HDL-Dil-Spesifikasyonu.md (v1 → bu belge v3)
>   - Volt-HDL-MVP-Yol-Haritasi.md (revize edildi)
>   - Volt-Sinerjik-Ozellikler-Analizi.md (buraya entegre)
>   - Volt-Ekosistem-Butunlesik-Yol-Haritasi-v2.md (v3'e yükseltildi)

---

# BÖLÜM I — MİMARİ İLKE: ÜÇ EKSEN

45 özelliği anlamlı kılan tek bir organize edici fikir var:

```
                    ┌─────────────────────┐
                    │   TEK KAYNAK        │
                    │   design.volt       │
                    └──────────┬──────────┘
                               │
        ┌──────────────────────┼──────────────────────┐
        │                      │                      │
        ▼                      ▼                      ▼
   ┌─────────┐          ┌──────────┐          ┌──────────┐
   │ EKSEN 1 │          │ EKSEN 2  │          │ EKSEN 3  │
   │ TİP     │          │ KONTRAT  │          │ ÜRETİM   │
   │ SİSTEMİ │          │ SİSTEMİ  │          │ KATMANI  │
   └─────────┘          └──────────┘          └──────────┘
   Hata İMKANSIZ        Niyet AÇIK            Artifact TÜREVİ
   (compile-time)       (verifiable)          (single source)

Her özellik bu üç eksenden birine oturur.
Hiçbir özellik "bağımsız eklenti" değildir.
```

## Eksen Ataması

```
EKSEN 1 — TİP SİSTEMİ (hatayı imkânsız kıl):
  CDC + RDC birleşik semantik
  Güç alanları (Power domains)
  Lineer tipler
  Timeline tipleri + kaynak çakışması
  Trit tipi
  Option<T>
  Güvenlik akış denetimi (@Secure/@Public)
  Modül sistemi + görünürlük
  Reset soyutlaması (if_reset)

EKSEN 2 — KONTRAT SİSTEMİ (niyeti açık yap):
  requires / ensures / invariant / cover
  Kaynak bütçeleri (@budget)
  Zamanlama kısıtları (@timing)
  Zamanlama istisnaları (@false_path, @multicycle)
  DFT farkındalığı (@dft)
  Debug altyapısı (@debug_*)
  SemVer donanım (@version, @abi_version)
  todo! mekanizması
  Hook mekanizması

EKSEN 3 — ÜRETİM KATMANI (tek kaynaktan türet):
  SystemVerilog (RTL)
  HW-SW köprüsü (Rust/C sürücü)
  SDC/XDC (zamanlama kısıtları)
  UPF (güç niyeti)
  Cocotb testbench (tip-farkında)
  SymbiYosys formal (SVA)
  Coverage grupları
  İnsan-okunabilir spec
  RDC analiz raporu
  Debug altyapısı (JTAG + GDB stub)
```

---

# BÖLÜM II — EKSEN 1: BİRLEŞİK TİP SİSTEMİ

## 2.1 Domain: Üç Alanın Tek Kavramı

**En kritik konsolidasyon kararı.** Saat, sıfırlama ve güç alanları ayrı
kavramlar değil — tek `domain` bloğunun üç yüzü:

```volt
domain SysDomain {
    // ─── SAAT (CDC) ───
    clock          = posedge
    frequency      = 100.mhz
    jitter         = 50.ps        // opsiyonel, timing analizi için

    // ─── SIFIRLAMA (RDC) ───
    reset_sync     = sync          // sync | async
    reset_polarity = active_high   // active_high | active_low
    reset_cycles   = 4             // kaç döngü aktif kalmalı
    reset_sequence = 1             // sıfırlama sırası (küçük = önce)
    reset_requires = []            // koşullu sıfırlama

    // ─── GÜÇ (PDC) ───
    voltage        = 0.8.V
    always_on      = false
    retention      = true          // uyku modunda durum korunur
    isolation      = clamp_low     // kapalıyken çıkış davranışı
    power_sequence = 1             // güç açma sırası
}

domain AlwaysOnDomain {
    clock          = posedge
    frequency      = 32.khz
    reset_sync     = async
    reset_polarity = active_low
    reset_sequence = 0             // en önce sıfırlanır
    voltage        = 0.9.V
    always_on      = true          // hiç kapanmaz
    power_sequence = 0             // en önce açılır
}
```

**Neden birleşik:**

```
Ayrı olsaydı:
  @clock(Sys) @reset(SysRst) @power(CoreVDD) data : u8
  → verbose, hata yapmaya açık, üç ayrı kontrol

Birleşik:
  data : u8 @SysDomain
  → tek anotasyon, üç kontrol birden

Sinerjik kazanç:
  Bir domain geçişi üç açıdan aynı anda kontrol ediliyor:
    Saat farklı mı?     → E3001 (CDC)
    Sıfırlama farklı mı?→ E3003 (RDC)
    Güç alanı farklı mı?→ E3006 (PDC)

  Tek tip sistemi geçişi, üç sınıf hatayı kapatıyor.
  Hiçbir rakip HDL'de bu üçlü birleşim yok.
```

**Hata kodları:**

```
E3001  Saat alanı uyumsuzluğu (CDC)
E3002  Tanımsız saat alanı
E3003  Sıfırlama alanı uyumsuzluğu (RDC)
E3004  Sıfırlama sekans ihlali
E3005  Koşullu sıfırlama karşılanmadı
E3006  Güç alanı geçişi izolasyonsuz (PDC)
E3007  Güç sekans ihlali
E3008  Retention gerektiren sinyal retention'sız alanda
```

**Stdlib köprüleri:**

```volt
// Tek çağrı, üç geçişi de halleder
module DomainBridge<T, Src: Domain, Dst: Domain> {
    in  data_in  : T @Src
    out data_out : T @Dst

    // Derleyici gerekli hücreleri otomatik seçer:
    //   Saat farklı → TwoFlop veya AsyncFifo
    //   Reset farklı → ResetSync
    //   Güç farklı  → IsolationCell + LevelShifter

    @auto_infer_cells   // hangi hücreler gerekli, araç karar verir
}

// Açık kontrol isteyenler için ayrı primitifler:
module TwoFlop<T, Src, Dst> { ... }       // sadece CDC
module ResetSync<Src, Dst> { ... }         // sadece RDC
module IsolationCell<Src, Dst> { ... }     // sadece PDC
module LevelShifter<Src, Dst> { ... }      // voltaj dönüşümü
```

## 2.2 Lineer Tipler (Spade'den)

```volt
// Ters tel (inverted wire): tam bir kez tüketilmeli
struct port MemPort<T> {
    addr  : &inv u16,    // & inv = tüketici tarafı yazar
    data  : &T,          // & = üretici tarafı yazar
}

module Top {
    let (p1, p2) = inst DualPortRam<u32>();

    inst compute_a(p1);
    inst compute_b(p1);
    //              ^^ E4003: p1 zaten tüketildi (satır 8)
    //                 help: p2 kullanın veya port çoğaltın
}
```

**Sinerji:** Lineer tipler + Domain sistemi = çift sürücü VE domain
ihlali aynı anda yakalanıyor. Spade'de sadece ilki var.

## 2.3 Modül Sistemi ve Ölçek

```volt
// ─── Paket tanımı (dizin yapısıyla eşleşir) ───
// src/interconnect/axi.volt
package soc::interconnect;

pub module AxiCrossbar<const N: usize> { ... }   // dışa açık
     module ArbiterInternal { ... }              // paket-içi

pub use soc::interconnect::AxiCrossbar;          // yeniden dışa aktar

// ─── Kullanım ───
// src/top.volt
import soc::interconnect::AxiCrossbar;
import soc::peripherals::{Uart, Spi};            // çoklu import

module Top {
    let xbar = AxiCrossbar<4>();
    // ...
}
```

**Ayrık derleme (incremental compilation):**

```
Modül arayüzü hash'i değişmediyse → bağımlı modüller
yeniden derlenmez.

Arayüz = port listesi + kontratlar + tip parametreleri
İmplementasyon değişikliği → sadece o modül yeniden derlenir

volt build --timings
  soc::interconnect::AxiCrossbar   [önbellek]  0.0s
  soc::peripherals::Uart           [derlendi]  1.2s
  soc::top                         [derlendi]  0.4s
  Toplam: 1.6s (tam derleme: 47s)
```

## 2.4 Trit ve Option — Stdlib Temel Tipleri

```volt
// Trit: opt-in, prelude'de DEĞİL
import volt::ternary::Trit;

// Option<T>: prelude'de VAR (evrensel valid/data deseni)
module Fifo<T, const N: usize> {
    out data : Option<T>    // Some(x) = valid, None = boş

    // Pattern matching ile güvenli erişim:
    match data {
        Some(x) => process(x),
        None    => idle(),
    }
    // "valid kontrolü unutuldu" hatası imkânsız
}
```

## 2.5 Güvenlik Akış Denetimi

```volt
// Domain'in dördüncü yüzü: güven seviyesi
domain SecureDomain {
    clock       = posedge
    reset_sync  = sync
    voltage     = 0.8.V
    trust_level = secret        // secret | confidential | public
}

module AesCore {
    in  key        : bits<256> @SecureDomain
    in  plaintext  : bits<128> @PublicDomain
    out ciphertext : bits<128> @PublicDomain

    // E3009: Bilgi akışı ihlali
    // out leaked : bits<256> @PublicDomain = key

    @constant_time(key)   // zamanlama yan kanalı yok
    @no_power_leak(key)   // güç örüntüsü key'e bağlı değil (v2)
}
```

**Sinerji:** Trust level domain'in bir özelliği olduğu için,
CDC/RDC/PDC ile aynı mekanizmayı kullanıyor — ayrı sistem değil.

---

# BÖLÜM III — EKSEN 2: KONTRAT SİSTEMİ

## 3.1 Birleşik Kontrat Sözdizimi

```volt
module I2cController {
    // ─── DAVRANIŞSAL KONTRAT ───
    requires:  speed_sel <= 2
    ensures:   busy → !tx_writable
    invariant: !(busy && arb_lost)
    cover:     speed_sel == 2 && transfer_ok      // 1MHz senaryosu
    cover:     ack_error && retry_count < 3       // hata kurtarma

    // ─── KAYNAK KONTRATI ───
    @budget(lut = 2_000, ff = 800, bram = 2)
    @budget(power = 15.mW, area = 0.08.mm2)
    @budget(latency = 100.ns)

    // ─── ZAMANLAMA KONTRATI ───
    @timing(scl_high >= 600.ns) when speed_sel == 0
    @timing(scl_high >= 60.ns)  when speed_sel == 1
    @false_path(from = config_reg, to = scl_gen)
    @multicycle(from = div_stage1, to = div_stage4, cycles = 4)

    // ─── TEST EDİLEBİLİRLİK KONTRATI ───
    @dft(scan_chain = true, coverage_target = 0.99)
    @dft(bist = memory)

    // ─── DEBUG KONTRATI ───
    @debug_visible(state, byte_counter)
    @debug_trace(depth = 512)
    @debug_breakpoint(when = ack_error)

    // ─── SÜRÜM KONTRATI ───
    @version("2.1.0")
    @abi_version(2)

    // ─── İMPLEMENTASYON ───
    // RTL buradan başlar
}
```

**Kritik tasarım kararı:** Tüm kontratlar aynı sözdizim ailesinde.
Ayrı dosya yok, ayrı dil yok, ayrı araç yok.

## 3.2 Kontratın Sekiz Tüketicisi

```
                    KONTRATLAR
                        │
    ┌──────┬──────┬─────┼─────┬──────┬──────┬──────┐
    ▼      ▼      ▼     ▼     ▼      ▼      ▼      ▼
  Formal Test  Cover  SDC   UPF   DFT  Debug  Spec
  (SVA)  bench  grup  (tim) (pwr) ins.  altya. belge
```

| Kontrat | Formal | Testbench | Coverage | SDC | UPF | DFT | Debug | Spec |
|---|---|---|---|---|---|---|---|---|
| `requires` | assume | stimulus kısıtı | — | — | — | — | — | ✓ |
| `ensures` | assert | kontrol | — | — | — | — | — | ✓ |
| `invariant` | assert | kontrol | — | — | — | — | — | ✓ |
| `cover` | cover | senaryo hedefi | covergroup | — | — | — | — | ✓ |
| `@budget` | — | — | — | — | — | — | — | ✓ |
| `@timing` | — | — | — | ✓ | — | — | — | ✓ |
| `@false_path` | kanıt! | — | — | ✓ | — | — | — | ✓ |
| `@dft` | — | — | — | — | — | ✓ | — | ✓ |
| `@debug_*` | — | — | — | — | — | — | ✓ | ✓ |
| domain (güç) | — | — | — | — | ✓ | — | — | ✓ |

**En değerli hücre:** `@false_path` → **kanıt**.
Bugün endüstride false path elle iddia edilir, kimse doğrulamaz.
Yanlış false path = silisyumda çalışmayan tasarım.
Volt formal ile kanıtlar: gerçekten ulaşılamaz mı?

## 3.3 todo! ve Kısmi Derleme

```volt
module Cache {
    // Emin olunan kısım — tam derleniyor
    comb { mem_req.addr = req.addr; }

    // Belirsiz kısım — tip kontrolünden geçiyor, simülasyonda durur
    comb { evict_way = todo!("LRU mu tree-PLRU mu?"); }

    // Kontrat da todo! olabilir
    ensures: todo!("tahliye politikası kararlaştırılınca yazılacak")
}
```

```bash
volt check design.volt
# ✓ Tip kontrolü: 0 hata
# ⚠ 2 todo! bulundu:
#     cache.volt:14 — "LRU mu tree-PLRU mu?"  [tip: bits<3>]
#     cache.volt:17 — "tahliye politikası..."  [kontrat]
# → Sentez engellendi (todo! ile build --release yapılamaz)
# → Simülasyon mümkün (todo! noktasında durur)
```

**Sinerji:** `todo!` + AI destekli tasarım. LLM emin olduğu kısmı
yazar, belirsizi `todo!` bırakır, tip sistemi geri kalanı doğrular.
Arch'ta bu var ama tip sistemi Volt kadar güçlü değil.

## 3.4 Hook Mekanizması

```volt
// Birinci sınıf yapıya politika enjeksiyonu
arbiter MemArbiter<const N: usize> {
    // Varsayılan: round-robin
    hook grant_select(req: bits<N>, last: bits<N>) -> bits<N>
        = priority_with_aging(req, last, age_counters);

    // Hook implementasyonu kullanıcının, ama:
    // Yapının kontratları KORUNUYOR:
    invariant: popcount(grant) <= 1        // tek grant
    invariant: grant & req == grant        // sadece isteyene ver
    // Hook bu invariantları ihlal ederse → derleme hatası
}
```

**Neden stdlib fonksiyonundan üstün:** Yapının güvenlik garantileri
korunuyor. Kullanıcı politikayı değiştiriyor, invariant'ı değil.

---

# BÖLÜM IV — EKSEN 3: ÜRETİM KATMANI

## 4.1 Tek Komut, On Artifact

```bash
volt build --all design.volt
```

```
design.volt
    │
    ├─→ build/rtl/design.sv              (SystemVerilog, ECO-uyumlu)
    ├─→ build/sw/design_driver.rs        (Rust sürücü)
    ├─→ build/sw/design_driver.h         (C header)
    ├─→ build/tb/test_design.py          (cocotb, tip-farkında)
    ├─→ build/formal/design.sby          (SymbiYosys)
    ├─→ build/formal/design.sva          (SVA özellikler)
    ├─→ build/constraints/design.sdc     (Synopsys timing)
    ├─→ build/constraints/design.xdc     (Xilinx timing)
    ├─→ build/power/design.upf           (güç niyeti)
    ├─→ build/dft/design_scan.tcl        (DFT insertion)
    ├─→ build/debug/design_gdb_stub.c    (debug altyapısı)
    ├─→ build/docs/design_spec.md        (insan-okunabilir)
    ├─→ build/reports/rdc_analysis.html  (RDC raporu)
    ├─→ build/reports/coverage_plan.html (coverage hedefleri)
    └─→ volt.lock                        (determinizm kaydı)
```

## 4.2 HW-SW Köprüsü (Detay)

```volt
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module I2cRegisters {
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        enable    : bool,       // [0]
        reset     : bool,       // [1] self-clearing
        speed     : bits<2>,    // [3:2]
        @reserved : bits<28>,
    }

    @reg(offset = 0x04, access = ReadOnly, volatile)
    status : {
        busy      : bool,
        ack_error : bool,       // @w1c: write-1-to-clear
        rx_count  : bits<8>,
    }
}
```

Üretilen Rust:

```rust
// build/sw/i2c_driver.rs — OTOMATIK ÜRETİLDİ, DÜZENLEMEYİN
// Kaynak: i2c_registers.volt @version 2.1.0

#[repr(C)]
pub struct I2cRegisters { /* ... */ }

impl I2cRegisters {
    /// I2C denetleyiciyi etkinleştir
    pub fn set_enable(&mut self, val: bool) { /* ... */ }

    /// Meşgul durumu oku (volatile)
    pub fn is_busy(&self) -> bool { /* ... */ }

    /// ACK hatasını temizle (write-1-to-clear)
    pub fn clear_ack_error(&mut self) { /* ... */ }

    // set_status() ÜRETİLMEDİ — ReadOnly register
    // → Yazma girişimi derleme hatası
}

/// Kontrattan türetilen güvenlik kontrolü
impl I2cRegisters {
    pub fn set_speed(&mut self, val: u8) -> Result<(), Error> {
        // requires: speed_sel <= 2  ← Volt kontratından
        if val > 2 { return Err(Error::InvalidSpeed); }
        /* ... */
    }
}
```

**Sinerji zinciri:** Kontrat (`requires`) → hem formal assertion
hem Rust runtime kontrolü. Tek kaynak, iki dünya.

## 4.3 Cocotb Tip-Farkında Entegrasyon

```python
# build/tb/test_i2c.py — otomatik üretildi
from volt_cocotb import VoltDut

@cocotb.test()
async def test_high_speed_transfer(dut):   # cover hedefinden türedi
    v = VoltDut(dut)

    # Volt tiplerini string olarak yaz — bit hesabı yok
    v.i.control = "Control { enable: true, speed: 2 }"
    v.i.tx_data = "Some(0xA5)"

    await RisingEdge(v.clk)

    # ensures kontratından otomatik assertion
    v.o.status.assert_field("busy", True)
    v.o.rx_data.assert_eq("Some(0x5A)")
```

## 4.4 Determinizm ve volt.lock

```toml
# volt.lock — OTOMATIK ÜRETİLDİ
[build]
volt_version   = "0.4.2"
volt_sha       = "a3f2b1c..."
circt_version  = "1.82.0"
circt_sha      = "d4e5f6a..."
timestamp      = "2026-08-07T14:32:11Z"

[sources]
"src/top.volt"           = "sha256:8f3a..."
"src/i2c.volt"           = "sha256:2b7c..."

[dependencies]
"stdlib-axi"             = { version = "2.1.0", sha256 = "9d1e..." }

[outputs]
"build/rtl/design.sv"    = "sha256:5c8f..."
"build/sw/driver.rs"     = "sha256:1a2b..."
```

```bash
volt verify --reproducible
# ✓ Kaynak hash'leri eşleşiyor
# ✓ Derleyici sürümü eşleşiyor
# ✓ Çıktı bit-bit özdeş
# → ISO 26262 tool qualification kanıtı üretildi
```

## 4.5 Anlamsal Diff ve SemVer

```bash
volt diff v2.0.0 v2.1.0
```

```
ANLAMSAL DEĞİŞİKLİK ANALİZİ

[KIRICI] Port kaldırıldı: awqos (i2c.volt:23)
  → MAJOR sürüm gerekli, ancak MINOR bump yapılmış
  → E7001: SemVer ihlali

[UYUMLU] Kontrat güçlendirildi: ensures satır 45
  Eski: busy → !tx_writable
  Yeni: busy → (!tx_writable && !rx_readable)
  → Daha güçlü garanti, kullanıcı kodu etkilenmez
  → MINOR uygun ✓

[İÇ] FSM durum kodlaması değişti: one-hot → binary
  → Arayüz etkilenmedi, PATCH yeterli ✓

[ETKİ] Bu değişiklikten etkilenen testler:
  test_backpressure.py  (ensures 45'e bağlı)
  test_qos_priority.py  (awqos portuna bağlı — KIRIK)
  Etkilenmeyen 14 test atlanabilir.
```

**Sinerji:** Anlamsal diff + kontrat sistemi + regresyon analizi.
Üçü aynı bilgiden besleniyor.

## 4.6 Fiziksel Tasarım Geri Bildirimi

```bash
# P&R sonrası, zamanlama ihlali var
volt feedback --from-sta timing_report.rpt design.volt
```

```
ZAMANLAMA İHLALİ → VOLT KAYNAK EŞLEMESİ

İhlal: -0.83 ns (setup)
Kritik yol:
  alu.volt:34  (adder çıkışı)
    → mux.volt:12  (seçici)
      → regfile.volt:56  (yazma portu)

Kontrat kontrolü:
  @budget(latency = 5.ns) tanımlıydı → aşıldı (5.83 ns)

Öneriler (yapısal):
  1. alu.volt:34 sonrası pipeline aşaması ekle:
       Delayed<u32, 1>(adder_out)
       → +1 döngü gecikme, kritik yol -2.1 ns tahmini
  2. mux.volt:12'yi ALU'dan önce taşı
       → mantık derinliği azalır
  3. @budget(latency) değerini 6.ns'e yükselt
       → sistem seviyesi etkisi kontrol edilmeli
```

---

# BÖLÜM V — ÇAKIŞMA ÇÖZÜMLERİ

Bu özelliklerin bazıları birbiriyle çelişiyor. Kararlar:

## Çakışma 1: CIRCT Optimizasyonu vs ASIC ECO Uyumluluğu

```
Sorun:
  CIRCT agresif optimizasyon yapabilir
  → Üretilen SV, Volt kaynağından tanınmaz hale gelir
  → ASIC timing ECO imkânsızlaşır (Veryl'in kaçındığı şey)

KARAR: İki mod
  volt build                    → varsayılan: yapı korunur
                                  (CIRCT optimizasyonu SINIRLI)
  volt build --optimize=aggressive → tam CIRCT optimizasyonu
                                     (FPGA için, ECO gerekmez)

Varsayılan modda garanti:
  Volt modülü ↔ SV modülü: 1:1
  Volt sinyali ↔ SV sinyali: isim korunur
  Volt reg ↔ SV always_ff: yapısal karşılık var
  → ECO mühendisi kaynağı tanıyabilir

ADR-0012: SV çıktı öngörülebilirlik garantisi
```

## Çakışma 2: LL(1) Gramer vs Zengin Sözdizimi

```
Sorun:
  LL(1) = tek token ileri bakış
  Ama: kontratlar, anotasyonlar, generic'ler karmaşık sözdizimi ister

KARAR: LL(1) hedef, LL(2) tavan
  Gramer LL(1) olmaya ÇALIŞIR
  Gerektiğinde LL(2) kabul edilir (2 token ileri bakış)
  ASLA: geri izleme (backtracking) veya belirsizlik

  Pratik kural: her yapı benzersiz anahtar kelimeyle başlar
    module, fn, pipeline, fsm, arbiter, fifo, domain, package
    requires, ensures, invariant, cover
    @budget, @timing, @dft, @debug_*, @mmio

  Bu sayede AI üretimi güvenilir kalıyor.

ADR-0013: Gramer sınıfı ve AI-üretilebilirlik kontratı
```

## Çakışma 3: Trit Görünürlüğü vs AI Donanımı Vizyonu

```
Sorun:
  Trit öne çıkarsa → "ezoterik dil" algısı
  Trit gizlenirse  → AI donanımı kullanıcıları bulamaz

KARAR: Katmanlı görünürlük
  Prelude:        Option<T>, temel tipler   (herkes görür)
  volt::ternary:  Trit                       (opt-in import)
  Belgeleme:      Bölüm 1-9 Trit YOK
                  Bölüm 10 "AI Donanımı"     (arayan bulur)
  README:         Trit ilk 3 paragrafta YOK

  Sentez uyarısı zorunlu:
    W2001: Trit FPGA hedefinde verimsiz (ASIC için tasarlandı)

ADR-0003 revizyonu: Trit görünürlük politikası
```

## Çakışma 4: Kontrat Zenginliği vs Öğrenme Eğrisi

```
Sorun:
  8 farklı kontrat türü (requires, ensures, invariant, cover,
  @budget, @timing, @dft, @debug) → yeni başlayan boğulur

KARAR: Kademeli açığa çıkarma (progressive disclosure)
  Seviye 1 (öğretici 1-5):  kontrat YOK, sadece RTL
  Seviye 2 (öğretici 6-10): invariant + cover
  Seviye 3 (ileri):         requires/ensures
  Seviye 4 (ASIC):          @budget, @timing, @dft
  Seviye 5 (üretim):        @debug, @version, @false_path

  Hiçbir kontrat ZORUNLU değil.
  Kontrat olmayan modül de derlenir.
  Kontrat eklendikçe araç daha fazla artifact üretir.

  "Ödediğin kadar al" (pay-as-you-go) modeli.

ADR-0014: Kontrat sistemi kademeli benimseme
```

## Çakışma 5: Determinizm vs Artımlı Derleme

```
Sorun:
  Artımlı derleme → önbellek → "aynı girdi aynı çıktı" riski
  Determinizm     → her şeyi sıfırdan derle → yavaş

KARAR: İki mod, açık ayrım
  volt build              → artımlı, hızlı, geliştirme için
  volt build --release    → sıfırdan, deterministik, volt.lock üretir
                            (tape-out ve sertifikasyon için)

  --release modu:
    Önbellek kullanılmaz
    Paralel derleme deterministik sıralı
    Zaman damgası, yol adı, ortam değişkeni çıktıya sızmaz
    volt.lock yazılır

ADR-0015: Determinizm garantisi kapsamı
```

## Çakışma 6: Hook Esnekliği vs Kontrat Garantisi

```
Sorun:
  Hook kullanıcı kodu enjekte ediyor
  → Yapının invariantları ihlal edilebilir mi?

KARAR: Hook kontrat altında
  Hook imzası kontrat içerir:
    hook grant_select(req: bits<N>) -> bits<N>
      ensures: popcount(result) <= 1
      ensures: result & req == result

  Kullanıcı hook'u bu kontratı sağlamalı
  → Formal ile doğrulanır
  → İhlal → derleme hatası

  Yani: esneklik VAR, garanti KAYBOLMUYOR.

ADR-0016: Hook kontrat uyumu
```

---

# BÖLÜM VI — REVİZE YOL HARİTASI

## F0 (Ay 0-1) — Değişmedi

```
Counter → SV → Verilator → CI
CIRCT yok, string template yeterli
```

## F1 (Ay 1-3) — Genişletildi

```
ÖNCEDEN: Parser + CST
ŞİMDİ:   Parser + CST + [YENİ EKLENENLER]

+ Modül sistemi ve paket semantiği     ← YENİ (kritik!)
+ todo! mekanizması                     ← YENİ (Arch'tan)
+ Adlandırılmış blok sonlandırma        ← YENİ (Arch'tan)
+ _ önekli değişken konvansiyonu        ← YENİ (Veryl'den)
+ Trailing comma izni                   ← YENİ (Veryl'den)
+ LL(1)/LL(2) gramer disiplini          ← YENİ

Neden F1'de: Bunlar gramer kararları. Sonradan değiştirmek
             tüm parser'ı etkiler.
```

## F2 (Ay 3-8) — Önemli Ölçüde Genişletildi

```
ÖNCEDEN: Tip sistemi + CDC
ŞİMDİ:   Birleşik domain tip sistemi

F2a: Temel tipler (u8-u64, i8-i64, bool, bits<N>)
F2b: Trit tipi (opt-in) + Option<T> (prelude)
F2c: Domain sistemi — SAAT + SIFIRLAMA + GÜÇ birleşik  ← GENİŞLETİLDİ
     E3001-E3008 hata kodları
     if_reset soyutlaması (Veryl'den)                   ← YENİ
F2d: Lineer tipler (port güvenliği)                     ← YENİ (Spade'den)
F2e: Modül görünürlüğü ve isim çözümleme
F2f: Güvenlik akış denetimi (trust_level)               ← YENİ

Süre: 5 ay (öncesi 4 ay) — genişleme nedeniyle
```

## F3 (Ay 8-11) — Ek Kısıt

```
CIRCT lowering
+ SV çıktı öngörülebilirlik garantisi (ADR-0012)  ← YENİ KISIT
+ İki optimizasyon modu (varsayılan/aggressive)    ← YENİ
```

## F4 (Ay 11-14) — Kontrat Sistemi Eklendi

```
ÖNCEDEN: Simülatör + Formal
ŞİMDİ:   Simülatör + Formal + Kontrat Sistemi v1

+ requires/ensures/invariant/cover sözdizimi     ← YENİ
+ Kontrat → SVA üretimi                           ← YENİ
+ Kontrat → coverage grup üretimi                 ← YENİ
+ SymbiYosys entegrasyonu (mevcuttu)
+ Cocotb tip-farkında köprü                       ← YENİ (Spade'den)
```

## F5 (Ay 14-16) — Ekosistem Temeli

```
+ LSP (mevcuttu)
+ Volt.toml proje yapısı (Swim'den)               ← YENİ
+ Option<T> ve domain köprüleri stdlib            ← YENİ
+ Hook mekanizması                                 ← YENİ (Arch'tan)
+ volt doc → spec belgesi üretimi                 ← YENİ
+ Beta sürümü
```

**MVP toplam: 12 ay → 16 ay** (kapsam genişlemesi nedeniyle dürüst tahmin)

## V1 (Yıl 2) — Üretim Katmanı

```
Öncelik sırası (sinerji sırasına göre):

1. HW-SW köprüsü (@mmio → Rust/C sürücü)      [3 ay]
   → En sinerjik, en çok kullanıcı çeker

2. Volt.lock + determinizm garantisi           [1 ay]
   → Fonksiyonel güvenlik kapısı

3. SDC/XDC üretimi (@timing, @false_path)      [2 ay]
   → ASIC ekipleri için kritik

4. UPF üretimi (domain güç bilgisinden)        [1 ay]
   → Güç alanları F2'de zaten var, üretim kolay

5. Paket registry + SemVer donanım             [3 ay]
   → Ekosistem büyümesi

6. volt diff (anlamsal + etki analizi)          [2 ay]
   → Regresyon verimliliği

7. Playground (WASM)                            [2 ay]
   → Topluluk büyümesi

8. AI destekli tasarım (LLM + tip sistemi)      [sürekli]
   → todo! + kontrat + hata mesajı döngüsü

9. @budget kaynak bütçeleri                     [2 ay]
   → ML PPA tahmin motoruyla birlikte

10. REPL (Clash'ten)                            [1 ay]
```

## V2 (Yıl 3-4) — İleri Doğrulama ve Alan Kütüphaneleri

```
+ @dft test edilebilirlik farkındalığı
+ @debug_* kalıcı hata ayıklama altyapısı
+ Fiziksel tasarım geri bildirimi (STA → Volt kaynak)
+ IP sertifikasyon sistemi
+ Doğrulama yapay zekası (formal counterexample → test)
+ Donanım testi otomatik üretimi
+ Surfer benzeri dalga formu görüntüleyici (veya Surfer entegrasyonu)
+ Kriptografi kütüphanesi (@constant_time ile)
+ DSP kütüphanesi (FixedPoint<W,F,Overflow>)
+ Ağ/iletişim protokol kütüphanesi (Ethernet, PCIe, USB)
+ Timeline tipleri L2 + kaynak çakışması analizi (Filament'ten)
+ Pipeline stage referansları (Spade'den)
+ Auto-pipelining delay çıkarımı (Clash'ten)
+ Spike<Trit> nöromorfik uzantı
```

## V3 (Yıl 5+) — Paradigma Genişlemesi

```
+ PTrit fotonik tipler
+ Analog/mixed-signal arayüzü
+ Mimari İşletim Sistemi
+ FE+EO CIM desteği
```

---

# BÖLÜM VII — GÜNCELLENMİŞ ADR LİSTESİ

```
MEVCUT (revize edildi):
ADR-0001  Lisans politikası                        [değişmedi]
ADR-0002  CDC semantiği → CDC+RDC+PDC BİRLEŞİK    [REVİZE ★]
ADR-0003  Trit tipi → + görünürlük politikası      [REVİZE]
ADR-0004  Hata kodu formatı → E3001-E3009 genişl.  [REVİZE]
ADR-0005  Workspace crate sınırları                [değişmedi]
ADR-0006  HIR düğüm stratejisi                     [değişmedi]
ADR-0007  L0/L1/L2 zamanlama seviyeleri            [değişmedi]
ADR-0008  SV çıktı stili                           [değişmedi]
ADR-0009  Test dosya formatı                       [değişmedi]
ADR-0010  CIRCT dialect seçimi                     [değişmedi]

YENİ (bu konsolidasyondan doğdu):
ADR-0011  Kontrat sistemi (requires/ensures/...)   [YENİ ★]
ADR-0012  SV çıktı öngörülebilirlik garantisi      [YENİ ★]
ADR-0013  Gramer sınıfı ve AI-üretilebilirlik      [YENİ]
ADR-0014  Kontrat kademeli benimseme               [YENİ]
ADR-0015  Determinizm garantisi kapsamı            [YENİ]
ADR-0016  Hook kontrat uyumu                       [YENİ]
ADR-0017  Modül sistemi ve paket semantiği         [YENİ ★]
ADR-0018  Lineer tipler (port güvenliği)           [YENİ ★]
ADR-0019  todo! semantiği                          [YENİ]
ADR-0020  Güvenlik akış denetimi (trust_level)     [YENİ]
ADR-0021  Artifact üretim mimarisi (Eksen 3)       [YENİ]
ADR-0022  SemVer donanım kuralları                 [YENİ]

★ = F1/F2 öncesi yazılmalı (kod yazmadan önce)
```

---

# BÖLÜM VIII — GÜNCELLENMİŞ CLAUDE.md EKLERİ

```markdown
## DOMAIN SİSTEMİ KURALLARI (GENİŞLETİLDİ)

domain bloğu üç alanı birden tanımlar:
  clock/frequency        → CDC kontrolü (E3001-E3002)
  reset_*                → RDC kontrolü (E3003-E3005)
  voltage/always_on/...  → PDC kontrolü (E3006-E3008)
  trust_level            → güvenlik akışı (E3009)

Bu birleşimi BOZMA. Ayrı @clock/@reset/@power anotasyonu ekleme.
Domain semantiği ADR-0002'de tanımlı — değiştirme.

## KONTRAT SİSTEMİ KURALLARI

Kontratlar ASLA zorunlu değildir.
Kontratsız modül de derlenmeli.
Kontrat eklendiğinde ilgili artifact üretimi aktifleşir.

Kontrat sözdizimi ADR-0011'de tanımlı.
Yeni kontrat türü ekleme (ADR gerektirir).

## ÇIKTI ÖNGÖRÜLEBİLİRLİK KURALI

Varsayılan build modunda:
  Volt modülü ↔ SV modülü 1:1 olmalı
  Sinyal isimleri korunmalı
  Bu kuralı bozan CIRCT optimizasyonu KAPALI olmalı

--optimize=aggressive modunda bu garanti geçersizdir.
Test: tests/ui/pass/*.volt → SV snapshot karşılaştırması

## todo! KURALLARI

todo! tip kontrolünden GEÇMELİ (tip biliniyor)
todo! ile volt build --release BAŞARISIZ olmalı
todo! ile volt build (debug) BAŞARILI olmalı
todo! simülasyonda o noktada durmalı
volt check → tüm todo! listesini göstermeli

## LİNEER TİP KURALLARI

&inv işaretli portlar tam bir kez tüketilmeli
Çift tüketim → E4003
Hiç tüketilmeme → E4004 (uyarı değil, hata)
Bu kural ADR-0018'de tanımlı.

## DETERMİNİZM KURALI

volt build --release çıktısı:
  Zaman damgası İÇERMEZ
  Mutlak yol İÇERMEZ
  Ortam değişkeni İÇERMEZ
  Paralel derleme sırası ÇIKTIYI ETKİLEMEZ

Test: aynı kaynak → iki build → bit-bit özdeş SV
```

---

# BÖLÜM IX — SİNERJİ HARİTASI

Bu özellikler bağımsız değil. En güçlü zincirler:

```
ZİNCİR 1 — "Tek Domain, Üç Kontrol, Beş Artifact"
  domain tanımı
    → CDC kontrolü (E3001)
    → RDC kontrolü (E3003)
    → PDC kontrolü (E3006)
    → SDC üretimi (clock groups)
    → UPF üretimi (power intent)
    → RDC raporu
  Tek kavram, altı fayda.

ZİNCİR 2 — "Kontrat → Sekiz Tüketici"
  requires/ensures/cover
    → Formal (SVA)
    → Testbench (cocotb)
    → Coverage grupları
    → Rust sürücü runtime kontrolü
    → Spec belgesi
    → Regresyon etki analizi
    → SemVer kırıcı değişiklik tespiti
    → Fiziksel geri bildirim eşlemesi

ZİNCİR 3 — "AI Geliştirme Döngüsü"
  LL(1) gramer     → LLM güvenilir üretim
  + todo!          → artımlı geliştirme
  + tip sistemi    → semantik hata yakalama
  + hata mesajı    → LLM öğrenme sinyali
  + kontrat        → niyet doğrulama
  = Endüstride %63 olan LLM assertion hata oranına karşı
    yapısal savunma

ZİNCİR 4 — "Determinizm → Güven"
  volt.lock
    → Yeniden üretilebilirlik
    → ISO 26262 tool qualification
    → Supply chain güvenliği
    → IP sertifikasyonu
    → SemVer doğrulaması

ZİNCİR 5 — "Tek Kaynak → Spec Uçurumu Kapanıyor"
  design.volt
    → RTL + Sürücü + Test + Formal + Belge + Kısıt
  Synopsys'in 30 yıldır aradığı "altın spec"
  → Volt'un yapısal cevabı
```

---

# BÖLÜM X — ÖZET: NE DEĞİŞTİ

```
KAPSAM DEĞİŞİKLİĞİ:
  MVP: 12 ay → 16 ay (dürüst tahmin)
  Neden: domain birleşimi + kontrat sistemi + lineer tipler

EN KRİTİK ÜÇ EKLENTİ:
  1. Domain = saat + sıfırlama + güç + güven (BİRLEŞİK)
     → Hiçbir rakipte yok, tek mekanizma dört fayda
  2. Kontrat sistemi (Eksen 2)
     → Spec uçurumunu yapısal olarak kapatıyor
  3. Lineer tipler
     → Sessiz hata sınıfını tamamen kapatıyor

EN KRİTİK ÜÇ ÇAKIŞMA ÇÖZÜMÜ:
  1. CIRCT optimizasyonu iki modlu (ECO uyumu korunuyor)
  2. Kontrat kademeli (yeni başlayan boğulmuyor)
  3. Trit katmanlı görünür (ezoterik algı önlendi)

YENİ ADR SAYISI: 12 (10 mevcut + 12 yeni = 22 toplam)
  Bunlardan 5'i F1/F2 öncesi yazılmalı (★ işaretli)

BU HAFTA YAPILACAK:
  ADR-0002 revizyonu: domain birleşimi
  ADR-0011: kontrat sistemi
  ADR-0017: modül sistemi
  ADR-0018: lineer tipler
  → Bu dördü olmadan F1'e başlanmamalı
```
