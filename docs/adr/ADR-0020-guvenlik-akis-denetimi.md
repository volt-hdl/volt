# ADR-0020: Güvenlik Akış Denetimi — `trust_level` Domain'in Dördüncü Boyutu

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16); uygulama ADR-0052
> Tarih: 2026-09-03 (karar); uygulama 2026-09-15 (ADR-0052)
> Etkilenen: grammar-full.ebnf §3 (`trust_level`), domain-inference.md K11,
> volt-hir/src/trust.rs, E3009 / E0016 / W3008
> Uygulama aşaması: F2f (ADR-0052)

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0020: Güvenlik akış denetimi (trust_level)" (Mimari-v3:918); F2f
"Güvenlik akış denetimi ← YENİ" (Mimari-v3:781). Uygulama ayrıntıları
ADR-0052'dedir; bu kayıt yalnız mimari yerleşimi belgeler.

## Bağlam

Kriptografik çekirdeklerde gizli anahtarın debug/public yollara sızması
RTL'de sessizdir. Ayrı bir bilgi akışı tip sistemi (`Signal<Secret,T>`)
kurmak yerine mevcut domain mekanizmasının kullanılıp kullanılamayacağı
sorusu vardı.

## Karar

- Güven seviyesi domain bloğunun bir alanıdır: `trust_level = secret |
  confidential | public` (grammar-full.ebnf:105-106); `DomainInfo.trust`
  (domain-inference.md:48-50; volt-hir/src/domain.rs:59-62).
- Akış kuralı CDC/RDC/PDC ile aynı çıkarım geçidine oturur: yüksekten
  düşüğe akış E3009; meşru düşürme yalnız gerekçeli `declassify()`.
- Varsayılan: `trust_level` yazılmazsa sınıflandırılmamış, geçit koşmaz
  (UX Anayasası tablosu "Trust level → kullanıcı görmez").

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:259-286 (2.5 "Trust level domain'in
bir özelliği olduğu için CDC/RDC/PDC ile aynı mekanizmayı kullanıyor —
ayrı sistem değil"); BÖLÜM VIII "trust_level → güvenlik akışı (E3009)";
ADR-0002 birleşik domain kararının doğrudan uzantısı.

## Alternatifler

- Ayrı `@Secure`/`@Public` anotasyon sistemi — ikinci mekanizma, çift
  öğrenme; reddedildi (Mimari-v3 "Bu birleşimi BOZMA").
- Tip parametresiyle taşıma (`Signal<Secret, T>`) — ADR-0002'nin phantom
  tip reddiyle aynı gerekçeyle reddedildi.

## Sonuçlar

- ADR-0052: kafes, K11 takma adı kuralı, `declassify` iz kaydı (W3008),
  E0016 gerekçe zorunluluğu.
- Planlanan `@constant_time` / `@no_power_leak` nitelikleri (Mimari-v3:281-282)
  gramerde yok; V2.
