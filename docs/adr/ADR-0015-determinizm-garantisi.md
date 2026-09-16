# ADR-0015: Determinizm Garantisi Kapsamı — Aynı Kaynak, Byte-Aynı Çıktı

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: cli-contract.md §0 İ3/§5, sv-mapping.md §12, volt-sv-emit `header()`,
> volt-hir/src/consteval.rs (sıralama), volt-diagnostics/src/messages/mod.rs
> Uygulama aşaması: F0-F2 (varsayılan determinizm); `--release`/volt.lock uygulanmadı

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0015: Determinizm garantisi kapsamı" (Mimari-v3:705-724, 913).

## Bağlam

Öngörülebilirlik değer hiyerarşisinde ikinci sırada (Mimari-Kararlar:138).
Fonksiyonel güvenlik (ISO 26262 tool qualification), IP sertifikasyonu ve
snapshot testleri "aynı girdi → aynı çıktı" ister; artımlı derleme ve
önbellek bunu tehdit eder.

## Karar

Uygulanan kapsam:
- Üretilen dosyalarda zaman damgası, mutlak yol, ortam değişkeni yok;
  başlık sabit (sv-mapping.md:396-413 "Üretim tarihi: deterministik
  build'de üretilmez"; ADR-0026 sabit İngilizce başlık).
- Const değerlendirme sırası DefId'ye sabitlendi; tanı konumu koşudan
  koşuya değişmez (CHANGELOG F2c "Determinizm düzeltmesi").
- Tanı dili sistem locale'inden okunmaz (messages/mod.rs:5, ADR-0004).
- Golden test: `counter.expected.sv` byte-aynı (emit_tests.rs:84).

Planlanan, uygulanmayan: `volt build` artımlı + `volt build --release`
sıfırdan deterministik + `volt.lock` (cli-contract.md:163; E9002 kodu
rezerve). Bugün tek mod vardır, her derleme sıfırdan ve deterministiktir.

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:705-724 ("Zaman damgası, yol adı,
ortam değişkeni çıktıya sızmaz"), 520-550 (volt.lock, "ISO 26262 tool
qualification kanıtı"), BÖLÜM VIII "Test: aynı kaynak → iki build →
bit-bit özdeş SV"; cli-contract.md:21-22 İ3.

## Alternatifler

- Önbellekli artımlı derleme varsayılan — determinizm riski; ertelendi.
- Başlıkta tarih (yaygın üreteç geleneği) — sahte diff; reddedildi (ADR-0026).

## Sonuçlar

- JSON çıktıda yalnız `duration_ms` koşuya bağlıdır (cli-contract.md §5).
- Artımlı derleme geldiğinde `--release` ayrımı ve E9002 bu ADR'ye
  dayanarak açılır; yeni ADR gerekir.
