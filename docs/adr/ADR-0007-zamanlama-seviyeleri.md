# ADR-0007: L0/L1/L2 Zamanlama Seviyeleri

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03 (karar); L1 uygulaması ADR-0037 (2026-09-10)
> Etkilenen: type-inference.md §12, grammar-full.ebnf (PipelineDecl, §17),
> volt-hir/src/timing.rs, ADR-0037, ADR-0038
> Uygulama aşaması: L0 F0, L1 F5 (ADR-0037), L2 V2

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). ADR-0037 bu kaydı
"ADR-0007'nin tanımladığı L1 zamanlama seviyesi" diye referanslar
(ADR-0037-l1-zamanlama.md:17); dosya yoktu.

## Bağlam

Boru hattı hizalaması RTL'de en sık sessiz hata kaynağıdır; ama her
tasarımcıya zamanlama tipleri dayatmak UX Anayasası İlke 2'yi
("kullanıcıyı cezalandırma") ihlal eder.

## Karar

Üç seviye, karmaşıklık talep üzerine:
- **L0 (varsayılan)**: derleyici zamanlama takip etmez; sıradan RTL
  (`@strict_timing` yoksa geçit hiç koşmaz — type-inference.md:875-877).
- **L1 (opt-in)**: `Delayed<T, N>` ve `delay<K>(x)`; `@strict_timing`
  modülde gecikme uyuşmazlığı E5010 (type-inference.md §12; ADR-0037).
- **L2 (V2)**: Filament tarzı timeline tipler, kaynak çakışması analizi
  (Volt-Butunlesik-Mimari-v3.md:876).

## Gerekçe

Kaynak: Volt-Dil-Spesifikasyonu-v3.md:360-385 (§7.1 "Üç Seviye");
Volt-Kodlama-Oncesi-Kritik-Adimlar.md:368-375 ("L0 varsayılan, L1
opt-in, L2 #[timeline]"); UX Anayasası İlke 3 "Karmaşıklık talep
üzerine". Emsal: Filament (Mimari-Kararlar:170 "Timeline tipler (L2
referansı)").

## Alternatifler

- Tüm modüllerde zorunlu timeline tipleri (Filament) — cezalandırıcı;
  reddedildi.
- Yalnız simülasyonla yakalama — ADR-0037'nin RV32I deneyi bunun
  yetersizliğini gösterdi.

## Sonuçlar

- Plandaki "L0 = otomatik hizalama" (Dil-Spes-v3:363-368) uygulanmadı;
  bugünkü L0 denetimsizdir.
- ADR-0038 `pipeline` sözdizimi L1 üzerine kuruludur.
- `#[timeline]` gramerde yok; L2 için yeni ADR gerekir.
