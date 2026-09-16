# ADR-0022: SemVer Donanım Kuralları — `@version`, `@abi_version`, Anlamsal Diff (Rezerve)

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16) — nitelikler ayrışıyor, denetim V1
> Tarih: 2026-09-03
> Etkilenen: grammar-full.ebnf §2 (`@version`, `@abi_version` [F5]), cli-contract.md §17
> (E7001, E7002), volt-hir/src/attrs.rs (W0021 listesi, ADR-0048)
> Uygulama aşaması: V1 (paket registry ile birlikte)

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0022: SemVer donanım kuralları" (Mimari-v3:920, 551-580). Kodlar ve
nitelikler tanımlı; denetim uygulanmadı, kullanım W0021 üretir.

## Bağlam

IP paylaşımında "arayüz değişti mi?" sorusu port listesi + kontrat + tip
parametresi üzerinden makine tarafından yanıtlanabilir; yazılım SemVer'i
donanım arayüzüne uyarlanmalıdır.

## Karar

- Modül düzeyinde `@version("x.y.z")` ve `@abi_version(N)` nitelikleri
  (grammar-full.ebnf:88).
- Kural seti: port kaldırma/tip değişikliği → MAJOR; kontrat
  güçlendirme → MINOR; iç implementasyon (FSM kodlaması) → PATCH.
  İhlal E7001, abi_version değişmeden arayüz değişimi E7002
  (cli-contract.md:663-664).
- `volt diff <v1> <v2>` anlamsal fark + etkilenen test listesi (plan).
- Bugün: nitelikler ayrışır, hiçbir geçit okumaz → W0021 "uygulanmıyor"
  (ADR-0048); E7001/E7002 üretilmiyor.

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:551-580 (4.5 "[KIRICI] Port
kaldırıldı → MAJOR gerekli… E7001: SemVer ihlali"; "[UYUMLU] Kontrat
güçlendirildi → MINOR uygun"); ZİNCİR 2 "Kontrat → SemVer kırıcı
değişiklik tespiti" (Mimari-v3:1011); arayüz tanımı "port listesi +
kontratlar + tip parametreleri" (Mimari-v3:225).

## Alternatifler

- Yalnız `Volt.toml` sürüm alanı — anlamsal denetim yok; V1'e kadar
  fiilî durum.
- Kontratları arayüz saymamak — ensures zayıflatma sessiz kırılma olur;
  reddedildi.

## Sonuçlar

- V1 önceliği "Paket registry + SemVer donanım" (Mimari-v3:845).
- Uygulama ADR-0011 (kontrat), ADR-0017/0042 (paket) ve ADR-0048 (W0021
  listesinden çıkarma) üzerine yeni ADR ister.
