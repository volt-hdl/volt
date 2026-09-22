# ADR-0065: RDC Denetimi ve Hedefli SDC Kısıtları — İki Güvenlik Ağı, İki Ayrı Yırtık

> Statü: KABUL EDİLDİ (tasarım; uygulama Aşama 2–5 ayrı PR'lar — bu ADR'de kod YOK)
> Tarih: 2026-09-22
> Etkilenen (plan): volt-hir (`domain/rdc.rs` YENİ; `domain/instance.rs`
> minimum), volt-diagnostics (E3003 etkin, W3xxx-A, W3xxx-B YENİ —
> numaralar Aşama 2'de W3008'den sonraki boş numaralardan tahsis edilir;
> `explain` iki dil), volt-sv-emit (açık `reset` portu E0003'ten çıkar; reset
> senkronizör zinciri; örnekleme bağlantısı), volt-hir `constraints/`
> + volt-sdc-emit (`set_clock_groups` yerine hedefli kısıtlar,
> `--sdc-style`), volt-driver (`--sdc-style`), docs/stdlib.md,
> docs/spec/sv-mapping.md §7 (yeni ADR ile), README.md Limitations,
> `.github/workflows/ci.yml` (OpenSTA adımı), tests/ui.
> Günceller: ADR-0054 §2 (`set_clock_groups -asynchronous`) ve §6
> (`-datapath_only` üretilmez kararı), ADR-0002 "RDC rezerve".
> DOKUNULMADI (tüm aşamalar): volt-hir `typeck/`, `resolve/`; docs/research/.

## Sorun

r/FPGA "[RANT] How not to solve CDC" (Eylül 2026): bir mühendis beş saat
alanının bütün çiftleri için `set_false_path` yazmış; senkronize edilmemiş
gerçek geçişler yıllarca gizlenmiş ("1 sn zamanlayıcı birden 1 saat
oluyor", "sistem sebepsiz duruyor"). Yorumların özü: iki saatin
ilişkisiz olduğunu söylemek gerçek eksik CDC yollarını gizler; asenkron
saat grupları reset recovery/removal analizini de kapatır; CDC çözümü
kısıtlarla eşleşmelidir.

Volt'ta iki kör nokta aynı yerden yırtık:

1. **SDC (ADR-0054 §2):** farklı alanlar arasına `set_clock_groups
   -asynchronous` yazılıyor. Gerekçe "derleyici her geçişin senkronizörden
   geçtiğini garanti ediyor" idi. Ama denetleyicinin kaçırdığı her yol
   (extern modül, elle düzenlenmiş SV, gelecekteki bir derleyici hatası)
   zamanlama aracından da kaçıyor. İki güvenlik ağı aynı varsayıma
   dayanıyor.
2. **RDC yok:** E3003 rezerve, hiç üretilmiyor (`README.md` Limitations;
   `docs/research/rekabet-2026-09.md` §6). Saat grupları reset
   recovery/removal analizini de kapattığı için reset tarafında hem
   derleyici hem zamanlama aracı sessiz.

Hedef: (A) RDC hataları derleme zamanında yakalansın; (B) SDC,
senkronizörsüz kalan her yolu zamanlama aracına GÖRÜNÜR bıraksın; (C)
reset recovery/removal analizi korunsun.

## Durum tespiti — Volt'un reset modeli (ADIM 1.1)

Kaynak atıfları güncel `main` (57f5170) üzerinden doğrulandı.

| Soru | Cevap | Kaynak |
|---|---|---|
| Reset nerede tanımlanır? | `domain D { reset = sync\|async active_high\|active_low }` ya da `reset = none`. Alan özelliğidir, sinyal değil. | `grammar-full.ebnf:146-148`; `volt-hir/src/domain/tables.rs:26-52` |
| Reset portu | Kullanıcı yazmaz; sv-emit polariteye göre `rst` / `rst_n` üretir, **ada göre tekler**: aynı polariteli iki alan (sync/async fark etmez, saat farklı olsa da) TEK porta bağlanır. | `volt-sv-emit/src/lib.rs:59-130` (`ResetCfg`, `reset_port_set`) |
| `always_ff` | async → `or posedge rst` / `or negedge rst_n`; sync → yalnız saat kenarı; `none` → reset dalı yok. | `lib.rs:90-97`, `lib.rs:1074-1128`; `sv-mapping.md` §7 |
| Reset veri olarak okunabilir mi? | Hayır. Üretilen `rst` Volt kaynağında bir ad değildir (çözümlenemez). Açık `in r : reset(...)` portu ayrıştırılır ve `Ty::Reset` alır ama genişliği yoktur → bit işlemlerinde E2003; sv-emit'te E0003. | `typeck/type_ref.rs:29-31`, `typeck/binop.rs:150-183`, `lib.rs:1540-1549` |
| Veri reset olarak kullanılabilir mi? | Hayır — ama `domain D { reset = some_wire }` **sessizce yok sayılır** (parser `DomainValue::Literal`, HIR eşleşmez, tanı yok). | `parser/item.rs:1676`, `tables.rs:34-42` |
| Açık reset portu | Gramer/typeck kabul eder; sv-emit E0003 ("the explicit 'reset' port"). `on clk.reset {}` de E0003. | `lib.rs:1540-1549`, `lib.rs:1084-1094` |
| Örneklemede reset | Çocuğun reset portu, ebeveynin **aynı adlı** portuna bağlanır; ebeveynde yoksa E2005. | `volt-sv-emit/src/instance.rs:150-168`; `sv-mapping.md` §9 |
| Extern modül | Reset portu kavramı yok (ADR-0047 yalnız sembolik saat alanı getirir); extern örneklemesi SV'ye zaten emit edilmez (E0003). | `domain/extern_decl.rs`, `instance.rs:120-128` |
| Stdlib CDC primitifleri | `AsyncFifo`, `HandshakeSync`, `PulseSync`, `AsyncDualPortRam` üst modül gövdesine satır içi üretilir; her taraf kendi alanının reset yapılandırmasıyla, ebeveynin `rst`/`rst_n` portunu doğrudan kullanır. Reset portu yok (`PortKind::Reset` yok). Bellek dizileri hiç resetlenmez. | `volt-ast/src/builtin.rs:47-62`; `volt-sv-emit/src/builtin_prim.rs:358-381, 445-471, 504-517, 722-738` |
| Reset'siz register | Yalnız alan seviyesinde: `reset = none`. Register başına yok. | `lib.rs:384-389`; `emit_tests.rs:229-236` |
| Aynı saat, farklı reset | Bir alanda tek reset var. İki alan aynı kenar/farklı reset ile tanımlanabilir; ama alanlar arası her geçiş kimlik tabanlı karşılaştırmayla E3001'dir (içerik aynı olsa da: `tests/ui/fail/01_cdc_violation.volt`). | `domain/join.rs:52-60` |
| Reset senkronizörü | **Yok.** `reset = async` yazan bir tasarımda dış `rst_n` doğrudan her flop'un asenkron temizleme pinine gider; bırakma (deassert) hiçbir saate senkronlanmaz. | `lib.rs:1102-1106` |
| SDC'de reset | Hiçbir şey: kısıt modeli reset taşımaz (ADR-0054 durum tespiti "SDC'ye girmez"). | `constraints/mod.rs:59-73` |

Çıkarım: Volt'ta reset bir değer değil, alan özelliğidir; kullanıcı reset'e
dokunamaz. Bu, klasik RDC hatalarının çoğunu **yapısal olarak imkânsız**
kılar (aşağıda). Geriye kalan gerçek boşluk, bir modülün DIŞ
DÜNYAYLA reset sözleşmesidir: dış asenkron reset'in bırakılması hangi
saate senkronize? Bugün cevap "hiçbirine" ve bunu ne derleyici ne SDC
söylüyor.

## Yapısal olarak imkânsız RDC hataları (denetim gerekmez)

| Aday | Neden imkânsız | Kalan boşluk |
|---|---|---|
| R3 Reset'in veri olarak kullanımı | Üretilen reset adsızdır; açık `reset` tipinin genişliği yoktur (E2003); senkronizör aşamaları da adsız. | E2003 mesajı genel ("bitwise operator is not defined for 'reset'"); yeterli, değişmez. |
| R4 Verinin reset olarak kullanımı | Register'ın reset kaynağı yalnız alan spesifikasyonudur; örnekleme bağlantısı otomatik; kullanıcı bir reset girişine ifade yazamaz. | `domain D { reset = ifade }` sessizce yok sayılıyor → Aşama 2'de parser hatası (aşağıda §3.4). |
| R2 Aynı saat, farklı reset | Aynı alanın tüm register'ları aynı reset'i kullanır. Farklı alanlar arası her yol E3001 ister (senkronizör hedef alanın reset'iyle sıfırlanır). Aşırı-yaklaşım (aynı saat olsa da E3001) belgelenmiş davranıştır. | Yok. Arch'ın "cross-async-reset-domain data path" fazı E3001'in alt kümesidir. |
| R7 Reset'siz → reset'li register | Reset'siz register ancak `reset = none` alanındadır; başka alana geçiş E3001. Aynı alanda karışık reset yok. | Derleyicinin ürettiği reset'siz yapılar (bellek dizileri, `AsyncDualPortRam` yazma portu) bilinçli ve belgeli (`stdlib.md`, W3006). Uyarı gerekmez. |
| Reset kaynaklı saat kapılama (Arch "reset-driven clock gating") | Volt'ta türetilmiş/kapılanmış saat yok: `clock` tipi yalnız porttan gelir, mantıktan üretilemez. | Yok. |
| Birleştiriciden türetilen reset (Arch "combiner-derived reset at inst") | R4 ile aynı: reset ifadesi yazılamaz, örnekleme bağlantısı otomatik. | Yok. |

## RDC kural seti ve kapsam kararı (ADIM 1.2)

Volt'ta OLUŞABİLEN durumlar ve karar:

| # | Kural | Volt'ta nasıl oluşur? | Karar | Kod | Tur |
|---|---|---|---|---|---|
| R1 | Senkronize edilmemiş asenkron reset bırakma | `reset = async` alanı; dış `rst_n` her flop'un CLR pinine doğrudan gider. Modül birim kökü ise dış dünya senkronlar mı, bilinmez. | Ham reset portu bildirilirse derleyici senkronizör zinciri ÜRETİR (§2). Bildirilmezse port "bırakılması saate senkron gelir" sözleşmesiyle kalır; birim kökünde bu varsayım **uyarı** ile görünür kılınır. | **W3xxx-A** (kökte) | bu tur |
| R5 | Bir saat alanının reset'i başka saat alanında senkronizörsüz | Aynı polariteli iki `async` alan **tek `rst_n` portunu paylaşır**; bir saate senkron bırakılan reset diğerine asenkrondur. Sözleşme tanımsız. | **Hata.** Çözüm: ham port → derleyici her saat için ayrı zincir üretir. | **E3003** | bu tur |
| R5' | Aynısı `sync` reset ile | İki `sync` alan tek `rst` paylaşır (VGA, hybrid_accel, soc örnekleri). Reset her flop'un D-mux'ünde veri gibi örneklenir; bırakma kenarı ikinci saate asenkrondur. | **Uyarı** (hata değil: mevcut çok saatli örneklerin tamamı bu desende; düzeltme tek satır). Aşama 4'te sınıf A olarak raporlanır. | **W3xxx-B** | bu tur |
| R6 | Reset yakınsaması (aynı reset iki ayrı zincirden aynı saate) | Yalnız ham port ile mümkün olur: ebeveyn ve çocuk (ya da iki kardeş) aynı ham reset'i aynı saatte ayrı ayrı senkronlarsa bırakma farklı çevrimde gerçekleşebilir. | **Hata.** Örnekleme ağacında (saat, ham reset) çifti başına en fazla bir zincir. | **E3003** (alt biçim) | bu tur |
| R2, R3, R4, R7 | — | Yapısal olarak imkânsız (yukarıdaki tablo). | Denetim yok; `reset = ifade` parser hatası. | E1xxx (mevcut `error_expected`) | bu tur |
| E3004 Reset sırası | `reset_sequence` anahtarı ayrışıyor, hiçbir geçit okumuyor. | Kapsam dışı: sıra semantiği (hangi alan önce bırakılır) bu ADR'nin modeliyle çelişmez ama ayrı tasarım ister. | E3004 rezerve kalır | gelecek |
| E3005 Koşullu reset | `reset_requires` gramerde yok. | Kapsam dışı. | rezerve | gelecek |
| `on clk.reset {}` | E0003. | Kapsam dışı (reset olayı bloğu bu ADR'ye bağlı değil). | — | gelecek |

> **Not (R5', Aşama 2):** W3xxx-B'nin uyarı olması **geçicidir**. Aşama 4'te
> örnekler (vga, hybrid_accel) ham reset portuna taşındıktan sonra hataya
> yükseltilmesi değerlendirilecek.
>
> **Karar (Aşama 5, 2026-09-22):** değerlendirildi — W3010 **uyarı kalır**,
> muafiyet niteliği yok. Gerekçe ve ölçümler §"Aşama 5".
>
> **Kod tahsisi (Aşama 2):** W3xxx-A → **W3009**, W3xxx-B → **W3010**
> (W3008'den sonraki ilk boş numaralar). Bu belgedeki sembolik adlar
> tasarım metninin parçası olarak korunur.

Minimum beklenti (R1, R2, R5) karşılanıyor: R1 zincir + W3xxx-A, R2 yapısal
(E3001), R5 E3003. Arch'ın beş fazı (rekabet raporu [38]) ile eşleme:
"one async reset in two clock domains" → R5/E3003; "cross-async-reset-domain
data path" → R2/E3001; "reset-driven clock gating" → imkânsız;
"reconvergent synchronizers (RDC)" → R6/E3003; "combiner-derived reset at
inst" → imkânsız. Eksik faz yok.

## Karar

### 1. İki tür reset girişi: sözleşmeli otomatik port ve ham açık port

Bugünkü otomatik port anlamını korur, ama sözleşmesi YAZILIR:

- **Otomatik port** (`rst`/`rst_n`, kullanıcı yazmaz): "bu port, bağlı
  olduğu saate senkron bırakılan bir reset alır" (IP sözleşmesi). Volt
  hiyerarşisi içinde derleyici bunu garanti eder: ebeveyn, çocuğa kendi
  (senkronlanmış ya da sözleşmeli) reset'ini bağlar. Birim kökünde
  sorumluluk dış dünyanındır → W3xxx-A.
- **Ham açık port** (`in ext_rst_n : reset(async, active_low)`): "bu reset
  asenkron gelir; bırakılması HİÇBİR saate senkron değildir." Derleyici,
  bu portun beslediği her saat portu için bir bırakma senkronizörü üretir
  ve alanın flop'larını o zincirin çıkışıyla sıfırlar. Bu porta sahip bir
  modülde aynı alanlar için otomatik port ÜRETİLMEZ.

Bağlama kuralları (K2 ile aynı ruh):

| Durum | Kural |
|---|---|
| Anotasyonsuz ham port | Modülün reset'i olan (`none` dışı) BÜTÜN alanlarını besler. |
| `@D` anotasyonlu ham port | Yalnız `D`'yi besler. Birden çok ham port varsa her biri anotasyonlu olmalı; aksi hâlde **E3010** (mevcut "ambiguous domain" tanısı, reset biçimi). |
| Ham portun `(sync\|async, polarite)` spesifikasyonu | Beslediği alanın spesifikasyonuyla aynı olmalı; değilse **E3003** (alt biçim: "port reset kind differs from domain"). Spesifikasyon yazılmazsa (`in r : reset`) beslediği alandan alınır. |
| Beslenmeyen alan | Otomatik port alır (bugünkü davranış). Otomatik port adı bir ham portun adıyla çakışırsa **E3003** (alt biçim, help: portu anotasyonsuz yap ya da yeniden adlandır). |
| Örneklemede ham port | Ebeveyn çocuğun ham portunu açıkça bağlar (`ext_rst_n: ext_rst_n`); değer yalnız ebeveynin bir ham portu olabilir (typeck: `Ty::Reset`, mevcut tip uyuşmazlığı tanısı). Bağlanmazsa E2005 (mevcut). |
| Çocuğun otomatik portu | Ebeveyn, çocuğun saat portuna bağladığı kendi saat portunun alan reset'ini bağlar: zincir varsa zincir çıkışı, yoksa kendi otomatik portu (bugünkü ad-ad bağlama bunun özel hâlidir). |

`reset = none` alanları hiçbir reset'ten etkilenmez (değişmez).

### 2. Reset senkronizörü: ham porttan türetilen yerleşik zincir (ADIM 1.3)

Ayrı bir `reset_sync()` çağrısı YOK (reddedilen alternatif §R1). Zincir,
ham portun varlığından türetilir; `sync()` ile aynı adlandırma disiplini:

```
in ext_rst_n : reset(async, active_low)     // Volt: tek satır
```

```systemverilog
    // reset synchronizer: ext_rst_n -> fast_clk (async assert, sync release)
    logic rst_sync_fast_clk_stage0;
    logic rst_sync_fast_clk_stage1;
    always_ff @(posedge fast_clk or negedge ext_rst_n) begin
        if (!ext_rst_n) begin
            rst_sync_fast_clk_stage0 <= 1'b0;
            rst_sync_fast_clk_stage1 <= 1'b0;
        end else begin
            rst_sync_fast_clk_stage0 <= 1'b1;
            rst_sync_fast_clk_stage1 <= rst_sync_fast_clk_stage0;
        end
    end
    // alanın register'ları:
    always_ff @(posedge fast_clk or negedge rst_sync_fast_clk_stage1) begin
        if (!rst_sync_fast_clk_stage1) begin ... end else begin ... end
    end
```

- Adlar: `rst_sync_<saat_portu>_stage<i>`; saat portu başına en fazla bir
  zincir. Senkronlanmış reset zincirin son aşamasıdır (ara tel yok).
- Aşama sayısı 2. (`@reset_stages(N)`, N ∈ 2..=4, port niteliği: gelecek
  iş; `sync3()` benzeri bir ihtiyaç ölçülünce.)
- Polarite: zincir ham portun polaritesiyle assert olur; çıkış polaritesi
  alanınkidir (farklıysa ilk aşamada evrilir).
- `sync` alan + ham port: aynı zincir üretilir, çıkış `if (!rst_sync_...)`
  ile senkron reset olarak kullanılır (assert asenkron, bırakma senkron —
  senkron reset için de doğru desen).
- ASYNC_REG: SV'ye yazılmaz (ADR-0054 kararı); XDC'de zincir hücrelerine
  `set_property ASYNC_REG TRUE` (§4).
- Çocuk örneklemesi: zincir çıkışı çocuğun otomatik portuna bağlanır;
  çocuk yeniden senkronlamaz (sözleşme). Böylece kademeli gecikme ve
  yakınsama yok.
- Simülasyon/formal etkisi: ham portlu tasarımda reset bırakma 2 çevrim
  gecikir. `volt test` harness'i reset'i en az 2 çevrim tutuyor
  (`examples/fir_filter_test.volt:5`); `volt verify`'daki reset varsayımı
  zincir uzunluğunu kapsamalı — Aşama 2'de ölçülür, gerekirse harness
  reset süresi 2 → 4.

Ölçülen SV/Yosys uyumu: yukarıdaki zincir biçimi (bir register'ın başka
register'ların asenkron temizleme pini olması) Yosys 0.36 `dfflibmap` ile
`$_DFF_PN0_` → `DFFR` hücresine sorunsuz eşlendi (§Ölçümler, 12 flop).

### 3. Tanılar (ADIM 2.3 için sözleşme; 5 parça, iki dil)

**E3003 — ana biçim (R5):**

```
error[E3003]: asynchronous reset 'rst_n' is shared by two clock domains without synchronization
  ┌─ top.volt:3:33
  │
3 │ domain Fast { clock = posedge, reset = async active_low }
  │                                ----------------------- asynchronous reset → port 'rst_n'
7 │ domain Slow { clock = posedge, reset = async active_low }
  │                                ----------------------- same port 'rst_n'
12│     in fast_clk : clock @Fast
  │        ^^^^^^^^ 'rst_n' can be released synchronously to at most one of 'fast_clk', 'slow_clk'
13│     in slow_clk : clock @Slow
  │
  = reason: a reset release that is synchronous to one clock is asynchronous to the
            other; registers of the second domain can leave reset in different cycles
            or go metastable (recovery/removal violation)
  = help: take the raw reset in explicitly; the compiler then synchronizes its release
          to each clock with a two-stage synchronizer:
              in rst_n : reset(async, active_low)
  = spec: domain-inference.md §5 (E3003), ADR-0065 §1
```

E3003 alt biçimleri (aynı kod, farklı mesaj): R6 yakınsama ("raw reset
'ext_rst_n' is synchronized twice on 'clk': in 'Top' and in instance 'u'";
help: çocuğun ham portunu kaldır, ebeveynin zinciri otomatik porttan
gelir), port/alan spesifikasyon uyumsuzluğu, otomatik port adı çakışması.

**W3xxx-A (R1, birim kökü):**

```
warning[W3xxx-A]: asynchronous reset 'rst_n' is assumed to be released synchronously to 'clk'
  = reason: 'Top' is not instantiated in this unit, so nothing in Volt synchronizes its reset release
  = help: if 'rst_n' comes from a pad or a power-on reset, declare it as raw and the compiler
          adds the synchronizer:
              in rst_n : reset(async, active_low)
  = spec: ADR-0065 §1
```

Modül başına bir kez; yalnız `async` alanı olan ve ham portu olmayan kök
modüllerde. Sözleşmeyi bilinçli kabul eden IP yazarı için susturma
niteliği bu turda yok (gelecek iş; W3xxx-A kabul edilebilir gürültü —
varsayılan `sync` alanlar etkilenmez).

**W3xxx-B (R5'):** "synchronous reset 'rst' is sampled by two clock domains
('sys_clk', 'pix_clk'); its release is asynchronous to at least one of
them" — aynı help. Modül başına bir kez.

**`reset = <ifade>`:** parser `error_expected("reset kind")` (mevcut E1xxx
sözdizimi tanısı), help "write `reset = sync active_high`". `clock = <ifade>`
için de aynı (aynı yol).

`volt explain` E3003 (metni bu ADR'ye göre yeniden yazılır; bugünkü
"dst <= sync(src, dst_clk)" örneği yanlış — CDC örneği), W3xxx-A, W3xxx-B:
EN + TR. Kod sayısı 120 → 122. Bu ADR yeni uyarılara sayı vermez:
`just consistency` ADR'de geçen her kodu enum'da arar; sayılar kodla
birlikte Aşama 2'de tahsis edilir (W3008 son kullanılan).

### 4. SDC stratejisi (ADIM 1.4): saat grubu yerine hedefli kısıtlar

**İlke:** SDC, derleyicinin TANIDIĞI senkronizörlerin giriş yollarını
tek tek zamanlama dışı bırakır; başka hiçbir yolu kapatmaz. Derleyicinin
kaçırdığı her alanlar arası yol zamanlama aracında normal bir yol olarak
kalır ve ihlal olarak görünür. Reset yolları (recovery/removal) hiçbir
zaman toptan kapatılmaz.

#### 4.1 Ölçümler (OpenSTA 3.1.0, `openroad/opensta`; Yosys 0.36+42, `hdlc/formal`; 2026-09-22)

Deney tasarımı `build/rdc-sta/top.sv` (geçici, gitignore): `clk_a` 10 ns,
`clk_b` 9 ns, dış `rst_n`; P1 doğru `sync()` köprüsü (`a_data →
sync_stage0 → sync_stage1`), P2 senkronizörsüz geçiş (`a_data2 → 4 kapı
→ b_bad`), P3 doğru reset (`rst_n → rstb_sync0/1 (clk_b) → b_user.RN`),
P4 yanlış alanın reset'i (`rsta_sync1 (clk_a) → b_wrong.RN (clk_b)`).
Minimal Liberty: `DFFR` hücresi `RN` pininde `recovery_rising` /
`removal_rising` yayları taşır. Flop hücreleri `<tel>_reg` adıyla
(`fixnames.py`, ADR-0054 §7 ile aynı yöntem).

| Sorgu | ESKİ (`set_clock_groups -asynchronous` + köprü false path) | YENİ (grup yok; köprüye `set_max_delay -ignore_clock_latency 10`; `set_false_path -from [get_ports rst_n]`) |
|---|---|---|
| P2 `report_checks -from a_data2_reg -to b_bad_reg` | `No paths found.` | `b_bad_reg/D … 80.90 80.90 0.00 (VIOLATED)`; tüm clk_a→clk_b: `-0.30 (VIOLATED)` |
| P1 `-from a_data_reg -to sync_stage0_reg` | `No paths found.` | `path delay` grubu, `9.60 (MET)` |
| P4 `report_checks -to b_wrong_reg/RN` (recovery, clk_a reset → clk_b flop) | `No paths found.` | `asynchronous` grubu, `80.80 80.30 0.50 (MET)` — analiz ediliyor |
| P3 `-to b_user_reg/RN` (aynı saat) | `8.50 (MET)` | `8.50 (MET)` |
| `report_check_types -recovery -removal` | yalnız aynı-saat uçlar | alanlar arası reset uçları da listede |
| `report_worst_slack -max` / `report_tns` | `8.50` / `0.00` | `-0.30` / `-0.30` |

Komut desteği sondası (`build/rdc-sta/probe.tcl`):

| Komut | OpenSTA 3.1.0 | Kanıt |
|---|---|---|
| `set_max_delay -datapath_only` | **YOK** | `Error 563: set_max_delay -datapath_only is not a known keyword or flag.` |
| `set_max_delay -ignore_clock_latency` | VAR | `/OpenSTA/sdc/Sdc.tcl:2826-2853` (`flags {-rise -fall -ignore_clock_latency -reset_path -probe}`); P1 "path delay" grubunda zamanlandı |
| `set_bus_skew` | **YOK** | `invalid command name "set_bus_skew"` |
| `set_false_path -to [get_pins {x_reg/RN …}]` | VAR (kabul) | sonda 3 boş sonuç |
| `set_false_path -from [get_cells src] -to [get_cells stage0]` (grup yok) | VAR; P2 hâlâ VIOLATED, P1 `No paths found.` | sonda 4 |
| `set_clock_groups -asynchronous -allow_paths` | VAR; yollar korunuyor (P2 VIOLATED) | sonda 6 — Vivado'da karşılığı yok, kullanılmaz |
| `report_check_types -recovery -removal` | VAR | `/OpenSTA/search/Search.tcl:426-450` |

Vivado ve Quartus (araçlar yerelde yok; belge ile):

| Komut | Vivado (XDC) | Quartus (SDC) |
|---|---|---|
| `set_max_delay -datapath_only` | VAR — UG835 `set_max_delay`; UG903 CDC bölümü: `set_clock_groups`, `set_max_delay -datapath_only` yolu "ya tümüyle yok sayar ya yalnız saat çarpıklığını"; **`set_clock_groups` `set_max_delay`'ı EZER** (UG906 TIMING-24 "Overridden Max Delay Datapath Only") | YOK (01signal "Timing exceptions": Quartus'ta `datapath_only` karşılığı yok) |
| `set_bus_skew` | VAR — UG903 "Constraining Bus Skew": gray kodlu işaretçiler için; **saat grubu, max delay ve false path tarafından EZİLMEZ** | YOK; karşılığı `set_max_skew` / `set_net_delay` (Intel Community: 16.1'den beri CDC yollarında `set_max_skew` sorunu bildiriliyor) |
| Reset senkronizörü | UG906 "Asynchronous Reset Synchronizer": ilk hücre senkronlanmış temizleme sinyaline bağlı, bırakma alıcı saate göre zamanlanabilir | — |

Sonuç: `-datapath_only` yalnız XDC'ye; genel `.sdc` OpenSTA/PrimeTime
sözlüğünü izler (`-ignore_clock_latency`). Quartus için ayrı stil
(`--sdc-style=quartus`, `set_net_delay`/`set_max_skew`) gelecek iş;
ADR-0054 zaten Quartus hücre adlandırmasını ertelemişti.

#### 4.2 Hedef bazlı tablo

`T_src` = kaynak saatin periyodu (`create_clock` varsa); yoksa
`set_max_delay` yerine `set_false_path` düşer ve dosyaya yorum yazılır.

| Köprü | Yol (`-from` → `-to`) | `.sdc` (genel; OpenSTA ile doğrulandı) | `.xdc` (Vivado; belge ile) | Recovery/removal etkisi |
|---|---|---|---|---|
| `sync()`/`sync3()` tek bit | `<kaynak>` → `sync_<x>_stage0_reg*` | `set_false_path` | `set_max_delay -datapath_only T_src` + `ASYNC_REG` (zincir) | yok (veri yolu) |
| `AsyncFifo` gray işaretçiler (çok bit) | `f_wgray_reg*` → `f_wgray_s0_reg*`, `f_rgray_reg*` → `f_rgray_s0_reg*` | `set_max_delay -ignore_clock_latency T_src` | `set_max_delay -datapath_only T_src` + `set_bus_skew T_src` + `ASYNC_REG` | yok |
| `AsyncFifo` veri | `f_mem_reg*` → `f_rd_data_reg*` | `set_false_path` (işaretçiyle nitelenmiş veri) | `set_false_path` | yok |
| `HandshakeSync` req/ack | `h_req_reg*` → `h_req_s0_reg*`, `h_ack_reg*` → `h_ack_s0_reg*` | `set_false_path` | `set_max_delay -datapath_only T_src` + `ASYNC_REG` | yok |
| `HandshakeSync` veri | `h_data_q_reg*` → `h_data_out_reg*` | `set_false_path` | `set_false_path` | yok |
| `PulseSync` toggle | `p_toggle_reg*` → `p_sync0_reg*` | `set_false_path` | `set_max_delay -datapath_only T_src` + `ASYNC_REG` | yok |
| `AsyncDualPortRam` | `m_mem_reg*` → `m_rd_data_reg*` | `set_false_path` | `set_false_path` | yok |
| Ham reset portu → zincir CLR pinleri | `[get_ports ext_rst_n]` → (hepsi) | `set_false_path -from [get_ports ext_rst_n]` | aynı + `ASYNC_REG` (`rst_sync_<clk>_stage0/1_reg*`) | Yalnız assert yolu kapanır (ham port, yapı gereği yalnız zincir CLR pinlerini sürer). Zincir çıkışı → flop CLR pinleri KISITSIZ kalır → recovery/removal alıcı saate göre analiz edilir (ölçüm P3). |
| Otomatik async port (sözleşmeli) | `[get_ports rst_n]` → flop CLR pinleri | kısıt YOK | kısıt YOK | Giriş gecikmesi olmayan port → araç "unconstrained" raporlar (`check_timing` no_input_delay): görünür, gizli değil. `set_false_path -from rst_n` YAZILMAZ (reddit deseni). |
| Alanlar arası diğer her yol | — | kısıt YOK | kısıt YOK | Normal zamanlama; senkronizörsüz geçiş ihlal olarak görünür (ölçüm P2). |

`set_clock_groups -asynchronous` ÜRETİLMEZ (varsayılan stil). Tek saatli
tasarımın çıktısı değişmez (grup zaten yoktu).

`set_false_path -to [get_cells {stage*}]` biçimi KULLANILMAZ: stage0 →
stage1 D yolunu da kapatır (zincir içi yol zamanlanmalı). `-to` her zaman
yalnız ilk aşamadır; ham reset için `-from port`.

#### 4.3 `--sdc-style`

`volt build --emit=sdc,xdc --sdc-style=targeted|clock-groups`; varsayılan
`targeted`. `clock-groups` ADR-0054 çıktısını bayt bayt korur (mevcut
akışlar, karşılaştırma) ve dosya başlığına `# Style: clock-groups — paths
between domains are NOT timed; see ADR-0065` yazar. Gerekçe: eski
davranışı silmek yerine seçilebilir bırakmak, Aşama 4'teki ESKİ/YENİ
karşılaştırmasını kalıcı bir CI kanıtına çevirir ve kullanıcının bilinçli
tercihini korur. Reset senkronizörü kısıtları her iki stilde üretilir.

#### 4.4 Recovery/removal: hangi kısıt korur, hangisi kapatır

| Kısıt | Etki | Karar |
|---|---|---|
| `set_clock_groups -asynchronous` | Alanlar arası recovery/removal KAPANIR (ölçüm P4 "No paths found") | üretilmez |
| `set_false_path -from [get_ports rst_n]` (otomatik port) | Tüm reset ağacının assert VE bırakma yolları kapanır | üretilmez |
| `set_false_path -from [get_ports ext_rst_n]` (ham port) | Yalnız ham → zincir CLR (assert, tanım gereği asenkron) kapanır; zincir çıkışı → CLR'ler açık | üretilir |
| `set_false_path -to [get_pins {*/CLR}]` | Kütüphaneye bağlı pin adı, tüm recovery kapanır | üretilmez |
| Kısıt yok | Recovery/removal alıcı saate göre analiz edilir (ölçüm P3/P4) | varsayılan |

### 5. Hücre adı tutarlılığı (Aşama 3)

`constraints/walk.rs:874-908` tablosu sv-emit adlarını elle aynalıyor (iki
doğruluk kaynağı). Aşama 3: üretilen her `get_cells` deseni, aynı
derlemenin SV çıktısındaki bir `logic`/register bildirimine eşleşmeli
(ADR-0063 regmap tutarlılığı fikri); test her köprü türü ve reset zinciri
için koşar. Paylaşılan sabit (tek kaynak) tercih edilir; sv-emit'te ad
değişikliği gerekirse ADR raporlanır.

## Reddedilen alternatifler

| Alternatif | Neden reddedildi |
|---|---|
| **`let rst_s = reset_sync(rst_async, clk)` çağrısı (görevin önerdiği biçim)** | Volt'ta reset bir değer değildir: çağrının sonucunu bir alana BAĞLAYACAK sözdizimi yoktur (`domain` bloğu dosya seviyesindedir, sinyal adı taşımaz). Değer biçimi reset'i veri yapar: R3 (veri olarak okuma) ve R6 (iki çağrı, birleşme) yeniden mümkün olur ve tip denetimi ister (typeck DOKUNULMAZ). Ham port biçimi aynı işi tek satırla, bağlama kuralı K2 ile yapar. |
| Otomatik `async` portu HER modülde kendiliğinden senkronlamak | Ebeveyn+çocuk aynı ham kaynağı ayrı zincirlerle senkronlar → R6 yakınsama (ölçülebilir 1 çevrim bırakma kayması). Ebeveynin senkronlanmış reset'ini çocuk yeniden senkronlarsa kademe başına +2 çevrim belirlenimci gecikme ve ebeveyn/çocuk bırakma sırası sorunu. Mevcut tüm `async` tasarımların SV'si sessizce değişir. |
| Alan başına ayrı otomatik port (`rst_n_fast`, `rst_n_slow`) | Port adları değişir (ADR-0012 isim korunumu, golden), sözleşme sorunu çözülmez (yine dış dünya senkronlamalı), R5 hatası yalnız porta taşınır. |
| R5' (`sync` reset paylaşımı) için hata | Çok saatli örneklerin tamamı bu desende; Aşama 2 CI'ı kırar, examples kapsam dışı. Uyarı + sınıf A rapor + tek satır düzeltme yolu. |
| Saat gruplarını KORUYUP `-datapath_only` eklemek | Vivado'da `set_clock_groups` `set_max_delay`'ı ezer (TIMING-24); kısıt etkisiz kalır. |
| `set_clock_groups -asynchronous -allow_paths` | OpenSTA/PrimeTime seçeneği; Vivado'da yok; iki lehçe arasında tek satır farkı (ADR-0054 §6) bozulur; hedefli kısıtlar aynı sonucu taşınabilir verir. |
| Genel `.sdc`'de de `-datapath_only` | OpenSTA `Error 563`; PrimeTime'da da yok. |
| Genel `.sdc`'de `set_bus_skew` | OpenSTA "invalid command"; Vivado'ya özgü. XDC'de üretilir. |
| Reset senkronizör çıkışını `set_false_path -to CLR` ile kapatmak | Recovery/removal analizini kapatır — hedef (C) ile çelişir. |
| Extern modül reset portları | Extern SV'ye emit edilmiyor (E0003); RDC sözleşmesi extern için ADR-0047 genişletmesi ister — gelecek iş. |

## Doğrulama planı (Aşama 4)

1. **İkinci ağ kanıtı (4.1):** Volt E3001 verdiğinden senkronizörsüz geçiş
   Volt kaynağıyla yazılamaz; `examples/vga` SV'sinin `build/` kopyasına
   elle bir `sys_clk → pix_clk` doğrudan atama eklenir (ya da bu ADR'nin
   `top.sv` deneyi). OpenSTA: `--sdc-style=clock-groups` → `No paths
   found.`; `targeted` → yol `VIOLATED`/`MET` olarak raporda. Bu ADR'nin
   §4.1 tablosu kalıcı kanıtın ilk sürümüdür; Aşama 4 aynı tabloyu Volt
   ÜRETİMİ SDC ile tekrarlar.
2. **Recovery/removal kanıtı (4.2):** `report_check_types -recovery
   -removal -format end` — eski stilde alanlar arası uç yok, yeni stilde
   var; ham portlu tasarımda zincir çıkışı → CLR uçları listede.
3. **Yanlış pozitif yok (4.3):** vga, hybrid_accel, soc yeni SDC ile
   temiz: senkronizör yolları `path delay` grubunda MET ya da false path;
   `report_tns` alanlar arası kaynaklı negatif slack yok. (Şu anki
   örneklerde W3xxx-B sınıf A: raporlanır; düzeltme örneklere tek satır ham
   port — kapsam kararı Aşama 4'te.)
4. **Yosys hücre eşleşmesi (4.4):** üretilen her `get_cells` deseni
   (köprüler + `rst_sync_<clk>_stage*`) Yosys netlistinde ≥1 hücre
   (ADR-0054 §7 yöntemi, `fixnames.py` benzeri son işlemci CI'a alınır).
5. **CI (4.5):** `openroad/opensta` imajı sürüm sabitli (3.1.0, digest
   `e04e5f38a0bc…`), önbellekli; 4.1 ve 4.3 kalıcı adım. Docker + küçük
   deney tasarımı (12 flop) — VGA'nın 8192 flop'u yerine bu ADR'nin
   `top.sv`'si ve `examples/hybrid_accel` (AsyncFifo, 2 saat).
6. **Belgeler (4.6):** README Limitations "No RDC checking" → gerçek
   durum (E3003/W3xxx-A/W3xxx-B kapsamı; E3004/E3005 hâlâ rezerve; extern
   reset yok); CHANGELOG ADR-0063/0064/0065.

## Ölçümler — bu ADR için yapılanlar

- Docker: `openroad/opensta` (`sta -version` → `3.1.0`), `hdlc/formal`
  (`Yosys 0.36+42`). Sentez: `read_verilog -sv top.sv; proc; opt; memory;
  techmap; dfflibmap -liberty min.lib; abc -liberty min.lib; write_verilog`
  → 12 `DFFR` (`stat`); `fixnames.py` 12 flop yeniden adlandırdı.
- STA: `/OpenSTA/build/sta -no_splash -exit sta_{old,new}.tcl` → §4.1
  tablosu; `probe.tcl` → komut desteği tablosu. Loglar `build/rdc-sta/`
  (gitignore; komutlar ADR'de, Aşama 4 CI'a alır).
- Kaynak tespiti: yukarıdaki durum tespiti tablosundaki satır atıfları.
- `cargo test` koşulmadı: kod değişmedi.

## Sınırlar / Ertelenen

- `@reset_stages(N)`: bu turda yok; zincir 2 aşama.
- W3xxx-A için susturma niteliği yok (sözleşmeyi bilinçli kabul eden IP yazarı
  uyarıyı görür).
- E3004 (reset sırası), E3005 (koşullu reset), `on clk.reset {}`: rezerve.
- Extern modüllerde reset sözleşmesi: extern emit edilene kadar yok.
- Quartus stili (`set_net_delay`/`set_max_skew`, `<ad>[i]` hücre adı):
  gelecek `--sdc-style=quartus`.
- Vivado komutları belge ile doğrulandı, araçla değil (Vivado yerelde
  yok); Aşama 3 `syntax_check` XDC sözlüğüne `-datapath_only`,
  `set_bus_skew` ekler.
- Otomatik `async` port sözleşmesi, giriş gecikmesi olmadığından
  zamanlama aracında "unconstrained" olarak kalır; `set_input_delay`
  üretimi (I/O zamanlaması) ayrı ADR.
- Ham port bağlama kuralında saat portu başına bir zincir varsayımı: bir
  saat portunun bir alanı vardır (E3010/K2), bu yüzden çelişki yok.

## Aşama planı ve kapsam (görev tanımından)

| Aşama | Kapsam | Bu ADR'deki karar |
|---|---|---|
| 2 RDC denetimi | `domain/rdc.rs` (E3003 R5/R6/alt biçimler, W3xxx-A, W3xxx-B), parser `reset = ifade` hatası, sv-emit ham port + zincir + örnekleme bağlantısı, stdlib.md bölümü, `explain` EN/TR, ui pass/fail, mutasyon, golden (RDC'siz dosyalar bayt bayt aynı) | §1, §2, §3 |
| 3 Hedefli SDC | `constraints/` + volt-sdc-emit: §4.2 tablosu, `--sdc-style`, XDC `ASYNC_REG` + `set_bus_skew`, hücre adı tutarlılık testi | §4, §5 |
| 4 Doğrulama | OpenSTA CI adımı, README/CHANGELOG | Doğrulama planı |

## Aşama 2 uygulama notları (2026-09-22)

Uygulama `crates/volt-hir/src/domain/rdc/` (facts, binding, share,
converge) ve `crates/volt-sv-emit/src/reset_sync.rs`. Karardan sapma yok;
aşağıdakiler ADR'nin açık bıraktığı ya da ölçümle netleşen noktalardır.

| Nokta | Uygulanan | Gerekçe / ölçüm |
|---|---|---|
| "İki saat alanı" (R5, R5', R1) | Yalnız modülde **kullanılan** saat portları sayılır (on bloğu, `reg(clk)`, `sync()`, örnek bağlaması). | Flop'u olmayan saat reset örneklemez; `tests/ui/pass/13` (kullanılmayan `fast_clk`) aksi hâlde W3010 alırdı (ADIM 2.4 sınıf C → kural daraltıldı). |
| Etkin reset | `reset` yazılmamış alan ve örtük saat alanı `sync active_high` sayılır (sv-emit'in ürettiği). | HIR `DomainInfo.reset` bu durumda `None` taşır; denetim sv-emit'in gerçek çıktısına bakar. |
| R6 yerel zincir | Modülün kendi zinciri, saat kullanıcı örneği bağlaması DIŞINDA kullanılıyorsa ya da bir çocuğun otomatik reset'li saatine bağlıysa sayılır. | Ham portu yalnız geçiren ara seviyenin zinciri ölü mantıktır; yakınsama sayılsaydı yanlış alarm olurdu (`rdc_tests::convergence_through_a_pass_through_level_is_found_at_the_top`). |
| Kaskad | E3010 (belirsiz ham port) olan modülde paylaşım/kök denetimi koşmaz; E3003 (R5) olan modülde W3009 basılmaz. | Kullanıcının niyeti ham porttur; aynı sorun iki kez raporlanmaz. |
| W1001 | Ham reset portu "kullanılmayan giriş" sayılmaz; süzgeç `rdc::without_raw_reset_unused` (analyze, sürücü, LSP). | Kullanım takibi `resolve/` içinde ve bu görevde DOKUNULMAZ; senkronizör portu örtük okur. |
| Harness (§2 "gerekirse 2 → 4") | Reset süresi 2 çevrim kalır; ham portlu tasarımda bırakmadan sonra zincir uzunluğu (2) kadar çevrim daha koşulur. | Reset'i uzun tutmak gecikmeyi kapatmaz — gecikme bırakmadan SONRA oluşur. Ölçüm (Verilator 5.050, Docker): ham portlu sayaç `reset()` sonrası otomatik portlu eşiyle aynı başlar (`count` 0 → 1); ham port bırakıldıktan sonra register'lar 2 çevrim reset'te kalır, 3. çevrimde sayar. Ham portsuz tasarımların testbench'i bayt bayt aynı (`sim_golden_tests`). Otomatik `rst_n` portu (önceden de) harness'te sürülmüyordu; ham portlu modülde artık adıyla sürülür. |
| `volt verify` | `initial assume` ve kontrat koruması zincir çıkışına uygulanır. | Ölçüm (sby, `hdlc/formal`): ham portlu sayaç `invariant: c <= 9` prove (k=8) ve bmc (16) geçti. |
| Kod sayısı | 128 → 130 (§3'teki "120 → 122" o günün sayısını yanlış aktarmıştı). | `explain_tests::CODE_COUNT`. |
| R5' sınıf A listesi | vga, hybrid_accel ve 9 ui fixture'ı W3010 alır; **soc almaz** (tek saatli). Async reset'li mevcut tek tasarım `tests/ui/pass/19` (W3009). | `build/rdc_golden.py cli`. |

## Aşama 3 uygulama notları (2026-09-22)

Uygulama: `crates/volt-hir/src/constraints/` (model: `CrossingClass`,
`Crossing`, `ResetChain`, `RESET_SYNC_STAGES`; `walk.rs`: geçiş sınıfları
ve `reset_chains`), `crates/volt-sdc-emit/` (`SdcStyle`, `crossing_rule`,
`resets_section`, `domains_note`, `syntax_check` + `set_bus_skew`),
`crates/volt-driver` (`--sdc-style`), `docs/spec/cli-contract.md §5`,
`tests/ui/pass/87_sdc_targeted_all_bridges.volt`. §4.2 tablosundan sapma
yok; aşağıdakiler ADR'nin açık bıraktığı ya da ölçümle netleşen
noktalardır.

| Nokta | Uygulanan | Gerekçe / ölçüm |
|---|---|---|
| "Tek saatli tasarımın çıktısı değişmez" (§4.2) | Kaynak saati OLMAYAN geçişte (alansız giriş portu; `examples/i2c`'de pad girişinin `sync()`'i) `set_false_path` yorumsuz yazılır ve köprü bölüm başlığı yalnız iki+ alanlı modülde değişir. | İlk uygulama her hedefli dosyaya yorum/başlık farkı koyuyordu; golden'da 20 tek alanlı kısıt dosyası değişmişti. Bağlanacak periyot yokken false path zaten doğru kısıttır. Şimdi tek alanlı modülün iki stildeki çıktısı bayt bayt aynı (`targeted_single_clock_output_equals_clock_groups_minus_style_line`). |
| `T_src` (§4.2) | Geçiş başına kaynak saat: `Crossing.src_clock`. AsyncFifo okuma işaretçisi (`rgray → rgray_s0`) ve HandshakeSync `ack` ters yönlüdür; kaynakları köprünün HEDEF saatidir. | Ölçüm: fixture 87'de `f_wgray` 10.000 (fast), `f_rgray` 40.000 (slow). Yazma işaretçisinin periyodunu okuma tarafına vermek kısıtı 4 kat gevşetirdi. |
| Komut biçimi | `.sdc`: `set_max_delay -ignore_clock_latency <T> -from ... -to ...`; `.xdc`: `set_max_delay -datapath_only <T> -from ... -to ...` + `set_bus_skew -from ... -to ... <T>` (değer Vivado sözdiziminde sonda). | OpenSTA 3.1.0 (`openroad/opensta`, Docker) fixture 87'nin üretilmiş `.sdc`'sini **hiç Error vermeden** okudu; `report_check_types -recovery -removal` iki `asynchronous` grubu listeledi. |
| Hücre adı tutarlılığı (§5) | Paylaşılan sabit yerine TEST: `sdc_emit_tests::every_generated_cell_pattern_names_a_register_in_the_generated_sv` üretilen her `get_cells` desenini hiyerarşide çözüp (`<Modül> <örnek> (` satırı) hedef modülün SV'sinde `logic` bildirimi + `<=` ataması olan bir register arar. 8 kaynak × 2 stil = 318 desen; türler: sync, AsyncFifo, HandshakeSync, PulseSync, bellek, reset zinciri. | `volt-sv-emit` `volt-hir`'e bağımlı DEĞİL (ters bağımlılık katman ihlali olurdu); `RESET_SYNC_STAGES` iki yerde yaşar ve mutasyonla korunur (M1: SDC adı, M2: sv-emit adı — ikisi de yakalandı). Negatif denetim: `cell_name_check_rejects_a_misnamed_pattern`. |
| Ham reset false path'i | Modülün KENDİ ham portları için bir kez (`set_false_path -from [get_ports r]`); alt modül zincirleri aynı portun `-from` kapsamındadır, ayrıca kısıt almaz. Zincir aşamaları yalnız yorumla (XDC'de ek `ASYNC_REG`) listelenir. | §4.4: zincir çıkışı → temizleme pinleri açık kalmalı. `reset_release_paths_are_never_excluded` testi `-to [get_pins`, `-from [get_cells {rst_sync` ve `[get_ports rst_n]` (otomatik port) satırlarının YOKLUĞUNU denetler. |
| `--sdc-style` yeri | `volt-driver` `build` alt komutunda (`SdcRequest`); görev tanımının Aşama 3 kapsamında yazılı değildi. | Bayrak sürücüde tanımlanır; `volt-sdc-emit` yalnız `SdcStyle`'ı model olarak taşır (saf üretici). |
| `reset = none` | Zincir üretilmez (hem sv-emit hem SDC). | `child_raw_reset_chain_is_prefixed_and_domain_without_reset_has_none`, `unannotated_raw_reset_feeds_every_domain_with_a_reset`. |

Ölçümler:

- **Golden** (`build/sdc_golden.py`, 788 dosya; temel Aşama 2 sonrası main
  `f94f5be`): `--sdc-style=clock-groups` çıktısı ADR-0054 ile bayt bayt
  aynı — tek fark `# Style:` satırı ve ham reset portlu 5 tasarımda
  "Reset synchronizers" bölümü (10 dosya). Varsayılan hedefli stilde 748
  dosya bayt bayt aynı, 86 kısıt dosyası değişti; hepsi ya iki+ alanlı ya
  ham reset portlu. RTL, SVA ve sürücü çıktılarında fark YOK.
- **Testler**: `cargo test --all` 0 başarısız; `just consistency` 130 kod,
  2610 test (2577 → 2610, +33: 2471 `#[test]` + 76 pass + 63 fail).
- **Mutasyon** (`build/sdc_mutate.py`, tek tek, `CARGO_BUILD_JOBS=2`):
  19/19 yakalandı — zincir adı (SDC ve sv-emit tarafı ayrı), geçiş sınıfı
  ve yönü (gray/kontrol/veri, rgray ve ack ters yön), varsayılan stil (CLI
  ve `SdcStyle::default()`), `# Style:` satırı, `-ignore_clock_latency`,
  `set_bus_skew`, ham reset false path'i, ASYNC_REG lehçesi, iki alan
  eşiği (başlık ve yorum), `.sdc` kontrol geçişi, `syntax_check` sözlüğü,
  `reset = none`, anotasyonlu ham port bağlaması. İlk koşuda ikisi
  (CLI varsayılanı, hedefli köprü başlığı) KAÇTI; ikisi için test eklendi
  (`default_style_is_targeted_and_multi_domain_bridge_header_says_so`).
- **OpenSTA**: yukarıdaki satır; ayrıntı ve Aşama 4 devri `build/sdc-sta/`.

Örneklerin eski/yeni çıktısı (ADIM 3.3; `--emit=sdc`, tam fark
`build/sdc-golden/`):

| Dosya | ESKİ (ADR-0054) | YENİ (hedefli) |
|---|---|---|
| `VgaTop.sdc` | `set_clock_groups -asynchronous -group {sys_clk} -group {pix_clk}` | grup yok; açıklayıcı yorum |
| `VgaTop.sdc` `fb/mem` (RAM verisi) | `set_false_path` | `set_false_path` (değişmedi) |
| `VgaTop.xdc` `sync_done_r` | `set_false_path -from done_r_reg* -to sync_done_r_stage0_reg*` | `set_max_delay -datapath_only 10.000 ...` |
| `VgaTop.xdc` `sync_vs_active` | `set_false_path -from [get_clocks pix_clk] ...` | `set_max_delay -datapath_only 39.722 -from [get_clocks pix_clk] ...` |
| `HybridTop.sdc` gray işaretçiler | iki `set_false_path` | `set_max_delay -ignore_clock_latency 5.000` (rgray, kaynak b_clk) ve `2.500` (wgray, kaynak t_clk) |
| `HybridTop.xdc` gray işaretçiler | `set_false_path` + `ASYNC_REG` | `set_max_delay -datapath_only` + `set_bus_skew` + `ASYNC_REG` |
| Her ikisi, alanlar arası başka yol | kapalı (grup) | **zamanlanır** |

Aşama 4'e aktarılan açık noktalar:

1. `fixnames.py` (ADR-0054 §7) yalnız TEK BİTLİK flop'ları
   `<tel>_reg` yapar; çok bitli register'lar (`fifo_wgray[4:0]`,
   `acc_r[7:0]`) Yosys'in `_NNN_` adıyla kalır. 4.4'teki "her desen bir
   hücreye eşleşiyor" kanıtı için son işlemcinin `<tel>_reg[i]` biçimini
   üretmesi gerekir.
2. `flatten` hiyerarşik adları `u.x` yapar, SDC `u/x` bekler (ADR-0054'te
   de not edilmişti): 4.4 ya hiyerarşiyi koruyarak (`link_design` ile
   çoklu modül) okumalı ya da son işlemci `.` → `/` çevirmeli.
3. Yosys `opt` özdeş register'ları birleştirir: fixture 87'de üst modülün
   `go_r`/`sync_go_r_*` zinciri, aynı ifadeyi süren `u.level_r`/
   `u.sync_level_r_*` ile tek flop'a indi. 4.4 deneyinde register'lar
   ayırt edilebilir olmalı (farklı ifadeler) ya da `opt` sınırlandırılmalı.
4. Aşama 2 notu (W3010 → hata yükseltmesi) SDC tarafını değiştirmez:
   örnekler ham porta taşındığında dosyalara yalnız "Reset synchronizers"
   bölümü eklenir (golden'da 5 tasarımda ölçüldü).

## Aşama 4 doğrulama notları (2026-09-22)

Doğrulama planının 1-6. maddeleri Volt ÜRETİMİ SDC ile tekrarlandı ve CI'a
alındı. Araç: `scripts/sta/run.py` (Volt → Yosys → `fixnames.py` →
OpenSTA; beklentiler betiğin içinde, biri düşerse çıkış 1), `scripts/sta/
min.lib` (§4.1'deki Liberty: `DFFR.RN` recovery/removal yayları, kapı 0,30
ns, kurulum 0,10 ns). Ölçüm ortamı: OpenSTA 3.1.0 `f89887b596`
(`openroad/opensta@sha256:e04e5f38a0bc…`), Yosys 0.36+42 (`hdlc/formal`,
yerel) / OSS CAD Suite 2026-09-21 (CI). Aynı 31 beklenti Yosys 0.66
(`hdlc/yosys`) ile de tutar (tek fark `worst slack max` 6,30 — farklı
kapı eşlemesi). CI'daki Yosys 0.69+77 HybridTop netlistinde `signed`
bildirimleri yazdı (0.36/0.66 yazmaz; AllBridges'te işaretli tip yok,
o yüzden 87 geçti) ve OpenSTA `clean_named.v line 18, syntax error`
verdi; `fixnames.py` `signed` sözcüğünü soyar (zamanlama işarete bağlı
değil), ikinci koşu 7/7 yeşil. Loglar `build/sta/<Modül>/`.

**Deney tasarımı:** `tests/ui/pass/87` (AllBridges — her köprü türü, iki
ham reset, çocuk örnek `u`, 100 MHz / 25,175 MHz). Plan `examples/vga`'yı
ya da §4.1'in `top.sv`'sini önermişti; 87 seçildi çünkü Volt'un kendi
SDC'sini ve sv-emit'in gerçek adlarını sınar, 353 flop ile CI'a sığar ve
saat oranı tam sayı olmadığından senkronizörsüz geçiş ihlal olarak
görünür (VGA'nın 8239 flop'u yalnız 4.3 için yerelde koşuldu).

**Sondalar (4.1/4.2):** üretilen `AllBridges.sv`'nin `build/` KOPYASINA
iki flop eklendi (Volt kaynağında yazılamaz: E3001 ve E3003): P2
`rdc_probe_cdc` (`slow_clk`, D = `go_r & sync_start_src`, ikisi
`fast_clk`) ve P4 `rdc_probe_rst` (`slow_clk`, asenkron temizleme
`rst_sync_fast_clk_stage1` — fast saatinin zinciri). `(* keep *)` ile
`opt_clean`'den korunur.

### 4.1 İkinci ağ kanıtı ve 4.2 recovery/removal (sondalı netlist, 355 flop)

| Sorgu | `--sdc-style=clock-groups` (ESKİ) | `--sdc-style=targeted` (YENİ) |
|---|---|---|
| P2 `report_checks -from go_r_reg -to rdc_probe_cdc_reg` | `No paths found.` | `rdc_probe_cdc_reg/D 2820.16 2820.60 -0.44 (VIOLATED)` |
| P4 `report_checks -to rdc_probe_rst_reg/RN` (recovery, fast zinciri → slow flop) | `No paths found.` | `rdc_probe_rst_reg/RN 2820.06 2820.30 -0.24 (VIOLATED)` — analiz ediliyor |
| P3 `report_checks -to go_r_reg/RN` (aynı saat, zincir çıkışı → CLR) | `9.80 0.30 9.50 (MET)` | `9.80 0.30 9.50 (MET)` |
| `report_checks -to [get_pins -hierarchical */RN]` uç sayısı | 35 | 36 (+P4) |
| `report_tns` | `0.00` | `-0.68` |

Kenar ilişkisi: OpenSTA `Warning 1010: No common period was found
between clocks fast_clk and slow_clk` basar (10 / 39,722 ns) ve yine de
en dar pencereyi bulur: fast kenarı 2820, slow kenarı 71 × 39,722 =
2820,262 → 0,26 ns; clk→Q 0,30 + AND 0,30 + kurulum 0,10 sığmaz. Bu,
ilişkisiz iki saat arasında senkronizörsüz yolun neden her zaman ihlal
olduğunun ölçümüdür: geçerli bir kurulum penceresi yoktur.

Ham reset ve otomatik port kararları (§4.4) doğrulandı: her iki stilde
zincir çıkışı → temizleme pinleri (`go_r_reg/RN` dâhil 35 uç) recovery/
removal olarak zamanlanır; yalnız YANLIŞ saatin zinciriyle temizlenen
flop eski stilde kaybolur.

### 4.3 Yanlış pozitif yok ve 4.4 hücre eşleşmesi (temiz netlistler, hedefli stil)

| Tasarım | Flop | `get_cells` deseni → Yosys hücresi | Alanlar arası zamanlanan uç | `worst slack max` / `tns` |
|---|---|---|---|---|
| AllBridges (87) | 353 | 22/22 (1 … 128 hücre; `u/level_r_reg*`, `u/sync_level_r_stage0_reg*` hiyerarşik) | 10, hepsi MET (`fifo_wgray_s0_reg[0..4]/D 9.30`, `fifo_rgray_s0_reg[0..4]/D 39.32`, `path delay` grubu) | 6,60 / 0,00 |
| HybridTop | 774 | 8/8 (`fifo_mem_reg*` 512, `fifo_rd_data_reg*` 32) | 10, hepsi MET (gray işaretçiler `path delay` 1.80 / 4.30) | −20,70 / −19 246 — TÜMÜ saat İÇİ (0,30 ns kapılı oyuncak kütüphanede 2,5 ns periyot); alanlar arası uçta ihlal yok |
| VgaTop | 8239 | 7/7 (`fb/mem_mem_reg*` 8192) | 0 (tüm köprüler `set_false_path`) | 6,60 / 0,00 |
| SocTop | 580 | desen yok (tek saat, `frequency` yok → `create_clock` yok) | — | — |

Desen eşleşmesi, `.sdc`'yi okuyan OpenSTA'nın kendi `get_cells`
sorgusuyla sayıldı (metin karşılaştırması değil). Bir desenin
eşleşmemesi güvenli tarafa düşer ama gürültüdür: `fixnames.py`'nin ilk
sürümü HybridTop'ta hiçbir flop'u adlandıramadığında (`\b` yerine
backspace) `fifo_mem → fifo_rd_data` false path'i uygulanmadı ve 5 bellek
verisi ucu `VIOLATED` göründü — 4.4 denetimi bu yüzden CI'da 4.3 ile
birlikte koşar.

Aşama 3'ten devralınan açık noktalar kapandı: (1) `fixnames.py` çok
bitli ve bellek dizili register'ları `\<ad>_reg[i]` / `\<ad>_reg[w][b]`
kaçışlı adla adlandırır, tekillik modül başına denetlenir (aynı `cnt`
iki modülde olabilir); (2) `flatten` YOK — hiyerarşi Yosys'te korunur,
`/` ayırıcı OpenSTA `link_design`'dan gelir; (3) `opt` YOK — yalnız
`opt_expr`/`opt_clean`, özdeş register'lar birleşmez (87'de `go_r` ve
`u/level_r` ayrı). SocTop'ta `stat` 584 / adlandırılan 580 farkı çok
örnekli modülün hiyerarşik sayımıdır (`clean_named.v`'de adsız `DFF _`
kalmadı).

### 4.5 CI

`.github/workflows/ci.yml` → `timing` işi (zorunlu): Yosys formal
işiyle aynı sabit OSS CAD Suite önbelleğinden; OpenSTA `openroad/opensta`
özet (digest) sabitli, `docker save` tar'ı `actions/cache` ile
(`opensta-<digest>` anahtarı), `opensta:pinned` etiketi. Adımlar:
`run.py --probe --require-zero-tns` (87: 4.1 + 4.2 + 4.3 + 4.4, 31
beklenti) ve `run.py --design examples/hybrid_accel/hybrid_top.volt --top
HybridTop` (9 beklenti); `summary.md`, `.sta.log` ve `.sdc` dosyaları
artifact olarak yüklenir. Yerel koşum: `VOLT_STA_YOSYS="docker run --rm
-v {work}:/work -w /work hdlc/formal yosys"`, `VOLT_STA_STA="docker run
--rm -v {work}:/work -w /work --entrypoint /OpenSTA/build/sta
openroad/opensta"`.

### 4.6 Belgeler ve sapmalar

README "No RDC checking" satırı gerçek kapsamla (E3003 üç biçim, W3009,
W3010; E3004/E3005 rezerve; extern reset yok; SDC'de saat grubu yok)
değiştirildi; CHANGELOG'a ADR-0063/0064/0065 girdileri eklendi (0064 PR
#11'de merge bekliyor, girdi bunu söyler). Karardan sapma yok. Kapsam
notu: kalıcı CI betikleri gitignore'daki `build/` yerine `scripts/sta/`
altında (görev tanımı `build/`'i geçici sayar; CI'ın izlediği dosyaya
ihtiyacı var). Kalan sınırlar §"Sınırlar / Ertelenen"deki gibi: Vivado
`.xdc` yalnız sözdizimi düzeyinde (`-datapath_only`, `set_bus_skew`
OpenSTA'da yok), Quartus stili yok, örnekler hâlâ W3010 (otomatik port;
Aşama 5'te taşındı).

## Aşama 5 — örneklerin ham porta taşınması ve W3010 kararı (2026-09-22)

Aşama 2 notunun (R5' "geçici uyarı") kapanışı. Adımlar: (1) W3009/W3010
üreten her dosyanın tespiti, (2) taşıma, (3) yükseltme kararı, (4)
golden.

### 5.1 Tespit

`volt check` (180 dosya: `tests/ui`, `tests/fixtures`, `examples`;
`build/rawreset_scan.py`) 20 dosyada E3003/W3009/W3010 buldu:

| Sınıf | Dosyalar | Neden bu desende | İşlem |
|---|---|---|---|
| Tanının kendi testi | `fail/62–65`, `fail/67`, `fail/68` | Kural testleri | dokunulmadı |
| Bilinçli W3009 | `pass/19` (async + negedge + active_low) | sv-mapping §7'nin otomatik port varyantını sınar; ham port SV'yi değiştirir, testin amacı kaybolur | gerekçeli kalır |
| Gerçek paylaşım (iki alanda da reset'li flop) | `examples/vga/vga_top`, `examples/hybrid_accel/hybrid_top`, `hybrid_test` (HybridTb), `pass/27`, `28`, `29`, `74` | ADR-0065 öncesi yazıldılar; tek otomatik `rst` | ham porta taşındı |
| **Yanlış pozitif** — saat reset'li flop sürmüyor | `examples/vga/frame_buffer`, `pass/65`, `fail/51` (`AsyncDualPortRam` yazma tarafı: yalnız bellek dizisi, reset'siz — ADR-0049); `pass/62`, `fail/49`, `fail/50` (saatler yalnız extern örneğine gider; extern'in reset portu yok) | Aşama 2'nin "kullanılan saat" kuralı her örnek bağlamasını sayıyordu | kural inceltildi (5.2), taşıma GEREKMEDİ |

Taşıma denemesinde bir **derleyici hatası** çıktı: VgaTop ham porta
geçince `FrameBuffer` örneğinin tek `rst` portuna `rst_sync_sys_clk_stage1`
bağlanıyordu (`instance.rs::parent_synced_reset` ilk saati seçer), oysa
FrameBuffer'ın tek reset'li flop'u `pix_clk` tarafındaki RAM okuma
register'ıdır. Bırakma pix saatine asenkron kalırdı — taşınmış örnekte
sessiz bir RDC hatası.

### 5.2 Kural inceltmesi (kapsam sapması: volt-ast, volt-sv-emit)

"Reset örnekleyen saat" = reset'li bir flop sürebilecek bir yerde
kullanılan saat. Reset taşımayan bağlamalar:

- `BuiltinPrim::clock_resets_flops(port)` (volt-ast, tablo):
  `AsyncDualPortRam.wr_clk` (`builtin_always_ff_no_reset`) ve
  `PriorityArbiter.clk` (yalnız `assign`) `false`; diğer her saat
  portu ve bilinmeyen ad `true` (tutucu).
- Extern örneğinin her bağlaması (extern'e reset gitmez).

Uygulama: volt-hir `rdc/facts.rs::reset_free_bindings` (`ClockFact.used`
bu bağlamaların İÇİNDEKİ kullanımları saymaz; `converge.rs::chain_used`
aynı kümeyi atlar) ve volt-sv-emit `reset_sync.rs::reset_free_clock`
(çocuğun otomatik portu için aday saatlerden yalnız reset'siz
bağlamada kullanılanlar elenir; `on clk`, `reg(clk)`, kullanıcı örneği
bağlaması ya da modül gövdesinde `sync(_, clk)` saati örnekleyen sayar;
aday kalmazsa eski davranış). volt-sv-emit volt-hir'e bağımlı olmadığı
için kural iki yerde yaşar; ortak tablo volt-ast'te, iki taraf mutasyonla
korunur. `Arena::iter` (volt-ast) `sync()` araması için eklendi.

Kapsam notu: görev tanımı volt-hir/volt-diagnostics'i sayıyordu; sv-emit
hatası taşımayı yanlış yapacağı için kullanıcı onayıyla (ikisi birden)
düzeltildi.

### 5.3 Taşıma

Her tasarıma `in rst : reset(sync, active_high)` eklendi (HybridTb ham
portunu HybridTop'a geçirir). Alanlar `sync active_high` olduğu için
tür/polarite aynı kalır; port adı `rst` kalır (harness adla sürer); tek
davranış farkı bırakmadan sonraki 2 çevrimlik zincir gecikmesidir
(harness bunu Aşama 2'den beri bekler).

| Tasarım | Zincir | Çocuk bağlaması | Lint (Verilator 5, `-Wall`) | `volt test` | Formal | SDC |
|---|---|---|---|---|---|---|
| VgaTop (+FrameBuffer, VgaTiming) | `rst_sync_sys_clk`, `rst_sync_pix_clk` | FrameBuffer ve VgaTiming ← pix zinciri | temiz | 7/7 | `bmc 24` **15/15** (main: 14/15) | "Reset synchronizers" +7 satır |
| HybridTop | `t_clk`, `b_clk` | TernaryArray/Ctl ← t, BinaryFront/Array ← b | temiz | 6/6 (hybrid_test) | `bmc 16` **32/32** (main: 31/32) | +7 |
| HybridTb (test sarmalayıcısı) | `t_clk`, `b_clk` (ölü) | HybridTop ← ham `rst` | 5 UNUSEDSIGNAL: 3'ü main'de de var (`top_*` çıkışları), 2'si ölü zincir (aşağıda) | — | — | +9 |
| pass/27 FifoBridge | fast, slow | — | temiz | — | `bmc 24` 3/3 | +7 |
| pass/28 HsBridge | fast, slow | — | temiz | — | 2/2 | +7 |
| pass/29 PulseBridge | fast, slow | — | temiz | — | 2/2 | +7 |
| pass/74 Bridge | fast, slow | — | temiz | — | 2/2 | +7 |
| pass/88 (yeni) Display + FrameStore | sys, pix | FrameStore ← pix zinciri | temiz | — | 3/3 | — |

Ölçüm ortamı: Docker `verilator/verilator` (lint + `volt test`, 9 örnek
test dosyası 123/123 geçti), `hdlc/formal` (sby); karşılaştırma main
`314f5c5` ikilisi `build/wt-main` ağacında (`build/rawreset_formal.sh`).

**Formal bulgusu:** main'de iki çok saatli tasarım `bmc`'de düşüyordu:
VgaTop `invariant: wx_r < 80` ve HybridTop AsyncFifo değişmezi, ikisi de
döngü 2'de. Bu, `examples/vga/README.md`'deki "formal sarmalayıcı reset
sırası" açığıdır: `rst` bir saatin ilk kenarından önce bırakılır, o
saatin register'ları hiç reset görmeden başlar. Ham portta `initial
assume` zincir çıkışına uygulanır (Aşama 2) ve zincir bırakmayı saate
senkronladığından her register ilk örneklenen kenarında reset'tedir —
açık bu tasarımlar için kapanır. Otomatik portlu çok saatli tasarımlarda
açık sürüyor.

**Ölü zincir (sınır):** yalnız ham portu çocuğa geçiren ara seviye
(HybridTb) yine de kendi zincirlerini üretir; Aşama 2 bunu yakınsama
saymamıştı (`chain_used`), sv-emit ise hâlâ emit ediyor. İşlev etkisi
yok (sentez budar); sv-emit'te "zincirsiz geçiş" ayrı iş.

### 5.4 Yükseltme kararı: W3010 uyarı kalır

**Soru:** bütün örnekler temizken W3010 hataya yükseltilsin mi?

**Ölçüm (belirleyici):** iki alanda reset'li flop'u olan bir çocuk,
aynı iki saatte kendi flop'ları olan bir ebeveynin altında
(`rdc_tests::multi_domain_child_under_a_synchronizing_parent_has_no_error_free_form`):

| Çocuğun reset'i | Sonuç |
|---|---|
| otomatik `rst` | çocukta W3010 (tek port iki saatin zincirini taşıyamaz — ebeveyn birini bağlar) |
| ham `rst`, ebeveyn ham `rst`'yi geçirir | **E3003 ×2** (R6: aynı ham reset her saatte iki kez senkronlanır) |
| iki anotasyonlu ham port | yine R6 (değer yalnız ebeveynin ham portu olabilir) |

Yani bu yapının bugün **hatasız bir biçimi yok**. Yaygın bir yapı: tepe
modülünde yapıştırıcı mantık + altında çift saatli bir alt blok (VGA'da
FrameBuffer'ın yazma tarafında tek bir reset'li sayaç olsaydı VgaTop
yazılamazdı).

| Seçenek | Değerlendirme |
|---|---|
| Hataya yükselt | Yukarıdaki yapıyı yazılamaz kılar. **Reddedildi.** |
| Hata + `@allow(shared_reset)` (ADR-0048 `@allow` mekanizması) | Tek geçerli kullanım durumu dil sınırıdır, tasarımcının bilinçli seçimi değil; susturma gerçek bir tehlikeyi (ikinci alanın asenkron bırakması) gizler ve kalıcı API yüzeyi ekler. Volt'ta "ilişkili saat" kavramı yok (her alanlar arası veri yolu E3001), bu yüzden "tasarımcı zamanlamayı garanti ediyor" durumu da dilde ifade edilemez. **Reddedildi.** |
| Yalnız birim kökünde hata | Aynı modül bir birimde kök, diğerinde çocuktur (`volt check examples/vga/frame_buffer.volt` kökü FrameBuffer yapar): birime göre değişen önem derecesi. **Reddedildi.** |
| **Uyarı kalır** (seçilen) | Kökte düzeltme tek satır ve tanının help'i onu söyler; `--deny-warnings` sıkı kullanıcıya hata davranışı verir. Mevcut kaçış yolları: ham port (önerilen), alan başına anotasyonlu ham port, ikinci alanı `reset = none`. |

Geriye uyumluluk: kullanıcı yok; yine de uyarı kalması hiçbir kodu
kırmaz. İnceltme (5.2) yalnız yanlış pozitifleri kaldırır; W3010'un
kalan her örneği gerçek bir paylaşımdır.

**Yeniden değerlendirme koşulu:** çok saatli çocuğa saat başına otomatik
reset portu (`rst_<saat>` ya da eşdeğeri; sv-emit + RDC) geldiğinde her
W3010'un hatasız bir biçimi olur; o zaman muafiyetsiz hataya yükseltme
yeniden açılır. `explain W3010` ve tanı notundaki "geçici" dili kalktı
(`explain_tests` bunu denetler).

### 5.5 Golden (`build/rawreset_golden.py`, main `314f5c5` ↔ dal)

791 dosya (`tests/ui`, `tests/fixtures`, `examples`, `build/` külliyatları)
için `volt check --format=json` + `volt build --emit=sva,sdc,xdc`: **776
bayt bayt aynı**, 15 fark (2'si yeni fixture):

| Dosya | Fark | Açıklama |
|---|---|---|
| `vga/vga_top` | check `W3006,W3010,W3010 → W3006`; `VgaTop.sv` +34 −8; `.sdc` +7; `.xdc` +9; `vgatop.sva` 10 satır | iki zincir; `if (rst)` → zincir çıkışı; FrameBuffer/VgaTiming `.rst` ← pix zinciri; SVA `disable iff`/`initial assume` zincire, satır atıfları +6 (yorum) |
| `vga/frame_buffer` | check `W3006,W3010 → W3006` | yalnız tanı (inceltme) |
| `hybrid_accel/hybrid_top` | check `W3010 → —`; `HybridTop.sv` +36 −10; `.sdc` +7; `.xdc` +9; `.sva` 8 satır | aynı desen |
| `hybrid_accel/hybrid_test` | check `W3010 ×2 → —`; `HybridTb.sv` +26; `HybridTb.sdc` +9 | sarmalayıcıya ham port + ölü zincir |
| `pass/27`, `28`, `29` | check −W3010; `*.sv` +28 −2; `.sdc` +7; `.xdc` +9 | zincir; otomatik `rst` portu kalkar |
| `pass/74` | check `W3005,W3010 → W3005`; `Bridge.sv` +30 −4; `.sdc` +7; `.xdc` +9 | aynı |
| `pass/62`, `pass/65`, `fail/49`, `50`, `51` | yalnız check (−W3010) | inceltme; SV/SDC bayt bayt aynı |
| `pass/88`, `fail/70` | yeni | 5.2 fixture'ları |

Her `.sdc` farkı yalnız "Reset synchronizers" bölümüdür (Aşama 3 notu
4'ün öngördüğü gibi): `set_false_path -from [get_ports rst]` + zincir
yorumları; `.xdc`'de ek `ASYNC_REG`.

### 5.6 Testler ve mutasyon

- `cargo test --all` 0 başarısız; `just check`, `just consistency` (131
  kod, 2665 → 2683 test: 2542 `#[test]` + 77 pass + 64 fail),
  `just clippy-strict` temiz.
- Yeni: `rdc_tests` 7 (RAM yazma tarafı, register'lı/`sync`'li yazma
  saati, extern ±flop, ölü ebeveyn zinciri, hiyerarşi kararı; RDC dışı
  hata güvencesi), `rdc/tests.rs` 1, `reset_sync_tests` 3 (pix zinciri,
  dört tutucu durum, aday kalmayınca geri dönüş), volt-ast
  `builtin_tests` 2 + `arena_tests` 1, `explain_tests` karar metni;
  ui `pass/88`, `fail/70`.
- Mutasyon (`build/rawreset_mutate.py`, tek tek): **16/16 yakalandı** —
  tablo (her giriş ayrı ve tamamen), `used` süzgeci, extern dalı, konum
  içerme, `chain_used`, sv-emit aday süzgeci, geri dönüş, `on`/`reg`/
  yerleşik/kullanıcı örneği kolları, `sync` araması ve modül aralığı.
