> UYARI: Bu belge geçersizdir.
> Güncel sürüm: docs/design/ altında.

# Volt Ekosistemi — Sinerjik Özellik ve Modüller

> Sinerji = diğer bileşenlerin değerini çarpan olarak artıran özellik.
> Her iyi özellik sinerjik değil. Bu belge hangilerinin gerçekten
> çarpan etkisi yarattığını, hangilerinin "iyi ama" olduğunu ayırt ediyor.

---

## Bölüm 1 — Sinerji Nedir, Neden Önemli

```
Normal eklenti:
  Özellik A + Özellik B = A + B değeri

Sinerjik eklenti:
  Özellik A + Özellik B = A × B değeri

Örnek:
  Volt tip sistemi + LSP = tip + editör (toplam)
  Volt tip sistemi + AI asistan = tip × AI
    (AI hataları derleme zamanında yakalanıyor
     → AI daha güvenilir kod üretiyor
     → güvenilir kod tip sistemini daha değerli kılıyor
     → döngü kendini besliyor)

Bu belge çarpan olanları önce listeliyor.
```

---

## Kategori 1 — Yüksek Sinerji (Çarpan Etkisi)

### 1.1 AI Destekli Tasarım (LLM + Volt Tip Sistemi)

**Neden sinerjik:**

```
Diğer HDL + LLM:
  LLM Verilog üretir
  → Hata sessizce geçer
  → Simülasyonda veya silisyumda ortaya çıkar
  → Güvenilmez

Volt + LLM:
  LLM Volt kodu üretir
  → Tip sistemi hatayı anında yakalar
  → LLM hata mesajını görür
  → Düzeltir
  → Döngü: her iterasyon daha güvenilir

  Ayrıca: Volt'un düzenli grameri ve güçlü tipleri
  LLM için diğer HDL'den daha az belirsiz
  → Daha iyi kod üretimi baştan
```

**Nasıl görünür:**

```volt
// Kullanıcı: "8 bitlik giriş için 2-to-1 mux yap"

// LLM üretiyor:
module Mux2to1 {
    in  sel : bool
    in  a   : u8
    in  b   : u8
    out out : u8

    out = if sel { a } else { b }
}
// → Derlendi, doğru ✓

// Kullanıcı: "Bunu iki farklı saat için yap"

// LLM üretiyor (hatalı):
module Mux2to1_CDC {
    in  sel : bool @Fast
    in  a   : u8   @Fast
    in  b   : u8   @Slow   // ← hata: farklı alan
    out out : u8   @Fast
    out = if sel { a } else { b }  // ← Slow → Fast direkt
}
// → Derleme hatası:
// E0401: sel @Fast ve b @Slow karıştırılamaz.
// LLM hatayı görür → CDC köprüsü ekler → doğru

// Diğer HDL'de: hata sessizce geçer
```

**Gereklilik:** Yüksek — "AI donanım geliştirme" trendi hızlı büyüyor.
Volt bu trende en iyi konumlanmış HDL olabilir.

---

### 1.2 Donanım-Yazılım Köprüsü

**Neden sinerjik:**

```
Bugün donanım-yazılım geliştirme:
  Donanım ekibi: Verilog/SV + Excel'de register haritası
  Yazılım ekibi: C/Rust + elle yazılmış sürücü
  → İki kaynak → senkronizasyon hatası → sessiz hata

Volt + otomatik sürücü üretimi:
  register_map myip.volt
  → volt codegen --target rust → myip_driver.rs
  → volt codegen --target c   → myip_driver.h

  Tek kaynak: Volt tanımı
  İki çıktı: donanım (SV) + yazılım (Rust/C sürücü)
  Senkronizasyon hatası: imkânsız
```

**Nasıl görünür:**

```volt
// Volt'ta register tanımı
@mmio(base=0x4000_0000)
module MyIPRegisters {
    @reg(offset=0x00, access=ReadWrite)
    control : {
        enable   : bool,      // bit 0
        reset    : bool,      // bit 1
        mode     : bits<2>,   // bit 3:2
    }

    @reg(offset=0x04, access=ReadOnly)
    status : {
        ready  : bool,
        error  : bool,
        count  : u16,
    }
}
```

```rust
// Otomatik üretilen Rust sürücüsü:
// volt codegen --target rust myip.volt

pub struct MyIP {
    base: *mut u32,
}

impl MyIP {
    pub fn set_enable(&mut self, val: bool) {
        // tip-güvenli, offset otomatik, mask otomatik
        unsafe { write_volatile(self.base, ...) }
    }
    pub fn get_status(&self) -> Status {
        // sadece ReadOnly alanlar burada
    }
    // set_status yok! ReadOnly = derleyici hatası
}
```

**Gereklilik:** Çok yüksek — donanım-yazılım ayrımı her projede var.
Bu köprü olmadan Volt tek taraflı araç kalıyor.

---

### 1.3 Güvenlik Özelliği Denetimi

**Neden sinerjik:**

```
Volt'un tip sistemi: hangi sinyalin hangi alanda olduğunu biliyor
Bu bilgi güvenlik analizine doğrudan uygulanabilir:

  domain Secure { /* gizli veri */ }
  domain Public  { /* açık veri */ }

  // Gizli → Açık akış: izin verilmemeli
  secret_key @Secure → public_output @Public
  → Derleme hatası (bilgi akış güvenliği)

Bu bugün hiçbir HDL'de yok.
Volt'un type system'ı bunu eklemek için mükemmel zemin.
```

**Nasıl görünür:**

```volt
// Güvenlik domain'leri
domain Secure { trust = high }
domain Public  { trust = low  }

module AES_Core {
    in  key        : bits<128> @Secure   // asla dışarı çıkmamalı
    in  plaintext  : bits<128> @Public
    out ciphertext : bits<128> @Public

    // Bu derleme hatası:
    // out leaked_key : bits<128> @Public = key
    // E0501: @Secure veri @Public alana akamaz

    // Yan kanal: güç tüketimi analizi (v2)
    @no_power_leak: key  // güç örüntüsü key'i sızdırmamalı
}
```

**Gereklilik:** Orta-Yüksek — güvenlik çipi pazarı büyüyor.
Donanım güvenlik mühendisi için Volt'u vazgeçilmez yapacak özellik.

---

### 1.4 Zamanlama Kısıtı Otomatik Üretimi

**Neden sinerjik:**

```
Volt L0/L1/L2 zamanlama tipleri pipeline derinliğini biliyor.
Bu bilgi → SDC (Synopsys Design Constraints) otomatik:

  pipeline 4-aşamalı → set_multicycle_path 4
  cross-domain       → set_clock_groups -async
  registered output  → set_output_delay

Bugün: mühendis Volt tasarımına bakıp SDC el ile yazıyor
       → hata kaynağı, senkronizasyon sorunu

Volt ile: tasarımdan SDC otomatik türetilir
          → tasarım değişince SDC otomatik güncellenir
```

**Nasıl görünür:**

```bash
$ volt build cpu.volt --emit-sdc

# Otomatik üretilen cpu.sdc:
create_clock [get_ports clk] -period 5.0
set_clock_groups -async \
  -group [get_clocks clk_fast] \
  -group [get_clocks clk_slow]
# ^ CDC geçişleri Volt tipinden türetildi

set_multicycle_path 4 \
  -from [get_cells fetch_stage] \
  -to   [get_cells writeback_stage]
# ^ Pipeline derinliği L1 tipinden türetildi
```

**Gereklilik:** Yüksek — ASIC hedefleyen herkes SDC yazar.
Hatalı SDC = yanlış silisyum = milyonlarca dolar.

---

## Kategori 2 — Önemli Ama Daha Az Sinerjik

### 2.1 Anlamsal Sürüm Kontrolü (volt diff)

```
Bugün git diff donanım için:
  - wire [7:0] data;
  + reg  [7:0] data;
  → "Bir değişkenin tipi değişti" — ama ne anlama geliyor?

volt diff anlamsal:
  $ volt diff v1.volt v2.volt

  [DEĞİŞTİ] data: wire u8 → reg(Sys) u8
    Etki: data artık kayıtlı → bir döngü gecikme
    CDC etkilenen sinyaller: downstream_bus
    Regresyon riski: ORTA

  [EKLENDİ] FastDomain → SlowDomain köprüsü
    Önceki: doğrudan bağlantı (CDC hatası vardı!)
    Şimdi: iki flip-flop senkronizatör

Gereklilik: Orta — tasarım incelemesini güçlendirir.
```

### 2.2 Kapsam Analizi (Coverage)

```
Simülasyon sonrası:
  "Hangi assert dalları test edilmedi?"
  "Hangi FSM durumuna hiç girilmedi?"

volt coverage report mydesign:
  FSM:
    STATE_IDLE:   %100 test edildi
    STATE_BUSY:   %100 test edildi
    STATE_ERROR:   %0  ← test yok! ← uyarı

  Assertions:
    assert overflow != 0: HIÇZAMAN tetiklenmedi ← uyarı

  Otomatik ek test önerisi:
  "Bu giriş kombinasyonu STATE_ERROR'u tetikler:
   data=0xFF, enable=1, reset=0"

Gereklilik: Orta-Yüksek — doğrulama kalitesi kritik.
```

### 2.3 Fiziksel Tasarım Geri Bildirimi

```
Bugünkü döngü:
  Volt → SV → Sentez → P&R → Zamanlama ihlali → El ile düzelt

Volt 2.0 döngüsü:
  Volt → SV → Sentez → P&R → Zamanlama ihlali
  → volt retiming-hint mydesign.volt
  
  "critical_path: ALU → MUX → Register (8.2ns, limit 5ns)"
  Öneri: register araya ekle veya
         ALU'yu pipeline et (Volt'ta: Delayed<T, 1>)
  
  Kullanıcı Volt'ta düzeltir → tekrar P&R

Gereklilik: Orta — ileride değerli, MVP'de değil.
```

---

## Kategori 3 — Alan-Özgü Kütüphaneler

### 3.1 Kriptografi Kütüphanesi

```volt
// Volt-native kriptografi — güvenlik tipleri dahil

import volt::crypto::aes

module SecureChannel {
    // AES-256-GCM — tip sistemi güvenliği garanti
    let aes = AES256_GCM(key @Secure)

    // key asla @Public alana çıkamaz — tip garantisi
    // timing side-channel: sabit zamanlı implementasyon
    // @constant_time anotasyonu zorunlu

    @constant_time
    fn encrypt(plaintext: bits<128> @Public) -> bits<128> @Public {
        aes.encrypt(plaintext)
    }
}
```

**Değeri:** Güvenlik çipi pazarı için kritik, genel kullanıcı için opsiyonel.
**Gereklilik:** Orta — v2 için.

### 3.2 DSP Kütüphanesi

```volt
// Sabit nokta hassasiyet takibi

import volt::dsp::fixed_point

// Tip: bit genişliği + kesir bitleri + taşma davranışı
type Q15 = FixedPoint<16, 15, Saturate>

module FIR_Filter<const N: usize> {
    in  x       : Q15
    in  coeffs  : [Q15; N]
    out y       : Q15

    // Akümülatör otomatik genişler (taşma yok)
    let acc : FixedPoint<32, 15, Saturate> = sum(
        for i in 0..N { coeffs[i] * delay(x, i) }
    )

    // Geri daral — Volt hassasiyet kaybını uyarır
    y = acc.truncate::<Q15>()
    // W0201: 16 bit hassasiyet kaybı (32→16)
}
```

**Gereklilik:** Orta — DSP kullanıcıları için değerli, genel için değil.

### 3.3 Ağ/İletişim Protokol Kütüphanesi

```volt
// Ethernet, PCIe, USB — tip-güvenli paket işleme

import volt::net::ethernet

module EthernetMAC {
    in  rx_stream : Stream<EthernetFrame>
    out tx_stream : Stream<EthernetFrame>

    // Frame boyutu derleme zamanı kontrol
    // CRC hesaplama otomatik
    // Pause frame yönetimi dahil
}
```

**Gereklilik:** Yüksek — ağ donanımı çok yaygın.

---

## Kategori 4 — Ekosistem Altyapısı

### 4.1 Paket Kayıt Sunucusu (Registry)

```
Bugün npm / PyPI / crates.io ne yapıyor:
  tek komutla IP ekle: volt add stdlib-axi@2.1

Volt registry olmadan:
  "IP buldum, elle kopyaladım, lisans belirsiz,
   sürüm takibi yok, güncelleme manuel"
  → Kullanıcı Volt'tan soğur

volt.toml:
  [dependencies]
  stdlib-axi   = "2.1"
  pcie-gen5    = "1.0"
  risc-v-core  = "0.9"
  ternary-npu  = "0.3"

$ volt add pcie-gen5
  Yükleniyor: pcie-gen5@1.0.2...
  Tip kontrolü: uyumlu ✓
  CDC analizi: 3 yeni alan algılandı, uyumlu ✓

Gereklilik: KRİTİK — olmadan ekosistem büyümez.
```

### 4.2 IP Sertifikasyon Sistemi

```
"Volt Certified" rozeti:
  □ Tüm portlar tip-güvenli
  □ CDC geçişleri doğrulanmış
  □ Formal özellikler yazılmış ve kanıtlanmış
  □ Simülasyon kapsamı > %90
  □ Belgeleme mevcut

Kurumsal alım için kritik:
  "Bu IP Volt Certified → güvenle kullanabiliriz"
  Bugün: "Bu IP'nin kalitesi nasıl?" → belirsiz

Gereklilik: Yüksek (v1 sonrası).
```

---

## Kategori 5 — Uzak Vade Sinerjiler

### 5.1 Doğrulama Yapay Zekası

```
Fikir: Volt tasarımı + simülasyon tarihini LLM'e ver
       → Otomatik hata hipotezi üret

$ volt ai-debug failing_test.volt

Analiz ediliyor...
  Test başarısız: cycle 847'de output = 0xAB (beklenen 0x00)

  Olası neden 1 (güven: %78):
    CDC geçişi cycle 843'te kararsız durum
    → TwoFlop yerine ThreeFlop dene

  Olası neden 2 (güven: %15):
    FIFO doluluk mantığında yarış koşulu

  Otomatik düzeltme önerisi: [kod]

Gereklilik: Uzak vade — LLM yetkinliği artınca.
```

### 5.2 Donanım Testi Otomatik Üretimi

```
Formal counterexample → simülasyon stimulusu:

  $ volt verify --depth 50 mydesign.volt
  Özellik ihlali bulundu!
  Karşı örnek: input=[0xFF, 0x00] @ cycle 23

  $ volt generate-test --from-counterexample
  // Otomatik üretilen test:
  test "formal_counterexample_001" {
      apply input 0xFF, 0x00
      tick 23
      assert output != ILLEGAL_STATE
  }

Sinerji: formal → test → kapsam → formal döngüsü kapanıyor
Gereklilik: Orta — v2 için değerli.
```

---

## Öncelik ve Zaman Çizelgesi

```
MVP (Yıl 1):
  ★★★★★ Paket registry               → ekosistem büyümesi
  ★★★★★ HW-SW köprüsü (codegen)      → kapatılan büyük boşluk
  ★★★★☆ SDC otomatik üretimi          → ASIC akışı kolaylaşır

V1 (Yıl 2):
  ★★★★★ AI destekli tasarım           → topluluk çekimi
  ★★★★☆ Kapsam analizi                → doğrulama kalitesi
  ★★★★☆ Ağ/iletişim kütüphanesi       → yaygın kullanım
  ★★★☆☆ IP sertifikasyon              → kurumsal güven

V2 (Yıl 3-4):
  ★★★★☆ Güvenlik özellik denetimi     → farklılaşma
  ★★★☆☆ Kriptografi kütüphanesi       → güvenlik çipi
  ★★★☆☆ DSP kütüphanesi               → sinyal işleme
  ★★★☆☆ Anlamsal versiyon kontrolü    → tasarım inceleme

Uzak Vade (Yıl 4+):
  ★★★☆☆ Fiziksel tasarım geri bildirimi
  ★★★☆☆ Doğrulama yapay zekası
  ★★☆☆☆ Donanım testi otomatik üretimi
```

---

## En Kritik Eklenti: HW-SW Köprüsü

Tüm listede en sinerjik tek eklenti bu:

```
Neden en kritik:

1. Her donanım projesinde yazılım da var
   → Bugün iki ayrı ekip, iki ayrı araç, senkronizasyon hatası

2. Volt bunu doğal olarak çözebilir:
   Register haritası Volt'ta → hem SV hem Rust/C üret
   Tek kaynak → iki çıktı → hata imkânsız

3. Yazılım geliştiriciyi çeker:
   "Donanımı anlamadan Volt register'ını güvenle kullanıyorum"
   → Volt kullanıcı tabanını genişletir (sadece HW değil)

4. Şirketlerde satın alma kararını değiştirir:
   "Volt'a geçersek donanım-yazılım entegrasyon hatalarımız azalır"
   → Somut ölçülebilir değer → ticari lisans gerekçesi

Bu özellik olmadan Volt tek taraflı araç.
Bu özellik ile Volt ekip aracı.
```

---

## Özet

```
Sinerjik mi?    Özellik                    Ne Zaman
────────────────────────────────────────────────────────
★★★★★ (kritik) Paket registry             MVP
★★★★★ (kritik) HW-SW köprüsü             MVP
★★★★★ (yüksek) AI destekli tasarım        V1
★★★★☆ (yüksek) SDC otomatik üretimi       MVP
★★★★☆ (yüksek) Güvenlik akış denetimi     V2
★★★★☆ (yüksek) Kapsam analizi             V1
★★★☆☆ (orta)   Ağ/DSP/Kriptografi lib    V1-V2
★★★☆☆ (orta)   Anlamsal diff              V2
★★☆☆☆ (düşük)  Fiziksel geri bildirim    V3+
★★☆☆☆ (düşük)  Doğrulama yapay zekası    V3+

Genel kural:
  Volt'un tip sistemini daha değerli kılan özellik → sinerjik
  Volt'un yanında çalışan bağımsız özellik → faydalı ama daha az sinerjik

En büyük fırsatı kısaca:
  "AI donanım üretir, Volt tip sistemi onu güvenli kılar"
  Bu döngü henüz hiçbir araçta yok.
  Volt bunu ilk yapan olabilir.
```
