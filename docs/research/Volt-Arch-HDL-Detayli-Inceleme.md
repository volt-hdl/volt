> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Arch HDL — Detaylı İnceleme Raporu

> arxiv:2604.05983 — Nisan 2026
> Shuqing Zhao (arch.hdl.lang@gmail.com)
> Makalenin tamamı okunarak hazırlanmıştır.

---

## Bölüm 1 — Temel Kimlik

```
Adı:    Arch (AI-native Register-transfer Clocked Hardware)
Yazar:  Shuqing Zhao — TEK kişi
Tarih:  Nisan 2026 (3 ay önce!)
Lisans: CC BY-SA 4.0 (makale), GitHub: arch-hdl-lang/arch-com
Dil:    Bağımsız (host dil yok)
Çıktı:  IEEE 1800-2017 SystemVerilog
Durum:  Çalışan derleyici + 2 büyük vaka çalışması
```

**Kritik bilgi:** Tek bir kişinin 3 ay önce yayımladığı,
çalışan uygulamalı bir proje. Akademik değil, pratik odaklı.

---

## Bölüm 2 — Tasarım Felsefesi (5 İlke)

<cite index="77-1">Arch beş ilke üzerine kurulu, her biri dil seviyesinde uygulanıyor:</cite>

```
1. No Baggage (Yük Yok):
   Host dil runtime'ı yok. Her anahtar kelime
   doğrudan donanım yapısına karşılık geliyor.
   Operatör aşırı yükleme arkasında gizli kütüphane yok.

2. Strong Types (Güçlü Tipler):
   Bit genişliği, saat alanı, port yönü, sinyal sahipliği
   statik olarak izleniyor. Her uyumsuzluk derleme hatası.

3. Micro-Architecture First:
   Pipeline, FSM, FIFO, Arbiter, RegFile birinci sınıf
   anahtar kelimeler — kütüphane deseni değil.

4. AI-Generatable:
   LL(1) gramer, tek tip şema, adlandırılmış blok
   sonlandırmaları → LLM fine-tuning olmadan doğru Arch üretir.

5. Predictable RTL:
   Bir Arch yapısı her zaman aynı SV yapısını üretiyor.
   Tasarımcı her satırı denetleyebilir.
```

---

## Bölüm 3 — Tip Sistemi: Volt ile Karşılaştırma

### 3.1 Clock ve Reset Birinci Sınıf Tip

<cite index="77-1">Arch'ta her saat Clock<D> olarak tanımlanır — D fantom alan parametresi, frekans/kaynak alanını isimlendirir (ör. SysDomain, UsbDomain). Her reset Reset<S,P,D?> olarak tanımlanır — eşzamanlılık (Sync/Async), polarite (High/Low) ve opsiyonel reset alan etiketi taşır.</cite>

```
Arch:
  port clk: in Clock<SysDomain>;
  port rst: in Reset<Sync, High>;
  port usb_clk: in Clock<UsbDomain>;

  // Hata: SysDomain sinyali UsbDomain register'ına
  // doğrudan atanamaz → derleme hatası

Volt (planlanan):
  domain Sys { clock = posedge, reset = sync active_high }
  domain USB { ... }
  
  in  data : u8 @Sys
  // @USB alana direkt bağlantı → derleme hatası

  Fark: Arch Clock<D> sinyale bağlı tip
        Volt @Domain anotasyon tabanlı
        İkisi de aynı problemi çözüyor, farklı sözdizimi
```

### 3.2 Arch'ın CDC Yaklaşımının Sınırlılıkları

<cite index="77-1">Derleyici şu an eksik veya hatalı senkronizörleri bireysel sinyaller için yakalıyor, ancak üretim CDC araçlarının sağladığı daha gelişmiş kontrolleri henüz uygulamıyor — özellikle yeniden yakınsama analizi (birden fazla doğru senkronize sinyal aşağı akışta birleştiğinde bit tutarlılığını kaybetmesini tespit etme) ve gri kodlu çok bitli tutarlılık kanıtları. Karmaşık CDC topolojili tasarımlar için Arch'ın statik kontrollerinin ticari bir CDC aracıyla tamamlanması gerekiyor.</cite>

```
Bu önemli bir kabul:
  Arch CDC: bireysel sinyal kontrolü ✓
  Arch CDC: yeniden yakınsama analizi ✗ (eksik)
  Arch CDC: çok bitli gray-code tutarlılık ✗ (eksik)

Volt'un fırsatı: bu eksik kısımlar formal tip sistemi
ile daha kapsamlı çözülebilir (K-framework + tip)
```

### 3.3 Dört Boyutlu Tip Kontrolü

<cite index="77-1">Arch tip sistemi dört bağımsız güvenlik boyutunu eş zamanlı uygular. Dördünü de karşılayan bir sinyal doğruluk garantisiyle inşa edilmiş sayılır:</cite>

```
Boyut 1: Bit genişliği güvenliği
  let a: UInt<8> = 255;
  let b: UInt<16> = a.zext<16>(); // açık dönüşüm
  // let c: UInt<16> = a; → HATA: genişlik uyumsuzluğu

Boyut 2: Saat alanı izleme
  Clock<D> etiketli her sinyal
  Çapraz alan atama → derleme hatası

Boyut 3: Port yön güvenliği
  in/out yön takibi
  Ters bağlantı → derleme hatası

Boyut 4: Tek sürücü kuralı
  Her sinyal tam bir sürücü
  Çok sürücü → derleme hatası
  → Simülasyon zamanı belirsizliği ortadan kalkar
  → Paralel simülasyon kilit gerektirmiyor!
```

### 3.4 Örtülü Latch Önleme

<cite index="77-1">Derleyici, comb bloğundaki her sinyalin tüm kontrol yollarında atandığını doğruluyor. Eksik else dalı veya tamamlanmamış match → derleme hatası. Kasıtlı latch için açık latch on ENABLE yapısı gerekiyor.</cite>

---

## Bölüm 4 — Birinci Sınıf Mikro-Mimari Yapılar

Bu Arch'ın en güçlü farkılaşma noktası.

### 4.1 Pipeline

```arch
pipeline IntPipe
  param WIDTH: const = 32;
  port clk: in Clock<SysDomain>;
  port rst: in Reset<Sync>;
  port data_in: in UInt<WIDTH>;
  port result: out UInt<WIDTH>;

  stage Fetch
    reg instr: UInt<WIDTH> reset rst=>0;
    seq on clk rising
      instr <= data_in;
    end seq
  end stage Fetch

  stage Execute
    reg out_val: UInt<WIDTH> reset rst=>0;
    seq on clk rising
      out_val <= Fetch.instr + 1;
    end seq
  end stage Execute

  stall when Execute.out_val == 0;
  flush Fetch when branch_mispred;
  comb result = Execute.out_val;
end pipeline IntPipe
```

<cite index="77-1">Derleyici: (1) aşamalar arası pipeline register'ları ekler, (2) stall when ifade ettiğinde geri yayılan stall sinyalleri üretir, (3) flush yönergeleri tetiklendiğinde flush maskeleri oluşturur, (4) pipeline doluluk için aşama başına valid_r registerları izler.</cite>

### 4.2 FSM

```arch
fsm Controller
  port clk: in Clock<SysDomain>;
  port rst: in Reset<Sync>;

  default state Idle;

  default
    comb
      busy = false;
      done = false;
    end comb
  end default

  state Idle
    -> Active when start;
  end state Idle

  state Active
    let busy = true;
    -> Done when count_done;
  end state Active

  state Done
    let done = true;
    -> Idle;
  end state Done
end fsm Controller
```

<cite index="77-1">Derleyici tek-sıcak veya ikili durum kodlaması üretir (param veya derleyici bayrağıyla seçilir) ve geçiş case mantığını oluşturur. Bir döngüde hiçbir geçiş ateşlenmezse FSM mevcut durumda kalır.</cite>

### 4.3 FIFO

```arch
fifo AsyncBuf
  param DEPTH: const = 16;
  param TYPE: type = UInt<32>;
  port clk_wr: in Clock<WriteDomain>;
  port clk_rd: in Clock<ReadDomain>;
  port push_valid: in Bool;
  port push_ready: out Bool;
  port push_data: in TYPE;
  port pop_valid: out Bool;
  port pop_ready: in Bool;
  port pop_data: out TYPE;
end fifo AsyncBuf
```

İki farklı saat alanı → derleyici otomatik gray-code CDC ekliyor.

### 4.4 Hook Mekanizması (Volt'ta Yok)

Bu Arch'ın özgün katkılarından biri:

```arch
// Özel arbiter politikası
function MyGrantFn(req_mask: UInt<4>, ...) -> UInt<4>
  // ... mantık
end function MyGrantFn

arbiter CustomArb
  policy MyGrantFn;
  hook grant_select(req_mask: UInt<4>, ...) -> UInt<4>
    = MyGrantFn(req_mask, last_grant, extra_port);
end arbiter CustomArb
```

<cite index="77-1">Hook'lar, tasarımcının derleyici tarafından üretilen kontrol yapılarına özel kombinasyonel mantık eklemesine olanak tanır — yapının güvenlik garantilerinden vazgeçmeden.</cite>

### 4.5 Template (Trait/Interface)

```arch
template Arbiter
  param NUM_REQ: const;
  port clk: in Clock<SysDomain>;
  port grant_valid: out Bool;
  hook grant_select(req_mask: UInt<4>) -> UInt<4>;
end template Arbiter

module MyArbiter implements Arbiter
  // ... zorunlu alanları uygula
end module MyArbiter
```

---

## Bölüm 5 — AI-Generatability Contract (Volt'tan Farkı)

Bu Arch'ın en özgün tasarım kararı ve Volt'ta bulunmayan bir katman.

### 5.1 LL(1) Gramer

<cite index="77-1">Arch grameri kesinlikle LL(1): ayrıştırma sırasında her noktada, sonraki tek token hangi üretim kuralının uygulanacağını açıkça belirler. Geri izleme yok, çok token ileri bakış yok, bağlam bağımlı ayrıştırma yok.</cite>

```
SV: belirsiz gramer, GLR/backtracking parser gerekiyor
Arch: LL(1), her token bir üretim kuralına → AI için ideal

AI avantajları:
1. Sözdizimi tuzağı yok — her token dizisi ya ayrıştırılır ya hemen hata verir
2. Anlık hata lokalizasyonu — geri izleme yok
3. Bağlamdan bağımsız anlama — herhangi bir snippet izole ayrıştırılabilir
4. Öngörülü token bütçesi — macro genişletme yok
```

### 5.2 Adlandırılmış Blok Sonlandırmaları

```arch
module Alu
  // gövde
end module Alu     // ← "end module Alu" tam eşleşme

pipeline Decode
  stage Fetch
    // iç gövde
  end stage Fetch  // ← "end stage Fetch"
end pipeline Decode
```

<cite index="77-1">En yaygın LLM başarısızlık modu — hatalı iç içe geçme — sabit bir derleme hatasına dönüşür. Üretici her zaman tam olarak hangi bloğu kapattığını biliyor.</cite>

### 5.3 todo! Kaçış Kapısı (Çok Akıllı!)

```arch
module Cache
  port req: in CacheReq;
  port resp: out CacheResp;
  port mem_req: out MemReq;

  // Emin olunan kısım: belleğe istek yönlendir
  comb
    mem_req.addr = req.addr;
    mem_req.valid = req.valid;
  end comb

  // Belirsiz kısım: tahliye mantığı ertelendi
  comb resp = todo!; end comb
end module Cache
```

<cite index="77-1">todo! derlenir ve tip kontrol edilir ama simülasyon çalışma zamanında durdurur. Bu AI destekli tasarım için artımlı yaklaşım sağlar: doğru iskelet üret, sonra mantığı bölüm bölüm doldur. Her ara durum derleniyor — AI güvenle ürettiği parçalar için anında geri bildirim alıyor.</cite>

### 5.4 Yönsel Bağlantı Okları

```arch
inst pe[i]: SystolicPE
  a <- data_in[i];       // ← girişi yerel'den besle
  sum_out -> result[i];  // → çıkışı yerel'e oku
end inst pe[i]
```

Yön sözdizimde görünür → AI veri akışını sessizce ters çeviremez.

### 5.5 Minimal AI Bağlamı

<cite index="77-1">Arch donanım tasarımı için efektif AI bağlamı üç bileşenden oluşuyor: ~400 satır Referans Kartı (yapı kataloğu, şema, tip tablosu), 5-20 satır Tasarım Amacı (doğal dil blok açıklaması), 5-30 satır Derleyici Çıktısı (yapılandırılmış hata geri bildirimi).</cite>

---

## Bölüm 6 — Derleyici Mimarisi

### 6.1 Mevcut Durum

<cite index="77-1">Mevcut derleyici çok aşamalı bir pipeline: parse → elaborate → resolve → type-check → codegen, AST'yi doğrudan SystemVerilog metnine dönüştürüyor — özel bir ara temsil (IR) olmadan. Bu mimari ilk sürüm için basitliği ve doğruluğu önceliklendiriyor.</cite>

```
Mevcut: AST → SV (doğrudan, IR yok)
Planlanan: özel IR (AIR) + CIRCT/MLIR arka ucu

Volt ile farkı:
  Volt: CIRCT'i başından itibaren planlıyor (arka uç)
  Arch: doğrudan SV üretiyor, CIRCT gelecek iş
```

### 6.2 Çıktı Garantisi

<cite index="77-1">Üretilen SystemVerilog şunlardan arındırılmış olarak garanti ediliyor: istem dışı latch'ler, çok sürücülü net'ler, çözülmemiş yüksek-Z çıkışlar, başlatılmamış durumdan X yayılımı, örtülü saat alanı geçişleri.</cite>

### 6.3 Simülasyon

<cite index="77-1">arch sim komutu entegre döngü doğru simülasyon sağlıyor. Derleyici her yapı için bağımsız C++ modeli üretiyor, g++ ile derliyor. VCD dalga formu çıktısı, assertion değerlendirme, başlatılmamış register tespiti (--check-uninit) ve CDC gecikme randomizasyonu (--cdc-random) destekleniyor.</cite>

Planlanan: LLVM IR doğal simülasyon → 50-200× hız artışı.

---

## Bölüm 7 — Vaka Çalışmaları

### 7.1 L1 Data Cache (32 KiB, 8-yollu)

<cite index="77-1">8-yollu set-associative write-back/write-allocate L1 data cache: 64 set × 8 yol × 64B satır = 32 KiB, CVA6-uyumlu CPU arayüzü ve AXI4 bellek arayüzü. Tasarım 12 dosyada 1,143 satır Arch kaynak kodu → 1,217 satır SystemVerilog üretiyor (~%6 daha kısa).</cite>

```
Kullanılan yapılar:
  fsm × 3: 9-durumlu ana önbellek kontrolcüsü,
           4-durumlu AXI4 okuma FSM, 4-durumlu yazma FSM
  ram × 3: Tag SRAM, veri SRAM, LRU durum SRAM
  bus × 2: AXI4 ve CVA6 CPU-to-cache
  module × 2: üst seviye entegratör + LRU güncelleme
  generate_for: 8-yollu tag dizisi

Test:
  9 C++ testbench (1,321 satır)
  Yük vuruşu: 3 döngü gecikme
  Yük kaçırma: ~15 döngü
  Kirli tahliye: ~25 döngü
```

### 7.2 AXI DMA Kontrolcüsü (Xilinx PG021 uyumlu)

<cite index="77-1">Çift kanallı AXI DMA kontrolcüsü: Simple DMA ve Scatter-Gather modları. 14 dosyada 1,042 satır Arch → 1,176 satır SV. Yosys ile Sky130 hedefine sentezlendi.</cite>

```
Sentez sonuçları (Sky130 130nm):
  Toplam alan: 78,134 μm²
  Flip-flop: 2,017
  Kritik yol: 4.478 ns
  Maksimum frekans: ~223 MHz
  Güç (100 MHz): 9.03 mW aktif, ~0.02 mW bekleme

Bu gerçek silikon kalitesinde çıktı!
```

---

## Bölüm 8 — Ampirik Değerlendirme

### 8.1 VerilogEval v2 (NVIDIA, 156 problem)

<cite index="77-1">Her problem yalnızca doğal dil spesifikasyonundan çözüldü; referans SystemVerilog'a başvurulmadı. Arch derleyicisi tüm SV çıktısını üretti, Verilator ile doğrulandı.</cite>

```
Sonuçlar:
  Çözülen: 156/156 (%100)
  Verilator temiz: 154/156 (%99)
  
  2 başarısızlık: dataset hatası (Arch değil!)
  - Test harness port adı uyuşmazlığı
  - Verilator'un yasadışı saydığı SV yapısı
  
  Kod yoğunluğu:
  FSM kategorisi: Arch %37 daha kısa
  Genel: Arch %29 daha kısa (3,199 Arch vs 4,518 SV satır)
```

### 8.2 CVDP (231 problem)

```
Sonuçlar:
  arch check geçen: 213/231 (%92)
  Cocotb test geçen: 133/191 (%70)
  
  18 başarısızlık: çok dosyalı tasarım eksikliği
  58 test başarısızlığı: çeşitli nedenler
```

---

## Bölüm 9 — Arch'ın Kabul Ettiği Sınırlar

<cite index="77-1">2-durumlu simülasyonun bilinen zayıflığı: 4-durumlu simülasyonun X yayılımıyla yakalayabileceği tasarım hatalarını maskeleyebilir — "X-iyimserlik" olarak bilinen fenomen.</cite>

```
Arch'ın kabul ettiği eksikler:

1. CDC: yeniden yakınsama analizi YOK
   (birden fazla senkronize sinyal birleşince bit tutarlılığı)

2. CDC: gray-code çok bitli tutarlılık kanıtı YOK
   (ticari CDC araçlar gerekiyor kompleks topoloji için)

3. RDC: tam reset alan geçişi kontrolü PLANLANMIŞ ama yok

4. RAM hücreleri başlatılmamış: runtime tespiti YOK
   (--check-uninit sadece register'lar için şu an)

5. Vec indeks sınır dışı: YOK

6. UPF/CPF güç alanı modeli: KAPSAM DIŞI

7. CIRCT/MLIR arka ucu: PLANLANMIŞ ama yok

8. Formal doğrulama (SymbiYosys): PLANLANMIŞ ama yok
```

---

## Bölüm 10 — Volt ile Detaylı Karşılaştırma

### 10.1 Özellik Matrisi

```
ÖZELLİK                      Volt      Arch
─────────────────────────────────────────────────────
CDC tip sisteminde             ✓         ✓
Latch önleme                   ✓         ✓
Tek sürücü kuralı              ✓         ✓
Pipeline birinci sınıf         ✓         ✓
FSM birinci sınıf              ✓         ✓
FIFO birinci sınıf             ✓         ✓
Arbiter birinci sınıf          ✓         ✓
RegFile birinci sınıf          ✗         ✓
Sayaç birinci sınıf            ✗         ✓
Bağlantılı liste birinci sınıf ✗         ✓
Clock kapısı birinci sınıf     ✗         ✓
Bus tanımı                     ✗         ✓
Hook mekanizması               ✗         ✓
Template (trait)               ✗         ✓
LL(1) gramer                   ✗         ✓ (AI hedefi)
todo! kaçış kapısı             ✗         ✓
Yönsel bağlantı okları         ✗         ✓
Koşullu port üretimi           ✗         ✓ (generate_if)
CIRCT/MLIR arka ucu            ✓(plan)   ✗(plan)
Formal doğrulama               ✓(plan)   ✗(plan)
Ternary/Trit tipi              ✓         ✗
Nöromorfik uzantı (v2)         ✓         ✗
Fotonik uzantı (v3)            ✓         ✗
Çalışan uygulama               ✗(plan)   ✓✓(case studies)
Ampirik değerlendirme          ✗         ✓(387 problem)
─────────────────────────────────────────────────────
```

### 10.2 Arch'ın Açık Üstünlükleri

```
1. Çalışan derleyici:
   Arch: iki büyük vaka çalışması (L1 önbellek + DMA)
   Volt: henüz F0 bile yok

2. AI-native tasarım kararları:
   LL(1) gramer, todo!, yönsel oklar, minimal bağlam
   Volt'ta bu tasarım katmanı eksik

3. Daha geniş birinci sınıf yapı seti:
   RegFile, sayaç, bağlantılı liste, bus, hook, template
   Volt bu yapıları stdlib olarak planlıyor

4. Koşullu port üretimi:
   generate_if → preprocessor gerektirmeden koşullu portlar
   Volt'ta net değil

5. Sentez kanıtı:
   Sky130 + Yosys → gerçek silikon kalitesi çıktı
   Volt'ta henüz SV çıktısı yok
```

### 10.3 Volt'un Fırsatları Arch'ta Yok

```
1. CIRCT/MLIR arka ucu:
   Arch: AST → SV (doğrudan, kırılgan)
   Volt: CIRCT → çok hedef, optimize edilmiş, sağlam
   Arch'ın bu eksikliği bir risk: CIRCT olmadan
   çıktı kalitesi manuel effort gerektiriyor

2. Daha güçlü CDC garantisi:
   Arch CDC: bireysel sinyal ✓, yeniden yakınsama ✗
   Volt CDC hedefi: formal tip sistemi + K-framework
   → daha kapsamlı CDC kanıtı mümkün

3. Çok paradigma vizyonu:
   Ternary + Nöromorfik + Fotonik
   Arch tamamen klasik dijital RTL

4. Paket yöneticisi + registry:
   Arch'ta yok
   Volt ekosistem stratejisinde kritik

5. Topluluk ekosistemi:
   Arch: tek kişi (Shuqing Zhao)
   Volt: topluluk, Foundation, açık çekirdek model hedefi
```

---

## Bölüm 11 — Stratejik Değerlendirme

### 11.1 Arch Bir Tehdit mi?

```
Evet ve Hayır:

Evet — Tehdit:
  Volt'un MVP hedefini (CDC + bağımsız + birinci sınıf yapılar)
  Arch 3 ay önce yayımladı ve çalışıyor
  Volt geliştirme başlarken Arch'ın üstüne çıkmak zorunda

Hayır — Tehdit Değil:
  Farklı hedef kitle: Arch = AI-native micro-arch mühendisi
                       Volt = geniş ekosistem, çok paradigma
  Arch ternary/nöromorfik/fotonik vizyonu taşımıyor
  Arch CIRCT/formal eksik → Volt orada farklılaşabilir
  Arch tek kişi → sürdürülebilirlik riski var
```

### 11.2 Volt için Ders Listesi

```
Arch'tan öğrenilebilecekler:

1. todo! mekanizması → Volt'a ekle (çok değerli!)
   AI ajanının artımlı çalışması için şart

2. Yönsel bağlantı okları (<-, ->) → sözdizim kararı
   Volt'un bağlantı sözdizimi buna benzer mi?

3. LL(1) gramer hedefi → Volt gramer tasarımında göz önünde
   AI üretimi için bu garanti değerli

4. Adlandırılmış blok sonlandırma → Volt'ta var mı?
   end module MyModule tarzı sözdizim

5. Hook mekanizması → stdlib yerine dil seviyesinde

6. Koşullu port üretimi → generate_if → preprocessor çözümü

7. Benchmark: VerilogEval v2 → Volt için aynı benchmark
   "Volt vs Arch: VerilogEval karşılaştırması" hedefi

8. Vaka çalışması önceliği → L1 önbellek veya DMA benzeri
   gerçek tasarım → itibar kanıtı
```

### 11.3 Arch'ın Zayıflıklarından Faydalanmak

```
Arch'ın eksiklerini Volt'un güçlü yönleri yapılabilir:

Arch eksiği 1: CIRCT yok
  Volt avantajı: CIRCT → çok hedef, optimize çıktı
  Mesaj: "Arch SV üretir, Volt optimize SV üretir"

Arch eksiği 2: CDC kapsamlı değil
  Volt avantajı: formal CDC kanıtı (yeniden yakınsama dahil)
  Mesaj: "Arch CDC temel, Volt CDC kanıt"

Arch eksiği 3: Tek paradigma (dijital RTL sadece)
  Volt avantajı: ternary, nöromorfik, fotonik
  Mesaj: "Arch bugünün donanımı için, Volt yarın için de"

Arch eksiği 4: Tek kişi
  Volt avantajı: topluluk odaklı, Foundation planı
  Mesaj: "Arch proje, Volt ekosistem"

Arch eksiği 5: Formal doğrulama yok
  Volt avantajı: SymbiYosys entegrasyonu
  Mesaj: "Arch üret, Volt kanıtla"
```

---

## Özet

```
Arch kimdir:
  Nisan 2026, tek kişi, çalışan derleyici
  AI-native, LL(1), CDC tipte, birinci sınıf mikro-mimari
  Vaka çalışması: L1 önbellek + AXI DMA (gerçek donanım!)
  VerilogEval: %100 başarı

Volt'tan ne kadar farklı:
  Çok örtüşen alan (CDC, birinci sınıf yapılar)
  Ama: CIRCT yok, formal yok, ternary/nöromorfik/fotonik yok
       Topluluk değil, tek kişi

Volt için öneri:
  1. todo! mekanizmasını ekle → hemen
  2. LL(1) gramer hedefle → gramer tasarımında
  3. Arch'tan öğren, onunla rekabet etme
  4. "Arch + CIRCT + formal + çok paradigma" = Volt

En kritik karar:
  Volt, Arch'ı fork etmeli mi? Hayır — farklı vizyon
  Volt, Arch ile işbirliği yapmalı mı? Belki — CIRCT katkısı
  Volt, Arch'ı görmezden gelmeli mi? Kesinlikle hayır
```
