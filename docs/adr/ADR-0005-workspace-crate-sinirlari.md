# ADR-0005: Workspace Crate Sınırları ve Araç Zinciri (Rust, logos, codespan-reporting)

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: Cargo.toml (workspace members/dependencies), crates/*/Cargo.toml,
> scripts/check-consistency (kontrol 4), CLAUDE.md "Dizin Yapısı"
> Uygulama aşaması: F0

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Betikler ve tasarım
belgesi "ADR-0005" olarak referanslar (check-consistency.sh:47,
Volt-Mimari-Kararlar-ve-CLAUDE-md.md:453-468).

## Bağlam

"Crate sınırı yanlış → yeniden yapılandırma, 2 gün" (Mimari-Kararlar:15).
Arka uç (CIRCT) belirsizken derleyicinin geri kalanının ondan
etkilenmemesi ve tip sistemine (F2) girmeden sınırların oturması
gerekiyordu.

## Karar

- Dil: Rust, Cargo workspace, edition 2021 (Cargo.toml).
- Katmanlar ve bağımlılık yönü (yalnız aşağı, döngü yok):
  `volt-span → volt-diagnostics → volt-ast → volt-syntax → volt-hir →
  {volt-sv-emit, volt-sw-emit, volt-sdc-emit, volt-lower} → volt-driver`;
  `volt-lsp` hir'e kadar iner (crates/*/Cargo.toml `path =` bağımlılıkları).
- CIRCT/melior yalnız `volt-lower` içinde olabilir; betik başka crate'te
  referans bulursa ihlal (check-consistency.sh:47-56).
- Lexer `logos` (volt-syntax/Cargo.toml:13), tanı çıktısı
  `codespan-reporting` (volt-diagnostics/Cargo.toml:9), CLI `clap`.
  Sürümler tek yerde (`[workspace.dependencies]`); yeni bağımlılık PR ve
  gerekçe ister (CLAUDE.md).

## Gerekçe

Kaynak: Volt-Kodlama-Oncesi-Kritik-Adimlar.md:97-146 ("volt-lower izole →
CIRCT değişirse sadece burası", "tests/ui ayrı → her özellik pass + fail
test"), 2.2 workspace taslağı (logos, cstree, codespan-reporting).
Emsal: rust-analyzer crate düzeni (Mimari-Kararlar:162-165).

## Alternatifler

- Tek crate — derleme süresi ve sınır disiplini kaybı.
- `volt-sim`, `volt-formal` ayrı crate'ler (Kodlama-Oncesi taslağı) —
  oluşturulmadı; simülasyon `volt-hir/sim.rs` + `volt-sv-emit/sim.rs`,
  formal `volt-sv-emit/sby.rs` + `volt-driver/verify.rs` içinde yaşıyor.
- `cstree` ile kayıpsız CST — `[workspace.dependencies]`'te tanımlı ama
  hiçbir crate kullanmıyor; parser AST'yi doğrudan kurar (ADR-0006).

## Sonuçlar

- 11 crate (Cargo.toml `members`); CLAUDE.md "7 crate" satırı eski.
- `unsafe` yasağı kural olarak var (CLAUDE.md), `#![forbid]` niteliğiyle
  zorlanmıyor.
