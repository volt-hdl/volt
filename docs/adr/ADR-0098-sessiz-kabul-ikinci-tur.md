# ADR-0098: Sessiz Kabul, İkinci Tur — Register'a `=` E0019, Etkisiz Nitelik W0024, Ayrılmış Reset Anahtarları, Gerçek Reset Portlu Testbench, Joker Kol Kuralı

> Statü: Uygulandı
> İlgili: ADR-0073 (çift sürücü sondaları d04/d04b/d15 artık sürücü analizinden önce E0019 alır), ADR-0048 (W0021'in tamamlayıcısı: W0024 yanlış yerdeki ve bağlanmayan nitelik), ADR-0065 (E3004 rezervi; `reset_cycles`/`reset_sequence` E0003), ADR-0033 (testbench reset'i), ADR-0058 (test dili), ADR-0097 (saatsiz modülde kontrat E5005; bu ADR saatsiz modülün `volt test`'ini düzeltir).
> Tarih: 2026-10-02
> Etkilenen: volt-syntax (`parser/register_assign.rs` YENİ: E0019;
> `parser/attr_use.rs` YENİ: nitelik defteri + yerleşim tablosu, W0024;
> `parser/item.rs`: defter kaydı, alan anahtarı E0003;
> `parser/module_expr_stmt.rs`: saat seçimi `RegClocks` ile ortak),
> volt-sv-emit (`lib.rs` `collect_written`: `for` gövdesi; `sim.rs`:
> reset portları), volt-diagnostics (E0019, W0024; explain en/tr),
> volt-hir, volt-sv-emit, volt-lower (`clippy::wildcard_enum_match_arm`),
> testler, `book/src/limitations.md`, `docs/roadmap.md`, CHANGELOG.

## Sorun

PR #72'nin "2b" taraması, derleyicinin yanlış ya da eksik kodu tanısız
kabul ettiği dört yer daha buldu. Kontratlarla ilgili beşinci bulgu
(saatsiz modülde kontrat) ADR-0097'de E5005 ile kapandı. Ölçümler
2026-10-02, `main` = 345bf563, Docker Verilator 5.052:

| # | Girdi | Önce | Bu ADR ile |
|---|---|---|---|
| 1 | `reg r : [u8; 2] = [7; 2]`, yalnız `on clk { for i in 0..2 { ... r[i] <= ... } }` içinde yazılıyor | Reset dalı boş (`if (rst) begin end`), reset sonrası `r[0]` = 0 | `for (int volt_i ...) r[volt_i] <= 8'd7;`, reset sonrası 7 |
| 2 | Modül düzeyinde `r = r + 1` (`r` reg) | `assign r = r + 8'd1;` (kombinasyonel döngü, reset değeri kayıp), yalnız W3001 | E0019, öneri `on clk { r <= r + 1 }` |
| 3a | `@no_auto_contracts` (ya da herhangi bir nitelik) bir kontratın önünde | Ayrıştırılıp atılıyor, tanı yok | W0024 "not attached to anything" |
| 3b | `domain D { ..., reset_cycles = 4 }` | Ayrıştırılıyor, hiçbir geçit okumuyor | E0003 "not supported yet" |
| 4 | Saat portu olmayan modülün `volt test`'i | C++ derlemesi düşüyor: `VAdder` has no member `rst` | Testler koşar |

## Karar

### 1. Reset dalı `on` bloğunda yazılan her register'ı kapsar

`collect_written` (sv-emit) `BlockStmt::For`'u `_ => {}` ile atlıyordu.
Fonksiyon artık her `BlockStmt` varyantını açık kolla ele alır; `for`
gövdesine (iç içe `for`, `for` içinde `match`, `match` kolunda `for`)
iner. sv-mapping.md §16.3 bu çıktıyı zaten gösteriyordu. `if`/`else if`
zinciri ve `match` blok kolları önceden de geziliyordu (kıyas testi);
deyim konumunda ifade gövdeli `match` kolu atama yapamaz (E0003).

### 2. E0019: register'a `on` bloğu dışında `=`

Register yeni değerini yalnız saat kenarında, `on` bloğunda `<=` ile alır
(sv-mapping.md §3-4). Modül düzeyinde, `comb` bloğunda ya da modül düzeyi
`for` gövdesinde `=` ile atanan register E0019'dur. E0007'nin ("`<=`
`on` dışında") aynası olduğu için E0 ailesinde ve parser'da denetlenir:
öneri kaynak metnini göstermek ister (`on clk { r <= r + 1 }`) ve
kaynak metni yalnız parser'da vardır. Saat seçimi E0007 ile ortaktır:
açık `reg(clk)`, yoksa modülün tek saat portu, yoksa `<clock>`. Modül
düzeyindeki deyimde öneri deyimi o bloğa çeviren bir düzenleme de taşır
(`MaybeIncorrect`); blok içindeki deyim tek başına taşınamaz.

Adlar modül gövdesinin `reg` bildirimlerinden okunur; blok `let`'i aynı
adı gölgelerse o ad register sayılmaz. Mevcut bir kod yeniden
kullanılmadı: E0007'nin başlığı ve explain metni `<=` hakkındadır, ters
durumu anlatmaz.

Sonuç: ADR-0073'ün çift sürücü sondalarından üçü (`d04` reg `on` + `comb`,
`d04b` reg `on` + modül düzeyi, `d15` reg modül düzeyinde iki kez) artık
sürücü analizine ulaşmadan E0019 alır.

**Kombinasyonel döngünün diğer yolları** (ölçüldü, tanı yok, bu ADR'nin
kapsamı dışında): birbirini besleyen teller (`a = b + x`, `b = a`),
atadığı teli okuyan `comb` bloğu, çıkış portları ve alt modül örneği
üzerinden döngü. Dördünü de Verilator `-Wall` UNOPTFLAT ile yakalar
(`comb` bloğu ayrıca ALWCOMBORDER); `volt test` bu tasarımlarda
Verilator hatasıyla durur. Genel döngü denetimi yol haritasında
("Combinational loop check"), bilinen sınırlarda da yazılı. Tel `on`
bloğunda `<=` ile yazılırsa (`wire w`, `on clk { w <= x }`) reset'siz bir
flop olur; bu da tanısızdır ve ayrı bir karar ister.

### 3. Hiçbir nitelik sessizce atılmaz: W0024

Gramerde nitelik yalnız Item, Port, StructField ve Stmt önünde yazılır
(grammar-full.ebnf §2); Contract'ın niteliği yoktur. Parser'ın nitelik
düşürdüğü yollar: modül ve pipeline gövdesinde kontrat; pipeline'da
`stage`/`stall`/`flush`; gövdenin kapanış `}`'i; `use`, `package` ve
dosya sonu; bozuk struct alanı. Bağlanan ama o düğümde hiçbir geçidin
okumadığı nitelikler de sessizdi (`@strict_timing` bir portta, tek
başına `@offset`, `@reg` sıradan bir `reg` bildiriminde).

Mekanizma iki parçalıdır (`parser/attr_use.rs`):

1. **Defter.** `parse_attributes` her niteliği, ardından gelen token
   türüyle bir deftere yazar. Öğeler ayrıştırıldıktan sonra (desugar
   öncesi, kaynak konumunda) AST'de bulunmayan her tanınan nitelik W0024
   alır. Yeni bir ayrıştırma yolu nitelik listesini unutursa defter onu
   yakalar; yolları tek tek düzeltmek gerekmez. İleti ardından gelen
   yapıya göre seçilir (kontrat, pipeline cümlesi, `use`/`package`, son).
2. **Yerleşim tablosu.** `placement(ad)` her tanınan niteliğin okunduğu
   yerleri listeler (modül, extern, diğer öğe, port, extern portu, struct
   alanı, `@reg` bildirimi, `reg` deyimi, diğer deyim). Kendi geçidi
   yanlış yeri zaten raporlayan nitelik için o yer de listededir (çift
   tanı olmasın): uygulanmayan nitelikler W0021 (ADR-0048), `@source`
   E0009 (ADR-0076), `@timing`/`@false_path`/`@multicycle` E0017
   (ADR-0054). `@reg` register alanındaki her nitelik `mmio` desugar'ında
   okunur ya da E0009 alır; o yer tablonun dışındadır. Tanınan her
   niteliğin tabloda olması bir birim testiyle zorlanır: yeni nitelik
   eklenince nerede okunduğu da yazılmak zorundadır.

Bilinmeyen ad zaten W0020 alır ("yok sayılır"); W0024 ikinci kez
uyarmaz. W0024 uyarıdır: W0020 ile aynı ağırlık, kullanıcının elindeki
kodu derlemeyi durdurmaz.

### 4. `reset_cycles` ve `reset_sequence`: E0003

İki anahtar gramerde (§3 DomainKey) ayrılmıştır ve ayrıştırılır; anlamları
spec'te tanımlı değildir (yalnız `docs/design/` taslaklarında), E3004
(reset sırası) ADR-0065'te rezerve kaldı. Uygulamak yeni bir tasarım
ister. Sessizce yok saymak kullanıcıya olmayan bir denetimi vaat
ettiğinden, yazıldıklarında E0003 ("not supported yet") verilir; değer
yine ayrıştırılır. Bilinen sınırlar sayfasında yazılı. Gramerdeki güç
anahtarları (`voltage`, `always_on`, `retention`, `isolation`, [V1])
parser'da tanınmaz ve W0020 alır; sessiz değildir, bu ADR değiştirmez.

### 5. Testbench reset'i modülün gerçek portlarını sürer

`apply_reset`, ham reset portu olmayan her modülde `dut->rst` yazıyordu.
Üç durumda bu port yoktur ve `volt test`/`volt run` C++ derlemesinde
düşüyordu (üçü de ölçüldü): saat portu olmayan modül, `reset = none`
alanı, `active_low` alanı (`rst_n`). `collect_sim_ports` otomatik reset
portlarını artık her zaman emitter'ın `reset_port_set`'inden ekler
(üretilen SV'deki portların aynısı); üreteç yalnız var olanları
polaritesiyle sürer. Senkronizör beklemesi yalnız ham reset varken kalır:
varsayılan alanın testbench metni birebir aynıdır, test zamanlaması
değişmez (örneklerin 123 testi geçti). Reset portu yoksa model bir kez
`eval()` ile oturtulur: Verilator'ın ilk değerlendirmesi saatin önceki
değerini kaydeder, o yapılmadan ilk posedge kenar sayılmaz (ölçüldü:
`reset = none` register'ı bir çevrim sonra hâlâ 0).

Port listesi (`collect_sim_ports`) artık otomatik `rst`/`rst_n`'yi de
taşır; GTKWave oturumu onu, ADR-0065'ten beri olduğu gibi, yalnız ham
reset'li modülde gösterir (kitaptaki ekran görüntüsü geçerli kalır).

### 6. Joker enum kolu kuralı

`collect_written` hatası bir sınıfın örneğidir: bir enum'u `_ =>` ile
gezen kod, varyant eklendiğinde (ya da baştan unutulduğunda) o varyantı
sessizce düşürür. volt-hir, volt-sv-emit ve volt-lower crate kökünde
`#![warn(clippy::wildcard_enum_match_arm)]` açılır; CI'ın clippy adımı
uyarıyı hataya çevirir (`-D warnings`). Mevcut joker kollar üç kurala
göre çevrildi:

- **Sorgu** ("bu şekil mi?", joker "değil" demek): `if let`, `let ...
  else` ya da `matches!`; davranış aynı, niyet açık. Yeni varyant doğal
  olarak "değil" tarafına düşer.
- **Gezinti, alçaltma, üretim**: kalan varyantlar açık kollarla
  yazılır (aynı gövde); bilerek atlanan varyantın yanında gerekçe yorumu.
- Varyantın o aşamada oluşamadığı kanıtlanabiliyorsa `unreachable!`
  (kod tabanının ICE kuralı); kuşkulu durumda davranış korunur.

Sayım (2026-10-02): 244 joker kol — volt-hir 154 (53 sorgu, 95 açık
kol, 5 yeni `unreachable!`, bir tanesi zaten öyleydi), volt-sv-emit 90
(27 sorgu, 59 açık kol, 4 `unreachable!`), volt-lower 0. Hiçbir yerde
`#[allow]` gerekmedi. Her `unreachable!` çağıranın muhafızıyla kanıtlı
(ör. `arith_result` yalnız `+ - * / %` ile çağrılır). Üretilen SV ve
tanılar 617 `.volt` dosyasında (`--emit sva`, iki kip) önce/sonra
birebir aynı. Yer yer tablo PR açıklamasındadır.

Tarama bir sessiz kabul daha buldu ve bu ADR'de kapandı: tekli `-`
`bits<N>` üzerinde E2004, `bool`/`clock`/`reset`/dizi/tuple üzerinde E2003
(önceden tanısız Error, SV'ye `assign y = -b;` gidiyordu).

Taramanın bulup bu ADR'de **kapatmadığı** yerler (açık, ayrı iş):

- `--emit=sva` ayrı dosyada kontratın part-select içinde okuduğu sinyal
  (`r[0 +: 4]`) checker modülünün portlarına eklenmez; üretilen `.sva`
  bildirilmemiş adı okur (`sva.rs` `collect_signal_names`). Kontrattaki
  dizi literali içindeki `prev()` da toplanmaz (`past.rs`). İkisi de
  kontrat/formal alanında.
- `domain D { clock = none }` kabul edilir ve `always_ff @(posedge clk)`
  üretir; kenarsız saat alanının anlamı karar ister.
- İfade konumundaki `match`'in kapsayıcılık hatası (E0014), birimde E1xxx
  varken kapalı-kapı yedeğinde görünmez (deyim `match`'i görünür);
  E1xxx düzeltilince gelir.
- Genişlik geçerliliği (E2025) yalnız modül düzeyi tiplerde denetlenir;
  blok `let`'i ve fn parametresindeki `bits<0>` E2005 alır, kullanılmayan
  `type W = bits<0>` hiç tanı almaz.

## Sınırlar

- Kombinasyonel döngü denetimi genel değildir (§2).
- `wire` `on` bloğunda `<=` ile yazılırsa reset'siz flop olur; tanı yok
  (§2).
- W0024 yerleşim tablosu, nitelikleri okuyan geçitlerin bugünkü hâlinin
  elle yazılmış aynasıdır; bir geçit yeni bir yerde okumaya başlarsa
  tablo da güncellenmelidir (aksi hâlde o yerde yanlış W0024 görünür).
  Ters yön (tablo izin verir ama geçit okumaz) yalnız inceleme ile
  yakalanır.
- Joker kuralı volt-syntax'te (parser desugar'ı) açılmadı: bu ADR'nin
  kapsamı alçaltma, HIR ve SV üretimidir.

## Ek: Son sessiz yanlışlar (2026-10-02)

Yukarıdaki karar metni değişmedi. Bu ek, "Taramanın bulup bu ADR'de
kapatmadığı yerler" listesinden dördünü v0.1 kuralına göre kapatır:
başarılı görünüp yanlış sonuç veren bilinen hata kalmaz; açık bir hatayla
duran sorun bilinen sınırlarda yazılı kalabilir. Ölçümler `main` =
3b54e1fd, Docker Verilator 5.052, `hdlc/formal` (Yosys + SymbiYosys).

| # | Girdi | Önce | Bu ek ile |
|---|---|---|---|
| E1 | `domain Async { clock = none }`, `in clk : clock @Async`, `on clk { r <= d }` | `always_ff @(posedge clk)`, tanı yok | E3016 |
| E2 | `wire w : u8`, `on clk { w <= d }` | Reset dalı boş flop (`if (rst) begin end else w <= d`), tanı yok | E0020, öneri `reg w : u8 = 0` |
| E3a | `--emit=sva` ayrı dosya, kontrat `r[0 +: 4] == ...` | `r` checker portu değil; Verilator "Can't find definition of variable" | `r` port olur |
| E3b | `volt test`, kontrat `r2 == [prev(a), prev(a)]` | `$past(a)`: test içi `reset()` sonrası ilk çevrimde reset öncesi değer okunur, özellik geçer | Zincir `past_a_1`; skaler yazımla aynı çevrimde (6) ihlal |
| E4 | Birbirini besleyen teller, `comb` bloğunda kendini okuyan tel | `volt build` uyarısız; `volt test`/`volt run` UNOPTFLAT, çıkış 3 | Kod değişmedi; bilinen sınırlar satırı netleşti |

### E1. Kenarsız alanda register: E3016

Spec `clock = none`'a gramerde yer verir (grammar-full.ebnf §3 ClockEdge,
ast-nodes.md `ClockEdge::None`), anlamını tanımlamaz; tasarım taslağı
"saatten bağımsız" der. `on` bloğu değerini saat kenarında örnekleyen
register tanımlar; kenarı olmayan alanda örneklenecek an yoktur. sv-emit
bunu `posedge` yazıyordu (kaynakta "mevcut davranış" yorumuyla).

Karar: kenarsız alandaki bir saatin `on` bloğu E3016'dır (E3 ailesi: saat
alanı kuralı). Denetim alan geçidindedir (`domain/edgeless.rs`): bloğun
alanı ancak alan tablosu çözüldükten sonra bilinir, parser bilmez.
Birincil etiket `on clk`'teki saatte, ikincil etiket alan tanımında. Öneri
iki yolu gösterir: alana kenar vermek (`clock = posedge`) ya da sinyali
kombinasyonel yazmak (`comb` / modül düzeyinde `=`). Kenarsız alandaki
kombinasyonel sinyaller (`in a : u8 @Async`, `y = a + b`) geçerli kalır.
Mevcut bir kod uymadı: E3001/E3012 alan uyuşmazlığı, E3010 belirsizlik
içindir.

### E2. Tele `on` bloğunda `<=`: E0020

sv-mapping.md §16.3: "`wire x : T` → `logic ... x;` bildirimi; sürücüsü
`comb` ya da `assign`." Spec teli saat kenarında yazılan bir sinyal olarak
tanımlamaz; hata bu tanımı değiştirmez, yalnız tanımın dışında kalan
girdiyi reddeder.

E0020, E0019 gibi parser'dadır (`parser/wire_assign.rs`): öneri bildirimin
kaynak metnini yeniden yazar. Her `on` bloğu (if/else, match kolu, `for`
gövdesi dahil) taranır; blok `let`'i aynı adı gölgelerse tel sayılmaz.
`on` içinde tele `=` zaten E0006'dır. Öneri `reg w : T[ @Alan] = 0`
(`bool` için `= false`); dizi, struct, enum ve tuple tiplerinde sıfır tek
sözcükle yazılamadığından yardım metni `<reset value>` gösterir ve makine
önerisi verilmez.

### E3. Kontrat toplayıcıları

İki bulgu da aynı kökten: `sva.rs` `collect_signal_names` ve `past.rs`
`collect_prev_calls` ifade ağacını kendi elle yazılmış kollarıyla geziyor,
parça seçimine (yalnız sva.rs), dizi/tuple/struct literaline inmiyordu.

- **E3a, açık hata.** Ayrı `.sva`'daki checker modülü yalnız portlarını
  görür; bağlanan modülde basit ad yukarı doğru çözülmez. Eksik port her
  zaman derleme hatasıdır (Verilator `--lint-only --assert`: "Can't find
  definition of variable: 'r'"); yanlış sinyali denetleyen bir biçim yok.
  Düzeltme küçük olduğu için yapıldı.
- **E3b, sessiz yanlış (`volt test`).** Toplanmayan `prev()`
  Immediate/Simulation kipinde zincir yerine `$past`'e düşüyordu.
  ADR-0040'a göre reset sonrası ilk çevrimde `prev(x) == 0`'dır; `$past`
  ise reset öncesi değeri verir. Ölçüm (Docker Verilator): `!b || r2 ==
  [prev(a), prev(a)]` ile skaler eşi `!b || r2[0] == prev(a) && r2[1] ==
  prev(a)`, `a = 7` iken test içi `reset()` sonrasında: skaler yazım cycle
  6'da ihlal verdi, dizi literalli yazım geçti. `volt verify`'da (Yosys)
  aynı girdi sözdizimi hatasıyla durur (açık).

İki toplayıcı da artık `volt_ast::visit`'in tam çocuk kümesini kullanır
(`walk_expr`, `expr_children`); yeni bir ifade biçimi eklenince ayrıca
güncellenmeleri gerekmez. Ad sırası aynı kaldı: depodaki 628 `.volt`
dosyasının 700 çıktısı (`--emit sva`, ayrı + gömülü) önce/sonra birebir
aynı. `prev_array_sim_tests` CI'ın araçlı adımında zorunlu koşar.

### E4. Kombinasyonel döngüler

Kod değişmedi. Ölçüm: `a = b + x`, `b = a` ve `comb { c = x; c = c + y }`,
`y = c` için `volt build` döngülü SV'yi yazar, döngüye dair uyarı vermez
(yalnız ilgisiz W1001). `volt test` ve `volt run` Verilator'da
`%Warning-UNOPTFLAT ... Circular combinational logic: 'y'` ve döngü
boyunca bir örnek yol basar, `error: Verilator failed for module` ile
çıkış 3 verir. Bilinen sınırlar satırı bu iletiyi ve `volt build`'in
uyarısız yazdığını artık açıkça söyler; ileti Volt'un kendi sözleriyle
açıklanmaz (Verilator metni olduğu gibi geçer).

### Bu ekin kapatmadığı yerler (raporlandı, ayrı iş)

Sınıflandırma ölçümle; kod değişmedi.

- **Sessiz yanlış — kenarsız saat `on` dışındaki flop'larda.** E3016 yalnız
  `on` bloğunu kapsar. Kenarsız alandaki saat şu yollarla yine `posedge`
  flop zamanlar: `sync(d, aclk)` hedefi ve kenarsız kaynaklı `sync()`'in
  kaynak flop'u (`always_ff @(posedge aclk)`), yerleşik primitif
  (`SyncFifo { clk: aclk, ... }`), alt modül örneğinin saat portu
  (`Child { clk: aclk }`, alt modül `posedge` yazar) ve kontratlar
  (`@(posedge clk)`).
- **Sessiz yanlış — `out` portu `on` bloğunda `<=`.** `out q : u8`,
  `on clk { q <= d }` reset dalı boş bir flop üretir (E2 ile aynı sınıf);
  tanı yok. Depodaki üretilen SV'de bu kalıp yok (tarandı).
- **Açık hata — kontratta dizi literali.** `volt verify`'da her biçim
  Yosys sözdizimi hatası (`'{...}`, "unexpected OP_CAST"); `volt test`'te
  iki literal ya da paketlenmiş (port) dizi ile karşılaştırma Verilator
  hatası ("Assignment pattern member not underneath a supported
  construct"); ayrı `.sva`'da unpacked dizi register'ı ile `$past`'li
  literal karşılaştırması Verilator iç hatası ("internal fault"). Yalnız
  `volt test`'te dizi register'ı ile literal karşılaştırması derlenir.
