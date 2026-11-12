# ADR-0115: Soneksiz Literalin `as` Dönüşümü — Sığan Literal Değerini Taşır, Sığmayan Kesilir ve W2010 Verir

> Statü: Uygulandı
> İlgili: ADR-0108 (fark testinin A14 varsayımı: sığan literalin `bits<N>`'e dönüşümü değerini taşır; bu ADR o okumayı karar yapar), ADR-0099 (W2010'un yardım satırı sığan değeri söyler; tanı metninde ADR numarası yok), ADR-0100 (sessiz kesmeyi görünür kılan uyarı son hanede)
> Tarih: 2026-11-12
> Etkilenen: volt-sv-emit (`expr.rs` `emit_cast`: `(-K) as T`), volt-hir (`typeck/cast.rs` `warn_literal_cut`,
> `typeck/literal_fold.rs` `literal_sign_mag`), volt-diagnostics (W2010
> açıklaması en/tr), `tests/ui/fail/276`, `crates/volt-driver/tests/wide_literal_sim_tests.rs`,
> `CHANGELOG.md`

## Sorun

type-inference.md §3.6'nın dönüşüm tablosu kaynağı tipli olan
dönüşümleri sayar: genişletme serbest, daraltma W2010 ile izinli.
Soneksiz literalin dönüşümü (`K as uN`, `K as iN`, `K as bits<N>`) tabloda
yoktur (#122'nin açık satırı). Derleyici bu dönüşümü kabul ediyor, sığan
literali değeriyle yazıyor, sığmayanı ise hedefin alt bitlerine **uyarısız**
kesiyordu: `0x1FF as bits<8>` → `8'hFF`, `300 as u8` → `8'h2C`. Literal
`bits<N>` değerine yalnız bu dönüşümle ulaşır (`bits<128>` bağlamında
çıplak literal E2003'tür): 128 bitlik anahtar ve maskeler hep
`K as bits<128>` biçimindedir (#207). Sessiz kesme orada en çok zarar
verir.

## Karar

1. **Sığan literal değerini taşır.** Hedef `uN` ya da `bits<N>` iken
   0 ≤ K < 2^N, `iN` iken -2^(N-1) ≤ K < 2^(N-1) ise dönüşümün değeri K'dır.
   K 2^128 - 1'e kadar her literaldir (daha genişi zaten E0005).
2. **Sığmayan literal hedefin alt N bitine kesilir ve W2010 verir.**
   Tipli daraltmayla aynı kural (§3.6 "daraltma — izinli ama uyarı"): `as`
   açık dönüşümdür, hata değildir; kesme görünürdür. İleti literali,
   hedefi ve kesilmiş değeri (`0x…`) söyler; yardım satırı sığan değerin
   yazılmasını önerir.
3. **Kapsam.** Çıplak literal, `-literal` (parantezli: `(-1) as u8`;
   parantezsiz `-1 as u8` `-(1 as u8)`'dir) ve döngü değişkeni taşımayan
   literal aritmetiği (`(200 + 100) as u8`, değeri i128'de). Döngü
   değişkenli literal ifadesi ve i128'i aşan bileşik ifade (E2022) bu
   denetime girmez.
4. Çıplak literalin üretilen SV'si değişmez: kesilmiş değer önceden de
   yazılıyordu (#123'ün örüntü kuralı). Parantezli negatif literal
   (`(-128) as i8`) önceden üreticide E2005 alıyordu ("cannot determine the
   width of the literal"), sığan değerde de; artık çıplak literal gibi
   hedef genişliğinde yazılır (`8'sh80`, `(-129) as i8` → `8'sh7F`,
   `(-1) as u8` → 255).

## Reddedilenler

- **Sığmayan literal E2010.** Literal atamada (`let x : u8 = 300`) sığmazsa
  E2010'dur; dönüşümde de hata olabilirdi. Reddedildi: `as` dilde açık
  kesme demektir, tipli daraltma uyarıdır; literal kaynağı için ayrı,
  daha sert bir kural iki biçim arasında tutarsızlık yaratır. Depodaki 524
  `.volt` dosyasında sığmayan literal dönüşümü yok; ileride E2010'a geçmek
  bakımcının kararıdır ve bu ADR'nin yerini alan yeni bir ADR ister.
- **Uyarısız kesme (önceki davranış).** #207'nin ilkesine aykırı: kesme
  sessiz olmamalı.

## Kanıt

- `typeck::cast::tests::issue_207_a_literal_that_does_not_fit_its_cast_target_warns`
  düzeltmeden önce düştü (`0x1FF as bits<8>: []`), sonra geçer; sığan
  yedi biçim (`0xFF as bits<8>`, `(-128) as i8`, `2^127 as bits<128>`,
  `2^128 - 1 as bits<256>` …) uyarısızdır.
- `tests/ui/fail/276` tek tanı olarak W2010 verir
  (`wide_literal_sim_tests::issue_207_ui_fail_fixtures_report_their_code`).
- Depodaki 524 `.volt` dosyası (`tests/ui/fail` hariç) yeni uyarı almadı. Sığan literal dönüşümü olan `tests/ui/pass/68`, `93` ve `110` uyarısız kalır (geçen tarafın fikstürleri).

## Şartname

docs/spec ajanlarca düzenlenmez. §3.6'ya bu kararın satırı bakımcı yaması
olarak eklenmeli (taslak #207'nin raporunda).
