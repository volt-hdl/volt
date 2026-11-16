# ADR-0117: Metamorfik Test ve Gerçek Hata Derlemi — Aynı Tasarımın İki Yazımı Aynı Tanıyı ve Aynı Donanımı Verir

> Statü: Önerildi
> İlgili: ADR-0108 (fark testi ifade düzeyinde dokuz biçimi karşılaştırır; bu test tasarım düzeyinde yazımları), ADR-0109 (bilinen başarısızlık kaydının issue denetimi C kayıtlarınınkiyle aynı mekanizmadır, #283), ADR-0113 (beklenen sonuç derleyiciden değil şartnameden gelir; güvenlik kodu envanteri aynıdır)
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
