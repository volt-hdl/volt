# ADR-0012: SV Çıktı Öngörülebilirlik Garantisi — 1:1 Modül, İsim Korunumu, ECO Uyumu

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: sv-mapping.md §0 İ1/İ2, volt-sv-emit, volt-syntax/src/parser/desugar.rs,
> ADR-0024, ADR-0038, ADR-0042, ADR-0054
> Uygulama aşaması: F0

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Kod ve sonraki
ADR'ler bu numarayı referanslar: desugar.rs:1139 "elle yazılmış gibi
okunur (ADR-0012)", ADR-0038-pipeline-sozdizimi.md:156.

## Bağlam

ASIC akışında zamanlama ECO'ları üretilmiş netlist üzerinde elle
yapılır; derleyici yapıyı tanınmaz hale getirirse mühendis kaynağı
bulamaz (Veryl'in kaçındığı şey). Öngörülebilirlik değer hiyerarşisinde
ikinci sıradadır ("Aynı Volt kodu → aynı SV çıktısı",
Mimari-Kararlar:138-140).

## Karar

Varsayılan build modunda garanti:
- Volt modülü ↔ SV modülü **1:1**; sinyal adları **korunur**; `reg` ↔
  `always_ff` yapısal karşılık (sv-mapping.md İ1/İ2).
- Parser desugar'ı (pipeline, bundle, `@mmio`, bidir) elle yazılmış RTL
  gibi okunan Volt/SV üretir; sentetik adlar öngörülebilir kalıptadır
  (`<port>_oe`, `örnek/` öneki).
- Bu garantiyi bozacak optimizasyon varsayılanda **kapalı** kalır.

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:617-638 (Çakışma 1): "Varsayılan
modda garanti: Volt modülü ↔ SV modülü 1:1, isim korunur, reg ↔
always_ff → ECO mühendisi kaynağı tanıyabilir"; sv-mapping.md:12-18
"Constraints dosyaları (SDC/XDC) çalışmaya devam eder". Emsal: Veryl
okunabilir çıktı hedefi (Mimari-Kararlar:172).

## Alternatifler

- Agresif CIRCT optimizasyonu varsayılan — ECO imkânsızlaşır; reddedildi.
- İki mod (`--optimize=aggressive` FPGA için) — planlandı, uygulanmadı
  (ADR-0010: CIRCT yok).
- Hiyerarşiyi düzleştirme — isim ve modül eşlemesi kaybolur.

## Sonuçlar

- ADR-0024 dosya adlandırması ve ADR-0042 "modül başına `build/rtl/<Modül>.sv`"
  bu garantinin uzantısıdır.
- ADR-0054 SDC üretimi hücre adlarının kaynakla eşleşmesine dayanır.
- Tests: `counter.expected.sv` birebir eşleşme ve Verilator lint bu
  garantiyi her commit'te sınar.
