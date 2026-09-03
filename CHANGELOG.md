# Değişiklik Günlüğü

Biçim [Keep a Changelog](https://keepachangelog.com/tr/) esaslıdır;
sürümleme [SemVer](https://semver.org/lang/tr/) izler.

## [Yayımlanmadı]

### Eklendi — F2b: İkili operatörler ve genişleme kuralları (2026-09-03)

- **Aritmetik taşma genişlemesi** (`volt-hir/src/typeck.rs`,
  type-inference.md §3.3): `+`/`-` bir bit, `*` genişlik kadar genişler;
  `/`/`%` genişlemez; sonuç `MAX_WIDTH` ile sınırlı. Operand genişliği
  uyuşmazsa E2001, işaret karışırsa E2002, `bits<N>` aritmetiğinde E2004.
  Literal operand somut tarafa uyarlanır (taşmada E2010); literal+literal
  literal kalır; `Ty::Error` sessiz yayılır.
- **Esnek genişlik aralığı** (ADR-0025, `Ty::UIntFlex`/`SIntFlex`):
  aritmetik sonuç `[işlem, doğal]` genişlik aralığı taşır — `u8 + u8`
  kullanıcıya `u9` görünür ama sayaç deseni (`count <= count + 1`) taşma
  bitini atarak operand genişliğine uyar; aralık dışı hedef E2001.
  Operand uyumu aralık kesişimiyle kurulur; `as` ve `reg` çıkarımı doğal
  genişliğe sabitler.
- **Trit kuralları**: `Trit * Trit → Trit` (kapalı küme),
  `Trit ± Trit → i3` (taşma), `Trit * iN → iN` (ternary MAC, iki yönde);
  kalan kombinasyonlar E2003.
- **Bit düzeyi** (`&`, `|`, `^`): genişlemez; `bool&bool → bool`, aynı
  genişlik `uN/iN/bits<N>` korunur; genişlik farkı E2001, işaret karışımı
  E2002, uyumsuz tipler E2003.
- **Kaydırma** (`<<`, `>>`): sonuç sol operandın tipi; sağ operand
  sayısal değilse E2003; sabit miktar sol genişliği aşarsa yeni W2013
  uyarısı (kod ADR-0025 ile tanımlı, tutarlılık taraması artık
  `docs/adr/` da okuyor).
- **Karşılaştırma**: her zaman `bool`; operandlar `unify_for_comparison`
  ile aynı tipe birleştirilir, uyumsuzluk iki tipi de gösteren E2003.
- **Mantıksal** (`&&`, `||`): iki operand da `bool`, sonuç `bool`.
- **Koşullu ifade** (§3.7): sentez konumunda dallar birleştirilir;
  uyumsuz dallar iki tipi de gösteren E2003; esnek aralıklar kesişimle
  birleşir; literal dal somut dala uyarlanır.
- Test: +93 (566 toplam; ikili operatör testleri 85); ui/fail 02→E2001,
  08→E2002, 09→E2004 artık doğru kodu üretiyor; ui/pass 21/21 temiz.

### Eklendi — F2a: Tip sistemi temeli (2026-09-03)

- **Tip gösterimi** (`volt-hir/src/ty.rs`): `Ty` enum'u (Bool, UInt,
  SInt, Bits, Trit, Clock, Reset, Array, Tuple, Struct, Enum, Instance,
  IntLit, Error) + interning'li `TypeArena` (aynı tip → aynı `TypeId`).
- **Çift yönlü tip kontrolü** (`volt-hir/src/typeck.rs`,
  type-inference.md §2-§6): `synth`/`check` akışı; literal çözümleme
  (soneksiz → bağlamdan, taşmada E2010, Trit dışı E2011); tekli
  operatörler (`!` → bool, `~` genişliği korur, `-` → i(w+1), işaretsiz
  negasyon E2002); bit seçimi (E2006 sınır denetimi), aralık seçimi
  (E2007 ters aralık, E2008 değişken sınır); cast tablosu (daraltmada
  W2010, sayısal→Trit E2009); atanabilirlik (örtük daraltma VE genişleme
  E2001, işaret uyumsuzluğu E2002); `reg` tip belirsizliği E2012, `let`
  varsayılanı W2012.
- **Sürücü analizi** (`volt-hir/src/drivers.rs`, §11): `DriverTable`;
  E4001 çift sürücü (aynı on/comb bloğu içi koşullu atamalar muaf),
  E4002 sürücüsüz çıkış portu, W4001/W4002 sürülen-ama-okunmayan sinyal.
- **İsim çözümleme köprüleri** (`resolve.rs`): bildirim/kullanım span →
  DefId haritaları, tip konumu Path çözümleri, okuma kümesi dışa açıldı.
- F2a kapsam sınırı: ikili operatörler (aritmetik/bit/karşılaştırma/
  mantıksal) F2b'de — şimdilik hatasız `Ty::Error` döner.
- Test: +81 (516 toplam); ui/fail 03, 04, 10, 11, 12, 17, 20 artık
  doğru kodu üretiyor; kapsam %83,9 (typeck %82,9).

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
