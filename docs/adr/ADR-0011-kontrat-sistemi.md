# ADR-0011: Kontrat Sistemi — `requires` / `ensures` / `invariant` / `cover`

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03 (karar); uygulama F4a/F4b (2026-09-07, commit 5120b2a, bfad616)
> Etkilenen: grammar-full.ebnf §4 (Contract), volt-sv-emit/src/sva.rs,
> volt-sv-emit/src/sby.rs, volt-driver/src/verify.rs, cli-contract.md §2/§17
> Uygulama aşaması: F4

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgeleri
"ADR-0011-kontrat-sistemi.md" olarak referanslar
(Volt-Butunlesik-Cozum-RDC-Dogrulama-Spec.md:664; Mimari-v3:909, 947).

## Bağlam

Endüstride tasarım niyeti spec belgesinde, doğrulama ayrı SVA
dosyalarında yaşar; ikisi ayrışır ("spec uçurumu", Mimari-v3 ZİNCİR 5).
Volt'un ikinci ekseni niyeti kaynağın parçası yapmaktır.

## Karar

- Modül gövdesinde `ContractKind ":" Expr`; türler `requires`
  (ön koşul), `ensures` (son koşul), `invariant` (değişmez), `cover`
  (kapsam hedefi) (grammar-full.ebnf:189-194).
- Kontrat ifadesi Bool olmalı (E5004); `->` implikasyonu ADR-0034.
- Üretim: invariant/ensures → `assert property`, requires → `assume
  property`, cover → `cover property` (sva.rs:1-11); `--emit=sva` ayrı
  dosya + `bind`, `volt verify` Yosys uyumlu immediate assert.
- `volt verify` SymbiYosys koşturur; karşı örnek E5001, çıkış kodu 6.

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:290-333 (3.1 "Tüm kontratlar aynı
sözdizim ailesinde. Ayrı dosya yok, ayrı dil yok, ayrı araç yok"),
3.2 "Kontratın Sekiz Tüketicisi". Emsal: Rust `#[requires]`/Dafny
tarzı sözleşmeler; SVA hedef araç dili olarak kaldı.

## Alternatifler

- Ayrı `.sva` dosyası elle yazmak — kaynakla ayrışır; reddedildi.
- Yalnız simülasyon `assert` — formal kanıt yok; reddedildi.
- cocotb testbench'te kontrol — dil dışı; V1 tüketici olarak planlandı.

## Sonuçlar

- Kademeli benimseme kuralı ADR-0014; `prev()` ardışık kontratlar
  ADR-0040; Handshake otomatik protokol kontratları ADR-0050.
- Kaynak/zamanlama "kontratları" (`@budget`, `@timing`) bu ADR'nin
  kapsamı dışında: `@timing` ADR-0054 ile uygulandı, `@budget` W0021
  uyarısıyla bekliyor (ADR-0048).
- Stdlib primitifleri kendi kontratlarını taşır (ADR-0027, ADR-0029).
