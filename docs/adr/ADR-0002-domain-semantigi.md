# ADR-0002: Domain Semantiği — `@Domain` Anotasyonu ve Birleşik Domain (Saat + Sıfırlama + Güç + Güven)

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03 (karar, F0 öncesi); CDC uygulaması F2c (2026-09-03)
> Etkilenen: grammar-full.ebnf §3 (DomainDecl), domain-inference.md §1-§2,
> volt-hir/src/domain.rs, volt-hir/src/trust.rs, E3xxx kod ailesi
> Uygulama aşaması: F2c (CDC), F2f (güven, ADR-0052); RDC/PDC rezerve

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgeleri bu
kararı "ADR-0002-cdc-semantik.md" adıyla referanslar
(Volt-Mimari-Kararlar-ve-CLAUDE-md.md:215, Volt-Butunlesik-Mimari-v3.md:898
"CDC+RDC+PDC BİRLEŞİK [REVİZE]"); dosya hiç yazılmamıştı.

## Bağlam

SystemVerilog iki saat alanı arasındaki doğrudan atamayı uyarısız
derler; hata silisyumda metastabilite olarak çıkar (README "The
problem"). Saat alanı bilgisinin dilde nasıl temsil edileceği ve
sıfırlama/güç/güven alanlarının ayrı mekanizmalar mı olacağı
belirlenmeliydi.

## Karar

1. Alan bilgisi tipe değil, **anotasyona** bağlanır: `data : u8 @Fast`.
   Tek saatli modülde anotasyon yazılmaz, çıkarılır (domain-inference.md
   K2; UX Anayasası Değişiklik 1).
2. `domain D { ... }` tek blokta dört boyutu tanımlar: `clock`/`frequency`,
   `reset*`, `voltage`/`always_on`/`retention`/`isolation` [V1],
   `trust_level` (grammar-full.ebnf:99-106). Gösterim:
   `DomainInfo { clock, reset, power, trust }` (domain-inference.md §1;
   volt-hir/src/domain.rs:52-63).
3. Alanlar arası geçiş açık köprü ister: `sync()`/`sync3()`; doğrudan
   atama E3001.

## Gerekçe

Kaynak: Volt-Mimari-Kararlar-ve-CLAUDE-md.md:199-216 (K1) ve
Volt-Butunlesik-Mimari-v3.md:83-130 (2.1).
- Phantom tip (`Signal<Domain, T>`, Clash) Rust'ta her sinyalde tip
  parametresi taşıtır: "aşırı verbose". Anotasyon görünür ve yereldir.
- Örtük (Verilog) yol CDC hatasını sessizce geçirir; değer hiyerarşisinde
  "doğruluk garantisi" en üsttedir (Mimari-Kararlar:134-136).
- Ayrı `@clock @reset @power` anotasyonları "verbose, hata yapmaya açık,
  üç ayrı kontrol" (Mimari-v3:117-123); tek kavram → CDC, RDC, PDC, SDC
  ve UPF aynı bilgiden türetilir (ZİNCİR 1).

## Alternatifler

- A: phantom tip parametresi — reddedildi (verbose).
- C: örtük saat bağlantısı — reddedildi (sessiz hata).
- Ayrı anotasyon sistemleri — reddedildi (Mimari-v3 BÖLÜM VIII "Bu
  birleşimi BOZMA").

## Sonuçlar

- E3001–E3014 tek çıkarım geçidinden üretilir; `trust_level` ADR-0052 ile
  aynı mekanizmaya oturdu; `frequency` ADR-0054 ile SDC'ye aktı.
- RDC (E3003) ve PDC (E3006) kodları tanımlı ama uygulanmadı (README
  "Not yet").
- Bu ADR domain semantiğinin kilit kaydıdır; değişiklik yeni ADR ister.
