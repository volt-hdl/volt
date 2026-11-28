# ADR-0148: Generic Trit Sabiti Tip Denetiminde Trit Kalır — Generic Argüman Bir Sabit İfadedir

> Statü: Önerildi
> İlgili: ADR-0041 (monomorfizasyon: parametre literale çevrilir; literal artık parametrenin bildirilen tipini taşır, argüman bir sabit ifadedir, negatif değer adda `n`), ADR-0142 (`~S` aynı mekanizmaya bağlanır: işlenenin tipi Trit), ADR-0145 (Karar 8'in Trit × iN kuralı generic Trit sabitine uygulanır; Doğrulama'daki generic notu kapanır), ADR-0146 (Sınırlar: `x[S]` artık E2003), ADR-0147 (Sınırlar: `P<-1>` yazılabilir; `const S : i8` literal kalır), ADR-0100 (sessiz yanlış düzeltmesi önce kabul edileni reddedebilir; yamada yeni kod yok), ADR-0137 (Trit'i tanıyan yer tip denetiminin tablosu), ADR-0139 (değer kümesi tip denetiminde; üretici Trit'in genişliğini tablodan okur), ADR-0136 (gölgenin girdisi), ADR-0108 (fark testinin t biçimi), ADR-0117 (gerçek hata derlemi), ADR-0003 (Trit'in değer kümesi {-1, 0, +1})
> Issue: #468, #471
> Tarih: 2026-11-28
> Etkilenen: `crates/volt-ast/src/lib.rs` (`GenerateInfo::generic_literals`), `crates/volt-ast/src/checked.rs` (`CheckedTypes::insert_trit`, `is_trit`), `crates/volt-syntax/src/parser/stmt.rs` (`instance_generics_ahead`), `crates/volt-syntax/src/parser/item.rs` (`parse_generic_args`, `generic_arg_is_expr`), `crates/volt-syntax/src/parser/mono/{mod,clone,scope,unroll}.rs` (`ArgValue`, `arg_info`, `named_const`, `ParamRange::contains`, `mangle`, `Cloner::subst_generic`, `visible_const_in`), `crates/volt-hir/src/typeck/{synth,check,literal_fold,checked}.rs` (`is_generic_trit`), `crates/volt-sv-emit/src/{trit,let_width,expr}.rs` (`is_trit` literal kolu, `checked_trit`, `checked_trit_sig`), `crates/volt-difftest/src/{harness,run,regress}.rs` (t biçimi, #468 örnekleri), `crates/volt-hir/tests/{generic_trit_const_tests,trit_index_tests,trit_bit_not_tests}.rs`, `crates/volt-syntax/tests/generic_arg_tests.rs` (YENİ), `crates/volt-driver/tests/generic_trit_sim_tests.rs` (YENİ), `tests/real_bugs/468_generic_trit_const.volt` (YENİ), `tests/real_bugs/471_generic_trit_minus_one.volt` (YENİ), `tests/ui/pass/167_trit_recognition_shapes.volt` (gölgenin girdisi), `book/src/limitations.md`, `CHANGELOG.md`; spec type-inference.md ve sv-mapping.md için yama bakımcıda

## Sorun

**#468.** Ayrıştırıcı generic `const S : Trit` parametresini örnekte
değerinin literaline çevirir (ADR-0041). Tip denetimi literalin S'den
geldiğini görmüyordu: S'li bir ifade literal kuralıyla tipleniyordu.
main'de (d9ff3513) tip denetiminin `let`'e verdiği tipi yazan bir probla
(geçici, commit edilmedi) `G<1>` içinde ölçüldü, x : i8, aynı yerde bir
Trit portu t ile:

| Biçim | S ile | t ile |
|---|---|---|
| `S * x`, `x * S`, `-S * x` | i16 | i8 |
| `-(S * x)` | i17 | i9 |
| `-S`, `S * S`, `S * 1`, `S + 1`, `S << 1` | literal (W2012) | Trit, Trit, Trit, i3, E2003 |
| `S`, `if c { S } else { -S }` | i32 (W2012) | Trit |
| `S * u` (u : u8) | u16 | E2003 |
| `S * 2` | literal | E2011 |
| `w = S` (w : i8) | tanısız | E2003 |
| `x[S]`, `x[S +: 2]` (x : u8) | tanısız | E2003 (ADR-0146) |
| `S == 1`, `match S { 1 => … }` | `volt check` E2005 | tanısız |
| `~S` | E2003 (ADR-0142) | E2003 |

Değer de spec'ten ayrılıyordu. `volt test` ile Verilator 5.054'te (Docker)
`V<1>`, x : i8 = -128, z : i2 = -2, aynı yerde t = +1:

| Biçim | S ile | t ile (spec, ADR-0145 Karar 8) |
|---|---|---|
| `((-S * x) as i12) as u12` | 128 | 3968 (-128) |
| `((-S * z) as i12) as u12` | 2 | 4094 (-2) |
| `let p = -S * x; w : i16 = p` | 128 | 65408 (-128) |
| `((-S * x + k) as i12) as u12`, k = 0 | 128 | 3968 |
| `(-S * x) < 0` | false | true |
| `((x * -S) as i12) as u12` | 128 | 3968 |

`(S * x) as i12`, `-(S * x)`, en üstte geniş hedefe yazılan `-S * x`, `S +
t`, `if`, Trit portuna atama ve register aynı değeri veriyordu. Depoda
generic `const S : Trit` yalnız `tests/ui/pass/167_trit_recognition_shapes.volt`'ta
(`Scaled<1>`; SV'si `16'sd1 * 16'(x)`) ve üç crate testinde
(`trit_index_tests`, `trit_bit_not_tests`; literal tiplemesini
sabitleyen `a_const_generic_trit_index_is_a_literal_and_is_accepted`)
geçiyordu; `examples/` (hybrid_accel dahil) ve kitap kullanmıyor.

**#471.** `Scaled<-1>`, `Scaled<(-1)>`, `Scaled<0 - 1>` ve aynı biçimde
`Scaled<(1)>`, `Scaled<1 - 1>` E0010 + E0001 alıyordu: örnekleme ileri
bakışı (`stmt.rs` `instance_generics_ahead`) `<` ile `>` arasında yalnız
ad, literal, virgül ve tip anahtar sözcüklerini kabul ediyordu; `-` ya da
`(` görünce deyim ifade yoluna düşüyor ve `Scaled < -1 > { … }` bir
karşılaştırma zinciri okunuyordu. Adlı sabit (`Scaled<NEG>`, `const NEG :
Trit = -1`) E2008 alıyordu: monomorfizasyon (`mono/mod.rs` `arg_info`)
yalnız literal argüman kabul ediyordu, ayrıştırıcı çıplak adı tip sayar
(`GenericArg::Type`) ve o kol değerlendirilmeden reddediliyordu. İşaretli
parametrede de aynıydı (`P<-1>`, `const K : i8`). Ayrıca `Scaled<2>`
tanısız kabul ediliyordu: Trit parametrenin değeri {-1, 0, +1} dışında
olabiliyordu.

## Karar

Bakımcının kararı (#468, 2026-11-28): (A) generic `const S : Trit` tip
denetiminde Trit olarak kalır; S'nin geçtiği her ifade, aynı yerde bir
Trit sinyali olsaydı alacağı tipi ve değeri alır.

### 1. Literal parametrenin bildirilen tipini taşır

Monomorfizasyon parametreyi yine literale çevirir (ADR-0041 değişmez);
klonun gövdesinde parametrenin yerine geçen her literal (negatif değerde
`-`'nin işleneni) yan tabloya yazılır: `GenerateInfo::generic_literals`,
literal → parametrenin bildirilen tip referansı. Ayrıştırıcı tipi
yorumlamaz. Tip denetimi tabloyu okur (`synth.rs` `is_generic_trit`):
bildirilen tipi `Trit` olan parametrenin literali Trit tiplidir, değeri
`check_int_lit`'in Trit kuralıyla denetlenir. Böylece:

- `S * x` Trit × iN kuralıyla iN'dir (§3.3); iç içe bağlamda iN'de sarar,
  en üstte geniş hedefe itilir (ADR-0145 Karar 8);
- `-S` Trit, `S * S` Trit, `S ± t` ve `S + 1` i3;
- `~S` E2003 (ADR-0142), `x[S]` ve `x[S +: 2]` E2003 (ADR-0146), `x[S + 1 :
  S]`'in üst sınırı i3 E2003 (ADR-0147);
- `w = S` (w : i8) E2003; `S * u` (u : uN) E2003; `S * 2` E2011;
- `S == 1` ve `match S` tanısız;
- `G<2>`, `G<-2>` E2011 (S'nin kullanıldığı yerde; literal Trit'in
  kümesinde olmalı).

Literal Trit bağlamının tipini almaz (`check.rs`: check kipinin literal
kolu ona uygulanmaz, sentezlenip atanabilirliği denetlenir) ve literal
ifadesi sayılmaz (`literal_fold.rs`: `collect_literal_vars`,
`literal_sign_mag`); tipi her bağlamda Trit'tir.

### 2. Trit'i tanıyan yer tip denetiminin tablosu

Tip denetiminin cevabı (`CheckedTypes`, ADR-0141) tipi `Ty::Trit` olan
ifadeleri ayrıca işaretler (`insert_trit`, `is_trit`): genişlik ve işaret
Trit'i i2'den ayırmaz. Üretici literalin Trit olduğunu sözdiziminden
tanımaz, tablodan okur: `trit.rs` `is_trit`'in literal kolu
`checked_trit`'tir (fn açılımının kopyası özgün düğümünkini okur, ADR-0081).
Trit tipli ifadenin kendi genişliği de tablodandır (`checked_trit_sig`):
literal, `-S` ve S'yle katlanan literal ağacı (`S * 1`) bağlamı olmayan
yerde (seçicinin `S == 2'sb01`'i, karşılaştırma) bu genişlikte yazılır.
Üretici Trit'in genişliğini yazmaz (ADR-0139). `is_trit`'in sözdizimsel
kolları (ad kümesi, ADR-0062; #467) büyümez: literal kolu eklenen tek
yerdir ve tabloyu okur.

ADR-0142'nin `~S` yolu aynı mekanizmaya bağlanır: S Trit olduğundan
`synth_unary` işlenenin tipinden E2003 verir; ADR-0142'nin check kipi yolu
(`check_literal_bit_not`, literal tipli ağaç) S'ye artık uğramaz. Tanı
bir kez çıkar.

### 3. Trit dışındaki generic const tipleri değişmez

`const W : u32`, `const K : i8` ve öteki tiplerin literali yan tabloya
yazılır ama tip denetimi yalnız `Trit`'e bakar: tiplemeleri literal kalır
(`K * x` i16, `-K` literal, `u[K]` tanısız ya da negatif değerde E2006;
ADR-0147 Sınırlar değişmez). Takma ad (`type W = Trit`, `const S : W`)
görülmez: şablonun tip yolları çözülmez, literal kalır (Sınırlar).

### 4. Generic argüman bir derleme zamanı sabitidir (#471)

Gramer argümanı bir ifade sayar (`GenericArg = Type | Expr`); argüman bir
derleme zamanı sabitidir:

- **İleri bakış:** `instance_generics_ahead` sabit ifadenin aritmetiğini
  ve parantezlerini (`-`, `+`, `*`, `/`, `%`, `(`, `)`) kabul eder. Yuva
  `>` ile kapanıp ardından `{` gelmiyorsa (`a < -1 > b`) bu bir
  karşılaştırma zinciridir: E0010 ifade yolunda kalır.
- **Argümanın ayrıştırılması:** `parse_generic_args` `(` ile başlayan
  sabit ifadeyi (`(-1)`, `(N - 1)`) ve aritmetiği başlayan adı (`N - 1`)
  ifade olarak ayrıştırır (`generic_arg_is_expr`); `(u8, u8)` demet tipi
  kalır.
- **Değer:** monomorfizasyon argümanı ayrıştırıcı katmanının sabit
  değerlendirmesiyle hesaplar (`eval_const_scoped`, `for` sınırınınki):
  literal, `-literal`, sabit aritmetik ve görünen bir kök const'un adı.
  Ad, örneğin bulunduğu modülde gölgelenmeyen bir kök const'a bağlanır
  (name-resolution.md §4: port, parametre ve modül düzeyi bildirim onu
  örter); sinyalin ya da tanımsız adın argümanı E2008 kalır. Değer işaret
  ve büyüklüktür (`ArgValue`); çıplak literal 2^128 - 1'e kadar (#207).
- **Ad:** negatif değer monomorf adında `n` ve büyüklüğüyle yazılır:
  `Scaled<-1>` → `Scaled_n1` (`-` bir SV adında olamaz). `Scaled<-1>`,
  `Scaled<(-1)>`, `Scaled<0 - 1>` ve `Scaled<NEG>` aynı değerdir, aynı
  modülü adlandırır. Çakışma kuralı değişmez (#205, E1003).
- **Aralık (#262):** negatif argüman parametrenin tipine sığmalıdır:
  i8'e -128 sığar, -129 sığmaz; uN'ye negatif değer sığmaz. Sığmayan E2010.

### 5. Tanılar: yeni kod yok

ADR-0100: yamada yeni kod yok. E2008'in iletisi kabul edileni söyler:
`Scaled S must be a compile-time constant`, yardım `write a literal or
the name of a const item, e.g. Scaled<8> { ... }`. E2010 alt sınırı
söyler: `generic argument -129 does not fit in the parameter K : i8 of P
(minimum -128)`, yardım `use a value from -128 to 127, or declare K with a
wider type`; işaretsizde `(minimum 0)` ve `declare K with a signed type`.

### 6. Fark testi ve gölge

Fark testinin üreteci generic const üretmiyordu. Harness yeni bir biçim
alır, **t** (`Variant::T`, ADR-0108'in biçim listesine): ifadenin okuduğu
ilk Trit portu bir generic modülün `const S : Trit` parametresi olur
(`T<id>t<const S : Trit>`), modülün `S = 1`, `0`, `-1` örneklerinden
portun değerine göre biri seçilir. Beklenen değer a biçimininkidir
(kararın kendisi); her geçerli Trit ifadesi her koşuda bu biçimde de
sınanır. Elle hesaplanmış pinler a–e biçimleriyle birlikte t biçiminde de
karşılaştırılır; `regress.rs` #468'in üç örneğini (`(-S * x) as i12`, `(S *
z) as i12`, `-S * x + y`; A15 köşesi, M<1>, M<0>, M<-1>) elle hesaplanmış
değerlerle sabitler, #463'ün Trit örnekleri de t biçiminde sınanır.
Gölgenin girdisi (`167_trit_recognition_shapes.volt`) `Scaled<-1>`'i ve
`-S`, `-(S * x)`, `-S * x`, `if c { S } else { -S }` biçimlerini alır.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; S'nin tipi zaten yazılıdır (`const S :
   Trit`). `M<-1>` ve `M<NEG>` grameri değiştirmez (`GenericArg = Type |
   Expr`).
2. **Çıkarım (İ2):** S'li ifadenin tipi tek cevaplıdır: aynı yerdeki Trit
   sinyalininki. Literalin tipi tek yerden, tip denetiminin tablosundan
   okunur.
3. **Anlam değişikliği (İ3):** önce kabul edilen girdinin tipi ve değeri
   değişir, bir kısmı reddedilir: `S * x`'in tipi i(2N)'den iN'e iner ve A15
   köşesinde değeri değişir (`(-S * x) as i12`, x = i8::MIN: +128 → -128);
   tipsiz `let`'in teli daralır (`let p = S * x` 16 → 8 bit, `let s = S` 32
   → 2 bit); `x[S]`, `x[S +: 2]`, `x[S + 1 : S]` artık E2003; `w = S` (w :
   i8), `S * u`, `S << 1` E2003; `S * 2`, `G<2>` E2011. Gerekçe ADR-0100'ün
   sessiz yanlış istisnası ve bakımcının kararıdır: önceki çıktı aynı
   tasarımda Trit sinyaliyle spec'in değerini, generic sabitle başka bir
   değeri veriyordu. Önce reddedilen girdi kabul edilir: `M<-1>`,
   `M<(-1)>`, `M<0 - 1>`, `M<NEG>`, `P<-1>`; `S == 1` ve `match S` (önce
   E2005). CHANGELOG'da "Davranış değişikliği" maddeleri. Kabul edilen başka
   hiçbir girdinin çıktısı değişmez (Doğrulama).
4. **Niyet tahmini (İ4):** uygulanmaz; derleyici bir yazımı başka birine
   çevirmez.
5. **Metamorfik yeniden yazım:** fark testinin t biçimi bir yeniden
   yazımdır: Trit portunu generic Trit sabitine çevirir; iki biçimin
   eşdeğerliğinin dayanağı bakımcının #468 kararıdır (S aynı yerdeki Trit
   sinyalinin tipini ve değerini alır).
6. **ADR-0139 denetimi.**
   - *Değer kümesi tip denetiminde:* S'nin Trit olduğunu tip denetimi
     bilir; değer kümesi ({-1, 0, +1}) `check_int_lit`'in Trit kuralıyla
     denetlenir. Ayrıştırıcı tipi yorumlamaz, yalnız taşır.
   - *Kodlama:* üretici Trit'in genişliğini ve işaretini tablodan okur
     (`checked_trit_sig`); "Trit'in temsilini bilen yerler" listesi
     büyümez. Seçici (ADR-0062) değişmez.
   - *Tek yer:* `is_trit`'in literal kolu tabloyu okur; sözdizimsel
     tanıma (#467) büyümez.

## Doğrulama

- Önce kırmızı (main = d9ff3513, kaynak değişmeden, yeni testler
  kopyalandı): `generic_trit_const_tests`'in 6 testinin 5'i düşer (S'li
  biçimin tipi literal kuralından, ret yok, `G<2>` kabul, `G<-1>`
  ayrıştırılmaz); geçen 1'i `~S`'nin tek E2003'ünü sabitler.
  `generic_arg_tests`'in 8 testinin 7'si düşer; geçen 1'i gerçek
  karşılaştırma zincirinin E0010'unu sabitler. Değiştirilen
  `trit_index_tests::a_const_generic_trit_index_is_e2003` ve
  `cross_file_visibility_tests::names_the_resolver_sees_keep_their_diagnostics`
  düşer.
  `generic_trit_sim_tests::the_design_checks_without_a_diagnostic` E0010
  ile düşer. İki `tests/real_bugs` dosyası ve 167 E0010 alır; 468'in
  `Weighted<1>` kısmı main'in `volt test`'inde iki W2010 verir ve
  köşede 128 ≠ 3968 ile düşer. Sonra hepsi geçer.
- Değerler Verilator'da (Docker, Verilator 5.054): iki gerçek hata
  dosyası `2 passed`; `generic_trit_sim_tests`'in tasarımı (21 biçim ve
  register, `F<1>`, `F<0>`, `F<-1>`, A15 köşesi dahil dört vektör) her S'li
  çıkışın t'li çıkışa eşit olduğunu `4 passed` ile gösterir.
- Çıktı karşılaştırması (ADR-0147'nin yöntemi): main ile dalın ikilileri
  gölgenin girdisinde (`examples/` dahil 503 dosya), `tests/ui/fail`'de
  (332 dosya) ve gölgenin dışındaki 109 `volt` dosyasında
  (`tests/suggestions`, `tests/fuzz_regressions`, `tests/quickfix`) `volt
  build --single-file --emit=sva,sdc` ve `volt check`: 944 dosyanın 941'inde
  çıkış kodları, üretilen dosyalar (sürüm satırı dışında) ve tanı kodları
  aynı. Üç fark generic Trit sabiti kullanan dosyalardır: iki yeni
  `tests/real_bugs` dosyası ve 167 (main'de `Scaled<-1>` E0010). 167'nin
  main'deki hâli iki ikiliyle tanısız derlenir; üretilen SV'de yalnız
  `Scaled_1` değişir (`p` 16 bitlik `16'sd1 * 16'(x)` yerine 8 bitlik
  seçici). `examples/` ve kitap etkilenmez: kitabın 31 `volt` bloğu iki
  ikiliyle aynı sonucu verir.
- Fark testinin t biçimi yerelde Verilator'sız denendi: CI tohumunun sekiz
  toplu modülü (geçerli ifadeler, 289 t biçimi) ve regresyon modülü `volt
  check`'ten üretici doğrulamasıyla tanısız geçer; üretilen SV Docker'da
  Verilator 5.054 `--lint-only -Wall` ile WIDTH uyarısı vermez (t
  biçiminde tek uyarı, S = 0 örneğinde sabitlenen bir karşılaştırmanın
  UNSIGNED'ı; fark testi CMPCONST ve UNSIGNED'ı yumuşak sayar). Değerleri
  CI'ın fark testi ölçer.

## Sınırlar

- **Takma ad:** `type W = Trit` ile `const S : W` literal kalır (şablonun
  tip yolları çözülmez): #468'in sessiz yanlışı bu biçimde sürer (dalda
  ölçüldü: `M<1>`, z : i2 = -2 için `(-S * z) as i12` +2, `x[S]` tanısız).
  Çözücü monomorfun parametre tiplerini çözdüğünde kural kendiliğinden
  uygulanır; bakımcıya issue metni önerildi.
- **Desen konumu:** `match x { S => … }`'deki S literal desenidir; desenin
  tipi sınananınkidir, bu ADR onu değiştirmez.
- **Yerleşik primitifler** (`Fifo<DEPTH>`) argümanı tip denetiminde yine
  literal ister (E2008; tek-yerde-bilgi envanterinin 4. satırı): bu ADR
  yalnız kullanıcı modüllerinin monomorfizasyonunu değiştirir.
- **`G<2>`** S kullanılmıyorsa tanı almaz: değer kümesini tip denetimi
  S'nin kullanıldığı yerde denetler. Kullanılmayan bir sabitin değeri
  donanıma girmez.
- **Trit dışı generic tipler** (`const S : i8` ile `x[S]`): literal kalır,
  ADR-0147 Sınırlar'daki gibi. Onların bildirilen tipi de yan tabloda;
  tiplemelerini değiştirmek ayrı bir karardır.

## Reddedilenler

- **Üreticide tanımak** (literalin S'den geldiğini `is_trit`'e
  söylemek). Trit'i iki yerde tanımak olurdu (ADR-0137) ve sözdizimsel
  tanımanın riskini (#467) büyütürdü.
- **S'yi modül düzeyinde bir `const` öğesine çevirmek.** Üretici adlı
  Trit sabitini bildirilen tipinden tanır; ama ad çözümü, SV'de yeni bir
  ad ve monomorf başına bir kök öğe gerektirirdi; tanıma yine üreticide
  kalırdı.
- **Literalin tipini bir sonekle taşımak** (yazılamayan bir `Trit`
  soneki). AST'de kaynağın yazamadığı bir biçim olurdu ve üretici Trit'in
  genişliğini sonekten yazardı (ADR-0139).
- **Bütün generic const tiplerini bildirilen tipte tiplemek.** Kapsam
  dışıdır (bakımcının kararı); `const W : u32`'nin literal tiplemesi
  değişmez.
- **Negatif argüman için `-` dışı bir yazım** (`M<neg 1>`). Gramer
  argümanı zaten bir ifade sayar; yeni yazım gerekmez.
