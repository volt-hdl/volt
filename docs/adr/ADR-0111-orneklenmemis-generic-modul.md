# ADR-0111: Hiç Örneklenmemiş Generic Modül — SystemVerilog Üretilmez, W1006

> Statü: Uygulandı
> İlgili: ADR-0041 (monomorfizasyon: örneklenmemiş şablon dokunulmadan kalır; artık SV'ye de yazılmaz), ADR-0056 (örneklenmemiş generic şablon açılmaz), ADR-0042 (eki: çıktı kümesi; kütüphane dosyasının şablonu uyarı almaz)
> Tarih: 2026-11-08
> Etkilenen: `crates/volt-sv-emit/src/lib.rs` (`emit_unit`,
> `uninstantiated_generic`), `crates/volt-diagnostics` (`W1006`: kod,
> kısa açıklama ve `volt explain` metni, en/tr),
> `crates/volt-driver/src/main.rs` (yalnız şablon içeren dosyaya
> "kütüphane dosyası" notu basılmaz),
> `crates/volt-driver/tests/uninstantiated_generic_tests.rs` (YENİ),
> `tests/ui/fail/244_uninstantiated_generic_module.volt` (YENİ),
> `tests/ui/pass/139_generic_module_instantiated.volt` (YENİ),
> `CHANGELOG.md`

## Sorun

Monomorfizasyon (ADR-0041) her generic örneklemeyi somut bir modüle
çevirir (`Fir<3>` → `Fir_3`) ve en az bir kez örneklenen şablonu öğe
listesinden çıkarır. Hiç örneklenmemiş şablon "dokunulmadan kalır". Üretici
(volt-sv-emit) ise öğe listesindeki her modülü yazıyordu: şablon da bir SV
modülü oldu. Gövdesindeki generic parametrenin değeri yoktur (#143):

- Aynı adlı bir kök const varsa üreticinin ad çözücüsü (`scope.rs`,
  #128) adı ona çözüyordu. Gövdede ad parametredir (name-resolution.md §6,
  W1002 doğru veriliyor); `const N : u32 = 7` ile `module Fir<const N:
  u32> { out y : u8  y = N }` `Fir.sv`'de `assign y = 32'd7;` oluyordu
  (Verilator WIDTHTRUNC; `volt test`'te ölümcül).
- Kök const yoksa ad olduğu gibi yazılıyordu: `assign y = N;`, SV'de
  tanımsız ad.

Şartname (docs/spec) örneklenmemiş generic modülün SV'si hakkında bir şey
söylemez; sv-mapping.md §9 yalnız örneklemenin monomorf adına çevrildiğini
söyler. cli-contract.md'nin üst modül çıkarımı generic modülü zaten aday
saymaz.

## Karar

1. **Hiç örneklenmemiş generic modül için SystemVerilog üretilmez.**
   Monomorfizasyondan sonra öğe listesinde generic parametreli kalan modül
   şablondur; üretici onu atlar. Şablonun SVA'sı, formal görevi,
   kısıtları ve dalga formu bilgisi de üretilmez (hepsi modülün
   üretiminden çıkar).
2. **Uyarı W1006**, şablonun adında:
   "module 'Fir' is generic and is never instantiated; no SystemVerilog is
   generated for it (parameterized output is planned: #171)". Etiket:
   şablonun parametrelerinin bu dosyada değeri yok; öneri: kullanıldığı
   yerde somut argümanlarla örneklemek (`let u = Fir<8> { ... }`
   `Fir_8`'i üretir) ya da silmek. Uyarı derlemeyi reddetmez, çıkış kodu
   değişmez. Metinde issue numarası durur (ADR-0099 yalnız ADR
   numarasını ve şartname yolunu yasaklar): kullanıcı parametreli
   çıktının durumunu oradan izler.
3. **Yalnız ana dosyanın şablonu.** `use` ile yüklenen kütüphane
   dosyasının modülleri, ana dosyadan örneklenmedikçe çıktı kümesine
   girmez (ADR-0042 eki, `retain_reachable`): orada davranış değişmez,
   uyarı da verilmez. Ana dosya, üreticinin kaynak listesinin ilk
   dosyasıdır (`unit_source_texts`); liste boşsa (kitaplık API'si) her
   dosya ana dosya sayılır.
4. **`volt check` ve LSP aynı uyarıyı verir** (ADR-0070: denetim üreticinin
   tanılarını görür).
5. **Generic olmayan modül değişmez.** Hiç örneklenmemiş, generic olmayan
   modül bugünkü gibi kendi dosyasına yazılır (ADR-0024) ve uyarı almaz:
   üst modül odur.
6. Dosyada yalnız şablon varsa `volt build` "0 SV file(s)" der ve
   modülsüz dosyanın "bu bir kütüphane dosyası" notunu basmaz; nedeni
   W1006 söyler.

Yeni kod: **W1006** (`volt explain W1006`, en/tr). W1 ailesi adlar ve
kullanılmayan bildirimlerdir (W1001 kullanılmayan sinyal, W1005
kullanılmayan import); örneklenmemiş şablon da kullanılmayan bir
bildirimdir, çıktısı olmayan.

## Gerekçe

Bakımcının #143'teki kararı (seçenek 1). Seçenek 2 (şablon yazılır,
parametre modül kapsamının ilk bildirimi olur, değer yoksa tanı) bir
parametre değeri olmadan geçerli SV vermez; SV'de karşılığı parametreli
modüldür (`module Fir #(parameter N)`), o da ayrı bir özelliktir (#171,
v0.2.0): ilk adımı, Volt'un generic özelliklerinden (parametreye bağlı
genişlik, `for` sınırı, const değerlendirme, iç içe generic örnekleme)
hangilerinin SV parametreleriyle ifade edilebildiğinin analizi;
edilemeyenlerde bu uyarı kalır.

## Sınırlar

- Parametresini bir tip konumunda (`uint<N>`) kullanan örneklenmemiş
  şablon üreticiye varmaz: HIR onu ADR-0041'in kararıyla E2021 ile
  reddeder ("this is a generic parameter (runtime value)"). Tanının metni
  yanlış ve kütüphane dosyasındaki örneklenmeyen generic modülü de
  kullanılamaz yapıyor; ayrı kök neden, karar #172'de (v0.1.2).
- Kütüphane dosyasındaki şablon hiçbir birimde örneklenmese de uyarı
  almaz (madde 3).

## Kanıt

Düzeltmeden önce düşen testler (`uninstantiated_generic_tests`):
şablonun dosyası yazılıyordu (`Fir.sv`, `assign y = 32'd7;`), W1006
yoktu; yalnız şablon içeren dosya "library file" notu ve "1 SV file(s)"
basıyordu. Örneklenen generic modülün monomorfu (`Fir_4.sv`), generic
olmayan örneklenmemiş modüller (`A.sv`, `B.sv`) ve kütüphanedeki şablon
düzeltmeden önce de aynıydı; testler bunu korur. Örneklenen şablonun
değeri Verilator'da: `Fir<3>`'te `y = 3` (gövdedeki `N` parametredir,
kök const değil).
