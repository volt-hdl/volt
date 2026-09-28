# Volt HDL — VS Code Uzantısı

`.volt` dosyaları için sözdizimi vurgulama ve dil sunucusu istemcisi
(tanılar, quick fix, inlay ipuçları, hover, otomatik tamamlama, tanıma
gitme, outline).

Uzantı VS Code Marketplace'te **yayınlanmaz**; her GitHub sürümüne
`volt-hdl-<sürüm>.vsix` olarak eklenir (ADR-0093).

## Gereksinimler

- `volt` çalıştırılabilir dosyası `PATH` üzerinde olmalı (sürüm arşivi ya
  da `cargo build --release` sonrası `target/release/volt`), ya da
  `volt.serverPath` ayarıyla tam yol verilmeli.

## Sürümden kurulum

Releases sayfasından `volt-hdl-<sürüm>.vsix` indirip
`code --install-extension volt-hdl-<sürüm>.vsix` (ya da Extensions
görünümü → `...` → *Install from VSIX...*).

## Kaynaktan paketleme (Node.js 18+ ve npm)

```sh
cd editors/vscode
npm ci
npx vsce package         # derler (vscode:prepublish) ve volt-hdl-0.1.0.vsix üretir
code --install-extension volt-hdl-0.1.0.vsix
```

Geliştirme için alternatif: VS Code'da `editors/vscode` klasörünü açıp
F5 (Run Extension) ile Extension Development Host başlatın.

## Doğrulama

Bir `.volt` dosyası açın:

- Sözdizimi renkli görünmeli (anahtar kelimeler, tipler, `@Domain`).
- CDC ihlali yazınca kırmızı altı çizgi + `E3001` görünmeli.
- `:` sonrası tip önerileri, `@` sonrası domain önerileri gelmeli.
- Sinyal üzerine gelince tip + domain, `F12` ile tanıma gitme çalışmalı.
- Tipsiz `let s = a + b` satırında `: u9` ipucu görünmeli; çok saatli
  modülde açıklamasız sinyalde `@Alan`, `@strict_timing` modülünde `+N`.
- `on` bloğunda `c = x` yazınca `E0006` ampulü `'<=' ile değiştir`
  önermeli (yalnız kesin düzeltmeler — ADR-0091).

## Ayarlar

| Ayar | Varsayılan | Açıklama |
|---|---|---|
| `volt.serverPath` | `volt` | `volt` çalıştırılabilir yolu; sunucu `volt lsp` ile başlar |
| `volt.inlayHints.types` | `true` | Tipsiz `let`/`reg`'in çıkarılan tipi |
| `volt.inlayHints.clockDomains` | `true` | Çok saatli modülde açıklamasız sinyalin saat alanı |
| `volt.inlayHints.latency` | `true` | `@strict_timing` modülünde çevrim gecikmesi |

İpuçlarının tamamı VS Code'un `editor.inlayHints.enabled` ayarıyla da
kapatılabilir.
