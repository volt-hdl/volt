# ADR-0114: Modül Özeti — Örnek Sınırında Kombinasyonel Bağımlılık: Saatsiz Modülün Çıkışı Girişlerinin Alanını Taşır, E4007 Alt Modülün İçinden Geçer

> Statü: Uygulandı
> Önceki karar: ADR-0047, ADR-0050 — ADR-0047 §3 tablosundaki "Clock portu yok | Timeless (kombinasyonel extern)" satırı (domain-inference.md K8a son cümlesi); ADR-0050 §4'teki "örnek çıkışları opaktır" ve "Sınırlar / Ertelenen"deki E4007 maddesi
> İlgili: ADR-0081 (Karar 9: fn çağrısının alanı argümanların join'i — saatsiz modül örneği aynı kuralı port başına uygular), ADR-0052 (K11 örnekleme kuralı değişmez; güven inceltmesi ayrı karar, #195), ADR-0056 (modül düzeyi `for` parser'da açılır: ADR-0050'deki "for gövdeleri" sınırı zaten geçersizdi), ADR-0041 (generic modülün her açılımı ayrı özet alır), ADR-0113 (derlemdeki #189/#193 satırları kapanır), ADR-0099 (yeni E3001 önerileri gidiş-dönüş denetiminden geçer), ADR-0100 (sessiz yanlış düzeltmesi son hanede), ADR-0090 (öneri adı örnek çıkışının üretilen SV adından kaçınır)
> Tarih: 2026-11-09
> Etkilenen: domain-inference.md §1, K2, K8, K8a, YENİ K8b, §3, §8
> (bakımcı yaması); volt-hir (`summary.rs` YENİ; `handshake.rs`,
> `domain/{mod,instance,assign,walk}.rs`, `lib.rs`, `pipeline.rs`),
> volt-ast (`BuiltinPrim::comb_inputs`), volt-diagnostics (E3001 ve E4007
> açıklamaları en/tr), `tests/ui/{pass,fail}`, `tests/suggestions/`,
> `crates/volt-driver/tests/{module_summary_tests,builtin_summary_tests,seeded_bug_tests}.rs`,
> `.cargo/mutants.toml` (özet dosyaları mutasyon kapsamında; bir A2 kaydı),
> `book/src/limitations.md`, `CHANGELOG.md`, `docs/adr/README.md`,
> ADR-0047 ve ADR-0050 (başlık satırları ve ek bölüm)

## Sorun

İki güvenlik denetimi alt modül sınırında kör kalıyordu; ikisi de
tohumlanmış hata testinde (ADR-0113) bulundu.

**#189 — saat alanı.** Saat portu olmayan bir modülün portları örnek
sitesinde `Timeless` sayılıyordu (`instance.rs::expected_port_domain`, K8
sözde kodundaki `unwrap_or(DomainId::Timeless)`):

```volt
module Pass {
    in  a : bool
    out y : bool
    y = a
}

module CdcBridge {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow
    let p = Pass { a: fast_flag }
    slow_flag = p.y
}
```

`volt check`: 0 hata. `volt build`: `Pass p (.a(fast_flag), .y(p_y)); assign
slow_flag = p_y;` — senkronizörsüz bir CDC. Aynı yol alt modülsüz, `fn`
ile ya da tek saatli bir alt modülle yazılınca E3001 alıyordu; güven
boyutunda aynı yapı E3009 alıyordu. K8a saatsiz **extern** portlarını
açıkça `Timeless` sayıyordu; aynı tehlike orada şartnameyle kabul
ediliyordu.

**#193 — el sıkışma.** E4007 (ADR-0050 §4) örnek çıkışlarını opak
sayıyordu. `valid`'in `ready`'ye yalnız bir alt modül üzerinden bağlandığı
üretici (`let g = AndGate { a: have, b: tx.ready }; tx.valid = g.y`)
E4007 almıyordu; aynısı extern, `for` içindeki örnek ve yerleşik primitif
(`EdgeDetect`, `PriorityArbiter`, `RoundRobinArbiter`) üzerinden de
geçerliydi. ADR-0050'nin "tutma kontratı bu boşluğu kapatır" gerekçesi
ölçüldü ve tutmuyor: `valid = ready && have` biçiminde tutma kuralının
öncülü (`prev(valid) && !prev(ready)`) ulaşılamaz, iki otomatik kural boş
yere kanıtlanıyor (`volt verify --mode prove`: 2/2 ok; öncülün `cover`'ı
derinlik 20'de ulaşılmıyor). "valid ready'ye bağlı değil" iki izli bir
özelliktir; tek izli bir `invariant` onu ifade edemez (ADR-0052 §5'teki
gerekçenin aynısı). ADR-0050'nin "`for` gövdelerini görmez" maddesi ise
ADR-0056'dan beri doğru değil: modül düzeyi `for` parser'da açılıyor ve
E4007 açılmış gövdeyi görüyordu (ölçüldü).

Ortak kök: örnek sitesi çocuğun hangi çıkışının hangi girişe
kombinasyonel bağlı olduğunu bilmiyordu.

## Karar

### 1. Modül özeti

Her modül için bir kez, çocuktan başlayarak bir **özet** çıkarılır:
çıkış portu → kombinasyonel bağlı olduğu giriş portları kümesi
(port başına bağımlılık, [K2]).

- Kenarlar: sürekli atama, `let`, `comb` bloğu; koşul, `match` sınananı ve
  kol muhafızı işlenendir (K5, ADR-0083 Karar 8); fn çağrısının
  argümanları işlenendir (ADR-0081). `on` bloğu, register,
  `sync()`/`sync3()` yolu keser.
- Modül düzeyi `for` zaten açılmıştır (ADR-0056).
- Alt örnek: `inst.out` okuması, çocuğun özetinde `out`un bağlı olduğu
  girişlere bağlanan ifadelerin okumasıdır. Aynı modülün her örneği aynı
  özeti kullanır; generic modülün her açılımı ayrı modüldür (ADR-0041).
  Örnekleme döngüsü E1006'dır; örnekler üzerinden kombinasyonel döngü
  özeti sonsuz kılmaz (özet erişilebilirlik kümesi, ziyaret kümesiyle).
- Yerleşik primitif ([K7]): primitifler Volt'ta değil sv-emit
  şablonlarında yazılı olduğundan özet otomatik hesaplanamaz; elle bir
  tablodur (`BuiltinPrim::comb_inputs`). Tablonun **her satırı bir makine
  denetimiyle** doğrulanır: primitif bütün portları dışarı açılmış bir
  modülde örneklenir, `volt build` SV üretir, Yosys onu netlist'e çevirir
  ve çıkış bitlerinden geriye flip-flop'larda durarak hangi girişlere
  yol olduğu hesaplanır; sonuç tabloyla birebir aynı olmalıdır
  (`builtin_summary_tests.rs`, araçlı CI işinde). Tablo: `EdgeDetect`
  `rising`/`falling`/`both` ← `signal`; `RoundRobinArbiter` ve
  `PriorityArbiter` `grant` ← `req`; öteki primitiflerin (FIFO'lar, RAM'ler,
  `Counter`, `ShiftRegister`, CDC primitifleri) bütün çıkışları
  register'dan okunur, özetleri boştur.
- Extern: saatsiz extern en kötü durumdur — her çıkış saat dışı her
  girişe bağlı ([K4]); saatli extern'in özeti boştur ([K5]).

Uygulama tek bir graf kurucuyu paylaşır (`summary.rs`, eskiden
`handshake.rs`'teki `CombGraph`): E4007 ve alan çıkarımı aynı özeti okur.
Özet birim başına bir kez hesaplanır (`pipeline.rs` ve `analyze`).

### 2. Saat alanı: saatsiz hedefin çıkışı (K8 adım 3, YENİ K8b)

Saatsiz hedefin (modül ya da extern) anotasyonsuz çıkışı, örnek
sitesinde özette bağlı olduğu girişlere bağlanan değerlerin **K5 join'ini**
taşır; `@Alan` anotasyonlu giriş kendi alanıyla katılır, anotasyonlu çıkış
kendi alanında kalır. Saatsiz Volt modülünün anotasyonlu çıkışı
denetlenir ([K8]): özette bağlı olduğu anotasyonsuz girişlere bağlanan
değerler anotasyonun alanında olmalıdır, başka alandan gelen değer
örnek sitesinde E3001'dir. Saatsiz extern'de anotasyon kullanıcının
beyanıdır, denetlenmez. Saatli hedefte değişiklik yoktur: portları saat
haritasından alanını zaten alır ve iç kombinasyonel yollar çocuğun kendi
denetiminde E3001 alır.

İki farklı alan aynı çıkışta birleşiyorsa **E3001 örnek sitesinde**
verilir ([K3]), çıkış okunmasa da: kapı netlist'te vardır (fn çağrısı ve
`let bad = fast & slow` ile aynı). İki bağlama etiketlenir; gerekçe notu
çıkışın hangi girişlere bağlı olduğunu söyler (`'And2.y' depends on 'a'
and 'b'`). Örnek başına bir kez; çıkış `Error` olur, okumaları kaskad
üretmez. Öneri, ikinci girişi ilkinin alanına senkronize eden bir
`let ... = sync(...)` satırı ve bağlamanın o adla değiştirilmesidir
(tek bitlik, adı yazılabilen değerde; ADR-0099 gidiş-dönüş). Modülün
içinde tanı yoktur: içeride portlar `Timeless`'tır.

Çıkış başka bir alana atanırsa E3001'in mevcut atama biçimi geçerlidir;
bir not çıkışın alanını nereden aldığını söyler (`'p.y' carries @Fast:
'Pass' has no clock, so its output 'y' carries the domain of the inputs it
depends on ('a')`). Önerisi iki düzenlemedir (`let p_y_raw = p.y` +
`sync(p_y_raw, <saat>)`): `sync(p.y, clk)` E0003 alır (bileşik `sync()`
kaynağı) ve `p_y` örnek çıkışının üretilen SV adıdır (ADR-0090, E1003).
Blok içinde bloğun üstüne iki `let` eklenir.

### 3. E4007 alt modülün içinden geçer

E4007'nin grafı `inst.out` okumasını özet üzerinden çocuğun girişlerine
bağlar. Tanının "kombinasyonel yol" notu yolu örnekle birlikte yazar:
`tx.valid <- g.y (AndGate: y <- b) <- tx_ready`.

Özet, hangi bağımlılığın **yalnız** saatsiz extern'in en kötü durum
varsayımıyla kurulduğunu da taşır (iç içe Volt modüllerinden geçse de).
Bu bilgi yalnız tanının notu içindir ([K4], karar B): varsayımla kurulan
yol da E4007'dir ve not yolun varsayıma dayandığını söyler (`extern
'Pair' has no visible body; Volt assumes every output depends on every
input, so 'y' is taken to depend on 'b'`). Öneri her durumda aynıdır:
`valid`'i register'lanmış durumdan üretmek; bu, extern ne yaparsa yapsın
yolu keser. `@no_protocol_check` E4007'yi etkilemez: tek işi ADR-0050
§3'teki gibi portun otomatik protokol kontratlarını kapatmaktır.

### 4. Güven seviyesi değişmez

K11'in örnekleme kuralı (sınıflandırılmış girişlerin en yükseği) olduğu
gibi kalır ([K6]).

## Bakımcı kararları

**[K1] Sınıf ve sürüm: (a).** Sessiz yanlış düzeltmesi; v0.1.2. ADR-0100
§1 son hanede önceden kabul edilen girdinin reddedilmesine sessiz yanlış
için izin veriyor; yeni tanı kodu yok (E3001, E4007). Sürüm notlarının
"Behavior changes" bölümünde açıkça yazılır: saatsiz extern için
şartname (K8a) bu durumu izinli sayıyordu; güvenlik nedeniyle değişti.

**[K2] Kesinlik: (a).** Port başına bağımlılık. E4007 için zaten gerekli
(register kesmeli); aynı özet alan için de kullanılır. Bağımsız yollar
(`y1 = a1`, `y2 = a2`) ve sabit çıkışlar yanlış alarm vermez.
Reddedilen (b) tutucu biçim (bütün girişlerin join'i) iki sentetik
yanlış alarm ve SoC'de 10 yanlış E4007 üretti.

**[K3] Tanının yeri: (a).** Örnek sitesinde, çıkış okunmasa da.

**[K4] Saatsiz extern: (a) en kötü durum; E4007'de karar B.** Saatsiz
extern'in her çıkışı her girişe bağlı sayılır (anotasyonsuz çıkış
girişlerinin join'ini taşır). Bağımsız yolları farklı alanlardan
beslenen extern'de E3001 için kaçış: portlara global `@Alan` anotasyonu
(bugün de geçerli); anotasyonlu çıkışın bağımlılıkları kullanıcının
beyanıdır, anotasyonlu girişe bağlanan değer K8 adım 2'deki gibi
denetlenir. ADR-0047'yi kısmen değiştirir.

E4007'de karar **B**: varsayımla kurulan `valid` → `ready` yolu da
E4007'dir; özet bağımlılığın varsayımsal olduğunu taşır ve tanının notu
bunu söyler, öneri `valid`'i register'da üretmektir. `@no_protocol_check`
E4007'ye bağlanmaz; tek işi ADR-0050'deki gibi otomatik protokol
kontratlarını kapatmak olarak kalır. Gerekçe (bakımcı): varsayımsal bir
E4007'yi susturmanın bedeli o portun kontratlarını da kaybetmek
olmamalı. Değerlendirilen öteki seçenek A, Reddedilen alternatifler'de.
Bu sürümde varsayımsal yol için kaçış yolu yoktur (Sınırlar).

**[K5] Saatli extern ve E4007: (a).** Opak kalır (özet boş); kitabın
sınırlar sayfasına yazılır. En kötü durum ölçüldü ve reddedildi: standart
FIFO boşaltma deseni (`tx.valid = !f.empty`, `rd_en: tx.fired`) yanlış
E4007 alıyor. (c) Extern çıkış portunda bildirimli özet (`@comb(a, b)`,
`@registered`) v0.2.0'ın konusu, ayrı ADR: #196.

**[K6] Güven (E3009) inceltmesi: (a).** Bu ADR'nin dışında: #195.

**[K7] Yerleşik primitif özetleri: (a), değişiklikle.** v0.1.2'de dahil.
Primitif Volt'ta yazılmış olsaydı özeti otomatik hesaplanırdı; yerleşikler
sv-emit şablonu olduğundan elle bir tablodur ve her satırı bir makine
denetimiyle (Yosys ile primitifin ürettiği SV'de kombinasyonel yol
analizi) doğrulanır (§1). Analizin kendisi de sınanır: bilinen bir kapı
ile bir register'ı ayırt etmesi beklenir.

**[K8] Saatsiz modülde anotasyonlu çıkış: denetlenir.** PR incelemesinde
ölçüldü: saatsiz `P` (`in a : bool`, `out y : bool @Fast`, `y = a`)
derleniyor ve `P { a: slow_flag }` örneğinin `p.y`'si `@Fast` bir çıkışa
hatasız, senkronizörsüz atanıyordu (CDC kaçağı). K8 adım 3 anotasyonlu çıkışı da
denetler: özette bağlı olduğu anotasyonsuz girişlere bağlanan
değerlerden biri anotasyondan farklı bir alandaysa E3001 örnek sitesinde,
o bağlamada, çıkış başına bir kez; çıkışın okumaları anotasyonun alanını
taşır (kaskad yok). Anotasyonlu giriş modülün içinde denetlenir (K6),
örnek sitesinde ikinci kez değil. Saatsiz extern'de anotasyonlu port
denetlenmez: gövde görünmediğinden anotasyon kullanıcının beyanıdır.

## Sonuçlar

- Depodaki bütün `.volt` dosyalarında, kitapta ve tohumlanmış hata
  derleminde tanı farkı ölçüldü (PR açıklamasında sayılarla): depodaki
  tasarımların hiçbiri yeni tanı almıyor. Bu, desenin derlemde
  bulunmamasından gelir; yanlış alarm riskini sentetik yakın ıskalar
  ölçer.
- Tohumlanmış hata derlemi: #189 ve #193'ün bütün biçimleri (iç modül,
  iki alanın birleşmesi, iç içe saatsiz modül, saatsiz extern; saatsiz ve
  saatli alt modül, `for` içindeki örnek, saatsiz extern, `EdgeDetect`,
  iki arbiter) şartnamenin verdiği kodla yakalanıyor; bilinen açıklar
  listesi boş. Yakın ıskalar sessiz.

Davranış değişikliği: saatsiz bir modül ya da saatsiz extern üzerinden
başka bir saat alanına geçen kombinasyonel yol E3001 alır; saatsiz modül
iki alanı aynı çıkışta birleştirirse örnek sitesinde E3001; saatsiz
modülün anotasyonlu çıkışına başka alandan değer ulaşırsa örnek sitesinde
E3001 ([K8]); alt modül,
saatsiz extern ve yerleşik `EdgeDetect`/arbiter üzerinden `ready`'den
`valid`'e giden yol E4007 alır; saatsiz extern'in en kötü durum
varsayımıyla kurulan yol da E4007'dir ve `@no_protocol_check` bunu
değiştirmez (önceden bu yollar hiç görülmüyordu).

## Sınırlar

- Saatli extern'in kombinasyonel yolları görünmez ([K5]): saatli bir
  extern'in içinden `ready`'den `valid`'e giden yol E4007 almaz (#196).
- Saatsiz extern'den geçen varsayımsal `valid` → `ready` yolu için bu
  sürümde kaçış yolu yoktur ([K4], karar B): gerçek extern'de yol
  olmasa da E4007 verilir; çözüm `valid`'i register'da üretmektir. Açık
  bağımlılık bildirimi (extern'de hangi çıkışın hangi girişe bağlı
  olduğunu yazmak) v0.2.0'da.
- Güven boyutu tutucu kalır ([K6], #195).
- Özet port düzeyindedir, bit düzeyinde değil: `y[0] = a`, `y[1] = b`
  olan bir çıkış iki girişe de bağlı sayılır.
- Bir E4007 yolu birden çok örneğin içinden geçerse not her okuma için
  en dıştaki örnek adımını yazar.
- Kombinasyonel döngü tanısı (#83) bu ADR'nin konusu değil; özet onun
  örnek sınırını aşan biçimi için de altyapı olur.

## Reddedilen alternatifler

- **A: `@no_protocol_check`'i E4007'ye de bağlamak ([K4]).** Nitelik
  portta yalnız saatsiz extern varsayımıyla kurulan yolları yok sayardı
  (görünür yol yine E4007). Reddedildi: nitelik aynı zamanda portun
  otomatik protokol kontratlarını kapatır (ADR-0050 §3), yani varsayımsal
  bir E4007'yi susturmanın bedeli `valid`'in `ready` gelene dek düşmemesi
  ve verinin sabit kalması kontratlarını kaybetmek olurdu. Bir hatayı
  susturmak bir güvenlik özelliğini kapatmamalı; doğru kaçış yolu
  bağımlılığı açıkça bildirmektir (v0.2.0).
- **Tutma kontratına güvenmek (ADR-0050'nin gerekçesi).** Kural başka
  bir özelliği kanıtlıyor; bağımlılık iki izli bir özellik (yukarıda).
- **Çocuğu örnek sitesinde satır içi açmak.** Her örnek çocuğun bütün
  gövdesini yeniden yürür; derin hiyerarşide maliyet örnek sayısıyla
  çarpılır ve tanılar çocuğun içine işaret eder. Özet bir kez hesaplanır.
- **Saatsiz modüle E3010 benzeri zorunlu anotasyon.** UX Anayasası'na
  aykırı (tek saatli ve saatsiz tasarım alanı hiç görmemeli) ve
  saatsiz modülü alan başına kopyalamaya zorlar.
- **Yerleşik primitif tablosunu yalnız elle yazılmış testlerle korumak.**
  Bir test tablonun kendisini tekrar ederse primitifle uyuştuğunu
  kanıtlamaz; denetim primitifin ürettiği SV'den yapılır ([K7]).
