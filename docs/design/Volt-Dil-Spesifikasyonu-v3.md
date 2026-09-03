# Volt Dil Spesifikasyonu v3.0

> Bu belge Volt-HDL-Dil-Spesifikasyonu.md (v1) yerine geçer.
> Konsolide edilmiş 45 özelliği içerir.

---

## 1. Sözcüksel Yapı

### 1.1 Anahtar Kelimeler

```
Yapısal:      module fn pipeline fsm arbiter fifo ram regfile
              package import pub use extern
Bildirim:     let reg wire const type domain enum struct
Kontrol:      if else match for while on when
Kontrat:      requires ensures invariant cover assert assume
Zamanlama:    stage stall flush delay
Tip:          bool bits u8..u64 i8..i64 Trit Option
Özel:         todo! unsafe hook self Self
```

### 1.2 Sözdizimsel Kolaylıklar (Veryl'den)

```volt
// Trailing comma her yerde serbest
module Foo #(
    param WIDTH: u32 = 8,      // ← son virgül OK
) (
    in  data: bits<WIDTH>,     // ← son virgül OK
    out res : bits<WIDTH>,
) { }

// _ önekli değişken: kullanılmama uyarısı susar
let _unused_debug_signal = internal_state;

// Adlandırılmış blok sonlandırma (Arch'tan, opsiyonel)
module Counter {
    // ...
} module Counter    // ← isim tekrarı, LLM iç içe hatasını yakalar
```

### 1.3 Gramer Sınıfı

```
Volt grameri LL(1) hedefler, LL(2) tavanıdır.
Geri izleme ve belirsizlik YASAKTIR.

Her yapı benzersiz anahtar kelimeyle başlar:
  module, fn, pipeline, fsm, arbiter, fifo, ram, regfile,
  domain, package, requires, ensures, invariant, cover
  @budget, @timing, @dft, @debug_*, @mmio, @version

Bu AI-üretilebilirlik kontratının temelidir. (ADR-0013)
```

---

## 2. Domain Sistemi (Volt'un Kalbi)

### 2.1 Birleşik Domain Tanımı

```volt
domain SysDomain {
    // ─── SAAT ───
    clock          = posedge        // posedge | negedge | none
    frequency      = 100.mhz        // opsiyonel, SDC için
    jitter         = 50.ps          // opsiyonel

    // ─── SIFIRLAMA ───
    reset_sync     = sync           // sync | async | none
    reset_polarity = active_high    // active_high | active_low
    reset_cycles   = 4              // aktif kalma süresi
    reset_sequence = 1              // sıra (küçük = önce)
    reset_requires = [pll_locked]   // koşullu sıfırlama

    // ─── GÜÇ ───
    voltage        = 0.8.V
    always_on      = false
    retention      = true           // uyku modunda durum korunur
    isolation      = clamp_low      // clamp_low | clamp_high | latch
    power_sequence = 1

    // ─── GÜVENLİK ───
    trust_level    = confidential   // secret | confidential | public
}
```

### 2.2 Domain Anotasyonu

```volt
module CrossDomain {
    in  fast_data : u8 @FastDomain
    out slow_data : u8 @SlowDomain

    // Doğrudan bağlantı → derleme hatası (üç kontrol birden)
    // slow_data = fast_data
    //   E3001: saat alanı uyumsuzluğu
    //   E3003: sıfırlama alanı uyumsuzluğu
    //   E3006: güç alanı izolasyonsuz

    // Doğrusu: otomatik köprü
    slow_data = DomainBridge<u8, FastDomain, SlowDomain>(fast_data);
}
```

### 2.3 Reset Soyutlaması (Veryl'den)

```volt
module Counter {
    reg(SysDomain) count : u8 = 0;

    on SysDomain {
        if_reset {              // polarite/senkronluk domain'den gelir
            count <= 0;
        } else if enable {
            count <= count + 1;
        }
    }
}
// Aynı modül, farklı domain → farklı reset politikası
// Kod değişmez, davranış domain tanımından türer
```

### 2.4 Hata Kodları

```
E3001  Saat alanı uyumsuzluğu (CDC)
E3002  Tanımsız saat alanı
E3003  Sıfırlama alanı uyumsuzluğu (RDC)
E3004  Sıfırlama sekans ihlali
E3005  Koşullu sıfırlama karşılanmadı
E3006  Güç alanı geçişi izolasyonsuz (PDC)
E3007  Güç sekans ihlali
E3008  Retention gerektiren sinyal retention'sız alanda
E3009  Bilgi akışı ihlali (trust_level)
```

---

## 3. Tip Sistemi

### 3.1 Temel Tipler

```volt
bool                    // 1 bit
bits<N>                 // N bit, işaretsiz, aritmetiksiz
u8 u16 u32 u64          // işaretsiz tam sayı
i8 i16 i32 i64          // işaretli tam sayı
Trit                    // {-1, 0, +1}, opt-in import
Option<T>               // Some(T) | None, prelude'de
```

### 3.2 Prelude ve Opt-in

```volt
// Prelude (otomatik, import gerekmez):
bool, bits<N>, u8..u64, i8..i64, Option<T>, Result<T,E>

// Opt-in (açık import gerekli):
import volt::ternary::Trit;           // AI donanımı
import volt::fixed::FixedPoint;       // DSP
import volt::spike::Spike;            // nöromorfik (v2)
import volt::photonic::PTrit;         // fotonik (v3)
```

### 3.3 Option<T> Kullanımı (Spade'den)

```volt
module Fifo<T, const DEPTH: usize> {
    out data : Option<T>

    // Pattern matching zorunlu — "valid unutuldu" imkânsız
    match data {
        Some(value) => process(value),
        None        => idle(),
    }
}
```

### 3.4 Lineer Tipler (Spade'den)

```volt
// Ters tel: tam bir kez tüketilmeli
struct port MemReadPort<T> {
    addr : &inv u16,        // tüketici yazar
    data : &T,              // üretici yazar
}

module Top {
    let (p1, p2) = inst DualPortRam<u32>();
    inst compute_a(p1);
    inst compute_b(p1);   // E4003: p1 zaten tüketildi (satır 6)
}
```

```
E4001  Çift sürücü
E4002  Sürücüsüz çıkış portu
E4003  Lineer port çift tüketim
E4004  Lineer port tüketilmedi
```

### 3.5 Trit Semantiği

```volt
import volt::ternary::Trit;

// Aritmetik kuralları:
Trit * Trit  → Trit    // kapalı: {-1,0,1}×{-1,0,1} ⊆ {-1,0,1}
Trit * i8    → i8      // genişleme
Trit + Trit  → i2      // taşma mümkün (+1+1 = +2)

// Sentez uyarısı zorunlu:
// W2001: Trit FPGA hedefinde verimsiz sentezleniyor.
//        Verimlilik avantajı ASIC custom cell hedefindedir.
```

---

## 4. Kontrat Sistemi

### 4.1 Davranışsal Kontratlar

```volt
module Divider {
    requires:  divisor != 0
    ensures:   quotient * divisor + remainder == dividend
    ensures:   remainder < divisor
    invariant: !(busy && done)
    cover:     divisor == 1              // özel durum
    cover:     dividend == u32::MAX      // sınır durumu
}
```

### 4.2 Kaynak Kontratları

```volt
module NeuralAccel {
    @budget(lut = 50_000, ff = 20_000, bram = 200, dsp = 512)
    @budget(power = 5.W, area = 12.mm2)
    @budget(latency = 100.ns, throughput = 1.gops)
}
```

```
E6001  → docs/spec/cli-contract.md §17
W6002  Bütçenin %90'ı kullanıldı (uyarı)
```

### 4.3 Zamanlama Kontratları

```volt
module I2c {
    @timing(scl_high >= 600.ns) when speed == 0
    @timing(scl_low  >= 1300.ns) when speed == 0

    // Zamanlama istisnaları — KANITLANIR, iddia edilmez
    @false_path(from = config_reg, to = scl_gen)
    @multicycle(from = div_s1, to = div_s4, cycles = 4)
}
```

```
E6003, E6004  → docs/spec/cli-contract.md §17
```

### 4.4 DFT ve Debug Kontratları

```volt
module CriticalLogic {
    @dft(scan_chain = true, coverage_target = 0.99)
    @dft(bist = memory)

    @debug_visible(state, pc, sp)
    @debug_trace(depth = 1024, trigger = error_flag)
    @debug_breakpoint(when = pc == 0xDEAD)
}
```

```
W8001  Scan chain'e bağlanamayan düğüm (fault coverage düşüyor)
W8002  DFT coverage hedefi karşılanamıyor
```

### 4.5 Sürüm Kontratı

```volt
@version("2.1.0")
@abi_version(2)
module AxiInterface { }
```

```
E7001, E7002  → docs/spec/cli-contract.md §17
```

### 4.6 Kademeli Benimseme

```
Hiçbir kontrat zorunlu değildir.
Kontratsız modül tam olarak derlenir.

Seviye 1: kontrat yok           → RTL üretilir
Seviye 2: + invariant, cover    → + formal, coverage
Seviye 3: + requires, ensures   → + testbench, sürücü kontrolü
Seviye 4: + @budget, @timing    → + SDC, kaynak raporu
Seviye 5: + @dft, @debug        → + DFT script, debug altyapısı
```

---

## 5. todo! Mekanizması (Arch'tan)

```volt
module Cache {
    comb { mem_req.addr = req.addr; }
    comb { evict_way = todo!("LRU mu tree-PLRU mu?"); }

    ensures: todo!("politika kararlaştırılınca yazılacak")
}
```

```
Davranış:
  volt check          → todo! listesi gösterilir, hata değil
  volt build          → derlenir (debug modu)
  volt build --release → E9001: todo! ile release build yapılamaz
  volt sim            → todo! noktasında durur, mesaj gösterir
  LSP                 → todo! listesi panelde görünür
```

---

## 6. Hook Mekanizması (Arch'tan)

```volt
arbiter MemArbiter<const N: usize> {
    // Hook imzası kontrat içerir
    hook grant_select(req: bits<N>, last: bits<N>) -> bits<N>
        ensures: popcount(result) <= 1
        ensures: result & req == result;

    // Yapının kendi invariantları korunur
    invariant: valid → popcount(grant) == 1
}

// Kullanıcı politikası
fn my_aging_policy(req: bits<4>, last: bits<4>) -> bits<4> {
    // ensures kontratını sağlamalı, aksi halde E5002
}

let arb = MemArbiter<4> { grant_select = my_aging_policy };
```

---

## 7. Pipeline ve Zamanlama

### 7.1 Üç Seviye

```volt
// L0 — Otomatik hizalama (varsayılan)
module Simple {
    let a = compute_a(x);       // 2 döngü
    let b = compute_b(y);       // 1 döngü
    let c = a + b;              // derleyici hizalar
}

// L1 — Açık gecikme
module Explicit {
    let a : Delayed<u32, 2> = compute_a(x);
    let b : Delayed<u32, 1> = compute_b(y);
    let c = a + delay<1>(b);    // açık hizalama
}

// L2 — Timeline tipler (Filament'ten)
#[timeline]
module Multiplier {
    in  a : u8  @['G+[0,1))
    in  b : u8  @['G+[0,1))
    out p : u16 @['G+[3,4))     // 3 döngü sonra hazır

    // Kaynak çakışması analizi:
    // Aynı çarpanı iki farklı zamanda kullanma → E5003
}
```

### 7.2 Pipeline Stage Referansları (Spade'den)

```volt
pipeline(5) Cpu {
    stage Fetch    { ... }
    stage Decode   { ... }
    stage Execute  { let alu_out = alu(a, b); }
    stage Memory   { ... }
    stage Writeback{ ... }

    // İleri aşamalara referans — veri yönlendirme
    let operand_a = if stage(Execute).dest == src_a {
        stage(Execute).alu_out
    } else if stage(Memory).dest == src_a {
        stage(Memory).result
    } else {
        regfile.read(src_a)
    };

    stall when memory_busy;
    flush Fetch, Decode when branch_mispredict;
}
```

### 7.3 Auto-Pipelining (Clash'ten)

```volt
// Derleyici gecikmeleri tip bilgisinden çıkarır
let product = mul(a, b);          // 3 döngü (tipten bilinir)
let sum = product + delay_auto(c); // c otomatik 3 gecikir
```

---

## 8. Modül Sistemi

```volt
// src/soc/interconnect.volt
package soc::interconnect;

pub module AxiCrossbar<const N: usize> { }
     module InternalArbiter { }             // paket-içi

pub use soc::interconnect::AxiCrossbar;

// src/top.volt
import soc::interconnect::AxiCrossbar;
import soc::peripherals::{Uart, Spi, I2c};
```

```
Görünürlük:
  pub          → paket dışına açık
  (varsayılan) → sadece paket içi

Ayrık derleme:
  Arayüz hash'i (portlar + kontratlar) değişmediyse
  bağımlı modüller yeniden derlenmez
```

---

## 9. MMIO ve HW-SW Köprüsü

```volt
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module I2cRegisters {
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        enable : bool,                   // [0]
        reset  : bool @self_clearing,    // [1]
        speed  : bits<2>,                // [3:2]
        @reserved : bits<28>,
    }

    @reg(offset = 0x04, access = ReadOnly, volatile)
    status : {
        busy      : bool,
        ack_error : bool @w1c,           // write-1-to-clear
        rx_count  : bits<8>,
    }
}
```

Üretilenler: `driver.rs`, `driver.h`, `regmap.json`, `regmap.md`

---

## 10. Yapı Sistemi (Volt.toml)

```toml
[package]
name    = "my_soc"
version = "1.2.0"
edition = "2026"
license = "Apache-2.0 OR MIT"

[dependencies]
stdlib-axi  = "2.1"
risc-v-core = { git = "https://github.com/...", tag = "v0.9" }
local-ip    = { path = "../shared-ip" }

[build]
target      = "asic-tsmc7"      # veya "fpga-xilinx-artix7"
optimize    = "structure"       # structure | aggressive
top         = "my_soc::Top"

[artifacts]
rtl         = true
driver      = ["rust", "c"]
testbench   = "cocotb"
formal      = "symbiyosys"
constraints = ["sdc", "xdc"]
power       = "upf"
docs        = true

[domains.SysDomain]
frequency   = "100MHz"
voltage     = "0.8V"

[format]
indent      = 4
max_width   = 100
```

---

## 11. Araç Komutları

```bash
volt new <isim>              # yeni proje
volt build                   # artımlı derleme
volt build --release         # deterministik, volt.lock üretir
volt build --all             # tüm artifact'lar
volt check                   # tip + CDC/RDC/PDC + todo! listesi
volt test                    # testbench çalıştır
volt formal --depth 20       # SymbiYosys
volt sim --interactive       # REPL (Clash'ten)
volt fmt                     # biçimlendirme
volt doc                     # spec belgesi
volt diff v1.0.0 v1.1.0      # anlamsal fark + SemVer kontrolü
volt analyze --rdc           # RDC raporu
volt analyze --impact <file> # regresyon etki analizi
volt feedback --from-sta <f> # fiziksel geri bildirim
volt verify --reproducible   # determinizm kanıtı
volt add <paket>             # bağımlılık ekle
volt publish                 # registry'ye yayımla
volt migrate <file.v>        # Verilog → Volt (kısmi)
```

---

## 12. Hata Kodu Kataloğu (Tam)

```
E0xxx  Sözdizimi
  E0001  Beklenmeyen token
  E0002  Eksik kapanış
  E0003  Bilinmeyen anahtar kelime
  E0004  Blok sonlandırma ismi uyuşmuyor

E1xxx  İsim çözümleme
  E1001  Tanımsız isim
  E1002  Çift tanım
  E1003  Özel modüle erişim (pub değil)
  E1004  Döngüsel paket bağımlılığı

E2xxx  Tip
  E2001  Bit genişliği uyumsuzluğu
  E2002  İşaret uyumsuzluğu
  E2003  Tip uyumsuzluğu
  E2004  Option<T> match tam değil

E3xxx  Domain (CDC + RDC + PDC + Güvenlik)
  E3001  Saat alanı uyumsuzluğu
  E3002  Tanımsız saat alanı
  E3003  Sıfırlama alanı uyumsuzluğu
  E3004  Sıfırlama sekans ihlali
  E3005  Koşullu sıfırlama karşılanmadı
  E3006  Güç alanı izolasyonsuz
  E3007  Güç sekans ihlali
  E3008  Retention eksik
  E3009  Bilgi akışı ihlali (trust_level)

E4xxx  Sürücü ve Lineer Tipler
  E4001  Çift sürücü
  E4002  Sürücüsüz çıkış
  E4003  Lineer port çift tüketim
  E4004  Lineer port tüketilmedi

E5xxx  Kontrat
  E5001  Kontrat ihlali (formal kanıtladı)
  E5002  Hook kontratı ihlal ediyor
  E5003  Timeline kaynak çakışması

E6xxx  Bütçe ve Zamanlama → docs/spec/cli-contract.md §17
E7xxx  Sürüm              → docs/spec/cli-contract.md §17
E9xxx  Yapı               → docs/spec/cli-contract.md §17

W1xxx  Genel uyarılar
  W1001  Kullanılmayan sinyal (_ öneki ile susar)

W2xxx  Trit
  W2001  Trit FPGA'da verimsiz
  W2002  Trit aritmetik taşma riski

W6xxx  Bütçe
  W6002  Bütçe %90 doldu

W8xxx  DFT
  W8001  Scan chain'e bağlanamayan düğüm
  W8002  DFT coverage hedefi risk altında
```
