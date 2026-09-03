# Değişiklik Günlüğü

Biçim [Keep a Changelog](https://keepachangelog.com/tr/) esaslıdır;
sürümleme [SemVer](https://semver.org/lang/tr/) izler.

## [0.1.0] - 2026-09-03 — F0 tamamlandı

### Eklendi

- **Lexer** (`volt-syntax`): logos tabanlı; 53 aktif anahtar kelime,
  36 ayrılmış kelime (E0003), tüm operatörler, radix'li/sonekli sayısal
  literaller, iç içe blok yorum desteği, mutlak span'ler.
- **Parser** (`volt-syntax`): LL(2) iniş + Pratt ifade parser'ı
  (operator-precedence.md §3 binding power tablosu birebir); hata
  kurtarma (senkronizasyon kümeleri, ilerleme garantisi, 2-token kaskad
  bastırma); E0002/E0004/E0006/E0007/E0008/E0010 özel tanıları;
  libFuzzer hedefi (525k koşu, 0 panik).
- **AST** (`volt-ast`): arena tabanlı (`Arena<T>`/`Idx<T>`), her düğümde
  span, her enum'da hata kurtarma için `Error` varyantı.
- **Diagnostics** (`volt-diagnostics`): 86 hata/uyarı kodu (Display'li,
  `volt explain` temeli); 5 parça kuralı (`kod + konum + açıklama +
  çözüm + spec referansı`) yapısal olarak zorunlu; codespan-reporting
  ile insan çıktısı, cli-contract.md §5 şemasıyla birebir JSON çıktısı.
- **Span** (`volt-span`): `SourceMap`, bayt ve UTF-8 karakter sütunu
  (`line_col` / `line_col_utf8`).
- **SV emisyonu** (`volt-sv-emit`): sv-mapping.md uyumlu string template;
  otomatik reset portu ve reset bloğu (§7'nin 4 varyantı + none),
  always_ff/assign/wire üretimi, literal boyutlandırma (belirsizlikte
  E2005), `counter.volt → counter.expected.sv` birebir eşleşme.
- **CLI** (`volt-driver`): `volt build` / `volt check`; çıkış kodları
  0/1/2/3 (cli-contract.md §2); tanılar stderr'de, insan formatında.
- **Test altyapısı**: 215 test (63 lexer, 80 parser, 38 emisyon,
  21 tanı, 8 span, 5 CLI); cargo-fuzz hedefi + yerleşik fuzzer.

### Bilinen Sınırlar

- F0 kapsamı gramerin ~%30'u: `fn`, `struct`, `enum`, `match`, `for`,
  `comb`, `wire`, generics, kontratlar ve modül örnekleme F1+ (E0003).
- Tip çıkarımı kaba (ifade genişliği = en geniş operand); gerçek
  çıkarım ve taşma genişlemesi F2'de HIR ile gelecek.
- **CDC kontrolü henüz YOK** — domain anotasyonları ayrıştırılıyor ama
  doğrulanmıyor (F2).
- `sync()` CDC çağrısı `sync` anahtar kelimesiyle çakışıyor; bağlamsal
  anahtar kelime çözümü ADR-0023'te kararlaştırıldı, F1'de uygulanacak.
- `u9`/`u17` gibi ara genişlikler tip sözdiziminde yok (bits<N>
  kullanılmalı); `tests/ui/pass` altındaki bazı F1+ fixture'ları bu
  nedenle F0'da tanıyla reddediliyor.
