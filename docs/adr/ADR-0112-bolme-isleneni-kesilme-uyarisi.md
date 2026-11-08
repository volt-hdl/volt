# ADR-0112: Bölmenin İşleneni Bölmeden Önce Kesiliyor — Geçici Uyarı W2015 (#150)

> Statü: Uygulandı — geçici: #150'nin kararıyla (v0.2.0) kalkar ya da yeni kurala göre değişir
> İlgili: ADR-0025 (`/` ve `%` esnek aralıkların kesişiminde; kesişimden geniş işlenen kesilir), ADR-0108 (fark testi: uyarının ifadeleri modelin kesilen işlenenleriyle aynı)
> Tarih: 2026-11-08
> Etkilenen: `crates/volt-hir/src/typeck/binop.rs`
> (`warn_cut_division_operands`), `crates/volt-diagnostics` (`W2015`: kod,
> kısa açıklama ve `volt explain` metni, en/tr),
> `crates/volt-difftest/src/{model,harness,run}.rs` (`tag::DIV_CUT`,
> `Case::div_cut`, `Class::DivCutWarning`, özet satırı),
> `.github/workflows/difftest.yml` (`div_operands` girdisi),
> `crates/volt-driver/tests/division_cut_warning_tests.rs` (YENİ),
> `tests/ui/fail/245_division_operand_cut.volt` (YENİ),
> `tests/ui/pass/140_division_operand_explicit_width.volt` (YENİ),
> `CHANGELOG.md`

## Sorun

#150: `/` ve `%`'nin işleneni bir aritmetik ifade (esnek tip, ADR-0025)
olduğunda tip denetiminin işlem genişliği ile üretilen SV'nin işlem
genişliği ayrışıyor. type-inference.md §3.3'e göre `/` ve `%` genişlemez:
bölme iki işlenenin aralıklarının kesişiminde yapılır, kesişimden geniş
işlenen önce ona kesilir. `o : u9 = a * b / c + d` (dördü u8): `a * b`
u8..u16, bölme 8 bitte, çarpım 16 bitten 8 bite kesilir. Üretici ise
bölmeyi bağlamın genişliğinde (9 bit) yapıyor. Kesişimden geniş işlenenin
bölmedeki değeri ADR-0025'in açık bıraktığı bir okumaya bağlı (A1
ailesi); karar v0.2.0'da verilecek ve kural değişecek. Bakımcı o kararı
ayrı bir ADR'de yazacak (#150).

O zamana kadar kullanıcı, değeri bu kurala bağlı olan bölmeyi görmüyor.

## Karar

1. **W2015**, tip denetiminde (`arith_of`): `/` ya da `%`'nin iki işleneni
   de literal değil ve bir işlenenin doğal genişliği (aralığının üst ucu)
   bölmenin yapıldığı genişlikten (kesişimin üst ucu) genişse, o işlenenin
   konumunda bir uyarı. İleti:
   "this operand of '/' is cut to 8 bits before the division (its natural
   width is 16); the rule changes in v0.2.0 (#150)". Etiket: "a 16-bit
   value, divided at 8 bits". Öneri: genişliği açıkça yazmak, ör.
   `(a + b) as u9 / (n as u9)` (9 bitte böler, hiçbir bit kesilmez).
2. **Uyarı reddetmez**; üretilen SV değişmez.
3. **Uyarı yok:**
   - itilen bölmede (check kipi, ADR-0041, type-inference.md §5:
     işlenenlerin doğal genişliği hedefe sığıyor, bölme hedef genişlikte;
     `o : u9 = (a + b) / c` kesmez);
   - literal işlenende (karşı yana uyarlanır, §4) ve literal aritmetiğinde
     (#144);
   - const ve aynı genişlikteki atom işlenende (somut tip, kesişimin
     kendisi).
   Tipsiz `let`'in esnek değeri (`let s = a + b`, `s / c`) aritmetik
   ifadedir: uyarı alır.
4. **Metinde ADR numarası yok** (ADR-0099), issue numarası var: kullanıcı
   kararın durumunu #150'den izler. Görevin önerdiği örnek düzeltme
   (`((a as u9) + b) / n`) derlenmiyor (E2001: u9 ile u8); öneri iki
   işlenene de `as` yazar.
5. **Kod W2015**: W2 ailesi tip ve genişlik uyarılarıdır (W2010 daraltan
   `as`, W2013 genişlik kadar kaydırma).

## Fark testi

Model (`model.rs`) bölmenin işleneni kesildiğinde bir işaret taşır,
`tag::DIV_CUT` (bir varsayım değil; `ASSUMPTIONS` listesinde yok, raporun
varsayım sayımına girmez): `int_arith`'te `/` ya da `%` ve işlenenlerden
birinin üst ucu kesişimin üst ucundan büyük. Check kipinin itmesi
(`push`) üstteki işlemi sentezlemediği için itilen bölme işaret almaz;
modelin kuralı derleyicininkiyle aynı yerden kurulmadı, şartnameden
(§3.3, §5) yazıldı.

Koşucu (`run.rs`) her toplu koşunun temiz `volt check` turunda a
biçiminin (`o = e`, çıkış ifadenin doğal tipinde) satırındaki W2015'i
okur ve modelin işaretiyle karşılaştırır. Ayrışma bir bulgudur
(`Class::DivCutWarning`, sert): koşu düşer. Özet satırı:

```
difftest: division operand cut (W2015, variant a): model=N compiler=N mismatches=0
```

Varsayılan koşuda bölmenin işlenenleri atomdur (ADR-0108, "Ek: `/` ve
`%`"): iki sayı da 0. Bileşik işlenen `VOLT_DIFFTEST_DIV_OPERANDS=1` ile
açılır; `difftest.yml`'e bunun için `div_operands` girdisi eklendi. O
koşu #150'nin değer farklarını da bulur (kural kararı bekliyor), yani
düşer; W2015 karşılaştırmasının sonucu özet satırındadır.

## Etki

Depodaki `.volt` dosyalarının hiçbiri W2015 almaz
(`no_volt_file_in_the_repository_gets_w2015`: `examples/`, `tests/`,
`templates/`, `book/`, `demo/`, `crates/` altındaki her dosya `volt check`
ile): örnekler, kitap, şablonlar ve fikstürler bölmeyi atom ya da literal
işlenenle yazıyor.

## Sınırlar

- Uyarı tip denetiminin okumasını söyler (kesişim). Bugünkü SV bölmeyi
  bağlamın genişliğinde yapıyor ve bazı durumlarda kesmiyor (#150'nin
  örneği: SV 256, iki okuma da 0). Uyarı bu ayrışmanın olduğu yerleri
  gösterir; SV'yi düzeltmez. Genişliği açık yazan ifade iki okumada da
  aynı değeri verir.
- Satır içi açılan fn gövdesi tip denetiminden bir kez geçer: uyarı fn
  tanımındaki işlenende bir kez çıkar, çağrı başına değil.
- #150 kapanınca bu ADR'nin yerini o karar alır: uyarı kalkar ya da yeni
  kuralın kestiği yerlere göre değişir.
