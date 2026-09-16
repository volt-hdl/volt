# ADR-0009: Test Dosya Formatı — `tests/ui/{pass,fail}`, `//~` Anotasyonları, Birebir Fixture

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: tests/README.md, tests/ui/, tests/fixtures/, error-recovery.md §8,
> crates/volt-hir/tests/ui_semantic_tests.rs, scripts/check-consistency (kontrol 5-6)
> Uygulama aşaması: F0

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0009: Test Dosya Formatı" (Volt-Kodlama-Oncesi-Kritik-Adimlar.md:386-392).

## Bağlam

Dil değiştikçe hangi programın derlenmesi, hangisinin hangi kodla
reddedilmesi gerektiği makine-doğrulanabilir olmalı; hata mesajı
kalitesi (5 parça) de test edilmeli.

## Karar

- `tests/ui/pass/*.volt` hatasız derlenir ve SV üretir;
  `tests/ui/fail/*.volt` ilk satırı `//~ EXXXX` ile beklenen kodu verir,
  `//~^ ERROR mesaj` bir üst satırda hata bekler (tests/README.md;
  error-recovery.md §8.1).
- Harness yalnız kodu değil satırı da doğrular
  (ui_semantic_tests.rs:22-26 "error-recovery.md §8.1 kural 4").
- `tests/fixtures/counter.volt → counter.expected.sv` byte-aynı eşleşir
  (emit_tests.rs:84).
- Her yeni özellik en az 1 pass + 1 fail testi ister (CLAUDE.md);
  test sayısı `.test-baseline` altına düşemez; fail ilk satır biçimi
  betikle denetlenir (check-consistency.sh kontrol 5-6).

## Gerekçe

Kaynak: tests/README.md:3 "Test formatı rustc'nin UI test sisteminden
uyarlanmıştır"; Volt-Mimari-Kararlar-ve-CLAUDE-md.md:421-427 ("Her yeni
özellik: en az 1 pass + 1 fail testi", "fail testleri: tam hata kodu");
UX Anayasası BÖLÜM X "Her hata mesajı çözüm satırı içermeli (otomatik
kontrol)".

## Alternatifler

- `insta` snapshot dosyaları (Kodlama-Oncesi "cargo insta review";
  tests/README "snapshot (insta)") — kullanılmadı; birebir string
  karşılaştırma ve `//~` anotasyonları yeterli geldi.
- Beklentileri ayrı JSON/YAML dosyasında tutmak — fixture ile beklenti
  ayrışır; reddedildi.

## Sonuçlar

- UX Anayasası'nın önerdiği ayrı `tests/ux/` dizini açılmadı; aynı rol
  `tests/ui/` altında.
- Fuzzing zorunlu (error-recovery.md §8.2; F0'da 525k koşu, 0 panik).
- Çoklu dosya birimleri için `tests/ui/multifile/` eklendi (ADR-0042).
