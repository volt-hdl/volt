# ADR-0136: Arka Uçlar Anlamsal Bilgiyi Tek Yerden Okur — Çözümlenmiş Ara Temsil, Aşamalı Geçiş, Genişlik Gölgesi

> Statü: Önerildi
> İlgili: ADR-0005 (crate sınırları: arka uçların `volt-ast` bağımlılığı Aşama 3'te kalkar; yerini alacağı kısım aşağıda), ADR-0010 (`volt-lower` yer tutucusu: ara temsilin yeri için seçeneklerden biri), ADR-0062 (Karar 4 "üretici HIR tiplerini görmez": Aşama 2'de kalkar), ADR-0081 (açılım AST kopyası üzerinde: Aşama 1 kopyadaki ifadeleri ayrıca sayar, Aşama 3 açılımı ara temsile taşır)
> Issue: #407
> Tarih: 2026-11-24
> Etkilenen: `crates/volt-sv-emit/src/width_shadow.rs` (YENİ, yalnız `width-shadow` özelliğinde), `crates/volt-sv-emit/src/expr.rs` (`width_of` sarmalayıcısı), `crates/volt-sv-emit/Cargo.toml` ve `crates/volt-driver/Cargo.toml` (özellik ve dev-dependency), `crates/volt-driver/tests/width_shadow_tests.rs` (YENİ), `docs/design/tek-yerde-bilgi-envanteri.md` (YENİ, Aşama 0)

## Sorun

Tip denetimi her ifadenin tipini hesaplar ve saklar
(`crates/volt-hir/src/typeck/mod.rs`, `TypeckResult::expr_types`). SV
üreticisi bu tabloyu okumaz: `volt-sv-emit` `volt-hir`'e değil
`volt-ast`'e bağlıdır ve genişliği, işareti ve sabit değeri kendi
fonksiyonlarıyla yeniden hesaplar (`crates/volt-sv-emit/src/expr.rs`:
`width_of`, `eval_const`, `fold_literal_arith`). ADR-0062 bunu açıkça
yazar: "üretici HIR tiplerini görmez". Aradaki `volt-lower` crate'i tek
satırlık bir yer tutucudur (ADR-0010).

Aynı bilginin iki hesabı ayrışınca sonuç sessiz bir yanlış çıktıdır.
0.1.x'teki sessiz-yanlış düzeltmelerinin birkaçının nedeni buydu;
GOVERNANCE.md'deki "One place per rule" kuralı (ADR-0075 §5, #235) onlardan
sonra yazıldı. Ortak yardımcılar teker teker ayrıldı
(`volt_ast::alias::alias_chain_end`, `volt_ast::const_eval`,
`volt_ast::struct_signals`), ama kural crate çizgesinde değil, gözden
geçirmede duruyor: bir arka uç bir bilgiyi yeniden hesaplamaya
başladığında hiçbir şey onu durdurmaz.

Aşama 0'ın envanteri (`docs/design/tek-yerde-bilgi-envanteri.md`) bugünkü
durumu sayar: hangi bilgi kaç yerde hesaplanıyor, hangi arka uç kullanıyor,
ortak yardımcıyı mı kullanıyor yoksa kendi hesabını mı yapıyor.

## Karar

### 1. Hedef

Her arka uç (SV üretimi, üretilen sürücüler `volt-sw-emit`, SDC
`volt-sdc-emit`, test dilinin simülasyonu) anlamsal bilgiyi tek yerden
okur ve kendisi hesaplamaz. Anlamsal bilgi şunlardır: bir ifadenin ya da
sinyalin genişliği, işareti, derleme zamanı sabit değeri, takma adın
çözüldüğü tip, saat alanı, struct tipli bir sinyalin yaprak sinyalleri.
Bu bilgilerin tek kaynağı tip denetimi ve alan çıkarımıdır (`volt-hir`).

Son hâl: arka uçlar çözümlenmiş bir ara temsil okur (generic ve bundle
açılımından, fn açılımından sonra; her ifade tipini, her ad tanımını
taşır) ve `volt-ast`'e bağımlı değildir. Böylece kuralı crate çizgesi
uygular: bir arka uç AST'yi göremediği için bilgiyi AST'den yeniden
çıkaramaz.

Dil ve üretilen SystemVerilog değişmez. Her adım, üretilen çıktının
önceki hâliyle bayt bayt aynı olmasıyla kabul edilir.

### 2. Aşamalar (#407)

0. **Envanter.** Bir anlamsal bilginin birden çok yerde hesaplandığı her
   nokta: genişlik, işaret, sabit değer, takma ad çözümü, saat alanı,
   struct sinyalleri. Her biri için yer, kullanan arka uç, ortak yardımcı
   mı kendi hesabı mı. Sonuç: `docs/design/tek-yerde-bilgi-envanteri.md`.
1. **Gölge karşılaştırma, genişlik.** Testlerde, her fixture, örnek ve
   kitap örneği için üreticinin her ifadeye verdiği genişlik tip
   denetiminin genişliğiyle karşılaştırılır. Davranış değişmez.
   Ayrışmalar üç sınıfa ayrılır:
   - (a) iki taraf da somut genişlik veriyor ve farklı: sessiz-yanlış
     adayı;
   - (b) bir taraf genişlik vermiyor: kapsam boşluğu;
   - (c) dilin tanımladığı bir fark (ör. bağlamdan tip alan boyutsuz
     sabit, esnek genişlik aralığı): beklenen, nedeni yazılı.

   AST kopyaları (ADR-0081 fn açılımı, ADR-0077 struct indirgemesi)
   yüzünden tip denetiminin tablosuyla eşleşemeyen ifadeler ayrıca
   sayılır. Bu ADR'yi getiren PR'da test yalnız karşılaştırmanın koştuğunu
   (sıfırdan çok birim ve ifade) ister; ayrışmalar sonucu düşürmez. Katı
   hâle getirmek (a ve b'nin sıfır olması, c'nin listeli olması) sonraki
   PR'ın işidir.
2. **Bilgi bilgi geçiş.** Üretici genişliği tip denetiminden okur ve kendi
   hesabı silinir. Sonra işaret, sabit değer, takma adlar, saat alanları.
   Her biri ayrı bir PR, çıktı bayt bayt aynı. Bir bilgi geçmeden önce
   onun gölgesi (Aşama 1'in biçimi) sıfır (a) ve sıfır (b) gösterir;
   (a) ya da (b) kalan bir bilgi geçmez, önce ayrışma çözülür (bakımcının
   kararıyla, gerekirse bir ADR ile).
3. **Tek girdi.** Üretici çözümlenmiş ara temsili okur ve `volt-ast`
   bağımlılığını kaybeder. Aynı yol `volt-sw-emit`, `volt-sdc-emit` ve
   test simülasyonu için izlenir. Generic ve fn açılımı ara temsilin
   kurulduğu yerde yapılır; üretici kopya AST görmez.

Yeni dil işleri (#403, #404, #406) baştan yeni yolda kurulur; eski yol
özellikler eklendikçe küçülür.

### 3. Ara temsilin yeri: seçenekler (karar bakımcının)

Bu ADR yeri seçmez; seçenekleri yazar. Aşama 3 başlamadan önce bakımcı
birini seçer ve bu ADR'nin "Önceki karar" bağlantıları o seçime göre
yazılır.

| Seçenek | Ne | Artı | Eksi |
|---|---|---|---|
| A. `volt-lower` | Yer tutucu crate ara temsili kurar ve taşır; arka uçlar `volt-lower`'a bağlanır. | Crate zaten var ve çizgede doğru yerde (`volt-hir`'in üstünde, arka uçların altında). Ad "alçaltma" işine uyar. | ADR-0010 `volt-lower`'ı CIRCT/melior için ayırır ve tutarlılık kontrolü 4 CIRCT referansını yalnız orada kabul eder. İki iş bir crate'te birleşir; CIRCT bir gün gelirse ara temsille aynı yerde durur. |
| B. Yeni crate (ör. `volt-mir`) | `volt-hir` ile arka uçlar arasında yeni bir katman; `volt-lower` CIRCT için yer tutucu kalır. | ADR-0010'a dokunmaz; sorumluluk adıyla ayrılır; `volt-lower` ileride ara temsili okuyup CIRCT'e indirebilir. | Bir crate daha (ADR-0005'in katman listesi değişir); iki crate'in sınırı yazılmalı. |
| C. `volt-hir` içinde bir modül | Ara temsil `volt-hir`'in genel API'sidir; arka uçlar `volt-hir`'e bağlanır. | Yeni crate yok; tipler zaten orada. | Arka uçlar `volt-hir`'in tamamını görür (çözümleyici, tip denetimi iç yapısı); `volt-hir` `volt-ast` tiplerini dışa açtıkça arka uç AST'ye yine ulaşır. Kuralı crate çizgesi değil disiplin uygular; Aşama 3'ün amacı yarım kalır. |

Her seçenekte ara temsil `volt-ast` tiplerini dışa açmaz (konumlar
`volt-span` ile taşınır); yoksa arka uç AST'ye ara temsil üzerinden yine
ulaşır.

### 4. Aşama 1'in mekanizması (bu PR)

- `volt-sv-emit`'te `width-shadow` cargo özelliği. Açıkken
  `Emitter::width_of` her sonucu (AST adresi, ifade indeksi, konum,
  ifade türü, genişlik ve işaret) bir iş parçacığı yerel kaydına yazar;
  kayıt yalnız `width_shadow::record` içinde açıktır. Özellik kapalıyken
  `width_of` yalnız `own_width`'ü (eski gövde) çağırır.
- Özelliği yalnız `volt-driver`'ın dev-dependency'si açar. Cargo'nun
  çözücüsü 2 (`resolver = "2"`) dev-dependency özelliklerini yalnız test
  derlemesinde birleştirir: `cargo build`, sürüm ikilisi ve kullanıcının
  kurduğu `volt` özelliği içermez. Kullanıcıya açık yeni seçenek, bayrak
  ya da ortam değişkeni yoktur.
- Yeni crate bağımlılığı yoktur: `volt-driver` `volt-hir`'e ve
  `volt-sv-emit`'e zaten bağlıdır; dev-dependency aynı kenarı özellikle
  açar. ADR-0005'in yönü (`volt-hir → arka uçlar → volt-driver`) korunur.
- Test (`crates/volt-driver/tests/width_shadow_tests.rs`) her birimi
  sürücünün yoluyla yükler (`volt_hir::unit_load::load_unit`,
  `check_imports`, `analyze_unit`), üreticiyi kayıtlı ve kayıtsız iki kez
  çalıştırır ve iki SV metninin aynı olduğunu ister; sonra kaydı
  `expr_types` ile karşılaştırır. Kopya AST'den gelen bir kayıt, kök
  AST'de aynı indeksli, aynı konumlu ve aynı türden ifade varsa onunla
  eşleşir; yoksa "kopya, eşleşmeyen" sayılır.
- Tip denetiminin genişliği: `bool`, `clock`, `reset` 1; `uN`, `iN`,
  `bits<N>` N; `Trit` 2; enum ve struct sinyal genişliği
  (`TypeArena::signal_width`); esnek aralık (ADR-0025) `[lo, hi]`;
  `Delayed<T, N>` T'ninki; boyutsuz literal, dizi, demet, örnek, yerleşik
  primitif ve hata tipi genişliksiz.

### 5. Aşama 1'in sonuçları

Bu ADR'yi getiren PR'ın yerel koşusu (`cargo test -p volt-driver --test
width_shadow_tests -- --nocapture`, 2026-11-24):

| Ölçü | Sayı |
|---|---|
| Girdi | 487 dosya (`tests/fixtures` 238, `tests/ui/pass` 147, `tests/ui/multifile` 23, `tests/real_bugs` 35, `examples` 34, `templates` 8, `demo` 2) + 25 kitap bloğu |
| Derlenen birim | 305 (`ui/pass`, `examples`, `templates` ve kitap bloklarının hepsi) |
| Atlanan birim | 207, hepsi bilerek hatalı girdi: `fixtures/parity`'nin hata örnekleri 178, `real_bugs`'ın beklenen tanılı dosyaları 19, `ui/multifile`'ın hata durumları 9, `demo/crossing.volt` (CDC hatası gösterimi) 1 |
| Karşılaştırılan ifade | 9811 (her birinde üretici tek genişlik verdi) |
| Tip denetiminin kaydettiği ifade (aynı birimlerde) | 33843; üretici bunların 9811'inin genişliğini sordu |
| Kopya AST'den gelen ifade | 3398: kökle eşleşen 2699, eşleşmeyen 699 |
| Aynı genişlik | 7624 (esnek tipte doğal genişlik 13 dahil) |
| İki taraf da genişliksiz | 494 |
| (a) sessiz-yanlış adayı | 1 |
| (b) kapsam boşluğu | 0 |
| (c) beklenen | 1692 |

**(a)** — tek ayrışma bilinen açık #215'tir
(`tests/real_bugs/215_match_expression_width.volt:18`):

```
let m = match c { true => a + b, _ => a - b }   // a, b : u8
y = m                                            // y : u9
```

Üretici `m` için 8, tip denetimi esnek [8, 9] (doğal 9) verir. Spec'e
göre 9 doğrudur: type-inference.md §3.9 "match ifadesi çok kollu `if`
ifadesidir, genişlik davranışı ayrışmaz", ve tipsiz `let`'in teli doğal
genişliktedir (ADR-0025). Üretilen SV `logic [7:0] m` bildirir ve toplamı
keser (`a = 200, b = 100` için `y = 44`). Gölge bu hatayı kendi kuralından
buldu; `real_bugs` dosyası ve metamorfik testin bilinen hata listesi onu
zaten taşır.

**(c)** — dört kural, her biri dilin bir kuralına dayanır:

| Kural | Sayı | Neden beklenen |
|---|---|---|
| Boyutsuz literal bağlamdan tip alır | 1596 | type-inference.md §3.1, §4: soneksiz literal (ve yalnız literallerden kurulan `-x`, aritmetik, `if`, `match`) kendi tipini taşımaz, check kipinde beklenen tipi alır; üretici onu bağlamın genişliğinde basar (`width_of` `None`). Ayrıca generic parametresi yerine konan literal (ör. `150_generic_distinct_mangled_names.volt:7`). |
| Bağlam genişliği (ADR-0041) | 60 | Check kipinde aynı işaretli operandlar işlemden önce hedef genişliğe genişletilir; tip denetimi operandın kaydına hedef tipi yazar (`typeck/check.rs` `widen_operands`). Üretici operandın doğal genişliğini verir ve genişlemeyi `W'(x)` ile yazar (ör. `fir_filter.volt:120`: `taps_r[0]` 16 bit, i32 bağlamda). Kural yapısaldır: aritmetiğin ve iki operandının kayıtlı tipi aynıysa (genişletme yolu) ya da `if`/`match`'in kaydı hiçbir kolununkiyle aynı değilse (check kipi), ve üreticinin genişliği aynı işaretle daha darsa. |
| Paketlenmiş dizi (ADR-0056) | 31 | Dizi tipli port ve tel SV'de paketlenmiş vektördür; üretici toplam genişliği verir, tip denetimi `[T; N]` (karşılaştırma `N × genişlik(T)` ile). |
| Esnek aralık (ADR-0025) | 5 | İşlem, `if` ya da `match` esnek tiplidir ve üretici aralığın içinde işlem genişliğini verir; genişliği okuyan seçer (ör. `140_division_operand_explicit_width.volt:17` `a * b`: 8, aralık [8, 16], ADR-0112'nin W2015 davranışı). Ad (tel) için bu kural uygulanmaz: tel doğal genişlikte olmalıdır, değilse (a) (#215). |

**Gölgenin dışında kalanlar:** tip denetiminin kaydettiği ifadelerin
24032'si için üretici genişlik hesaplamaz (metni bağlamın genişliğiyle
yazar ya da başka bir yoldan, ör. `let_width.rs`, okur); bunların
karşılaştıracağı bir üretici genişliği yoktur. Kopya AST'nin eşleşmeyen
699 ifadesi fn açılımının (ADR-0081) ve struct indirgemesinin (ADR-0077)
yeni düğümleridir. Aşama 2 genişliği geçirirken bu iki küme de tip
denetiminden okunmalıdır; geçiş, üreticinin bugün sormadığı ifadeleri de
kapsar.

### 6. Yerini alacağı kısımlar

Bu ADR önerildi statüsündedir; aşağıdaki kısımlar ancak ilgili aşama
uygulanınca yerini alır. O PR'da bu ADR'ye `> Önceki karar:` satırı
eklenir ve eski ADR'nin statüsü `Kısmen yerini aldı: ADR-0136` olur
(bugün yalnız `İlgili` bağlantısı vardır):

- **ADR-0005, "Karar", katmanlar maddesi:** arka uçların (`volt-sv-emit`,
  `volt-sw-emit`, `volt-sdc-emit`) `volt-ast`'e doğrudan bağımlılığı
  Aşama 3'te kalkar; arka uçlar ara temsilin crate'ine bağlanır.
  Seçenek B seçilirse katman listesine yeni crate girer. Dil, lexer, tanı
  kütüphanesi ve sürüm politikası maddeleri değişmez.
- **ADR-0010, "Karar", `volt-lower` maddesi:** yalnız seçenek A
  seçilirse: `volt-lower` yer tutucu olmaktan çıkar ve ara temsili taşır.
  Doğrudan SV üretimi (string template) ve CIRCT'in ertelenmesi değişmez.
- **ADR-0062, Karar 4'ün "üretici HIR tiplerini görmez" gerekçesi:**
  Aşama 2'de genişlik ve işaret geçince Trit-lik tip denetiminden okunur;
  çarpansız `Trit * x` eşlemesi değişmez.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; kullanıcıdan yeni bir yazım istenmez.
2. **Çıkarım (İ2):** çıkarımın kendisi değişmez; değişen, çıkarımın kaç
   yerde yapıldığıdır. Hedef, her bilginin tek cevaplı tek bir çıkarımı
   olmasıdır (İ2'nin "tek cevaplı" koşulu bugün iki hesap arasında
   ayrışabiliyor; Aşama 1 bunu ölçer).
3. **Anlam değişikliği (İ3):** yok. Her aşama üretilen SV'nin bayt bayt
   aynı kalmasıyla kabul edilir. Aşama 1'in bulduğu bir (a) ayrışması
   anlam değişikliği gerektiriyorsa (üreticinin bugünkü genişliği spec'e
   göre yanlışsa) o, bu ADR'nin dışında, kendi issue'su ve İ3'ün koşullarıyla
   ele alınır.
4. **Niyet tahmini (İ4):** uygulanmaz.
5. **Metamorfik yeniden yazım:** uygulanmaz; gölge karşılaştırma bir
   yeniden yazım değil, iki hesabın aynı ifadede karşılaştırılmasıdır.

## Doğrulama

- Üretilen SV aynı: gölge testi her birimi kayıtlı ve kayıtsız üretir ve
  metinlerin eşitliğini ister; özellik kapalı derlemede (`cargo build`)
  tek değişiklik `width_of`'un `own_width`'ü çağıran bir sarmalayıcı
  olmasıdır. Bütün altın karşılaştırmalar ve testler bugünkü gibi geçer.
- İkili karşılaştırma (PR'ın yerel koşusu): main'in (2bff047d) ve bu
  dalın özelliksiz `volt` ikilisi gölgenin 487 girdi dosyasının her biri
  için `volt build --single-file --emit=sva,sdc` ile çalıştırıldı. Çıkış
  kodları 487 dosyanın hepsinde aynı; üretilen 640 dosya (SV, SVA, SDC)
  sürüm başlık satırı dışında bayt bayt aynı.
- Gölge testi karşılaştırmanın koştuğunu ister: sıfırdan çok birim, ifade
  ve kitap bloğu.

## Sınırlar

- Aşama 1 yalnız genişliği karşılaştırır; işaret, sabit değer, takma ad,
  saat alanı ve struct sinyalleri envanterde sayılır ama gölgesi yoktur.
- Üreticinin yalnız `width_of`'u kaydedilir. Kendi genişlik hesabını
  yapan başka yerler (`let_width.rs`, `sim_width.rs`, `sig_of_typeref`'in
  bildirim genişlikleri, yerleşik primitif portları) envanterdedir,
  gölgede değildir.
- Kitap blokları `book/tools/check_book.py`'nin kuralıyla toplanır
  (`should_fail` atlanır, adlı bloklar dizin başına ortak çalışma
  dizininde), ama derlenmeyen blok atlanır ve sayılır.
- Gölge `width_of`'un sonucunu karşılaştırır, üretilen metni değil. (c)'nin
  "bağlam genişliği" kuralı, üreticinin genişlemeyi gerçekten `W'(x)` ile
  yazdığını kanıtlamaz; bunu davranış testleri (fark testi, metamorfik test,
  `real_bugs`) sınar.
- Kopya AST'deki eşleşme indeks, konum ve ifade türüyle yapılır: aynı
  indeksteki ifadenin alt ağacı açılımda değişmişse (ör. struct alanı
  yaprak sinyale indirildiyse) karşılaştırma yine kök ifadenin tipiyle
  yapılır.

## Reddedilenler

- **Üreticiye doğrudan `volt-hir` bağımlılığı eklemek ve `expr_types`'ı
  hemen okumak:** Aşama 1'in ölçümü olmadan hangi ifadelerin eşleşmediği
  (AST kopyaları) ve iki tarafın nerede ayrıştığı bilinmez; geçiş bayt
  bayt aynı çıktıyı tutturamayabilirdi.
- **Kaydı her zaman derlemek (özelliksiz):** kullanıcı ikilisine test
  kodu girerdi; görevin kuralı karşılaştırmanın yalnız test derlemesinde
  koşmasıdır.
- **Ayrışmaları bu PR'da düzeltmek:** her (a) ayrışması bir anlam
  sorusudur; bakımcı karar verir.
