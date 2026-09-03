> UYARI: Bu belge geçersizdir.
> Güncel sürüm: docs/design/ altında.

# Volt HDL — Birleşik Tasarım ve İnşa Dokümanı

> **Bu doküman iki kaynağın sentezidir:**
> 1. *Volt HDL Tasarım Dokümanı* — somut bir dilin ürün/sözdizimi spesifikasyonu.
> 2. *Yeni Nesil HDL Tasarım Metodolojisi* — derleyici teknolojileri, IP/hukuk ve
>    açık kaynak EDA ekonomisi üzerine inşa stratejisi.
>
> Birincisi **"dil neye benzeyecek"**, ikincisi **"onu nasıl inşa edip koruyup
> ucuza üretiriz"** sorusunu yanıtlar. Bu belge ikisini tek bir tutarlı plan
> hâline getirir ve iki kaynak arasındaki **iki gerçek tasarım çatışmasını**
> gerekçeli kararlarla çözer.

---

## 0. Yönetici Özeti

Volt; donanımı *donanım gibi* modelleyen, ama yazılım dünyasının en iyi araçlarını
(güçlü statik tip sistemi, mükemmel hata mesajları, LSP, tek derleyici) yerleşik
sunan **bağımsız, amaca özel bir HDL**'dir. Saat alanı, reset stratejisi, bit
genişliği, işaretlilik ve **zamanlama**, tipin parçasıdır; böylece en pahalı
donanım hataları (CDC, gizli latch, sessiz veri kaybı, boru hattı yarışları)
derleme zamanında imkânsız kılınır.

Bu birleşik plan üç katmandan oluşur:

| Katman | Kaynak | İçerik |
|---|---|---|
| **Dil** | Belge 1 | Sözdizimi, tip sistemi, modelleme, doğrulama |
| **Derleyici & araç** | Belge 2 | Rust önyüz + MLIR/CIRCT arka uç + LSP + formal semantik |
| **İş & hukuk & ekonomi** | Belge 2 | Temiz oda IP stratejisi, açık kaynak EDA + hibrit broker, ML-tabanlı PPA |

İki kaynak arasındaki çatışmaların kararları (Bölüm 1'de gerekçeli):

1. **Önyüz:** Bağımsız, statik tipli dil **çekirdek** olarak kalır; Python
   **gömülü HDL değil**, jeneratör/doğrulama yardımcısı olarak konumlanır.
2. **Zamanlama tipleri:** Tam Filament/Anvil tarzı timeline tipleri zorunlu
   değil; **kademeli (gradual) zamanlama güvenliği** benimsenir — hafif form
   çekirdekte zorunlu, tam form opt-in ileri katmanda.

---

## 1. İki Belgenin Sentezi: Çözülen Tasarım Çatışmaları

### 1.1 Çatışma A — Önyüz: Bağımsız Dil mi, Gömülü Python mı?

**Gerilim.** Belge 1, ev sahibi dil yükünden (Chisel'in Scala/JVM'i) kaçınmak için
**bağımsız** bir dil savunur. Belge 2 ise MyHDL/Amaranth/PyMTL çizgisinde
**Pythonic bir önyüz** + Rust/C++ arka uç önerir; gerekçesi düşük öğrenme eğrisi,
Python ekosistemi ve referans modelleriyle birleşik doğrulamadır.

**Kök analiz.** Python'a *gömülü* bir HDL'de "dil" aslında Python'dır ve donanım,
çalışma zamanında (elaboration) metaprogramlama ile *inşa edilir*. Bu yaklaşım
Python ekosistemini kazandırır — ama Volt'un tüm değer önerisinin dayandığı şeyi,
yani **yazım anında statik tip denetimini** (genişlik, saat alanı, zamanlama)
yapısal olarak veremez. Çünkü Python dinamik tiplidir; Amaranth gibi araçlar
hataları ancak elaborasyon sırasında, çalışma zamanında yakalar — derleyici, kod
yazılırken kırmızı çizgi çizemez. Volt'un "en pahalı hatalar *derlenmez*" tezi bu
durumda çöker.

**Karar — Hibrit, ama doğru sınırla.** Donanımı *tanımlayan* dil, kendi gramerine
ve statik tip denetleyicisine sahip **bağımsız Volt** olarak kalır. Belge 2'nin
Python'dan gerçekte istediği üç şey ise kaybedilmeden geri kazanılır:

1. **Ergonomi:** Volt'un yüzey sözdizimi bilinçli olarak Python/Swift okunabilirliğinde
   tutulur (az noktalı virgül, temiz blok yapısı, ifade odaklılık).
2. **Jeneratör/metaprogramlama:** Tip-güvenli parametrik tasarımın *ötesinde* tam
   programatik üretim gerektiğinde, donanımı **üreten** komut betikleri için resmî
   bir **Python API**'si sunulur. Python burada donanımı *tanımlamaz*, statik tipli
   Volt IR'sini *üretir* — tüm güvenlik denetimleri yine Volt tarafında işler.
3. **Birleşik doğrulama:** Python referans modelleri ve **cocotb** uyumlu test
   altyapısı birinci sınıf desteklenir; algoritma ekibinin Python modeli, donanım
   testbench'inde doğrudan koşturulabilir.

> **Net sınır:** *Donanımı tanımlayan* her şey statik tipli Volt'tadır; *donanımı
> üreten ve doğrulayan* yardımcı betikler Python olabilir. Böylece hem statik
> güvenceler korunur hem de Python ekosisteminin üretkenliği erişilebilir kalır.

### 1.2 Çatışma B — Zamanlama Tipleri: Opsiyonel mi, Çekirdek mi?

**Gerilim.** Belge 1, gecikme-farkında tipi (`Delayed<T,N>`) "deneysel/opsiyonel"
bırakır. Belge 2 ise Filament (timeline types, olaylar `'G`, initiation interval)
ve Anvil çizgisini **çekirdeğe** koymayı önerir; bu araştırma yönü teknik olarak
daha rigorludur ve yapısal kaynak çakışmalarını (structural hazard) derleme
zamanında eler.

**Kök analiz.** Filament tarzı tam timeline tipleri en güçlü statik güvenceyi
verir — ama her sinyali zaman penceresiyle anotlamak öğrenme eğrisini ciddi
biçimde dikleştirir. Bu, Volt'un birincil hedefiyle (düşük öğrenme eğrisi; önce
FPGA geliştiricileri ve öğrenciler) doğrudan çelişir. "Hep opsiyonel" ise Belge
2'nin haklı eleştirisini görmezden gelir: pipeline hizalama ve kaynak çakışması
hataları çok pahalıdır ve simülasyona/sentez sonrasına bırakılmamalıdır.

**Karar — Kademeli (gradual) zamanlama güvenliği.** İki ucu birleştiren orta yol,
Volt'un "basit şeyler basit, karmaşık şeyler mümkün" ilkesinin zamanlamaya
uygulanmasıdır:

| Seviye | Durum | Ne sağlar |
|---|---|---|
| **L0 — Pipeline hizalama** | **Çekirdekte zorunlu** | `pipeline`/`stage` içinde bir sinyali yanlış aşamada okumak derleme hatasıdır. Otomatik; anotasyon gerekmez. |
| **L1 — Gecikme tipi** | Varsayılan açık, hafif | `Delayed<T, N>`: bir sinyalin kaç cycle gecikmeli olduğu tipte taşınır; hizalama hataları yakalanır. |
| **L2 — Tam timeline tipleri** | **Opt-in** (`#[timeline]`) | Filament/Anvil tarzı olaylar ve initiation interval; agresif kaynak paylaşımında yapısal tehlikeleri sıfıra indirir. Yüksek-güvence/ASIC ekipleri için. |

> Böylece sıradan kullanıcı zamanlama tipi *yazmadan* en yaygın boru hattı
> hatalarından korunur; ileri ekipler ise `#[timeline]` ile tam statik güvenceye
> kademe atlar. Bu, "gradual typing"in donanım karşılığıdır.

---

## 2. Temel Tasarım Felsefesi

### 2.1 Beş Çekirdek Prensip

| Prensip | Açıklama |
|---|---|
| **Açıklık (No Hidden Behavior)** | Sensitivity list yok, örtük latch yok, örtük genişlik genişletme yok. Kod ne diyorsa donanım odur. |
| **Tek Semantik Model** | Simülasyon ve sentez aynı anlambilimi paylaşır; "sim'de çalıştı, çipte çalışmadı" sınıfı imkânsızdır. |
| **Güvenli Varsayılanlar** | Senkron RTL, kayıtlı reset, tam genişlik aritmetiği varsayılan olarak güvenlidir; tehlikeli işlem görünür sözdizimi ister. |
| **Tipler Donanımı Modeller** | Genişlik, işaretlilik, saat alanı, birim ve zamanlama tipin birinci sınıf parçalarıdır. |
| **Yerel Akıl Yürütme** | Bir modülü anlamak için global bağlam gerekmez — insan okuyucu ve LLM üretimi için kritik. |

### 2.2 Bilişsel Borç (Cognitive Debt) Yönetimi — Belge 2 Katkısı

AI destekli üretim, kodu insanların *ortaklaşa anlama* hızından daha hızlı
ürettiğinde "bilişsel borç" birikir: ekip hızlandırılmış bloklara müdahale etmekten
çekinir, kod incelemesi yüzeysel kalıp eşleştirmesine iner. Volt bunu dil
seviyesinde frenler:

- **Belirsizliğin reddi:** Derleyici, niyette en ufak eksiklikte (örn. eksik dal)
  varsayımsal tamamlama yapmaz; sert derleme hatası verir.
- **Okunabilir ara temsil zorunluluğu:** Üretilen SystemVerilog asla kara kutu
  değildir; insan ve standart araçlarca okunup doğrulanabilecek şeffaflıktadır.

### 2.3 Hedef Kullanıcı Kitlesi (Öncelik Sırasıyla)

1. **FPGA geliştiricileri / gömülü mühendisler** — en geniş, en az hizmet alan,
   geçiş maliyeti en düşük kitle.
2. **Öğrenciler ve yeni başlayanlar** — düşük eğri + öğretici hatalar = uzun
   vadeli benimseme tohumu.
3. **ASIC tasarım/doğrulama ekipleri** — formal-dostu semantik, CDC ve L2 timeline
   güvenliği bu ekiplerin en pahalı hatalarını hedefler.
4. **AI donanım tasarımcıları / araştırmacılar** — parametrik jeneratörler + Python
   API + LLM-dostu yapı.

---

## 3. Dil Sözdizimi ve Tip Sistemi

### 3.1 Sözdizimi İlhamı

| Kaynak | Alınan | Neden |
|---|---|---|
| **Rust** | Güçlü tipler, ifade odaklılık, `match`, hata mesajı kültürü | Donanım için güvenlilik disiplini |
| **Swift/Python** | Okunabilirlik, düşük görsel gürültü | Yeni başlayan dostu |
| **SpinalHDL** | Birinci sınıf `ClockDomain`, `Bundle`, `Stream`/`Flow` | Donanıma özgü doğru soyutlamalar |
| **Bluespec** | Guarded atomic actions (opsiyonel `rule`) | Karmaşık kontrolde yarışsızlık |
| **Filament/Anvil/Spade** | Timeline/gecikme tipleri (kademeli) | Statik zamanlama güvenliği |
| **VHDL** | Katı, açık anlambilim disiplini | Belirsizliği azaltır |

### 3.2 Tip Sistemi — Donanım Kaynaklarını Kodlar

- **Genişlik tiptir:** `u8`, `i16`, `bits<12>`, `bits<N>` (parametrik).
- **İşaretlilik tiptir:** `u*` ≠ `i*`; karıştırma açık dönüşüm ister.
- **Saat alanı tiptir:** `T @Domain` — CDC ihlali derleme hatası (Bölüm 4.3).
- **Birim/anlamsal tip:** `newtype Addr = u32`, `newtype Volts = fixed<8,4>`.
- **Zamanlama tiptir (kademeli):** `Delayed<T, N>` (L1) ve `#[timeline]` (L2).

Taşma davranışı **operatörle açıktır**: `+` (genişleyen, sonuç `u9`), `+%` (wrapping),
`+|` (doygunluklu). Sessiz veri kaybı yoktur; `u8`'e dar atama `truncate()`/`resize()`
ister.

**Tür çıkarımı:** sınırlarda açık (port/register zorunlu tipli), içeride çıkarımlı
(yerel `let`). Rust felsefesi: arayüz sözleşmesi okunur, gövde gürültüsüz.

---

## 4. Donanım Modelleme Yaklaşımı

### 4.1 Çok Seviyeli Modelleme

- **RTL:** `reg`, `wire`, `on <domain>` blokları.
- **Dataflow:** `Stream<T>`/`Flow<T>` + `>>` ile geri-basınçlı boru hatları.
- **Transaction:** Opsiyonel `rule` (guarded atomic actions); derleyici çelişen
  kuralları çözer, yarışsız kontrol üretir.

Tüm seviyeler aynı IR'ye düşer.

### 4.2 Eşzamanlılık ve Tek Sürücü Kuralı

Modüldeki tüm `on`/`rule` blokları kavramsal olarak eş zamanlıdır; **her sinyalin
tek sürücüsü** olmak zorundadır (multiple-driver = derleme hatası). "Kim bu wire'ı
sürüyor?" belirsizliği ortadan kalkar.

### 4.3 Saat Alanları, Reset, CDC

```volt
domain Fast { clock = posedge }
domain Slow { clock = posedge }

module Bridge {
    in  flag_fast : bool @Fast
    out flag_slow : bool @Slow

    // flag_slow = flag_fast            ❌ alanlar uyuşmuyor → derleme hatası
    flag_slow = cdc::sync_2ff(from = Fast, to = Slow, flag_fast)   ✅
}
```

Saat alanı tiptedir; bir alandan diğerine geçiş ancak onaylanmış CDC primitifiyle
mümkündür. Çok-bit geçişlerde derleyici gri-kod/handshake zorlar. Reset stratejisi
alan tanımında merkezîdir; tutarsız reset imkânsızdır.

### 4.4 FSM ve Pipeline — Birinci Sınıf + L0 Zamanlama Güvenliği

```volt
fsm Traffic on Sys {
    state Red, Green, Yellow
    init  Red
    Red    => Green  after 50.cycles
    Green  => Yellow after 40.cycles
    Yellow => Red    after  5.cycles
}

pipeline Mac on Sys {
    stage mul: let p   = a * b
    stage add: let acc = p + c     // önceki aşama otomatik register'lanır
    out result = acc               // L0: yanlış aşamadan okuma = derleme hatası
}
```

NoC/veri yolu için hazır `Stream`/`Flow`, arbiter, FIFO, crossbar; cache için
parametrik `Memory<>` jeneratörleri.

---

## 5. Doğrulama ve Test

### 5.1 Yerleşik Test + Python/cocotb Köprüsü

Test dilin birinci sınıf parçasıdır; ayrıca Belge 2'nin haklı vurguladığı gibi
**Python referans modelleri ve cocotb** birinci sınıf desteklenir.

```volt
test "sayaç enable ile artar" {
    let dut = Counter()
    dut.enable = true
    tick(Sys, 5)
    expect dut.count == 5
}
```

`forall x: u8 in 0..256 { ... }` ile constrained-random; Python tarafında cocotb
testbench'leri aynı DUT'a bağlanabilir → algoritma modeli = donanım doğrulaması.

### 5.2 Yerleşik Formal Doğrulama

```volt
assert on Sys: req |=> eventually(ack within 8.cycles)   // temporal
invariant on Sys: !(write && read)                       // değişmez
```

Tek anlambilim formal-dostudur: `assert`/`invariant`/`cover` doğrudan formal
motorlara (yerleşik bounded model checker + SymbiYosys köprüsü) çevrilir. Hazır
property kütüphanesi: one-hot, handshake doğruluğu, FIFO taşma yok.

---

## 6. Derleyici Mimarisi — Belge 2'nin Ana Katkısı

Volt'un dil tasarımı ancak güçlü bir derleyici altyapısıyla gerçeğe dönüşür. Önerilen
yığın, sıfırdan IR icat etmek yerine **olgun açık altyapıya yaslanır**.

### 6.1 Önyüz (Frontend) — Rust + Kayıpsız CST

- **Rust:** Devasa AST'ler üzerinde bellek güvenliği ve eşzamanlılık güvenliği;
  derleyici önyüzü ve tip analizörü için ideal.
- **cstree (Swift `libsyntax` esinli):** Konumdan bağımsız *yeşil düğümler* +
  tembel *kırmızı düğümler* ile kayıpsız somut sözdizimi ağacı (CST). Hatalı/eksik
  koddan kurtulma (error recovery) — LSP'nin anlık tanısı için şart.
- **Bellek:** Arena tahsisatçıları + `u32` indeks-tabanlı handle'lar; borrow
  denetimini kolaylaştırır.
- **Çözünüm:** Ağacı yerinde değiştirmek yerine `UnboundAst` tüketip yeni `BoundAst`
  üreten fonksiyonel dönüşüm mimarisi.

### 6.2 Orta/Arka Yüz — MLIR / CIRCT

```
İdeal HDL Önyüzü (Rust parser + cstree CST)
        │  lowering
        ▼
CIRCT / MLIR ortak alanı
   ├─ Calyx lehçesi      → FSM ve kontrol akışı optimizasyonu
   ├─ Handshake lehçesi  → asenkron veri akışı, FIFO, elastik devre
   └─ FIRRTL & HW lehçeleri → alt-seviye sentezlenebilir RTL
        │  kod üretimi
        ▼
Okunabilir, sentezlenebilir SystemVerilog (EDA entegrasyonu)
```

- **Calyx:** kontrol akışını yapısal donanımla birleştirip kaynak paylaşımını çözer.
- **Handshake:** veri akış grafiklerini FIFO-tabanlı asenkron yapılara modeller.
- **FIRRTL/HW + `sv`/`moore`:** standart SystemVerilog üretimi ve simülasyon.

### 6.3 CIRCT'nin Açığı ve Telafisi — Formal Semantik

CIRCT'nin en büyük yapısal riski **resmî semantik tanımının eksikliğidir**; bu,
sentezlenen devrelerin matematiksel doğrulanmasını zorlaştırır. Telafi: MLIR/CIRCT
lehçelerinin davranışsal kurallarını resmîleştiren **K framework** tabanlı bir
semantik katman derleyiciye entegre edilir. Bu, Volt'un "formal-dostu" iddiasını
altyapı seviyesinde garanti altına alır.

> **Not:** Bu önyüz kararı, Bölüm 1.1'deki "bağımsız dil" kararıyla tutarlıdır —
> Volt'un kendi gramer ve tip denetleyicisi vardır; CIRCT yalnızca *arka uç*
> hedefidir, dilin kendisi değil.

---

## 7. Hata Önleme ve Geliştirici Deneyimi

### 7.1 Derleme Zamanında Yakalananlar

Genişlik/işaretlilik uyumsuzlukları, sessiz kesmeler, örtük latch, eksik atama
yolları, çoklu sürücü, CDC ihlalleri, uyumsuz saat alanı, ulaşılamaz/eksik FSM
durumları, birleşimsel döngüler, reset tutarsızlıkları, pipeline hizalama (L0/L1)
ve yapısal kaynak çakışmaları (L2 `#[timeline]`).

### 7.2 Hata Mesajı (Elm/Rust Standardı)

```
hata[E0412]: olası veri kaybı içeren atama
  ┌─ alu.volt:14:9
14│     out_byte = sum
  │                ^^^ bu ifade 'u9' tipinde (genişlik 9)
  │     -------- hedef 'out_byte' 'u8' tipinde (genişlik 8)
  = neden: 'u8 + u8' toplamı taşmayı korumak için 'u9' üretir.
  = öneri:
        out_byte = sum.truncate()   // üst biti at
        out_byte = sum +| 0         // doygunlukla sınırla
        out_byte : u9               // hedefi genişlet
```

Her hata: **konum + neden + somut düzeltme**; hata kodları (`E0412`) belgeli ve
aranabilir.

### 7.3 LSP ve IDE (rust_hdl/Verible çizgisi)

| Özellik | İşlev | Bilişsel yük azaltma |
|---|---|---|
| Anlık tip & yol denetimi | Bit genişliği ve saat alanı uyumsuzluğunu yazım anında kırmızı çizer | Yüksek (aylarca hata ayıklamayı önler) |
| Otomatik esnek kablolama | AXI/APB/AHB portlarını yön eşleştirerek tek tıkla bağlar | Çok yüksek (boilerplate'i sıfıra indirir) |
| Görsel FSM izleyici | Durum makinelerini canlı grafiğe çevirir | Orta |
| Canlı PPA öngörüsü | Sentez öncesi tahmini alan/güç (Bölüm 9.3) | Yüksek (erken mimari karar) |

Ek olarak `volt fmt` (biçimlendirici), `volt clippy` (donanım lint), ve makine-okunur
JSON tanılar (`--message-format=json`).

---

## 8. Yapay Zekâ ve Gelecek Uyumluluğu

LLM'ler için Verilog'un sorunu: bağlam-bağımlı, örtük ve "sessizce hata-toleranslı"
doğası — model yanlış genişlik üretir, uyarı çıkmaz, hata silikona kadar gizlenir.
Volt bunu tersine çevirir:

| Özellik | LLM faydası |
|---|---|
| Düzenli, bağlamdan-bağımsız gramer | Az sözdizimi halüsinasyonu |
| Yerel akıl yürütme | Tek modül için tüm projeyi "hatırlamak" gerekmez |
| Güçlü tipler + zengin hatalar | Üret→derle→düzelt (agentic) döngüsü güvenilir |
| Açık niyet operatörleri (`+%`, `+\|`) | Taşma davranışı belirsiz değil |
| Tek anlambilim | "Sim'de çalışan ama sentezlenmeyen" örnek yok |

JSON tanılar AI ajanları için doğrudan geri bildirim kanalıdır. **Bilişsel borç
korumaları** (Bölüm 2.2) AI hızını ekibin anlama hızıyla dengeler.

---

## 9. Performans, Sentez, Maliyet ve EDA — Belge 2 Katkısı

### 9.1 Sentez Kalitesi

- **Şeffaf lowering:** Volt → CIRCT IR → RTL belgelidir ve tahmin edilebilir
  (HLS'in kara kutusunun tersi).
- **Sıfır-maliyet soyutlama:** `Stream`/`fsm`/`pipeline`, elle yazılmış RTL'den
  kötü olmayan donanım üretir.
- **Kaçış kapakları:** kritik yolda aşama sınırı, kodlama (binary/one-hot/gray) ve
  kaynak eşlemesi (`@dsp`, `@bram`) elle yönlendirilebilir — pragma değil, tiplenmiş
  öznitelik (taşınabilir, doğrulanabilir).

### 9.2 Açık Kaynak EDA + Hibrit Broker Modeli

Maliyet engelini aşmak için **açık kaynak araç zinciri** benimsenir: **Yosys**
(mantıksal sentez), **OpenROAD** (RTL→GDSII P&R, OpenSTA zamanlama), **Verilator**
(simülasyon). SystemVerilog ön işleme için Bender/Morty/SVase/SV2V zinciri.

Belge 2'nin bildirdiği bir gerçeklik: tamamen açık kaynak akış, ticari akışa kıyasla
ciddi PPA kaybı yaşayabilir ve dökümhaneler yalnızca ticari imza-doğrulama
(Calibre) onaylı GDSII kabul eder. Çözüm **Hibrit Broker Modeli**: tasarım ücretsiz
açık kaynak araçlarla tamamlanır, üretilen GDSII yalnızca DRC/LVS imza-doğrulaması
için yetkili bir silikon broker'a gönderilir; böylece düşük bütçeyle MPW shuttle
slotlarına erişilir.

> **Doğrulama notu:** Belge 2'deki nicel iddialar (ör. ~%53 alan / ~%118 güç farkı,
> MPW maliyet aralıkları, imza-doğrulama kiralama ücretleri) kaynağa dayalıdır ama
> bağımsız olarak teyit edilmemiştir; mimari kararı bunlara bağlamadan önce güncel
> dökümhane/broker tekliflerinden doğrulanmalıdır.

### 9.3 Sentez Öncesi ML-Tabanlı PPA Tahmini (MasterRTL Çizgisi)

Her değişiklikten sonra saatlerce sentez beklemek yerine, RTL'den nihai PPA'yı
tahmin eden bir model derleyiciye gömülür. **MasterRTL** yaklaşımı kaynağı tek-bit
operatörlerden oluşan **Basit Operatör Grafiği (SOG)**'a indirger; bu, kapı-seviyesi
netlist'i yüksek doğrulukla taklit eder ve kritik yol/güç tahminini saniyeler
içinde verir. Bu motor, LSP'deki "canlı PPA öngörüsü"nü besler ve iterasyon
maliyetini dramatik düşürür.

### 9.4 FPGA ⇄ ASIC Dengesi

Hedef-bağımsız çekirdek + hedef-özel primitif kütüphanesi: aynı kaynak
`--target=fpga:xilinx-us+` veya `--target=asic:sky130` ile farklı primitiflere
(BRAM↔SRAM, DSP48↔çarpan hücresi) eşlenir. Reset/saat soyutlaması alan tipinden
türetilir. Çıktı standart SystemVerilog → Vivado, Quartus, Yosys ve ticari ASIC
akışlarına girer.

---

## 10. Hukuki Strateji: IP, Temiz Oda ve AI Riski — Belge 2 Katkısı

Yeni bir dil ve standart IP kütüphanesi inşa ederken telif/patent en kritik risktir.

- **Telif vs patent:** Telif yalnızca *ifade biçimini* korur; bir dilin sözdizimi,
  veri formatları ve işlevsel çıktıları telife tabi değildir (*SAS Institute v. World
  Programming* emsali). Patent ise *çalışma prensiplerini* korur — bağımsız üretim
  telife karşı mutlak savunmadır ama patente karşı koruma sağlamaz. Bu yüzden Volt'un
  standart kütüphanesi ve derleyici optimizasyonları tescilli donanım patentlerini
  (örn. özel önbellek tutarlılık algoritmaları) ihlal etmemelidir.
- **Temiz Oda (Clean-Room):** Tescilli modülleri yeniden kurarken birbirinden izole
  iki ekip — **A** (tersine mühendislik + kod içermeyen şartname) ve **B** (şartnameyi
  sıfırdan, ön bilgisiz uygulayan) — kullanılır. Meşruiyeti *NEC v. Intel*, Compaq–IBM
  BIOS ve VTech–Apple II ROM emsalleriyle tescillidir.
- **AI riski:** Üretken modellerin eğitim verisindeki telifli ifadeleri kopyalaması
  (tainting) riski vardır. Önlem: tüm prompt/şartname geçmişini "zincirleme velayet"
  (chain of custody) olarak belgelemek ve diferansiyel-gizlilik korumalı modelleri
  tercih etmek. ("Hizmet olarak temiz oda" / `(κ,β)-clean` gibi iddialar henüz
  olgunlaşmamış olup hukuki olarak temkinli değerlendirilmelidir.)

---

## 11. Kaçınılması Gereken Zayıflıklar ve Volt'un Yanıtı

| Mevcut HDL zayıflığı | Volt çözümü |
|---|---|
| Dil karmaşıklığı (SV LRM ~1300 syf) | Küçük tutarlı çekirdek; ileri özellikler opsiyonel katmanda |
| Gizli davranışlar | Sensitivity list yok; eksik atama derleme hatası |
| Belirsiz semantik | Tek, K-framework ile resmîleştirilmiş anlambilim |
| Yarış durumları | Tek sürücü kuralı + `rule` atomikliği |
| Sessiz veri kaybı | Genişlik tip kontrolü; açık operatörler |
| Zayıf tip güvenliği | Genişlik+işaretlilik+saat alanı+birim+zamanlama tipte |
| Kötü hata mesajları | Rust/Elm sınıfı, düzeltme öneren tanılar |
| Sim/sentez uyumsuzluğu | Tek semantik model, tek IR |
| Boru hattı/kaynak çakışması | Kademeli zamanlama tipleri (L0–L2) |
| Araç bağımlılığı | Açık CIRCT IR + standart SV; satıcı kilidi yok |
| Maliyet engeli | Açık kaynak EDA + hibrit broker + ML PPA |
| Hukuki risk | Temiz oda + chain-of-custody |
| Ölçeklenebilirlik | Paket yöneticisi, yerel akıl yürütme, jenerikler, arayüz tipleri |

---

## 12. Nihai Tasarım Önerisi

### 12.1 Bütünleşik Mimari

```
Volt Kaynak (.volt)            [bağımsız, statik tipli dil]
   • RTL · Dataflow · Rule
   • Tipler: genişlik+işaret+saat alanı+birim+zamanlama (L0–L2)
   • Python API: yalnızca jeneratör + cocotb doğrulama
        │
   Rust önyüz (cstree CST, arena, error-recovery)
        │  lowering
   CIRCT/MLIR  (Calyx → Handshake → FIRRTL/HW)  +  K-framework semantik
        │
   ├─ SystemVerilog (Yosys/OpenROAD/Verilator → hibrit broker → GDSII)
   ├─ Native simülatör (volt test)  +  Python/cocotb köprüsü
   ├─ Formal köprü (BMC / SymbiYosys)
   └─ ML PPA motoru (SOG, MasterRTL çizgisi) → LSP canlı öngörü
        │
   Araçlar: volt fmt · LSP · paket yöneticisi · JSON tanılar
```

### 12.2 Örnek — Tam Bir Modül

```volt
domain Sys { clock = posedge, reset = sync active_high }

/// 4-bit doygunluklu sayaç + komut FSM'i
module SatCounter {
    in  cmd   : Cmd
    out value : u4
    out maxed : bool

    reg(Sys) count : u4 = 0
    enum Cmd { Hold, Inc, Clear }

    on Sys {
        match cmd {
            Cmd::Inc   => count <= count +| 1   // 15'te doygun
            Cmd::Clear => count <= 0
            Cmd::Hold  => {}                    // kayıt korunur (latch DEĞİL)
        }
    }

    value = count
    maxed = (count == 15)

    invariant on Sys: count <= 15
    assert    on Sys: (cmd == Cmd::Clear) |=> (count == 0)
}

test "doygunluk 15'te durur" {
    let dut = SatCounter()
    dut.cmd = Cmd::Inc
    tick(Sys, 20)
    expect dut.value == 15 && dut.maxed == true
}
```

### 12.3 Karşılaştırma

| Boyut | Verilog/SV | VHDL | Chisel | Filament/Spade | **Volt (birleşik)** |
|---|---|---|---|---|---|
| Öğrenme eğrisi | Orta (tuzaklı) | Dik | Çok dik (Scala) | Çok dik | **Düşük (kademeli)** |
| Bağımsız dil? | Evet | Evet | Hayır | Evet | **Evet** |
| Tip güvenliği | Zayıf | Güçlü | Orta-iyi | Çok güçlü | **Çok güçlü** |
| Zamanlama güvenliği | Yok | Yok | Sınırlı | **Çekirdek** | **Kademeli L0–L2** |
| Sim/sentez | Sorunlu | İyi | İyi | İyi | **Tek model** |
| CDC | Manuel | Manuel | Manuel | Kısmî | **Dilde zorunlu** |
| Doğrulama | SVA | Sınırlı | Kütüphane | Tip-tabanlı | **Yerleşik + cocotb** |
| Derleyici altyapısı | Çeşitli | Çeşitli | FIRRTL | Özel | **Rust + CIRCT + K** |
| Hata mesajları | Kötü | Orta | Kriptik | İyi | **Rust/Elm sınıfı** |
| Maliyet stratejisi | — | — | — | — | **Açık EDA + broker + ML PPA** |
| Hukuki çerçeve | — | — | — | — | **Temiz oda + custody** |
| Ekosistem | Çok yüksek | Yüksek | Orta | Düşük | Yeni (risk) |

> **Dürüst dezavantaj:** Volt'un tek gerçek zayıflığı ekosistem olgunluğudur.
> Telafi: standart SV üretip mevcut akışlara girmek, yabancı IP'yi `extern` ile
> sarmak, açık altyapıya (CIRCT) yaslanmak. Yine de benimseme uzun vadeli yatırımdır.

---

## 13. Stratejik Yol Haritası (Birleşik)

1. **Çekirdeği zamanlama-güvenli kur:** L0 pipeline hizalama + L1 gecikme tipi
   zorunlu; L2 timeline (Filament/Anvil) opt-in.
2. **Rust + MLIR/CIRCT ortaklığı:** cstree CST'li Rust önyüzü → Calyx/Handshake/FIRRTL
   lowering → standart SystemVerilog. K-framework ile semantik açığı kapat.
3. **Bağımsız dil + Python yardımcısı:** Donanım tanımı statik tipli Volt'ta; Python
   yalnızca jeneratör + cocotb doğrulama.
4. **Gün-1 tooling:** açık kaynak derleyici + Verilator-hızında simülatör + LSP +
   biçimlendirici + JSON tanılar. (Kötü tooling iyi dilleri öldürür.)
5. **Maliyet & üretim:** Yosys/OpenROAD/Verilator + SOG-tabanlı ML PPA motoru +
   hibrit broker ile düşük bütçeli tape-out.
6. **Hukuki güvenlik:** temiz oda metodolojisi + chain-of-custody + telif-temiz AI
   üretimi.
7. **Benimseme:** önce FPGA topluluğu, sonra ASIC; üniversite müfredatı + playground;
   açık spesifikasyon ve IR (satıcı kilidi yok).

---

## Kapanış

İki kaynağın birleşik tezi: **donanım anlambiliminden ödün vermeden en pahalı
donanım hatalarını dil seviyesinde imkânsız kılan, olgun açık altyapı (Rust +
CIRCT) üzerine kurulu, hukuki ve ekonomik olarak gerçekçi bir HDL.** Yenilik radikal
yeni bir hesaplama modelinde değil; doğru soyutlamaları güçlü bir statik tip
sistemi, tek bir resmî anlambilim, kademeli zamanlama güvenliği ve birinci sınıf
araçlarla *bir arada* sunmaktadır — ki mevcut diller bunları ayrı ayrı başarmış
ama bir bütün olarak sunamamıştır.

Çözülen iki çatışma bu birleşik tasarımın omurgasıdır: **dil bağımsız ve statik
tiplidir (Python yardımcıdır, HDL değildir)** ve **zamanlama güvenliği kademelidir
(çekirdekte hafif, opt-in tam)**.
