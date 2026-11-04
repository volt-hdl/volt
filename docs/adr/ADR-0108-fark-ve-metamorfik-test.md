# ADR-0108: Aritmetik Anlam için Fark ve Metamorfik Test — Derleyiciden Bağımsız Referans Model, Araçlı İşte Sabit Tohumlu Koşu

> Statü: Uygulandı
> Önceki karar: ADR-0036 — §2'nin "genişlik değişen cast'ler aynen kalır" satırı: genişlik değiştiren `as` sonucunun SV işareti artık hedefin işaretidir (`$signed`/`$unsigned` sınırı, #115)
> İlgili: ADR-0106 (§6'nın yalıtma ilkesi değeri genişliğe bağlı bütün alt ifadelere genişler, #117), ADR-0041 (check kipinde itme yalnız en üstteki işlemi hedef genişlikte yapar; işlenenler doğal değerleriyle genişler), ADR-0107 (fark testi araç sorgusunu `tools` modülünden yapar, araçlı işte SKIP olmadan koşar)
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
| #123 | Üretilen SV'de Verilator WIDTH uyarısı (varsayılan olarak ölümcül, `volt test` durur): değişken indeksin ya da parça seçimi başlangıcının genişliği adres genişliğinden farklı (`x[i]`, x : u15, i : u3); bileşik değerin genişleten dönüşümü `W'(a + b)` (içteki örtük genişleme, ADR-0041'in biçimi); daha geniş hedefteki aynı genişlikte dönüşüm; bileşik tabandan `-:` seçiminde boyutsuz başlangıç farkı | Genişliği adres genişliğine (`clog2`) uymayan vektör indeksi ve başlangıcı kaydırmaya iner (`x[i]` → `1'((x) >> i)`, `q[k +: 35]` → `35'((q) >> k)`; kaydırma miktarına uyarı yok, sınır dışı indeks zaten x'tir, ADR-0035; açık genişletilmiş başlangıç `q[6'(k) +: 35]` Verilator 5.053'te DFG iç hatasına yol açıyordu); unpacked dizi indeksi adres genişliğinde (`regs[5'(i)]`); değeri genişlikten bağımsız bileşik işlenen hedef genişlikte, atomları açık: `(a + b) as i32` → `(32'(a) + 32'(b))`; aynı genişlikte dönüşüm geniş bağlamda `W'($unsigned(...))`; fark sabitin genişliğinde `(5'(s) - 5'd27)`. Fark testinde WIDTH uyarısı sert bulgudur |

`sv-mapping.md` §6 tablosundaki `a as i16` → `{{8{a[7]}}, a}` satırı ve
§10'daki literal biçimleri bu ADR'yle değişir (`$signed(...)` sınırı,
`'sh`); §16.1'in "bileşik alt ifadeye bağlam içeri aktarılır" kuralı
değeri genişlikten bağımsız alt ifadeler için geçerli kalır.
Spesifikasyon metni değişmez; çelişki burada kayıtlıdır.

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
4. **Son durum** (1fddec7a):
   - CI run 41767138224, araçlı iş: `difftest: seed=20261104 target=2000
     generated=2250 accepted=2118 rejected-as-predicted=126 unsupported=6
     unspecified=0 values-compared=205933 diffs=0`, `difftest:
     regressions=22 failed=0`, `Tool ledger [integration]: 65 test(s)
     asked for a tool; 67 lookup(s) found the tool (67 required), SKIP:
     0`; "Tool coverage" `68 (test, tool) pair(s) in the ledgers, 68 ran
     with the tool required, 0 did not`.
   - Büyük koşu run 41767138333, tohumlar 20261105–20261108: kabul
     edilen 12.724 + 12.514 + 12.537 + 12.518 = **50.293 ifade**
     (üretilen 53.250; modelin öngördüğü gibi reddedilen 2.697; E0003
     ile reddedilen `concat()` 260; tanımsız 0); 349.400 biçim,
     4.926.905 değer karşılaştırıldı (x olduğu için atlanan 663.495),
     654.704 sözleşme örneklemesi; her parçada `diffs=0`, regresyonlar
     22/22, `SKIP: 0`.
5. **Yerel** (Docker, Verilator 5.052, tohum 777): düzeltmeden önce 3000
   ifadede 2442 fark; düzeltmeden sonra 1500 hedefte 1646 kabul, 0 fark.
6. **Külliyat** (455 dosya, main ile `volt build --emit=sva`): 15
   dosyanın SV/SVA metni değişti, değerleri aynı (`$signed({...})`
   sınırı, `8'sh80`, kaydırma miktarı doğal genişlikte, `bits<13>`
   dönüşümünde `$unsigned`).

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

- `if`, `match`, `/`, `%`, `prev()`, fonksiyon çağrısı, struct ve enum
  ifadeleri üretilmez.
- Sözleşme biçimi yalnız `ensures`'tır ve simülasyonda (Verilator) sınanır;
  formal akış (Yosys/sby) fark testinin dışında.
- 64 bitten geniş tam sayı (E0003) ve `concat()` (E0003) yalnız reddin
  sayılması için üretilir.
- Verilator `-Wall` uyarıları (WIDTH dışında) yumuşak bulgudur: üretilen
  ifadenin kendisi sabit karşılaştırma (CMPCONST, UNSIGNED) ya da kısmen
  okunan `let` (UNUSEDSIGNAL) içerebilir.
