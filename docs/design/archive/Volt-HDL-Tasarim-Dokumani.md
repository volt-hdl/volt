> UYARI: Bu belge geçersizdir.
> Güncel sürüm: docs/design/ altında.

# Volt HDL — Yeni Nesil Donanım Tanımlama Dili Tasarım Dokümanı

> **Çalışma adı:** Volt (elektrik/elektronik teması; nihai adlandırma açık)
> **Amaç:** Verilog/SystemVerilog, VHDL, Chisel, SpinalHDL, Bluespec ve Clash'in
> güçlü yönlerini birleştiren; zayıf yönlerini dil seviyesinde ortadan kaldıran;
> öğrenme eğrisi düşük, geliştirici dostu ve endüstride benimsenebilir bir HDL.

---

## 0. Yönetici Özeti

Mevcut HDL ekosistemi iki kutba sıkışmıştır:

1. **Endüstri standardı diller (Verilog/SV, VHDL):** Olgun araç desteği var ama
   zayıf tip güvenliği, gizli davranışlar (sensitivity list, blocking/non-blocking),
   simülasyon/sentez uyumsuzlukları ve X-propagasyon gibi tuzaklarla dolu.
2. **Modern gömülü diller (Chisel, SpinalHDL, Clash, Bluespec):** Güçlü soyutlama
   ve üretkenlik sunar ama bir ev sahibi dile (Scala/Haskell) ve onun öğrenme
   eğrisine, derleme zincirine (JVM/FIRRTL) ve kriptik hata mesajlarına bağımlıdır.

**Volt'un tezi:** Donanımı *donanım gibi* modelleyen, ama yazılım dünyasının en iyi
araçlarını (güçlü tip sistemi, mükemmel hata mesajları, LSP, tek derleyici) yerleşik
sunan **bağımsız, amaca özel (purpose-built) bir dil** gereklidir. Volt; saat alanını,
reset stratejisini, bit genişliğini ve işaretliliği **tipin parçası** yapar ve böylece
en pahalı donanım hatalarını derleme zamanında imkânsız kılar.

---

## 1. Temel Tasarım Felsefesi

### 1.1 Beş Çekirdek Prensip

| Prensip | Açıklama |
|---|---|
| **Açıklık (No Hidden Behavior)** | Sentezlenen donanımı etkileyen hiçbir şey örtük olmamalı. Sensitivity list yok, örtük latch yok, örtük tür genişletme yok. Kod ne diyorsa donanım odur. |
| **Tek Semantik Model** | Simülasyon ve sentez **aynı** anlambilimi paylaşır. "Simülasyonda çalıştı, çipte çalışmadı" sınıfı hatalar tasarımca imkânsızdır. |
| **Güvenli Varsayılanlar** | En yaygın durum (senkron RTL, kayıtlı reset, tam genişlik aritmetiği) varsayılan olarak güvenlidir. Tehlikeli işlem yapmak için bilinçli, görünür sözdizimi gerekir. |
| **Tipler Donanımı Modeller** | Genişlik, işaretlilik, saat alanı ve birim, tip sisteminin birinci sınıf parçalarıdır. Tip kontrolü = donanım doğruluğu kontrolüdür. |
| **Yerel Akıl Yürütme (Local Reasoning)** | Bir modülü anlamak için global bağlam gerekmez. Bu hem insan okuyucu hem de LLM üretimi için kritiktir. |

### 1.2 Donanım Düşüncesi ⇄ Yazılım Deneyimi Dengesi

Volt, "yazılım gibi yaz, donanım çıksın" yanılgısına **düşmez**. HLS (yüksek
seviyeli sentez) yaklaşımlarının başarısızlık nedeni, geliştiriciden donanım
gerçekliğini gizlemeleridir. Volt'un dengesi şöyledir:

- **Donanım modeli korunur:** Geliştirici hâlâ register, wire, saat ve paralel
  blok düşünür. Üretilen RTL tahmin edilebilirdir.
- **Yazılım deneyimi ödünç alınır:** Tip çıkarımı, ifade odaklı sözdizimi, modül
  jeneratörleri, paket yöneticisi, LSP, biçimlendirici ve test çatısı *araç*
  katmanından gelir — anlambilimi değiştirmez.

> Slogan: **"Donanım anlambilimi, yazılım ergonomisi."**

### 1.3 Hedef Kullanıcı Kitlesi (Öncelik Sırasıyla)

1. **FPGA geliştiricileri ve gömülü mühendisler** — en geniş ve en az hizmet
   alan kitle; üretkenlik ve hata önleme onlar için doğrudan zaman tasarrufudur.
2. **Öğrenciler ve yeni başlayanlar** — düşük öğrenme eğrisi ve öğretici hata
   mesajları benimseme için tohumdur (üniversite müfredatı = uzun vadeli kazanç).
3. **ASIC tasarım/doğrulama ekipleri** — formal-dostu anlambilim, güçlü
   doğrulama ve CDC güvenliği bu ekiplerin en pahalı hatalarını hedefler.
4. **AI donanım tasarımcıları ve araştırmacılar** — parametrik jeneratörler ve
   LLM-dostu yapı, sistolik diziler/NoC gibi tasarımları hızlandırır.

Bu sıralama bilinçlidir: **önce FPGA topluluğunu kazanmak**, sonra yukarı doğru
(ASIC) ölçeklenmek, geçmişte başarısız olan "önce ASIC'i ikna et" stratejisinden
daha gerçekçidir.

---

## 2. Dil Sözdizimi ve Kullanılabilirlik

### 2.1 Sözdizimi İlhamı ve Gerekçesi

| Kaynak | Alınan özellik | Neden |
|---|---|---|
| **Rust** | Güçlü tipler, ifade odaklılık, `match`, mükemmel hata mesajları, `Result`/sonuç tipleri | Donanım için "güvenlilik" kültürü; topluluk hata mesajı standardını yükseltti |
| **Swift/Python** | Okunabilir, az noktalı virgül, temiz blok yapısı | Yeni başlayan dostu, düşük görsel gürültü |
| **SpinalHDL** | Birinci sınıf `ClockDomain`, `Bundle`, akış (Stream/Flow) soyutlamaları | Donanıma özgü doğru soyutlamalar |
| **Bluespec** | Guarded atomic actions (opsiyonel üst katman) | Karmaşık kontrol mantığında yarış durumlarını ortadan kaldırır |
| **VHDL** | Açık ve katı anlambilim disiplini | Belirsizliği azaltır |

Volt **C-benzeri Verilog gürültüsünden** (begin/end, sürekli `reg`/`wire` ayrımı,
genişlik uyumsuzluğunun sessiz kabulü) ve **Scala/Haskell ev sahibi dil
yükünden** kasıtlı olarak kaçınır.

### 2.2 Okunabilirlik ⇄ İfade Gücü Dengesi

- **Yaygın işler kısa:** Sayaç, FSM, pipeline için birinci sınıf sözdizimi var
  (Bölüm 3). Standart bir kayıtlı sayaç 3 satırdır.
- **Karmaşık işler mümkün:** Tam parametrik jeneratörler, derleme zamanı
  hesaplama ve tip seviyesinde genişlik aritmetiği desteklenir — ama bunlar
  ayrı, görünür bir katmandadır (`gen` blokları), her satıra sızmaz.

İlke: **basit şeyler basit, karmaşık şeyler mümkün, tehlikeli şeyler görünür.**

### 2.3 Yeni Başlayan Hatalarını Önleme

| Klasik hata | Volt'un çözümü |
|---|---|
| Örtük latch oluşturma (eksik `else`) | Kombinasyonel blokta her dalın atama yapması **zorunlu**; eksikse derleme hatası ("bu sinyal X yolunda atanmamış"). |
| Blocking vs non-blocking karışıklığı | İki kavram yok. Kombinasyonel atama `=`, kayıtlı güncelleme `<=` sadece `on <domain>` bloğunda geçerli; karıştırmak derleme hatası. |
| Genişlik uyumsuzluğu sessiz kesme | Genişlikler tip kontrolünden geçer; kayıp olabilecek atama açık `truncate()`/`resize()` ister. |
| Sentezlenmeyen kod yazma | Sentezlenebilir alt küme **varsayılan**; test/doğrulama yapıları ayrı `test`/`assert` bağlamına izolelidir. |
| Latch'lı/asenkron reset kazaları | Reset stratejisi saat alanı tipinde belirtilir; modülde tutarsız reset = derleme hatası. |

### 2.4 Sezgisel ve Kendi Kendini Açıklayan Sözdizimi

```volt
// Bir saat alanı, donanım gerçekliğini açıkça belirten bir tiptir.
domain Sys {
    clock = posedge,        // saat kenarı
    reset = sync active_high // senkron, aktif-yüksek reset
}

module Counter {
    in  enable : bool
    out count  : u8

    // 'reg' yalnızca bir saat alanında yaşar; reset değeri zorunludur.
    reg(Sys) value : u8 = 0

    on Sys {
        if enable {
            value <= value +| 1   // doygunluklu toplama; taşma davranışı AÇIK
        }
    }

    count = value   // kombinasyonel bağlantı
}
```

Burada her satır kendini açıklar: `reg(Sys)` → "bu Sys alanında bir kayıt",
`+|` → "doygunluklu toplama", `<=` → "saat kenarında güncelle". Hiçbir gizli
sensitivity list veya örtük genişlik genişletmesi yoktur.

---

## 3. Donanım Modelleme Yaklaşımı

### 3.1 Çok Seviyeli Modelleme

Volt tek bir çekirdek IR üzerine **kademeli soyutlama** sunar:

- **RTL seviyesi:** `reg`, `wire`, `on <domain>` blokları (yukarıdaki örnek).
- **Dataflow seviyesi:** `Stream<T>` / `Flow<T>` tipleri ve `>>` operatörü ile
  geri-basınçlı (back-pressure) boru hatları.
- **Davranışsal/transaction seviyesi:** Opsiyonel `rule` blokları (Bluespec
  esinli guarded atomic actions) — derleyici çelişen kuralları çözer ve **yarış
  durumu olmayan** kontrol mantığı üretir.

Tüm seviyeler aynı IR'ye düşer; karıştırılabilir ve aralarındaki sınır açıktır.

### 3.2 Eşzamanlılığın Sadeleştirilmesi

Donanım doğası gereği paraleldir. Volt bunu **modül kapsamı** ile sadeleştirir:
modül içindeki tüm `on`/`rule` blokları kavramsal olarak eş zamanlı çalışır; bir
sinyalin **tek bir sürücüsü** olmak zorundadır (multiple-driver = derleme hatası).
Böylece "kim bu wire'ı sürüyor?" belirsizliği ortadan kalkar.

### 3.3 Saat Alanları, Reset ve CDC

Bu, Volt'un **en ayırt edici** özelliğidir:

```volt
domain Fast { clock = posedge }
domain Slow { clock = posedge }

module Bridge {
    in  flag_fast : bool @Fast    // sinyalin alanı tipinin parçası
    out flag_slow : bool @Slow

    // Doğrudan bağlamak DERLEME HATASIDIR: alanlar uyuşmuyor.
    // flag_slow = flag_fast   ❌

    // Geçiş için açık bir senkronizör gerekir:
    flag_slow = cdc::sync_2ff(from = Fast, to = Slow, flag_fast)  ✅
}
```

- **Saat alanı tipin parçasıdır** (`bool @Fast`). Bir alandan diğerine sinyal
  geçirmek ancak onaylanmış bir CDC primitifi ile mümkündür.
- **Reset stratejisi** alan tanımında merkezîdir; tek tek register'larda tutarsız
  reset yazmak imkânsızdır.
- Çok-bit CDC için derleyici gri-kod / handshake gerektirir ve naif geçişleri reddeder.

Bu, sektörde silikon dönüşü maliyetine yol açan en sinsi hata sınıfını
(metastabilite/CDC) **dil seviyesinde** kapatır.

### 3.4 Pipeline, FSM, Cache, NoC, Veri Yolu

**FSM** birinci sınıf yapıdır — durum kodlaması, geçiş ve "ulaşılamaz durum"
kontrolü derleyici tarafından sağlanır:

```volt
fsm Traffic on Sys {
    state Red, Green, Yellow
    init  Red

    Red    => Green  after 50.cycles
    Green  => Yellow after 40.cycles
    Yellow => Red    after 5.cycles
}
```

**Pipeline** otomatik register ekleme + valid/stall mantığı üretir:

```volt
pipeline Mac on Sys {
    stage mul: let p   = a * b
    stage add: let acc = p + c     // önceki aşama otomatik register'lanır
    out result = acc
}
```

**NoC/veri yolu** için standart kütüphane `Stream`/`Flow` ve hazır arbiter,
FIFO, crossbar jeneratörleri sağlar; cache için parametrik `Memory<>` ve
set-associative şablonları sunar. Hepsi parametriktir (Bölüm 5).

---

## 4. Tip Sistemi

### 4.1 Tasarım

Volt'un tip sistemi **donanım kaynaklarını kodlar**, yalnızca bit kalıplarını değil:

- **Genişlik tiptir:** `u8`, `i16`, `bits<12>`, `bits<N>` (parametrik).
- **İşaretlilik tiptir:** `u*` (unsigned) ile `i*` (signed) farklı tiplerdir;
  karıştırma açık dönüşüm ister.
- **Saat alanı tiptir:** `T @Domain` (Bölüm 3.3).
- **Birim/anlamsal tip:** `newtype Addr = u32`, `newtype Volts = fixed<8,4>` —
  yanlışlıkla adres ile veri toplamayı engeller.

### 4.2 Genişlik / İşaretlilik / Taşma Hatalarının Önlenmesi

| Senaryo | Volt davranışı |
|---|---|
| `u8 + u8` | Sonuç tipi `u9` (genişleme açık); `u8`'e atamak `truncate()`/`+%` (wrapping) gerektirir. |
| `signed + unsigned` | Derleme hatası; açık `as i9` dönüşümü gerekir. |
| `u8`'e `u12` atama | Derleme hatası: "olası veri kaybı, `truncate()` veya `resize()` kullanın". |
| Taşma davranışı | `+` (genişleyen), `+%` (wrapping), `+|` (doygunluklu) ayrı operatörlerdir — niyet daima görünür. |

Böylece **sessiz veri kaybı** ve **işaretlilik kazaları** sınıfı tamamen kapanır.

### 4.3 Tür Çıkarımı (Type Inference)

- **Portlar ve register'lar:** Açık tip **zorunlu** (arayüz = sözleşme; okunabilirlik
  ve LLM doğruluğu için kritik).
- **Yerel `let` bağları:** Tip ve genişlik **çıkarılır** (gürültü azalır).
- İlke: *sınırlarda açık, içeride çıkarımlı* (Rust felsefesi). Bu, hem güvenliği
  hem ergonomiyi korur.

### 4.4 Birim, Zamanlama ve Kaynak Tip Güvenliği

- **Birim güvenliği:** `newtype` + sabit ondalık (`fixed<I,F>`) tipleriyle fiziksel
  birim karışıklığı engellenir.
- **Zamanlama tipi (deneysel/opsiyonel):** Bir sinyalin "kaç cycle gecikmeli"
  olduğu (`Delayed<T, 2>`) tip seviyesinde izlenebilir; pipeline hizalama hataları
  derleme zamanında yakalanır.
- **Kaynak tipi:** `Memory`, `DSP`, `LUT` gibi primitifler tiplenmiştir; jeneratörler
  kaynak bütçesini derleme zamanı hesaplamayla raporlayabilir.

---

## 5. Modülerlik ve Yeniden Kullanılabilirlik

### 5.1 Modüller, Jenerikler, Parametrik Tasarım

```volt
// Genişliği ve derinliği parametrik bir FIFO
module Fifo<T, const Depth: usize> {
    in  push : Stream<T>
    out pop  : Stream<T>
    // ... gövde derleme zamanı 'Depth' ile özelleşir
}

// Kullanım — tip ve sabitler açık
let f = Fifo<u16, 32>()
```

- **Const-generics:** Genişlik/derinlik gibi parametreler tip seviyesindedir;
  yanlış boyut bağlantısı derleme hatasıdır.
- **Tip parametreleri:** `Bundle`/struct geçirilebilir; arayüzler jeneriktir.
- **Bileşen kompozisyonu:** Modüller değer gibi örneklenir (`let u = Sub()`),
  arayüzler `>>`/`<>` ile bağlanır; tip uyumsuzluğu anında yakalanır.

### 5.2 Büyük Projelerde Sürdürülebilir Mimari

- **Paket sistemi + paket yöneticisi** (`volt.toml`): sürüm sabitleme,
  bağımlılık çözümü, IP kataloğu.
- **Arayüz (interface) tipleri:** AXI, AXI-Lite, Wishbone gibi protokoller
  standart kütüphanede tek-yönlü/iki-yönlü `Bundle` olarak tanımlı; `master`/`slave`
  yönleri tipte kodlu, yanlış bağlantı imkânsız.
- **Görünürlük kontrolü:** `pub`/özel ayrımı; iç sinyaller arayüze sızmaz.

### 5.3 IP Entegrasyonu ve Üçüncü Taraf Kütüphaneler

- **Yerel IP:** Volt paketleri doğrudan import edilir, tip-güvenli bağlanır.
- **Yabancı IP (Verilog/VHDL kara kutu):** `extern module` ile imza beyan edilir;
  Volt arayüz tiplerini sınırda zorlar, içeriyi opak bırakır.
- **Dışa aktarım:** Volt derleyicisi temiz, okunabilir, satıcı-bağımsız Verilog-2005
  ve VHDL üretir → mevcut akışlara sürtünmesiz girer.

---

## 6. Doğrulama ve Test

### 6.1 Basitleştirilmiş Testbench

Test, dilin **birinci sınıf** parçasıdır; ayrı bir simülatör DSL'i öğrenmeye gerek yok:

```volt
test "sayaç enable ile artar" {
    let dut = Counter()
    dut.enable = true
    tick(Sys, 5)              // 5 saat kenarı ilerlet
    expect dut.count == 5
}
```

`tick`, `expect`, `drive`, `monitor` gibi yapılar yerleşiktir; rasgele/constrained-random
uyarım için `forall x: u8 in 0..256 { ... }` desteklenir.

### 6.2 Yerleşik Doğrulama Özellikleri

Evet — doğrulama dilin parçası olmalıdır (SystemVerilog'un SVA gücünü, ama temiz
sözdizimiyle):

```volt
// Eşzamanlı (temporal) assertion — istek mutlaka onay almalı
assert on Sys: req |=> eventually(ack within 8.cycles)

// Değişmez (invariant)
invariant on Sys: !(write && read)   // aynı anda iki işlem olamaz
```

### 6.3 Assertion, Formal Verification, Property Checking

- **Tek anlambilim → formal-dostu:** Yerel akıl yürütme ve örtük davranışın
  olmaması, modelin formal araçlara (model checking) doğrudan beslenebilmesini sağlar.
- **`assert`/`invariant`/`cover`** doğrudan formal motorlara çevrilir (yerleşik
  bounded model checker + SymbiYosys gibi açık araçlara köprü).
- **Property kütüphanesi:** Yaygın özellikler (one-hot, handshake doğruluğu, FIFO
  taşma yok) hazır gelir.

### 6.4 Geliştirici Dostu Simülasyon/Doğrulama

- **Tek komut:** `volt test` → derleme + simülasyon + assertion + kapsama raporu.
- **Hızlı simülatör:** IR'den doğrudan native koda derlenen olay tabanlı simülatör
  (Verilator hızında, ama kurulumsuz).
- **Dalga formu:** `volt test --wave` ile VCD/FST; assertion ihlali otomatik olarak
  ilgili zaman damgasını ve sinyal yolunu işaretler.

---

## 7. Hata Önleme ve Geliştirici Deneyimi

### 7.1 Derleyicinin Erken Yakaladığı Hatalar

Derleme zamanında (silikon/bitstream öncesi) yakalanan sınıflar:

- Genişlik/işaretlilik uyumsuzlukları ve sessiz kesmeler
- Örtük latch ve eksik atama yolları
- Çoklu sürücü çatışmaları
- CDC ihlalleri ve uyumsuz saat alanı bağlantıları
- Ulaşılamaz/eksik FSM durumları, one-hot ihlalleri
- Birleşimsel döngüler (combinational loops)
- Reset stratejisi tutarsızlıkları
- Pipeline hizalama/gecikme uyumsuzlukları (zamanlama tipiyle)

### 7.2 Hata Mesajı Tasarımı (Elm/Rust Standardı)

```
hata[E0412]: olası veri kaybı içeren atama
  ┌─ alu.volt:14:9
  │
14│     out_byte = sum
  │                ^^^ bu ifade 'u9' tipinde (genişlik 9)
  │     -------- hedef 'out_byte' 'u8' tipinde (genişlik 8)
  │
  = neden: 'u8 + u8' toplamı taşmayı korumak için 'u9' üretir.
  = öneri: niyetini açıkça belirt:
        out_byte = sum.truncate()   // üst biti at
        out_byte = sum +| 0         // doygunlukla sınırla
        out_byte : u9               // hedefi genişlet
```

Her hata: **konum + neden + somut düzeltme önerisi** içerir. Hata kodları
(`E0412`) belgelendirilir ve aranabilir.

### 7.3 IDE Entegrasyonu

- **Resmî LSP sunucusu** (gün-1): otomatik tamamlama, tipte gezinme, anlık tanı,
  yeniden adlandırma (refactor), modül/ port önizleme.
- **Biçimlendirici** (`volt fmt`) ve **linter** (`volt clippy` benzeri stil/donanım
  uyarıları) standart araç zincirinde.
- **Şema görünümü:** LSP, bir modülün bağlantı/blok şemasını canlı üretebilir.

### 7.4 Kullanıcıyı Yanlış Tasarımdan Koruma

Volt'un felsefesi: **"yanlış kod derlenmemeli"**. Tehlikeli işlem (truncate,
asenkron geçiş, çoklu sürücü, manuel saat kapısı) ya imkânsızdır ya da açık,
görünür ve gerekçelendirilmesi gereken bir sözdizimi ister. "Güvenli yol = kolay
yol" ilkesi her tasarım kararına uygulanır.

---

## 8. Yapay Zekâ ve Gelecek Uyumluluğu

### 8.1 AI Destekli Geliştirme İçin Optimizasyon

LLM'ler için zorluk, Verilog/SV'nin **bağlam bağımlı, örtük ve hata-toleranslı
sessiz** doğasıdır — model yanlış genişlik üretir, hiçbir uyarı çıkmaz, hata
silikona kadar gizlenir. Volt bunu tersine çevirir.

### 8.2 LLM'in Doğru HDL Üretmesini Kolaylaştıran Yapısal Özellikler

| Özellik | LLM için faydası |
|---|---|
| **Düzenli, bağlamdan-bağımsız gramer** | Tahmin edilebilir token dağılımı; daha az sözdizimi halüsinasyonu. |
| **Yerel akıl yürütme** | Model tek modülü doğru üretmek için tüm projeyi "hatırlamak" zorunda değil. |
| **Güçlü tipler + zengin hata mesajları** | Üretim → derleme → hata → düzeltme döngüsü (agentic loop) güvenilir; derleyici, modelin halüsinasyonunu somut öneriyle düzeltir. |
| **Açık niyet operatörleri (`+%`, `+|`)** | Belirsizlik yok; model taşma davranışını kasıtlı seçer. |
| **Tek anlambilim** | "Simülasyonda çalışan ama sentezlenmeyen" örnekler eğitim setini kirletmez. |

Volt'un derleyici tanıları, makine-okunur JSON formatında da yayınlanır
(`volt build --message-format=json`) → AI ajanları için doğrudan geri bildirim kanalı.

### 8.3 Otomatik Optimizasyon ve Tasarım Keşfi

- **Parametrik jeneratörler + derleme zamanı hesaplama**, tasarım uzayı keşfini
  (design space exploration) doğal kılar: bir jeneratörü farklı `const` parametreleriyle
  süpürüp PPA (güç/performans/alan) raporlarını karşılaştırmak tek komuttur.
- Temiz IR, otomatik retiming/pipeline-balancing pas'larına (passes) ve gelecekteki
  ML-tabanlı optimize edicilere uygundur.

---

## 9. Performans ve Sentez

### 9.1 Üretilen RTL'nin Verimliliği

- **Şeffaf lowering:** Volt → IR → RTL dönüşümü belgelidir ve tahmin edilebilir;
  geliştirici hangi yapının ne ürettiğini bilir (HLS'in "kara kutu" sorununun tersi).
- **Sıfır-maliyet soyutlama hedefi:** `Stream`, `fsm`, `pipeline` gibi soyutlamalar
  elle yazılmış RTL'den daha kötü olmayan donanım üretir (Rust'ın "zero-cost
  abstractions" ilkesinin donanım karşılığı).

### 9.2 Soyutlamayı Artırırken Sentez Kalitesini Koruma

- **Kaçış kapakları (escape hatches):** Kritik yolda geliştirici aşama sınırlarını,
  kodlamayı (binary/one-hot/gray) ve kaynak eşlemesini (`@dsp`, `@bram`) elle
  yönlendirebilir.
- **Pragma yerine tip/öznitelik:** Optimizasyon ipuçları tiplenmiş özniteliklerdir,
  araç-özel yorum-pragma'lar değil → taşınabilir ve doğrulanabilir.

### 9.3 FPGA ⇄ ASIC Dengesi

- **Hedef-bağımsız çekirdek + hedef-özel primitif kütüphanesi:** Aynı kaynak,
  `--target=fpga:xilinx-us+` veya `--target=asic:sky130` ile farklı primitiflere
  (BRAM vs SRAM derleyici, DSP48 vs çarpan hücresi) eşlenir.
- **Reset/saat soyutlaması** her iki dünyada da güvenli; FPGA için global reset,
  ASIC için reset ağacı stratejisi alan tipinden türetilir.
- **Çıktı:** Standart Verilog/VHDL → Vivado, Quartus, Yosys, ve ticari ASIC
  akışlarına (Genus/Design Compiler) sorunsuz girer.

---

## 10. Kaçınılması Gereken Zayıflıklar — ve Volt'un Yanıtı

| Mevcut HDL zayıflığı | Volt'taki çözüm |
|---|---|
| **Dil karmaşıklığı** (SV'nin 1300+ sayfalık LRM'i) | Küçük, tutarlı çekirdek dil; gelişmiş özellikler opsiyonel katmanlarda. |
| **Gizli davranışlar** (sensitivity list, örtük latch) | Sensitivity list yok; eksik atama derleme hatası; her şey açık. |
| **Belirsiz semantik** | Tek, biçimsel olarak tanımlı anlambilim; uygulama-tanımsız davranış yok. |
| **Yarış durumları** | Tek sürücü kuralı + `rule` katmanında atomik anlambilim → simülasyon yarışları yok. |
| **Sessiz veri kaybı** | Genişlik tip kontrolü; kesme/taşma açık operatör ister. |
| **Zayıf tip güvenliği** | Genişlik, işaretlilik, saat alanı, birim — hepsi tipte. |
| **Kötü hata mesajları** | Konum + neden + düzeltme öneren Rust/Elm sınıfı tanılar. |
| **Sim/sentez uyumsuzluğu** | Tek semantik model; sim ve sentez aynı IR'den. |
| **Araç bağımlılığı** | Açık kaynak referans derleyici + standart Verilog/VHDL çıktısı; satıcı kilidi yok. |
| **Ölçeklenebilirlik** | Paket yöneticisi, yerel akıl yürütme, jenerikler, arayüz tipleri. |

---

## 11. Nihai Tasarım Önerisi

### 11.1 Mimari Özeti

```
┌──────────────────────────────────────────────────────────┐
│  Volt Kaynak (.volt)                                       │
│   • RTL · Dataflow(Stream/Flow) · Rule(atomik) katmanları  │
│   • Tipler: genişlik + işaretlilik + saat alanı + birim    │
├──────────────────────────────────────────────────────────┤
│  Volt Derleyici (tek, açık kaynak)                         │
│   1. Tip kontrolü + donanım doğrulukları (CDC, latch, vb.) │
│   2. Volt-IR (biçimsel, tek anlambilim)                    │
│   3. Optimizasyon pas'ları (retiming, balancing)           │
├──────────────────────────────────────────────────────────┤
│  Arka uçlar                                                │
│   • Verilog-2005 / VHDL (sentez akışlarına)                │
│   • Native simülatör (volt test)                           │
│   • Formal köprü (model checking)                          │
│   • Hedef eşleme: fpga:* / asic:*                          │
├──────────────────────────────────────────────────────────┤
│  Araçlar: volt fmt · LSP · paket yöneticisi · JSON tanılar │
└──────────────────────────────────────────────────────────┘
```

### 11.2 Temel Dil Özellikleri

- Saat alanı, bit genişliği, işaretlilik ve birimi kapsayan **güçlü tip sistemi**
- **Tek semantik model** (sim = sentez), biçimsel ve formal-dostu
- Derleme zamanında **CDC, latch, çoklu sürücü, taşma** kontrolü
- Birinci sınıf **`fsm`, `pipeline`, `Stream`/`Flow`** soyutlamaları
- **`rule`** (guarded atomic actions) ile yarışsız kontrol mantığı (opsiyonel)
- **Const-generics + tip parametreleri** ile parametrik jeneratörler
- Yerleşik **test** ve **temporal assertion / invariant** dili
- **Rust/Elm sınıfı hata mesajları** + LSP + biçimlendirici + paket yöneticisi
- Standart **Verilog/VHDL çıktısı** ve hedef-bağımsız çekirdek

### 11.3 Örnek Sözdizimi — Tam Bir Modül

```volt
domain Sys {
    clock = posedge,
    reset = sync active_high
}

/// 4-bit doygunluklu sayaç + tek-sıcak FSM kontrolü
module SatCounter {
    in  cmd   : Cmd          // enum tipinde komut
    out value : u4
    out maxed : bool

    reg(Sys) count : u4 = 0

    enum Cmd { Hold, Inc, Clear }

    on Sys {
        match cmd {
            Cmd::Inc   => count <= count +| 1   // 15'te doygun
            Cmd::Clear => count <= 0
            Cmd::Hold  => {}                    // değişmez (latch DEĞİL, kayıt korunur)
        }
    }

    value = count
    maxed = (count == 15)

    // yerleşik doğrulama
    invariant on Sys: count <= 15
    assert on Sys: (cmd == Cmd::Clear) |=> (count == 0)
}

test "doygunluk 15'te durur" {
    let dut = SatCounter()
    dut.cmd = Cmd::Inc
    tick(Sys, 20)
    expect dut.value == 15
    expect dut.maxed == true
}
```

### 11.4 Verilog/SV, VHDL, Chisel ile Karşılaştırma

| Boyut | Verilog/SV | VHDL | Chisel | **Volt** |
|---|---|---|---|---|
| Öğrenme eğrisi | Orta (ama tuzaklı) | Dik, ayrıntılı | Çok dik (Scala) | **Düşük** |
| Bağımsız dil mi? | Evet | Evet | Hayır (Scala/JVM) | **Evet** |
| Tip güvenliği | Zayıf | Güçlü | Orta-iyi | **Çok güçlü (alan/birim dahil)** |
| Genişlik hatası | Sessiz | Kısmen yakalar | Çoğu yakalanır | **Derleme hatası + öneri** |
| Sim/sentez tutarlılığı | Sorunlu | İyi | İyi | **Tek model — garanti** |
| CDC güvenliği | Manuel/araç | Manuel | Manuel | **Dil seviyesinde zorunlu** |
| FSM/pipeline | Elle | Elle | Kütüphane | **Birinci sınıf sözdizimi** |
| Yerleşik doğrulama | SVA (karmaşık) | Sınırlı | Kütüphane | **Yerleşik, temiz** |
| Hata mesajları | Kötü | Orta | Kriptik (Scala) | **Rust/Elm sınıfı** |
| Araç bağımsızlığı | İyi | İyi | FIRRTL'e bağlı | **Açık IR + std Verilog** |
| LLM-dostu | Düşük | Orta | Düşük | **Yüksek (düzenli, tipli)** |
| Ekosistem olgunluğu | **Çok yüksek** | **Yüksek** | Orta | Yeni (risk) |

> **Dürüst dezavantaj:** Volt'un tek gerçek zayıflığı ekosistem olgunluğudur —
> mevcut diller onyıllarca araç/IP birikimine sahiptir. Volt bunu *standart
> Verilog/VHDL üretip mevcut akışlara girerek* ve *yabancı IP'yi `extern` ile
> sararak* hafifletir; yine de benimseme uzun vadeli bir yatırımdır.

### 11.5 Volt Neden Daha İyi — Teknik Gerekçeler

1. **En pahalı hata sınıflarını derleme zamanında kapatır.** Silikon dönüşüne
   ve haftalarca hata ayıklamaya yol açan CDC, genişlik kesmesi, latch ve çoklu
   sürücü hataları Volt'ta *derlenmez*. Bu, doğrudan ölçülebilir maliyet tasarrufudur.
2. **"Sim'de çalıştı, çipte çalışmadı" mümkün değildir.** Tek semantik model bu
   tüm sınıfı ortadan kaldırır — Verilog'un blocking/non-blocking ve sensitivity
   list tuzaklarının kök nedeni budur.
3. **Üretkenlik soyutlamayı kaliteden ödün vermeden sunar.** `fsm`/`pipeline`/
   `Stream` birinci sınıftır ama şeffaf, tahmin edilebilir RTL'e düşer (HLS'in
   öngörülemezliği yok).
4. **Bağımsız ama sürtünmesiz.** Chisel/Clash'in aksine bir ev sahibi dile (Scala/
   Haskell) ve onun yüküne bağımlı değildir; yine de standart Verilog/VHDL üretip
   mevcut sentez/IP akışlarına girer.
5. **Geleceğe dayanıklı.** Düzenli gramer, güçlü tipler, makine-okunur tanılar ve
   yerel akıl yürütme; Volt'u AI-destekli (üret→derle→düzelt) iş akışları için
   bugünün dillerinden yapısal olarak daha uygun kılar.

### 11.6 Benimseme Stratejisi (Gerçekçilik Notu)

Teorik üstünlük yeterli değildir; benimseme planı tasarımın parçasıdır:

1. **Gün-1 araç olgunluğu:** Açık kaynak derleyici + hızlı simülatör + LSP +
   biçimlendirici. Kötü tooling iyi dilleri öldürür.
2. **FPGA topluluğu önce:** En geniş, en az hizmet alan ve geçiş maliyeti en
   düşük kitle; hızlı kazanımlar burada.
3. **Birlikte çalışabilirlik:** `extern` ile Verilog/VHDL IP sarma + temiz Verilog
   çıktısı → mevcut projelere kademeli giriş (her şeyi yeniden yazma zorunluluğu yok).
4. **Eğitim:** Üniversite müfredatı ve etkileşimli "playground" → yeni nesil
   mühendisler doğrudan Volt öğrenir.
5. **Açık standart:** Dil spesifikasyonu ve IR açık; satıcı kilidi olmaması kurumsal
   güveni artırır.

---

### Kapanış

Volt'un özü tek cümlede: **donanım anlambiliminden ödün vermeden, en pahalı
donanım hatalarını dil seviyesinde imkânsız kılan ve yazılım dünyasının en iyi
geliştirici deneyimini donanıma getiren bir HDL.** Yenilik radikal yeni bir
hesaplama modelinde değil; doğru soyutlamaları güçlü bir tip sistemi, tek bir
temiz anlambilim ve birinci sınıf araçlarla birleştirmektedir — ki mevcut diller
bunların her birini ayrı ayrı *değil*, bir arada sunmakta başarısız olmuştur.
