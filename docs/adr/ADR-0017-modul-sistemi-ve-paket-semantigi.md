# ADR-0017: Modül Sistemi ve Paket Semantiği — `package` / `use` / `pub`

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03 (karar); uygulama ADR-0042 (2026-09-13)
> Etkilenen: grammar-full.ebnf §1 (PackageDecl, UseDecl, `pub`), name-resolution.md
> §3.2/§3.3/§10, volt-driver/src/unit.rs, E1004/E1006/E1010/E1011
> Uygulama aşaması: F5 (ADR-0042)

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgeleri
"ADR-0017: Modül sistemi ve paket semantiği [YENİ ★]" olarak işaretler
(Mimari-v3:915, 1066: "Bu dördü olmadan F1'e başlanmamalı"). Gramer
kararı F1'de alındı; çoklu dosya derlemesi ADR-0042 ile geldi.

## Bağlam

Tek dosyalık örnekler ölçeklenmez; SoC düzeyinde IP yeniden kullanımı
görünürlük ve isim alanı ister. Aynı zamanda UX Anayasası "modül sistemi
yok → tek dosya çalışır" der (İlke 1): tek dosya boilerplate'siz kalmalı.

## Karar

- `package a::b;` dosya başına en fazla bir kez; `use a::b::X`,
  `use a::{X, Y}`, `use a::*`, `use a::X as Z` (grammar-full.ebnf:36-43).
- `pub` olmayan öğe paket dışına kapalı (E1004); dosya yolu paket yoluyla
  eşleşir; `std::` yerleşik (name-resolution.md §3.3).
- Package/use yokken dosya tek başına derlenir; hiçbir bildirim zorunlu değil.

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:201-240 (2.3 "Paket tanımı dizin
yapısıyla eşleşir", `pub`/paket-içi görünürlük); Rust modül sistemi
emsali (`use`, `pub`, `as`). Neden F1'de: "Bunlar gramer kararları.
Sonradan değiştirmek tüm parser'ı etkiler" (Mimari-v3:770-771).

## Alternatifler

- Verilog `include` / kütüphane arama yolları — isim alanı yok, çakışma
  sessiz; reddedildi.
- v3 taslağındaki `import` kelimesi (Mimari-v3:213) — Rust'la tutarlılık
  için `use` seçildi; `import` gramerde yok.

## Sonuçlar

- Çoklu dosya birimi, dosya keşfi ve hata kodları ADR-0042'de; bilinen
  sınırlar (tek kök kapsam) orada listelidir.
- Planlanan arayüz-hash'li ayrık/artımlı derleme (Mimari-v3:221-238)
  uygulanmadı (bkz. ADR-0015).
- Paket registry ve SemVer (ADR-0022) V1.
