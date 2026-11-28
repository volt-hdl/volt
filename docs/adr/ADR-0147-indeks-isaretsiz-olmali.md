# ADR-0147: İndeks İşaretsiz Olmalı — İşaretli Tamsayı Tipli İndeks E2003, Yardım Tabanın Adres Genişliğinde

> Statü: Önerildi
> İlgili: ADR-0146 (aynı kural yeri; Karar 2'nin açık bıraktığı `iN` indeks kuralını bu ADR verir, Trit kuralı değişmez), ADR-0100 (sessiz yanlış düzeltmesi önce kabul edileni reddedebilir; yamada yeni kod yok), ADR-0125 (tanının üç parçası, help'teki kod sınanır), ADR-0137 (kural tek yerde), ADR-0144 (ret tablosuna üç satır), ADR-0120 (kaydırma miktarının genişlik kuralı; Sınırlar), ADR-0035 (parça seçimi), ADR-0117 (gerçek hata derlemi), ADR-0099 (kullanıcı metninde ADR ve spec yolu yok)
> Issue: #475
> Tarih: 2026-11-28
> Etkilenen: `crates/volt-hir/src/typeck/select.rs` (`reject_trit_index` → `reject_index_type`; `address_width`, `signed_index_error`, `trit_index_error`, `IndexForm`), `crates/volt-hir/src/typeck/synth.rs` (yorum), `crates/volt-diagnostics/src/explain/{en,tr}.rs` (E2003 notu), `crates/volt-difftest/src/{reject,model}.rs`, `crates/volt-difftest/tests/reject_tests.rs`, `crates/volt-hir/tests/{signed_index_tests,trit_index_tests,ui_semantic_tests}.rs`, `crates/volt-driver/tests/signed_index_sim_tests.rs` (YENİ), `crates/volt-driver/tests/part_select_wide_start_tests.rs` (işaretli başlangıç E2003), `tests/ui/fail/332`–`344` (YENİ), `tests/real_bugs/475_signed_index.volt` (YENİ), `docs/design/ikili-varsayimlar-envanteri.md`, `book/src/limitations.md`, `CHANGELOG.md`; spec type-inference.md §3.5 için yama bakımcıda

## Sorun

Spec type-inference.md §3.5 indeksin tipini yalnız Trit için söyler
(ADR-0146); ADR-0146 Karar 2 öteki tiplerin kuralını açık bıraktı.
SystemVerilog indeksi işaretsiz bir sayı olarak okur (IEEE 1800
§11.5.1). Tipi işaretli tamsayı olan bir indeks tanısız derleniyordu ve
negatif değeri tasarımın adlandırmadığı bir konumu seçiyordu. v0.1.6
hazırlığında (#477) `volt test` ile Verilator 5.054'te ölçüldü (#475'in
yorumları), `x : u8`, 8 elemanlı diziler, `i : i3 = -1`:

| Biçim | Üretilen SV | i = -1 için okunan |
|---|---|---|
| `x[i]` | `assign b = x[i]` | `x[7]` |
| `TABLE[i]` (const dizi) | `TABLE_at(i)` | `TABLE[7]` |
| `arr[i]` (dizi portu) | `arr[8 * (i) +: 8]` | 7. eleman |

Üçü de sessiz yanlıştır (#475). Literal `x[-1]` zaten E2006 alıyordu;
değeri -1 olan bir sinyal ya da `let` almıyordu.

Bugünkü kabul kümesi main'de (b237311a) ölçüldü: tip denetiminin indekse
verdiği tipi yazan bir probla (geçici worktree'de, kendi target'ında)
`volt check`, taban `x : u8`. Hepsi tanısız kabul ediliyor:

| İndeks | Tip |
|---|---|
| `a + b`, `a - b`, `a + 1`, `1 - a` (a, b : i8) | esnek `i9` (`SIntFlex { lo: 8, hi: 9 }`) |
| `a * b` | esnek `i16` |
| `-a` | `i9` |
| `a >> 1`, `a & b`, `if c { a } else { b }`, `t * a` | `i8` |
| `t as i3`, `t + t` (t : Trit), `a as i3` | `i3` |
| `const K : i8 = 3` ile `x[K]` (SV `x[8'sd3]`, konum doğru) | `i8` |
| `3i8` (sonekli literal) | `i8` |
| `reg d : i3` (ya da `Delayed<i3, 1>`) okunuşu | `i3` |
| `u - v`, `u + v` (u, v : u8) | esnek `u9`, işaretsiz |
| döngü değişkeni `for k in 0..8` | literal |

`const K : i8 = -1` ile `x[K]` E2006'dır (sabit değerlendirilir).

Depoda kullanım: aynı prob gölgenin girdisinde (`tests/fixtures`,
`tests/ui/pass`, `tests/ui/multifile`, `tests/real_bugs`, `examples`,
`templates`, `demo`; 500 dosya, 2660 indeks) ve kitabın 31 `volt`
bloğunda çalıştı: işaretli indeks yok (indeks tipleri uN, esnek uN,
literal, `bits<N>`; tek işaretli kayıt #465'in reddedilen Trit dosyası).
`examples/`, kitap ve `tests/ui/pass` iN indeks kullanmıyor; bir std
kütüphanesi yok. İşaretli bir `const` indeks de depoda geçmiyor. Gölgenin
dışında iki crate testi işaretli indeksi kabul edilen girdi olarak
kullanıyordu: `trit_index_tests::other_index_types_are_unchanged`'in
`i3` satırı (ADR-0146 Karar 2) ve `part_select_wide_start_tests`'in (#282)
esnek `i127`/`i128` başlangıçları (`(si * ti + (-1)) -: 8`, `(x * y) -:
8`). Gölgenin dışındaki 109 `volt` dosyası (`tests/suggestions`,
`tests/fuzz_regressions`, `tests/quickfix`) işaretli indeks kullanmıyor
(iki ikilinin `volt check` kodları aynı).

## Karar

Bakımcının kararı (#475, 2026-11-28): (A) reddet.

### 1. İşaretli tamsayı tipli indeks E2003'tür

Bit ya da dizi indeksi (`x[i]`, `arr[i]`), aralık seçiminin sınırları
(`x[h:l]`) ve parça seçiminin başlangıcı (`x[i +: W]`, `x[i -: W]`),
ifadede de atamanın sol tarafında da, işaretli tamsayı tipinde olamaz:
tipi `iN` ya da esnek `iN` olan indeks E2003'tür. Küme tip arenasının
`is_signed`'ıdır, Trit tutan tip önce ADR-0146'nın kuralına gider. Kural
değere değil tipe bakar (ADR-0146'daki Trit kuralı gibi): işaretli
ifadeler (yukarıdaki tablo), işaretli bir `const` (`const K : i8 = 3`,
değeri doğru okunurdu), sonekli literal `3i8` ve değeri hiç negatif
olmayan bir `iN` sinyal de reddedilir. Negatif değerli işaretli sabit
(`const K : i8 = -1`) E2006 yerine E2003 alır: tip, sınır denetiminden
önce bakılır.

Gerekçe: indeks bir konumdur ve konumun kümesi işaretsizdir; SV de onu
öyle okur. İşaretli bir indeks kaynakta işaretli bir değerdir, donanımda
işaretsiz; aradaki fark tanısız kalıyordu. Kuralın tipe bakması gerekir,
çünkü indeks bir sinyaldir ve değeri derleme zamanında bilinmez.

### 2. Öteki indeks tipleri değişmez

uN, esnek uN, soneksiz literal, `bits<N>`, `bool`, enum ve Trit tutmayan
dizi (`[u1; N]`, `[i8; N]` dahil: bir dizinin tipi işaretli değildir)
bugünkü gibi kabul edilir. `bool`, enum ve dizi indeksi sessiz yanlış
değildir; kuralları v0.2.0'da ayrı bir karardır. İşaretli taban (`x : i8`
için `x[i]`, `ws : [i8; 4]` için `ws[i]`) bu kural değildir: kural
indekse bakar.

### 3. Tanı: E2003, yeni kod yok

ADR-0100: yamada yeni kod yok. ADR-0125'e göre:

- ileti (ne oldu): `an index must be unsigned, found 'i3'`; Trit'inkiyle
  (`an index must not be a Trit, found 'Trit'`, ADR-0146) aynı biçim, o
  ileti değişmez;
- etiket, indeks ifadesinin üstünde: `a negative value of this index
  would select a high bit or element`;
- `reason` (neden): `SystemVerilog reads an index as an unsigned number,
  so a negative value selects a high bit or element without a
  diagnostic`;
- `help` (ne yapılabilir, ADR-0125 §2 (a), kodla): indeksi tabanın adres
  genişliğinde işaretsize açıkça çevirmek; negatif değerin ne olacağını
  da söyler. `x : u8` için `convert the index explicitly to an unsigned
  number of the address width: x[i as u3] for x : u8; a negative value
  then becomes a large index, -1 as u3 is 7`; `[u8; 4]` için `arr[i as
  u2] for arr : [u8; 4]`, `-1 as u2 is 3`.

**Adres genişliği** tabanın konum sayısını (dizide eleman, vektörde bit)
işaretsiz tutan en dar genişliktir, en az 1: `u8` → 3, `u5` → 3, `u9` →
4, `u1` ve `u2` → 1, `[u8; 4]` → 2, `[u8; 5]` → 3, `[u1; 1024]` → 10. İç
içe dizide her indeksin tabanı kendi seviyesidir. Genişliği yardımı
üreten kod tabandan hesaplar (`address_width`), indeksin genişliğinden
değil: daha geniş bir dönüşüm const dizide ve dizi register'ının sol
tarafında `volt check`'ten geçer ama `volt test` Verilator'ın
WIDTHTRUNC'ıyla durur, daha dar bir dönüşüm vektörün sol tarafında
WIDTHEXPAND'la (#474). Yardımın kodu seçimin biçimini izler: `x[i as u3]`
ya da `arr[i as u2]`, parça seçiminde `x[i as u3 +: W]` / `x[i as u3 -:
W]`, aralıkta reddedilen sınır (`x[h as u3 : l]`, `x[h : l as u3]`).

İşaret değişimi (`iN as uM`) her genişlikte uyarısızdır (§3.6); indeks
adresten genişse dönüşüm üst bitleri keser, darsa işaretle doldurur
(`-1 as uM` her M için 2^M − 1; Verilator'da ölçüldü). Dönüşüm bugünkü
SV okumasını korur, kaynakta yazılı olarak: aynı genişlikte i = -1 için
`x[i as u3]` yine `x[7]`'yi okur.

Makine önerisi yoktur: negatif değerin hangi konumu seçeceği, yani
sarmalı okuma mı, bir sınır denetimi mi, baştan `uN` bir indeks mi,
tasarımcının niyetidir (İ4); yardım bir yolu kodla gösterir, kullanıcı
yazar. `volt explain E2003`'ün notu iki dilde bir paragraf alır.

Trit'in yardımı değişmez (`x[(t as i3 + 1) as u3]`, ADR-0146 Karar 3);
onun genişliğini adrese bağlamak bakımcının seçeneğidir (Sınırlar).

### 4. Kuralın tek yeri

Kural ADR-0146'nın fonksiyonundadır; fonksiyonun adı
`reject_trit_index`'ten `reject_index_type`'a döner, çünkü artık
indeksin tipini denetleyen tek yerdir (ADR-0137): Trit tutan tip
(ADR-0146) ya da işaretli tamsayı, ikisi de E2003. Çağıranlar
değişmez: `index_result` (bit ve dizi indeksi), `range_result` (iki
sınır) ve `part_select_result` (başlangıç); ifade (`synth.rs`
`synth_select`) ve atama hedefi (`stmt.rs` `lvalue_type`) aynı yoldan
geçer. Fonksiyon artık tabanın tipini ve seçimin biçimini de alır
(`IndexForm`): yardımın adres genişliği ve kodu onlara bağlıdır.

Adresi olmayan bir tabanda (tipi hata olan ya da seçilemeyen: `bool`
tabanda bit seçimi, dizide aralık ya da parça seçimi) işaretli indeks
reddedilmez: tabanın kendi tanısı çıkar ve seçim hatadır, ikinci tanı
yoktur. Sessiz yanlış kalmaz, çünkü tasarım zaten reddedilir. Trit
kuralının sırası değişmez: o tabandan önce bakar.

İndeks reddedilince seçimin tipi hatadır; zincirleme tanı çıkmaz. İki
sınırı da işaretli olan aralık iki E2003 alır. Üretici işaretli indeksi
ayrıca denetlemez: reddedilen tasarım üreticiye ulaşmaz.

### 5. Fark testi

ADR-0144'ün ret tablosu (`reject::UNDEFINED`) üç satır alır: `iN: x[a]`,
`iN: x[a +: W]`, `iN: x[a -: W]`, beklenen E2003; işlenen üretecin `iN`
ifadeleridir (port, dönüşüm, aritmetik, iç içe), taban `uN` ya da
`bits<N>` portu. Modelin `Unspecified("signed index")` ve
`Unspecified("signed part-select start")` okuması E2003 olur; parça
başlangıcında derleyici gibi genişlikten önce. Üreteç geçerli program
olarak işaretli indeks üretmez (indeks ve başlangıç `uN` ya da literal);
yeni satırlar beklenen rettir.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; yeni yazım istenmez. `i as uN` zaten
   dildedir (§3.6).
2. **Çıkarım (İ2):** indeksin tipi tek cevaplıdır; kural tip denetiminin
   bir ifadeye verdiği tipi okur.
3. **Anlam değişikliği (İ3):** önce kabul edilen girdi reddedilir: tipi
   işaretli tamsayı olan her indeks, değeri hiç negatif olmayan doğru
   programlar dahil (işaretli bir `const` indeks, hep pozitif bir `iN`
   sinyal). Gerekçe ADR-0100'ün sessiz yanlış istisnası ve tipe bakan
   kuraldır: önceki çıktı negatif değerde tasarımın adlandırmadığı bir
   konumu tanısız okuyor ve yazıyordu, ve bir sinyalin değeri derleme
   zamanında bilinmediğinden yanlış okuyan programı doğru okuyandan
   ayıracak bir kural yoktur. CHANGELOG'da "Davranış değişikliği"
   maddesi: kimi etkiler, ne yapmalı (yardımın yolu). Kabul edilen başka
   hiçbir girdinin çıktısı değişmez (Doğrulama). Negatif değerli
   işaretli sabitin kodu E2006'dan E2003'e döner.
4. **Niyet tahmini (İ4):** derleyici indeksi kendisi işaretsize çevirmez;
   negatif değerin ne olacağı tasarımcınındır, yardım bir yolu gösterir.
5. **Metamorfik yeniden yazım:** uygulanmaz; reddedilen girdinin eşdeğer
   bir biçimi yoktur.
6. **ADR-0139 denetimi:** uygulanmaz; kural değer kümesine (konum
   işaretsizdir) bakar, Trit'in kodlamasına dokunmaz; "Trit'in temsilini
   bilen yerler" listesi büyümez. Envanterin `select.rs` satırı yeni
   fonksiyon adıyla güncellenir.

## Doğrulama

- Önce kırmızı (main = b237311a, kaynak değişmeden, yeni testler
  kopyalandı): `signed_index_tests`'in 20 testinin 15'i düşer, işaretli
  indeks tanısız kabul edildiği için; geçen 5'i bugünkü davranışı
  sabitler (öteki indeks tipleri, işaretli taban, adresi olmayan taban,
  yardımın kodu, generic `const S : i8`). `ui_fail_332_344_signed_index_e2003`,
  `signed_index_sim_tests` ve `reject_tests`'in iki yeni denetimi düşer;
  on üç `tests/ui/fail` dosyası ve `tests/real_bugs/475_signed_index.volt`
  main'in `volt check`'inde tek tek `0 error(s), 0 warning(s)` verir. Ret
  kipi (`volt-difftest-reject`, tohum 458, satır başına 8) main'in
  ikilisiyle 24 işaretli indeks örneğini kabul eder (`diffs=24`). Sonra
  hepsi geçer, ret kipi `diffs=0`.
- Yardımın kodu (ADR-0125 §4): `signed_index_tests` onu bit seçiminde,
  dizi portunda, const dizide, parça seçiminde (`+:`, `-:`), aralık
  sınırında ve sol tarafta (dizi ve vektör register'ı) indeksin yerine
  koyar, indeks adresten geniş (i8), eşit (i3) ve dar (i2) iken; tanı
  çıkmaz. `signed_index_sim_tests` yardımı `volt check`'in kendi
  çıktısından alır (`x : u8` için `i as u3`, `TABLE : [u8; 4]` için `i as
  u2`), indekslerin yerine koyar, tasarım tanısız geçer ve Verilator'da
  i = -1, 2, -3 için x = 0b1000_0100'ün x[7], x[2], x[5]'ini (1, 1, 0),
  TABLE[3], TABLE[2], TABLE[1]'i (40, 30, 20) okur ve `y[i as u3] =
  true` ile y = 128, 4, 32 yazar (değerler spec'ten elle; Docker'da
  Verilator 5.054 ile `3 passed`). Adresten geniş indeks (i : i8) ve dizi
  register'ının sol tarafı (`wa[i as u2] <= 99`, `r[i as u3] <= true`)
  ayrıca elle Verilator'da denendi: WIDTHTRUNC ya da WIDTHEXPAND yok,
  değerler doğru.
- Çıktı karşılaştırması (ADR-0136'nın yöntemi): main ile dalın ikilileri
  gölgenin girdisinde (`examples/` dahil 500 dosya) ve `tests/ui/fail`'de
  (333 dosya) `volt build --single-file --emit=sva,sdc` ve `volt check`:
  833 dosyanın 819'unda çıkış kodu, üretilen dosyalar (sürüm satırı
  dışında) ve tanı kodları aynı; 14 fark yeni dosyaların (on üç
  `tests/ui/fail`, bir `tests/real_bugs`) yeni E2003'leridir. Kitabın 31
  `volt` bloğu iki ikiliyle aynı sonucu verir.
- Düzeltilen iki test (kabul ettikleri girdi artık E2003):
  `trit_index_tests::other_index_types_are_unchanged`'den `i3` satırı
  çıkar (ret `signed_index_tests`'te). `part_select_wide_start_tests`'in
  (#282) işaretli 127 ve 128 bitlik başlangıçları aynı genişlikte
  `bits<N>` seçimine döner (`(si * ti + (-1))[126 : 0]`, `(x * y)[127 :
  0]`; değerler pozitif, bitleri ve elle hesaplanan beklentiler aynı;
  Docker'da Verilator 5.054 ile `2 passed`); işaretli biçimin iki E2003'ü
  yeni `a_signed_wide_start_is_e2003` testinde.
- Gerçek hata derlemi: `tests/real_bugs/475_signed_index.volt`
  (`//~ E2003`).

## Sınırlar

- **Negatif değerli işaretli sabit:** E2003 alır (Karar 1). Yardımın kodu
  uygulanınca (`x[K as u3]`, `const K : i8 = -1`) sabit değerlendirme
  dönüşümü uygulamaz (`consteval.rs`: "sabit değer cast'te değişmez") ve
  değeri -1 sayar: E2006. Açık bir hatadır, sessiz yanlış değil; donanım
  `K as u3`'ü 7 okurdu. Nedeni #422'nin 1. ve 2. satırlarıyla aynıdır
  (sabit değerlendirmede `as` kesmez); bu ADR onu değiştirmez. Sabit
  indeksin doğru yolu onu `uN` tanımlamaktır.
- **Generic `const S : i8`:** ayrıştırıcı parametreyi literale çevirir
  (ADR-0041); `x[S]` örnekte `x[3]`'tür ve tip denetimi S'nin işaretli
  olduğunu görmez. Değer bugün doğrudur (`P<3>` ölçüldü; `P<-1>` bugün
  ayrıştırılmaz, E0010). Kural uygulanmaz;
  `a_const_generic_signed_index_is_a_literal_and_is_accepted` bugünkü
  davranışı sabitler. Parametrenin tipi tip denetimine ulaştığında
  (#468) kural kendiliğinden uygulanır.
- **Kaydırma miktarı** (`x >> i`, `x << i`) indeks değildir ve bu ADR onu
  değiştirmez. Ölçüldü (main, `volt test`, Verilator 5.054): SV miktarı
  işaretsiz okur; i : i3 = -1 için `x >> i` = `x >> 7`, `x << i` = `x <<
  7`, işaretli `s >> i` = `s >>> 7`; i : i4 = -1 için miktar 15'tir ve u8
  sonuç 0, işaretli sonuç işaret bitleri olur (ADR-0120). Spec negatif
  miktarı söylemez (fark testinin modeli `Unspecified("signed shift
  amount")` der); aynı -1 değeri miktarın genişliğine göre farklı sonuç
  verir. Açık karar; bakımcıya issue metni önerildi.
- **`bool`, enum ve dizi indeksi:** değişmez (Karar 2); v0.2.0'da ayrı
  karar (bakımcıya issue metni önerildi).
- **Test dilinde indeks** (`v[dut.o]`): kapsam dışı; test dili portu bit
  deseni olarak okur. Tasarımın tip denetimi test diline uygulanmaz.
- **Trit'in yardımı** `u3` indeks verir (ADR-0146 Sınırlar); adresi 3
  bitten dar bir const dizide ya da dizi register'ının sol tarafında
  `volt test`'te WIDTHTRUNC'la durur (#474). Aynı `address_width` onu da
  bağlayabilir, ama adresi 1 bit olan tabanda (`[u8; 2]`) Trit'in üç
  değeri iki konuma sığmaz ve yardımın sözü ("reads x[0], x[1] and x[2]")
  yanlış olur. Bu ADR Trit'in yardımını değiştirmez; bakımcının
  seçeneğidir.
- Tip generic'i (`module M<T>`, `M<i8>`) bugün E0003'tür; açıldığında
  indeks örneklenen tipte aynı kurala gider.

## Reddedilenler

- **Negatif indeksi aralık dışı saymak** (çalışma zamanında `x[i]`,
  i < 0 iken X ya da 0). Aralık dışı okumanın donanımdaki sonucu
  belirsizdir (benzetimde X, sentezde araca bağlı); sessiz yanlış
  kalkmaz, yalnız biçim değiştirir, ve üretilen SV bir karşılaştırma ve
  çoklayıcı kazanır.
- **Değere bakan kural** (yalnız negatif olabilen indeksi reddetmek).
  İndeks bir sinyaldir; değeri derleme zamanında bilinmez. Sabit indeks
  zaten değeriyle denetlenir (E2006).
- **İşaretli indeksi işaretsiz okumayı dilde tanımlamak** (bugünkü SV'yi
  kural yapmak). Kaynak işaretli bir değer gösterir, donanım başka bir
  konumu seçer; fark yine tanısız kalır. Bir yamada yeni anlam eklenmez
  (ADR-0100).
- **Makine önerisi** (`i` → `i as u3`, `MaybeIncorrect`). Negatif
  değerde sarmalı okumayı seçmek tasarımcının niyetidir (İ4); yardım
  yolu gösterir.
- **Üreticide denetlemek.** Kural iki yerde hesaplanırdı (ADR-0137).
