# ADR-0135: Kitabın Adresi volthdl.org — Kök Son Sürümün, `/dev/` main'in Kitabı Kalır; Eski Adres Yönlenir

> Statü: Uygulandı
> Önceki karar: ADR-0099 — §3'teki kitap adresi ve tutarlılık kontrolü 16 (alınmamış alan adına bağlantı yok): kitap https://volthdl.org/ adresindedir, kontrol 16 kitabın eski adresini arar
> İlgili: ADR-0100 (§6'nın site düzeni aynı kalır: kök son sürüm etiketinin, `/dev/` main'in kitabı), ADR-0096 (kurulum betikleri sitenin kökünde kalır; adresleri https://volthdl.org/install.ps1 ve install.sh), ADR-0125 (§7: `volt explain` metnindeki kitap bağlantıları yalnız metin olarak değişir; yeni kod, seçenek ya da JSON alanı yok)
> Tarih: 2026-11-23
> Etkilenen: `book/book.toml` (`site-url`), `.github/workflows/book.yml`, `book/tools/assemble_site.py` (yorum), `book/tools/check_book.py`, `scripts/check-consistency.{ps1,sh}` (kontrol 13, 15, 16, 17), `scripts/install/` (betikler ve testleri), `scripts/release/package.sh`, `.github/workflows/install.yml`, `.github/workflows/README.md`, `crates/volt-diagnostics/src/explain/` (`en.rs`, `tr.rs`, `topics.rs`, `mod.rs`) ve testleri, `README.md`, `AGENTS.md`, `RELEASING.md`, `docs/roadmap.md`, `book/src/`, `book/CONTRIBUTING-BOOK.md`, `CHANGELOG.md`

## Sorun

Bakımcı 2026-11-23'te volthdl.org alan adını aldı ve GitHub Pages'e
bağladı. Ölçüm aynı gün (`curl -sI`):

| Adres | Yanıt |
|---|---|
| `https://volthdl.org/` | 200, `text/html` |
| `https://volthdl.org/dev/` | 200, `text/html` |
| `https://volthdl.org/install.sh` | 200 |
| `https://www.volthdl.org/` | 301 → `https://volthdl.org/` |
| `https://volt-hdl.github.io/volt/install.sh` | 301 → `https://volthdl.org/install.sh` |
| `https://volt-hdl.github.io/volt/tour/cdc-error.html` | 301 → `https://volthdl.org/tour/cdc-error.html` |

Depo ise eski adresi taşıyordu ve yeni adresi yasaklıyordu:

1. **Bağlantılar.** README, kitap, kurulum betikleri, iş akışları ve
   `volt explain`'in DAHA FAZLA bağlantıları `https://volt-hdl.github.io/volt/`
   altını gösteriyordu (21 dosyada 71 adres). Yönlenme onları kırmaz, ama
   her tıklama bir yönlenmeden geçer ve okur iki ayrı adres görür.
2. **Kontrol 16.** ADR-0099 §3, alan adı alınmadığı için
   `crates/`, `book/`, `README.md` ve `scripts/` altında volthdl.org'a
   bağlantıyı ihlal sayar. Artık tersi doğrudur.
3. **404 sayfası.** mdBook `site-url`'i `404.html`'e `<base href>` olarak
   yazar. `book/book.toml` `site-url = "/volt/"` diyordu. Yeni adreste
   site alt yolda değil kökte durur; `https://volthdl.org/olmayan-sayfa`
   bugün `<base href="/volt/">` taşıyan bir 404 döndürür ve sayfanın
   stilleri, betikleri ve bağlantıları `/volt/` altında aranır (yoktur).
   Sitenin kökü son sürüm etiketinin kitabıdır (ADR-0100 §6) ve o etiketin
   `book.toml`'u değişmez: v0.1.2 de `"/volt/"` der.

## Karar

1. **Kitabın adresi https://volthdl.org/'dur.** Sitenin yapısı ADR-0100
   §6 ile aynıdır:
   - `/` son `vX.Y.Z` etiketinin kitabı (etiket yoksa main'in kitabı,
     ön sürüm bandıyla);
   - `/dev/` main'in kitabı, her zaman, geliştirme bandıyla;
   - `/install.ps1`, `/install.sh` main'den, her zaman (ADR-0096).

   Yayın GitHub Actions ile sürer. Alan adı depo ayarındadır (Settings →
   Pages → Custom domain); depoya `CNAME` dosyası eklenmez: Actions ile
   yayında GitHub bu dosyayı kullanmaz.
2. **Eski adres yönlenir.** `https://volt-hdl.github.io/volt/<yol>`
   GitHub'ın yönlendirmesiyle `https://volthdl.org/<yol>`'a gider. Eski
   adresi taşıyan ve değişmeyecek metinler kırılmaz: yayımlanmış sürüm
   notları (`docs/release-notes/`), CHANGELOG'un yayımlanmış bölümleri,
   ADR karar metinleri, kurulu 0.1.x sürümlerinin `volt explain`
   metinleri ve kurulum betikleri.
3. **`site-url` köktür (`"/"`).** `book/book.toml` böyle der ve
   `book.yml`'nin "Build the book" işi aynı değeri
   `MDBOOK_OUTPUT__HTML__SITE_URL` ortam değişkeniyle iki derlemeye de
   verir: main'in ve son sürüm etiketinin. Böylece `book.toml`'u `"/volt/"`
   diyen eski bir etiketin kitabı da kökteki `404.html`'e doğru `<base>`'i
   yazar. mdBook 0.5.4 bu değişkeni `output.html.site-url`'ün yerine
   koyar (yerelde doğrulandı, aşağıda). `/dev/404.html`'in davranışı
   değişmez: GitHub Pages alt dizindeki 404'ü sunmaz, `assemble_site.py`
   ona bant eklemez (`keep_404`).
4. **Depodaki adresler yeni köke geçer:** `README.md`, `AGENTS.md`,
   `RELEASING.md`, `docs/roadmap.md`, `book/src/`, `book/CONTRIBUTING-BOOK.md`,
   `scripts/install/install.{ps1,sh}` ve testleri (saf ASCII ve BOM'suz
   kalır, kontrol 14), `scripts/release/package.sh`, iş akışı yorumları ve
   `install.yml`'nin canlı kurulum işleri, `volt explain` metinleri
   (`en.rs`, `tr.rs`, `topics.rs`, `mod.rs`'teki yorum) ve testleri.
   Yalnız metin değişir: yeni kod, seçenek ya da JSON alanı yoktur
   (ADR-0125 §7).
5. **Tutarlılık denetimi (`scripts/check-consistency.{ps1,sh}`, iki betik
   aynı kuralla):**
   - Sitenin kökü her betikte tek değişkendedir (`SITE_URL` / `$siteUrl`;
     düzenli ifade biçimi ondan türetilir). Kontrol 13 (kurulum
     adresleri), 15 (kitap sayfası bağlantıları) ve 17 (`volt explain`
     bağlantıları) bu kökle başlayan adresleri okur.
   - **Kontrol 16 tersine döner:** `crates/`, `book/`, `README.md` ve
     `scripts/` altında kitabın eski adresi (`volt-hdl` ve `.github.io`)
     ihlaldir; harf büyüklüğüne bakılmaz; `target/` ve mdBook çıktısı
     (`book/book/`) taranmaz; desen betik kendini yakalamasın diye bölünmüş
     yazılır. volthdl.org serbesttir. `docs/` tarihçedir, taranmaz.
   - `book/tools/check_book.py`'nin planlanmış bölüm denetimi iki adresi
     de okur: yayımlanmış sürüm notları eski adresi taşır ve onların
     bağlantıları da planlanmış bir bölüme gitmemelidir.
6. **Değişmeyenler:** `docs/spec/` ve `docs/research/` (düzenlenmez),
   `docs/release-notes/` ve CHANGELOG'un yayımlanmış bölümleri (tarihçe),
   ADR karar metinleri (ADR-0096, ADR-0099, ADR-0100'deki eski adresler).

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; karar dilin bir yazımını istemez, kitabın
   adresini ve deponun denetimini belirler.
2. **Çıkarım (İ2):** uygulanmaz. Denetim bir adresi tahmin etmez; metinde
   yazılı adresi kökle karşılaştırır.
3. **Anlam değişikliği (İ3):** dilin anlamı, üretilen SV ve SDC, tanı
   kodları ve hangi girdide çıktıkları aynıdır. Değişen, `volt explain`'in
   DAHA FAZLA satırlarındaki ve kurulum betiklerinin iletisindeki kitap
   bağlantısıdır (aynı sayfa, yeni adres). Önceden kontrol 16'yı geçen bir
   depo artık eski adresi taşıyorsa düşer; volthdl.org'u taşıyan depo
   artık geçer.
4. **Niyet tahmini (İ4):** uygulanmaz.
5. **Metamorfik yeniden yazım:** uygulanmaz.

## Doğrulama

- **Tutarlılık denetimi:** `just consistency` temiz. Negatif sınama
  (dosyalar sonra geri kondu): README'ye eski adresle bir bağlantı ve
  olmayan bir sayfa, `book/src/tour/install.md`'ye `/dev/install.sh` ve
  yayınlanmayan bir betik, `book/tools/check_book.py`'ye eski adres,
  `en.rs`'e `/dev/` ve olmayan bir sayfa eklendi. İki betik aynı yedi
  ihlali verdi (13 ×2, 15, 16 ×2, 17 ×2). `.sh`'nin önceki kontrol 16'sı
  `grep --exclude-dir=book` yüzünden `book/` dizininin tamamını atlıyordu
  (GNU grep komut satırındaki dizine de uygular); `.ps1` yalnız
  `book/book/`'u atlıyordu. Yeni kural iki betikte aynıdır.
- **`check_book.py --links-only --self-test`:** 7/7; yeni durum, eski
  adresle planlanmış bölüme giden sürüm notu bağlantısını yakalar.
- **mdBook 0.5.4 (yerel, Windows):** `mdbook build book` uyarısız.
  `MDBOOK_OUTPUT__HTML__SITE_URL=/` ile v0.1.2'nin kitabı (`book.toml`'u
  `"/volt/"`) `<base href="/">` yazar; değişkensiz `<base href="/volt/">`.
  `assemble_site.py --release <v0.1.2> --tag v0.1.2` ile kurulan sitede
  `404.html` ve `dev/404.html` ikisi de `<base href="/">`; `dev/404.html`
  bant taşımaz.
- **Canlı:** yukarıdaki tablo. Bu kararın PR'ı birleşip main'de book
  koşusu yayımlayana dek canlı 404 `<base href="/volt/">` taşır.

## Sınırlar

- Yönlenme GitHub'a bağlıdır: depo adı ya da Pages ayarı değişirse eski
  bağlantılar (sürüm notları, kurulu 0.1.x'in metinleri) kırılabilir.
- Kontrol 16 `docs/`'u taramaz; ADR karar metinleri ve sürüm notları eski
  adresi korur.
- `docs/spec/cli-contract.md` `https://volthdl.org/errors/<KOD>`,
  `https://volthdl.org/guide/<ad>` ve "Daha fazla: https://volthdl.org"
  biçimlerini gösterir. Sitede `/errors/` ve `/guide/` yoktur; `explain_url`
  ADR-0099 §3 gereği `null` kalır. Kod başına sayfa ayrı bir karardır.

## Reddedilenler

- **`CNAME` dosyası:** Actions ile yayında gereksizdir; alan adı depo
  ayarındadır.
- **Eski etiketlerin `book.toml`'unu yamamak:** etiket değişmez.
- **`404.html`'i `assemble_site.py`'de yeniden yazmak:** mdBook'un kendi
  ayarı aynı sonucu daha az parçayla verir ve kitabın ürettiği her yerde
  aynı değeri kullanır.
- **Kontrol 16'yı kaldırmak:** eski adres yeni metne kopyalanabilir ve
  yönlenmeye bağlı kalırdı.
