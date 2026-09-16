# ADR-0019: `todo!` Semantiği — Kısmi Derleme

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03 (karar); parser/typeck F1-F2
> Etkilenen: grammar-full.ebnf (TodoExpr, §17 `todo`), volt-syntax/src/parser/expr.rs,
> volt-hir/src/typeck.rs, volt-sv-emit/src/expr.rs, cli-contract.md §6/§17 (E9001)
> Uygulama aşaması: F1 (sözdizimi), F2 (tip); release engeli ve sim durması uygulanmadı

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0019: todo! semantiği" (Mimari-v3:917, 960-966 "todo! KURALLARI").

## Bağlam

Artımlı geliştirmede (ve LLM destekli üretimde) tasarımcı emin olduğu
kısmı yazıp belirsizi işaretlemek ister; işaret tip kontrolünden geçmeli
ama sentezlenmemelidir (Arch HDL emsali).

## Karar

- Sözdizimi: `todo!` veya `todo!("mesaj")`, yalnız string alır
  (grammar-full.ebnf:467; parser/expr.rs:564-596).
- Tip: her tiple uyumlu hata tipi olarak sessiz uyum sağlar; tanı
  üretmez (typeck.rs:958 `ExprKind::Todo → self.types.error()`).
- Sentez: SV üretiminde desteklenmez; `1'b0` yer tutucu + E0003 "not
  supported in F0 SV generation" (sv-emit/src/expr.rs:588-596, lib.rs:446-452).
- Plan (uygulanmadı): `volt check` todo listesi, `build --release`'de
  E9001, simülasyonda durma (cli-contract.md:357-363; Dil-Spes-v3:324-330).

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:362-389 ("Belirsiz kısım — tip
kontrolünden geçiyor, simülasyonda durur"; "Sinerji: todo! + AI destekli
tasarım. LLM emin olduğu kısmı yazar, belirsizi todo! bırakır, tip
sistemi geri kalanı doğrular"); BÖLÜM VIII "todo! tip kontrolünden
GEÇMELİ… build --release BAŞARISIZ… volt check tüm todo! listesini
göstermeli". Emsal: Arch `todo!` (Mimari-Kararlar:171), Rust `todo!()`.

## Alternatifler

- Yorum satırı / `unimplemented` kalıbı — derleyici görmez; reddedildi.
- `'x` değeri üretmek — ADR-0008 x/z yasağı; reddedildi.

## Sonuçlar

- Bugün `todo!` içeren modül `volt check`'ten geçer, `volt build`'de
  E0003 ile durur; "debug build başarılı" planı hâlâ açık.
- E9001 kodu tanımlı ama üretilmiyor (`--release` yok, ADR-0015).
