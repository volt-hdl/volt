# ADR-0142: `~` Trit Üzerinde Tanımlı Değildir — İşlem Değer Kümesine Aittir, Trit'in Tersi `-t`

> Statü: Uygulandı — 2026-11-26
> İlgili: ADR-0003 (Trit'in değer kümesi {-1, 0, +1}; `-Trit` Trit'tir), ADR-0062 (Trit'in iki bitlik kodu; `~` önce bu kod üzerinde çalışıyordu), ADR-0139 (envanterin "Açık bulgular" 1'i kapanır; "işlem değer kümesine aittir" ilkesinin ilk uygulaması), ADR-0144 (beklenen ret ölçütü bu ADR'nin Trit satırlarını her işte sınar; v0.1.3'te #434'ü bulur), ADR-0148 (generic `const S : Trit` tip denetiminde Trit: `~S`'nin E2003'ü işlenenin tipinden)
> Issue: #434
> Tarih: 2026-11-26
> Etkilenen: `crates/volt-hir/src/typeck/synth.rs` (`bit_not_result`, `holds_trit`, `check_literal_bit_not`, `literal_bit_not`), `crates/volt-hir/src/typeck/check.rs`, `crates/volt-diagnostics/src/explain/{en,tr}.rs` (E2003 notu), `docs/design/ikili-varsayimlar-envanteri.md`, `book/src/limitations.md`, `CHANGELOG.md`; spec type-inference.md §3.4 için yama bakımcıda

## Sorun

Spec `~x`'in tipini her tipte korur (type-inference.md §3.4:
`UnOp::BitNot => ot`); tip denetimi böyle uyguluyordu. `t : Trit` için
`~t` Trit tipliydi ve tanısız derleniyordu. Ama `~` değeri değil kodu
tersler: Trit'in kodu +1 = `01`, 0 = `00`, -1 = `11`'dir (ADR-0062). main'de
(ad6b0d4d) `volt test` ile Verilator'da `(~t) as i2`:

| t | kod | `~kod` | Trit olarak okunan |
|---|---|---|---|
| +1 | `01` | `10` | -2 (değer kümesinin dışında) |
| 0 | `00` | `11` | -1 |
| -1 | `11` | `00` | 0 |

Sonraki Trit aritmetiği yanlış bir değerle çalışır: bir ternary MAC'te
`(~w) * x`, w = +1 için `-2 * x` toplar. Bu sessiz bir yanlıştır (#434).
ADR-0139'un envanteri bunu okuyarak bulmuştu ("Açık bulgular" 1) ve
çözümü bakımcının kararına bırakmıştı.

`~` Trit'e başka yollardan da ulaşıyordu: Trit tutan dizinin kendisi
(paketlenmiş vektörün `~`'si her elemanın kodunu tersler, ADR-0056),
bağlamdan Trit tipini alan literal (`y = ~1` `~2'sd1` yazılıyordu, yani
`10`; `-(~1)`, `~1 | 0`, `t * ~1` de) ve ayrıştırıcının literale çevirdiği
generic `const S : Trit` parametresi (`~S`).

## Karar

### 1. `~` Trit'te tanımlı değildir

`~` bitleri tersler ve işlenenin tipini korur: bool, uN, iN, bits<N> ve
bunların dizileri. Trit'te ve Trit tutan dizide (iç içe dahil) tanımlı
değildir: E2003. Kural değere değil tipe bakar: `~0`'ın kodu (`11`, -1)
bir Trit değeri olsa da reddedilir. İşlem Trit'in değer kümesi üzerinde
bir anlam taşımaz; Trit'in tersi `-t`'dir (ADR-0003: `-Trit` Trit'tir,
-(+1) = -1, -0 = 0, -(-1) = +1).

Bu, ADR-0139 §1'in "değer kümesi gereği ikili olan şey ikili kalır"
ayrımının Trit tarafıdır: bir işlem bir tipin değer kümesine aittir; ikili
bir işlem bir değer kümesinin kodlamasına sızmaz.

### 2. Tanı: E2003, yeni kod yok

Struct ve enum işlenenleri bugün aynı biçimde reddedilir ("operator '~'
is not defined for struct ..."); Trit aynı kodu alır (ADR-0100: yamada
yeni kod yok). ADR-0125'e göre:

- ileti (ne oldu): `operator '~' is not defined for 'Trit'` (dizide
  `'[Trit; 2]'`);
- etiket: `'~' inverts the bits that encode a Trit, not its value`;
- `reason` (neden): `a Trit is -1, 0 or +1; inverting the bits that
  encode +1 gives a pattern that is none of the three values`;
- `help` (ne yapılabilir, ADR-0125 §2 (a), kodla): Trit'te `use '-' for
  ternary negation: write -t instead of ~t; '-' maps +1 to -1, 0 to 0 and
  -1 to +1`; dizide `-` dizinin kendisinde tanımlı olmadığı için eleman
  eleman, dizinin gerçek uzunluğuyla: `[Trit; 2]` için `for i in 0..2 {
  y[i] = -a[i] }`; iç içe dizide her boyut bir döngü (`i`, `j`, ...).

Makine önerisi yoktur: `~a`'yı `-a`'ya çeviren öneri dizide yanlış olur
ve bir literal bağlamında (`~1`) niyet belli değildir.

`volt explain E2003`'ün notu iki dilde bir paragraf alır: bir operatör
yalnız bazı tiplerde tanımlıdır, `~` Trit'te E2003'tür, `-t` kullanılır.

### 3. Kuralın tek yeri

Kural tip denetimindedir, tek fonksiyonda:
`crates/volt-hir/src/typeck/synth.rs` `bit_not_result` (`~`'nin
tanımlı olduğu tipler) ve `holds_trit`. İki çağıranı vardır, ikisi de
yalnız işlenenin tipini verir:

- sentez (`synth_unary`): işlenenin tipi;
- check kipi (`check.rs` `check` sonu → `check_literal_bit_not`):
  bağlamdan tipini alan literal ağacındaki `~`'nin işleneni o bağlamın
  tipindedir (üretici onu o genişlikte yazar).

SV üreticisi `~`'yi Trit için ayrıca denetlemez; reddedilen tasarım
üreticiye ulaşmaz (ADR-0137: kural bir kez hesaplanır).

### 4. Öteki bit işlemleri değişmez

`&`, `|`, `^`, `<<`, `>>` ve bit seçme (`t[1]`) Trit'te bugün de E2003'tür
(`binop.rs` `synth_bitwise`, `synth_shift`; `select.rs`); Volt'ta
indirgeme işleci yoktur (`&a` ayrıştırılmaz, E0001). Bu ADR onları
değiştirmez; testler bugünkü davranışı sabitler.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; yeni bir yazım istenmez. `-t` zaten
   dildedir.
2. **Çıkarım (İ2):** `~`'nin tipi tek cevaplıdır: tanımlı olduğu tiplerde
   işlenenin tipi, Trit'te hata. Literalin uyarlandığı tip (check kipi)
   tek yerden okunur.
3. **Anlam değişikliği (İ3):** önce kabul edilen girdi reddedilir: Trit
   (ya da Trit dizisi) üzerindeki `~`. Bu bir sessiz yanlış düzeltmesidir
   ve ADR-0100'ün istisnasına girer: önceki çıktı Trit'in değer kümesinin
   dışında bir değer üretebiliyordu ve ADR-0003'ün tipini çiğniyordu.
   CHANGELOG'da "Davranış değişikliği" maddesi: kimi etkiler, ne yapmalı
   (`~t` yerine `-t`). Kabul edilen başka hiçbir girdinin çıktısı
   değişmez (Doğrulama).
4. **Niyet tahmini (İ4):** derleyici `~t`'yi `-t`'ye kendisi çevirmez;
   yardım yolu söyler, kullanıcı yazar.
5. **Metamorfik yeniden yazım:** uygulanmaz; reddedilen girdinin eşdeğer
   bir biçimi yoktur.
6. **ADR-0139 denetimi.**
   - *Değer kümesi ile kodlama ayrımı:* kural değer kümesine bakar
     ("Trit'te `~` yok"); kodlamayı (Trit'in genişliği, `01`/`11`
     kodları) okumaz ve yazmaz. Tanı ve explain metni bit desenini
     yazmaz; "Trit'in temsilini bilen yerler" listesi büyümez.
   - *Kodlamanın dilden görünmesi:* `p as uN` ile struct içindeki Trit'in
     kodunun görünmesi (ADR-0077 Karar 3) değişmez; bu ADR yalnız `~`'yi
     kaldırır.
   - *Tek yer:* kural `bit_not_result`'tadır; üretici, test dili, dalga
     biçimi ve formal yol onu yeniden hesaplamaz.
   - *Envanter:* "Açık bulgular" 1 kapanır; `synth_unary` satırı
     güncellenir.

## Doğrulama

- Önce kırmızı (main = ad6b0d4d, kaynak değişmeden): `trit_bit_not_tests`
  14 testinin 11'i (`~`'nin her yolu ve tanının metni) düşer; geçen 3'ü
  bugünkü davranışı sabitler (öteki tipler, öteki bit işlemleri, yardımın
  kodu). `ui_fail_318`, `real_bugs` ve
  `the_same_design_with_bit_not_is_e2003_with_minus_t_in_the_help` de
  düşer. Sonra hepsi geçer.
- Yardımın kodu Verilator'da: `-t` +1, 0, -1 için -1, 0, +1 verir, `(-t) *
  x` x = 5 için -5, 0, 5 (`trit_negate_sim_tests`; değerler spec'ten elle).
- Çıktı karşılaştırması (ADR-0136'nın yöntemi): main ile dalın ikilileri
  gölgenin girdisinde (`examples/` dahil 497 dosya) `volt build
  --single-file --emit=sva,sdc`: çıkış kodları ve üretilen 688 dosya yeni
  `tests/real_bugs/434_trit_bit_not.volt` dışında aynı. Kitabın 31 `volt`
  bloğu dalın ikilisiyle derlenir.
- Gerçek hata derlemi: `tests/real_bugs/434_trit_bit_not.volt`
  (`//~ E2003`).

## Sınırlar

- Spec type-inference.md §3.4 hâlâ `UnOp::BitNot => ot` der; yaması
  bakımcıdadır (spec düzenlenmez).
- Kullanıcı modüllerinde tip generic'i (`module M<T>`, `M<Trit>`) bugün
  E0003'tür; o yol açıldığında `~` örneklenen tipte aynı kurala gider
  (örnek başına tip denetimi), ayrı bir iş gerekmez, ama bugün sınanamaz.
- Demet (`~(t, t)`) sınanmaz: demet literalleri bugün E0003'tür. `holds_trit`
  demetin içine bakmaz; demet desteği geldiğinde bu satır onun işidir.

## Reddedilenler

- **`~`'yi Trit'te `-` olarak tanımlamak.** `~`'nin anlamı tipe göre
  değişirdi (bit tersleme ya da olumsuzlama) ve ikisi farklı sonuç verir;
  okuyan `~t`'nin ne yaptığını tipe bakmadan bilemezdi. `-t` zaten var.
- **`~`'yi Trit'te üç değerli bir "tümleyen" işlem yapmak** (ör. Łukasiewicz
  değili). Dil tasarımı kararıdır; bir yamada yeni işlem anlamı
  eklenmez (ADR-0100).
- **Yalnız değeri kümeden çıkan durumu reddetmek** (`~0` -1'dir, kabul).
  Kural değere bağlı olurdu; işlenen bir sinyal olduğunda değeri derleme
  zamanında bilinmez.
- **Üreticide denetlemek.** Kural iki yerde hesaplanırdı (ADR-0137).
