# ADR-0006: Arena Tabanlı AST/HIR — `Idx<T>` Handle Deseni, Tip Bilgisi HIR'da

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: ast-nodes.md §0-§1, volt-ast/src/arena.rs, volt-ast/src/lib.rs,
> volt-hir/src/ty.rs (TypeArena), name-resolution.md §0
> Uygulama aşaması: F0 (AST), F2a (HIR tip tablosu)

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0006-hir-dugum-stratejisi.md" olarak referanslar
(Volt-Mimari-Kararlar-ve-CLAUDE-md.md:239, 433-437).

## Bağlam

"HIR düğüm yapısı yanlış → tip sistemi yeniden yazılır, 3 hafta"
(Mimari-Kararlar:17). Modül → port → modül gibi döngüsel referanslar,
hata kurtarma sırasında eksik ağaçlar ve LSP için ucuz kopyalanabilir
handle'lar gerekiyordu.

## Karar

- Düğümler `Arena<T>` (`Vec<T>`) içinde tutulur, `Idx<T>` (u32 + PhantomData)
  ile referans verilir; `Idx` Copy/Eq/Hash (volt-ast/src/arena.rs:8-34;
  ast-nodes.md İ1).
- Her düğüm `Span` taşır (İ2); her enum hata kurtarma için `Error`
  varyantı içerir (İ3; volt-ast/src/lib.rs:6-7).
- **AST sözdizimseldir, tip bilgisi HIR'da yaşar** (ast-nodes.md İ5);
  tipler `TypeArena` ile intern edilir, aynı tip aynı `TypeId`
  (volt-hir/src/ty.rs:1-6). İsim çözümleme AST'yi tipsiz alır
  (name-resolution.md:12-25).

## Gerekçe

Kaynak: Volt-Mimari-Kararlar-ve-CLAUDE-md.md:218-239 (K2).
- `Rc<RefCell<>>` değil: döngüsel referans sorunu, çalışma zamanı panik
  riski, Salsa ile karmaşık.
- `Box<>` ağacı değil: döngüsel referans imkânsız, CFG döngülü.
- Arena: tek allocation, cache dostu, döngü `Idx` ile mümkün,
  rust-analyzer'ın kanıtlanmış yaklaşımı.
- Tip bilgisinin ayrılması: "aşamaların ayrılması, yeniden
  kullanılabilirlik" (ast-nodes.md:32); arşiv tasarımı "HIR mı, AST'de
  tipleme mi?" sorusunu "ayrı arena-tabanlı HIR" ile kapatmıştı
  (design/archive/Volt-HDL-F0-F1-Teknik-Tasarim.md:615).

## Alternatifler

- `Rc<RefCell<Node>>` grafı — reddedildi (yukarıda).
- Tipleri AST düğümüne yazmak — aşama ayrımı bozulur; reddedildi.

## Sonuçlar

- `Name` interning ertelendi: F0/F1'de metin taşınır (volt-ast/src/lib.rs:17-18).
- Salsa/artımlı derleme MVP dışı (archive planı "MVP'de salsa yok");
  arena deseni geçişe açık kapı bırakır.
- CST (cstree) katmanı kurulmadı; formatter ihtiyacı doğarsa ADR gerekir.
