# ADR-0008: SV Çıktı Stili — `always_ff`, Açık Genişlik, Yasak Liste (x/z Üretilmez)

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: sv-mapping.md §0/§11/§13, volt-sv-emit, .github/workflows/ci.yml
> (Verilator lint), tests/fixtures/counter.expected.sv
> Uygulama aşaması: F0

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0008: SV Çıktı Stili" (Volt-Kodlama-Oncesi-Kritik-Adimlar.md:377-384).

## Bağlam

Üretilen SystemVerilog ASIC/FPGA akışına girer: lint araçları, sentez,
ECO mühendisleri ve SDC/XDC dosyaları onu okur. Çıktı hem okunabilir hem
lint-temiz olmalıydı.

## Karar

sv-mapping.md §0 ilkeleri ve §11 yasak listesi:
- Hedef IEEE 1800-2017; her dosya `` `default_nettype none`` ile başlar (§12).
- Çıplak `always` ASLA; `always_ff` / `always_comb` (İ3). `reg`, `wire`
  yerine `logic`; `initial` ve `#delay` yok.
- **Açık genişlik**: her sinyal `logic [N-1:0]`, örtük genişlik yok (İ4).
- **x veya z değeri üretilmez**; tek istisna çift yönlü port tamponu
  `'z` (ADR-0051, §17); `casex`/`casez` yok.
- İsimler ve `///` doc yorumları korunur (İ1, İ5); pozisyonel bağlama yok.
- Doğrulama: `verilator --lint-only -Wall` sıfır uyarı, CI'da zorunlu
  (ci.yml:41-42; sv-mapping.md §13).

## Gerekçe

Kaynak: Volt-Mimari-Kararlar-ve-CLAUDE-md.md:278-281 ("SV okunabilirlik
şartı (Veryl referansı): sinyal isimleri korunur, always_ff/always_comb,
FF seviyesinde elle düzenlenebilir"); sv-mapping.md:21-28 ("Lint araçları
latch/race uyarısı vermez", "Örtük genişlik çıkarımı yok"); Kodlama-Oncesi:
"Constraints dosyası (XDC/SDC) ile uyum". x/z yasağı: sentez sonrası
belirsiz davranış ve simülasyon/sentez uyumsuzluğu — gerekçe belgelenmemiş,
yasak listesinden çıkarıldı.

## Alternatifler

- Verilog-2001 `always @(posedge clk)` — daha geniş araç uyumu ama
  latch/race koruması yok; reddedildi.
- `'x` ile don't-care optimizasyonu — determinizm ve formal uyumu bozar.

## Sonuçlar

- `counter.volt → counter.expected.sv` birebir golden test (emit_tests.rs:84).
- Reset portu ve bloğu otomatik üretilir (§7; UX Anayasası Değişiklik 5).
- Türevler: ADR-0024 (dosya adı), ADR-0026 (başlık dili), ADR-0051 (`'z` istisnası).
