# ADR-0014: Kontrat Sistemi Kademeli Benimseme — Hiçbir Kontrat Zorunlu Değil

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: grammar-full.ebnf §4 (`{ Contract }` opsiyonel), volt-hir/src/attrs.rs
> (W0021 kapsamı), volt-driver `check` çıktısı, docs/stdlib.md
> Uygulama aşaması: F4

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0014: Kontrat sistemi kademeli benimseme" (Mimari-v3:682-703, 912).

## Bağlam

Kontrat ailesi geniştir (requires/ensures/invariant/cover, `@budget`,
`@timing`, `@dft`, `@debug_*`); yeni başlayan "boğulur". v3 taslağı
`volt check`'te "⚠ Bu modülde kontrat yok" uyarıları öngörüyordu; UX
Anayasası bunu cezalandırıcı buldu (Değişiklik 4).

## Karar

- Kontratsız modül derlenir ve **hiçbir uyarı almaz**; `ModuleBody =
  { Port } { Contract } { Stmt }` içinde kontratlar opsiyoneldir
  (grammar-full.ebnf:133).
- "Ödediğin kadar al": kontrat eklendikçe ilgili artifact (SVA, .sby,
  SDC) üretilir; eklenmediğinde sessizlik.
- Uyarı yalnız **yazılıp uygulanmayan** nitelik için verilir (W0021,
  ADR-0048) — eksik kontrat için asla.
- Belgeleme sırası: Seviye 1 RTL, 2 invariant+cover, 3 requires/ensures,
  4 ASIC nitelikleri, 5 üretim.

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:682-703 ("Hiçbir kontrat ZORUNLU
değil. Kontrat olmayan modül de derlenir… pay-as-you-go"); UX Anayasası
İlke 2 ("kontrat eksik → uyarı" YASAK), Değişiklik 4 ("Sessiz başarı.
Kontrat yoksa bahsedilmez"), BÖLÜM X "Kontrat/annotation eksikliği için
uyarı" yasak kalıp.

## Alternatifler

- Eksik kontrat uyarıları (v3) — reddedildi (cezalandırıcı).
- Zorunlu `@version`/`@dft` üretim modülleri için — reddedildi; ihtiyaç
  doğarsa proje politikası (Volt.toml lint) düzeyinde ele alınır.

## Sonuçlar

- `volt check` kontratsız tasarımda "0 error(s)" ile susar.
- Stdlib primitifleri kontrat taşır ama kullanıcıya görünmez (ADR-0027/0029).
- `@allow(unenforced)` ve `[lint]` politikası W0021 için ADR-0048'de.
