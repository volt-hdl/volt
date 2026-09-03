# Volt UX Anayasası
## "En İyi Onboarding Olmayan Onboarding'dir"

> Bu belge, Volt-Butunlesik-Mimari-v3.md'nin özellik listesini
> kullanıcı deneyimi merceğinden yeniden düzenler.
> ÇATIŞMA DURUMUNDA BU BELGE ÖNCELİKLİDİR.

---

# BÖLÜM I — TEMEL İLKELER

## İlke 1: Sıfır Bilgiyle İlk Başarı

```
Kullanıcı Volt'u hiç bilmiyor.
İlk 60 saniyede çalışan bir şey görmeli.

Bu dosya öğretici gerektirmemeli:

    module Counter {
        in  clk    : clock
        in  enable : bool
        out count  : u8

        reg count = 0

        on clk {
            if enable { count <= count + 1 }
        }
    }

Görülmeyenler (kasıtlı):
    domain tanımı yok        → clock tipi yeterli
    @Domain anotasyonu yok   → tek saat varsa gerekmez
    reset tanımı yok         → varsayılan uygulanır
    kontrat yok              → opsiyonel
    Trit yok                 → opt-in
    lineer tip yok           → sadece port'larda
    modül sistemi yok        → tek dosya çalışır

Bunların hepsi VAR ama GÖRÜNMÜYOR.
```

## İlke 2: Kullanıcıyı Cezalandırma

```
CEZALANDIRICI (yasak):
  ✗ "reset tanımlanmadı" → hata
  ✗ "domain belirtilmedi" → hata
  ✗ "kontrat eksik" → uyarı
  ✗ "Bu özelliği kullanmak için önce X'i öğrenin"
  ✗ Kriptik hata mesajı
  ✗ 10 satır boilerplate zorunluluğu

DESTEKLEYİCİ (hedef):
  ✓ Makul varsayılan sessizce uygulanır
  ✓ Hata mesajı çözümü gösterir
  ✓ Gelişmiş özellik istendiğinde ortaya çıkar
  ✓ Yanlış yaparsa nazikçe yönlendirir
```

## İlke 3: Karmaşıklık Talep Üzerine

```
Kullanıcının karşılaştığı karmaşıklık = ihtiyacı kadar

Tek saat alanı kullanıyor       → domain sistemi görünmez
İki saat alanı kullanıyor       → domain kavramı ortaya çıkar
Güç yönetimi yapıyor            → power alanları ortaya çıkar
ASIC'e gidiyor                  → @budget, @timing ortaya çıkar
Sertifikasyon gerekiyor         → determinizm, DFT ortaya çıkar

Hiçbiri başlangıçta zorunlu değil.
```

---

# BÖLÜM II — KADEMELİ AÇILIM MİMARİSİ

## Seviye 0 — "Merhaba Donanım" (0-15 dakika)

```volt
// counter.volt — bilinmesi gereken her şey burada
module Counter {
    in  clk    : clock       // yerleşik tip
    in  enable : bool
    out count  : u8

    reg count = 0            // tip çıkarımı: u8 (porttan)

    on clk {
        if enable { count <= count + 1 }
    }
}
```

```bash
volt build counter.volt
# ✓ counter.sv üretildi (28 satır)

volt sim counter.volt
# ✓ Simülasyon: 0, 1, 2, 3, 4...
```

**Bu seviyede öğrenilenler:** `module`, `in/out`, `reg`, `on clk`, `<=`
**Bu seviyede öğrenilmeyenler:** Diğer her şey.

**Kritik tasarım kararı — `clock` yerleşik tip:**

```
Domain sistemi VAR ama görünmüyor:
  in clk : clock
  → derleyici otomatik olarak implicit domain oluşturur
  → reset: sync, active_high (varsayılan)
  → tek saat alanı → CDC kontrolü trivially geçer

Kullanıcı domain kelimesini hiç duymadan çalışan tasarım yapar.
```

## Seviye 1 — "Gerçek Tasarım" (15-60 dakika)

Kullanıcı ikinci saati eklediğinde domain kavramı **kendiliğinden ortaya çıkar**:

```volt
module CrossClock {
    in  fast_clk : clock
    in  slow_clk : clock
    in  data     : u8 on fast_clk       // "on" ile saat belirtme
    out result   : u8 on slow_clk

    result = data      // ← burada hata gelir
}
```

```
error[E3001]: farklı saat alanları arasında doğrudan bağlantı
  ┌─ crossclock.volt:7:14
  │
7 │     result = data
  │              ^^^^ 'data' fast_clk alanında
  │     ^^^^^^ 'result' slow_clk alanında
  │
  = neden: iki saat arasında doğrudan bağlantı metastabilite
           yaratır — sinyal yarı yolda yakalanabilir
  = çözüm: senkronizatör ekleyin
  
  ┌ önerilen düzeltme ─────────────────────┐
  │  result = sync(data, slow_clk)          │
  └─────────────────────────────────────────┘
  
  = daha fazla: volt explain E3001
```

```bash
volt explain E3001
```
```
CDC (Clock Domain Crossing) Nedir?

Bir sinyal bir saat alanından başka bir saat alanına geçtiğinde,
hedef flip-flop sinyali "kararsız" bir anda yakalayabilir.
Sonuç: bazen 0, bazen 1 — öngörülemez.

Volt bunu derleme zamanında yakalar. Verilog'da bu hata
sessizce geçer ve genellikle silisyumda ortaya çıkar.

Çözüm: sync() fonksiyonu iki flip-flop ekler, metastabiliteyi
kararlı hale getirir.

    result = sync(data, slow_clk)

Daha karmaşık durumlar için: volt explain cdc-advanced
```

**Kritik nokta:** Kullanıcı `domain` bloğu yazmadı. `on fast_clk` yazdı.
Sistem gerisini hallediyor. Hata geldiğinde **öğreniyor**, cezalandırılmıyor.

## Seviye 2 — "Domain Kontrolü" (kullanıcı isterse)

Kullanıcı reset polaritesini değiştirmek istediğinde:

```volt
domain UsbDomain {
    clock  = posedge
    reset  = async active_low        // sadece istediğini yaz
}

module UsbController {
    in  data : u8 @UsbDomain
    // ...
}
```

**Belirtilmeyen her şey varsayılan kalır.** `voltage`, `retention`,
`trust_level` yazılmazsa yok sayılır — hata değil.

## Seviye 3 — "Doğrulama" (kullanıcı hazır olduğunda)

```volt
module Divider {
    // Tek satır ekle, formal doğrulama başlasın
    invariant: remainder < divisor
}
```

```bash
volt check divider.volt
# ✓ Tip kontrolü
# ✓ Formal: invariant 20 döngü boyunca doğrulandı
#   İpucu: 'volt formal --depth 100' ile daha derin kontrol
```

## Seviye 4 — "ASIC Hazırlığı" (proje gerektirdiğinde)

```volt
@budget(area = 0.5.mm2, power = 100.mW)
module Accelerator { }
```

## Seviye 5 — "Sertifikasyon" (nadiren)

```volt
@version("1.0.0")
@dft(coverage_target = 0.99)
```

---

# BÖLÜM III — VARSAYILANLAR TABLOSU

Hiçbir şey yazılmadığında ne olur:

| Kavram | Varsayılan | Kullanıcı Görür mü? |
|---|---|---|
| Domain | `clock` tipinden otomatik | Hayır |
| Reset senkronluğu | `sync` | Hayır |
| Reset polaritesi | `active_high` | Hayır |
| Reset sinyali | Otomatik `rst` portu | SV çıktısında |
| Güç alanı | Tek alan, always-on | Hayır |
| Trust level | `public` | Hayır |
| Register başlangıç | `= 0` yazılmazsa hata | Evet (açık) |
| Kontratlar | Yok | Hayır |
| Optimizasyon modu | `structure` (ECO-uyumlu) | Hayır |
| Hedef | Verilator lint | Hayır |
| Bit genişliği | Porttan/atamadan çıkarım | Hayır |

**Kural:** Varsayılan davranış her zaman en güvenli ve en yaygın olandır.

---

# BÖLÜM IV — HATA MESAJI ANAYASASI

## Zorunlu Yapı

Her hata mesajı beş parçadan oluşur:

```
error[KOD]: ne oldu (bir cümle, teknik jargonsuz)
  ┌─ dosya:satır:sütun
  │
N │ ilgili kod satırı
  │ ^^^^ tam olarak nerede
  │
  = neden: bu neden bir sorun (bir cümle)
  = çözüm: ne yapmalı (kod örneği ile)
  = daha fazla: volt explain KOD
```

## Örnekler

**Kötü (cezalandırıcı):**
```
error: type mismatch in assignment
  expected `Signal<SlowDomain, UInt<8>>`, found `Signal<FastDomain, UInt<8>>`
```

**İyi (destekleyici):**
```
error[E3001]: iki farklı saat alanı doğrudan bağlanamaz
  ┌─ design.volt:12:14
  │
12│     result = data
  │              ^^^^ 'data' → fast_clk alanında (satır 4)
  │     ^^^^^^ 'result' → slow_clk alanında (satır 5)
  │
  = neden: sinyal kararsız bir anda yakalanabilir
  = çözüm: result = sync(data, slow_clk)
  = daha fazla: volt explain E3001
```

## Hata Mesajı Yasakları

```
✗ İç terminoloji: "HIR düğümü", "lowering hatası", "MLIR dialect"
✗ Yığın izi (stack trace) kullanıcıya gösterilmesi
✗ "beklenmeyen hata" (her hata beklenmiş olmalı)
✗ Çözüm önermeden hata verme
✗ Aynı anda 50 hata gösterme (ilk 5 + "42 hata daha")
✗ Suçlayıcı dil: "yanlış yaptınız", "geçersiz kullanım"
```

## Hata Kurtarma

Bir hata diğerlerini gizlememeli:

```
Kötü:
  Satır 5'te sözdizimi hatası → derleyici durur
  Satır 20'deki gerçek sorun görülmez

İyi:
  Satır 5'te hata → hata kaydedilir, ayrıştırma devam eder
  Tüm hatalar tek seferde gösterilir
  Kullanıcı bir turda hepsini düzeltir
```

---

# BÖLÜM V — ARAÇ DENEYİMİ

## Tek Komut İlkesi

```bash
volt run design.volt
```

Bu tek komut:
1. Derler
2. Hata varsa gösterir ve durur
3. Simüle eder
4. Sonucu gösterir

**Kurulum gerektirmez.** Verilator yoksa yerleşik simülatör kullanır.

## Playground İlk Deneyim

```
play.volt-lang.org açılır:

┌─────────────────────────┬──────────────────────┐
│ module Counter {        │  Üretilen SV:        │
│   in  clk : clock       │  module Counter (    │
│   out count : u8        │    input clk,        │
│                         │    input rst,        │
│   reg count = 0         │    output [7:0] cnt  │
│   on clk {              │  );                  │
│     count <= count + 1  │  ...                 │
│   }                     │                      │
│ }                       │  Dalga formu: ▁▂▃▄▅  │
└─────────────────────────┴──────────────────────┘

Örnek kod ZATEN yüklü. Kullanıcı "Çalıştır"a basar.
Boş editör = kaybedilmiş kullanıcı.
```

## LSP: Öğreten Editör

```
Hover davranışı:
  'clock' üzerine gel → "Saat sinyali. Domain otomatik oluşturulur."
  'reg' üzerine gel   → "Kaydedici. Saat kenarında güncellenir."
  '<=' üzerine gel    → "Kaydedici ataması (saat kenarında)"
  '=' üzerine gel     → "Sürekli atama (kombinasyonel)"

Otomatik tamamlama:
  'on ' yazınca → mevcut saat sinyalleri listelenir
  'sync(' yazınca → uygun hedef domain önerilir

Anlık uyarı (hata değil):
  Kullanılmayan sinyal → gri renk, '_' önerisi
  Bit genişliği daralması → sarı altı çizgi
```

---

# BÖLÜM VI — BELGELEME MİMARİSİ

## Katmanlı Yapı

```
1. "60 Saniyede Volt"      → tek sayfa, tek örnek, çalışıyor
2. "İlk Tasarımınız"       → 15 dakika, UART TX
3. "Yaygın Desenler"       → FIFO, FSM, arbiter (kopyala-yapıştır)
4. "İki Saat Alanı"        → CDC ilk kez burada geçer
5. "Doğrulama"             → invariant ilk kez burada
6. "FPGA'ya Yükleme"       → Vivado/Quartus entegrasyonu
7. "ASIC'e Hazırlık"       → @budget, @timing
8. "Referans"              → tam dil grameri
9. "İleri Konular"         → lineer tipler, timeline, hook
10. "AI Donanımı"          → Trit ilk kez burada
```

**Kural:** Bir kavram, ihtiyaç duyulacağı bölümden önce geçmez.

## Verilog'dan Gelenler İçin

```markdown
# Verilog'dan Volt'a: 10 Dakikada Geçiş

| Verilog | Volt |
|---|---|
| `always @(posedge clk)` | `on clk { }` |
| `reg [7:0] x;` | `reg x : u8 = 0` |
| `wire [7:0] y;` | `let y : u8 = ...` |
| `x <= y;` | `x <= y` (aynı) |
| `assign z = a & b;` | `z = a & b` |

Bilmeniz gereken 3 fark:
1. Sensitivity list yok — otomatik
2. Register başlangıç değeri zorunlu (= 0)
3. Farklı saatler karıştırılamaz (derleme hatası)
```

---

# BÖLÜM VII — V3 ÖZELLİKLERİNİN YENİDEN DEĞERLENDİRİLMESİ

Bu ilkeler ışığında v3'teki bazı kararlar değişmeli:

## Değişiklik 1: `domain` Bloğu Opsiyonel Oldu

```
ÖNCE (v3):
  domain SysDomain { clock = posedge, reset_sync = sync, ... }
  in data : u8 @SysDomain
  → Her tasarım 10 satır boilerplate ile başlıyor

SONRA (v3.1):
  in clk  : clock
  in data : u8              // saat tek ise domain gereksiz
  → domain sadece çoklu alan veya özelleştirme gerektiğinde

Domain sistemi TAM OLARAK KORUNUYOR, sadece görünmüyor.
```

## Değişiklik 2: Lineer Tipler Varsayılan Değil

```
ÖNCE (v3):
  Tüm portlarda lineer tip kontrolü
  → "port zaten tüketildi" hatası yeni kullanıcıyı şaşırtır

SONRA (v3.1):
  Sadece &inv işaretli portlarda (açık opt-in)
  Normal portlar: çift sürücü kontrolü (E4001) yeterli
  → Lineer tipler ileri kullanıcı özelliği
```

## Değişiklik 3: Adlandırılmış Blok Sonlandırma Opsiyonel

```
ÖNCE (v3):
  } module Counter    // zorunlu tekrar

SONRA (v3.1):
  }                   // yeterli
  } module Counter    // opsiyonel, LSP kontrol eder
  → Fazladan yazma zorunluluğu = ceza
```

## Değişiklik 4: Kontratlar Görünmez Başlar

```
volt check design.volt

ÖNCE (v3):
  ⚠ Bu modülde kontrat yok
  ⚠ @budget tanımlanmamış
  ⚠ DFT kontratı eksik
  → Kullanıcı suçlanmış hissediyor

SONRA (v3.1):
  ✓ Tip kontrolü: temiz
  ✓ Saat alanları: 1 alan, sorun yok
  → Sessiz başarı. Kontrat yoksa bahsedilmez.
```

## Değişiklik 5: Reset Portu Otomatik

```
Kullanıcı yazmıyor:
  module Counter {
      in clk : clock
      // rst portu yok!
  }

Volt üretiyor:
  module Counter (
      input clk,
      input rst,      // ← otomatik eklendi
      ...
  );

Kullanıcı reset davranışını değiştirmek isterse:
  in rst : reset(async, active_low)   // açık kontrol
```

---

# BÖLÜM VIII — ÖĞRENME EĞRİSİ HEDEFLERİ

| Aşama | Süre | Bilmesi gereken |
|---|---|---|
| İlk çalışan tasarım | 60 saniye | Kopyala-yapıştır |
| Kendi sayacını yazma | 15 dakika | module, in/out, reg, on |
| UART TX | 1 saat | + FSM, koşullar |
| İki saat alanı | 2 saat | + sync() |
| FPGA'ya yükleme | 3 saat | + Volt.toml |
| Formal doğrulama | 1 gün | + invariant |
| ASIC hazır tasarım | 1 hafta | + @budget, @timing |
| İleri özellikler | İhtiyaç halinde | lineer tip, timeline, hook |

**Verilog bilen için:** 30 dakikada üretken olmalı.

---

# BÖLÜM IX — "CEZALANDIRMA" TESTLERİ

Her özellik eklenirken sorulacak sorular:

```
1. Bu özellik olmadan tasarım derleniyor mu?
   Hayır → özellik zorunlu → CEZALANDIRICI → yeniden tasarla

2. Bu özelliği bilmeyen kullanıcı hata alır mı?
   Evet → hata mesajı çözümü gösteriyor mu?
   Hayır → CEZALANDIRICI

3. Bu özellik ilk 15 dakikada karşılaşılıyor mu?
   Evet → Seviye 0'a ait mi?
   Hayır → gizlenmeli

4. Boilerplate ekliyor mu?
   Evet → varsayılan ile kaldırılabilir mi?
   Hayır → gerekçe ADR'de belgelensin

5. Hata mesajı "volt explain" ile açıklanabiliyor mu?
   Hayır → mesaj yeniden yazılsın
```

---

# BÖLÜM X — CLAUDE.md UX KURALLARI

```markdown
## UX KURALLARI (İHLAL EDİLEMEZ)

### Varsayılan Davranış
- Her yeni özellik varsayılan olarak KAPALI veya OTOMATİK olmalı
- Hiçbir özellik kullanıcıdan ek yazım talep etmemeli
- Boilerplate ekleyen değişiklik reddedilir

### Hata Mesajları
Her hata mesajı ZORUNLU olarak içerir:
  1. Hata kodu (E3001)
  2. Tek cümlelik açıklama (jargonsuz)
  3. Kaynak konumu + snippet
  4. "= neden:" satırı
  5. "= çözüm:" satırı (kod örneği ile)
  6. "= daha fazla: volt explain KOD"

Eksik olan mesaj MERGE EDİLEMEZ.

### Yasak Kalıplar
- İç terminoloji hata mesajında (HIR, MLIR, lowering)
- Çözüm önermeyen hata
- 5'ten fazla hatayı tek seferde gösterme
- "Bu özelliği kullanmak için önce X öğrenin"
- Kontrat/annotation eksikliği için uyarı

### Test Zorunluluğu
tests/ux/ klasöründe:
  - hello_world.volt: 10 satırdan az, çalışmalı
  - Her hata kodu için: mesaj snapshot testi
  - Her hata mesajı "çözüm" satırı içermeli (otomatik kontrol)
```

---

# ÖZET: DEĞİŞEN ONBEŞ KARAR

```
1.  domain bloğu → OPSİYONEL (clock tipi yeterli)
2.  @Domain anotasyonu → OPSİYONEL (tek alan varsa)
3.  reset portu → OTOMATİK (yazılmazsa üretilir)
4.  Lineer tipler → OPT-IN (&inv ile)
5.  Blok sonlandırma ismi → OPSİYONEL
6.  Kontratlar → SESSİZ (yoksa bahsedilmez)
7.  Trit → OPT-IN import + belgede Bölüm 10
8.  Kontrat uyarıları → KALDIRILDI
9.  Bit genişliği → ÇIKARIM (mümkün olduğunca)
10. sync() fonksiyonu → BASİT ARAYÜZ (TwoFlop<T,S,D> yerine)
11. volt run → TEK KOMUT (derle+simüle)
12. volt explain → HER HATA KODU için
13. Playground → ÖRNEK YÜKLÜ başlar
14. Belgeleme → 10 katman, kavram ihtiyaçtan önce geçmez
15. Hata mesajı → 5 parça ZORUNLU

Kural:
  Güçlü özellikler KORUNDU.
  Görünürlükleri ihtiyaca bağlandı.
  Hiçbiri zorunlu değil.
```
