# CI iş akışları

| Dosya | Tetik | İçerik |
|---|---|---|
| `ci.yml` | push (main), PR | biçim + clippy + test, Verilator, tutarlılık, formal, OpenSTA, coverage, **60 sn fuzz** |
| `fuzz-nightly.yml` | her gün 03:00 UTC, elle (`workflow_dispatch`) | **30 dk fuzz**, corpus geceden geceye taşınır |
| `release.yml` | `v*` tag'i, elle (`workflow_dispatch` = kuru koşu) | 4 platform arşivi + `.vsix` + `SHA256SUMS` + derleme kaynağı kaydı; tag'de **taslak** Release (ADR-0093). PR'da koşmaz |
| `book.yml` | push (main), PR, elle | kitap (`book/`, mdBook): denetleyicinin öz-testi, her ```` ```volt ```` bloğu `volt check`'ten, her test bloğu Verilator'dan geçer; mdbook uyarısız derlenir. Yalnız main'de GitHub Pages'e yayımlar (https://volt-hdl.github.io/volt/; depo ayarı Settings → Pages → Source: GitHub Actions gerekir) |

## Fuzz

- PR/push'taki fuzz işi 60 saniyelik bir duman testidir; CI'nın toplam süresi
  artık en uzun iş (Verilator lint, ~2 dk) tarafından belirlenir. Önceden 300 sn
  fuzz tek başına ~6 dk sürüyordu.
- Gecelik iş corpus'u `actions/cache` ile saklar: en yeni
  `fuzz-corpus-parse_never_panics-*` girdisini geri yükler, üstüne fuzz'lar ve
  büyüyen corpus'u yeni bir anahtarla kaydeder (önbellek girdileri değişmez).
  Çökme olsa da corpus kaydedilir.
- PR fuzz işi aynı corpus'u yalnız okur (main'de kaydedilen önbellek PR'lara
  görünür); `tests/ui/` ve `tests/fuzz_regressions/` (eski fuzz bulguları,
  ADR-0067) her iki işte de salt okunur tohumdur.
- Çökme bulunursa girdi `fuzz-crash-parse_never_panics*` artifact'ı olarak
  yüklenir. Yerelde yeniden üretmek için (Linux/WSL, nightly):
  `cargo +nightly fuzz run parse_never_panics <indirilen-dosya>`.
- Elle tetikleme: `gh workflow run fuzz-nightly.yml -f seconds=1800`
  (`seconds` isteğe bağlı, varsayılan 1800).

## Sürüm yayımlama (ADR-0093)

İş akışı yalnız **taslak** oluşturur; yayımlamak her zaman elle yapılır.

1. Sürüm numarasını üç yerde aynı yap: `Cargo.toml`
   (`[workspace.package] version`), `crates/volt-sv-emit/src/lib.rs`
   (`VOLT_VERSION`) ve `editors/vscode/package.json` (`version`). İlk ikisi
   `cli_tests::version_flag_prints_the_cargo_package_version`, üçüncüsü
   `release.yml`'nin sürüm işi tarafından denetlenir. `CHANGELOG.md`'de
   `## [Yayımlanmadı]` başlığını `## [X.Y.Z] - YYYY-AA-GG` yap (taslağın
   notları bu bölümden alınır). PR + CI yeşil + merge.
2. İsteğe bağlı prova: `gh workflow run release.yml --ref main` (kuru koşu:
   Release oluşturmaz, arşivler `volt-release-X.Y.Z` artifact'ında).
3. Tag: `git tag -a vX.Y.Z -m "Volt X.Y.Z" origin/main` ve
   `git push origin vX.Y.Z`. Tag ile Cargo sürümü uyuşmazsa ilk iş düşer,
   hiçbir şey yüklenmez.
4. İş bitince Releases sayfasında `Volt X.Y.Z` taslağı görünür: dört arşiv,
   `volt-hdl-X.Y.Z.vsix`, `SHA256SUMS`. Notları gözden geçir, **Publish
   release**. Yanlış tag'i silmek için: taslağı sil,
   `git push origin :refs/tags/vX.Y.Z`.

Denetim: `gh attestation verify <arşiv> -R volt-hdl/volt` ve
`sha256sum -c SHA256SUMS`. VS Code Marketplace ve crates.io yayını bu iş
akışında yoktur (hesap/jeton gerektirir).
