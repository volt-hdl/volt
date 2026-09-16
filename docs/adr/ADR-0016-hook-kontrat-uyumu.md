# ADR-0016: Hook Mekanizması — Kontrat Altında Politika Enjeksiyonu (`hook` Rezerve)

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16) — yalnız rezervasyon uygulandı
> Tarih: 2026-09-03
> Etkilenen: grammar-full.ebnf §17 (ayrılmış: `hook`), volt-syntax/src/token.rs:167,
> E0003 davranışı
> Uygulama aşaması: F1 (rezerve); mekanizma V1+

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0016: Hook kontrat uyumu" (Mimari-v3:726-747, 914). Mekanizma
uygulanmadı; yalnız anahtar kelime rezerve edildi.

## Bağlam

Stdlib yapılarına (arbiter, fifo) kullanıcı politikası enjekte etmek
esneklik verir; ama kullanıcı kodu yapının invariantlarını bozabilir
(Mimari-v3 Çakışma 6: "Hook esnekliği vs kontrat garantisi").

## Karar

- `hook` kelimesi §17 AYRILMIŞ listesindedir; tanımlayıcı olarak
  kullanımı E0003 (grammar-full.ebnf:549; token.rs:162-167 "Ayrılmış
  anahtar kelimeler (§17, kullanımı E0003)").
- Tasarım niyeti: hook imzası `ensures` kontratı taşır; kullanıcı
  implementasyonu bu kontratı sağlamalı, formal ile doğrulanır; ihlal
  derleme hatasıdır (Dil-Spesifikasyonu-v3.md:333-349).

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:726-747 ("esneklik VAR, garanti
KAYBOLMUYOR"), 391-411 ("Neden stdlib fonksiyonundan üstün: yapının
güvenlik garantileri korunuyor"). Rezervasyon gerekçesi: "kullanıcı
bunları tanımlayıcı olarak kullanırsa V1'de dil genişlediğinde kodu
kırılır" (grammar-full.ebnf:555-557). Emsal: Arch HDL hook mekanizması
(Mimari-v3:766).

## Alternatifler

- Politikayı sıradan stdlib fonksiyonuyla geçirmek — invariant garantisi
  yok; reddedildi.
- `impl`/`trait` tabanlı genişletme — kelimeler rezerve, karar
  verilmedi.

## Sonuçlar

- Bugün politika değişikliği ayrı primitifle yapılır
  (`RoundRobinArbiter` / `PriorityArbiter`, ADR-0029).
- Hook uygulanırken ADR-0011 kontrat sözdizimi ve ADR-0027 stdlib
  mimarisi üzerine yeni ADR açılmalıdır.
