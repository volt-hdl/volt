# ADR-0010: Arka Uç — CIRCT Dialect Seçimi Ertelendi, Doğrudan SystemVerilog Üretimi

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: volt-sv-emit (string template), volt-lower (yer tutucu),
> scripts/check-consistency kontrol 4, grammar-full.ebnf aşama etiketi [F3]
> Uygulama aşaması: F0 — bugün hâlâ geçerli

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgeleri
"ADR-0010: CIRCT Dialect Seçimi — hw + seq + comb + sv" olarak planlar
(Volt-Kodlama-Oncesi-Kritik-Adimlar.md:394-400; Mimari-Kararlar:264-284).
Fiilen alınan karar plandan farklıdır: CIRCT hiç bağlanmadı.

## Bağlam

İlk mimari CIRCT/MLIR arka ucunu (melior crate) öngörüyordu: çok hedef,
optimizasyon geçişleri, "string template kırılgan" endişesi
(Mimari-Kararlar:269-272). Ancak F0 hedefi "Counter → SV → Verilator →
CI" idi ve Çakışma 1 CIRCT optimizasyonunun ECO uyumunu bozma riskini
gösterdi (Mimari-v3:617-638).

## Karar

- F0'dan itibaren SV üretimi `volt-sv-emit` içinde sv-mapping.md'ye
  birebir string template ile yapılır: "CIRCT yok — F3'te volt-lower
  devralacak" (volt-sv-emit/src/lib.rs:1-5).
- `volt-lower` tek satırlık yer tutucudur; CIRCT/melior referansı yalnız
  orada olabilir (check-consistency.sh:47-56). `Cargo.toml`'da melior yok.
- Dialect seçimi (hw/seq/comb/sv) yapılmadı; gerekirse yeni ADR.

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:752-757 ("F0: CIRCT yok, string
template yeterli"). ADR-0012'nin 1:1 modül / isim korunumu garantisi
string template ile doğrudan sağlanır; sv-mapping.md her yapının SV
karşılığını zaten tanımlar. CIRCT'e geçişi erteleme gerekçesi ötesi
belgelenmemiş; koddan çıkarıldı: ADR-0054'e kadar tüm üreticiler
(SV, SVA, SBY, sim tezgâhı, Rust/C, SDC) aynı şablon yaklaşımıyla yazıldı
ve Verilator/Yosys/OpenSTA doğrulamalarından geçti.

## Alternatifler

- CIRCT (hw/seq/comb/sv) — çok hedef ve optimizasyon; ertelendi.
- LLHD — "daha az topluluk, CIRCT zaten LLHD fikirlerini absorbe etti"
  (Mimari-Kararlar:274-276); reddedildi.

## Sonuçlar

- `--optimize=aggressive` modu (Mimari-v3 Çakışma 1) yok; tek mod, yapı korunur.
- grammar-full.ebnf'teki "[F3] CIRCT lowering" etiketi tarihseldir.
- İzolasyon sınırı korunduğu için CIRCT ileride yalnız `volt-lower`'ı etkiler (ADR-0005).
