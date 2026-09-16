# ADR-0013: Gramer ve Parser Sözleşmesi — LL(2), Geri İzleme Yasağı, Hata Kurtarma, Öncelik Kararları

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: grammar-full.ebnf (başlık, §17), operator-precedence.md,
> error-recovery.md §1-§5, volt-syntax/src/parser/{mod,expr,recovery}.rs
> Uygulama aşaması: F0-F1

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgeleri
"ADR-0013: Gramer sınıfı ve AI-üretilebilirlik kontratı" olarak
referanslar (Volt-Dil-Spesifikasyonu-v3.md:54; Mimari-v3:659, 911).

## Bağlam

Kontratlar, anotasyonlar ve generic'ler zengin sözdizimi ister; LLM'lerin
güvenilir Volt üretmesi ve LSP'nin yarım yazılmış kodda çalışması
için gramer basit ve parser dayanıklı olmalı (Mimari-v3 Çakışma 2;
error-recovery.md §0).

## Karar

1. **Gramer sınıfı**: LL(1) hedef, LL(2) tavan; geri izleme ve belirsizlik
   YASAK (grammar-full.ebnf:6-7; parser/mod.rs:3; stmt.rs:42 "geri izleme
   DEĞİL"). Her yapı benzersiz anahtar kelimeyle başlar; V1 kelimeleri
   şimdiden rezerve, kullanımı E0003 (§17).
2. **Hata kurtarma** (error-recovery.md İ1-İ5): ASLA panik, her girdide
   AST, hata düğümü, tek hata tek mesaj (kaskad bastırma), her kurtarma
   en az bir token tüketir; fuzz zorunlu (§8.2).
3. **Öncelik**: Pratt parser, operator-precedence.md §3 tablosu; karşılaştırma
   operatörleri zincirlenmez (`a < b < c` → E0010); bit operatörleri
   karşılaştırmadan sıkı bağlar (`a & MASK == 0` → `(a & MASK) == 0`);
   şüpheli karışım W0010.

## Gerekçe

Kaynak: Volt-Dil-Spesifikasyonu-v3.md:44-54 ("Bu AI-üretilebilirlik
kontratının temelidir"); error-recovery.md:9-28 (LSP: "Parser çökerse
otomatik tamamlama çalışmaz"); operator-precedence.md §2.1-§2.2 ("C'de
`a < b < c` sessizce `(a<b) < c` olur… Rust bu kararı verdi; Volt
izler", "C'nin sırası tarihsel bir hatadır"). Emsal: Arch LL(1)
(Mimari-Kararlar:171), rustc kurtarma.

## Alternatifler

- PEG / geri izlemeli parser — belirsizliği gizler, hata konumu bulanık;
  reddedildi.
- C öncelik sırası — donanım kodundaki `a & MASK == 0` tuzağı; reddedildi.
- Python tarzı zincirli karşılaştırma — Rust'la tutarsız; reddedildi.

## Sonuçlar

- ADR-0023 bağlamsal anahtar kelimeler ve ADR-0034 `->` LL(2) sınırında
  çözüldü; her ikisi bu sözleşmeyi korur.
- Ayrılmış kelime listesi ADR-0028 ile daraltıldı (stdlib bileşenleri).
- F0 fuzz sonucu: 525k koşu, 0 panik (CHANGELOG 0.1.0).
