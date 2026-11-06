# ADR-0108: Aritmetik Anlam için Fark ve Metamorfik Test — Derleyiciden Bağımsız Referans Model, Araçlı İşte Sabit Tohumlu Koşu

> Statü: Uygulandı
> Önceki karar: ADR-0036 — §2'nin "genişlik değişen cast'ler aynen kalır" satırı: genişlik değiştiren `as` sonucunun SV işareti artık hedefin işaretidir (`$signed`/`$unsigned` sınırı, #115)
> İlgili: ADR-0106 (§6'nın yalıtma ilkesi değeri genişliğe bağlı bütün alt ifadelere genişler, #117), ADR-0041 (check kipinde itme yalnız en üstteki işlemi hedef genişlikte yapar; işlenenler doğal değerleriyle genişler), ADR-0107 (fark testi araç sorgusunu `tools` modülünden yapar, araçlı işte SKIP olmadan koşar), ADR-0109 (mutasyon testinde fark testi her mutant için koşar; parçalı, elle tetiklenen yapı büyük koşudan)
> Tarih: 2026-11-04
> Etkilenen: `crates/volt-difftest/` (YENİ: model, üreteç, yazıcı, koşucu,
> regresyon kümesi), `crates/volt-driver/tests/difftest_tests.rs` (YENİ),
> `crates/volt-sv-emit/src/expr.rs` (`emit_cast`, `width_dependent`,
> `tree_width_dependent`, `isolate_if_dependent`, `emit_self_sized`,
> `pushes_into`, `fmt_int`, `is_selectable_base`, `is_scalar_base`),
> `crates/volt-sv-emit/tests/{cast_literal_sign,width_isolation}_tests.rs`
> (YENİ), `.github/workflows/ci.yml`, `.github/workflows/difftest.yml`
> (YENİ), `scripts/ci/difftest_summary.py` (YENİ), `CHANGELOG.md`

## Sorun

Aynı sınıftan altı sessiz yanlış sonuç rastlantıyla bulundu: #106
(`a == b + 1` satır içinde 3 bitte, `let`'le 4 bitte), #111'deki
`c + d < 300` → 44, `(~a) + 0` → 4095, kaydırma genişliği, #112. Kök hep
aynıdır: Volt'ta bir alt ifadenin genişliği işlenenlerinden gelir, SV'de
içinde bulunduğu ifadeden. Emitter SV bağlamını alt ifadelere iter
(sv-mapping §16.1); bu yalnız değeri genişlikten bağımsız işlemlerde
doğrudur. Her bulgu tek tek kapatıldı; sınıfın kendisi aranmadı.

## Karar

### 1. Referans model: spesifikasyondan, derleyiciden bağımsız

`crates/volt-difftest` bağımlılığı olmayan bir crate'tir: hiçbir `volt-*`
crate'ine bağlanmaz, derleyiciyi yalnız `volt` ikilisi olarak, Verilator'u
bir süreç olarak çağırır. Model (`model.rs`) `type-inference.md` §3–§5,
`operator-precedence.md`, `sv-mapping.md` §6, ADR-0025, ADR-0035,
ADR-0036, ADR-0041 ve ADR-0106'dan doğrudan yazılmıştır:

- Her alt ifadenin bağlamdan bağımsız bir değeri vardır: doğal
  genişliğinde (esnek aralığın üst ucu) bit örüntüsü.
- İkili işlem işlenenlerini sonucun doğal genişliğine dönüştürür (geniş
  olan kesilir, dar olan kendi işaretiyle genişler) ve o genişlikte
  hesaplar. Aralıklar kesişimle buluşur (ADR-0025); `+ -` 1 bit, `*` iki
  kat; bit işlemi kesişimi korur; kaydırma sol işlenenin tipindedir.
- Karşılaştırma iki yanın eşit doğal genişliğinde, farklıysa E2031;
  literal geniş yanın doğal genişliğinde (ADR-0106).
- Check kipinde (çıkış ataması, tipli `let`) ADR-0041'in itmesi en
  üstteki aritmetik işlemi hedef genişlikte yapar; işlenenler
  sentezlenmiş değerleriyle genişler (iç içe aritmetiğe hedef itilmez).
- Bileşik değerden seçim doğal genişliğinde (ADR-0106 §4); değişken
  indeks ya da başlangıç sınır dışındaysa değer x'tir ve karşılaştırılmaz
  (ADR-0035 §1).

Spesifikasyonun sessiz ya da belirsiz kaldığı her yerde model bir okuma
seçer ve kullandığı her ifadeye bir varsayım etiketi koyar (aşağıdaki
tablo); fark raporu farkı etiketleriyle basar. Spesifikasyonun hiç
tanımlamadığı biçimler (`Unspecified`) üretilmez ya da karşılaştırılmaz,
sayılır.

### 2. Üreteç

Tip yönlü: `int(s, w, d)` aralığı `w`'yi içeren bir ifade üretir, aynı `w`
ile üretilen iki işlenenin aralıkları kesişir ama doğal genişlikleri
farklı olabilir (sınıfın yaşadığı yer). İşlemler: `+ - * & | ^ ~ << >>`,
altı karşılaştırma, `!`, tekli `-`, bit seçimi (sabit ve değişken
indeks), aralık seçimi, `+:`/`-:` (sabit ve değişken başlangıç), bileşik
taban, `as` (genişletme, daraltma, işaret değişimi, `bits<N>` ↔ sayı,
`bool` ↔ `u1`), ondalık/onaltılık/ikili literal. Tam sayılar 1–64 bit
(üstü E0003, ADR-0031), `bits<N>` 1–70 bit; iç içelik 1–5. İfadelerin
%5'i bilerek hatalıdır (işaret karışımı, genişlik uyuşmazlığı, `bits`
aritmetiği, E2031, sınır dışı indeks, ters aralık, geçersiz dönüşüm,
işaretsiz `-`, sığmayan literal), %0,5'i `concat()` (E0003, ADR-0072).

### 3. Biçimler ve metamorfik ilişkiler

Her ifade tek modülde dokuz biçimde sınanır:

| Biçim | Volt | Beklenen |
|---|---|---|
| a | `o = e` (satır içi, doğal tipte çıkış) | model değeri |
| b | `let t = e; o = t` | a ile aynı |
| c | bir alt ifade `let s = …`'e çıkarılmış | a ile aynı |
| d | her alt ifade (atomlar dahil) parantezli | a ile aynı |
| e | değişmeli işlemlerin işlenenleri yer değiştirmiş, sıralama tersine | a ile aynı |
| f | `o = (e'nin e biçimi) == t` | hep doğru |
| k | `o = e < K` (rastgele işlem ve literal) | model değeri |
| w | `o : u(N+k) = e` (daha geniş hedef) | ADR-0041 itmesiyle model değeri |
| g | `ensures: o_a == e` | simülasyonda hiç düşmez |

Hatalı ifade `b`, `d`, `e` biçimlerinde tipsiz `let`'lere girer: her biçim
reddedilmeli ve aynı kodla. Modelin geçerli dediği bir ifadeyi derleyici
reddederse, model hatalı dediğini derleyici kabul ederse, biçimler
farklı tanı verirse ya da üretilen SV Verilator'da derlenmezse ayrı bulgu
sınıfıdır; ikisi de reddedip kodlar farklıysa yumuşak bulgu (raporlanır,
koşuyu düşürmez). Verilator `-Wall` uyarıları yumuşak bulgudur; WIDTH uyarıları sert
(varsayılan olarak ölümcül, `volt test` durur).

### 4. Koşucu

Toplu koşu başına 250 ifade tek modüle girer; her satırın sahibi
(ifade, biçim) kaydedilir, böylece tek `volt check --format=short`
çağrısının tanıları ve üretilen SV'nin satırları ifadeye geri eşlenir.
Hatalı ifade çıkarılıp yeniden denetlenir (önceki aşamanın hataları
sonrakini gizlediği için tanısız ifade temiz tura kadar bekletilir).
Sonra `volt build --emit=sva --sva inline`, Verilator `--binary --timing
--assert -Wall -Wno-fatal` ile tek derleme: testbench 16 vektörü
(sıfır, hepsi bir, yalnız üst bit, üst bit dışında hepsi, bir, sonra
rastgele) en çok 1024 bitlik parçalar hâlinde `$readmemh` ile okur, her
vektörde girişleri geri basar, çıkışları aynı sınırla parça parça basar ve
bir saat kenarı üretir (sözleşmeler). Geri basılan giriş beklenenden
farklıysa ya da testbench'te bir port bağlanmadıysa (Verilator `PIN*`)
sınamanın kendisi bozuktur: sert "harness" bulgusu. Modelin herhangi bir
vektörde x dediği ifadeye (sınır dışı değişken seçim) sözleşme biçimi
konmaz: iki x'i karşılaştırırdı. Fark bulunan ifade küçültülür: bir düğümü
çocuğuyla ya da aynı tipte bir portla değiştiren adaylar tek toplu koşuda
denenir, farkı koruyan en küçük aday seçilir.

Her bulunan kök nedenin en küçük örneği `regress.rs`'te sabit regresyon
kümesidir (issue numarasıyla); her koşunun ilk toplu koşusudur.

Regresyon kümesinin beklenen değeri iki kaynaktan gelir. Her vektörde
model; ayrıca her issue'nun en az bir örneğinde spesifikasyondan elle
hesaplanmış giriş/çıkış çiftleri (`pin`), gerekçesi (type-inference
bölümleri, ADR-0025, ADR-0035) örneğin yanında yorum olarak. Koşucu elle
değerleri son vektörlere yazar ve a–e biçimlerinin Verilator çıktısını
onlarla modelden bağımsız karşılaştırır: model ile derleyici aynı yanlışı
yapsa da küme onu yakalar. Elle değer modelinkinden farklıysa araçsız
birim testi düşer. Değeri bir varsayım etiketine bağlı örnek elle
sabitlenmez.

### 5. CI

- "Verilator lint" (araçlı iş, ADR-0107) bütün testleri koşturur; fark
  testi orada `VOLT_DIFFTEST_SEED=20261104`, `VOLT_DIFFTEST_COUNT=2000`
  ile koşar. Araç sorgusu `tools::require(Tool::Verilator)`'dır, deftere
  yazılır, araçlı işte SKIP olamaz.
- Test raporunu `VOLT_DIFFTEST_SUMMARY` dosyasına da yazar; "Differential
  test summary" adımı (`scripts/ci/difftest_summary.py`) onu günlüğe basar,
  tohum ve sayı satırını not olarak koyar, dosya yoksa (test koşmadıysa)
  ya da fark varsa düşer.
- Büyük koşu `.github/workflows/difftest.yml`'dir (`workflow_dispatch`:
  ilk tohum ve parça başına sayı; dört parça, ardışık tohumlar). Her parça
  deftere yazar ve sıfır SKIP ister.

### 6. Bulunan kök nedenler ve SV eşlemesindeki değişiklikler

| Issue | Kök neden | Düzeltme |
|---|---|---|
| #115 | Genişlik değiştiren `as`'ın SV ifadesi hedefin işaretini taşımıyor: birleştirme ve dilim işaretsiz, boyut dönüşümü kaynağın işaretinde | Sonuç hedefin işaretine `$signed(...)`/`$unsigned(...)` ile getirilir: `a as i16` → `$signed({{8{a[7]}}, a})` (ADR-0036 §2'nin genişlik değişen biçimleri bu sınırla sarılır) |
| #116 | İşaretli bağlamda onaltılık/ikili/sekizli literal `W'h..` (işaretsiz); `-W'sdM` en küçük negatif değerde (M = 2^(W-1)) geniş bağlamda pozitif | `W'sh..`/`W'sb..`/`W'so..`; en küçük negatif değer örüntüsüyle (`1'sh1`, `8'sh80`); `-K` literali tekli işlem değil literal olarak biçimlenir |
| #117 | Değeri genişliğe bağlı alt ifade daha geniş SV bağlamına yalıtılmadan giriyor (#106 sınıfı): işaretsiz çıkarma, doğal genişlikleri farklı işlenenlerin işlemi, bileşik `as` işleneni, aynı genişlikte dönüşüm altında `~`, `$signed(...)` içinde `-x`, esnek sol işlenenin kaydırması, öz-belirlenimli konumda (kaydırma miktarı, indeks, başlangıç) bileşik ifade, bağlamdan geniş kaydırma işleneni | ADR-0106 §6'nın yöntemi genel kural olur: değeri genişliğe bağlı işlenen daha geniş bağlama doğal genişliğinde hesaplanıp `W'($unsigned(...))`/`W'($signed(...))` ile girer; en üstteki aritmetik işlem ADR-0041 itmesiyle hedefte kalır; öz-belirlenimli konumdaki bileşik ifadenin işlenenleri doğal genişliğe genişler; kaydırmanın sol işleneni doğal genişliğindedir; bağlamdan geniş kendi genişlikli işlenen bağlama kesilir |
| #118 | `-(-x)` → `--(...)` (SV azaltma işleci) | `-(-(...))` |
| #119 | Seçimin seçimi: tabanı seçim olan seçim "seçilebilir ad" sayılıyordu; `70'(...)[69:0]` ve `q[43:0][42:15]` geçersiz SV | Seçilebilir taban yalnız ad, alan ve unpacked dizi elemanı; geri kalanı boyut dönüşümüne iner (`28'((q[43:0]) >> 15)`) |
| #120 | 1 bitlik değerden seçim: `u1`/`i1`/`bits<1>` SV'de skaler `logic`, `x[0]` ve `x[0:0]` geçersiz | 1 bitlik tabandan seçim tabanın kendisi; işaretli tabanda `$unsigned(...)` (seçim işaretsizdir) |
| #123 | Üretilen SV'de Verilator WIDTH uyarısı (varsayılan olarak ölümcül, `volt test` durur): değişken indeksin ya da parça seçimi başlangıcının genişliği adres genişliğinden farklı (`x[i]`, x : u15, i : u3); bileşik değerin genişleten dönüşümü `W'(a + b)` (içteki örtük genişleme, ADR-0041'in biçimi); daha geniş hedefteki aynı genişlikte dönüşüm; bileşik tabandan `-:` seçiminde boyutsuz başlangıç farkı | Genişliği adres genişliğine (`clog2`) uymayan vektör indeksi ve başlangıcı kaydırmaya iner (`x[i]` → `1'((x) >> i)`, `q[k +: 35]` → `35'((q) >> k)`; kaydırma miktarına uyarı yok, sınır dışı indeks zaten x'tir, ADR-0035; açık genişletilmiş başlangıç `q[6'(k) +: 35]` Verilator 5.053'te DFG iç hatasına yol açıyordu); unpacked dizi indeksi adres genişliğinde (`regs[5'(i)]`); değeri genişlikten bağımsız bileşik işlenen hedef genişlikte, atomları açık ve boyut dönüşümü sınırıyla: `(a + b) as i32` → `32'((32'(a) + 32'(b)))`; bağlamdan geniş pozitif literal kesilmiş örüntüsüyle (`1'd3` yerine `1'h1`, SV zaten keser); aynı genişlikte dönüşüm geniş bağlamda `W'($unsigned(...))`; fark sabitin genişliğinde `(5'(s) - 5'd27)`. Fark testinde WIDTH uyarısı sert bulgudur |

`sv-mapping.md` §6 tablosundaki `a as i16` → `{{8{a[7]}}, a}` satırı ve
§10'daki literal biçimleri bu ADR'yle değişir (`$signed(...)` sınırı,
`'sh`); §16.1'in "bileşik alt ifadeye bağlam içeri aktarılır" kuralı
değeri genişlikten bağımsız alt ifadeler için geçerli kalır.
Spesifikasyon metni değişmez; çelişki burada kayıtlıdır.

Düzeltmelerin varsayım etiketleriyle ilişkisi (etiketler regresyon
örneklerinde modelin kullandıklarıdır):

| Issue | Dayandığı okuma | Önceki davranış | Yeni davranış |
|---|---|---|---|
| #115 | Yok: `as T`'nin sonucu `T` tipindedir (type-inference §3.6), karşılaştırmanın ve kaydırmanın işareti `T`'den gelir. Bir örnekte A7 var (`s as i1`'in geçerli olması); derleyici onu önce de sonra da W2010 ile kabul eder, farkı belirlemez | SV ifadesi hedefin işaretini taşımıyor: `a as i11 > 880` (a : i10 = -1) doğru | `$signed(...)`/`$unsigned(...)` sınırı: yanlış |
| #116 | Yok: soneksiz literal bağlamın tipini alır (§4), yazımı değeri değiştirmez. Örnekte A6 var (i2 → u35), değer pozitif olduğundan sonucu belirlemez | `s > 0x0` (s : i50 = -1) işaretsiz, doğru; `-1'sd1` geniş bağlamda +1 | `50'sh0`, yanlış; en küçük negatif değer örüntüsüyle |
| #117 | Kısmen. İşaretsiz çıkarmalı örnekler A2 ve A3'e (biri A1'e), esnek toplamın kaydırması A4'e dayanır; bit işlemiyle düşen taşma, dönüşüm altında `~` ve kaydırma içinde `-x` etiketsizdir (ADR-0025 kesişimi, §3.4) | Satır içinde SV bağlamı alt ifadeye itiliyordu (sv-mapping §16.1: çıkarma hedef genişlikte sarar), `let` biçiminde doğal genişlikteki değer kullanılıyordu: `(a - b) + c9` satır içinde 1023 + c9, `let`'le 511 + c9 | İki biçim de 511 + c9. A2'nin okuması seçim değil: `let` teli doğal genişlikte bildirilir ve ADR-0106'ya göre `let`'e çıkarmak değeri değiştirmez, satır içi biçim ona uyar. A1 ve A3'ün kendisi (çıkarmanın negatif sonucu tanımlı mı) #122'de açık |
| #118 | Yok | `--(...)`, geçersiz SV | `-(-(...))` |
| #119 | Yok | `29'(q)[27]`, geçersiz SV | `1'((29'(q)) >> 27)` |
| #120 | Yok | skaler `logic`'ten seçim, geçersiz SV; işaretli 1 bitlik taban işaretli kalıyordu | tabanın kendisi; işaretli tabanda `$unsigned(...)` |
| #123 | Yok: değer değişmez. Değişken parça seçiminin sınır dışı bitleri dilde x'tir (ADR-0035); kaydırma biçimi onları 0 okur, model karşılaştırmaz | WIDTH uyarısı, `volt test` durur | kaydırma biçimi ve açık genişlikler, uyarı yok |

## Spesifikasyon belirsizlikleri

Modelin varsayım etiketleri (`model::ASSUMPTIONS`) ve modelin
karşılaştırmadığı biçimler. "Derleyici" sütunu düzeltmeden sonraki
gözlemdir. Hiçbirinde derleyici modelden farklı değer vermez; yazılması
gereken okumalar ve seçenekleri bakıcının kararına #122'de açıktır.

| Kimlik | Belirsizlik | Modelin okuması | Derleyici |
|---|---|---|---|
| A1 | ADR-0025: "işlem genişliği 16'da buluşur" — geniş (esnek) işlenen işlemden önce kesişimin üst ucuna mı kesilir, sonucun doğal genişliğine mi dönüşür? | Sonucun doğal genişliğine dönüşür (`[8,9] + [8,8]` 9 bitte, toplamın 9. biti sarar) | Aynı |
| A2 | type-inference.md §5 / ADR-0041: itmede işlenenler "önce genişletilir"; sv-mapping §16.1 "bağlam içeri aktarılır" der. İç içe işaretsiz çıkarmada ikisi farklı değer verir | Sentezlenmiş değer genişler (let'e çıkarmak değeri değiştirmez, #106) | Aynı (#117 sonrası) |
| A3 | `u8 - u8 → u9`: negatif sonuç tanımsız | Doğal genişlikte (9 bit) modüler sarar | Aynı |
| A4 | Kaydırma "sol operandın tipi": esnek sol işlenen hangi genişlikte kayar? | Doğal genişliğinde (`(a + b) >> n` taşmayı okur) | Aynı (#117 sonrası) |
| A5 | `~x`, x esnekken | Doğal genişliğinde | Aynı |
| A6 | İşaret değiştiren genişletme (`i8 as u16`, `u8 as i16`) | Kaynağın işaretiyle genişler | Aynı |
| A7 | §3.6 yalnız işaretsiz daraltmayı sayar; `iN as iM` (M < N) kurala göre E2009 | Geçerli, keser | Kabul eder (W2010) |
| A8 | §3.6 `iN as bits<N>`'i saymaz | Geçerli | Kabul eder |
| A9 | Esnek işlenenin yanındaki literal hangi genişliğe sığmalı? | Doğal genişliğe | Aynı |
| A10 | Esnek işaretli `-x` | `i(doğal + 1)` (ADR-0025 eki "somut i(N+1)") | Aynı |
| A11 | `~b`, b : bool (§3.4 `BitNot => ot`) | `bool` | Aynı |
| A12 | W2013 "sonuç hep 0" der; işaretli `>>` genişlik kadar kaydırınca | İşaretle dolar (-1 ya da 0) | Aynı |
| — | `MAX_WIDTH` §3.3'te kullanılır, tanımlanmaz | Doğal genişlik 64'ü aşan tam sayı üretilmez | — |
| — | Kaydırma miktarı "herhangi bir sayısal tip": işaretli ve `bits<N>` miktarın anlamı | Üretilmez | — |
| — | İşaretli indeks ve parça seçimi başlangıcı | Üretilmez | — |
| — | `bool` ve `bits<N>` arasında `<`, `<=`, `>`, `>=` | Üretilmez | — |
| — | İşaretleri farklı karşılaştırmanın ve bit işleminin kodu (aritmetikte E2002) | Kod beklenmez | E2003 |
| — | Değişken indeks ya da başlangıç sınır dışında: SV'de x (ADR-0035) | Karşılaştırılmaz | Bileşik tabanda 0 |
| — | Soneksiz literalin dönüşümü (`5 as u8`): §3.6'da kural yok | Üretilmez | — |

## Kanıt

PR #121:

1. **Önce düşen** (yalnız test altyapısı, d023e6b2; run 41763373693):
   araçlı işte `difftest: seed=20261104 target=2000 generated=2250
   accepted=2061 ... diffs=1613` ve `difftest: regressions=21 failed=21`;
   `width_isolation_tests` 15/15, `cast_literal_sign_tests` 8'de 7 düştü
   (geçen, işaretsiz bağlamdaki literalin değişmediğini sınayan koruma
   testi).
2. **Düzeltme** (5fee6881; run 41763452442): regresyonlar 21/21 geçti; aynı
   tohumda 5 fark kaldı, hepsi #120 düzeltmesinin işaretli 1 bitlik
   tabanı (`s[0:0]`, s : i1, SV'de işaretli kalıyordu) → 3ae72064.
3. **Büyük koşu, ilk** (run 41763520527, 4 × 12.500): 10 sözleşme düşmesi
   ve 7 değer farkı; ikisi de koşucudan: sözleşme sınır dışı seçimde iki
   x'i karşılaştırıyordu (85f2e3eb), 4096 biti aşan `$display` parçası
   girişlerle açıklanamayan değer basıyordu (1fddec7a; ara koşu
   41765714347'de sözleşmeler gitmiş, 7 değer farkı kalmıştı).
4. **50.293 ifade, 0 fark** (1fddec7a, büyük koşu run 41767138333):
   değer, sözleşme ve SV geçerliliği temiz. Tam uyarı sayımı (run
   41768507802) ayrıca `WIDTHEXPAND=315 WIDTHTRUNC=15` gösterdi: Verilator
   WIDTH uyarısı varsayılan olarak ölümcüldür, `volt test` durur → #123.
5. **#123** (ee92755b, d5246c2e, 91ea38f0, 53ca196e, 24e70f09): ilk biçim
   (açık genişletilmiş indeks `p[7'(i) +: 69]`) Verilator 5.053'ün DFG
   iyileştiricisinde iç hataya yol açtı (run 41776172748: `Internal
   Error: ... V3DfgPeephole.cpp:1508: Out of range select should have been
   fixed up earlier`); genişliği uymayan vektör indeksi kaydırmaya indi.
   Ardından iç içe genişleten dönüşüm ve bağlamdan geniş literal.
6. **Son durum** (24e70f09):
   - CI run 41779299711 yeşil; araçlı işte `difftest: seed=20261104
     target=2000 generated=2250 accepted=2118 ... diffs=0`,
     `regressions=24 failed=0`, `SKIP: 0`.
   - Büyük koşu run 41779299601, tohumlar 20261105–20261108: kabul
     edilen 12.724 + 12.514 + 12.537 + 12.518 = **50.293 ifade**
     (üretilen 53.250; modelin öngördüğü gibi reddedilen 2.697; E0003
     ile reddedilen `concat()` 260; tanımsız 0); 349.400 biçim,
     4.926.905 değer karşılaştırıldı (x olduğu için atlanan 663.495),
     654.704 sözleşme örneklemesi; her parçada `diffs=0`, WIDTH uyarısı
     yok, regresyonlar 24/24, `SKIP: 0`. Yumuşak uyarılar: CMPCONST,
     UNSIGNED, UNUSEDSIGNAL (ifadenin kendisi) ve sabite katlanan sınır
     dışı başlangıçta SELRANGE (#124'e ayrıldı).
7. **Yerel** (Docker, Verilator 5.052, tohum 777): düzeltmeden önce 3000
   ifadede 2442 fark; düzeltmeden sonra 1500 hedefte 1646 kabul, 0 fark.
8. **Külliyat** (455 dosya, main ile `volt build --emit=sva`; #123'ten
   önceki hâliyle ölçüldü, #123'ün biçimleri ölçüme girmedi): 15
   dosyanın SV/SVA metni değişti, değerleri aynı (`$signed({...})`
   sınırı, `8'sh80`, kaydırma miktarı doğal genişlikte, `bits<13>`
   dönüşümünde `$unsigned`). Son kodda araçlı işin bütün testleri
   (ui/pass ve örneklerin Verilator `-Wall` taraması dahil,
   `output_net_tests`) geçer.

## Reddedilenler

- **Modeli Python'da yazmak:** koşucu ADR-0107 gereği araç sorgusunu
  `tests/tools` modülünden yapan bir Rust testidir; model aynı dilde ve
  bağımlılıksız bir crate'te olunca bağımsızlık `Cargo.toml`'dan görünür.
- **`volt test` ile sınamak:** test dili 64 bitten geniş değerleri ve
  ayrı kurallarını (ADR-0058/0059/0060) araya sokar; testbench doğrudan
  üretilen SV'yi sınar.
- **İfade başına bir derleme:** 50.000 ifade × dokuz biçim; toplu koşu
  tek Verilator derlemesinde 250 ifadeyi sınar.
- **Her alt ifadeyi her zaman yalıtmak:** değeri genişlikten bağımsız
  ifadelerde (`9'(a) + 9'(b)`) SV metni gereksiz uzar ve mevcut çıktıların
  hepsi değişirdi; yalıtma yalnız değeri genişliğe bağlı alt ifadede.

## Sınırlar

Kapsamın genişletilmesi #125'te (v0.1.1).

- `if`, `match`, `/`, `%`, `prev()`, fonksiyon çağrısı, struct ve enum
  ifadeleri üretilmez.
- Sözleşme biçimi yalnız `ensures`'tır ve simülasyonda (Verilator) sınanır;
  formal akış (Yosys/sby) fark testinin dışında.
- 64 bitten geniş tam sayı (E0003) ve `concat()` (E0003) yalnız reddin
  sayılması için üretilir.
- Verilator `-Wall` uyarıları (WIDTH dışında) yumuşak bulgudur: üretilen
  ifadenin kendisi sabit karşılaştırma (CMPCONST, UNSIGNED) ya da kısmen
  okunan `let` (UNUSEDSIGNAL) içerebilir.

## Ek: r biçimi — ad hijyeni (#128, 2026-11-05)

#128'i fark testi değil mutasyon testi buldu (PR #127): ifadeler tek
modülde, fonksiyonsuz, sabit adsız üretiliyordu; bir adın yanlış tanıma
bağlanabileceği hiçbir girdi yoktu. Onuncu biçim bu sınıfı sınar:

| Biçim | Volt | Beklenen |
|---|---|---|
| r | ifade `fn h<id>r`'nin gövdesinde; seçim konumundaki literaller (`x[3]`'ün indeksi, `x[2 +: 4]`'ün başlangıcı) kök `const t<id>r`, `K<id>r<n>`; (yalnız `VOLT_DIFFTEST_FN_LET=1` ile) c biçiminin alt ifadesi gövdede `let s<id>r`'ye bağlı ve aynı adlı kök `const s<id>r`'yi gölgeler; çağrı `comb` içinde, `for t<id>r in V..V+1` döngüsünde (V, `t<id>r`'nin değerinden bir fazla), bitleri seçilmeyen bir argüman çağıranın `let s<id>r`'sinden geçer | a (gövde `let`'i varsa c) ile aynı |

Ad çakışmaları anlamı değiştirmemeli (name-resolution.md §2, §6;
ADR-0081 Karar 12.1: fn kapsamının ebeveyni kök kapsamdır): gövdedeki
`t<id>r` const'tur, döngü değişkeni değil; gövdedeki `s<id>r` gövdenin
`let`'idir, ne kök const ne çağıranın yereli. Gölgelemeler W1002
uyarısıdır, koşucu uyarıları saymaz. `let` satırı `;` ile biter (son
ifade `(` ile başlarsa `let`'in değeri çağrı gibi sürerdi; `LetDecl = …
[";"]`). İkame kipinin desteklemediği iki durum biçime girmez: bitleri
seçilen gövde `let`'i (o zaman gövde `let`'siz) ve bitleri seçilen
parametreye yerel geçirmek (o argüman port olarak geçer). Çakışacak ad
yoksa (seçim literali yok, `let` yok, yerelden geçecek argüman yok) biçim
konmaz.

**Gövde `let`'i isteğe bağlı (#132).** İlk CI koşusunda (tohum 20261104,
2000 ifade) gövde `let`'li r biçimi 9 değer farkı ve 44 WIDTH uyarısı
buldu; hepsi gövdenin tipsiz `let`'ini içeriyor ve çakışan ad olmadan da
oluyor: ikame kipindeki tipsiz fn `let`'i esnek genişliğinin kesişimini
kaybediyor, aynı fn `comb` içinde ve modül düzeyinde farklı değer veriyor.
Kök neden ad hijyeni değil; #132'de. Bu yüzden gövde `let`'i varsayılan
olarak kapalı, `VOLT_DIFFTEST_FN_LET=1` ile açılır (açık koşu #132'yi
bulur); #132 düzelince varsayılan açılır.

Regresyon kümesine #128'in örneği eklendi (`p0[1]`, elle hesaplanmış iki
değer): r biçimi düzeltmesiz derleyicide bit 2'yi seçer.

Düzeltme olmadan: yerelde 8 toplu koşunun 1816 r çıkışından 297'sinin SV
satırı farklı (döngü değişkeninin değeri), diğer biçimlerin bütün satırları
aynı. Düzeltmesi geri alınmış dalda (`test/hygiene-revert-demo`)
`difftest.yml` (41978380162, dört parça, tohum 20261104–20261107, parça
başına 2000): dört parça da düştü (diffs 299, 281, 276, 293), küçültülmüş
bulguların hepsi r biçiminde, örneğin "regression #128 … — r u8[K]".

Koşunun günlüğünden alıntı (GitHub günlükleri 90 gün sonra siler; dal
silindi). Dalın tek commit'i (a7b646dd) bu PR'ın ağacında yalnız
`crates/volt-sv-emit/src`'yi `main`'deki haline döndürüyordu (9 dosya,
`scope.rs` silinir). Her parçanın özet satırları ve ilk iki bulgusu:

```
[shard 0] difftest: seed=20261104 target=2000 generated=2250 accepted=2118 rejected-as-predicted=126 unsupported=6 unspecified=0 values-compared=234541 diffs=299
[shard 0] difftest: regressions=25 failed=3 hand-computed=31 compared=31
[shard 0]   [value differs from the model] regression #128: constant index in an inlined function called in a loop whose variable has the constant's name — r u8[K]
[shard 0]   [value differs from the model] r u13[K]
[shard 1] difftest: seed=20261105 target=2000 generated=2250 accepted=2127 rejected-as-predicted=116 unsupported=7 unspecified=0 values-compared=236594 diffs=281
[shard 1] difftest: regressions=25 failed=3 hand-computed=31 compared=31
[shard 1]   [value differs from the model] regression #128: constant index in an inlined function called in a loop whose variable has the constant's name — r u8[K]
[shard 1]   [value differs from the model] r bits<38>[K +: w]
[shard 2] difftest: seed=20261106 target=2000 generated=2250 accepted=2111 rejected-as-predicted=125 unsupported=14 unspecified=0 values-compared=235738 diffs=276
[shard 2] difftest: regressions=25 failed=2 hand-computed=31 compared=31
[shard 2]   [value differs from the model] regression #128: constant index in an inlined function called in a loop whose variable has the constant's name — r u8[K]
[shard 2]   [value differs from the model] r u5[K]
[shard 3] difftest: seed=20261107 target=2000 generated=2250 accepted=2108 rejected-as-predicted=132 unsupported=10 unspecified=0 values-compared=233529 diffs=293
[shard 3] difftest: regressions=25 failed=2 hand-computed=31 compared=31
[shard 3]   [value differs from the model] regression #128: constant index in an inlined function called in a loop whose variable has the constant's name — r u8[K]
[shard 3]   [value differs from the model] r u60[K]
Tool ledger [difftest-0]: 1 test(s) asked for a tool; 1 lookup(s) found the tool (1 required), SKIP: 0
```

(`Tool ledger` satırı dört parçada da aynı, `difftest-0`..`difftest-3`.)

Sınırlar: generic modül üretilmez; parser'ın generic ikamesi iç gölgelemeyi
görmüyor (#130), eklenirse koşu düşer. Fonksiyon çağrısı yalnız bu biçimde
ve `comb` içinde (ikame kipi) üretilir; tel kipi (modül düzeyi çağrı) ve
`on` bloğu sınanmaz. Gövde `let`'i #132'ye kadar varsayılan kapalı.

## Ek: r biçimi generic modül ve modül düzeyi `for` üretir (#130, 2026-11-05)

Önceki ekin sınırı ("generic modül üretilmez; parser'ın generic ikamesi
iç gölgelemeyi görmüyor, #130") kalktı. r biçiminin çağıran tarafı artık
bir generic modüldür:

| Biçim | Volt | Beklenen |
|---|---|---|
| r | önceki ekteki gibi `fn h<id>r` ve kök const'lar; çağrı test modülünde değil, `R<id>r<const s<id>r: u32, const t<id>r: u32>` modülünde (ikinci parametrenin adı döngü değişkeninin adıdır); test modülü onu `R<id>r<0, V+1>` olarak örnekler. Çift kimlikli ifadede çağrı `comb` içindedir: `let s<id>r = p`, `for t<id>r in V..V+1 { o = h<id>r(..., s<id>r, ...) }`; tek kimlikli ifadede modül düzeyi `for t<id>r in V..V+1 { let s<id>r = p  o = h<id>r(...) }` (ADR-0056 açılımı, çağrı modül düzeyinde: tel kipi) | a ile aynı |

Generic parametreler çağıranın adlarıyla aynıdır ama başka değerleri
taşır: `s<id>r` 0 (her tam sayı tipine sığar; 3 gibi bir değer dar tipte
E2010 verir, ikame değer farkı olarak görünmezdi), `t<id>r` döngünün
değerinden bir fazla. Blok `let`'i, gövde `let`'i ve döngü değişkeni
onları gölgeler (name-resolution.md §2, §6); gölgelenmeyen ad argümana
generic'in literali olarak girer. Gölgelemeler W1002'dir, koşucu
uyarıları saymaz.

Koşucu Verilator'a monomorfların dosyalarını verir: `-y build/rtl`
(`R<id>r_<argümanlar>.sv` örnek adından bulunur) ve `--timescale 1ns/1ps`
(`tb.sv`'nin birimi komut satırında sonraki dosyaya geçer, kitaplıktan
yüklenene geçmez: TIMESCALEMOD). Bu dosyalardaki hata ve uyarılar dosya
adından ifadenin r biçimine eşlenir. Bu ikisi olmadan ilk koşu
(42032953987) MODMISSING ile düştü.

Regresyon kümesine #130'un iki örneği eklendi: `p0 ^ p1` (u8, u8), biri
modül düzeyi `for`'da (kimlik 25), biri `comb`'da (kimlik 26); elle
hesaplanmış değerler (15 ^ 240 = 255, 1 ^ 2 = 3; 85 ^ 170 = 255, 5 ^ 7 =
2). Düzeltmesiz derleyicide argüman generic'in literali (0) olur.

Gövde `let`'i (#132) bu ekte de kapalı (`VOLT_DIFFTEST_FN_LET`).

**Düzeltme geri alındığında #130'u yakalıyor.**

- Araçsız (yerel): regresyon toplu koşusunun `dt.volt`'u düzeltmesiz
  derleyicide (`main`, e1c7663) iki `bits<N>` argümanlı r biçiminde E2003
  verdi (argüman literal oldu); o biçimler çıkarılınca #130 örneklerinin
  SV'si `assign o = 8'd0 ^ p40;` ve `o = 8'(8'd0 ^ p42);` (düzeltmeyle
  `p39`/`p41` okunur).
- Verilator'lı (CI), dört tohum (20261104–20261107, parça başına 2000
  ifade):
  - düzeltmesi geri alınmış dal `test/parser-scope-revert-demo` (yalnız
    `crates/volt-syntax/src`, `crates/volt-hir/src`, `crates/volt-ast/src`
    `main`'deki hâline döner; birleştirilmez; satırlar aşağıda
    alıntılandıktan sonra silindi):
    https://github.com/volt-hdl/volt/actions/runs/42034067237 — dört parça
    da düştü, `diffs=1703`, `1698`, `1715`, `1670`; `regressions=27
    failed=20`. Küçültülmüş 600 bulgunun 591'i r biçiminde, iki kipte de
    (`module R95r<..> { for .. { let ..; o = call } }`, `module R176r<..>
    { comb { let ..; for .. { o = call } } }`).
  - düzeltme dalı: https://github.com/volt-hdl/volt/actions/runs/42034073521
    — dört parça geçti, her parçada `diffs=0`, `regressions=27 failed=0
    hand-computed=35 compared=35`, WIDTH yok.

Günlükten alıntı (GitHub günlükleri 90 gün sonra siler; parça öneki eklendi, bulgu satırları kısaltıldı):

```
[shard 0] difftest: seed=20261104 target=2000 generated=2500 accepted=2188 rejected-as-predicted=137 unsupported=7 unspecified=0 values-compared=242539 diffs=1703
[shard 0] difftest: regressions=27 failed=20 hand-computed=35 compared=33
[shard 0]   [1x] r !!(i13 as u13 >> u6)[u3]
[shard 0]       module R95r<..> { for .. { let ..; o = call } }
[shard 0]       inputs: p84=0x9ec p92=0x1 p122=0x6
[shard 0]       model: 0x1, Verilator: 0x0 (bool)
[shard 1] difftest: seed=20261105 target=2000 generated=2500 accepted=2177 rejected-as-predicted=134 unsupported=10 unspecified=0 values-compared=240422 diffs=1698
[shard 1] difftest: regressions=27 failed=20 hand-computed=35 compared=33
[shard 2] difftest: seed=20261106 target=2000 generated=2500 accepted=2139 rejected-as-predicted=136 unsupported=16 unspecified=0 values-compared=239865 diffs=1715
[shard 2] difftest: regressions=27 failed=20 hand-computed=35 compared=33
[shard 3] difftest: seed=20261107 target=2000 generated=2500 accepted=2160 rejected-as-predicted=142 unsupported=11 unspecified=0 values-compared=238562 diffs=1670
[shard 3] difftest: regressions=27 failed=20 hand-computed=35 compared=33
Tool ledger [difftest-0]: 1 test(s) asked for a tool; 1 lookup(s) found the tool (1 required), SKIP: 0
```

(`Tool ledger` satırı dört parçada da aynı.) Düzeltme dalı:

```
[shard 0] difftest: seed=20261104 target=2000 generated=2250 accepted=2118 rejected-as-predicted=126 unsupported=6 unspecified=0 values-compared=233605 diffs=0
[shard 0] difftest: regressions=27 failed=0 hand-computed=35 compared=35
[shard 0] difftest: lint codes: CMPCONST=8 UNSIGNED=12 UNUSEDSIGNAL=863
[shard 1] difftest: seed=20261105 target=2000 generated=2250 accepted=2127 rejected-as-predicted=116 unsupported=7 unspecified=0 values-compared=235614 diffs=0
[shard 2] difftest: seed=20261106 target=2000 generated=2250 accepted=2111 rejected-as-predicted=125 unsupported=14 unspecified=0 values-compared=234746 diffs=0
[shard 3] difftest: seed=20261107 target=2000 generated=2250 accepted=2108 rejected-as-predicted=132 unsupported=10 unspecified=0 values-compared=232746 diffs=0
```

Satır içi üç bulgu açıklandı (#145). Geri alma koşusunun 3. parçasında 150 küçültülmüş bulgunun üçü satır içi biçimlerdeydi: a, b, d; `p247 - p247`, p247 = 0, Verilator `0xf`. Kaynak Volt değil, CI'ın Verilator'ı:

- OSS CAD Suite 2026-09-21, `5.053 devel rev v5.052-159-g46774fded`.
- Yerelde aynı commit ve tohumla (MINIMIZE=0, Docker) Verilator 5.052 bu üç bulguyu vermiyor (`diffs=1667`). 5.053 devel aynısını veriyor (`diffs=1670`).
- Toplu koşu 9'un SV'si aynen derlenince o151a/b/d/e 16 vektörün 8'inde sıfırdan farklı; `-fno-dfg` ile 0'ında.
- Aynı metinli `o151e` her vektörde doğru.

DFG optimizasyonunun yanlış derlemesi. Düzeltme dalının dört parçasında değer farkı yok. CI aracı için seçenekler #145'te.

Sınırlar: modül düzeyi çağrının ikame kipi (`comb` içinde açılım) ve tel
kipi (modül düzeyi) artık ikisi de sınanır; `on` bloğu sınanmaz.

## Ek: `/` ve `%`, literal aritmetiği (#144, 2026-11-06)

#144'ü (yalnız literal içeren aritmetik, her literal bağlam genişliğine
kesilerek SV'ye yazılıyordu: u2'de `7 / 3` = 1) fark testi yakalamadı
(#125): üreteç `/` ve `%` üretmiyordu, model iki literalin aritmetiğini
"unspecified" sayıyordu (`literal-literal arithmetic`) ve `+ - *`'da
kesme sonucu değiştirmez. Bu ek ikisini de kapsama alır. Önceki karar
metni (§1–§6) değişmedi.

**Model (şartnameden).**

- §3.3: `/` ve `%` genişlemez; esnek işlenende sonucun doğal genişliği
  kesişimin üst ucudur (`+ - *` ile aynı buluşma, ADR-0025).
- İki literalin aritmetiği literaldir (§3.3) ve değeri derleme zamanında
  `i128`'de tam hesaplanır (const-eval.md §1); ara değerler kesilmez.
  const-eval.md §4: `checked_*` taşması E2022, bölen 0 E2023, `/` ve `%`
  sıfıra doğru. Literal ifadesinin değeri karşı yanın ya da hedefin tipine
  sığmalı (§4, E2010); karşılaştırmada ADR-0106 §1, §2.
- Check kipinde itme (ADR-0041; type-inference.md §5 "`+ - * / %`") `/`
  ve `%`'yi de hedef genişlikte yapar. Modelin `push`'u iki literal
  işlenende yaprakların hedefe sığmasını istiyordu; derleyicinin
  `widen_operands`'ı gibi önce iki yanın literal olup olmadığına bakar
  (itme yok, değer katlanır).
- Çalışma zamanında bölen 0: değer SV'de x'tir, model karşılaştırmaz
  (`eval` `None`; G biçimi o ifadede düşer). Şartname tanımlamıyor: #122.

Yeni varsayım etiketleri ("Spesifikasyon belirsizlikleri" tablosunun
devamı; #122):

| Kimlik | Okuma | Şartnamenin söylediği |
|---|---|---|
| A13 | çalışma zamanında işaretli `/` sıfıra doğru keser, `%` bölünenin işaretini alır | const-eval.md §4 derleme anını (`checked_div`, `checked_rem`) tanımlar; çalışma zamanını söylemez |
| A14 | `iN::MIN / -1` sonucun genişliğinde sarar (`iN::MIN`) | §3.3 "bölme/mod genişlemez"; taşmayı söylemez |

**Üreteç.**

- Aritmetik düğüm `+ - * / %` arasından seçer (genişlik 32'yi aşınca
  `*` yok).
- İşlenen çiftinin ikinci yanı (%10) ve karşılaştırmanın literal yanı
  (%10) bir literal aritmetiği ağacıdır (`lit_expr`): değeri bağlamın
  aralığına sığar, yaprakları bilerek geniştir. `/` ve `%` ağırlıklı
  (`a / b == q` için `a = q·b + r`, `a % b == q` için `a = ±m·|b| + q`),
  derinlik en çok 2. Yaprakları bağlam genişliğine kesen derleyici `/` ve
  `%`'de yanlış değer verir; `+ - *`'da kesme sonucu değiştirmez, onların
  hatası sığmayan sonucun kabulüdür.
- İki yeni geçersiz biçim: değeri karşı yanın tipine sığmayan literal
  ağacı (aritmetik, bit işlemi ve karşılaştırmada; E2010) ve sıfır bölenli
  literal bölmesi ya da kalanı (`k / (z - z)`, E2023).
- Üst düzeyde literal ifadesi üretilmez: çıkışın tipi ifadenin doğal
  tipidir, literalin doğal tipi yoktur (`untyped literal expression`).
  Literal aritmetiği işlenen ve karşılaştırma konumlarında sınanır.

**Regresyon kümesi.** #144'ün beş örneği (kimlik 27–31), hepsi elle
hesaplanmış değerli: `p0 == 7 / 3` ve `p0 < 5 % 3` (u2), `p0 & (7 / 3)`,
`p0 + 100 * 3 / 150` (ara değer 300) ve işaretli `p0 == -9 / 2` (i4; -9
i4'e sığmaz, -4 sığar). #130'un örnekleri (25, 26) kaymadı.

Sınırlar: bit işlemi, kaydırma ve dönüşümde iki literalli biçimler
(`7 | 1`, `6 >> 1`, `(7 / 3) as u8`) model için hâlâ unspecified
(#122); üreteç onları üretmez.
