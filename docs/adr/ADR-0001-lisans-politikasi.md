# ADR-0001: Lisans Politikası — Apache-2.0 OR MIT

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03 (karar, F0 öncesi); LICENSE dosyaları 2026-09-07
> Etkilenen: Cargo.toml `[workspace.package] license`, LICENSE-APACHE,
> LICENSE-MIT, README.md "License", bağımlılık politikası (CLAUDE.md)
> Uygulama aşaması: F0

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Karar
`docs/design/Volt-Kodlama-Oncesi-Kritik-Adimlar.md` Adım 1.1'de alındı;
ADR dosyası o dönemde açılmadı.

## Bağlam

Lisans, değiştirmesi en pahalı karar sınıfındadır: "Lisans yanlış → her
dosya değişir → hukuki sorun" (Kodlama-Oncesi-Kritik-Adimlar.md:17;
Volt-Mimari-Kararlar-ve-CLAUDE-md.md:19 "telafi edilemez"). Derleyici
Rust ekosisteminde yaşayacak ve donanım IP'si üreten kurumsal
kullanıcılar hedefleniyor; patent sorusu ve bağımlılık uyumu baştan
çözülmeliydi.

## Karar

Çift lisans: kullanıcı Apache-2.0 **veya** MIT'yi seçer.
`Cargo.toml:19` `license = "MIT OR Apache-2.0"`; kökte `LICENSE-APACHE`
ve `LICENSE-MIT` (commit 1cc81de, 4ab5ad3); README.md "Licensed under
either of the Apache License, Version 2.0 or the MIT license, at your
option."

## Gerekçe

Kaynak: Volt-Kodlama-Oncesi-Kritik-Adimlar.md:26-41.
- Apache-2.0 birincil: açık patent izni ("MIT'de yok, önemli fark"),
  kurumsal güven, CIRCT/Filament/DAHLIA ile ekosistem tutarlılığı.
- MIT ek: bazı projeler Apache uyumsuz bağımlılık zorlar; "OR" seçim
  hakkı verir. Rust ekosisteminin (std, rustc) yerleşik kalıbı.

## Alternatifler

- Yalnız MIT — patent izni yok; reddedildi.
- Yalnız Apache-2.0 — MIT-only bağımlılık zincirleriyle sürtünme.
- GPL — viral; bağımlılık olarak bile yasak
  (Volt-Mimari-Kararlar-ve-CLAUDE-md.md:382 "GPL lisanslı bağımlılık YOK").

## Sonuçlar

- Yeni bağımlılık lisans uyumu için PR gerektirir (CLAUDE.md "Kesin Kurallar").
- Planlanan ama uygulanmayanlar: `.rs` dosyalarına SPDX başlığı
  (Kodlama-Oncesi:44-46; depoda 0 dosya), stdlib için ayrı MIT lisansı
  (stdlib derleyiciye gömülü, ADR-0027), CONTRIBUTING.md + DCO (1.2).
