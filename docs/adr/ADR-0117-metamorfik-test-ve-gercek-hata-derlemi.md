# ADR-0117: Metamorfik Test ve Gerçek Hata Derlemi — Aynı Tasarımın İki Yazımı Aynı Tanıyı ve Aynı Donanımı Verir

> Statü: Kısmen yerini aldı: ADR-0123 — §5'in gecelik tohumu ve saati (tohum koşunun başlangıcından 12 saat önceki UTC tarihi)
> İlgili: ADR-0108 (fark testi ifade düzeyinde dokuz biçimi karşılaştırır; bu test tasarım düzeyinde yazımları), ADR-0109 (bilinen başarısızlık kaydının issue denetimi C kayıtlarınınkiyle aynı mekanizmadır, #283), ADR-0113 (beklenen sonuç derleyiciden değil şartnameden gelir; güvenlik kodu envanteri aynıdır), ADR-0119 (niyet ilkesi: yazımdan bağımsız çıkarım metamorfik testle sınanır), ADR-0123 (fark testinin gecesi aynı issue betiğini ve aynı tohum kuralını kullanır)
> Tarih: 2026-11-16
> Etkilenen: `crates/volt-driver/tests/metamorphic_tests.rs` (YENİ),
> `crates/volt-driver/tests/metamorphic/` (YENİ: `lex.rs`, `edit.rs`,
> `rewrites/`, `corpus.rs`, `check.rs`, `sim.rs`, `known.rs`,
> `known_failures.json`), `tests/real_bugs/` (YENİ),
> `scripts/ci/known_failures.py` (YENİ), `scripts/ci/nightly_issue.py`
> (YENİ), `.github/workflows/ci.yml`,
> `.github/workflows/known-failures.yml` (YENİ),
> `.github/workflows/metamorphic-nightly.yml` (YENİ), `AGENTS.md`,
> `CHANGELOG.md`

## Sorun

#233'teki sessiz yanlış sonuçların çoğu aynı tasarımın iki yazımıydı ve
iki yazım farklı davranıyordu: alan başlatma kısaltması ile uzun biçim
(#204), `if` ile `match` (#215), deyim match'i ile ifade match'i (#216),
`let` ile `comb` (#217), `wire` ile `let` üzerinde güven açıklaması
(#211), satır içi modül ile örnek (#210), `clock` ile tür takma adı
(#234). Hepsini bir insan buldu. Derlemdeki her tasarıma bu yazım
değişikliklerini uygulayan bir araç onları kendisi bulurdu (#254).

Kapanan sessiz hataların yeniden üretimleri de dağınıktı: ui fikstürleri,
sürücü testleri, issue metinleri. Biri yeniden ortaya çıksa hangi testin
düşeceği belli değildi.

## Karar

### 1. Gerçek hata derlemi

`tests/real_bugs/`: v0.1.0'dan beri kapanan her sessiz yanlış issue'sunun
en küçük yeniden üretimi, dosya başına bir issue, numara adın başında
(`204_field_init_shorthand.volt`); çok dosya gerekiyorsa giriş dosyası
`main.volt` olan dizin. İlk satırlar `volt check`'in beklenen sonucudur,
şartnameden: `//~ E3001` (her kod bir satır; hata ve uyarı kodlarının
kümesi tam olarak budur) ya da `//~ PASS`. Ardından `// #N: ...`
açıklaması: ne yanlış gitti, doğru cevabı veren şartname bölümü ya da ADR,
düzelten PR. Yanlış değer üreten hatanın dosyası şartnameden elle
hesaplanmış değerlerle `test` blokları taşır; koşucu onları `volt test`
ile Verilator'da koşturur.

**Kapsam.** Derlem Volt'un hatalarıdır: derleyicinin, standart
kütüphanenin ya da ürettiği çıktının yanlış sonucu. Volt dışındaki bir
aracın hatası (örneğin #148: Verilator 5.052'nin DFG eniyileyicisi doğru
SV'yi yanlış derliyor) derleme girmez; bir Volt tasarımı onu
göstermez, düzeltmesi de Volt'un çıktısını değiştirmez. Böyle hataların
kalıcı testleri araçla ilgili yerdedir (#148 için
`crates/volt-driver/tests/verilator_canary_tests.rs`).

Açık bir sessiz hata issue'sunun yeniden üretimi de, bir denetim onu
göstermek için gerektiğinde, düzeltmeden önce eklenebilir; başarısızlığı
bilinen başarısızlık listesindedir (§4). Silent-wrong düzelten her PR
yeniden üretimini ekler (yoksa) ve bilinen başarısızlık kaydını siler
(`AGENTS.md`).

Derlem bu testin, metamorfik testin ve (Kalite 3, #255) fark testinin
tasarım derleminin girdisidir.

### 2. Yeniden yazımlar: her biri bir şartname cümlesine dayanır

Bir yeniden yazım, bir yapının bir yazımını şartnamenin aynı tasarım
dediği öteki yazıma çeviren küçük bir fonksiyondur
(`metamorphic/rewrites/`). Kuralları:

- **Kaynak.** Her yeniden yazım iki biçimi eşdeğer kılan şartname
  cümlesini ya da ADR'yi `source` alanında taşır. Şartname bir şey
  söylemiyorsa yeniden yazım eklenmez; boşluk bulgu olarak raporlanır
  (şartnameye dokunulmaz, karar bir ADR'yle verilir).
- **Derleyiciden bağımsız.** Yeniden yazım derleyicinin ayrıştırıcısını
  kullanmaz: kendi küçük sözcük çözümleyicisiyle (`lex.rs`) metni okur,
  yalnız tanıdığı biçimdeki yerlere dokunur, öteki her metni olduğu gibi
  bırakır. #204 tam olarak ayrıştırıcıdaki bir açılımın denetimlere
  görünmemesiydi; derleyicinin AST'sinden yazılan bir yeniden yazım aynı
  hatayı miras alırdı.
- **Birim testi.** Her yeniden yazımın metin düzeyinde birim testleri
  (uygulanan, uygulanmayan yerler) ve `examples` alanında tam tasarımları
  vardır; `rewrite_examples_keep_their_meaning` ilişkilerin bu örneklerde
  bilinen başarısızlık olmadan tuttuğunu `volt check` ve Verilator ile
  gösterir.
- **Konum haritası.** Her düzenleme, yeni metnin hangi parçasının
  özgün metnin hangi parçasının kopyası olduğunu (`Anchor`) kaydeder;
  yeniden yazılmış metindeki bir konum özgün metne geri eşlenir.
- **Belgelenmiş istisnalar.** Ad ekleyen bir yeniden yazım (örneğin
  kullanılmayan bir sinyal için W1001) eklediği kodları gerekçesiyle
  `extra_codes`'ta yazar; başka istisna yoktur.

Bu kararla gelen yeniden yazımlar:

| Yeniden yazım | Dayanak | Bulacağı issue |
|---|---|---|
| `shorthand-to-long`, `long-to-shorthand`: `S { x }` ↔ `S { x: x }`, örnek bağlamasında da | grammar-full.ebnf, `FieldInit` ve `PortBind`: `x` tam olarak `x: x` demektir, her denetim iki biçime aynı uygulanır | #204 |
| `match-stmt-to-expr`, `match-expr-to-stmt`: `on`/`comb` içinde `t op match s { p => e, … }` ↔ `match s { p => { t op e } … }` | sv-mapping.md §19.1 (ADR-0083): atamanın bütün sağ tarafı olan match ifadesi deyim biçimine iner, hedef her kolda tekrarlanır; grammar-full.ebnf `MatchExpr`: ifade match'i deyimle aynı kuralları izler | #216 |

### 3. İlişkiler: kullanıcının çalıştırdığı komut üzerinden

Özgün tasarım ve yeniden yazılmış hâli `volt check --format json` ve
`volt build` ile, ikili olarak derlenir (#258: kullanıcının gördüğü
denetim kullanıcının çalıştırdığı komutla sınanır). Beklenen ilişkiler:

- **codes**: tanı kodlarının çoklu kümesi aynıdır (`extra_codes`
  dışında). Farklı kodlardan biri güvenlik özelliği koruyan bir kodsa
  (ADR-0113 envanteri) ilişkinin adı **safety**'dir.
- **location**: kodlar aynıysa, her hatanın birincil konumu yeniden
  yazıma göre aynıdır: yeniden yazılmış konum haritayla özgün metne
  eşlenir; bir kopyanın içindeyse tam konum, düzenlemenin kendi metnindeyse
  düzenlemenin kapsadığı özgün aralık karşılaştırılır.
- **hardware**: özgün tasarımda hata yoksa iki tasarımın üretilen SV'si
  Verilator'da aynı rastgele uyaranla benzetilir ve çıkışları her giriş
  değişiminden ve her saat kenarından sonra aynıdır. Testbench üretilen
  SV'nin kök modüllerini (başka modülün örneklemediği) örnekler, böylece
  çocuktaki değişiklik ebeveynden görünür. Her adımda veri girişleri
  rastgeledir; değerlerin dörtte biri uç değerdir: 0, 1 ya da bütün
  bitleri 1. Saatlerin rastgele bir alt kümesi yükselir, reset girişleri
  seyrek değişir (kutbu ne olursa olsun tasarım uzun süre reset içinde ve
  dışında kalır). Uyaranın tohumu §5'tedir. Verilator `volt test`'in
  bayraklarıyla (`VERILATOR_BUILD_FLAGS`, `-fno-dfg`) çağrılır; arama
  `tools::require` iledir, araçlı işte atlanamaz (ADR-0107).

Üretilen SV'nin metni hiçbir ilişkide karşılaştırılmaz. Özgün tasarımın
benzetilemediği durum (çıkışı yok, Verilator'un reddettiği extern gövdesi,
`inout` port) bulgu değildir: raporda nedeniyle "karşılaştırılmadı" diye
sayılır.

### 4. Bilinen başarısızlıklar

`crates/volt-driver/tests/metamorphic/known_failures.json`: açık bir
issue'nun yeniden ürettiği her başarısızlık bir kayıttır: denetim
(`real-bug` ya da yeniden yazımın adı), tasarım (derlem kimliği), ilişki
(`expect`, `test`, `codes`, `safety`, `location`, `hardware`), issue
numarası, gerekçe. Liste katıdır:

- listede olmayan her başarısızlık koşuyu kırmızı yapar;
- denetimi o tasarımda o ilişki için koşup başarısız olmayan kayıt da
  kırmızıdır ("artık düşmüyor: kaydı silin"); koşulmadıysa (araçsız iş,
  başka katman) değerlendirilmez;
- `hardware` ilişkisinde "artık düşmüyor" yalnız sabit tohumla
  değerlendirilir: PR katmanı ya da `VOLT_METAMORPHIC_SEED=pr` (§5).
  Rastgele uyaran hatayı gösteren girişe her tohumda denk gelmez; tarihli
  tohumla düşmeyen bir `hardware` kaydı raporda not olarak yazılır ve
  koşuyu kırmızı yapmaz, yoksa gecelik koşu boş yere kırmızı olur ve boş
  bir issue açar. Öteki ilişkiler (`expect`, `test`, `codes`, `safety`,
  `location`) uyarana bağlı değildir; onlarda kural her tohumla aynıdır;
- `scripts/ci/known_failures.py` (iş akışı "Known failures") her kaydın
  issue'sunu GitHub'dan okur: issue kapalıysa, yoksa ya da bir PR'sa, ve
  PR'da, PR issue'yu kapatıyorsa (`Closes #N`) ve kayıt duruyorsa düşer.
  GitHub istemcisi ve kurallar mutasyon hariç tutmalarının C kayıtlarıyla
  aynıdır (`scripts/ci/mutants_exclusions.py`, #283); belirteç ya da ağ
  yoksa geçmez, düşer. PR açıklaması düzenlendiğinde de koşar.

### 5. Katmanlar

- **PR**: her PR'da ve main'de, `cargo test` içinde: gerçek hata derlemi
  ve `tests/ui/` (pass, fail, multifile) üzerinde bütün yeniden yazımlar.
  Araçlı işte ("Verilator lint") donanım ilişkisi ve test blokları
  koşar; araçsız işte yalnız `volt check` ilişkileri. Rapor
  `VOLT_METAMORPHIC_SUMMARY` ve `VOLT_REAL_BUGS_SUMMARY` dosyalarına da
  yazılır ve günlüğe basılır (süre dahil). Hedef 15 dakikanın altı.
  Uyaranın tohumu sabittir: `pr`; her tasarımın uyaranı bu tohumdan ve
  tasarımın kimliğinden türer, aynı PR iki kez aynı uyaranla koşar.
- **Gecelik** (`metamorphic-nightly.yml`, her gece ve elle): bütün derlem
  (`tests/ui`, `examples/`, kitabın ```` ```volt ```` blokları, gerçek hata
  derlemi; `VOLT_METAMORPHIC_TIER=nightly`), Verilator zorunlu. Tohum
  koşunun UTC tarihidir (`2026-11-16`); her tasarımın uyaranı bu tohumdan
  ve tasarımın kimliğinden türer, böylece her gece başka uyaranlar
  denenir. Tohum günlüğe, rapora ve açılan issue'ya yazılır;
  `VOLT_METAMORPHIC_SEED=<tohum>` (her katmanda tohumun yerine geçer) aynı
  koşuyu yerelde yeniden üretir. Gecelik kırmızıysa iş akışı başlığı sabit
  bir issue açar ya da açık olanına raporu ve tohumu yorum olarak ekler
  (`scripts/ci/nightly_issue.py`; sahte bir `gh` ile self-test'i her PR'da
  koşar); yeşil gece bir şey yazmaz.
- Haftalık katman (yeniden yazımların ikili bileşimi) 2. aşamadadır (#254).

### 6. Kapsam

Rapor her yeniden yazımın derlemde kaç tasarımda ve kaç yerde
uygulandığını, kaç tasarımda donanımın karşılaştırıldığını ve
karşılaştırılamayanları nedenleriyle yazar. Kendi katmanının derleminde
hiç uygulanmayan yeniden yazım o katmanın koşusunu kırmızı yapar: o
katmanda hiçbir şeyi sınamıyordur. Kural her katmanda o katmanın
derlemine uygulanır: yalnız `examples/`'ta yeri olan bir yeniden yazım
gecelik katmanı geçer, PR katmanını düşürür; PR katmanının derlemi
(`tests/real_bugs/`, `tests/ui/`) her yeniden yazım için en az bir yer
taşımalıdır.

## Kanıt

- **#204 geri alınınca kırmızı.** `evidence/254-revert-204` dalı (#290,
  birleştirilmedi) #204'ün düzeltmesini (2a1b390b, `crates/*/src`) bu
  kararın üstünde geri aldı. Normal CI'ın 43835075278 koşusu, "Verilator
  lint" işi: `metamorphic` ve `real_bugs` düştü (`test result: FAILED. 22
  passed; 2 failed`). Listede olmayan başarısızlıklar: `shorthand-to-long`
  yedi tasarımda: `tests/real_bugs/204_…` ile `tests/ui/fail/258`–`261`'de
  `safety` (kısaltmada E3001, E3009, E4007, E5010 yok, uzun biçimde var),
  `256` ve `257`'de `codes` (E2001). `real-bug … 204_… [expect]`:
  "expected E3001 W3010, volt check gave W3010". Araç defteri `SKIP: 0`.
- **#216 bilinen başarısızlık.** #289'un 43854655621 koşusu, "Verilator
  lint", tohum `pr`: `match-stmt-to-expr tests/real_bugs/216_…
  [hardware]: first difference (outputs M.y): original A 1 09, rewritten
  A 1 07`; deyim biçimi s = 1 için 9, ifade biçimi 7 verir. Gerçek hata
  derleminde aynı dosyanın `expect` (W2014 yok) ve `test` (`assert_eq
  failed`) başarısızlıkları. Üçü de #216 ile listede; koşu yeşil.
- **Süre.** PR katmanı (447 tasarım, 22 yeniden üretim, Verilator ile)
  aynı koşuda 154,9 s'de bitti (`metamorphic_tests`: `29 passed; 0
  failed`); hedef 15 dakikanın altı. Donanımı karşılaştırılan yer
  sayıları: 2 + 28 + 6 + 2. Altı tasarım karşılaştırılamadı, nedenleri
  raporda: üçünde `inout` port var, üçü extern gövdesiz. İlk ölçüm
  (43834597865, test sayısı henüz 24'tü) 230,9 s'ydi.
- **Known failures.** 43854655569 koşusu: "issue #216 is open (3 known
  failure(s))", "pull request #289 closes: no issue".

## Sınırlar

- Yeniden yazımlar metin düzeyindedir ve muhafazakârdır: tanımadıkları
  biçime dokunmazlar. Örneğin kolu birden çok deyim ya da birden çok satır
  olan deyim match'i ve kolu boş olan (`_ => {}`) match yeniden yazılmaz.
- Modül düzeyindeki `y = match …` ile `comb { match … }` arasında yeniden
  yazım yoktur: şartnamede dayanağı yok. sv-mapping.md §19.1 iki konumu
  da kök sayar ama iki yazımın her denetimde aynı tasarım olduğunu
  söylemez; karar #291'dedir. #217'nin `let ↔ wire + comb` yeniden yazımı
  (#254) aynı soruya bağlıdır ve şartnamede dayanağı yoktur: o da #291'in
  kararını bekler.
- Rastgele uyaranla benzetim eşdeğerliği kanıtlamaz; nadir bir girişte
  ayrılan iki tasarımı kaçırabilir. Eşdeğerlik denetimi (Yosys eqy) 2.
  aşamadadır (#254).
- Kısıt (SDC/XDC) ve `@mmio` sürücü ilişkileri 2. aşamadadır.

## Reddedilenler

- **Yeniden yazımı derleyicinin AST'si üzerinden yapmak.** Daha az kod,
  ama ölçülen derleyicinin ayrıştırıcısına bağlanır (§2).
- **Üretilen SV metnini karşılaştırmak.** İki eşdeğer tasarımın SV'si
  farklı olabilir (deyim ve ifade match'i farklı `case` biçimlerine
  iner); metin eşitliği yanlış alarm, metin farkı da kanıt değildir
  (AGENTS.md, "Emitter tests test behavior").
- **Bilinen başarısızlıkları `#[ignore]` ya da tasarım dışlamasıyla
  susturmak.** Testi atlamak yasaktır; liste katıdır ve issue'ya bağlıdır.

## Ek: kalan yeniden yazımlar (#254 1. aşama PR 2, 2026-11-16)

Bu ek kararı değiştirmez; §2'nin kurallarıyla eklenen yeniden yazımları,
dayanaklarını, buldukları issue'ları ve eklenmeyen yeniden yazımın
gerekçesini kaydeder.

### 1. Yeni yeniden yazımlar

| Yeniden yazım | Dayanak | Bulduğu issue |
|---|---|---|
| `if-to-match`, `match-to-if`: `if c { a } else { b }` ↔ `match c { true => a, _ => b }` (ya da `false =>` ile, iki sırada); yalnız ifade biçimi, bütün bir işlenen olarak (`=`, `<=`, `(`, `,`, `:` sonrasında, satır sonu ya da `)`, `,`, `;`, `}` öncesinde); `else if` zinciri `_` kolu olur | type-inference.md §3.9 (ADR-0083): "Match ifadesi çok kollu `if` ifadesidir (§3.7) — genişlik davranışı ayrışmaz"; grammar-full.ebnf `MatchExpr`: literal desen ve `_`, ilk eşleşen kol; domain-inference.md K5: koşul ve sınanan sonucun join'ine aynı biçimde girer | #215: `tests/real_bugs/215_…`'te iki yönde `hardware` (match biçimi 8, `if` biçimi 9 bit) |
| `wire-to-let`, `let-to-wire`: modül düzeyinde `wire w : T @D` + hemen ardından tek sürücüsü `w = e` ↔ `let w : T @D = e` | ADR-0088'in tablosu ve domain-inference.md K1: `wire` ve modül `let`'indeki açıklama sinyalin alanını sabitler, sürücüye ya da değere karşı aynı denetlenir (E3001); K11 (ADR-0052): sinyalin güven seviyesi açıklanan alanınkidir; type-inference.md §3.9: tipli `let` ve atama hedefi aynı check bağlamıdır; sv-mapping.md §5.2, §16.3: ikisi de sürekli atamayla sürülen bir ağdır | #211: `tests/real_bugs/211_…`'te `let-to-wire` `safety` (let biçiminde E3009 yok, tel biçiminde var) |
| `clock-to-alias`, `alias-to-clock`: port, `wire`, `reg`, `let` bildirimindeki `: clock` ↔ dosyanın ilk öğesinden önce bildirilen `type C = clock;` ile `: C` | ADR-0070 §6: genel olmayan takma ad hedef tipine çözülür; `type C = clock` port, reg, wire ve let'te "tipin kendisi gibi çalışır" | #234: PR derleminde 20 tasarım (aşağıda §3) |

Sınırları (her biri birim testleriyle):

- `if-to-match`/`match-to-if` deyim biçimine (`if c { y = a } else { y =
  b }`) dokunmaz: §3.9 ifadeden söz eder. Bir işlecin işleneni olan
  `if`/`match`, muhafızlı ya da üç kollu match ve sayısal sınanan
  yeniden yazılmaz. Yeniden yazım tipleri görmez: koşul `bool` değilse iki
  biçim de hatalıdır ve kodları ayrışabilir; PR derleminde böyle tasarım
  yok.
- `wire-to-let` yalnız bildirimin bir sonraki deyimi tek sürücüsüyse
  uygular (`let` yalnız bildiriminden sonra görünür, name-resolution.md
  §5); parça sürücüsü, ikinci sürücü ve `comb`'dan sürülen tel yeniden
  yazılmaz (`let` ↔ `wire` + `comb` #291'i bekliyor). Blok `let`'i ve
  tipsiz ya da açıklamasız bildirim yeniden yazılmaz.
- Konum haritası §2 ve §3'teki gibidir: ad, açıklamalı tip ve değer
  kopyadır ve özgünlerine çapalanır (`let`'in adı telin bildirimindeki
  ada); yalnız anahtar sözcük ve noktalama yeniden yazımın kendi
  metnidir. Bugün telin E3001'i atamada, `let`'inki açıklamada
  gösteriliyor; `location` ilişkisi `tests/ui/fail/185` ve `186`'da bu
  yüzden düşer. K1 E3001'in hangi simgede gösterileceğini söylemez; karar
  #298'de, iki kayıt onunla listede. (PR'ın ilk hâli çapaları kaldırıp
  deyimi karşılaştırıyordu; bu §2'nin konum haritasına aykırı, `extra_codes`
  dışındaki bir istisna olurdu ve geri alındı.)
- `clock-to-alias` genel argümana, struct alanına ve `fn` parametresine
  dokunmaz (ADR-0070 dört bildirimi sayar); `domain` gövdesindeki
  `clock = posedge` bir tip değildir. Yer sayısı her tasarımda eklenen
  takma ad bildirimini de sayar. `alias-to-clock` yalnız her kullanımı
  bir bildirim tipi olan, `pub` olmayan `type X = clock` için uygular ve
  takma adı siler.

### 2. Eklenmeyen yeniden yazım: alt modülün satır içi alınması ↔ örneklenmesi (#210)

Şartnamede iki yazımı aynı tasarım sayan bir cümle yok; üç yerde
şartname onları bilerek ayırıyor:

- **Güven seviyesi.** K11: "sınıflandırılmamış çıkış, örneğin
  sınıflandırılmış girişlerinin en yükseğini (tutucu özet)"; K8b: "Güven
  seviyesi (K11) özeti kullanmaz". Satır içi alınmış çıkış yalnız
  okuduğunu taşır; örnek bütün sınıflandırılmış girişlerin en yükseğini.
  Yeniden yazım şartnamenin istediği bir farkı bulgu sayardı.
- **Saatsiz çocuğun tanı yeri.** K8b madde 2 (ADR-0114): iki alanı
  birleştiren saatsiz modülün E3001'i örnek sitesindedir, "çıkış
  okunmasa da", ve "modülün içinde tanı yoktur". Satır içinde birleşim
  sıradan bir ifadedir; `location` ilişkisi kuralla düşerdi.
- **Sınır.** ADR-0119 İ2.1: portlar sınırda beyandır. Satır içi alma
  sınırı kaldırır; portu açıklamalı bir `let`'e ya da `wire`'a çevirmek
  şartnamenin yapmadığı bir seçimdir.

Metin düzeyindeki engeller (bir kural olsa da): çocuğun her adına
ebeveynde hiçbir şeyle (örnek çıkışlarının üretilen SV adları dahil,
ADR-0090) çakışmayan önekli bir ad, modül içi sıralı çözümleme için
doğru yer, tek örneği satır içi alınan çocuğun silinmesi (yoksa üretilen
SV'nin yeni bir kökü olur ve `hardware` ilişkisi farklı port listelerini
karşılaştırır), adla bağlanan saat ve reset'in yerine koyulması. Karar
#296'da (A: listelenmiş istisnalarla aynı tasarım; B: yalnız aynı
donanım; C: aynı tasarım değil). #210'un kendisi bu karara bağlı değildir:
K11'in "sınıflandırılmamış sinyal kendisine yazılan en yüksek seviyeyi
alır" cümlesi eksik E3009'u her seçenekte hata yapar.

### 3. Bulunanlar

Başarısızlıkların hepsi açık issue'larla `known_failures.json`'da
(gecelik derlemin dört kaydıyla 31 yeni kayıt: #234 24, #215 3, #211 2,
#298 2). Sessiz yanlış olan yeni bir başarısızlık çıkmadı; `wire`/`let`
konum farkı açık bir hatanın yeridir, karar issue'su #298.

- **#234** (`clock-to-alias`, PR derleminde 20 tasarım): sıfırlama alanı
  denetimleri (`rdc/facts.rs`) takma adla yazılmış saati görmüyor:
  `tests/ui/fail/62`, `63`, `65`'te E3003, `tests/ui/pass/19` ve
  `tests/ui/fail/67`'de W3009, `tests/real_bugs/204`, `tests/ui/fail/68`,
  `70`, `194`, `197`, `tests/ui/pass/107`, `131`'de W3010,
  `tests/ui/fail/219`'da W3011 kayboluyor (`safety`); `tests/ui/fail/64`
  E3003 ve W3009 yerine E1003 veriyor. `tests/ui/pass/73`'te SDC geçişinin
  sahte E0017'si (`codes`). Issue'da adı geçmeyen iki yer daha, ikisi de
  açık hata (sahte alarm): `@mmio` açılımı kendi saat portunu ekliyor
  (`tests/ui/pass/57`, `58`, `72`: E1003 "'clk' is already defined"),
  pipeline denetimi saat portu bulamıyor (`tests/ui/pass/138`,
  `tests/real_bugs/137`: E5011). Gecelik derlemde aynı kökten
  `examples/fir_filter.volt` (E5011), `examples/soc/gpio.volt` (E1003),
  `examples/vga/vga_timing.volt` (E0017) ve kitabın
  `cookbook/reset-sync.md` blinker'ı (W3009). Donanım ilişkisi fark
  bulmadı. Yeni yerler #234'e yorum olarak yazıldı.
- **#215** (`if-to-match`, `match-to-if`): `tests/real_bugs/215_…`
  (yeni, §1'in açık issue kuralıyla): `let m = match c { true => a + b, _
  => a - b }` 8 bit üretiliyor; `test` bloğu a = 200, b = 100 için
  şartnameden 300 bekler, 44 alır (`test`); iki yeniden yazımın
  `hardware` ilişkisi ilk farkta düşer.
- **#211** (`let-to-wire`): `tests/real_bugs/211_…` (yeni): `let w : u8
  @Pub = key` (key gizli) E3009 almıyor (`expect`); tel biçimi alıyor
  (`safety`). Tel ve `let` üzerindeki açıklama ADR-0088'e göre aynı
  denetlenir. #211'in öteki yönü (saatsiz modülde `wire w : u8 @Pub`
  E3009 almıyor; saatli modülde `wire w : u8 @Sec` sahte E3009 alıyor)
  telin açıklamasının okunmamasıdır. Sahte alarmı `wire-to-let` bulur,
  ama derlemde böyle bir tasarım yok; saatsiz modülde iki biçim de sessiz
  kaldığı için o yönü hiçbir yeniden yazım göstermez, #211'in kendi
  fikstürleri gösterir.

### 4. Kanıt

Uygulama sayıları (tasarım / yer / donanımı karşılaştırılan /
karşılaştırılamayan), Verilator ile, yerelde Docker'da
(`volt-difftest-dev`, Verilator 5.052, `VOLT_REQUIRE_TOOLS=verilator`):

| Yeniden yazım | PR katmanı (449 tasarım, tohum `pr`) | Gecelik (490 tasarım, tohum `2026-11-16`) |
|---|---|---|
| `if-to-match` | 14 / 16 / 12 / 0 | 24 / 83 / 16 / 1 |
| `match-to-if` | 3 / 3 / 1 / 0 | 3 / 3 / 1 / 0 |
| `wire-to-let` | 3 / 3 / 1 / 0 | 3 / 3 / 1 / 0 |
| `let-to-wire` | 3 / 3 / 1 / 0 | 3 / 3 / 1 / 0 |
| `clock-to-alias` | 247 / 605 / 94 / 7 | 283 / 692 / 115 / 8 |
| `alias-to-clock` | 2 / 5 / 1 / 1 | 2 / 5 / 1 / 1 |

Donanımın karşılaştırılmadığı tasarımlar hatasız olup benzetilemeyenlerdir
(hatalı özgün tasarımın donanımı karşılaştırılmaz, §3): `inout` portlu
olanlar (`Listener`, `OpenDrainPad`, `Pad`, `Sampler`; gecede
`I2cMaster`) ve extern gövdesi olmayanlar (Verilator `MODMISSING`:
`Bank`, `ExtAsyncFifo`, `VendorFifo`). Donanım sütunu küçük kalan
yeniden yazımlarda öteki tasarımlar ya özgünde hatalıdır
(`wire-to-let`: `tests/ui/fail/185`, `187`; `let-to-wire`:
`tests/ui/fail/186`) ya da kodları ayrıştığı için donanıma gelmeden
düşer (`tests/real_bugs/211`).

- **Rapor artık Verilator çağrılarını sayar:** `hardware: N
  comparisons, M Verilator builds and runs, X s in them (summed over the
  workers)`; `metamorphic` testi karşılaştırma sayısı Verilator çağrı
  sayısını aşarsa düşer (karşılaştırma benzetimsiz sayılamaz).
- **Gecelik katman, tarihli tohumla (`2026-11-16`), yeni bir konteynerde
  üç kez.** Her koşuda 490 tasarım, 178 karşılaştırma, 313 Verilator
  derleme ve koşusu (178 yeniden yazılmış, 135 özgün), 30 başarısızlık,
  hepsi bilinen, koşu yeşil:

  | ccache | Süre | Verilator'da (işçiler toplamı) | ccache isabeti |
  |---|---|---|---|
  | kapalı (`CCACHE_DISABLE=1`) | 402,6 s | 1569,1 s | — |
  | açık, boş önbellek | 59,5 s | 217,2 s | 1086 / 1232 |
  | açık, dolu önbellek | 26,9 s | 87,5 s | 1232 / 1232 |

  `volt-difftest-dev` imajında ccache 4.9.1 var ve Verilator'ın
  `verilated.mk`'si onu kullanır (`OBJCACHE ?= ccache`); tasarımlar
  arasında Verilator'ın çalışma zamanı kütüphanesi aynıdır, boş önbellekte
  bile derlemelerin çoğu isabettir. İlk raporlanan 31,6 s aynı konteynerde
  hemen önce koşan PR katmanının doldurduğu önbellekle ölçülmüştü. CI'ın
  araçlı işinde ccache yok; oradaki süreler kapalı satıra yakındır.
  #215 ve #216'nın `hardware` kayıtları bu tohumla da düştü (başka
  uyaranla: `A 0 02c 12c`, `A 3 09`).
- **PR katmanı, yerelde (önbellek dolu):** `metamorphic` 51,8 s,
  `real_bugs` 24,7 s; `metamorphic_tests`: `45 passed; 0 failed`.
- **CI (#297, 213c2812):** 43934640812 koşusu, "Verilator lint":
  `metamorphic_tests` `45 passed; 0 failed` (518,3 s); `metamorphic`
  tablosu yereldekiyle aynı, `failures: 24; 518.3 s`, hepsi bilinen;
  `real_bugs` "24 reproductions, 10 with test blocks (10 run in
  Verilator); 4 failures, 4 known; 272.0 s"; `Tool ledger [integration]:
  … SKIP: 0`; "Tool coverage": 197 çiftin 197'si araç zorunluyken koştu.
  PR katmanı 154,9 s'den 518,3 s'ye çıktı (en çok `clock-to-alias`'ın 94
  donanım karşılaştırması); hedef 15 dakikanın altında kalıyor. "Known
  failures" 43934640011: "32 known failure(s), issues #211, #215, #216,
  #234: every issue is open and no pull request closes one".
- **CI (#297, f8018ca5, çapalar ve sayaç):** 43946682096 koşusu, "Verilator
  lint": `metamorphic_tests` `45 passed; 0 failed` (446,5 s); `hardware:
  148 comparisons, 260 Verilator builds and runs, 1762.9 s in them
  (summed over the workers)`, `failures: 26; 446.5 s` (hepsi bilinen);
  `real_bugs` "4 failures, 4 known; 227.0 s"; `SKIP: 0`; "Tool coverage"
  197/197. "Known failures" 43946681806: "34 known failure(s), issues
  #211, #215, #216, #234, #298: every issue is open and no pull request
  closes one". Bir Verilator derleme ve koşusu bir işçinin 6,8 s'si;
  benzetim duvar süresinin neredeyse hepsi. PR katmanında yeniden yazım
  başına karşılaştırma sınırı önerisi #254'te (bu ekte uygulama yok).
