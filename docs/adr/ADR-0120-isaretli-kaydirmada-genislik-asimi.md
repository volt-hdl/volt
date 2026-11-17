# ADR-0120: İşaretli `>>` Genişlik Kadar ya da Daha Fazla Kaydırınca İşaret Bitiyle Dolar; Sabit Miktar SV'de `>>> (w-1)` Yazılır (#302)

> Statü: Uygulandı
> Önceki karar: ADR-0025 — W2013 bölümündeki "sonuç her zaman 0'dır" cümlesi, işaretli `>>` için: sonuç işaret bitiyle dolar (işaretsiz `>>` ve `<<` için cümle yürürlükte)
> İlgili: ADR-0036 (§1: işaretli `>>` aritmetiktir, `>>>`; bu ADR genişlik kadar ve fazla kaydırmanın değerini yazar), ADR-0100 (sessiz yanlış istisnası: düzeltme son hanede, v0.1.3), ADR-0107 (hakem sürümü: sabitlenen Verilator 5.052'nin hatası kanaryada belgelenir), ADR-0108 (modelin A12 okuması bu ADR'yle kural olur), ADR-0119 (İlke denetimi), ADR-0121 (Sınırlar'ın "Volt'ta sabit olmayan, Verilator'da sabitleşen miktar" maddesini kapatır: hakem 5.054, #309; kalan dar hata Verilator #9256)
> Issue: #302
> Tarih: 2026-11-17
> Etkilenen: `crates/volt-sv-emit/src/expr.rs` (`emit_binary`; YENİ: `clamped_signed_shift`,
> `shift_amount`),
> `crates/volt-hir/src/typeck/binop.rs` (W2013 etiketi), `crates/volt-diagnostics`
> (W2013 kısa açıklaması ve `volt explain` metni, en/tr),
> `crates/volt-difftest/src/{model,regress}.rs` (A12 açıklaması, #302 örneği),
> `crates/volt-hir/tests/binop_tests.rs`,
> `crates/volt-driver/tests/signed_overshift_sim_tests.rs` (YENİ),
> `crates/volt-driver/tests/verilator_canary_tests.rs` ve
> `verilator_canary/issue302.sv` (YENİ), `tests/real_bugs/302_signed_overshift.volt`
> (YENİ), `CHANGELOG.md`

## Sorun

#302: fark testi `i63 >> 64`'te ayrıştı. Model (ADR-0108, A12) işaretli
`>>`'yi genişlik kadar ya da fazla kaydırınca işaret bitiyle doldurur
(negatif değerde -1); Verilator 0 verdi. Teşhis (#302'deki yorum) iki şey
buldu:

1. **Bir Verilator hatası.** Verilator 5.050–5.052, işaretli `>>>`'da
   sabit miktar K, C++ kelime genişliğine (cppw: w ≤ 32 için 32, 33–64 için
   64, üstünde 32·⌈w/32⌉) eşit ya da büyük ve w cppw'nin katı değilken
   işaret bitini w-1'den değil cppw-1'den alır; negatif değere 0 verir
   (V3Premit, Verilator #7806 ile 5.050'de geldi, 84dec259c8 ile 5.054'te
   düzeldi). 5.054'te dar bir hâli kalır: 64 bitten geniş, w mod 32 = 1
   (65, 97, 129) bileşik işlenende K ≥ cppw sonucu `…0001` verir. Volt
   Verilator 5.052'yi hakem olarak sabitler (ADR-0107): `volt test` ve
   `volt run` negatif değere 0 verirken Yosys, sentez ve `volt verify` -1
   verir. Simülasyon donanımla ayrışır: sessiz yanlış.
2. **Şartnamede bir boşluk.** Hiçbir bağlayıcı kural işaretli `>>`'nin
   genişlik kadar ya da fazla kaydırmadaki değerini yazmaz. Yakın belgeler
   ayrışır: ADR-0025'in W2013 cümlesi "sonuç her zaman 0'dır" der (yazıldığı
   gün `>>` hep mantıksaldı); ADR-0036 §1 işaretli `>>`'yi aritmetik yapar
   ve sabit değerlendirmenin i128 `>>` ile tutarlı olduğunu söyler;
   const-eval.md `Shr`'yi i128 `checked_shr` ile katlar (`-1 >> 10` = -1);
   sv-mapping.md §6 işaretli `>>`'yi `>>>`'a eşler, IEEE 1800-2017
   §11.4.10'a göre boşalan bitler işaret bitiyle dolar. Kullanıcıya
   gösterilen W2013 metni de işaretli işlenende yanlıştır.

Bugünkü davranış (derleyicinin ürettiği SV'nin IEEE anlamı, Icarus, Yosys,
sentez, model) zaten işaretle doldurur; ayrışan yalnız W2013 metni ve
Verilator 5.050–5.052'dir.

## Karar

### 1. Kural: işaretli `>>` işaret bitiyle dolar

`x >> K`, `x : iN` ve K ≥ N (sabit ya da çalışma anı): bütün bitler boşalır
ve işaret bitiyle dolar. Negatif `x` için sonuç -1 (N birli örüntü),
negatif olmayan için 0. Eşdeğer yazımla: işaretli `x` için her K ≥ N - 1'de
`x >> K` = `x >> (N - 1)`.

İşaretsiz (`uN`, `bits<N>`) işlenende `>>` ve her işlenende `<<`, K ≥ N
için 0 verir.

Bu kural bugünkü anlamı yazıya geçirir, değiştirmez. ADR-0036 §1'i,
const-eval.md'nin i128 `>>`'ini ve modelin A12 okumasını bağlayan kuraldır:
üçü de aynı değeri verir. A12 bundan sonra varsayım değil, bu ADR'nin
kuralıdır; fark testinin #302 örneği elle hesaplanmış değer (`pin`) taşır.

### 2. Üretilen SV: sabit K ≥ w için `>>> (w-1)`

İşaretli `>>`'nin sol işleneni SV'de doğal genişliğinde (w) hesaplanır
(ADR-0036, #117). Miktar derleme zamanı sabitiyse ve K ≥ w ise üretici

```systemverilog
a >>> 64     // önce (a : i63)
a >>> 62     // şimdi: w - 1
```

yazar. IEEE 1800-2017 §11.4.10'a göre iki yazım aynı değeri verir: K ≥ w - 1
olan her miktarda bütün bitler işaret bitidir. Sabit, üreticinin sabit
değerlendirmesinin gördüğü her şeydir: soneksiz ve tipli literal, `const`,
döngü değişkeni, generic parametre, bunların aritmetiği ve işaretsiz bir
tipe `as` dönüşümü (`a >> (64 as u8)`; dönüşüm hedefin genişliğine keser).
`w - 1` ondalık, boyutsuz yazılır.

Değişmeyenler: işaretsiz `>>`, `<<`, K < w olan sabit ve çalışma anı
miktarları. Üretilen SV yalnız W2013 alan yerlerde değişir.

**Gerekçe.** w - 1 her Verilator sürümünde cppw'den küçüktür: 5.050–5.052'nin
hatası da, 5.054'te kalan dar hata da bu miktarda oluşmaz (ikisi de K ≥
cppw ister). Hakemi 5.054'e taşımak tek başına yetmezdi: kalan dar hatayı
(`(a + b) >> 130`, `a, b : i64`) ve yerelinde 5.050–5.052 olan kullanıcıları
açıkta bırakırdı. Yosys, Icarus ve sentez iki biçime aynı değeri verir;
`volt verify` değişmez.

Bu biçimi bu ADR tanımlar (AGENTS.md, "Emitter tests test behavior"). Testler
yine davranışı sınar: biçim, sabitlenen Verilator'da (K ≥ cppw'de yanlış
olan hakem) her genişlik sınıfında doğru değeri verdiği için sınanır; metin
karşılaştırılmaz.

### 3. W2013'ün metni

Kod numarası aynı kalır. Kısa açıklama artık sonucu söylemez ("Shift amount
exceeds the width" / "Kaydırma miktarı genişliği aşıyor"); not işlenenin
işaretine göre değişir:

- işaretli `>>`: her bit işaret biti olur; sonuç negatif olmayan değerde 0,
  negatif değerde -1;
- işaretsiz `>>` ve `<<`: bütün bitler dışarı kayar, sonuç hep 0.

`volt explain W2013` iki durumu da anlatır, en ve tr. Yardım satırı
değişmez.

### 4. Sürüm

Düzeltme ADR-0100'ün sessiz yanlış istisnasıyla son hanede, v0.1.3'te
çıkar. Sürüm notunun "Behavior changes" bölümüne girer: işaretli `>>`'nin
K ≥ w sabit miktarı SV'de `>>> (w-1)` yazılır; değer IEEE'ye göre aynıdır,
Verilator 5.050–5.052'de (`volt test`, `volt run`) negatif değerin sonucu
artık 0 değil -1'dir; W2013'ün metni değişir.

## İlke denetimi

1. **Beyan:** uygulanmaz; kural kullanıcıdan bir şey istemez.
2. **Çıkarım:** uygulanmaz; kural bir şey çıkarmaz. SV biçimini seçmek için
   üretici yalnız miktarın sabit değerine bakar (yerel, tek cevaplı).
3. **Anlam değişikliği:** yok. ADR-0036, const-eval.md, IEEE anlamı ve
   model aynı değeri veriyordu; değişen tanı metni ve bir aracın (Verilator
   5.050–5.052) yanlış sonucudur. İ3'ün koşulları ve geçiş uyarısı
   gerekmez; W2013 etkilenen her yerde zaten çıkıyordu.
4. **Niyet tahmini:** uygulanmaz; W2013'ün yeni metni değeri söyler, niyet
   tahmin etmez.
5. **Metamorfik yeniden yazım:** eklenebilir: işaretli `x` için `x >> K` ↔
   `x >> (N - 1)` (K ≥ N - 1), işaretsiz için `x >> K` ↔ `0` (K ≥ N). Bu
   ADR'yle eklenmedi; yeniden yazım W2013'ü kaldırdığından iki yazımın
   tanıları farklıdır ve metamorfik testin tanı karşılaştırması buna göre
   ayarlanmalıdır.

## Sınırlar

- **Volt'ta sabit olmayan, Verilator'da sabitleşen miktar.** Bir portun
  ya da fonksiyon parametresinin sabite bağlandığı örnekte Volt miktarı
  çalışma anı sayar ve `a >>> k` yazar; Verilator modülleri açıp sabiti
  yayarsa aynı hata oluşur: `G { a: a, k: 64 }` ile örneklenen
  `y = a >> k`, 5.052'de negatif değere 0 verir (#309). Volt bunu genel
  olarak göremez. Hakemi 5.054'e taşımak bu sınıfı da kapatır (5.054'te
  kalan dar hata hariç).
- **5.054'te kalan dar hata** Verilator master'ında da vardır (2d6aba6,
  2026-11-16, `-fno-dfg` ve DFG açık); upstream'e bildirilmedi, taslak
  bakımcıdadır.
- **Şartname** kuralı 6f19a248 ile taşır: type-inference.md §3.3
  (kaydırma kuralı: genişlik kadar ya da fazla kaydırmanın değeri),
  const-eval.md (`Shr` kolu) ve sv-mapping.md §6 (sabit K ≥ N için
  `>>> (N-1)`). Cümleleri bakımcı yazdı.

## Reddedilenler

- **Yalnız hakemi 5.054'e taşımak.** Kalan dar hatayı ve yerelinde
  5.050–5.052 olan kullanıcıları açıkta bırakır. Ayrı bir PR'da, kanaryayla
  yapılabilir; bu ADR'nin yerine değil, yanında.
- **W2013'ün cümlesini kural yapmak (sonuç 0).** Üretici sabit K için
  literal 0 yazmalıydı; çalışma anı miktarı için ya her işaretli değişken
  kaydırmaya karşılaştırıcı ve çoklayıcı eklenmeli (`(n >= w) ? '0 :
  (a >>> n)`) ya da sabit ve çalışma anı miktarı aynı değerde farklı sonuç
  verirdi. const-eval.md, ADR-0036 ve IEEE `>>>` ile çelişir.
