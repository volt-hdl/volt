# ADR-0144: Fark Testinin Kapsamı Ölçülür, Trit Üretilir, Tanımsız İşlemler Beklenen Retle Sınanır

> Statü: Önerildi
> İlgili: ADR-0108 (fark testi: üretecin kapsamı bir raporla ölçülür, model ve üreteç `Trit`'i taşır, varsayım tablosu A15–A16 ile sürer, tanımsız işlemler için beklenen ret ölçütü eklenir)
> Issue: #458 (aşama 0 ve 1a; #434 ölçütün sınaması)
> Tarih: 2026-11-27
> Etkilenen: `crates/volt-difftest/src/coverage.rs` (YENİ), `crates/volt-difftest/src/reject.rs` (YENİ), `crates/volt-difftest/src/{ast,model,gen,harness,run,lib}.rs`, `crates/volt-difftest/examples/volt-difftest-reject.rs` (YENİ), `crates/volt-difftest/tests/{coverage_tests,reject_tests,trit_model_tests}.rs` (YENİ), `crates/volt-driver/tests/difftest_reject_tests.rs` (YENİ), `CHANGELOG.md`

## Sorun

Fark testinin üreteci (ADR-0108) `bool`, `uN`, `iN` ve `bits<N>` üretiyordu.
`Trit` hiç üretilmiyordu; #434 (`~t` Trit'in kodunu tersliyordu, sessiz
yanlış) bu yüzden görülemedi. Neyin üretildiği de hiçbir yerde
sayılmıyordu: bir tip ya da işlem üreteçten düşse koşu yine yeşildi.

İkinci bir boşluk: ADR-0142'den beri `~t` E2003'tür. Bugünkü spesifikasyona
uyan bir üreteç `~t`'yi geçerli program olarak hiç üretmez; geçersiz
akışın kodu da yalnız "yumuşak" bir bulgudur (ADR-0108 §3, `CodeMismatch`).
Tanımsız bir işlemin kabul edilmesi, #434'ün sınıfı, ancak o işlemi bilerek
üretip reddini bekleyen bir ölçütle görülür.

## Karar

### 1. Kapsam raporu (aşama 0)

Her koşu üretilen ifadelerin tiplerini, işlemlerini, literal yazımlarını
ve biçimlerini (satır içi, `let`, alt ifade `let`'te, parantez, yer
değiştirme, `let`'le ve literalle karşılaştırma, geniş hedef, `ensures`,
fn) sayar; rapor fark testinin çıktısının sonundadır. Dilin ifade
düzeyindeki tip ve işlem listesi tek tablodadır
(`coverage::LANGUAGE`; kaynağı `grammar-full.ebnf` §12–§14 ve
`type-inference.md` §1, §3). Her satır üretecin onu üretip üretmediğini
söyler; hiç üretilmeyen satır raporda "never generated" ile, kaynağı ve
sonraki adımıyla görünür. Biçimlerin listesi `harness::Variant::ALL`'dur;
ikinci bir kopyası yoktur.

`tests/coverage_tests.rs` CI'nın küçük koşusunun tohumuyla
(`Config::DEFAULT_SEED`, 2000 ifade) Verilator'suz koşar: üretilen her
satır en az bir kez çıkar, üretilmeyen hiç çıkmaz, her biçim vardır. Bir
tür sıfıra düşerse ya da tabloya yazılmadan üretilmeye başlarsa test
kırmızıdır.

### 2. Trit modeli (aşama 1a)

Model `Trit`'i değer kümesiyle taşır ({-1, 0, +1}, ADR-0003); değer
örüntüsü ADR-0062 Karar 1'in kodudur (+1 = `01`, 0 = `00`, -1 = `11`).
Kurallar:

| İfade | Model | Kaynak |
|---|---|---|
| `-t` | Trit; -(+1) = -1, -0 = 0, -(-1) = +1 | type-inference.md §3.4, ADR-0142 |
| `t * u` | Trit (kapalı) | §3.3, §10 |
| `t + u`, `t - u` | i3 | §3.3, §10 |
| `t * x`, `x * t` (x : iN, somut) | iN; geniş hedefte en üstteki çarpım hedef genişliğinde | §3.3, §10; ADR-0062 Karar 2, ADR-0041 |
| `t as iN`, N >= 2 | işaretle genişler | §3.6 |
| `t as` (uN, i1, bool, bits<N>); her tipten ve literalden Trit'e | E2009 | §3.6, §10 |
| Trit bağlamında literal | {-1, 0, +1}; dışı E2011 | §4, §7; ADR-0003 |
| `t == u`, `t != u`, Trit literaliyle | bool, değerle | §3.3 |
| `t < u` ve öteki sıralamalar | bool, A16 | §3.3 (yalnız "aynı tip") |
| `~t`; `&`, `\|`, `^`; `<<`, `>>`; `t[i]` | E2003 | ADR-0142 §1, §4 |
| Trit'in öteki tip karışımları (`t + x`, `t * y` (y : uN), `t == x`) | hata, kod adlandırılmamış | §3.3 `error_incompatible` |

Üreteç Trit portlarını ve literallerini, bu işlemleri ve dönüşümleri
bugünkü bütün biçimlerde üretir; `-t` ve Trit çarpımı (`t * u`, `t * x`)
bugün yalnız `VOLT_DIFFTEST_TRIT_ARITH=1` ile (§5). Varsayılanda Trit
portları, literalleri, `t ± u`, `t as iN` ve karşılaştırmalar üretilir.
Model bütün tabloyu taşır; ret ölçütü işlenenlerini kapıdan bağımsız,
bütün Trit ifadelerinden alır. Testbench Trit portunu yalnız değer
kümesinin kodlarıyla sürer: `10` (-2) Volt kaynağından üretilemez
(ADR-0062 Karar 1) ve Trit'in anlamı onu tanımlamaz.

Model şu biçimleri karşılaştırmaz ve üreteç onları üretmez (Unspecified):
esnek aralıklı işlenenle Trit çarpımı (ADR-0025'in aralığı Trit'te
tanımlı değil), Trit ile indeksleme ve Trit kaydırma miktarı (§3.3–§3.5
Trit'in "sayısal" olup olmadığını söylemez), `t as Trit` (Sınırlar).

### 3. Varsayım etiketleri A15 ve A16

ADR-0108'in "Spesifikasyon belirsizlikleri" tablosunun devamı. Etiketli
ifadeye elle hesaplanmış beklenen değer (`pin`) konmaz.

| Kimlik | Belirsizlik | Modelin okuması | Derleyici |
|---|---|---|---|
| A15 | `t * x` (x : iN) iN'dir (§3.3); t = -1, x = iN::MIN'de çarpım 2^(N-1) iN'e sığmaz, şartname değeri söylemez | Çarpımın yapıldığı genişlikte ikiye tümleyenle sarar: doğal genişlikte iN::MIN; geniş hedefte en üstteki çarpım hedef genişliğinde yapıldığından sarmaz (ADR-0062 Karar 2) | Aynı (`-(x)` çarpımın bağlamında; iç içe çarpım doğal genişliğinde yalıtılır) |
| A16 | §3.3 karşılaştırmada yalnız "aynı tip" ister; Trit'te sıralamanın anlamını söylemez | Değerler karşılaştırılır, -1 < 0 < +1 | Kabul eder; SV'de işaretli 2 bitlik karşılaştırma, aynı sıra |

### 4. Beklenen ret ölçütü

`reject::UNDEFINED` her tipin tanımsız işlemlerini tek tabloda, beklenen
tanı koduyla ve kaynağıyla tutar: Trit'te `~`, `&`, `|`, `^`, `<<`, `>>`,
bit seçme (E2003, ADR-0142); Trit'ten uN, i1, bool, bits<N>'e ve uN, iN,
bits<N>, bool, literalden Trit'e dönüşüm (E2009, §3.6); Trit bağlamında
{-1, 0, +1} dışı literal (E2011, §4); `bits<N>`'de `+ - * / %` (E2004,
§3.3); `uN`'de `-` (E2002, §3.4). Mod her satırdan, işlenenleri üreteçten
(port, `-t`, `t * u`, iç içe ifade) alarak örnek üretir, tek `volt check`
ile denetler ve her ifadenin satırında tam o kodu bekler. Kabul ya da başka
kod farktır. Verilator gerekmez.

Tablo ile model aynı kuralı iki yerde hesaplamaz: tablo hangi
birleşimlerin deneneceğini ve kaynağını taşır; her örneğin kodu modelden
de alınır ve tablonunkiyle aynı olmalıdır (ayrışma koşuyu düşürür).

- `crates/volt-driver/tests/difftest_reject_tests.rs` ölçütü bu derleyiciyle
  her işte koşar (araç gerekmez).
- `cargo run -p volt-difftest --example volt-difftest-reject -- --volt
  <yol> [--seed N]` aynı ölçütü verilen bir ikiliyle koşar (yayımlanmış
  bir sürüm gibi); ikili depoya girmez.

### 5. Bulunan derleyici hatası ve kapısı

SV üreticisinin doğal genişlik aralığı (`volt-sv-emit` `let_width.rs`
`int_range`/`binary_range`) `-t`'yi i3 sayar, `Trit * iN`'e aralık vermez
(N ≠ 2), `Trit * Trit`'e [2, 4] verir. Bu PR'ın ilk CI koşusu tam üreteçle
dört sonucunu gösterdi (v0.1.3'te de var olan hatanın sonuçları):

- literalle karşılaştırmada geçerli tasarım E2005 alır (`t * x == 1`,
  `-t * u < 0`);
- tipsiz `let`'in teli kısa bildirilir ve değer kesilir (sessiz yanlış):
  `let s = p44 * p192 * (1 * p282)` (i14) `wire signed [7:0]`, p44 =
  0x2000, iki Trit +1 iken 0;
- `-(t * x)` genişlemez (sessiz yanlış): `(-(p79 * p227))[17:1]`, t = -1,
  x = -1, beklenen 0x1ffff, Verilator 0xffff; Trit'siz eşdeğer biçim doğru;
- `let s = -t` 3 bitlik teldir; `s * 1` 2 bitlik kodla karşılaştırır:
  WIDTHEXPAND ve WIDTHTRUNC (`volt test` durur, #123).

Model ve spesifikasyon bu ifadelerin hepsini geçerli ve tanımlı der; fark
derleyicinin hatasıdır ve bu ADR'nin değişikliğinde düzeltilmez. Üreteç
`-t`'yi ve Trit çarpımını yalnız `VOLT_DIFFTEST_TRIT_ARITH=1` ile üretir
(#150'nin `VOLT_DIFFTEST_DIV_OPERANDS`'ı gibi). Hatanın issue'su kapanınca
varsayılan açılır; o zaman A15 de koşuda görünür olur.

## ADR-0108 ve ADR-0139 ile ilişkisi

- **ADR-0108.** §1'in ilkesi aynı: `volt-difftest` hiçbir `volt-*`
  crate'ine bağlanmaz, derleyiciyi `volt` ikilisi olarak çağırır; model
  spesifikasyondan yazılır, belirsizlik bir varsayım etiketidir. Bu ADR
  modelin tiplerine Trit'i, varsayım tablosuna A15–A16'yı, koşuya kapsam
  raporunu ve beklenen ret ölçütünü ekler. Ret ölçütü ADR-0108 §3'ün
  geçersiz akışından ayrıdır: orada model "hata" der, kod yumuşak bir
  bulgudur; burada tablo kodu adlandırır ve yanlış kod sert farktır.
- **ADR-0139.** Model Trit'in değer kümesiyle hesaplar (`-t`, çarpım,
  toplam, karşılaştırma değerler üzerinde) ve örüntüyü yalnız Verilator'la
  karşılaştırmak için ADR-0062'nin kodundan alır. Ret tablosunun Trit
  satırları ADR-0139 §1'in "işlem değer kümesine aittir" ayrımının
  sınamasıdır: değer kümesinde anlamı olmayan ikili işlemler (`~`, bit
  işlemleri, kaydırma, bit seçme) reddedilmelidir. ADR-0139 §4'ün
  "ikili–üçlü sınır kuralları bugünkü SV arka ucunda sınanabilir" yönü
  için fark testinin iki yanı da vardır: kuralın kabul ettiği tasarım
  Verilator'da modelle, reddettiği tasarım beklenen kodla sınanır.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; dilin yazımı değişmez.
2. **Çıkarım (İ2):** uygulanmaz; derleyicinin çıkarımı değişmez. Model
   Trit'in tipini spesifikasyonun kurallarından tek cevaplı çıkarır;
   kuralın sustuğu yerde (A15, A16, Unspecified biçimler) okuma etiketle
   adlandırılır, tahmin edilmez.
3. **Anlam değişikliği (İ3):** dilin anlamı, üretilen SV ve tanılar
   aynı; derleyicinin kaynağı değişmez. Değişen fark testinin kendisidir:
   üretilen ifadeler (aynı tohum başka ifadeler verir), rapor ve yeni
   ölçüt.
4. **Niyet tahmini (İ4):** uygulanmaz.
5. **Metamorfik yeniden yazım:** yeni bir eşdeğerlik ilişkisi yoktur;
   Trit ifadeleri ADR-0108'in mevcut biçimlerinden geçer. Yer değiştirme
   biçimi `t * x`'i `x * t`'ye çevirir: §3.3'ün kuralı iki sırayı da sayar
   (`(Trit, SInt) | (SInt, Trit)`).

## Doğrulama

- Yerelde, Verilator'suz: tip denetimi karşılaştırması (model geçerli
  diyor, `volt check` kabul ediyor, `volt build` geçiyor; model geçersiz
  diyor, derleyici reddediyor) CI tohumuyla 12 toplu koşuda (3000 ifade,
  440'ı Trit'li geçerli) ve 2026112700 tohumuyla 24 toplu koşuda (857)
  sıfır ayrışma (tam üreteçle, kapıdan önce; E2005 dışında).
- Tam üreteçle CI: "Verilator lint" (koşu 45942223357, tohum 20261104,
  2000 ifade) `diffs=179`: 4 değer farkı, 175 WIDTH uyarısı. Elle
  tetiklenen büyük koşu (45942236536, tohum 2026112600 + k, parça başına
  12 500) dört parçada `diffs=933, 945, 987, 987`: 272 değer farkı, 25
  sözleşme düşmesi, ~3550 WIDTH uyarısı. Değer farkı ve sözleşme
  gruplarının hepsi `-t` ya da Trit çarpımı içerir; içermeyen Trit'li
  bulgu yok.
- Kapıdan sonra, yerelde: CI tohumunun ilk 9 toplu koşusunun SV'si
  sabitlenmiş Verilator 5.054 ile `--lint-only -Wall`: WIDTH uyarısı yok;
  `VOLT_DIFFTEST_TRIT_ARITH=1` ile 3 toplu koşuda 50'den çok.
- Beklenen ret ölçütü, tohum 458, satır başına 8 örnek (23 satır, 184
  ifade): v0.1.3'ün yayımlanmış Windows ikilisiyle `diffs=8`, sekizi de
  `Trit: ~a — expected E2003, compiler accepted` (#434); bugünkü main
  (ad0dd2d7) ile `diffs=0`.
- Trit'in olumlu ifadelerinin değerleri Verilator'da PR'ın CI'ındaki küçük
  koşuda ve dalda elle tetiklenen büyük koşuda sınanır (PR açıklaması).

## Sınırlar

- **`t as Trit`:** §3.6'nın `(_, Trit) => false` kolu onu E2009 yapar;
  §3.6'da `bits<N> as bits<N>` ve `bool as bool` için de özdeşlik kolu
  yoktur ve derleyici üçünü de kabul eder. Spesifikasyonun bir boşluğudur;
  model karşılaştırmaz, üreteç üretmez, karar bakımcınındır.
- **Trit ile indeksleme (`x[t]`):** derleyici kabul eder, Trit kaydırma
  miktarını (`x >> t`) E2003 ile reddeder; §3.3 ve §3.5 Trit'in sayısal
  olup olmadığını söylemez. Model karşılaştırmaz, üreteç üretmez.
- **Aralık ve parça seçimi Trit'te** (`t[1:0]`, `t[0 +: 1]`): derleyici
  E2003 verir; ADR-0142 §4 yalnız bit seçmeyi adlandırır, §3.5'in
  `width_of`'u Trit'in genişliğini söylemez. Model hata der, kodu
  adlandırmaz; ret tablosunda değil.
- Trit dizisi (`[Trit; N]`) ve Trit içeren struct aşama 1b ve 1c'nin
  işidir; register ve `prev` aşama 2'nin (#458).

## Reddedilenler

- **Kapsamı elle tutulan ikinci bir listeyle raporlamak.** İki liste
  ayrışırdı; tablo tek yerde ve test onu iki yönden sınar.
- **Beklenen reddi ADR-0108'in geçersiz akışına katmak.** Orada kod
  yumuşak bir bulgudur ve geçersiz ifadeler Verilator koşusunun içindedir;
  ret ölçütünün sert, araçsız ve eski bir ikiliye uygulanabilir olması
  istenir.
- **A15'te çarpımı karşılaştırmamak** (`iN::MIN / -1` gibi, #122). Değeri
  ADR-0062 Karar 2'nin çarpım bağlamı belirler; okumayı etiketlemek
  derleyiciyle ayrışırsa farkı gösterir, karşılaştırmamak gizlerdi.
- **Hatalı biçimleri üretip farkı her koşuda görmek.** CI'ı bilinen bir
  hata için kırmızı tutar; kapı biçimi adlandırır, `=1` onu geri açar.
- **Kapıyı yalnız hatalı biçimlere daraltmak** (yalnız `let` biçimleri,
  yalnız literalle karşılaştırma). Hata satır içi biçimde de değer
  değiştiriyor; sınırı üreticinin hatalı aralık verdiği iki işlem çiziyor.
