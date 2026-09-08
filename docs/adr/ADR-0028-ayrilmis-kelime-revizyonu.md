# ADR-0028: Ayrılmış Kelime Revizyonu — Stdlib'e Taşınanlar Serbest

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-08
> Etkilenen: volt-syntax (token.rs Reserved listesi, lexer testleri),
> docs/spec/grammar-full.ebnf §17 (bu ADR kaynaklı güncelleme),
> tests/ui/pass/27 (örnekleme adı), crates/volt-sv-emit (etki yok, bkz. Sonuçlar)
> Uygulama aşaması: F5

## Sorun

§17'deki ayrılmış kelime listesi, V1'de dil yapısı olması öngörülen
adları şimdiden rezerve ediyor (kullanım E0003). ADR-0027 stdlib
mimarisini belirledi: FIFO'lar (ve genel olarak bellek/veri-akışı
primitifleri) dil yapısı DEĞİL, kütüphane bileşenidir — bugün Rust'ta
yerleşik (`AsyncFifo<u8, 16>` bir `Path` + `InstanceDecl`), F3
sonrasında Volt stdlib modülü. Dil yapısı olmayacak bir adın anahtar
kelime rezervasyonu gereksizdir ve kullanıcıyı en doğal isimden
(`let fifo = AsyncFifo<...>`) mahrum bırakır.

## Karar

### Serbest bırakılanlar (artık normal Ident)

`fifo`, `ram`, `regfile`, `arbiter`

Dördü de stdlib'e taşındı / taşınacak: bellek, kayıt dosyası ve
hakemlik yapıları ADR-0027 deseniyle (yerleşik primitif → ileride
stdlib modülü) sunulur. Dil yapısı olmayacaklarsa kelime rezervasyonu
gereksizdir. Lexer bunlar için `Ident` üretir; E0003 kalkar.

### Rezerve kalanlar ve neden

| Kelimeler | Neden rezerve |
|---|---|
| `pipeline` `stage` `stall` `flush` | L1/L2 zamanlama katmanları (F5+) — dil yapısı olacak |
| `fsm` `hook` `impl` `trait` `where` | Yapı özelleştirme (V1) |
| `Spike` `PTrit` `spike` | v2/v3 tipleri (nöromorfik / çok değerli mantık) |
| `Option` `Some` `None` `Result` `Ok` `Err` | Prelude tipleri (F5) |
| `mut` `ref` `move` `self` `Self` | Sahiplik modeli (belki hiç gelmeyecek; ihtiyatla rezerve) |
| `secret` `confidential` `public` | Bilgi güvenliği nitelikleri (V1) |
| `clamp_low` `clamp_high` `latch` `retention` `isolation` `always_on` `voltage` | Güç/analog alan kelimeleri (V1) |

Ayrım ölçütü: SÖZDİZİMİ getirecek adlar rezerve kalır; yalnızca TİP
ya da MODÜL adı olacaklar da (Option/Result, Spike/PTrit) prelude'da
gölgeleme karmaşası yaratmamak için rezerve kalır. Serbest bırakılan
dördü ise ne sözdizimi ne prelude adı — sıradan kütüphane
bileşenlerinin doğal örnek adlarıdır.

## Sonuçlar

- **volt-syntax/token.rs**: `arbiter`, `fifo`, `ram`, `regfile`
  `Reserved` varyantından çıkarıldı; lexer `Ident` üretir. Rezerve
  kalanlar E0003 vermeyi sürdürür.
- **grammar-full.ebnf §17**: AYRILMIŞ listesi güncellendi. Spec salt
  okunurdur; bu değişikliğin tanım kaynağı bu ADR'dir (W2013/ADR-0025
  ve W3005/ADR-0027 emsali).
- **tests/ui/pass/27_async_fifo.volt**: `u_fifo` → `fifo` — serbest
  bırakmanın somut kazanımı; `u_` çalışma-çevresi öneki gereksizleşti.
  28/29'daki `u_hs`/`u_ps` de aynı temizlikle `hs`/`ps` oldu (bu ikisi
  zaten rezerve değildi, önek yalnız üslup tutarlılığıydı).
- **volt-sv-emit**: kod değişikliği YOK. `fifo`, `ram`, `regfile`,
  `arbiter` SystemVerilog-1800 ayrılmış kelimesi değildir; üretilen
  SV'de tanımlayıcı olarak geçmeleri güvenlidir.
- **Geri dönüş maliyeti**: bu kelimelerden biri ileride yeniden dil
  yapısı olmak istenirse rezervasyon geri getirilemez (kullanıcı kodu
  kırılır). ADR-0027'nin "stdlib deseni" kararı bu riski bilinçli
  olarak kabul eder: FIRRTL/Verilog emsalinde bellek ve FIFO yapıları
  hiçbir zaman anahtar kelime olmadı.

## Emsal

Rust `usize`/`String`'i rezerve etmez — prelude adlarıdır ve
gölgelenebilir; buna karşın `async`'i yıllar önceden rezerve etti
çünkü SÖZDİZİMİ getirecekti. Aynı ölçüt burada uygulanır: sözdizimi
getirecekler rezerve, kütüphane bileşenleri serbest.
