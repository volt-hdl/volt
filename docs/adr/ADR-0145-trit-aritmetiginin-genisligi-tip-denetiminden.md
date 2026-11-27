# ADR-0145: Trit Aritmetiğinin Genişliği Tip Denetiminden — `-t`, Trit Çarpımı ve `t ± u` Tablodan Okunur

> Statü: Uygulandı — 2026-11-27
> İlgili: ADR-0141 (tablo ve E2005 yolu; "üretici yalnız `prev()`'i okur" sınırı Trit aritmetiğine genişler), ADR-0136 (Aşama 2'nin bir adımı: üretici bir bilgiyi tip denetiminden okur), ADR-0137 (§1'in kuralı Trit aritmetiğinin genişliğine uygulanır), ADR-0139 (Trit'in genişliği kodlama bilgisidir; üretici onu yeniden yazmaz, tablodan okur), ADR-0062 (çarpansız seçici değişmez; Karar 4'ün "üretici HIR tiplerini görmez" gerekçesi Trit aritmetiği için de kalkar), ADR-0144 (§5'in hatası; `VOLT_DIFFTEST_TRIT_ARITH` kapısı kalkar; §3'ün A15 satırını Karar 8 kapatır), ADR-0106 (§6'nın yalıtma yolu Trit çarpımını da yalıtır), ADR-0041 (en üstteki çarpımın itmesi değişmez), ADR-0120 (A12 gibi A15'in okuması da kural olur)
> Issue: #463
> Tarih: 2026-11-27
> Etkilenen: `crates/volt-sv-emit/src/let_width.rs` (`int_range`, `is_trit_arith`, `checked_trit_range`, `checked_value`; `binary_range`'in Trit kolu kaldırıldı), `crates/volt-sv-emit/src/trit.rs` (`is_trit`: literal çarpanı, süreç yereli; `trit_product_int_side`), `crates/volt-sv-emit/src/expr.rs` (`width_dependent`: Trit çarpımı), `crates/volt-sv-emit/src/match_expr.rs` (`ProcLocal::trit`), `crates/volt-sv-emit/src/inline/{mod,expand}.rs` (`InlineNotes::origins`), `crates/volt-sv-emit/src/lib.rs` (`validate_unit` ve fn doğrulaması tabloyu alır), `crates/volt-lsp/src/analysis.rs`, `crates/volt-difftest/src/{gen,regress,run,harness,model}.rs` (regresyonun sabit geniş hedefi; A15'in metni), `crates/volt-difftest/tests/coverage_tests.rs`, `crates/volt-sv-emit/tests/{trit_emit_tests,wave_emit_tests,emit_tests}.rs` (sürücünün yolu, `tests/typed`), `crates/volt-driver/tests/trit_width_sim_tests.rs` (YENİ), `crates/volt-lsp/tests/lsp.rs`, `tests/real_bugs/463_trit_product_width.volt` (YENİ), `tests/ui/pass/167_trit_recognition_shapes.volt` (YENİ, gölgenin girdisi; ui/pass sayımı 152: `ui_pass_build_tests`, `ui_semantic_tests`, `parser_tests`), `docs/design/tek-yerde-bilgi-envanteri.md`, `docs/design/ikili-varsayimlar-envanteri.md`, `CHANGELOG.md`

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
6. **Trit'in tanınması tip kuralına yaklaşır.** Okumanın kapısı
   `is_trit` olduğundan, onun tanımadığı Trit değeri eski hesaba
   düşüyordu. Tek tanıyıcı iki yerde genişler: soneksiz literal çarpanı
   Trit'in tipini alır (§4; `t * 1`, `(-1) * t` Trit'tir, önce
   `-(t * 1 * u) * -(1 * v * w) == 1` E2005 alıyordu) ve süreç yereli
   (`comb`/`on` bloğunun `let`'i) modül `let`'iyle aynı kuralla Trit
   sayılır (`let t : Trit` ya da Trit değerli tipsiz yerel; önce
   `comb { let t : Trit = ...  let s = -(t * x) }` 8 bitte kalıyordu).
   İkisi de bu PR'ın ilk CI koşusundaki fark testinde görüldü.
7. **Fark testinin kapısı kalkar.** `VOLT_DIFFTEST_TRIT_ARITH` silinir:
   üreteç `-t`'yi ve Trit çarpımını her koşuda üretir (ADR-0144 §5'in
   "issue kapanınca varsayılan açılır" adımı). Kapatma bayrağı
   bırakılmaz: hatası düzeltilmiş bir biçimi gizlemenin yolu olurdu.
   `regress.rs` #463'ün iki sessiz yanlışını elle hesaplanmış değerlerle
   sabitler.
8. **A15'in okuması kuraldır** (bakımcı kararı, 2026-11-27): "Trit × iN
   çarpımının (t * x, x * t; etkin Trit'i -t olanlar dahil) tipi iN'dir.
   Çarpım bir işlemin işleneni olduğunda (as, + ve - işleneni, bit
   işlemi, karşılaştırma, seçim) iN içinde ikiye tümleyenle sarar: t = -1,
   x = iN::MIN için sonuç iN::MIN'dir. En üstte geniş bir hedefe
   yazıldığında, öteki aritmetik gibi ADR-0041 ile hedefe itilir ve hedef
   genişliğinde hesaplanır; ADR-0062 Karar 2 değişmez. Örnek (x : i8 =
   -128, t = -1): let y: i12 = t * x → +128; let y: i12 = (t * x) as i12
   → -128." ADR-0144 §3'ün A15 satırını bu karar kapatır; #122'nin bu
   köşesi kapanır, `t * x`'in tipinin `-x` gibi i(N+1) olup olmaması v0.2'ye
   kalır.

   Üretici çarpımı ADR-0106 §6'nın yoluyla yalıtır: `width_dependent`
   Trit çarpımını (`Trit × iN`, değeri Trit olmayan) bağlama bağlı sayar,
   `isolate_if_dependent` onu doğal genişliğinde basar ve
   `W'($signed(...))` ile sarar. Yeni bir hesap yoktur. Doğal genişlik
   tablodan, çarpımın iN yanının değerinden okunur (`trit_product_int_side`,
   `checked_trit_range`): çarpım itilen bir aritmetiğin işleneniyse tip
   denetimi çarpıma hedef tipi kaydeder (`widen_operands`, ADR-0041); §3.3
   çarpımın tipini iN yanınınki sayar. Karar 1'in ilk hâli çarpımın kendi
   kaydını okuyordu ve itilen `+`/`-`'nin işleneni olan çarpımı yalıtmayı
   bırakmıştı (main'de iki yanı farklı genişlikteki çarpım yalıtılıyordu);
   aşağıdaki büyük koşunun i13, i33, i37 ve i53 bulguları bu yüzdendi. i2
   çarpımı (Trit ile i2'nin genişliği aynı) main'de de yalıtılmıyordu. En
   üstte geniş hedefe yazılan çarpım (`emit_assigned`'ın itmesi) yalıtılmaz.
   Karşılaştırmanın iki somut yanı aynı genişlikte olmalıdır (E2031):
   karşılaştırma çarpımı genişletmez. Modelin hesabı değişmez; A15 etiketi
   A12 gibi (ADR-0120) raporda kalır, açıklaması kuralı söyler.
   `regress.rs` sınırın iki yanını (0x080 ve 0xF80) ve büyük koşunun sekiz
   bulgusunun en küçüklerini elle hesaplanmış değerlerle sabitler; en üstteki
   geniş hedefin değeri için regresyon örneği w biçiminin hedefini sabit
   tutar (`Regression::pin_wide`).

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; kullanıcıdan yeni bir yazım istenmez.
2. **Çıkarım (İ2):** çıkarım değişmez (§3.3, §3.4). Trit aritmetiğinin
   genişliği iki hesaptan bire iner; "tek cevaplı" koşul üreticide de
   sağlanır.
3. **Anlam değişikliği (İ3):** dilin anlamı değişmez; üretilen donanım
   spec'in söylediği değeri hesaplar. Sessiz yanlış düzeltmesidir
   (ADR-0100): Trit çarpımını tipsiz `let`'te tutan ya da eksisini alan
   tasarımların değeri değişir (CHANGELOG'da "Davranış değişikliği").
   A15 köşesindeki değer de değişir (sessiz yanlış düzeltmesi, Karar 8):
   Trit -1 ile iN::MIN'in çarpımını bir işlemin işleneni olarak kullanan
   tasarımda çarpım artık iN::MIN'dir, +2^(N-1) değil (ayrı madde). En
   üstte geniş hedefe yazılan çarpımın değeri değişmez.
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
- Fark testi: kapı kalktıktan sonra PR'ın ilk CI koşusunda küçük koşu
  (tohum 20261104, 2000 ifade) `diffs=39` verdi: 13 ifade literalle
  karşılaştırmada E2005 aldı (hepsinde Trit bir literalle çarpılıyordu)
  ve #463'ün `(-(t * y))[17:1]` örneği R biçiminde (argüman çağıranın
  `comb` yerelinden geçer) 0xffff verdi; kalan 25'i Verilator `-Wall`
  uyarısıydı (soft). Karar 6 ikisini kapatır: on üç ifadenin ve R
  örneğinin kopyaları dalda tanısız derlenir, R örneği ve bir literal
  karşılaştırması Verilator'da elle hesaplanan değeri verir (main'de
  65535 ve E2005). Dalda ilk commit'le elle tetiklenen büyük koşu
  (45976548997, tohum 2026112600 + k, parça başına 12 500; #462'nin
  45942236536 koşusuyla aynı) `diffs=217, 224, 200, 241` verdi (#462'de
  933, 945, 987, 987): 114 değer farkı, 5 sözleşme düşmesi, 300 E2005,
  463 WIDTH. WIDTH'lerin hepsi R biçimindeydi (Trit çağıranın `comb`
  yerelinden geçer), E2005'lerin hepsinde Trit bir literalle çarpılıyordu;
  ikisi Karar 6'nın yollarıdır. Kalan değer farklarının altı ifadesi
  yalnız A15'in köşesindedir (t = -1 ya da -t = -1, x = iN::MIN; çarpım
  geniş bir bağlamda sarmadan hesaplanır, model doğal genişlikte sarar):
  ADR-0144 §3'ün okumasıdır. Karar 6'dan sonraki büyük koşu (45989294363,
  aynı tohum ve sayı, d94732f7) `diffs=14, 15, 12, 8` verdi: 45 değer
  farkı, 4 sözleşme düşmesi, 0 E2005, 0 WIDTH. Hepsi A15'in köşesindeki
  sekiz ifadeydi (dördünün sözleşme biçimiyle 12 bulgu); Karar 8 onları
  kapatır.
- Karar 8, önce kırmızı: regresyon örneklerinin aynısı ve real-bug
  dosyasının yeni blokları `volt test` (Verilator 5.054, Docker) ile.
  Main'in (c4f24632) ikilisiyle i2 örnekleri düştü (`(t * x2) as i12`,
  `((t * x2 | 1) as i55)[41:7] as u35`, `z - (t * u) * x2`); `-t`'li
  örnekler main'de #463'ün WIDTH uyarısında durdu. Karar 8'den önceki dalın
  ikilisiyle i2 örnekleri ve `+`/`-` işleneni olan i13, i33, i37, i53
  örnekleri düştü (`-t * x13 + y` 0x1000). En üstte geniş hedefin 0x080'i
  iki ikilide de yeşildi; i8 çarpımının dönüşümü (0xF80) ve seçimi de
  (iki yanı farklı genişlikte, yalıtılıyordu). Karar 8'le hepsi geçer.
- Gölgenin girdisi `tests/ui/pass/167_trit_recognition_shapes.volt`:
  Trit struct alanında, dizi elemanında, fn parametresinde, generic
  const'ta, `comb` ve `on` yerelinde; her birinde Trit çarpımı ve `-t`
  tipsiz `let`'te. Gölge ayrışma bulmadı (yalnız beklenen C sınıfı:
  bağlamdan tip alan literal); bütün biçimler seçiciye iner (struct alanı
  indirgemede `w_t` adına dönüşür). Generic `const S : Trit` ayrıştırıcıda
  literale döner (ADR-0041): `S * x` literal çarpımıdır, `-(S * x)`
  i17'dir; bu tip denetiminin davranışıdır, üreticinin değil.

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
  değeri Trit ile i2'yi ayırmaz. Tanınmayan bir Trit ifadesi eski kurala
  düşer: bu bilinen bir sessiz yanlış riskidir; Karar 3'ün istisnasıdır.
  Gölgenin girdisi (`tests/ui/pass/167_trit_recognition_shapes.volt`)
  bilinen yerleri sınar; kalıcı çözüm, tablonun ya da IR'nin değer
  kümesini taşımasıdır (ADR-0139, ADR-0136 §3).
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
