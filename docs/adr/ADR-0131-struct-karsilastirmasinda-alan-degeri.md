# ADR-0131: Bütün-Struct Karşılaştırmasında Literal Alanı, Alanın Tipinin Bir Değeri Olarak Denetlenir (#375)

> Statü: Uygulandı
> İlgili: ADR-0059 (port genişlik denetimi: sabit E8512, hesaplanmış değer koşuda düşer, maskeleme reddedildi; bu ADR kuralı bütün-struct karşılaştırmasının literal alanına uygular), ADR-0077 (test dilinde struct literali; bütün yazma alanları zaten port yazımı gibi denetliyordu), ADR-0100 (sessiz yanlış istisnası: önceden geçen testi düşüren düzeltme son hanede, v0.1.3)
> Issue: #375 (kapsam incelemesi #379)
> Tarih: 2026-11-21
> Etkilenen: `crates/volt-hir/src/sim_port.rs` (`check_value_fits`, `port_path_width`: yazma kuralının ve yaprak genişliğinin tek yeri), `crates/volt-hir/src/sim_struct.rs` (`check_struct_compare` her yaprakta bu kuralı çağırır), `crates/volt-hir/src/sim.rs`, `crates/volt-sv-emit/src/sim.rs` ve `sim_script.rs` (`TbStep::CheckValue`: `SetPortChecked`'in denetimi, yazmasız), `crates/volt-driver/src/sim_lower.rs` (`literal_field_checks`, `port_check`), `crates/volt-driver/src/sim_struct.rs`, `tests/ui/fail/309_test_struct_compare_leaf_overflow.volt`, `crates/volt-hir/tests/ui_semantic_tests.rs`, `crates/volt-driver/tests/struct_compare_range_tests.rs` (YENİ), `crates/volt-sv-emit/tests/test_script_tests.rs`, `CHANGELOG.md`.
> DOKUNULMADI: `docs/spec/` (E8512 yalnız ADR'lerde tanımlı; spec değişikliği gerekmez), üretilen SV.

## Sorun

#375: `assert_eq(dut.q, P { a: 17, b: true })` (`a : u4`) denetimden
geçiyordu; testbench literalin alanlarını alan genişliğine maskeleyip
paketliyordu (`17 & 15 = 1`), test `q.a = 1` tutan donanımda geçiyordu.
Hesaplanmış alan da aynı biçimde maskeleniyordu (`P { a: v + 16 }` →
`(v + 16) & 15`). Aynı dosyada `assert_eq(dut.q.a, 17)` ve
`dut.x = P { a: 17, .. }` E8512 alıyordu.

Kök neden: "struct literalinin bir alanı, alanın tipinde bir değer
olmalı" kuralı yalnız bir yolda uygulanıyordu. Bütün-struct yazımı
yapraklara açılır ve her yaprak port yazımı gibi denetlenir (derlemede
`PortWidth::accepts`, koşuda `volt_port_fits`, ADR-0059). Bütün-struct
karşılaştırması ise literali paketler ve yaprağı yalnız "sayı mı" diye
denetler; aralık kuralının yerini maskeleme alıyordu — ADR-0059'un "hatayı
GİZLER" diye reddettiği seçenek.

## Karar

1. **Kural:** bütün-struct karşılaştırmasında literalin her alanı, alanın
   tipinin bir değeri olmalıdır — bütün-struct yazımındaki kuralın
   aynısı (`PortWidth::accepts`): işaretsiz alanda `0..2^W-1`, işaretli
   alanda bit deseni ya da aralıktaki negatif sayı (`0 - 1`, ADR-0059 §2).
   Tek bir port okumasıyla karşılaştırmadaki "okunan bit deseni" kuralı
   (`fits_pattern`; `assert_eq(dut.out, 0 - 1)` E8512) burada geçmez:
   literal, tipin bir değerini kurar, `P { s: 0 - 1 }` geçerli bir `P`
   değeridir (ADR-0077'nin doğrulamasında "işaretli yaprak -1" zaten
   destekleniyordu).
2. **Sabit alan:** sığmıyorsa E8512, yazımdaki tanının aynısı (ikincil
   etiket alanı adlandırır: `field 'a' of 'P' is u4 (max 15)`).
3. **Hesaplanmış alan:** karşılaştırmadan önce testbench'te denetlenir;
   sığmıyorsa test `port_overflow` ile düşer (yazımdaki denetimin aynısı,
   `TbStep::CheckValue`). Sığdığı kanıtlı değer (0/1, alandan dar bir
   port okuması) denetlenmez — yazımla aynı `fits_without_check`.
4. **Tek yer:** kural ve yaprak genişliği bir kez hesaplanır
   (`sim_port::check_value_fits`, `sim_port::port_path_width`; koşuda
   `volt_port_fits` ve sürücünün `port_check`'i); yazım ve karşılaştırma
   aynı fonksiyonları çağırır.

## İlke denetimi

1. **Beyan (İ1):** Yeni beyan istenmez. Struct alanının tipi zaten bir
   beyandır; bütün-struct karşılaştırması literalin alanını bu beyanla
   denetlemiyordu (İ1'in "denetlenmeyen beyan" durumu). Artık bütün-struct
   yazımındaki gibi denetlenir: sabit derlemede, hesaplanmış değer koşuda.
2. **Çıkarım (İ2):** Kural yeni bir şey çıkarmaz; alanın genişliği
   bildirilen tipten, yazımla aynı tek yoldan okunur.
3. **Anlam değişikliği (İ3):** Alanına sığan literalli karşılaştırmanın
   anlamı ve testbench'i değişmez. Değişen, önceden sessizce maskelenen
   girdinin reddi ya da düşmesidir; bu İ3'ün değil ADR-0100'ün sessiz
   yanlış istisnasının konusudur.
4. **Niyet tahmini (İ4):** Uygulanmaz.
5. **Metamorfik yeniden yazım (ADR-0117):** Kural iki yazımı aynı sığma
   kuralına bağlar: bütün-struct karşılaştırması ile alan alan
   karşılaştırma (`assert_eq(dut.q.a, ..)`). Katalogdaki yeniden yazımlar
   tasarım yapıları üzerindedir, test bloğu yazımı yoktur; bu yeniden
   yazım eklenebilir, ama test bloklarını yeniden yazan bir koşucu ister
   ve bu ADR'de eklenmez.

## Sürüm (ADR-0100)

Önceden geçen bazı testler artık düşer: yalnız literal alanı alanına
sığmayan bütün-struct karşılaştırmaları (sabitse derlemede E8512, çıkış
1; hesaplanmışsa koşuda `port_overflow`, çıkış 5). Bu bir sessiz yanlışın
düzeltmesidir (yanlış donanımda test geçiyordu); ADR-0100'ün istisnasıyla
son hanede (v0.1.3) çıkar. Yeni tanı kodu yok; alanına sığan literallerin
testbench'i ve sonucu değişmez.

## Reddedilenler

- **Okuma kuralı (`fits_pattern`).** Tek port karşılaştırmasıyla aynı
  olurdu ama işaretli alana `0 - 1` yazan bugünkü testleri (ADR-0077)
  kırardı; literal bir okuma değil, tipin bir değeridir.
- **Maskelemeyi koruyup yalnız derlemede denetlemek.** Hesaplanmış alan
  yine sessizce maskelenirdi (ADR-0059: maskeleme reddedildi).
- **Taşmayı karşılaştırmanın sonucuna katmak** (sığmayan alanlı literal
  hiçbir değere eşit değildir: `assert_eq` düşer, `assert_ne` geçer).
  Yazımdaki kuraldan ayrılırdı ve raporda maskelenmiş değer görünürdü;
  E8512 de `assert_ne(dut.out, 300)`'ü "hiç eşleşemez" diye reddeder.
