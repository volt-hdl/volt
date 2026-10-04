# ADR-0100: 1.0 Öncesi Sürüm Politikası — 0.x Numaraları, Yalnız Son Sürüm, "Behavior changes", Sürümle Eşleşen Kitap

> Statü: Kabul edildi — politika yürürlükte ve kitap düzeni (§6) kuruldu; ilk sürüm (v0.1.0) henüz atılmadı, sürüm notundaki "Behavior changes" bölümü henüz iş akışında denetlenmiyor
> İlgili: ADR-0093 (sürüm iş akışı: tag → taslak Release; bu ADR numaranın anlamını, notların biçimini ve desteği belirler), ADR-0022 (kullanıcı IP'si için SemVer — `@version`/`@abi_version`; Rezerve kalır, 1.0'ın önkoşulu olan SemVer sözünün parçasıdır).
> Tarih: 2026-10-04
> Etkilenen: `.github/workflows/book.yml` (kök = son sürümün kitabı, `/dev/` = main), `.github/workflows/README.md`,
> `scripts/check-consistency.{ps1,sh}` (kontrol 15 ve 17: `/dev/` adresleri),
> `docs/roadmap.md`, `AGENTS.md` (YENİ), `CHANGELOG.md`

## Sorun

Sürüm iş akışı (ADR-0093) ve kurulum betikleri (ADR-0096) hazır, ama
sürüm numarasının ne vaat ettiği yazılı değil: v0.1'den sonra hangi
değişiklik hangi haneyi artırır, eski sürüm düzeltme alır mı, sürüm notu
neyi söylemek zorunda, ne zaman sürüm çıkar. Yol haritası 1.0'ı bir
kararlılık sözüyle tanımlıyor, ama o sözün önkoşulları da yazılı değil.

İkinci sorun kitap: GitHub Pages her main push'unda main'in kitabını
yayımlıyor. İlk sürümden sonra kurulan ikili (en son sürüm) ile kitabın
anlattığı dil (main) birbirinden uzaklaşır: kitapta main'e yeni giren bir
özellik, kurulu sürümde E0001 verir.

Depoda `v0.1.0-f1`, `v0.2.0-f2`, `v0.3.0-f4` etiketleri var. Bunlar
geliştirme aşaması (F1, F2, F4) işaretleridir, sürüm değildir: Release'leri
yoktur ve kitap içermezler. SemVer önceliğinde `v0.2.0-f2`, `v0.1.0`'dan
büyük sayılır; "en son `v*` etiketi"ni safça seçen bir araç yanlış etiketi
bulur.

## Karar

### 1. 0.x numaralarının anlamı

Sürüm etiketi tam olarak `vX.Y.Z` biçimindedir (sonek yok). 1.0'a kadar
`X = 0`:

- **Orta hane (`0.Y.0`):** yeni özellik ve olası bozucu değişiklik. Önceden
  derlenen kodun hata vermesi, üretilen SV'nin değişmesi, bir tanı kodunun,
  çıkış kodunun, CLI seçeneğinin ya da JSON alanının değişmesi orta hane
  ister.
- **Son hane (`0.Y.Z`, `Z > 0`):** yalnız düzeltme. Yeni özellik, yeni
  seçenek, yeni tanı kodu yoktur. Tek istisna sessiz yanlıştır: başarılı
  görünüp yanlış sonuç veren bir hatanın düzeltmesi, önceden kabul edilen
  girdiyi reddedebilir; bu durumda son hanede çıkabilir ve "Behavior
  changes"te yazılır.

Bu, Cargo'nun 0.x yorumuyla aynıdır (`0.Y` uyumluluk sınırıdır). Sonekli
etiket (`-rc1`, `-fN` gibi) sürüm değildir; Release'i "pre-release" olarak
yayımlamak ADR-0093'e göre zaten yoktur. Eski aşama etiketleri tarih kaydı
olarak kalır; yeni sonekli etiket açılmaz.

### 2. Yalnız en son sürüm desteklenir

Düzeltmeler main'e girer ve sıradaki sürümle çıkar; eski bir sürüme geri
taşıma (backport) ve bakım dalı yoktur. Bir hata raporu en son sürümde ya
da main'de yeniden üretilmelidir. Kurulum betikleri her zaman en son sürümü
kurar (ADR-0096) ve kitabın kökü en son sürümü anlatır (§6).

### 3. Her sürüm notunda "Behavior changes" bölümü

Her sürümün notu (taslak Release, `CHANGELOG.md`'deki sürüm bölümünden
alınır — ADR-0093) `### Behavior changes` başlıklı bir bölümle başlar. O
bölüm, aynı girdiyle önceki sürümden farklı sonuç veren her değişikliği
listeler: önceden derlenen kodun artık hata ya da uyarı vermesi, üretilen
SV, SVA, SDC ya da sürücü çıktısının değişmesi, tanı kodu, çıkış kodu, CLI
seçeneği, varsayılan değer ya da JSON alanı değişikliği. Değişiklik yoksa
bölüm "None." der; bölüm hiçbir zaman atlanmaz. Başlık İngilizcedir, çünkü
Release notları kullanıcıya dönüktür ve araçlarla aranabilir olmalıdır.

Gündelik `CHANGELOG.md` girdileri Türkçe kalır ve davranış değişikliklerini
bugünkü gibi "**Davranış değişikliği.**" diye işaretler. Sürüm kesilirken
(ADR-0093 tarifi, adım 1) bu işaretli maddeler `### Behavior changes`
altında toplanır.

### 4. Sürüm sıklığı içeriğe göre

Sürüm takvime göre değil, içeriğe göre çıkar: main'de kullanıcıya dönük
bir iyileştirme biriktiğinde ve CI yeşilken. Sessiz yanlış düzeltmesi
beklemeden bir son hane sürümü gerektirir. Sabit tarih ya da sabit aralık
vaat edilmez (yol haritasının "No dates" kuralıyla aynı).

### 5. İlk sürüm v0.1.0; 1.0'ın önkoşulları

İlk sürüm `v0.1.0`'dır (`Cargo.toml` bugün `0.1.0`). Kararlılık sözü
yoktur: sözdizimi, tanılar ve çıktı sonraki her orta hanede değişebilir.

1.0 şu üç önkoşul karşılanmadan çıkmaz; her biri kendi ADR'sini ister:

1. **SemVer sözü:** Volt'un kendisi için kamuya açık arayüzün yazılı tanımı
   (dil, tanı kodları, CLI ve çıkış kodları, JSON şemaları, üretilen SV'nin
   ad eşlemesi) ve hangi değişikliğin büyük hane istediği. Kullanıcı IP'si
   için `@version`/`@abi_version` denetimleri (ADR-0022) bu sözün
   parçasıdır.
2. **`volt.lock`:** bir sürüm derlemesinin girdilerini kaydeden kilit
   dosyası ve `volt build --release` (ADR-0015).
3. **Paket yönetimi:** `Volt.toml`'dan başka Volt paketlerine sürüm
   çözümlemeli bağımlılık (ADR-0017, ADR-0042).

### 6. Kitap sürümle eşleşir

GitHub Pages sitesi (`book.yml`) üç parçadan kurulur:

| Yol | İçerik | Uyarı bandı |
|---|---|---|
| `/` (kök) | En büyük `vX.Y.Z` etiketinin `book/`'u | yok |
| `/` (kök), henüz sürüm etiketi yokken | main'in `book/`'u | "pre-release": henüz sürüm yok, kitap main'i anlatır |
| `/dev/` | Her zaman main'in `book/`'u | "development version": son sürümde olmayan özellikler içerebilir, sürümün kitabı köktedir |
| `/install.ps1`, `/install.sh` | Her zaman main'in `scripts/install/`'u | — |

- Etiket seçimi yalnız `^v[0-9]+\.[0-9]+\.[0-9]+$` kalıbına uyan
  etiketlere bakar, sürüm sıralamasıyla (`git tag --sort=-v:refname`).
  Aşama etiketleri ve sonekli etiketler seçilmez.
- Kurulum betikleri sürümden bağımsızdır (her zaman `releases/latest`
  indirirler), bu yüzden düzeltmeleri bir sonraki main push'unda yayına
  girer (ADR-0096 ile aynı).
- Kitap main push'unda, sürüm etiketi push'unda ve elle yeniden kurulur;
  PR'da site aynı adımlarla kurulur, yayımlanmaz.
- Var olan adresler bozulmaz: kökteki her sayfa yerinde kalır, `/dev/`
  yalnız eklenir. `volt explain` bağlantıları kökü gösterir; kurulu sürümün
  bağlantısı o sürümün kitabına gider.
- Tutarlılık denetimi (kontrol 15, 17) `https://volt-hdl.github.io/volt/dev/<ad>.html`
  adresini de `book/src/<ad>.md`'ye eşler; kontrol 13 kurulum betiklerini
  `book.yml`'nin kök kopyasından okumaya devam eder.

## Gerekçe

- 0.x'te orta haneyi bozucu saymak, Cargo ve crates.io ekosisteminin
  okurun zaten bildiği yorumudur; ayrı bir "0.x'te her şey kırılabilir"
  kuralından daha çok bilgi verir: son hane güvenle yükseltilebilir.
- Tek bakımcılı bir projede birden çok sürümü desteklemek, düzeltmeleri
  geciktirir; kurulum betikleri zaten en son sürümü kurar.
- "Behavior changes" bölümünün zorunlu olması ve "None." ile kapatılması,
  okurun bölümün unutulmasıyla boş olmasını ayırt etmesini sağlar.
  Projenin v0.1 kuralı (başarılı görünüp yanlış sonuç veren hata kalmaz)
  çoğu düzeltmeyi davranış değişikliği yapar; bunların görünür olması
  gerekir.
- Kitabın sürümü izlemesi, kurulumdan sonra okurun ilk denediği örneğin
  çalışmasını sağlar; `/dev/` katkıcılara ve main'den derleyenlere açık
  kalır.

## Alternatifler

- **Kitap her zaman main'den** (bugünkü hâl): ilk sürümden sonra kitap ile
  kurulu ikili uzaklaşır. Reddedildi.
- **Her sürüm için ayrı yol** (`/v0.1/`, `/v0.2/`): yalnız son sürüm
  desteklendiğinden eski kitaplar okuru eski sürüme yönlendirir. Gerektiğinde
  sonra eklenebilir; bugün yok.
- **Kök için en son yayımlanmış Release** (etiket yerine): taslak Release
  yayımlanana dek kök eski kalır, ama etiketlerden ayrı bir API çağrısı ve
  jeton ister. Etiket kalıbı yeterli: tag ile Release yayımı arasındaki
  aralık kısadır (ADR-0093 tarifi).
- **Takvimli sürüm** (ör. aylık): tek bakımcıyla boş sürümler ya da yarım
  özellikler doğurur. Reddedildi.

## Sonuçlar

- `.github/workflows/README.md`'nin sürüm tarifi "Behavior changes"
  bölümünü ve etiket biçimini söyler; Pages düzeni orada kısaca yazılıdır.
- `CHANGELOG.md`'nin "Yayımlanmadı" bölümündeki "Davranış değişikliği"
  maddeleri v0.1.0 kesilirken `### Behavior changes` altında toplanır
  (v0.1.0 ilk sürüm olduğu için "None." da yazılabilir; önceki sürüm yok).
- İleride: `release.yml`'nin sürüm işi, sürüm bölümünde `### Behavior
  changes` başlığı yoksa düşebilir. Bugün tarifte elle denetlenir.
- Eski kitabı yeni mdBook sürümüyle derlemek bozulabilir; o durumda
  `book.yml`'deki sabit `MDBOOK_VERSION` ile etiketin kitabı birlikte
  düşünülür.
