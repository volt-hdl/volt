# ADR-0004: Tanı Sözleşmesi — Hata Kodu Formatı, 5 Parçalı Mesaj, Tanı Dili

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03 (kod formatı + 5 parça); 2026-09-07 (İngilizce varsayılan, commit 1dc21b6)
> Etkilenen: volt-diagnostics (code.rs, diagnostic.rs, emit.rs, messages/),
> cli-contract.md §3/§5/§17, GLOSSARY.md §7, scripts/check-consistency
> Uygulama aşaması: F0 (format), F4 sonu (i18n)

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0004-hata-kodu-format.md" olarak referanslar
(Volt-Mimari-Kararlar-ve-CLAUDE-md.md:261).

## Bağlam

Hata kodu formatı geriye uyumluluk taşır ("yanlış → 1 hafta",
Mimari-Kararlar:16). Mesaj kalitesi değer hiyerarşisinde üçüncü sırada
(Mimari-Kararlar:142-144); UX Anayasası kriptik ve çözümsüz hatayı yasaklar.
Tasarım belgeleri Türkçe yazıldığından tanı dili de kararlaştırılmalıydı.

## Karar

1. **Kod formatı** `E`/`W` + 4 hane; ilk hane kategori: E0 sözdizimi,
   E1 isim, E2 tip, E3 domain, E4 sürücü, E5 kontrat, E6 bütçe, E7 sürüm,
   E9 yapı (volt-diagnostics/src/code.rs:33-40; cli-contract.md §17).
2. **5 parça zorunlu**: kod + konum + açıklama + çözüm (help) + spec
   referansı; `Diagnostic::validate()` denetler
   (volt-diagnostics/src/diagnostic.rs:1-4), tutarlılık betiği bypass
   arar (check-consistency.sh kontrol 3). İnsan çıktısı codespan-reporting
   (emit.rs); `= reason:` / `= help:` / `= for more: volt explain KOD`
   satırları (UX Anayasası BÖLÜM IV).
3. **Dil**: İngilizce varsayılan, Türkçe opt-in; öncelik
   `--lang` > `VOLT_LANG` > `Volt.toml [ui] lang` > en; sistem locale'i
   bilerek okunmaz (messages/mod.rs:3-5; cli-contract.md:102). Joker kolsuz
   `match` her yeni kodu iki dilde zorlar (commit 1dc21b6).

## Gerekçe

Kaynak: Volt-Mimari-Kararlar-ve-CLAUDE-md.md:242-261 (K3): "kategori
koddan görünür, E3xxx = CDC akılda kalıcı, araçlar filtreleyebilir";
rustc emsali. 5 parça: UX Anayasası "Eksik olan mesaj MERGE EDİLEMEZ"
(satır 562). Locale okunmaması: "CI'da sürpriz üretir" (messages/mod.rs:5).

## Alternatifler

- Sıralı numaralama (E0001, E0002, …) — kategori görünmez; reddedildi.
- Serbest metin hata (kodsuz) — `volt explain` ve JSON filtreleme imkânsız.
- Sistem locale'inden dil — CI determinizmini bozar; reddedildi.

## Sonuçlar

- Yeni kod: spec veya ADR'de tanım + `messages/{en,tr}.rs` + `explain`;
  tutarlılık betiği eksikliği yakalar.
- Üretilen SV'nin dili bu karardan bağımsız sabit İngilizcedir (ADR-0026).
