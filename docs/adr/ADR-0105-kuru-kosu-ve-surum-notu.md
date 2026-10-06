# ADR-0105: Sürüm Kuru Koşusu Hiçbir Şey Yayımlamaz; Sürüm Notu `docs/release-notes/vX.Y.Z.md`

> Statü: Uygulandı — kuru koşu yolu dalda koşturuldu; `publish` işi ilk sürüm etiketiyle koşacak
> Önceki karar: ADR-0093, ADR-0100 — ADR-0093 §2.5'te derleme kaynağı kaydının kuru koşuda da üretilmesi, §2.6'da `checksums` işindeki kayıt ve notların CHANGELOG bölümünden alınması; ADR-0100 §3'te sürüm notunun kaynağı (CHANGELOG'daki sürüm bölümü)
> İlgili: ADR-0096 (kurulum betikleri: `VOLT_ARCHIVE` testleri sürüm arşivleriyle de koşar), ADR-0104 (duman testi `volt X.Y.Z (<commit>)` satırını bekler).
> Tarih: 2026-10-06
> Etkilenen: `.github/workflows/release.yml`, `.github/workflows/book.yml`
> (`release-notes` işi), `scripts/release/notes.py` (YENİ),
> `docs/release-notes/v0.1.0.md` (YENİ), `RELEASING.md` (YENİ),
> `.github/workflows/README.md`, `AGENTS.md`, `CHANGELOG.md`

## Sorun

1. **Kuru koşu yayımlıyordu.** ADR-0093 §2.5 derleme kaynağı kaydını
   "kanıt için" kuru koşuda da üretir. `actions/attest-build-provenance`
   kaydı Sigstore'un herkese açık şeffaflık günlüğüne (Rekor) ve deponun
   attestation deposuna yazar; ikisi de geri alınamaz. Hiç yayımlanmayacak
   bir arşiv için de `gh attestation verify` "bu depoda üretildi" der.
   Örnek: kuru koşu 36624446192 "Attestation signature uploaded to Rekor
   transparency log" ve "Attestation uploaded to repository
   (attestations/51250358)" yazdı.
2. **Notun kaynağı kullanıcıya dönük değildi.** ADR-0093 §2.6 ve ADR-0100
   §3 taslak Release'in notunu `CHANGELOG.md`'deki `## [X.Y.Z]`
   bölümünden alır. v0.1.0 için bu bölüm bugünkü `[Yayımlanmadı]`
   bölümüdür: aşamaların Türkçe geliştirme günlüğü, 119 912 karakter.
   GitHub Release gövdesinin üst sınırı 125 000 karakterdir; kısa,
   İngilizce bir not çıkmaz. `Behavior changes` bölümü de iş akışında
   denetlenmiyordu (ADR-0100'ün Statü notu).
3. **Kurulum betikleri sürüm arşivleriyle sınanmıyordu.** `install.yml`
   betikleri debug derlemeyle ve üç platformda sınar; `dist` profilindeki
   arşivler ve `x86_64-apple-darwin` arşivi kurulum testinden geçmiyordu.

## Karar

### 1. Kuru koşu: sürüm etiketi push'u olmayan her koşu

- `release.yml`'nin iş akışı düzeyindeki `DRY_RUN` değişkeni
  `!(github.event_name == 'push' && github.ref_type == 'tag')`'dir.
  `version` işi bunu `dry_run` çıktısı olarak verir ve koşunun türünü bir
  `::notice::` satırıyla yazar. Kuru koşu, herhangi bir daldan
  `gh workflow run release.yml --ref <dal>` ile başlar.
- Kuru koşu derleyen ve denetleyen her işi koşar: dört arşiv ve her
  platformda duman testi (`volt X.Y.Z (<commit>)`, ADR-0104), `.vsix`,
  kurulum betikleri (§3), `SHA256SUMS` ve `sha256sum -c` denetimi, sürüm
  notu (§2). Sonuçları yalnız iş akışı artifact'larıdır.
- Yayımlayan iki adım, derleme kaynağı kaydı (attestation) ve taslak
  Release, tek bir `publish` işindedir; işin koşulu
  `needs.version.outputs.dry_run == 'false'`. Yazma izinleri
  (`contents: write`, `id-token: write`, `attestations: write`) yalnız bu
  iştedir: kuru koşuda hiçbir iş Release açacak ya da imza kaydı yazacak
  izni taşımaz.
- Kuru koşunun son işi (`dry-run`) atlanan adımları iş özetine yazar ve o
  sürümün Release'inin ve etiketinin var olup olmadığını gösterir.
- Derleme kaynağı kaydı yalnız etiket koşusunda üretilir (ADR-0093
  §2.5'in "kuru koşuda da" cümlesinin yerine). Keyless imza zinciri
  (Fulcio sertifikası, Rekor kaydı) yayımlamadan denenemez; kuru koşu
  imzalanacak dosya kümesini üretip `SHA256SUMS` ile denetler, imzayı
  etiket koşusu atar.

### 2. Sürüm notu `docs/release-notes/vX.Y.Z.md`

- Her sürümün notu İngilizce olarak `docs/release-notes/vX.Y.Z.md`'dedir
  ve taslak Release'in gövdesi budur. `CHANGELOG.md` Türkçe geliştirme
  günlüğü olarak kalır; sürüm bölümü yine `### Behavior changes` ile
  başlar ve "**Davranış değişikliği.**" maddeleri sürüm kesilirken orada
  toplanır (ADR-0100 §3'ün kalanı yürürlükte).
- Biçim: üst satır `# Volt X.Y.Z` (Release başlığı "Volt X.Y.Z"
  olduğundan gövdeye girmez); ilk `##` bölümü `## Behavior changes` ve boş
  değil (değişiklik yoksa "None."); düzyazıda kitabın yasaklı sözcükleri
  (`book/tools/check_book.py`, `BANNED_WORDS`) yok; gövde en çok 125 000
  karakter.
- `scripts/release/notes.py` bu kuralları denetler: `--build X.Y.Z OUT`
  (`release.yml`'nin `checksums` işi, kuru koşuda da), `--check`
  (`book.yml`'nin `release-notes` işi, her PR'da), `--self-test`. Sürümün
  not dosyası yoksa ya da kurala uymuyorsa sürüm koşusu `publish`'ten önce
  düşer.
- Bu, ADR-0100'ün "Behavior changes bölümü henüz iş akışında
  denetlenmiyor" sınırını kapatır.

### 3. Kurulum betikleri sürüm arşivleriyle

`install` işi dört platformda, koşunun kendi arşiviyle
`scripts/install/test/test-install.sh` (Linux: bash ve sh; macOS: zsh ve
bash) ve `test-install.ps1` (PowerShell 7 ve Windows PowerShell 5.1)
koşar: `install.yml`'nin `VOLT_ARCHIVE` yolu. `publish` bu işi de
bekler.

### 4. Sürüm günü tarifi `RELEASING.md`

Bakımcının sürüm günü koşturacağı komutlar depo kökündeki
`RELEASING.md`'dedir. `.github/workflows/README.md` iş akışını anlatır ve
tarife bağlanır; `AGENTS.md` tarifin yerini söyler.

## Gerekçe

- Kuru koşunun amacı etiket koşusunu önceden sınamaktır. Geri alınamaz bir
  iz bu amaca gerekmez ve "hiçbir şey yayımlanmadı" sözünü bozar. Etiket
  koşusu kuru koşudan yalnız yayımlayan iki adımla ayrılınca, kuru
  koşudan geçen bir commit etiket koşusunda yalnız o iki adımda
  düşebilir.
- İzinlerin `publish` işinde toplanması koşulu yapısal yapar: kuru koşuda
  bir adım yanlışlıkla koşsa bile `contents: read` jetonuyla Release
  açamaz, `id-token` olmadan imza alamaz.
- Ayrı not dosyası kısa, İngilizce ve gözden geçirilebilir bir metin
  verir; PR'da denetlenir ve CHANGELOG'un büyüklüğünden bağımsızdır.

## Alternatifler

- **Kuru koşuda kaydı tutmak** (ADR-0093'ün seçimi): imza zincirini her
  kuru koşuda sınar, ama her kuru koşu herkese açık ve silinemez bir kayıt
  bırakır. Reddedildi.
- **Kuru koşuda yerel anahtarla imza** (`cosign sign-blob`,
  `--tlog-upload=false`): yayımlamaz, ama gerçek yolu (OIDC, Fulcio,
  attestation deposu) sınamaz ve yeni bir eylem bağımlılığı getirir.
  Reddedildi.
- **Notu CHANGELOG'dan almaya devam etmek**, sürüm bölümünü İngilizceye
  çevirip kısaltarak: geliştirme günlüğünü kaybettirir. Reddedildi.

## Sonuçlar

- İlk sürüm etiketi `publish` işinin ilk koşusudur; o koşuya dek
  attestation ve `gh release create` adımları gerçek ortamda koşmamıştır.
  `RELEASING.md` etiketten sonra `gh attestation verify` ve
  `sha256sum -c` denetimini bir adım olarak içerir.
- Sürüm notu dosyası yazılmadan sürüm kesilemez; kuru koşu da düşer.

## Doğrulama

- Yerel: `python scripts/release/notes.py --self-test` (10 durum, 0
  yanlış), `--check` (1 dosya, 0 hata), `--build 0.1.0` (11 389
  karakter).
