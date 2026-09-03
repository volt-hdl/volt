# Volt HDL

Volt, Rust benzeri sözdizimiyle SystemVerilog üreten, saat alanı (clock domain)
güvenliğini tip sisteminde doğrulayan bir donanım tanımlama dilidir.

## Derleme Komutları

```
cargo build                 # tüm workspace
cargo test                  # tüm testler
cargo run -p volt-driver -- build <dosya.volt>   # tek dosya derle
cargo clippy --all-targets  # lint
```

## Belge Haritası (docs/spec/ — BAĞLAYICI)

| Dosya | Konu |
|---|---|
| grammar-full.ebnf | Tam dilbilgisi |
| operator-precedence.md | Operatör öncelikleri |
| ast-nodes.md | AST düğüm tanımları |
| error-recovery.md | Parser hata kurtarma |
| name-resolution.md | İsim çözümleme |
| type-inference.md | Tip çıkarımı |
| domain-inference.md | Saat alanı çıkarımı (CDC) |
| const-eval.md | Sabit değerlendirme |
| sv-mapping.md | SystemVerilog eşlemesi |
| cli-contract.md | CLI sözleşmesi |
| GLOSSARY.md | Terminoloji sözlüğü (TR/EN) |
| README.md | Spec dizini ve çeviri durumu |

## Kesin Kurallar

- `docs/spec/` ve `docs/adr/` SALT OKUNURDUR — değiştirme, yeni karar için ADR aç.
- Test silmek veya atlamak (`#[ignore]`, silme, yorum satırına alma) YASAKTIR.
- `unsafe` kod YASAKTIR.
- Yeni bağımlılık eklemek PR ve gerekçe GEREKTİRİR (workspace.dependencies üzerinden).
- Her hata mesajı 5 parça içermelidir: hata kodu, konum, açıklama,
  öneri (fix-it) ve ilgili spec referansı.
- Yeni terim çevirirken önce `docs/spec/GLOSSARY.md`'ye bak.
- `docs/spec/` kanonik dili İngilizcedir; `docs/spec/tr/` çeviridir.

## Kalite Kontrol

- Commit öncesi: `just check` (fmt + clippy + test — sıfır uyarı).
- Haftalık: `just weekly` (coverage + consistency + bench + fuzz).
- Yeni özellik: en az 1 pass + 1 fail testi zorunlu (`tests/ui/`).
- `just consistency` spec-kod tutarlılığını denetler (`scripts/check-consistency.ps1|.sh`);
  test sayısı `.test-baseline`'dan düşemez, test eklenince `-Update`/`--update` ile yenile.
- `just coverage` HTML raporu `target/llvm-cov/html/` altına üretir.
- `just bench` yalnızca lokal çalıştırılır (CI ortamı gürültülü); referans değerler
  `crates/volt-syntax/benches/baseline.json`.

## Belge Öncelik Sırası (çelişki durumunda)

1. `docs/design/Volt-UX-Anayasasi.md` (en yüksek)
2. `docs/adr/`
3. `docs/spec/`
4. `docs/design/`
5. `docs/research/` (bağlayıcı DEĞİL — yalnızca arka plan)

## Dizin Yapısı

- `crates/` — 7 crate: span → diagnostics → syntax → ast → hir → lower → driver
- `tests/ui/pass/` derlenmeli, `tests/ui/fail/` beklenen hatayı (`//~ EXXXX`) üretmeli
- `tests/fixtures/` — girdi/beklenen-çıktı çiftleri (ör. counter.volt → counter.expected.sv)
