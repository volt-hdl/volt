> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Spade HDL — Detaylı İnceleme Raporu

> ACM Transactions on Reconfigurable Technology and Systems, Ocak 2026
> Frans Skarman, Gustav Sörnäs, Oscar Gustafsson
> Linköping Üniversitesi (İsveç) → Münih Uygulamalı Bilimler Üniversitesi
> Makalenin tamamı okunarak hazırlanmıştır.

---

## Bölüm 1 — Temel Kimlik

```
Adı:     Spade
Yazarlar: Frans Skarman, Gustav Sörnäs, Oscar Gustafsson
Kurum:   Linköping Üniversitesi → Münih Uygulamalı Bilimler Üniversitesi
Yayın:   ACM Trans. Reconfig. Technol. Syst., Ocak 2026
         (revizyon tarihleri: 30 Mayıs 2025, 19 Ağustos 2025)
Lisans:  EUPL-1.2 (derleyici), stdlib: permissive
Finansman: NLNet NGI Zero Core
PhD Tezi: Frans Skarman, Ağustos 2025
Kaynak:  GitLab: gitlab.com/spade-lang/spade
         GitHub: github.com/spade-lang/spade (salt okunur ayna)
Discord: ~300 kullanıcı

Önemli not: GitHub katkı politikası açıkça şunu belirtiyor:
"LLM veya diğer olasılıksal araçlar tarafından üretilmiş içerik
 kabul edilmez." — Arch HDL'nin tam tersi yaklaşım!
```

---

## Bölüm 2 — Tasarım Felsefesi

### 2.1 Temel İddia

<cite index="84">Spade, yüksek performanstan ödün vermeden geleneksel HDL'lerden daha üretken olmayı hedefliyor. Bu, pipeline gibi yaygın donanım yapıları için soyutlamalar oluşturarak ve Rust veya Haskell'inkine yakın güçlü bir tip sistemi gibi yazılım dillerinden fikirleri doğrudan benimseyerek başarılıyor.</cite>

### 2.2 "Donanım Yazılım Değildir" İlkesi

<cite index="84">Donanımda, özel çip inşa etmenin amacı yazılımın sağlayabileceğinden daha iyi performans elde etmek olduğundan, üretkenlik için ham performanstan ödün vermek yazılımdaki kadar sık kabul edilemez. Bu nedenle, Spade'deki soyutlamalar RTL soyutlamasının üstüne inşa edilmiş, onu yerinin değiştirmek yerine.</cite>

### 2.3 Sentez-Öncelikli Yaklaşım

<cite index="87">Spade programlama modeli senkron tasarımlarla sınırlı. Bugün dijital tasarımların büyük çoğunluğu senkron olduğundan bu özellikle kısıtlayıcı değil. Spade yalnızca sentezlenebilir donanım yazmak için tasarlandı — simülasyon veya üst seviye modelleme mümkün değil.</cite>

---

## Bölüm 3 — Dil Temelleri

### 3.1 Üç Temel Birim Türü

```spade
// Fonksiyon: sadece kombinasyonel mantık
fn add(a: uint<8>, b: uint<8>) -> uint<9> {
    a + b
}

// Entity: sıralı mantık, genel amaçlı
entity blink(clk: clock, rst: bool, max: uint<20>) -> bool {
    reg(clk) counter reset(rst: 0) =
        if counter == max { 0 }
        else { trunc(counter + 1) };
    counter > (max / 2)
}

// Pipeline: pipeline yapısı (aşağıda detay)
pipeline(2) mac(clk: clock, a: int<16>, b: int<16>) -> int<33> {
    let product = a * b;
    reg;
    let acc = product + prev_acc;
    reg;
    acc
}
```

### 3.2 İfade Tabanlı Tasarım

<cite index="87">Spade değişmez değişkenler ve ifade tabanlı sözdizimi kullanıyor — koşullu atama yerine if-ifadesi bir değer döndürüyor. Bu yaklaşım hem donanıma daha yakın (multiplexer) hem de latch oluşturmayı çok daha zor kılıyor — çünkü her dalda çıkış belirtilmek zorunda, aksi halde derleme hatası.</cite>

**Volt ile kıyaslama:**
```
Spade: değişmez değişkenler + ifade tabanlı
Volt: on Sys { } + <= ataması

Spade'in yaklaşımı fonksiyonel — değer akışı
Volt'un yaklaşımı imperatif — sinyal ataması

Farklı tasarım kararı, ikisi de meşru.
```

---

## Bölüm 4 — Pipeline Sistemi (Spade'in Güçlü Farkı)

### 4.1 Pipeline Derinliği Arayüzde

<cite index="87">Pipeline'larda bu arayüz, etkili olarak girişler ve çıkışlar arasındaki gecikme olan derinliği içeriyor. Bu derinliği dış arayüze dahil etmek, hem kullanıcının hem derleyicinin pipeline gövdesini okumadan pipeline'ların zamanlama davranışını anlamasını sağlıyor.</cite>

```spade
// Derinlik arayüzde görünür: pipeline(2)
pipeline(2) X(clk: clock, a: int<32>, b: int<32>) -> int<33> {
    let x = inst(1) g(clk, a);   // 1 gecikme
    let product = a * b;
    reg;                          // aşama 1
    let sum = x + f(a, product)
    reg;                          // aşama 2
    sum
}
```

### 4.2 Derleme Zamanı Pipeline Güvenliği

<cite index="87">Derleyici pipeline'ların gecikmesini bildiğinden, iç içe pipeline'ları doğru şekilde ele alıyor. Derleyici ayrıca hazır olmadan önce pipelined sonuçların kullanılmamasını da sağlıyor.</cite>

Hata mesajı örneği:
```
error: Use of x before it is ready
  ┌─ src/main.spade:5:15
  │
2 │   let x = inst(3) g(clk, a);
  │           - x stage 0'da tanımlandı, gecikmesi 3
5 │   let sum = x + f(a, product);
  │             ^
  │             x için 2 aşama daha bekleniyor
  = not: x'in stage 3'te hazır olacak
  = help: x tanımı ve kullanımı arasına daha fazla reg; ekleyin
```

### 4.3 Stage Referansları ve Veri Yönlendirme

<cite index="87">İşlemci pipeline'ındaki veri yönlendirme mantığı stage referanslarıyla yazılabiliyor. stage(+x) gelecekteki aşamaya, stage(name) ise adlandırılmış aşamaya referans veriyor.</cite>

```spade
// Processor veri yönlendirme — stage referansları ile
let opa = if stage(+1).dest == srca { stage(+1).alu_out }
          else if stage(+2).dest == srca { stage(+2).alu_out }
          else { reg_out.a };
```

**Volt ile kıyaslama:**
```
Spade: pipeline(N) + reg; aşama belirteci + stage(+n) referansı
Volt L1: Delayed<T,N> ve L2: timeline tipler

Spade daha pratik (bugün çalışıyor)
Volt L2 daha formal (derleme zamanı kanıt)
Filament en güçlü (kaynak çakışması analizi)
```

---

## Bölüm 5 — Tip Sistemi (En Özgün Katkı)

### 5.1 Sum Tipleri ve Pattern Matching

<cite index="87">Spade, Rust, Haskell ve ML gibi dillerden ilham alan sum tiplerini destekliyor. C veya VHDL'deki enum'lardan farklı olarak Spade'deki enum'lar birden fazla değere ek olarak her varyantla ilişkili veri taşıyor.</cite>

```spade
enum State {
    Idle,
    WaitAddr1{ addr0: uint<8> },
    WaitData{ addr0: uint<8>, addr1: uint<8> },
}

// Pattern matching ile FSM — tam ve güvenli
reg(clk) state reset(rst: State::Idle) =>
    match (data, state) {
        (_, (false, _)) => state
        (State::Idle, (_, byte)) => State::WaitAddr1(byte),
        (State::WaitAddr1(addr0), (_, byte)) => State::WaitData(addr0, byte),
        (State::WaitData(_, _), (_, _)) => State::Idle,
    };
```

**Volt'ta ne var:** Volt'ta enum/match benzeri yapı planlandı ama tasarım detayları Spade'den öğrenilebilir. Spade'in "payload ile varyant" yaklaşımı güçlü.

### 5.2 Generics ve Trait Sistemi

```spade
// Trait tanımı
trait Addable {
    fn add(self, other: Self) -> Self;
}

// Herhangi genişlik için uygulama
impl<#uint N> Addable for int<N> {
    fn add(self, other: int<N>) -> int<N> {
        trunc(self + other)
    }
}

// Generic entity — where clause ile kısıt
entity sum<T>(clk: clock, rst: bool, reset_value: T, in: T) -> T
where T: Addable
{
    reg(clk) acc reset(rst: reset_value) = acc.add(in);
    acc
}
```

### 5.3 Yüksek Mertebeli Fonksiyonlar (Functional Hardware)

```spade
// FIR filtre — fonksiyonel stil
samples
    .inst sliding_window::<16>()
    .map(fn (window) {
        window
            .zip(coefficients)
            .map(fn (x, c) { x * c })
            .sum()
    })
```

<cite index="87">ShakeFlow'u Spade içinde yeniden uygulamak mümkün. Bu, hem gecikmeye duyarsız birleştiricilerin faydalarından yararlanmayı hem de bunları saf RTL veya pipeline gibi diğer soyutlamalarla karıştırmayı sağlıyor.</cite>

### 5.4 Lineer Tipler ve Port Güvenliği (Spade'in En Özgün Özelliği)

<cite index="87">Spade, derleme zamanında tüm ters tellerin tam bir kez tüketilmesini sağlamak için lineer tipler kullanıyor. Ters tel ya bir değer atandığında ya da başka bir birime iletildiğinde tüketiliyor.</cite>

```spade
// Port tipi tanımı
struct port MemReadPort<T> {
    addr: &inv uint<16>   // ters tel: okuyucu seti
    value: &T             // normal tel: bellek döndürür
}

// Birim kullanımı
entity compute(clk: clock, mem: MemReadPort<MemData>) -> Out { ... }

// Derleme zamanı hata — p1 iki kez kullanılıyor!
entity top() {
    let (p1, p2) = inst dp_mem();
    (inst compute(p1), inst compute(p1))
    //                             ^^ lineer tip hatası:
    //                                p1 zaten tüketildi
}
```

**Bu Volt için kritik ders:**
```
Volt'un port yönü güvenliği: tip sistemi port yönünü izliyor
Spade'in lineer tipler: her tel tam bir kez sürülüyor

Spade daha güçlü: "iki kez bağlama" hatası derleme zamanı
Volt bunu düşünmeli mi? Evet — bu sessiz bir hata kaynağı
```

---

## Bölüm 6 — CDC Durumu (Kritik Bulgu)

<cite index="87">Saat alanlarını geçmek, yalnızca aralıklı sorunlara yol açması nedeniyle hata ayıklaması zor olan potansiyel bir hata kaynağı. Bu nedenle soyutlama için birincil aday. Yerel saat alanları ayrıca pipeline'lar için enable sinyallerinin yayılmasını da sağlıyor. Bu nedenle Spade'e saat alanı desteği eklemek birincil adaylardan biri. Chisel, SpinalHDL ve Clash dahil çeşitli mevcut HDL'ler saat alanlarını destekliyor.</cite>

```
KRİTİK: Spade CDC'yi açıkça gelecek iş olarak belirtiyor!

Bu son derece önemli:
  Spade yazarları sorunu biliyor
  Çözme planları var
  Ama henüz yok

Volt için bu ne anlama gelir:
  Spade CDC eklemeden önce Volt bunu yapabilir
  → Volt öne geçebilir
  Spade CDC ekledikten sonra Volt bunu yaparsa
  → "Spade'in yaptığı" denir

  Hız kritik burada.
  Spade aktif bir proje — beklemek riskli.
```

---

## Bölüm 7 — Araç Ekosistemi (Spade'in Güçlü Yönü)

### 7.1 Swim — Build Aracı

```toml
# swim.toml
name = "example"

[libraries]
hdmi = { git = "https://gitlab.com/TheZoq2/hdmi" }

[synthesis]
command = "synth_ecp5"
top = "example::main::main"

[pnr]
...
```

```bash
swim upload   # tek komut: derle + sentez + PnR + yükle
swim test     # paralel test çalıştırma (cocotb + Verilator)
```

<cite index="87">Swim, proje bağımlılıklarının ad alanlamasını ve yönetimini otomatik olarak halledip, backend araçlarını çalıştırarak kullanıcıların sıfırdan bir projeyi tek swim upload komutuyla FPGA kartına yüklemesine imkân tanıyor.</cite>

### 7.2 Surfer — Dalga Formu Görüntüleyici

<cite index="87">Surfer, bit vektörlerini hiyerarşik değerlere çeviren uzantıları destekliyor. Kullanıcıya değerin insan tarafından okunabilir bir temsili sunuluyor ve struct gibi yapıların bireysel alanlara genişletilmesine olanak tanıyor.</cite>

Önemli gelişme:
<cite index="87">Surfer başlangıçta Spade için oluşturulmuş olsa da Tywaves projesi aracılığıyla Chisel entegrasyonu dahil önemli topluluk benimsemesiyle bağımsız bir proje haline geldi.</cite>

### 7.3 Test Altyapısı

```python
# Cocotb testi — Spade tipi ifadeleri doğrudan kullan
@cocotb.test()
async def test_mul(dut):
    s = SpadeExt(dut)
    s.i.op = "Op::Mul(5, 6)"   # Spade enum değeri string olarak
    await FallingEdge(clk)
    s.o.out.assert_eq("Some(30)")
```

<cite index="87">Spade tip sistemini ön plana çıkardığından, kullanıcıların çoğu değerin bit örüntülerini bilmesi nadiren gerekiyor. Sadece Spade gösterimi olarak biliyorlar. Bu yüzden bit dizileri yerine Spade ifadeleri string olarak yazılabilen cocotb ve Verilator değerleri etrafında paketleyiciler var.</cite>

### 7.4 Playground (WebAssembly) — Şu An Var!

```
https://play.spade-lang.org

WebAssembly içinde tam derleyici ve simülasyon
Kurulum gerektirmeden tarayıcıda çalışıyor
Volt henüz bunu yapmadı!
```

### 7.5 LSP Sunucusu

<cite index="87">Spade'in henüz devam eden (work-in-progress) LSP sunucusu var — inline tanılar, hover ipuçları ve değişken/birim referanslarına ve tanımlarına navigasyonu destekliyor. Sözdizimi vurgulama için Vim ve Helix'i destekleyen tree-sitter grameri var.</cite>

---

## Bölüm 8 — Topluluk ve Benims

```
Topluluk:
  ~300 Discord kullanıcısı
  Topluluk katkıcıları: 23 kişi listelenmiş
  Filament yazarı Rachit Nigam da katkıcılar listesinde!

Finansman:
  NLNet NGI Zero Core (Avrupa açık kaynak finansmanı)
  Akademik: Linköping → Münih

Yayınlar:
  FPL 2022 → OSDA 2023 → LATTE 2023 → FDL 2023
  → Latch-Up 2024 → ACM TRETS Ocak 2026
  → PhD Tezi Ağustos 2025

Eğitim:
  UC Santa Cruz CSE 228A'da misafir ders (Bahar 2025)
  Bazı üniversitelerde müfredata girmeye başlıyor
```

---

## Bölüm 9 — Öğrenme Eğrisi ve Hedef Kitle

<cite index="87">Topluluğun hem donanım geliştiricilerinden hem de donanım geliştirmeyle ilgilenen yazılım geliştiricilerinden oluştuğu görünüyor; ancak izlenimimiz hafifçe donanım geliştirmek isteyen yazılım geliştiricilerine doğru eğilimli.</cite>

```
Hedef kitle profili:
  → Rust/modern dil bilen yazılımcı, donanım öğrenmek istiyor
  → Verilog'dan bıkan ama Scala/Haskell istemeyenler
  → FPGA hobicileri (asıl mevcut kitle)

Spade'in hedeflediği değil:
  → Büyük ASIC ekipleri (UVM, CDC, sign-off eksik)
  → Enterprise kullanım (EUPL lisans belirsizliği)
```

---

## Bölüm 10 — Volt ile Detaylı Karşılaştırma

### 10.1 Özellik Matrisi

```
ÖZELLİK                    Volt        Spade
─────────────────────────────────────────────────────────────
Bağımsız dil                ✓           ✓
Rust ilhamlı sözdizim       ✓           ✓
Güçlü tip sistemi           ✓           ✓
İfade tabanlı               ✓(kısm)     ✓✓(tam)
Değişmez değişkenler        ✗           ✓
Pipeline desteği            ✓(L0/L1/L2) ✓(pipeline birimi)
Sum tipler + pattern match  ✓(planlandı)✓✓(mevcut)
Generics + traits           ✓           ✓
Yüksek mertebeli fonksiyon  ✗           ✓ (ShakeFlow)
Lineer tipler (portlar)     ✗           ✓ (benzersiz!)
Option<T> standardı         ✗           ✓
Latency-insensitive         ✗           ✓ (Rv combinator)
CDC tip sisteminde          ✓           ✗ (planlandı!)
Latch önleme                ✓           ✓ (expr tabanlı)
Formal doğrulama            ✓(plan)     ✗
CIRCT arka ucu              ✓(plan)     ✗(mevcut: Verilog)
                                        ✓(plan: CIRCT/Calyx)
Ternary/Trit                ✓           ✗
Nöromorfik (v2)             ✓           ✗
Fotonik (v3)                ✓           ✗
Build aracı                 ✓(plan)     ✓✓(Swim, mevcut)
LSP sunucu                  ✓(plan)     ✓(WIP, mevcut)
Playground (WASM)           ✓(plan)     ✓✓(MEVCUT!)
Paket yöneticisi            ✓(plan)     ✓✓(Swim ile, mevcut)
Dalga formu görüntüleyici   ✗           ✓✓(Surfer)
Cocotb entegrasyonu         ✓(plan)     ✓✓(mevcut, tip farkında)
Topluluk                    ✗           ✓(300 kullanıcı)
Çalışan uygulama            ✗(plan)     ✓✓(üretim kalitesi)
Akademik makale             ✗           ✓✓(ACM TRETS 2026)
─────────────────────────────────────────────────────────────
```

### 10.2 Spade'in Açık Üstünlükleri

```
1. MEVCUT çalışan uygulama:
   Gerçek projeler üretiliyor
   Volt henüz F0'da bile değil

2. Eksiksiz araç ekosistemi (BUGÜN mevcut):
   Swim + Surfer + cocotb entegrasyonu + Playground
   Volt bunları planlıyor

3. Lineer tip sistemi (benzersiz, değerli!):
   Her tel tam bir kez sürülüyor
   Çift sürücü hatası derleme zamanı
   Volt'ta yok, eklenmeli mi?

4. Daha olgun tip sistemi:
   Sum tipler, Option<T>, trait sistemi çalışıyor
   ShakeFlow latency-insensitive combinators

5. Akademik meşruiyet:
   ACM TRETS (en prestijli FPGA dergisi)
   PhD tezi tamamlandı
   Volt'ta henüz makale yok

6. Topluluk (300 kullanıcı, aktif):
   Discord, katkıcılar, Surfer yan projesi
   Volt'ta topluluk henüz yok
```

### 10.3 Volt'un Fırsatları Spade'de Yok

```
1. CDC tip sisteminde:
   Spade bunu açıkça gelecek iş olarak planlıyor
   Volt bunu önce yaparsa: güçlü farklılaşma
   Spade bunu önce yaparsa: Volt'un ana özelliği kaybolur

2. Ternary/Trit + nöromorfik + fotonik:
   Spade'de hiç yok
   Bu Volt'un uzun vadeli farklılaşması

3. CIRCT arka ucu:
   Spade mevcut Verilog emitter kullanıyor
   CIRCT/Calyx planlanmış ama yok
   Volt başından CIRCT → daha sağlam, çok hedef

4. Formal doğrulama:
   Spade'de yok
   Volt'ta SymbiYosys entegrasyonu planlandı
```

---

## Bölüm 11 — Volt için Ders Listesi

```
Spade'den öğrenilecekler:

1. Lineer tip sistemi → Volt'a ekle!
   Her port tam bir kez bağlanmak zorunda
   Çift sürücü hatası derleme zamanı
   Bu sessiz bir hata kaynağını ortadan kaldırır

2. Option<T> standardı → stdlib'e ekle
   valid-data çifti için canonical tip
   Cocotb entegrasyonu tip farkında olmalı

3. Swim'in TOML proje yapısı → Volt benzer olmalı
   [dependencies], [synthesis], [pnr] bölümleri

4. Cocotb tip farkında entegrasyonu:
   "Op::Mul(5, 6)" string Spade expression olarak
   Bu çok akıllı bir yaklaşım

5. Surfer benzeri dalga formu görüntüleyici:
   Spade struct değerlerini insan okunabilir gösteriyor
   Volt için de benzer araç değerli

6. Playground (WASM) önceliği:
   Spade şu an mevcut
   Volt Ay 6 planı — bu doğru ama Spade zaten var

7. "Contributions must not include LLM-generated content":
   Spade'in tersi politika
   Volt bu konuda ne yapacak?
   (Muhtemelen LLM katkısına izin vermeli — farklı hedef)

8. Stage referansları (stage(+1), stage(name)):
   Pipeline içi veri yönlendirme için zarif çözüm
   Volt L1/L2'de benzer mekanizma düşünülmeli
```

---

## Bölüm 12 — Stratejik Değerlendirme

### 12.1 Spade Bir Tehdit mi?

```
Evet — Tehdit:
  En aktif, en olgun bağımsız HDL
  Volt'un hedef kitlesini (FPGA hobici + öğrenci) paylaşıyor
  Araç ekosistemi çok daha olgun
  CDC planlanmış → yakında bu avantaj da kapanabilir

Hayır — Tehdit Değil:
  CDC henüz yok → Volt'un fırsatı
  Ternary/nöromorfik/fotonik yok → uzun vadeli fark
  ASIC hedeflemesi zayıf
  Enterprise özellikler yok (UVM, sign-off)

En gerçekçi senaryo:
  Spade FPGA'da dominant, Volt ASIC+çok paradigmada
  İki araç farklı nişlerde — rekabet kısmı
```

### 12.2 İşbirliği Fırsatı

```
Spade-Volt ortaklığı mümkün mü?

  Surfer: Spade topluluk projesi → Volt desteği eklenebilir
  CIRCT: İkisi de hedefliyor → ortak çalışma?
  Pipeline semantiği: Spade'in stage referansları
                      Volt L1/L2'yi etkileyebilir

  Frans Skarman ile iletişim:
  "CDC planlarınız ne zaman? Birlikte çalışabilir miyiz?"

  Spade: "Biz CDC yapmadan önce sen yap, bize entegre et"
  → Bu mantıklı bir iş birliği modeli
```

---

## Özet

```
Spade Kimdir:
  Ocak 2026 ACM TRETS makalesi
  Frans Skarman PhD tezi ağustos 2025
  NLNet finansmanı, ~300 kullanıcı
  En olgun bağımsız HDL, FPGA odaklı

Güçlü yönleri:
  Çalışan ekosistem (Swim, Surfer, Playground, cocotb)
  Lineer tip sistemi (benzersiz, çok değerli)
  Güçlü tip sistemi (sum tipler, trait, Option<T>)
  ShakeFlow latency-insensitive combinators
  Akademik meşruiyet (ACM TRETS)

Zayıf yönleri:
  CDC yok (ama planlandı → hız önemli!)
  CIRCT yok (planlandı)
  Formal doğrulama yok
  Ternary/nöromorfik/fotonik yok
  ASIC hedefleme zayıf

Volt için kritik çıkarım:
  CDC önce yapılmalı — Spade bunu planlıyor
  Lineer tip sistemi Volt'a eklenmeli (sessiz hata sınıfı kapatılıyor)
  Playground Ay 6'da olmalı — Spade zaten var
  Surfer benzeri tip-farkında dalga formu araç değerli

Tek cümle:
  Spade Volt'un en olgun ve en ciddi rekabeti;
  CDC yokluğu şu anki tek büyük açığı,
  ama bu boşluk kapanmadan Volt bu fırsatı kullanmalı.
```
