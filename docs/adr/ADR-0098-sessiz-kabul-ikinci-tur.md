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

Sayılar ve yer yer tablo PR açıklamasındadır.

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
