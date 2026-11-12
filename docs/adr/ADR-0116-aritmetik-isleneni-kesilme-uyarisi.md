# ADR-0116: Aritmetik İşlenen İşlemden Önce Kesiliyor — Geçici Uyarı W2016 (#266)

> Statü: Uygulandı — geçici: #266'nın v0.2.0 kararıyla kalkar ya da yeni kurala göre değişir
> İlgili: ADR-0025 (`+ - *` esnek aralıkların kesişiminde; kesişimden geniş işlenen işlem genişliğine kesilir), ADR-0041 (check kipinin itmesi: bildirilmiş genişliğe atanan sonuç), ADR-0108 (fark testinin modeli aynı okumayı paylaşır, bu yüzden görmedi; A1), ADR-0112 (W2015: bölmenin işleneni; iki uyarı aynı yerde çıkmaz)
> Tarih: 2026-11-12
> Etkilenen: `crates/volt-hir/src/typeck/arith_cut.rs` (YENİ:
> `warn_cut_arith_operands`, değer aralığı, geri alma),
> `crates/volt-hir/src/typeck/{synth,check,binop,cast,mod,stmt,width}.rs`
> (`arith_demand`, `arith_chain`, `read_low_bits`), `crates/volt-diagnostics`
> (`W2016`: kod, kısa açıklama ve `volt explain` metni, en/tr),
> `crates/volt-hir/tests/arith_cut_property_tests.rs` (YENİ),
> `crates/volt-driver/tests/arith_cut_warning_tests.rs` (YENİ),
> `tests/ui/fail/274_arith_operand_cut.volt` (YENİ),
> `tests/ui/pass/153_arith_operand_explicit_width.volt` (YENİ),
> `book/src/limitations.md`, `CHANGELOG.md`; durum sözlüğüne `Önerildi`:
> `docs/adr/README.md`, `scripts/check-consistency.{sh,ps1}`, `AGENTS.md`

## Sorun

#266: ADR-0025'e göre `+ - *` iki işlenenin genişlik aralıklarının
kesişiminde, kendi taşmasıyla yapılır. İşlem genişliği `C`, kesişimin üst
ucu + 1'dir (`*` için iki katı). Doğal genişliği `C`'den geniş (esnek)
işlenen işlemden önce `C` bite kesilir. `a * b - a` (a, b : i32): çarpım
i32..i64, `- a` 32'de buluşur, işlem 33 bitte; 64 bitlik çarpım 33 bite
iner. Üretilen SV:

```systemverilog
assign p = 33'(a) * 33'(b) - 33'(a) == -33'sd1048576;
```

a = b = 2^20 için gerçek değer 2^40 - 2^20, sonuç -2^20 (2^40, 2^33'ün
katı). `let d : i64 = a * b - a` doğru değeri verir (itme, ADR-0041, 64
bitte hesaplar). i64 işlenenlerde çarpım 128 bittir ve 128 bitlik `let`
yoktur (uN/iN en çok 64, E0003): kesmeden kaçınmanın yolu yok.

Kural değişmiyor: bakımcı v0.2.0'da işlenenin doğal genişliğinde kalmasını
kararlaştırdı (#266, "Plan"). O zamana kadar kullanıcı değeri bu kurala
bağlı olan işlemi görmüyor.

## Fark testi bunu neden görmedi

Hipotez doğru: fark testinin modeli derleyicinin okumasını paylaşıyor.
`crates/volt-difftest/src/model.rs`, `int_arith`: sonuç tipi kesişimin üst
ucu `c`'den `n = c + 1` (`*` için `2c`); `eval_bin` iki işleneni de
`operand(x, n, …)` ile `n` bite dönüştürür (`conv`, kesme dahil) ve işlemi
`n` bitte yapar. A1 varsayımının metni ("esnek işlenen sonucun doğal
genişliğine dönüştürülür, kesişimin üst ucuna kesilmez") `n`'yi kastediyor;
`a * b - a`'da `n` = 33, yani çarpım modelde de 33 bite kesilir. Model ile
derleyici aynı değeri bulur, fark çıkmaz. İkinci kör nokta: model doğal
genişliği 64'ü aşan ifadeyi `Unspecified("natural width over 64")` ile
eler (`MAX_INT`); i64 işlenenli `a * b` hiç üretilmez.

Sonuç: şartnamenin kuralını paylaşan tek bir hakem kuralın kendi hatasını
göremez. Not #255'e (Kalite 3, bağımsız hakemler) eklendi: sınırsız tam
sayıyla hesaplayan "matematiksel" bir hakem, kuralın matematikten ayrıldığı
her yeri listeleyebilir; bilinen ve bilerek kabul edilmiş farklar (sayaç
sarması, bildirilmiş genişliğe atama) bir listede tutulur.

## Karar

1. **W2016**, tip denetiminde (`arith_of`, `+ - *`, iki işlenen de literal
   değil): bir işlenenin doğal genişliği (aralığının üst ucu) işlem
   genişliği `C`'den genişse, o işlenenin konumunda bir uyarı; aşağıdaki
   üç durum hariç. İleti: "this operand of '-' is cut to 33 bits before
   the operation (its natural width is 64); the rule changes in v0.2.0
   (#266)". Etiket: "a 64-bit value, computed at 33 bits".
2. **Uyarı yok** (değer kurala bağlı değil; gerekçe "Sağlamlık"):
   1. **Bağlam yalnız alt `k` biti okuyor ve `k ≤ C`.** Somut uN/iN
      hedefe giren `+ - *` (check kipi: atama, tipli `let`, register,
      çağrı argümanı, port bağlantısı) sonucun yalnız `k` bitini tutar.
      Bu okuma `+ - *`, bit işlemleri, tekli `-` ve `~` zinciri boyunca
      işlenenlere geçer (`arith_demand`); başka her biçim (karşılaştırma,
      bölme, kaydırma, seçim, `if`, çağrı, dönüşüm) işlenenin tamamını
      okur. Biriktirici deseni (`acc <= acc + a * b`) bu yüzden uyarı
      almaz.
   2. **Sonucun değer aralığı `C` bite sığıyor.** Aralık yapraklardan
      kurulur: literal ve `const` adı (değeri SV'ye değer olarak yazılır),
      blok içi `for` değişkeni (sınırları), başka her ifade doğal tipinin
      aralığı; `+ - *` ve tekli `-` aralık aritmetiğiyle. Sonucu düğümün
      tip aralığını aşan düğüm sarar, aralığı tip aralığıdır; işlenenini
      kesen düğümün değeri kurala bağlıdır, aralığı yoktur. **Yalnız
      işlenenin sığması yetmez:** `u * 2 + w` (u8) işleneni 0..510 ile 9
      bite sığar, toplam 0..765 sığmaz; doğal genişlikte 765, bugün 253.
   3. **Ebeveyn zincirin kökünün yalnız alt `k` bitini okuyor** (W2015'li
      bölme: işleneni bölme genişliğine keser; daraltan `as`): zincirde en
      az `k` bitte yapılan işlemin W2016'sı geri alınır
      (`read_low_bits`). `(a * b - a) / b` yalnız W2015 alır; iki uyarı
      aynı yerde çıkmaz (iki işlenen de aynı sütunda başlar).
      `(a * b - a) as i32` yalnız W2010 alır; genişleten `as i64` W2016'yı
      korur.
3. **Uyarı reddetmez**; üretilen SV değişmez.
4. **Yardım satırı** çözümü gösterir: "put the result of this '-' in a
   'let' with an explicit width, e.g. 'let v : i64 = …': it is computed
   at 64 bits and nothing is cut". Genişlik işlenenlerin en geniş doğal
   genişliğidir; itme (ADR-0041) o genişlikte hiçbir işleneni kesmez.
   Bu genişlik 64'ü aşıyorsa (i64/u64 çarpımı) yardım kaçınma yolu
   olmadığını söyler: "no integer type is 128 bits wide yet (the widest
   is 64), so the cut cannot be avoided until the rule changes in v0.2.0;
   see issue #266". Metinde ADR numarası yok (ADR-0099), issue numarası
   var.
5. **Makineyle uygulanabilir öneri yok.** Çözüm yeni bir ad ve bir
   deyim konumu ister (`let` ifadenin üstüne, ifade adla değişir); ad
   çakışması, blok içi ve tel kipi ayrımı ve okunan genişlik yüzünden
   gidiş-dönüşü (ADR-0099 §2) her bağlamda güvence altına almak bu
   geçici uyarının kapsamını aşar. `(…) as i64` gibi tek düzenlemeli bir
   öneri işe yaramaz: `a * b - a`'da `(a * b) as i64 - a` E2001'dir
   (i64 ile i32 kesişmez). Yardım satırı ve `volt explain` biçimi
   gösterir.
6. **Kod: yeni W2016, W2015'i genişletmek değil.** (a) W2015'in metni,
   kısa açıklaması ve `volt explain`'i bölmeyi anlatır ve #150'ye
   bağlanır; bu uyarı #266'ya bağlanır ve kuralı ayrı bir kararla değişir.
   (b) Fark testi W2015'i modelin `DIV_CUT` işaretiyle karşılaştırır
   (ADR-0112); aynı kod iki farklı koşulu taşısa o karşılaştırma bozulur.
   (c) ADR-0112'nin karar metni değişmez; W2015'in kapsamını genişletmek
   yeni bir ADR'de yine ayrı bir koşul olurdu. W2 ailesi tip ve genişlik
   uyarılarıdır; sıradaki boş numara W2016.

## Sağlamlık

İddia: W2016'nın çıkmadığı her programda, her girdi için, gözlenen değer
(çıkışın tuttuğu bitler, karşılaştırmanın sonucu, bölüm, dönüşüm)
bugünkü kuralla (A) ve hiçbir işlenenin doğal genişliğinden aşağı
kesilmediği hesapla (B: her `+ - *`, `max(C, N_l, N_r)` bitte) aynıdır.

- **Alt bitler.** `+ - *`'nın sonucunun alt `k` biti işlenenlerin alt `k`
  bitinden hesaplanır; bit işlemleri, tekli `-` ve `~` de öyle. `C ≥ k`
  bitte kesmek ve sonra en az `k` bitte işlem yapmak alt `k` biti
  değiştirmez (2^C, 2^k'nin katı). Zincir boyunca her susturulan düğümün
  genişliği en az `k`'dır; susturulmayan düğüm uyarı alır. Okumayı
  zincire yalnız sonucun alt `k` bitini tutan bağlam (somut hedef, daraltan
  `as`, işleneni kesen bölme) başlatır.
- **Değer aralığı.** Kesilmiş işlenenlerle `C` bitte yapılan işlem gerçek
  sonuçla `2^C` modülünde eşittir; gerçek sonuç `C` bite sığıyorsa ikisi
  aynı sayıdır, B de onu verir. Aralık gerçek sonucu kapsar: yaprakların
  değeri kesin ya da tipinin aralığındadır; aralık aritmetiği yalnız
  işlenenini kesmeyen ve sarmayan düğümlerden geçer, bunlarda A ile B
  aynıdır. Const değerlendirmesi bileşik ifadeye uygulanmaz: ara değerleri
  kesmeden hesaplar, çalışma zamanındaki değer ise iç kesmeyi taşıyabilir.

**Test** (`arith_cut_property_tests`): derleyiciden bağımsız bir model
(tipler ADR-0025'ten, elle) rastgele programlar üretir (işaretli ve
işaretsiz, 1..10 bit üç port, derinlik 1..3, `+ - *`, küçük literaller)
ve dört bağlamda (atama, karşılaştırma, dönüşüm, bölme) A ile B'yi 48
rastgele girdide karşılaştırır; girişleri (bölmede bölenle birlikte)
toplam 12 bit ya da daha az olan programlarda ayrıca bütün girdileri
(en çok 4096) dener. Özellik: derleyici W2016 vermediyse A = B. Ek
koşullar: model kesme gördüğü hâlde derleyicinin sustuğu programlar her
bağlamda üretilir ve bunların bir kısmı tam taranır, uyarı çıkan
programların en az birinde A ≠ B bulunur. Bağlam başına 4000 program
(tohumlar sabit; tam tarama rastgele girdilerden sonra koşar, tohumun
akışını değiştirmez):

| Bağlam | Uyarılı | Susturulan kesme | Uyarılı ve A ≠ B | Tam taranan | Tam taranan susturulan | Denenen girdi |
|---|---|---|---|---|---|---|
| atama | 41 | 275 | 35 | 866 | 44 | 1588272 |
| karşılaştırma | 357 | 108 | 263 | 814 | 19 | 1496152 |
| dönüşüm | 145 | 327 | 88 | 806 | 42 | 1550480 |
| bölme | 145 | 304 | 24 | 290 | 21 | 675552 |

Tam tarama karşılaştırmada bir uyarılı programda daha A ≠ B girdisi
buldu (262 → 263); susturulan programların hiçbirinde, hiçbir girdide
A ≠ B yok. Dört test birlikte yaklaşık 2 saniye sürer.

Testin ayırt ettiği, iki bozuk kuralla atılabilir bir worktree'de
denendi: "yalnız işlenen sığıyor" koşulu iki testi düşürdü (ör. atamada
`[511, 0, 165]` girdisi: A 184982, B 1375894); okunan genişliğin bir bit
gevşetilmesi (`read <= width + 1`) atama testini düşürdü.

## Gürültü

`volt check` depodaki 785 `.volt` dosyasında (`examples`, `tests`,
`templates`, `book`, `demo`, `crates`) ve `book/tools/check_book.py` 30
kitap bloğunda:

| Kural | W2016 | Gerçekten yanlış sonuç | Zararsız |
|---|---|---|---|
| Saf: her kesilen işlenen | 10 | 0 | 10 |
| + okunan genişlik (Karar 2.1) | 8 | 0 | 8 |
| + değer aralığı (Karar 2.2, 2.3) | 0 | 0 | 0 |

Saf kuralın zararsız 10 uyarısı: bildirilmiş genişliğe atanan iki
sonuç (`examples/systolic/pe_array.volt:38` biriktirici, `acc <= acc +
(a_in as i16) * (b_in as i16)`; `tests/ui/pass/04_operator_precedence.volt:11`,
u16 hedefe `a as u16 + (b as u16) * (c as u16)`) ve modül düzeyi
`for`'un sekiz dizin ifadesi (`act_link[y * (BN + 1) + BN]`:
`examples/hybrid_accel/{binary_array,hybrid_test,hybrid_top,ternary_array}.volt`,
`examples/systolic/pe_array.volt:71`,
`tests/ui/pass/76_nested_for_instantiation.volt:40`; değerleri derleme
zamanında küçük sabitler). Kitap bloklarında hiç kesme yok. Kalıcı test:
`no_volt_file_in_the_repository_gets_w2016` (W2016'yı bilerek taşıyan
ui/fail 274 dışında).

## Sınırlar

- Kural tutucudur: kanıtlanamayan her kesmede uyarır. `(u & 7) * 3 + w`
  uyarı alır (bit işleminin aralığı izlenmez, `u & 7` 0..255 sayılır);
  kaydırma, `if`, `match` ve çağrı işlenenin tamamını okur sayılır.
  Depoda bu biçimler uyarı üretmiyor; gerekirse aralık izleme sonradan
  genişletilir.
- Tipsiz `let`'in değeri tam okunur sayılır; `let s = u * v + w` sonra
  `o8 = s` biçimi uyarı alır.
- Satır içi açılan fn gövdesi tip denetiminden bir kez geçer: uyarı fn
  tanımındaki işlenende bir kez çıkar.
- Fark testi W2016'yı karşılaştırmaz: modelin okunan genişlik zinciri
  yok. #266'nın v0.2.0 kararıyla model ve uyarı o ADR'ye göre yazılır.
- #266 kapanınca bu ADR'nin yerini o karar alır: uyarı kalkar ya da yeni
  kuralın kestiği yerlere göre değişir.
