# ADR-0146: İndeks Trit Olamaz — Trit Tutan İndeks E2003, Öteki İndeks Tipleri Değişmez

> Statü: Uygulandı — 2026-11-28
> İlgili: ADR-0003 (Trit'in değer kümesi {-1, 0, +1}), ADR-0062 (Trit'in iki bitlik kodu; indeks önce bu kodu okuyordu), ADR-0142 ("işlem değer kümesine aittir"; `holds_trit` kümesi bu ADR'de de kullanılır), ADR-0100 (sessiz yanlış düzeltmesi önce kabul edileni reddedebilir; yamada kural değişikliği yok), ADR-0125 (tanının üç parçası, help'teki kod sınanır), ADR-0137 (kural tek yerde), ADR-0139 (değer kümesi ile kodlama ayrımı), ADR-0144 (Sınırlar'daki "Trit ile indeksleme" maddesi kapanır; ret tablosuna üç satır), ADR-0035 (parça seçimi), ADR-0117 (gerçek hata derlemi)
> Issue: #465
> Tarih: 2026-11-28
> Etkilenen: `crates/volt-hir/src/typeck/select.rs` (`reject_trit_index`; `index_result`, `range_result`, `part_select_result` onu çağırır), `crates/volt-hir/src/typeck/synth.rs` (`holds_trit` paylaşılır; `synth_select` aralık sınırlarını sentezler), `crates/volt-hir/src/typeck/stmt.rs` (`lvalue_type` aralık sınırlarını sentezler), `crates/volt-diagnostics/src/explain/{en,tr}.rs` (E2003 notu), `crates/volt-difftest/src/{reject,model}.rs`, `crates/volt-difftest/tests/{reject_tests,trit_model_tests}.rs`, `crates/volt-hir/tests/{trit_index_tests,ui_semantic_tests}.rs`, `crates/volt-driver/tests/trit_index_sim_tests.rs` (YENİ), `tests/ui/fail/319`–`331` (YENİ), `tests/real_bugs/465_trit_index.volt` (YENİ), `docs/design/ikili-varsayimlar-envanteri.md`, `book/src/limitations.md`, `CHANGELOG.md`; spec type-inference.md §3.5 için yama bakımcıda

## Sorun

Spec type-inference.md §3.5 indeksin tipini söylemez; tip denetimi de ona
bakmıyordu: `select.rs` `index_result` ve `part_select_result` yalnız
tabanın tipine ve sabit indeksin sınırına bakıyordu. Kaydırma miktarı
ise sayısal olmalıdır (`binop.rs` `synth_shift`): `x >> t` E2003'tür.
`t : Trit` için `x[t]` tanısız derleniyordu. Üretici indeksi Trit'in iki
bitlik kodu (+1 = `01`, 0 = `00`, -1 = `11`, ADR-0062) üzerinden
yazıyordu ve kod işaretsiz okununca -1, 3 oluyordu. main'de (4608a47d)
`volt test` ile Verilator 5.054'te ölçüldü:

| Biçim | Üretilen SV | t = -1 için okunan |
|---|---|---|
| `x[t]` (x : u8) | `1'((x) >> t)` | `x[3]` |
| `TABLE[t]` (const dizi) | `TABLE_at(t)` | `TABLE[3]` = 40 |
| `ra[t]` (register dizisi) | `ra[t]` | `ra[3]` |
| `x[t +: 2]` | `2'((x) >> t)` | `x[4:3]` |
| `x[t -: 2]` | `(x) >> (t - 2'd1)` | `x[3:2]` |
| `wa[t] <= 99` (sol taraf) | `wa[t] <= 99` | `wa[3]`'e yazar |

Altısı da sessiz yanlıştır (#465). Aynı kural fn gövdesinde, kontratta
(`assert: x[t] == b`), dizi portunda (`arr[8 * (t) +: 8]`), vektör sol
tarafında (`r[t] <= true`, `comb` içinde `y[t] = true`) ve Trit tipli
her ifadede (`-t`, `t * u`, `if c { t } else { -t }`, Trit dizisinin
elemanı, struct alanı) kabul ediliyordu. Vektör sol tarafı bugün
`volt test`'te Verilator'ın WIDTHEXPAND'ıyla duruyor; aynı duruş `u2`
indeksle de oluyor, Trit'e özgü değil.

Bugünkü kabul kümesi ölçüldü (`volt check`, taban `x : u8`, `arr : [u8;
4]`): uN, iN, esnek (`a + b`), literal, `bits<N>`, `bool`, enum ve dizi
(`[u1; 2]`, `[Trit; 2]`) indeks olarak kabul ediliyor; struct E0003,
`clock` E3010 (başka nedenle). Gölgenin girdisinde (499 dosya, kitabın
blokları) indeks tipi olarak yalnız uN, literal ve `bits<N>` geçiyor;
`bits<N>` iki pass testinde: `tests/ui/pass/41_dynamic_array_index.volt`
ve `42_indexed_part_select.volt`. `examples/` ve kitap Trit indeksi
kullanmıyor.

## Karar

### 1. Trit tutan indeks E2003'tür

Bit ya da dizi indeksi (`x[i]`, `arr[i]`), aralık seçiminin sınırları
(`x[h:l]`) ve parça seçiminin başlangıcı (`x[i +: W]`, `x[i -: W]`),
ifadede de atamanın sol tarafında da, Trit tutan bir tipte olamaz:
tipi `Trit` ya da elemanı Trit tutan bir dizi (iç içe dahil) olan indeks
E2003'tür. Küme ADR-0142'nin `holds_trit`'idir. Kural değere değil tipe
bakar: `-t`, Trit çarpımı ve dalları Trit olan `if` Trit'tir; değeri
doğru okunacak bir Trit sabiti (`const K : Trit = 1`, `x[K : 0]`) de
reddedilir.

Gerekçe ADR-0142'ninkidir: bir işlem bir tipin değer kümesine aittir.
Trit'in kümesi {-1, 0, +1}'de bit ya da eleman konumu yoktur; -1 bir
konum değildir. İndeks onu kodlamasından okuyordu, bu da kodlamanın dile
sızmasıdır (ADR-0139). Kaydırma miktarı aynı nedenle zaten E2003'tü.

### 2. Öteki indeks tipleri değişmez

uN, iN, esnek, literal, `bits<N>`, `bool`, enum ve Trit tutmayan dizi
(`[u1; N]`) bugünkü gibi kabul edilir; 41 ve 42 numaralı pass testleri
değişmez. Bu tiplerin indeks kuralı ayrı bir karar konusudur (açık
karar; bakımcı issue'yu açar). Bu ADR onu belirlemez.

### 3. Tanı: E2003, yeni kod yok

ADR-0100: yamada yeni kod yok. ADR-0125'e göre:

- ileti (ne oldu): `an index must not be a Trit, found 'Trit'` (dizide
  `'[Trit; 2]'`); kaydırmanın iletisiyle (`shift amount must be numeric,
  found 'Trit'`) aynı biçim;
- etiket, indeks ifadesinin üstünde: `the index would read the bits that
  encode the Trit, not its value`;
- `reason` (neden): `a Trit is -1, 0 or +1 and -1 is no bit or element
  position; an index reads the bits that encode the Trit as an unsigned
  number`; bit desenini yazmaz (ADR-0139 §2);
- `help` (ne yapılabilir, ADR-0125 §2 (a), kodla): `convert the Trit to
  an unsigned index explicitly: x[(t as i3 + 1) as u3] reads x[0], x[1]
  and x[2] for t = -1, 0 and +1`.

Makine önerisi yoktur: `-1, 0, +1`'i hangi konumlara eşleyeceği
tasarımcının niyetidir (İ4); yardım bir yolu kodla gösterir, kullanıcı
yazar. `volt explain E2003`'ün notu iki dilde bir paragraf alır.

### 4. Kuralın tek yeri

Kural tip denetimindedir, tek fonksiyonda: `select.rs`
`reject_trit_index`. Bütün indeks yolları onu çağırır: `index_result`
(bit ve dizi indeksi), `range_result` (iki sınır) ve
`part_select_result` (başlangıç). Bu üçünün çağıranları ifade
(`synth.rs` `synth_select`) ve atama hedefidir (`stmt.rs`
`lvalue_type`); const dizi, register dizisi, dizi portu, fn gövdesi ve
kontrat aynı yoldan geçer, ayrı kod yoktur. Fonksiyon indeksin tipini
tip denetiminin tablosundan okur; çağıranlar onu sentezler. Aralık
sınırları önceden sentezlenmiyordu (yalnız sabit olarak
değerlendiriliyordu); şimdi iki çağıran da sentezler. Çıktı
karşılaştırması bunun kabul edilen hiçbir girdinin tanısını ve
çıktısını değiştirmediğini gösterir (Doğrulama).

İndeks reddedilince seçimin tipi hatadır; zincirleme tanı çıkmaz.
Üretici Trit indeksini ayrıca denetlemez: reddedilen tasarım üreticiye
ulaşmaz (ADR-0137).

### 5. Fark testi

ADR-0144'ün ret tablosu (`reject::UNDEFINED`) üç satır alır: `Trit: x[a]`,
`Trit: x[a +: W]`, `Trit: x[a -: W]`, beklenen E2003; işlenen üretecin
Trit ifadeleridir (port, `-t`, `t * u`, iç içe), taban `uN` ya da
`bits<N>` portu. Model aynı kuralı söyler (`model.rs`: indeks ve parça
başlangıcı Trit ise E2003; derleyici gibi parça genişliğinden önce).
Üreteç geçerli program olarak Trit indeksi üretmez. `bool`, enum ve dizi
indeksi tabloya girmez (Karar 2).

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; yeni yazım istenmez. `t as i3` zaten
   dildedir (§3.6).
2. **Çıkarım (İ2):** indeksin tipi tek cevaplıdır; kural tip
   denetiminin bir ifadeye verdiği tipi okur.
3. **Anlam değişikliği (İ3):** önce kabul edilen girdi reddedilir: tipi
   Trit tutan her indeks. Bu bir sessiz yanlış düzeltmesidir ve
   ADR-0100'ün istisnasına girer: önceki çıktı tanısız olarak tasarımın
   adlandırmadığı bir konumu okuyor ve yazıyordu. CHANGELOG'da
   "Davranış değişikliği" maddesi: kimi etkiler, ne yapmalı (yardımın
   yolu). Kabul edilen başka hiçbir girdinin çıktısı değişmez
   (Doğrulama). Değeri doğru olan bir biçim de reddedilir: Trit tipli bir
   `const` sınır; kural tipe bakar.
4. **Niyet tahmini (İ4):** derleyici Trit'i bir indekse kendisi çevirmez;
   eşleme tasarımcınındır, yardım bir yolu gösterir.
5. **Metamorfik yeniden yazım:** uygulanmaz; reddedilen girdinin eşdeğer
   bir biçimi yoktur.
6. **ADR-0139 denetimi.**
   - *Değer kümesi ile kodlama ayrımı:* kural değer kümesine bakar
     ("Trit'te konum yok"); kodlamayı okumaz ve yazmaz. Tanı ve explain
     metni bit desenini yazmaz; "Trit'in temsilini bilen yerler" listesi
     büyümez.
   - *Kodlamanın dilden görünmesi:* bir yolu kapanır: indeks artık
     Trit'in kodunu okumaz. `p as uN` ile struct içindeki Trit'in kodunun
     görünmesi (ADR-0077 Karar 3) değişmez.
   - *Tek yer:* kural `reject_trit_index`'tedir, küme `holds_trit`'ten;
     üretici, test dili, dalga biçimi ve formal yol onu yeniden
     hesaplamaz.
   - *Envanter:* `select.rs` satırı güncellenir.

## Doğrulama

- Önce kırmızı (main = 4608a47d, kaynak değişmeden): `trit_index_tests`
  17 testinin 13'ü düşer, Trit indeksi tanısız kabul edildiği için; geçen 4'ü
  bugünkü davranışı sabitler (öteki indeks tipleri, Trit taban, generic
  `const S : Trit`, yardımın kodu). `ui_fail_319_331_trit_index_e2003`
  düşer; on üç `tests/ui/fail` dosyası ve `tests/real_bugs/465_trit_index.volt`
  main'in `volt check`'inde tek tek `0 error(s), 0 warning(s)` verir.
  Ret kipi (`volt-difftest-reject`, tohum 458, satır başına 8) main'in
  ikilisiyle 24 Trit indeksi örneğini kabul eder (`diffs=24`). Sonra hepsi
  geçer, ret kipi `diffs=0`.
- Yardımın kodu (ADR-0125 §4): `trit_index_tests` onu bit seçiminde,
  dizi portunda, parça seçiminde ve sol tarafta indeksin yerine koyar,
  tanı çıkmaz. `trit_index_sim_tests` yardımı `volt check`'in kendi
  çıktısından alır, `x[t]` ve `TABLE[t]`'nin yerine koyar, tasarım
  tanısız geçer ve Verilator'da x = 5 için t = -1, 0, +1'de x[0..2] =
  1, 0, 1 ve TABLE[0..2] = 10, 20, 30 okunur (değerler spec'ten elle;
  tabanların adresi 3 bit, Docker'da Verilator 5.054 ile `3 passed`).
  Reddedilen seçimin tipi hatadır: uyumsuz bir hedefte ikinci tanı
  çıkmaz, iki sınırı da Trit olan aralık iki E2003 alır
  (`a_rejected_selection_has_the_error_type`).
- Çıktı karşılaştırması (ADR-0136'nın yöntemi): main ile dalın ikilileri
  gölgenin girdisinde (`examples/` dahil 500 dosya) ve `tests/ui/fail`'de
  (319 dosya) `volt build --single-file --emit=sva,sdc` ve `volt check`:
  819 dosyanın 805'inde çıkış kodu, üretilen dosyalar ve tanı kodları
  aynı; 14 fark yeni dosyaların (on üç `tests/ui/fail`, bir
  `tests/real_bugs`) yeni E2003'leridir. Kitabın 31 `volt` bloğu dalın
  ikilisiyle derlenir.
- Gerçek hata derlemi: `tests/real_bugs/465_trit_index.volt` (`//~ E2003`).

## Sınırlar

- **Generic `const S : Trit`:** ayrıştırıcı parametreyi literale çevirir
  (ADR-0041); `x[S]`, `x[S + 1 : S]`, `x[S +: 2]` örnekte `x[1]`,
  `x[2:1]`, `x[1 +: 2]`'dir ve tip denetimi S'nin Trit olduğunu görmez.
  `~S`'de (ADR-0142) bağlamın Trit tipi literale geçiyordu; indekste
  bağlam yoktur. Değer bugün doğrudur (`G<1>` ölçüldü; `G<-1>` bugün
  ayrıştırılmaz, E0010). Kural uygulanmaz;
  `a_const_generic_trit_index_is_a_literal_and_is_accepted` bugünkü
  davranışı sabitler. Parametrenin tipi tip denetimine ulaştığında
  (#468) kural kendiliğinden uygulanır.
- **Test dilinde indeks** (`v[dut.o]`, `sim_expr.rs`): kapsam dışı. Test
  dili bir portu bit deseni olarak okur (o : Trit = -1 için 3); `v[3]`
  bu kurala uygundur. Tasarımın tip denetimi test diline uygulanmaz.
- **Parça genişliği** (`x[0 +: W]`) indeks değildir: derleme zamanı
  sabitidir ve değeri denetlenir (E2008, E2006).
- **Öteki indeks tipleri** (`bool`, enum, dizi, `iN`) açık karardır
  (Karar 2). Enum indeksi de kodlamayı okur (varyantın kodu); `iN`'nin
  negatif değeri tanımsızdır (fark testinin modeli `Unspecified("signed
  index")` der).
- Tip generic'i (`module M<T>`, `M<Trit>`) bugün E0003'tür; açıldığında
  indeks örneklenen tipte aynı kurala gider.
- Yardımın kodu `u3` indeks verir. Adresi 3 bitten dar bir const dizide
  ya da dizi register'ının sol tarafında (`[u8; 4]`) bu kod `volt check`'ten
  geçer, ama `volt test` Verilator'ın WIDTHTRUNC'ıyla durur. Bu Trit'ten
  bağımsızdır: main'de `u3` port indeksiyle de aynı (#134'ün değişken
  indeksli kardeşi; bakımcıya önerildi). Adres 2 bitse `as u2` yazılır.

## Reddedilenler

- **(a) Kaydırmanın kümesini almak** (`binop.rs` `synth_shift`: literal,
  uN, iN, esnek). `bits<N>`, `bool`, enum ve dizi indeksini de reddeder.
  `bits<N>` indeks iki pass testinde (`41_dynamic_array_index`,
  `42_indexed_part_select`) kullanılıyor; bu girdiler kabul ediliyor ve
  yanlış değer vermiyor. Yanlış değer vermeyen girdiyi reddetmek yama
  sürümünde bir kural değişikliğidir (ADR-0100); sessiz yanlış istisnası
  onu kapsamaz.
- **(b) Kaydırmanın kümesi artı `bits<N>`.** 41 ve 42'yi korur, ama
  `bool`, enum ve dizi indeksini reddeder; aynı gerekçeyle bir yamada
  kural değişikliğidir. Bu tiplerin kuralı ayrı karardır (Karar 2).
- **Yalnız değeri yanlış okunan durumu reddetmek** (`t = -1`). Kural
  değere bağlı olurdu; indeks bir sinyal olduğunda değeri derleme
  zamanında bilinmez.
- **Trit indeksi bir eşlemeyle tanımlamak** (ör. -1, 0, +1 → 0, 1, 2).
  Dil tasarımı kararıdır; bir yamada yeni anlam eklenmez (ADR-0100) ve
  hangi eşlemenin doğru olduğu tasarıma bağlıdır.
- **Üreticide denetlemek.** Kural iki yerde hesaplanırdı (ADR-0137).
