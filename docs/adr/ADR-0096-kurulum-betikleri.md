# ADR-0096: Kurulum Betikleri — Tek Komutla Doğrulanmış, Yönetici Yetkisi İstemeyen, Geri Alınabilir Kurulum

> Statü: Uygulandı — betikler, Pages yayını ve CI testleri kuruldu; yayımlanmış sürüm henüz yok, haftalık canlı iş o güne dek atlanır
> Önceki karar: ADR-0093 — §2.1 ve §2.4'teki sürümlü arşiv adları (`volt-vX.Y.Z-<hedef>`) sürümsüz oldu (`volt-<hedef>`)
> İlgili: ADR-0093 (sürüm iş akışı: arşivler, `SHA256SUMS`, taslak Release; betikler her sürüme varlık olarak eklenir), ADR-0094 (Docker köprüsü: kurulumdan sonra simülasyon için gereken tek ek), ADR-0084 (`volt doctor`: betiğin gösterdiği sonraki adım).
> Tarih: 2026-09-29
> Etkilenen: `scripts/install/` (YENİ: `install.ps1`, `install.sh`,
> `PSScriptAnalyzerSettings.psd1`, `test/`), `scripts/release/package.sh`
> (YENİ), `.github/workflows/install.yml` (YENİ), `release.yml`, `book.yml`,
> `.github/workflows/README.md`, `scripts/check-consistency.{ps1,sh}`
> (kontrol 13), `book/src/tour/install.md`, README "Installation"

## Sorun

Kitabın kurulum sayfası okurun Volt'u bir şekilde indirdiğini varsayıp
doğrudan PATH adımına geçiyordu; Releases bağlantısı dışında indirme yolu
yoktu. Okur testinde ilk sayfada takılındı. Arşiv adları sürüm taşıdığı
için (`volt-v0.1.0-...`) her sürümde değişen bir ad elle bulunmalıydı ve
alt klasör düzeni PATH adımını karıştırıyordu.

Hedef: desteklenen her platformda tek komutla, doğrulanmış, yönetici
yetkisi istemeyen, geri alınabilir bir kurulum; kitabın ve README'nin bunu
ilk yol olarak göstermesi. Derleyici değişmez (`crates/` dokunulmadı).

## 1. Kararlar

### 1.1 Kararlı indirme adresleri

Arşiv adlarından sürüm çıkarıldı: `volt-<hedef>.zip` / `.tar.gz`.
GitHub `https://github.com/volt-hdl/volt/releases/latest/download/<ad>`
adresini en yeni **yayımlanmış** sürümün aynı adlı varlığına yönlendirir;
ad sabit kaldıkça bağlantı hiç değişmez.

| Platform | Varlık adı | latest adresi |
|---|---|---|
| Windows x86_64 | `volt-x86_64-pc-windows-msvc.zip` | `…/releases/latest/download/volt-x86_64-pc-windows-msvc.zip` |
| Linux x86_64 (musl) | `volt-x86_64-unknown-linux-musl.tar.gz` | `…/releases/latest/download/volt-x86_64-unknown-linux-musl.tar.gz` |
| macOS x86_64 | `volt-x86_64-apple-darwin.tar.gz` | `…/releases/latest/download/volt-x86_64-apple-darwin.tar.gz` |
| macOS arm64 | `volt-aarch64-apple-darwin.tar.gz` | `…/releases/latest/download/volt-aarch64-apple-darwin.tar.gz` |
| hepsi | `SHA256SUMS` | `…/releases/latest/download/SHA256SUMS` |
| hepsi | `install.sh`, `install.ps1` | `…/releases/latest/download/install.sh` (sürüme sabit kopya, §1.8) |

- Arşiv yine tek bir klasöre açılır, adı arşivle aynı (`volt-<hedef>/`);
  sürüm arşivdeki `README.md`'de ve `volt --version`'da. İki sürümün
  arşivini aynı klasöre açmak artık çakışır; bunun için betik ya da ayrı
  klasör var.
- `.vsix` adı (`volt-hdl-X.Y.Z.vsix`) değişmedi: VS Code kurulumu adresle
  değil dosyayla yapılır.
- `latest` taslakları ve ön sürümleri (pre-release) görmez. Sürüm tarifine
  (`.github/workflows/README.md`) not eklendi: v0.1 **normal release**
  olarak yayımlanmalı.
- Paketleme `scripts/release/package.sh`'e taşındı; `release.yml` ve
  `install.yml` aynı betiği çağırır, böylece testteki arşiv sürümdekiyle
  aynı düzende.

### 1.2 Kurulum konumu

| Platform | Varsayılan | İkili |
|---|---|---|
| Windows | `%LOCALAPPDATA%\Programs\Volt` | `bin\volt.exe` |
| Linux, macOS | `~/.volt` | `bin/volt` |

Kök klasörde ayrıca `LICENSE-APACHE`, `LICENSE-MIT`, `README.md`.
`VOLT_INSTALL_DIR` kökü değiştirir. Gerekçe:

- Kullanıcının kendi klasörü: yönetici/`sudo` gerekmez.
  `%LOCALAPPDATA%\Programs` Windows'ta kullanıcı başına programların
  yeridir.
- Unix'te `~/.local/bin` yerine ayrı `~/.volt`: paylaşılan bir klasöre
  dosya bırakmamak, kaldırmayı "bizim dosyalarımız"la sınırlamak ve kendi
  `PATH` satırını taşımak (rustup `~/.cargo`, deno `~/.deno`, bun `~/.bun`
  aynı düzeni kullanır).

### 1.3 PATH politikası

- **Windows:** yalnız kullanıcı PATH'i (`HKCU\Environment\Path`); sistem
  PATH'ine dokunulmaz. Girdi **başa** eklenir, zaten varsa (büyük/küçük
  harf, sondaki `\` ve `%DEĞİŞKEN%` açılımı yok sayılarak) eklenmez. Değer
  kayıt defterinden `DoNotExpandEnvironmentNames` ile okunur ve aynı
  kayıt türüyle (`REG_SZ`/`REG_EXPAND_SZ`) yazılır.
  `[Environment]::SetEnvironmentVariable` kullanılmadı: değerdeki
  `%USERPROFILE%` gibi girdileri açar ve `REG_EXPAND_SZ`'yi `REG_SZ`'ye
  çevirir, yani PATH geri alınamaz biçimde değişirdi. Yazdıktan sonra
  `WM_SETTINGCHANGE` yayınlanır; Explorer'dan açılan yeni terminaller
  oturumu kapatmadan yeni PATH'i görür. O anki oturumun `$env:Path`'i de
  güncellenir (`irm | iex` çağıranın oturumunda çalışır).
- **Başa ekleme** yerel denemede ölçülen bir soruna karşı: bu makinede
  kullanıcı PATH'inde `C:\Tools\bin\volt.exe` vardı; sona eklenen girdi
  onun arkasında kaldı ve yeni terminal eski ikiliyi çalıştırdı. Sistem
  PATH'i kullanıcı PATH'inden önce arandığı için oradaki bir `volt.exe`
  yine kazanır; betik bu durumu **uyarır**, dokunmaz.
- **Linux/macOS:** kabuk `$SHELL`'den çıkarılır, ilgili başlangıç
  dosyasına **tek satır** eklenir: bash → `~/.bashrc` (Linux) /
  `~/.bash_profile` (macOS: Terminal oturum açma kabuğu başlatır), zsh →
  `${ZDOTDIR:-~}/.zshrc`, fish → `~/.config/fish/config.fish` (`set -gx
  PATH`), diğerleri → `~/.profile`. Satır `# added by the Volt installer`
  ile biter; varsa yeniden eklenmez. Betik neyi nereye eklediğini yazar.
  `curl | sh` çağıran kabuğun PATH'ini değiştiremez; betik o terminal için
  `export` satırını da gösterir.

### 1.4 Doğrulama

- Önce `SHA256SUMS`, sonra arşiv indirilir; arşivin satırı
  `<özet>  <ad>` ya da `<özet> *<ad>` biçiminde aranır ve özet
  karşılaştırılır (Unix'te `sha256sum` ya da `shasum -a 256`; ikisi de
  yoksa betik kurmayı reddeder). Uyuşmazlıkta ya da satır yoksa betik
  durur; indirme geçici klasördeydi ve silinir, kurulum klasörüne ve
  PATH'e hiçbir şey yazılmamıştır.
- `VOLT_ARCHIVE` ile yerel arşivde yanındaki `SHA256SUMS` kullanılır; yoksa
  uyarı verilir ve kurulum sürer (kendi derlediği arşivi deneyen katılımcı
  için). Sürüm kuru koşusunun artifact'ı (`volt-release-X.Y.Z`) arşivleri
  `SHA256SUMS` ile birlikte taşır.
- Sağlama toplamı aynı sunucudan gelir: bu, bozuk ya da yarım indirmeyi
  yakalar, GitHub hesabının ele geçirilmesine karşı korumaz. Onun için
  kitap `gh attestation verify` yolunu (ADR-0093 §2.5) gösterir.

### 1.5 Güncelleme ve kaldırma

- **Güncelleme** = aynı komutu yeniden çalıştırmak. İkili önce geçici bir
  adla kopyalanıp eskisinin üstüne taşınır (Unix `mv`), Windows'ta eski
  `volt.exe` önce `volt.exe.old` adına çekilir: çalışan bir `volt lsp`
  kilidi güncellemeyi engellemez. Betik eski ve yeni `--version`'ı yazar.
- **Kaldırma** = `VOLT_UNINSTALL=1`. Yalnız betiğin yazdığı dosyalar
  (`bin/volt`, iki lisans, `README.md`) silinir, klasörler yalnız boşsa
  kaldırılır: `VOLT_INSTALL_DIR` yanlışlıkla ev klasörünü gösterse de
  başka dosya gitmez. PATH girdisi (Windows) ya da işaretli satır (Unix)
  kaldırılır; Windows'ta kalan dize kurulumdan önceki dizeyle bayt bayt
  aynıdır (değer boş kalırsa değer silinir). Unix'te yalnız işaretli satır
  gider; dosya boş kalırsa (yalnız bizim satırımızı taşıyordu) silinir.
  Başlangıç dosyası satır sonuyla bitmiyorsa betik önce bir satır sonu
  ekler ve satırı `(file had no final newline)` ekiyle işaretler; kaldırma
  o satır sonunu da geri alır, dosya yine bayt bayt eski hâline döner
  (macOS runner'ının `~/.bash_profile`'ı böyleydi, CI'da yakalandı).
  Özel `VOLT_INSTALL_DIR` ile kurulduysa kaldırırken aynı değer verilir.

### 1.6 Ayarlar ortam değişkeniyle

`irm … | iex` ve `curl … | sh` biçiminde betiğe argüman verilemez; bu
yüzden bütün ayarlar ortam değişkenidir: `VOLT_VERSION` (belirli sürüm,
`v` önekli ya da öneksiz), `VOLT_INSTALL_DIR`, `VOLT_ARCHIVE`,
`VOLT_UNINSTALL=1`. `VOLT_` öneki derleyicinin kendi değişkenleriyle
(`VOLT_TOOL_BACKEND`, `VOLT_MANIFEST_DIR`) aynı ad alanındadır; çakışan ad
yoktur.

### 1.7 Sürüm yoksa, desteklenmeyen platform

- `SHA256SUMS` 404 dönerse (henüz yayımlanmış sürüm yok ya da
  `VOLT_VERSION` yok) betik "no published Volt release" der ve kaynaktan
  derleme komutlarını (`cargo install --locked --path crates/volt-driver`)
  ve kitaptaki bölümü gösterir. Başka ağ hataları ayrı iletiyle biter.
- Linux x86_64, macOS arm64/x86_64 dışında `install.sh` desteklenen
  platformları listeleyip durur; Rosetta altında çalışan bir kabukta
  (`sysctl.proc_translated = 1`) yerel arm64 ikilisini seçer; MSYS/Cygwin'de
  PowerShell komutunu gösterir.
- **arm64 Windows:** yerel arşiv yok. Betik bunu açıkça yazar ve x86_64
  arşivini kurar; Windows 11 on Arm onu x64 öykünmesiyle çalıştırır.
  (Windows 10 on Arm'da x64 öykünmesi yoktur; orada kaynaktan derleme
  gerekir.) 32 bit Windows desteklenmez, betik durur.

### 1.8 Yayın

- Kaynak `scripts/install/`. `book.yml` mdBook çıktısının köküne kopyalar:
  `https://volt-hdl.github.io/volt/install.ps1` ve `…/install.sh` (yalnız
  main'e push'ta yayımlanır, PR'da derlenir).
- `release.yml` iki betiği her sürüme varlık olarak ekler, `SHA256SUMS` ve
  derleme kaynağı kaydı onları da kapsar. Eklenen kopyada boş
  `PINNED_VERSION` / `$pinnedVersion` o sürümün numarasıyla doldurulur:
  `releases/download/vX.Y.Z/install.sh` X.Y.Z'yi kurar. Pages'teki kopya
  her zaman en yeni sürümü kurar.
- `install.ps1` yalnız ASCII'dir (Windows PowerShell 5.1 BOM'suz betiği ANSI
  kod sayfasıyla okur), `exit` çağırmaz (`iex` altında kullanıcının
  penceresini kapatırdı; hatalar `throw` edilir, `powershell -File` bunu
  çıkış kodu 1 yapar) ve bütün gövdesi tek bir betik bloğunda çalışır:
  çağıranın oturumuna işlev ya da tercih sızmaz (test bunu denetler).
- `install.sh` POSIX sh'dir (dash ile de çalışır), `curl` yoksa `wget`
  kullanır. Bütün iş `main` işlevindedir ve son satırda çağrılır: yarıda
  kesilen bir indirme hiçbir şey çalıştırmaz.

### 1.9 Tutarlılık (kontrol 13)

`scripts/check-consistency.{ps1,sh}` kontrol 13: README'de, kitapta ve
betiklerde geçen her `https://volt-hdl.github.io/volt/<ad>` adresi
`book.yml`'nin kopyaladığı bir betik olmalı; her
`releases/…/download/<ad>` adresi `release.yml`'nin ürettiği bir varlık
(matristeki `target`+`archive`, `SHA256SUMS`, iki betik); yayınlanan her
betiğin adresi README'de ve `book/src/tour/install.md`'de geçmeli.

## 2. Paket yöneticileri neden sonraya bırakıldı

winget, Scoop ve Homebrew tanıdık bir yol olurdu, ama bugün:

- **Yayımlanmış sürüm yok.** Üçü de indirme adresi ve SHA256 içeren bir
  manifest/formül ister; manifest ancak yayımlanmış bir sürümden yazılır.
- **Bakım ve inceleme süresi.** winget (`microsoft/winget-pkgs`) ve
  homebrew-core her sürümde ayrı bir PR ve dış inceleme ister; homebrew-core
  ayrıca kaynaktan derleme ve bilinirlik ölçütleri koyar. Kendi tap'ımız /
  bucket'ımız için ayrı bir depo ve her sürümde güncelleme işi gerekir.
- **İmzasız ikili.** winget, imzasız ikililerde SmartScreen uyarısını
  kaldırmaz; ikili imzalama (ADR-0093 §2.5) ayrı bir karardır.

Betikler bu kanalların yapacağını (indir, doğrula, PATH, güncelle, kaldır)
bugün, tek kaynaktan yapar. Paket yöneticileri v0.1 yayımlandıktan sonra,
betiklerin varlık adları ve `SHA256SUMS` biçimi üzerine eklenebilir (adlar
sürümsüz olduğu için bir manifest `releases/download/vX.Y.Z/<ad>`'ı
doğrudan kullanır). Bu, ayrı bir ADR konusudur.

## 3. Doğrulama

### CI (`install.yml`)

- PR ve main push'unda (betik/iş akışı yolları değişince): `shellcheck
  --shell=sh` (üç betik) ve PSScriptAnalyzer (tek dışlama:
  `PSAvoidUsingWriteHost`, gerekçesi ayar dosyasında), `install.ps1`
  ASCII denetimi.
- windows-latest, ubuntu-latest, macos-latest (arm64): volt derlenir,
  `scripts/release/package.sh` ile paketlenir, `SHA256SUMS` yazılır ve
  `scripts/install/test/` betikleri çalışır. Her kabuk için (Linux: bash,
  sh/dash; macOS: zsh, bash; Windows: PowerShell 7 ve ayrı adımda Windows
  PowerShell 5.1): `VOLT_ARCHIVE` ile kurulum, **yeni bir kabukta**
  (`env -i` ile çıplak PATH / PATH'i kayıt defterinden kuran yeni süreç)
  `volt --version`, ikinci kurulum (güncelleme; PATH girdisi tek), kaldırma
  (dosyalar yok, başlangıç dosyası ve kullanıcı PATH'i kurulum öncesiyle
  bayt bayt aynı, yeni kabuk volt'u bulmuyor), yanlış `SHA256SUMS` (kurulum
  durur, dosya, geçici klasör ya da PATH değişikliği kalmaz) ve var olmayan
  sürüm (`VOLT_VERSION=0.0.0`: kaynaktan derleme iletisi).
- Haftalık (pazartesi) ve elle: gerçek tek satırlık komutlar, Pages'teki
  betik ve en yeni yayımlanmış sürümle; sürüm yoksa iş bir not düşüp
  atlanır, düşmez.

### Yerel denetim (Windows 11, 2026-09-29)

Kullanıcı PATH'i önce dosyaya yedeklendi (`REG_SZ`, 720 karakter).
`test-install.ps1 -Shell powershell` (Windows PowerShell 5.1) bütün
adımları geçti; kurulum → Explorer'dan açılan yeni PowerShell penceresinde
`volt --version` → `VOLT_UNINSTALL=1` sonrası kullanıcı PATH'i yedekle
birebir aynı (değer ve tür). Bulgular:

- Yukarıdaki gölgeleme (`C:\Tools\bin\volt.exe`) → başa ekleme + uyarı.
- Test tarafı: Windows PowerShell 5.1 yerel bir programa verilen komut
  satırında çift tırnakları düşürür ve ASCII dışı karakterleri
  (`C:\Users\Çağlar`) bozar. Test komutları tek tırnak kullanır, yolları
  ortam değişkeniyle geçirir. Kurucu etkilenmez (yolları kendisi üretir).
- Test tarafı, CI'da: macOS `/bin/sh` (bash'in POSIX kipi) bir işlev
  çağrısından önceki `VAR=değer` atamasını çağrıdan sonra da tutar (dash
  tutmaz); test ayarları `env` ile geçirir. Beklenen başarısız son kurulumun
  `$LASTEXITCODE`'u GitHub'ın PowerShell sarmalayıcısında adımın çıkış kodu
  olur; test sonda açıkça `exit 0` der.
- `cargo install --locked --path crates/volt-driver` (kaynaktan kurulum
  komutu) ayrı bir köke denendi: `volt 0.1.0`.

## Sınırlar

- GitHub Pages'in `.ps1` dosyasını hangi içerik türüyle sunduğu ve `irm`'in
  onu dize olarak döndürdüğü ilk main yayınından sonra, haftalık canlı işte
  (sürüm yayımlanınca) kanıtlanır; PR'da ölçülemez.
- Sağlama toplamı ile ikili aynı kaynaktan gelir (§1.4); imzalama yok.
- Özel `VOLT_INSTALL_DIR` hatırlanmaz; kaldırırken yeniden verilmelidir.
- Windows 10 on Arm ve 32 bit Windows için ikili yok.
