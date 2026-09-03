# ADR-0023: Bağlamsal Anahtar Kelimeler — `sync` Çakışması

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-03
> Etkilenen: lexer (volt-syntax), parser (volt-syntax), grammar-full.ebnf §17
> Uygulama aşaması: F1

## Sorun

`sync` kelimesi dilde iki farklı rolde kullanılıyor:

1. **Reset spesifikasyonu** (grammar-full.ebnf §3):
   `domain D { reset = sync active_high }` — burada anahtar kelime.
2. **CDC köprü fonksiyonu** (sv-mapping.md §8, domain-inference.md):
   `slow_data = sync(fast_data, slow_clk)` — burada fonksiyon adı.

F0 lexer'ı `sync`'i koşulsuz anahtar kelime (KwSync) olarak ürettiği için
`sync(...)` çağrısı ifade konumunda E0001 "beklenmeyen 'sync'" hatasıyla
reddediliyor. `tests/ui/pass/13_cdc_correct_bridge.volt` bu yüzden F0'da
ayrıştırılamıyor. Aynı sorun UX Anayasası'nın "CDC çözümü tek çağrı"
ilkesini de bloke ediyor.

## Karar: Seçenek A — Bağlamsal Anahtar Kelime

`sync` (ve simetri gereği `async`) **bağlamsal anahtar kelime** olur:

- `reset =` değerinin ilk tokenı konumunda → anahtar kelime
  (ResetSync üretimi: `sync active_high`, `async active_low`).
- Diğer her yerde → sıradan `Ident`
  (`sync(...)` çağrısı, hatta `let sync = ...` bağlaması geçerli).

Uygulama: lexer `sync`/`async` için Ident üretir; parser yalnızca
`DomainField = "reset" "=" ...` bağlamında bu Ident'leri ResetSync
tokenı gibi yorumlar. Lexer'da bağlam durumu tutulmaz (tek doğruluk
kaynağı parser'dır); LL(2) özelliği korunur çünkü ayrım tek token
ileriye bakışla yapılır (`reset =` sonrası).

## Reddedilen Alternatifler

**Seçenek B — reset kelimesini değiştir: `synchronous` / `asynchronous`**
Reddedildi: domain bildirimleri dilin en sık kopyalanan kalıbı;
uzun kelimeler UX Anayasası'nın kısalık ilkesine aykırı ve mevcut
tüm spec örnekleriyle/testlerle uyumsuzluk yaratır.

**Seçenek C — CDC fonksiyonunu yeniden adlandır: `cdc_sync()`**
Reddedildi: `sync(data, clk)` domain-inference.md ve UX Anayasası'nda
CDC çözümünün imzası olarak geçiyor; hata mesajları (`= çözüm:
result = sync(data, slow_clk)`) bu ismi öğretiyor. Ayrıca ad değişse
bile `sync` §17'de ayrılmış kalacağından kullanıcı tanımlayıcısı
olarak yine yasaklanırdı — çakışma çözülmez, gizlenirdi.

## Emsal

Rust'ta `union` bağlamsal anahtar kelimedir: yalnız `union İsim {`
konumunda anahtar kelime, diğer her yerde geçerli tanımlayıcıdır
(`fn union()` derlenir). Aynı teknik `dyn` (2015 edition) ve `raw`
için de kullanıldı. C#'ta `var`, `record`; Go'da tüm yerleşikler
benzer şekilde bağlamsaldır. Teknik kanıtlanmış ve LL parser'larla
uyumludur.

## Sonuçlar

- Lexer: KwSync/KwAsync varyantları kalkar (F1'de) → Ident.
- Parser: `parse_domain_value` reset bağlamında `sync`/`async`
  metnine bakar; `sync` çağrısı normal Call ifadesi olur.
- grammar-full.ebnf §17: `sync`/`async` aktif listeden "bağlamsal"
  notuyla işaretlenir (bu ADR ile eklendi).
- Testler: 13_cdc_correct_bridge.volt F1'de ayrıştırılabilir hale gelir.
- Geriye uyumluluk: F0'da `sync` zaten tanımlayıcı olarak
  kullanılamıyordu; kısıt gevşediği için kırılma yok.
