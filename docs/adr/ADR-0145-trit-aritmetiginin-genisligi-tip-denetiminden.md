# ADR-0145: Trit Aritmetiğinin Genişliği Tip Denetiminden — `-t`, Trit Çarpımı ve `t ± u` Tablodan Okunur

> Statü: Uygulandı — 2026-11-27
> İlgili: ADR-0141 (tablo ve E2005 yolu; "üretici yalnız `prev()`'i okur" sınırı Trit aritmetiğine genişler), ADR-0136 (Aşama 2'nin bir adımı: üretici bir bilgiyi tip denetiminden okur), ADR-0137 (§1'in kuralı Trit aritmetiğinin genişliğine uygulanır), ADR-0139 (Trit'in genişliği kodlama bilgisidir; üretici onu yeniden yazmaz, tablodan okur), ADR-0062 (çarpansız seçici değişmez; Karar 4'ün "üretici HIR tiplerini görmez" gerekçesi Trit aritmetiği için de kalkar), ADR-0144 (§5'in hatası; `VOLT_DIFFTEST_TRIT_ARITH` kapısı kalkar)
> Issue: #463
> Tarih: 2026-11-27
> Etkilenen: `crates/volt-sv-emit/src/let_width.rs` (`int_range`, `is_trit_arith`, `checked_trit_range`, `checked_value`; `binary_range`'in Trit kolu kaldırıldı), `crates/volt-sv-emit/src/inline/{mod,expand}.rs` (`InlineNotes::origins`), `crates/volt-sv-emit/src/lib.rs` (`validate_unit` ve fn doğrulaması tabloyu alır), `crates/volt-lsp/src/analysis.rs`, `crates/volt-difftest/src/{gen,regress}.rs`, `crates/volt-difftest/tests/coverage_tests.rs`, `crates/volt-sv-emit/tests/{trit_emit_tests,wave_emit_tests,emit_tests}.rs` (sürücünün yolu, `tests/typed`), `crates/volt-driver/tests/trit_width_sim_tests.rs` (YENİ), `crates/volt-lsp/tests/lsp.rs`, `tests/real_bugs/463_trit_product_width.volt` (YENİ), `docs/design/tek-yerde-bilgi-envanteri.md`, `docs/design/ikili-varsayimlar-envanteri.md`, `CHANGELOG.md`

## Sorun

SV üreticisi bir ifadenin doğal genişlik aralığını (ADR-0025) kendisi
hesaplar (`let_width.rs`, `int_range`/`binary_range`). Tipsiz `let`'in
teli, karşılaştırmada literalin boyutu, tekli eksinin ve dönüşümün
işlenen bağlamı bu aralıktan gelir. Aralık Trit'i iki bitlik bir tamsayı
sayıyordu (ADR-0144 §5, #463):

- `-t` i3 sayılıyordu (doğrusu Trit, type-inference.md §3.4);
- `Trit * iN` (N ≠ 2) aralık almıyordu: Trit'in [2, 2] aralığı iN'inkiyle
  kesişmiyordu (doğrusu iN, §3.3);
- `Trit * Trit` [2, 4] alıyordu (doğrusu Trit, §3.3).

Dört sonucu vardı. Tipsiz `let s = t * x * u` (x : i14) `wire signed
[3:0]` bildiriliyor, değer kesiliyordu (sessiz yanlış). `-(t * y)`
(y : i17) 17 bitte alınıyordu, 18 değil (sessiz yanlış). `let n = -t` 3
bitlik teldi; okuyan 2 bitlik kodla karşılaştırınca Verilator WIDTH
uyarısı verdi ve `volt test` durdu. Trit çarpımının literalle
karşılaştırması (`t * x == 1`) geçerli bir tasarımda E2005 aldı.

Tip denetimi doğru cevabı bilir: `trit_arith_result` ve `synth_unary`
(§3.3, §3.4) ve ADR-0141'in `CheckedTypes` tablosu her kök ifadenin
genişliğini ve işaretini taşır. Üretici tabloyu yalnız `prev()` için
okuyordu (ADR-0141 "Sınırlar"). Genişlik gölgesi (ADR-0136) iki sessiz
yanlışı yakaladı: `s` üreticide 4 bit, tip denetiminde i14; `-(t * y)`
üreticide 17 bit, tip denetiminde i18.

## Karar

1. **Trit aritmetiğinin aralığı tablodan okunur.** `int_range` bir
   ifadeyi Trit aritmetiği olarak tanırsa (`is_trit_arith`) aralığı
   hesaplamaz; tip denetiminin ifadeye verdiği genişliği ve işareti
   tablodan okur (`checked_trit_range`). Trit aritmetiği Trit'in eksisi
   (`-t`) ve en az bir yanı Trit olan çarpım, toplam ve farktır (`t * u`,
   `t * x`, `x * t`, `t ± u`). Trit ile Trit olmayan bir değişkenin
   toplamı tip denetiminde hatadır; literal Trit'in tipini alır (`t + 1`
   i3'tür, §4). Trit'i tanıyan, çarpımın seçici olarak basılıp
   basılmayacağına da karar veren `is_trit`'tir (trit.rs, ADR-0062);
   seçici ile genişlik aynı tanımaya bağlı kalır. `binary_range`'in Trit kolu (`trit_sum_sig` çağrısı)
   kalkar: `let_width.rs`'te Trit için ikinci bir kural kalmaz.
2. **Okumanın kapsamı.** `int_range`'i okuyan her yol cevabı alır:
   tipsiz `let`'in teli (modül `let`'i ve süreç yereli,
   `untyped_let_sig`), işlemin bağlamı (`natural_sig`, `operation_sig`),
   karşılaştırmanın genişliği (`comparison_ctx`). #463'ün dört sonucu bu
   yollardadır. Trit aritmetiği olmayan ifadeler kendi hesabıyla kalır;
   `-x` (x : iN) yine `i(N+1)` kuralıyla, işleneninin aralığı Trit
   çarpımıysa o aralık tablodan gelir (`-(t * y)` → i18). `width_of`
   (kaba genişlik, `expr.rs`) değişmez.
3. **Cevap yoksa E2005.** Tablo ifade için `Bits` vermiyorsa (tablo yok,
   ifade kök AST'de değil, tip denetimi genişlik vermedi) üretici kendi
   hesabına dönmez: ifadenin konumunda E2005, "cannot determine the width
   of this Trit operation" (ADR-0141 §5'in yolu). Yeni tanı kodu yoktur
   (ADR-0100). Tablosuz giriş noktaları (`emit`, `emit_full`) Trit
   aritmetiğinde E2005 verir; sürücü her kipte tabloyu verir.
4. **Tablo bütün üretim yollarına gider.** `validate_unit` (LSP'nin emit
   doğrulaması, ADR-0070) tabloyu parametre olarak alır; LSP tip
   denetiminin cevabını `volt check` gibi verir. fn gövdelerinin
   tanımdaki doğrulaması (ADR-0081 Karar 12.4) kök AST'nin kopyası
   üzerindedir, gövde ifadeleri aynı indekste ve konumdadır: tabloyu okur.
5. **fn açılımının kopyaları kökenlerini taşır.** Çağrı yerinde açılan
   gövde (ADR-0081) yeni düğümlerdir ve tabloda yoktur.
   `InlineNotes::origins` her kopyayı gövdedeki özgün düğüme bağlar;
   kopyanın değeri özgün düğümünkidir, konumu da denetlenir
   (`checked_value`). fn generic değildir (generic fn E0003, ADR-0081
   Karar 7): gövde parametrelerin bildirilen tipleriyle bir kez
   denetlenir, cevap her çağrıda aynıdır. Struct indirgemesi (ADR-0077)
   kök ifadeleri aynı indekste tutar; Trit çarpımının düğümü kopyada da
   kendi değerini okur.
6. **Fark testinin kapısı kalkar.** `VOLT_DIFFTEST_TRIT_ARITH` silinir:
   üreteç `-t`'yi ve Trit çarpımını her koşuda üretir (ADR-0144 §5'in
   "issue kapanınca varsayılan açılır" adımı). Kapatma bayrağı
   bırakılmaz: hatası düzeltilmiş bir biçimi gizlemenin yolu olurdu.
   `regress.rs` #463'ün iki sessiz yanlışını elle hesaplanmış değerlerle
   sabitler.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; kullanıcıdan yeni bir yazım istenmez.
2. **Çıkarım (İ2):** çıkarım değişmez (§3.3, §3.4). Trit aritmetiğinin
   genişliği iki hesaptan bire iner; "tek cevaplı" koşul üreticide de
   sağlanır.
3. **Anlam değişikliği (İ3):** dilin anlamı değişmez; üretilen donanım
   spec'in söylediği değeri hesaplar. Sessiz yanlış düzeltmesidir
   (ADR-0100): Trit çarpımını tipsiz `let`'te tutan ya da eksisini alan
   tasarımların değeri değişir (CHANGELOG'da "Davranış değişikliği").
   Önce reddedilen ve şimdi kabul edilen girdi vardır: Trit çarpımının ya
   da `-t`'nin literalle karşılaştırması E2005 almaz (ayrı madde). Önce
   kabul edilen ve şimdi reddedilen bir tasarım yoktur: tablo sürücünün
   her yolunda vardır (gölgenin girdisinde tanılar aynı, "Doğrulama").
4. **Niyet tahmini (İ4):** kaldırılır: Trit'in genişliği üreticide tahmin
   edilmez.
5. **Metamorfik yeniden yazım:** uygulanmaz.
6. **ADR-0139:** Trit'in genişliği ve işareti kodlama bilgisidir;
   `let_width.rs` onu yazmaz, tablodan okur (`TypeArena::bit_width` ve
   `is_signed`, ADR-0141). `width_of`'un `trit_sum_sig` kolu (`Trit ±
   Trit` i3) ve `sig_of_typeref`'in Trit'i (`bit_width::Scalar`) yerinde
   kalır; envanterlerde satırları güncellendi.

## Doğrulama

- Kırmızı, sonra yeşil: `tests/real_bugs/463_trit_product_width.volt`.
  Main'in (c4f24632) ikilisiyle `volt test` (Verilator 5.054) `wire signed
  [3:0] s` satırında WIDTHEXPAND ve WIDTHTRUNC ile durdu; `width_shadow_tests`
  iki (a) ayrışmasıyla kırmızıydı (`s`: üretici 4, tip denetimi i14;
  `-(t * y)`: üretici 17, tip denetimi i18). Bu dalda iki test geçer,
  gölge sıfır ayrışma verir.
- `crates/volt-driver/tests/trit_width_sim_tests.rs` (Verilator): main'in
  ikilisiyle tasarım E2005 ile derlenmedi; literal karşılaştırması
  çıkarılınca `let n = -t`'nin 3 bitlik teli WIDTHEXPAND/WIDTHTRUNC verdi.
  Bu dalda dört test geçer (fn açılımının tel ve ikame kipleri, bütün
  sağ taraf, `comb` yereli, literal karşılaştırması).
- Çıktı karşılaştırması (ADR-0136'nın yöntemi), main (c4f24632) ile dalın
  ikilileri: gölgenin dosya girdisinin 498 dosyasında `volt build
  --single-file --emit=sva,sdc,xdc,rust,c,regmap` (çıkış kodu, JSON
  tanılar, üretilen her dosya) ve aynı girdi artı kitabın 25 bloğunda
  (523 birim) üretici beş kipte (RTL, `--emit=sva` ayrı ve satır içi,
  `volt test`, `volt verify`), tip denetiminin tablosuyla. Fark tek
  dosyada: `tests/real_bugs/463_trit_product_width.volt` (`s` 4 → 14 bit,
  `-(t * y)` 18 bitte). Trit çarpımı içeren öteki yedi birim (`examples/
  hybrid_accel/` dört dosya, `ui/pass` 16, 82, 166) bayt bayt aynı;
  Trit'siz tasarımlarda fark yok.
- Fark testi: kapı kalktıktan sonra CI'ın küçük koşusu ve dalda elle
  tetiklenen büyük koşu (PR açıklaması).

## Sınırlar

- Okuma yalnız Trit aritmetiğindedir. Öteki ifadelerin aralığı
  `let_width.rs`'in kendi hesabıyla kalır (ADR-0136 Aşama 2'nin geri
  kalanı); tablo onlar için de doludur ama okunmaz.
- Tablo esnek tipin yalnız doğal genişliğini taşır. Esnek işlenenli
  çarpımda (`t * (a + b)`) aralığın alt ucu da doğal genişlik sayılır;
  zincirli `let`'in kesişimi o ifadede tip denetiminden ayrılabilir.
  Biçimin aralığı Trit'te tanımlı değildir (ADR-0144 §2, Unspecified);
  üreteç onu üretmez.
- Trit'i tanıyan `is_trit` sözdizimseldir (ad kümesi, ADR-0062); tablonun
  değeri Trit ile i2'yi ayırmaz. Tanınmayan bir Trit ifadesinde çarpım
  SV `*` olarak basılır ve aralık eski kuralla hesaplanır. Bu PR'da
  bulunan örnek: süreç yereli Trit (`comb { let t : Trit = ... }`) Trit
  sayılmaz; `let s = -(t * x)` (x : i8) 8 bit bildirilir, t = +1, x =
  -128'de değer 128 yerine -128'dir (main'de de aynı; ayrı bir sessiz
  yanlış, issue'su önerildi). Tanıma tip denetimine taşındığında (ara
  temsil, ADR-0136 §3) bu sınır kalkar.
- IR kararı verilmez; `CheckedTypes` ADR-0141'deki dar kanal olarak
  kalır.

## Reddedilenler

- **Yalnız `let_width.rs`'in Trit kollarını düzeltmek** (`-t` → Trit,
  `Trit * iN` → iN, `Trit * Trit` → Trit): kuralın üreticideki ikinci
  kopyası kalırdı (ADR-0137 §1); #463 tam bu biçimde ayrıştı.
- **Bütün tipsiz `let`'lerin telini tablodan okumak:** esnek aralığın alt
  ucu tabloda yok; struct, enum, dizi ve açılım yaprakları kök AST'de
  değil. Kapsam ADR-0136 Aşama 2'nin kendi adımlarıdır.
- **Tablo cevap vermezse eski hesaba düşmek:** sessiz yanlışı geri
  getirirdi; E2005 yeni kod gerektirmez.
- **`VOLT_DIFFTEST_TRIT_ARITH`'in varsayılanını 1 yapmak:** bir kapatma
  yolu bırakırdı; bayrak hatanın sürdüğü süre içindi.
