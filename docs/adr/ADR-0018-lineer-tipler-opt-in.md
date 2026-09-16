# ADR-0018: Lineer Tipler — Opt-in (`&inv`), Normal Portlarda Sürücü Analizi

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16) — kodlar rezerve, denetim V1
> Tarih: 2026-09-03
> Etkilenen: type-inference.md §11 (E4001/E4002 uygulandı, §11.4 E4003/E4004 [V1]),
> volt-hir/src/drivers.rs, volt-diagnostics messages (E4003, E4004)
> Uygulama aşaması: F2a (sürücü analizi); lineer denetim V1

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgeleri
"ADR-0018: Lineer tipler (port güvenliği) [YENİ ★]" (Mimari-v3:916,
973 "Bu kural ADR-0018'de tanımlı"). Lineer denetim uygulanmadı; sürücü
analizi aynı hata sınıfının büyük kısmını kapatıyor.

## Bağlam

Spade'in lineer tipleri "port iki kez bağlandı / hiç bağlanmadı"
hatalarını derleme zamanında yakalar. v3 bunu tüm portlara uygulamayı
planladı; UX Anayasası "port zaten tüketildi" hatasının yeni kullanıcıyı
şaşırtacağını saptadı (Değişiklik 2).

## Karar

- Lineer denetim yalnız `&inv` işaretli portlarda, açık opt-in; kodlar
  E4003 (çift tüketim) ve E4004 (tüketilmedi) rezerve, "[V1]" etiketli
  (type-inference.md:806-813; messages/en.rs:90-91).
- Normal portlar için **sürücü analizi** yeterli: E4001 çift sürücü,
  E4002 sürücüsüz çıkış, W4001/W4002 sürülen-ama-okunmayan
  (type-inference.md §11.1-§11.3; volt-hir/src/drivers.rs).
- `&inv` sözdizimi gramerde henüz yok.

## Gerekçe

Kaynak: Volt-UX-Anayasasi.md:436-447 ("Sadece &inv işaretli portlarda
(açık opt-in). Normal portlar: çift sürücü kontrolü (E4001) yeterli →
Lineer tipler ileri kullanıcı özelliği"); Volt-Butunlesik-Mimari-v3.md:179-200
(Spade emsali; "çift sürücü VE domain ihlali aynı anda yakalanıyor").

## Alternatifler

- Tüm portlarda lineer tip (v3) — cezalandırıcı; reddedildi.
- Yalnız lint uyarısı — sessiz hata sınıfı açık kalır; E4004'ün "uyarı
  değil, hata" olması karara bağlandı (Mimari-v3:971-972).

## Sonuçlar

- Bundle port yönleri (ADR-0039, E4005) ve Handshake (ADR-0050) sürücü
  analizi üzerine kuruldu; lineer tip beklemedi.
- `&inv` gramer ve E4003/E4004 üretimi için yeni ADR gerekir.
