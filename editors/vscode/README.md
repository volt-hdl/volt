# Volt HDL — VS Code Uzantısı

`.volt` dosyaları için sözdizimi vurgulama ve dil sunucusu istemcisi
(tanılar, quick fix, inlay ipuçları, hover, otomatik tamamlama, tanıma
gitme, outline).

Uzantı **yayınlanmaz** — yalnızca yerel kurulum içindir.

## Gereksinimler

- `volt` çalıştırılabilir dosyası `PATH` üzerinde olmalı
  (`cargo build --release` sonrası `target/release/volt`), ya da
  `volt.serverPath` ayarıyla tam yol verilmeli.
- Node.js 18+ ve npm.

## Yerel kurulum

```sh
cd editors/vscode
npm install
npm run compile          # TypeScript → out/extension.js
npx @vscode/vsce package # volt-hdl-0.1.0.vsix üretir
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
