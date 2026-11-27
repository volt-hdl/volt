# ADR-0141: `prev()`'in Genişliği ve İşareti Tip Denetiminden — Üreticiye Tip Denetiminin Cevabı, Yardımcı Register `x`'in Tipinde

> Statü: Uygulandı — 2026-11-26
> İlgili: ADR-0040 (`prev(x: T) -> T`; yardımcı register zincirinin bildirimi `x`'in tipini alır), ADR-0136 (Aşama 2'nin ilk adımı: üretici bir bilgiyi tip denetiminden okur; kanal `volt-hir` bağımlılığı eklemez), ADR-0137 (§1'in kuralı `prev()`'in genişliğine ve işaretine uygulanır), ADR-0062 (Karar 4'ün "üretici HIR tiplerini görmez" gerekçesi `prev()` için kalkar; çarpansız `Trit * x` eşlemesi değişmez), ADR-0139 (genişlik ve işaret kodlama bilgisidir; değer kümesi ayrı kalır), ADR-0136 §3 (IR kararı bu ADR'de verilmez), ADR-0145 (üretici tablodan Trit aritmetiğinin genişliğini de okur: `-t`, Trit çarpımı, `t ± u`; "yalnız `prev()`" sınırı genişler, E2005 yolu aynı)
> Issue: #413
> Tarih: 2026-11-26
> Etkilenen: `crates/volt-ast/src/checked.rs` (YENİ, `CheckedTypes`), `crates/volt-hir/src/ty.rs` (`TypeArena::is_signed`, `struct_fields`), `crates/volt-hir/src/typeck/checked.rs` (YENİ, `TypeckResult::checked_types`), `crates/volt-hir/src/typeck/structs.rs`, `crates/volt-sv-emit/src/past.rs`, `crates/volt-sv-emit/src/expr.rs`, `crates/volt-sv-emit/src/lib.rs` (`emit_unit`), `crates/volt-sv-emit/src/structs/` (`StructNotes::prev_leaves`), `crates/volt-sv-emit/src/width_shadow.rs`, `crates/volt-driver/src/main.rs`, `crates/volt-driver/tests/width_shadow_tests.rs`, `docs/design/tek-yerde-bilgi-envanteri.md`

## Sorun

`prev(x)`'in tipi `x`'in tipidir (ADR-0040 §1). `volt test` ve `volt
verify` `prev()`'i bir yardımcı register zinciriyle üretir (ADR-0040 §4,
ADR-0064); zincirin bildirimi `x`'in tipinde olmalıdır. Üretici bunu tip
denetiminden değil kendi hesabından alıyordu (`past.rs`,
`self.width_of(x).map(|s| s.width).unwrap_or(1)`) ve işareti hiç
yazmıyordu (`logic [W-1:0]`):

- işaretli `x` (`iN`, işaretli struct alanı, `Trit`) işaretsiz bildirildi:
  `prev(sa) < sb` -1'i 255 olarak karşılaştırdı, `prev(sa) as i16`
  sıfırla genişletti;
- aritmetik `x` işlem genişliğinde bildirildi: `u8 + u8`'in doğal
  genişliği 9 bittir (ADR-0025), register 8 bitti, `prev(a + b) != 300`
  300'ü `8'h2C` yazdı; `--emit=sva` `$past(a + b) != 8'h2C` yazdı (`$past`'in
  argümanı öz-belirlenimli, toplam 8 bitte);
- genişliği bulunamayan `x` bir bit sayıldı (`unwrap_or(1)`); bugünkü
  girdilerde bu yola düşen her biçim başka bir tanı da alıyordu.

Tanı yoktu: kontrat başka bir değeri sınadı (sessiz yanlış, #413). Gölge
(ADR-0136 Aşama 1) bunu görmedi, çünkü yalnız RTL kipinde koşuyordu ve
yardımcı register'lar o kipte üretilmez.

Tip denetimi doğru cevabı zaten verir: `TypeArena::bit_width` (#419, tek
genişlik fonksiyonu) esnek tipte doğal genişliği verir; işaret tipin
kendisindedir. Üretici o cevaba ulaşamaz: `volt-sv-emit` `volt-hir`'e bağlı
değildir (ADR-0062 Karar 4, ADR-0136 "Reddedilenler").

## Karar

1. **İşaretin tek fonksiyonu.** `TypeArena::is_signed`: `iN`, işaretli
   esnek aralık ve `Trit` işaretli; `Delayed<T, N>` `T`'ninki; öteki her
   tip işaretsiz (`bits`, `bool`, enum, struct ve dizi paketlenmiş
   vektördür). `bit_width`'in eşidir.
2. **Tip denetiminin cevabı üreticiye bir tabloyla gelir.**
   `volt_ast::checked::CheckedTypes` kök AST'nin her ifadesi için
   (indeks ve konum) genişlik ve işareti ya da struct değerinin alanlarını
   taşır. Tek doldurucu `TypeckResult::checked_types`'tır (genişlik
   `bit_width`, işaret `is_signed`, struct alanları tip denetiminin
   `struct_field_type`'ıyla aynı çözümden). Sürücü tabloyu
   `volt_sv_emit::emit_unit`'e verir. Üretici `volt-hir`'e bağlanmaz;
   tablo `volt-ast`'tedir, çünkü iki taraf da ona bağlıdır. Tablo hesap
   yapmaz.
3. **`prev(x)`'in değeri tablodan okunur, tek yerde.**
   `Emitter::prev_sig` üç kullanımı besler: yardımcı register'ın bildirimi
   ve kaynağı (`past_reg_block`), karşılaştırmanın `prev()`'e verdiği
   genişlik (`width_of`'un `prev()` kolu: literalin boyutu, işaretli
   karşılaştırma) ve `--emit=sva`'da bağlamsız `$past`'in argümanının
   genişliği. Struct indirgemesinin (ADR-0077) yeni yaprak düğümleri
   kökenlerini (`StructNotes::prev_leaves`: özgün `prev()` çağrısı ve alan
   yolu) yazar; yaprağın tipi kökteki argümanın tipinin o alanıdır.
4. **Bildirim biçimi** (sv-mapping.md §15'in genişlemesi): işaretsiz tip
   `logic [W-1:0] past_<x>_<k>;` (1 bit `logic`), işaretli tip
   `logic signed [W-1:0] past_<x>_<k>;`. Kaynak register'ın genişliğinde
   yazılır (`past_e1_1 <= 9'(a) + 9'(b);`). `--emit=sva`'da bağlam
   yoksa `$past(9'(a) + 9'(b))`; basit bir sinyalin metni değişmez.
5. **Genişlik yoksa varsayılan yok.** Tip denetimi argümana genişlik
   vermiyorsa (boyutsuz literal `prev(5)`, demet) yardımcı register
   bildirilmez ve E2005 argümanı gösterir: "cannot determine the width of
   the prev() argument". Yeni tanı kodu yoktur (ADR-0100). Tablo olmadan
   (`emit_full`, tip denetimi koşmamış birim) de aynısı olur: üretici
   kendi genişliğine dönmez.
6. **Gölge kararı okur.** Genişlik gölgesi (`width_shadow_tests`)
   `volt test` ve `volt verify` kiplerinde her yardımcı register'ın
   bildirildiği genişliği ve işareti (`past_reg_block`'ta kaydedilir) ve
   `width_of`'un `prev()`'e verdiğini, `bit_width` ve `is_signed` ile
   karşılaştırır; (a), (b) ve kökeni yazılmamış kopya düğüm sıfırdır.

`prev(x, N)`'in derinliği değişmez: `N` isim çözümlemede pozitif bir
tamsayı literali olmalıdır (ADR-0040 §2, E5017); generic parametre
monomorflamada literaldir. Üreticinin `prev_depth`'indeki `unwrap_or(1)`'e
derlenen bir tasarım ulaşmaz.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; kullanıcıdan yeni bir yazım istenmez.
2. **Çıkarım (İ2):** çıkarımın kendisi değişmez (`prev(x: T) -> T`,
   ADR-0040; doğal genişlik, ADR-0025). Değişen, `prev()`'in genişliğinin
   ve işaretinin iki hesaptan bire inmesidir: İ2'nin "tek cevaplı" koşulu
   üreticide de sağlanır.
3. **Anlam değişikliği (İ3):** dilin anlamı değişmez; üretilen donanım
   spec'in söylediği değeri hesaplar. Bu bir sessiz yanlış düzeltmesidir
   (ADR-0100): `volt test`'te ve `volt verify`'da işaretli bir `prev()`'in
   sıralı karşılaştırması ve aritmetik `prev()`'in taşması değişir;
   CHANGELOG'da iki ayrı "Davranış değişikliği" maddesi. Önce kabul edilen
   ve şimdi reddedilen girdi yoktur (gölgenin girdisinde tanılar aynı).
4. **Niyet tahmini (İ4):** kaldırılır: genişliği bilinmeyen değer bir bit
   sayılmaz, E2005 verir.
5. **Metamorfik yeniden yazım:** uygulanmaz.
6. **ADR-0139:** yeni bir ikili varsayım eklenmez; Trit'in işareti
   `bit_width`'in yanındaki tek fonksiyondadır.

## Doğrulama

- Kırmızı, sonra yeşil: `tests/real_bugs/413_prev_width_and_sign.volt`'ün
  dört testi main'in (dccaeda4) ikilisiyle `volt test`'te (Verilator
  5.054) 0 geçti, 4 düştü (her biri tanısız bir kontrat ihlali); bu dalda
  4 geçti. `prev_width_sim_tests` iki tasarımda tutan ve ihlal edilen
  kontratın yerini main'de tersine çevirir.
- Gölge: eski mantıkla (yardımcı `width_of(x)`'ten, işaretsiz) (a) ve
  kökeni yazılmamış kopya düğüm sıfırdan büyük (sayılar PR açıklamasında);
  bu dalda (a) 0, (b) 0, eşleşmeyen 0.
- Çıktı karşılaştırması (ADR-0136'nın yöntemi): main ile dalın ikilileri
  gölgenin girdisinde `volt build --single-file
  --emit=sva,sdc,xdc,rust,c,regmap` ile; üretici dört kipte (RTL,
  `--emit=sva`, `volt test`, `volt verify`) bütün girdi birimlerinde. Fark
  yalnız `prev()` yardımcı register'larının bildirimindedir (örneklerde
  `logic [7:0]` → `logic signed [7:0]`) ve #413'ün yeni girdilerindedir
  (sayılar PR açıklamasında).

## Sınırlar

- Üretici tip denetiminden yalnız `prev()`'in değerini okur. Öteki
  ifadelerin genişliği ve işareti `width_of`'tan gelmeye devam eder
  (ADR-0136 Aşama 2'nin geri kalanı); tablo onlar için de doludur ama
  okunmaz.
- Tablo kök AST'nin ifadelerini taşır. Fn açılımının (ADR-0081) ve struct
  indirgemesinin yeni düğümleri tabloda yoktur; `prev()` için struct
  yaprağının kökeni yazılır, fn açılımı `prev()` üretmez (`prev()` fn
  gövdesinde E5017).
- `volt check` ve LSP üreticiyi kontratsız kipte (RTL) çalıştırır; tablo
  onlara verilmez, `prev()` sorulmaz.
- IR kararı verilmez. `CheckedTypes`, ADR-0136 §3'teki ara temsil
  değildir; o karar ayrı bir ADR'dir. Tablo, o karar verilene kadar tip
  denetiminin cevabını üreticiye taşıyan dar bir kanaldır; IR geldiğinde
  onun içine taşınır ya da kaldırılır.
- Tablonun taşıdığı genişlik ve işaret kodlama bilgisidir (ADR-0139).
  Trit'in işaretli 2 biti değer kümesi (−1/0/+1) değil, bugünkü SV
  kodlamasıdır. `is_signed`'ın ve `bit_width`'in Trit kolları kodlama
  bilgisinin yeridir ve ikili varsayımlar envanterinde kayıtlıdır (#436).

## Reddedilenler

- **Üreticiye `volt-hir` bağımlılığı eklemek** (`TypeckResult`'ı doğrudan
  okumak): ADR-0136 Aşama 3'ün hedefi arka uçların AST'den ve tip
  denetiminin iç yapısından ayrılmasıdır; tablo yalnız arka ucun okuduğu
  cevabı taşır.
- **`width_of(x)`'i düzeltip işareti oradan almak:** genişliğin ikinci
  hesabı kalırdı (ADR-0137 §1); #215 ve #419 bu biçimde ayrıştı.
- **Genişliği bilinmeyen değeri 1 bit saymak ya da sessizce `$past`'e
  düşmek:** sessiz yanlış; E2005 yeni kod gerektirmez.
- **Yalnız `prev()` argümanlarını tabloya yazmak:** struct indirgemesi
  `prev(p).lo`'yu yerinde `prev(p_lo)`'ya çevirir; argümanın kökteki
  karşılığı yoktur, kökeni ve kökteki struct argümanının tipi gerekir.
  Bütün ifadeler yazılır; Aşama 2'nin sonraki adımları aynı tabloyu
  okuyabilir.
