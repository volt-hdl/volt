# ADR-0099: Öneri, İleti ve Açıklama Kalitesi — Gidiş-Dönüş Denetimi, Okura Dönük Metin, W0025, Derleme Damgası, Ön Uç E3002

> Statü: Uygulandı
> İlgili: ADR-0004 (5 parçanın spec referansı `volt explain <KOD>` satırıdır; JSON `explain_url` kitapta sayfa olmadığından `null`), ADR-0091 (quick fix çok düzenlemeli olabilir; kesin düzeltme kuralı aynı), ADR-0094 (Docker köprüsü: derleme damgası arka ucu ve imaj özetini taşır; test düzeninde arka uç açık), ADR-0098 (eki 2'nin flop denetimi üreticide yedek kalır; ön uç E3002), ADR-0031 (uN/iN 1..=64; daha genişi E0003 sınırı söyler), ADR-0061 (Volt.toml araması), ADR-0079 (araç gerektiren testlerin atlama/düşme kuralı).
> Tarih: 2026-10-03
> Etkilenen: volt-diagnostics (`Suggestion` düzenlemeleri, `a_an`, W0025, explain metinleri, `explain_url`), volt-hir (öneriler, `manifest_lint`, ön uç E3002), volt-syntax, volt-sv-emit (E0003 geniş tamsayı, E2028 önerisi), volt-lsp (çok düzenlemeli quick fix), volt-driver (doctor, `verify --engine`, derleme damgası, testler), `scripts/check-consistency.*` (kontrol 16, 17), `book/tools/check_book.py`, `book/src/limitations.md`, `docs/roadmap.md`.

## Sorun

Ölçümler 2026-10-03, `main` = ab0e02a:

| # | Girdi | Önce |
|---|---|---|
| 1 | `on sclk { r <= a }`, `a` başka alanda (E3001) | Öneri `dest = sync(src, <clock of Slow>)`; blok içine yazılınca E0003 ("sync() inside an expression or a block") |
| 2 | `y : u4 = a + 1`, `a : u8` (E2001) | İleti "a 8-bit value"; öneri `(expr) as u4` uygulanınca W2010 |
| 3 | `y : u4 = a[3:0]` (E2003) | Genel ileti; `bits<4>`'ün sayıya dönüşümü söylenmiyor |
| 4 | İki alanın paylaştığı otomatik senkron reset (W3010) | İki yan cümlelik ileti, iç belge atfı |
| 5 | Yazılıp okunmayan register `q` (W1004) | "_q öneki" yalnız bildirimi adlandırsa yazmalar E1001 olurdu |
| 6 | `volt explain` ve JSON | Alınmamış bir alan adına bağlantılar; `build/rtl/blink.sv` (gerçek: `Blink.sv`); stdlib için iki farklı bileşen sayısı; tanı, explain ve doctor metinlerinde yüzlerce ADR numarası |
| 7 | `volt doctor`, Docker kurulu ama kapalı (Windows) | İpucu "install Docker Desktop" |
| 8 | `volt doctor`, Docker köprüsü | Sorulmadan "bitwuzla is not in this image" |
| 9 | Yarıda kalmış ya da Docker'da kurulmuş Verilator obj dizini | Yeniden kullanılıyor; make yabancı yollarla düşüyor, elle silmek gerekiyor |
| 10 | `in a : u128` | E0003 "user-defined type 'u128'" |
| 11 | Volt.toml'da `scr = "rtl"`, `[dependencies]` | Sessizce yok sayılıyor; `src` varsayılanda kalıyor |
| 12 | `on d` (`d : bool`), `in c1 : clock @c2` / `in c2 : clock @c1` | Yalnız SV üreticisinin flop denetimi durduruyor ("derleyici hatası" notuyla); editör göstermiyor. İleri başvuru (`in c1 : clock @c0` sonra `in c0 : clock`) ön uçta sessizce Error alanı |
| 13 | Sahte araç yolu bozuk test | Otomatik geri düşüş gerçek Docker konteynerleri başlatıyor (bir koşuda 6) |

## Karar

### 1. Öneri bir ya da daha çok düzenlemedir

`Suggestion { edits, applicability }`; düzenleme `Replace` (aralığı
değiştirir) ya da `LineAbove` (metni, aralığın satırının üstüne aynı
girintiyle yeni satır olarak ekler; girinti tanı üretilirken bilinmez,
tüketici kaynaktan çözer). JSON şeması aynen kalır: birincil düzenleme
`span`/`replacement`, diğerleri yalnız varsa `additional_edits`. LSP
quick fix tüm düzenlemeleri tek `WorkspaceEdit`'te uygular; kesin
düzeltme kuralı (tanı başına tek `machine-applicable`) değişmez.

### 2. Gidiş-dönüş sözleşmesi (kalıcı)

Her öneri uygulanınca özgün tanı gider, yeni hata ya da uyarı çıkmaz ve
hatasız kalan dosya `volt build` ile derlenir
(`tests/suggestions/`, `suggestion_roundtrip_tests`). Kaynakta öneri
kuran her yer `// suggestion: <fikstür>` işaretini taşır (yardımcılar
`// suggestion-helper: <ad>`); işaret ve fikstür birebir eşleşir,
işaretsiz yer ya da fikstürsüz işaret testi düşürür; `tests/ui`
derlemindeki her önerili kodun fikstürü vardır. Volt.toml önerileri için
fikstür bir dizindir (`Volt.toml` + `main.volt`).

Bağlama uygun biçimler: E3001/E3012 modül düzeyinde kaynağı `sync()`
ile sarar, blok içinde bloğun üstüne `let x_sync = sync(x, clk)` ekleyip
adı okur; yalnız tek bitlik, adı yazılabilen kaynakta (çok bitli veri
W3003'tür). E2001 daraltması `(ifade)[N-1:0] as uN` (uyarısız),
operand uyumsuzluğu dar operandı genişletir. E2003 `bits<N>` → `uN` için
`(ifade as uN)`. W3010/E3003/W3009 ham reset portunu ilk portun üstüne
ekler. W1001/W1004/W3004 `_` önekini bildirim ve bütün kullanımlara
yazar; portlarda öneri yoktur (örnekler portu adıyla bağlar).

### 3. Okura dönük metin

- Kitap https://volt-hdl.github.io/volt/ adresindedir. Kod başına
  kitap sayfası yoktur: `explain_url` `null`, LSP `codeDescription`
  yok; spec referansı insan çıktısının `volt explain <KOD>` satırıdır.
  `volt explain`'in DAHA FAZLA bölümü yalnız yazılmış bir kitap bölümüne
  bağlanır, yoksa yazılmaz. Tutarlılık kontrolü 16: `crates/`, `book/`,
  `README.md`, `scripts/` alınmamış alan adına bağlantı vermez; kontrol
  17: explain'deki her kitap bağlantısı `book/src` altında planlanmamış
  bir bölümdür.
- Tanı, explain, `--list` ve doctor metinleri ADR numarası ve spec dosya
  atfı taşımaz (okur bunları bilmez); kalıcı testler iki dilde denetler.
- Sayıdan önceki İngilizce artikel `a_an(n)` ile seçilir (8, 11, 18,
  80–89, 800 için "an"); iletiyi artikelsiz yeniden yazmak yerine bu
  yol seçildi: iletiler doğal İngilizce kalır ve değişiklik artikelle
  sınırlı kalır.
- Metinde zamanla eskiyecek sabit sayı (bileşen sayısı, imaj boyutu,
  araç sürümü) yoktur; sürümleri `volt doctor` listeler.
- uN/iN 64 bitten genişse E0003 sınırı söyler ve `bits<N>` önerir.

### 4. W0025 — Volt.toml'da bilinmeyen anahtar ya da bölüm

Okunan anahtarlar: `[package]` `name`, `src`, `top`; `[test]` `paths`;
`[lint]` `unenforced_attributes`; `[ui]` `lang`. Gerisi W0025'tir;
bilinen bir ada bir iki harf uzak yazım "did you mean" alır (yer
değiştirme tek düzenleme sayılır). `[dependencies]` "package management
is not available yet" der. Uyarı manifest başına süreçte bir kez
üretilir; proje kipinde kaynak bulunamayınca hata satırından önce basılır.

### 5. Verilator derleme dizini damgası

Başarılı derleme obj dizinine `.volt-build-stamp` yazar: Volt sürümü,
arka uç (`local`/`docker`), Verilator sürümü (yerelde yoluyla) ve Docker
imajının özeti. Derlemeden önce damga yoksa ya da uyuşmuyorsa dizin
silinip temizden kurulur; damga derleme süresince kaldırılır.

### 6. Doctor ve `verify --engine`

Docker kurulu ama kapalıysa kurulum ipucu "start Docker Desktop" der.
İmajın içermediği çözücü doctor'da söylenmez; `volt verify --engine
bitwuzla` Docker köprüsünde konteyner başlatmadan çıkış 3 ile söyler.

### 7. Ön uç E3002

Domain çıkarımı aynı kodla iki durumu yakalar: `on X`'te X modülün saat
portu değilse; saat portu açıklama zinciri hiçbir alana varmıyorsa
(döngü, modül başına bir kez). Zincir port sırasından bağımsız çözülür:
ileri başvuru geri başvuruyla aynı alanı alır. Üreticinin flop denetimi
yedek olarak kalır.

### 8. Test düzeninde araç arka ucu açık

`volt test/run/verify` çalıştıran her Rust testi ve kitap denetleyicisi
`VOLT_TOOL_BACKEND`'i kendisi verir: `local` (araç yoksa test açıkça
düşer ya da ADR-0079 kuralıyla atlanır) ya da otomatik geri düşüşü
sınayan testlerde bilerek `auto`. `tool_backend_harness_tests` bunu
denetler.

## Kapsam dışı

`volt test`/`volt run` için `--format=json`: test kaydının şeması yok;
limitations.md ve yol haritasında ("Machine-readable test results").
Docker arka ucu için makine genelinde eşzamanlı konteyner sınırı: yol
haritası.
