# ADR-0003: Trit Tipi — Kısıtlı i2 ve Katmanlı Görünürlük

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03 (karar); tip kuralları F2a/F2b (2026-09-03)
> Etkilenen: type-inference.md §3.3/§3.6/§10, grammar-full.ebnf §17,
> volt-hir/src/ty.rs (`Ty::Trit`), volt-hir/src/typeck.rs
> Uygulama aşaması: F2

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgeleri
"ADR-0003-trit-tipi.md" ve "ADR-0003 revizyonu: Trit görünürlük
politikası" olarak referanslar (Volt-Mimari-Kararlar-ve-CLAUDE-md.md:303,
Volt-Butunlesik-Mimari-v3.md:679).

## Bağlam

Ternary/nöromorfik AI donanımı için üç değerli bir temel tip isteniyor;
aynı zamanda Trit'in dili "ezoterik" göstermemesi gerekiyor
(Mimari-v3 Çakışma 3, satır 662-680).

## Karar

- `Trit` = {-1, 0, +1}, 2 bit işaretli depolama (`Ty::Trit`,
  volt-hir/src/ty.rs:75).
- Aritmetik: `Trit * Trit → Trit` (kapalı), `Trit ± Trit → i3` (taşma),
  `Trit * iN → iN` (ternary MAC); sayısal → Trit dönüşümü yasak
  (E2009), literal {-1,0,+1} dışı E2011 (type-inference.md:223-232,
  424-427, 728-736).
- Görünürlük: README ve stdlib belgesinde Trit geçmez (README'de 0
  geçiş); belgeleme "Bölüm 10 — AI Donanımı"na ayrılır.

## Gerekçe

Kaynak: Volt-Mimari-Kararlar-ve-CLAUDE-md.md:287-303 (K5).
- Enum değil: "donanım seviyesinde 2 bit depolama şeffaf olmalı,
  aritmetik native çalışmalı".
- i3 veya özel tip değil: i2 doğal hizalıdır; kısıt derleme zamanında
  denetlenir.
- Görünürlük: "Trit öne çıkarsa ezoterik dil algısı; gizlenirse AI
  donanımı kullanıcıları bulamaz" → katmanlı görünürlük (Mimari-v3:662-680).

## Alternatifler

- `enum Trit { Neg, Zero, Pos }` — depolama/aritmetik şeffaf değil.
- `i3` — bir bit israf, kısıt ifade edilemez.
- Prelude'de görünür Trit — ilk izlenim riski.

## Sonuçlar

- `Spike`, `PTrit` §17'de rezerve (V2/V3 uzantıları).
- Planlanan `import volt::ternary::Trit` opt-in yolu (Mimari-v3:242-244)
  uygulanmadı: `Trit` bugün yerleşik anahtar kelimedir
  (grammar-full.ebnf:534). Planlanan FPGA verimsizlik uyarısı için kod
  tanımlanmadı.
