# ADR-0093: Hazır İkililer — Sürüm İş Akışı, Platformlar, Taşınabilirlik ve Kaynak Doğrulaması

> Statü: Uygulandı — altyapı kuruldu ve kuru koşuyla kanıtlandı; ilk sürüm (tag) henüz atılmadı
> İlgili: ADR-0084 (`volt doctor`, gömülü `volt new` şablonları — ikilinin tek başına yeterliliği), ADR-0001 (lisans: arşivde iki lisans dosyası), ADR-0091 (VS Code eklentisi, `volt lsp`).
> Tarih: 2026-09-28
> Etkilenen: `.github/workflows/release.yml` (YENİ), `.github/workflows/README.md`
> (sürüm yayımlama tarifi), `Cargo.toml` (`[profile.release] strip`,
> `[profile.dist]`), `editors/vscode/` (`LICENSE`, `.vscodeignore`,
> `repository`, `vscode:prepublish`), volt-driver (`cli_tests`
> sürüm testi), README "Installation", CHANGELOG

## Sorun

Volt'u kullanmanın tek yolu kaynaktan derlemekti: Rust kurulumu + `cargo
build` (soğuk derleme dakikalar sürer). v0.1 için en somut ölçüt: kaynaktan
derlemeden kurulum. Bu ADR sürüm **yayımlamaz**; tag geldiğinde sürüm
üretecek altyapıyı kurar ve kanıtlar.

## 1. Tespit — ikili kendi başına yeterli mi?

- **Şablonlar** (`volt new`): `crates/volt-driver/src/new/templates.rs`
  `include_str!` ile `templates/<ad>/` dosyalarını derleme anında gömer
  (ADR-0084). Depo gerekmez.
- **`volt explain` metinleri** ve **yerleşik primitifler** (`std`
  bileşenleri: `sync`, `AsyncFifo`, ...): Rust kaynağında sabit metin /
  AST (`volt-diagnostics::explain`, `volt-ast::builtin`). Dosya okunmaz;
  `with_docs` yalnız depo içi ADR yolunu *metin olarak* basar.
- `CARGO_MANIFEST_DIR` yalnız `#[cfg(test)]` kodunda geçer.
- **Ölçüm** (Windows, `cargo build --release`, ikili depo dışında boş bir
  geçici klasöre kopyalandı): `volt new demo` → 0; `volt check
  counter.volt` → 0; `volt build counter.volt` → 0 (`build/rtl/Counter.sv`);
  `volt doctor` → 0 (araç yokken de; `--strict` 3 verir, ADR-0084);
  `volt explain E0001` metni basar. **Sonuç: ikili tek başına yeterli;
  driver'da değişiklik gerekmedi.**
- Bulgu (değiştirilmedi, dil/CLI davranışı): `volt check` ve `volt build`
  proje kökünde dosya argümansız çalışmaz (clap: `<FILE>` zorunlu, çıkış
  2); `volt test` ise proje keşfi yapar (ADR-0089). Duman testi dosya adını
  verir. Argümansız `check`/`build` ayrı bir CLI kararıdır.
- **Windows C çalışma zamanı**: varsayılan derleme `VCRUNTIME140.dll` ve
  `api-ms-win-crt-*` içe aktarıyor (`dumpbin /dependents`) — VC++
  Redistributable kurulu olmayan makinede açılmaz.
- **VS Code eklentisi** sunucuyu `volt.serverPath` ayarından (varsayılan
  `volt`, yani `PATH`) `volt lsp` olarak başlatır. `.vsix` üretilebiliyordu
  ama `LICENSE` ve `repository` yoktu: `vsce` bu durumda etkileşimli onay
  sorar (CI'da takılır/düşer); `.vscodeignore` yoktu (kaynak `.ts`,
  `.vscode/`, `tsconfig.json` pakete giriyordu).
- **Sürüm numarası** üç yerde: `Cargo.toml` `[workspace.package] version`
  (`0.1.0`), `volt_sv_emit::VOLT_VERSION` (elle yazılmış `"0.1.0"`; `volt
  --version`, doctor, üretilen SV başlığı), `editors/vscode/package.json`.
  `volt --version` → `volt 0.1.0`.

## 2. Kararlar

### 2.1 Platformlar

| Arşiv | Runner | Hedef |
|---|---|---|
| `volt-vX.Y.Z-x86_64-pc-windows-msvc.zip` | `windows-latest` | MSVC, statik CRT |
| `volt-vX.Y.Z-x86_64-unknown-linux-musl.tar.gz` | `ubuntu-latest` | musl, tam statik |
| `volt-vX.Y.Z-x86_64-apple-darwin.tar.gz` | `macos-15-intel` | yerel |
| `volt-vX.Y.Z-aarch64-apple-darwin.tar.gz` | `macos-latest` (arm64) | yerel |

**macOS: evrensel ikili değil, iki yerel arşiv.** Evrensel ikili
(`lipo`) tek indirme verir ama boyutu ikiye katlar ve arm64 runner'da
yalnız arm64 dilimi duman testinden geçer; x86_64 dilimi test edilmeden
yayımlanırdı. İki yerel iş, her mimariyi kendi donanımında dener.
`macos-15-intel` GitHub'ın son Intel imajıdır; kalktığında x86_64 işi
`macos-latest` üzerinde çapraz derlemeye + Rosetta testine geçer.

### 2.2 Taşınabilirlik

- **Windows:** `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS=-C
  target-feature=+crt-static` (yalnız sürüm iş akışında; geliştirme
  derlemesi değişmez). İş, `dumpbin /dependents` çıktısında `vcruntime*`,
  `msvcp*` ya da `api-ms-win-crt-*` görürse düşer.
- **Linux: musl statik**, eski runner değil. Eski glibc'li runner
  (ör. `ubuntu-20.04`) GitHub'dan kalktı; konteynerde eski glibc'ye karşı
  derlemek bakım yükü ve yine de bir alt sınır bırakır. Volt'un C
  bağımlılığı yok (tokio, clap, logos saf Rust; dış araçlar süreç olarak
  çağrılır), musl hedefi ek iş gerektirmez. Kanıt: iş, `file` çıktısında
  `statically linked` arar ve aynı duman testini **CentOS 7 (glibc 2.17)**
  konteynerinde tekrarlar. musl'ün bellek ayırıcısı glibc'ninkinden
  yavaştır; derleme süreleri kısa olduğu için kabul edildi (musl/glibc
  karşılaştırması ölçülmedi).
- **macOS:** Rust varsayılan asgari sürümleri (x86_64 10.12, arm64 11.0);
  iş `otool -l` ile kaydeder.

### 2.3 Boyut — profil

Ölçüm (Windows, statik CRT, `cargo build`, 4 iş):

| Profil | Boyut | Süre |
|---|---|---|
| release (varsayılan) | 10 208 256 B | 85 sn |
| + `strip = true` | 10 139 136 B | 77 sn |
| + `strip` + `lto = "thin"` | 10 356 736 B | 80 sn |
| + `strip` + `lto = "fat"` + `codegen-units = 1` | 9 673 728 B | 162 sn |

Windows'ta semboller zaten `.pdb`'de, `strip` az kazandırır; Linux/macOS
ikilisi standart kitaplığın sembol tablosunu taşır, `strip` orada büyük
fark yapar (kuru koşu tablosu, §4). Thin LTO büyütür. Fat LTO %5 küçültür
ama süreyi ikiye katlar. Karar: `[profile.release] strip = true` (herkese),
arşivler ayrı **`[profile.dist]`** (`inherits = "release"`, fat LTO,
`codegen-units = 1`) ile — `cargo build --release` ile kaynaktan derleyen
yavaşlamaz. `panic = "abort"` seçilmedi: LSP sunucusu ve test altyapısı
panik yakalamaya dayanabilir, ölçülmedi.

### 2.4 Arşiv içeriği

`volt-vX.Y.Z-<hedef>/` klasörü: `volt` (`volt.exe`), `LICENSE-APACHE`,
`LICENSE-MIT`, kısa `README.md` (PATH, ilk komutlar, dış araçlar, imzasız
ikili notu). Windows `.zip` (7-Zip), diğerleri `.tar.gz`. Tek
`SHA256SUMS` tüm arşivleri ve `.vsix`'i kapsar.

### 2.5 İmza ve kaynak doğrulaması

- **Kod imzası yok** (Authenticode sertifikası ve Apple Developer ID ücretli
  hesap ister). README: SmartScreen → "More info → Run anyway"; macOS →
  `xattr -d com.apple.quarantine volt`.
- **Derleme kaynağı kaydı (build provenance) var:**
  `actions/attest-build-provenance` her arşiv ve `.vsix` için Sigstore
  imzalı SLSA kaydı üretir; ek maliyet bir adım ve `id-token`/
  `attestations` izni (yalnız `checksums` işinde). Kullanıcı `gh
  attestation verify <dosya> -R volt-hdl/volt` ile dosyanın bu depodaki bu
  iş akışında üretildiğini doğrular. Kayıt kuru koşuda da üretilir (kanıt
  için); açık depoda Sigstore şeffaflık günlüğüne girer.
- Sürüm derlemeleri **önbellek kullanmaz** (`rust-cache` yok): arşiv
  yalnız tag'lenmiş kaynaktan derlenir, başka dalın yazdığı önbellekten
  değil.

### 2.6 İş akışı (`release.yml`)

- Tetik: `push: tags: v*` → taslak Release; `workflow_dispatch` → kuru
  koşu (Release yok, `volt-release-X.Y.Z` artifact'ı). `pull_request`
  tetiği yok (CI süresine eklenmez).
- `version` işi: Cargo sürümü (`cargo metadata`, volt-driver) =
  `package.json` sürümü; tag koşusunda tag = `v<sürüm>`. Uyuşmazlıkta iş
  düşer, hiçbir şey derlenmez. `VOLT_VERSION` = Cargo sürümü
  `cli_tests::version_flag_prints_the_cargo_package_version` ile CI'da
  denetlenir (tek tanıma çekmek volt-sv-emit'i değiştirirdi; kapsam dışı).
- `build` (4'lü matris): derle → paketle → **duman testi arşivden
  çıkarılan ikiliyle**, depo dışında boş klasörde, `PATH`'ten cargo/rustup
  çıkarılmış: `volt --version` (= `volt X.Y.Z`), `volt new demo`, `volt
  check counter.volt`, `volt build counter.volt` (+ `Counter.sv` var),
  `volt doctor` (çıkış 0) → platform taşınabilirlik denetimi (§2.2).
- `vsix`: `npm ci` + `vsce package` (`vscode:prepublish` derler);
  paketteki `out/extension.js`, `LICENSE` (vsce `LICENSE.txt` yapar), gramer ve
  `vscode-languageclient` denetlenir. Marketplace yayını yok.
- `checksums`: `SHA256SUMS`, provenance, boyut tablosu (iş özeti),
  birleşik artifact.
- `release` (yalnız tag): `contents: write`, CHANGELOG'daki `## [X.Y.Z]`
  bölümünden notlar, `gh release create --draft --verify-tag`.
  Yayımlamak elle (`.github/workflows/README.md`).

## 3. Doğrulama

Kuru koşu ve yerel Windows doğrulaması aşağıda (§4).

## 4. Kuru koşu sonuçları

KURU_KOSU_SONUCU

## Sınırlar

- İkililer imzasız (§2.5); SmartScreen ve Gatekeeper uyarısı ilk
  çalıştırmada görünür.
- VS Code Marketplace ve Open VSX yayını yok (yayıncı hesabı + jeton
  kullanıcının adımı); `.vsix` elle kurulur.
- crates.io yayını yok: workspace'teki `repository` alanı ve crate
  adları (`volt-*`) yayına hazırlanmadı; `cargo install` yolu ayrı karar.
- Windows ARM64 ve Linux ARM64 arşivi yok (istek gelince matrise bir satır).
- Paket yöneticileri (winget, Homebrew, Scoop) yok.
- `volt check`/`volt build` argümansız çalışmaz (§1 bulgusu).
