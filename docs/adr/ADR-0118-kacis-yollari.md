# ADR-0118: Kaçış Yolları — Dar, Bilinçli, Gerekçeli, Görünür ve Denetlenebilir

> Statü: Önerildi
> İlgili: ADR-0048 (W0021, `@allow(unenforced)`, Volt.toml `[lint]`), ADR-0050 (`@no_protocol_check`), ADR-0052 (`declassify`, E0016, W3008), ADR-0054 (`@false_path` / `@multicycle` çevrilir, yapısal kanıt yok), ADR-0066 (`@no_auto_contracts`), ADR-0099 (eksik kaçış işaretinin düzeltmesi gidiş-dönüş testinden geçer), ADR-0100 (son hanede yeni tanı kodu ve yeni seçenek yok, önceden kabul edilen girdinin reddi orta hane ister: bu ADR'nin bütün adımları v0.2.0'dadır), ADR-0114 (extern özeti; anotasyonlu çıkış kullanıcının beyanı), ADR-0119 (niyet ve çıkarım ilkesi: denetlenmeyen beyan bu ADR'nin kurallarıyla görünür olur)
> Issue: #231 (`@false_path` ve `@multicycle`: kanıt ya da kullanıcının beyanı), #292 (uyarılar ve kaçış raporu), #293 (gerekçe, proje politikası, yapısal kanıt)
> Tarih: 2026-11-16
> Etkilenen: domain-inference.md (K8, K9, K11), cli-contract.md (`volt check` raporu, Volt.toml), volt-hir, volt-diagnostics, volt-driver, book

## Sorun

Volt'un sözü: "0 hata" güvenilir. Derleyici kabul ettiği tasarımın saat
alanı geçişlerini, gizli veri akışını ve el sıkışma yollarını denetlemiştir.

Gerçek tasarımlarda derleyicinin kanıtlayamadığı ama doğru olan yapılar
vardır: kendi yazılmış bir senkronlayıcı, bilerek açılan şifreli veri,
zamanlama analizinden bilerek çıkarılan bir yol. Bunlar için kullanıcının
"bunu ben biliyorum" demesinin bir yolu olmalıdır. Bu yollara bu ADR'de
**kaçış** denir: kullanıcının, derleyicinin kanıtlayamadığı bir şeyi
beyan ettiği yer.

Bugün Volt'ta sekiz kaçış var. Biri dışında hiçbiri gerekçe istemiyor
ya da iz bırakmıyor; üçü, kullanıcı bir kaçış yazmadan, yalnız `extern`
bildirimiyle devreye giriyor:

| Kaçış | Atladığı denetim | Gerekçe | İz | Kanıt | Kaynak |
|---|---|---|---|---|---|
| `declassify(expr, "gerekçe")` | E3009 (güven seviyesi) | Zorunlu (E0016) | W3008, her kullanım | Yok (doğası gereği) | ADR-0052, K11 |
| Portları birden çok saat alanında olan `extern module` | İçindeki saat alanı geçişi (E3001) | Yok | Yok | Yok: gövde görünmez, port anotasyonları tek sözleşme | K8 |
| Saatsiz extern'in anotasyonlu çıkışı | Çıkışın bağımlılıkları (E3001) | Yok | Yok | Yok: "kullanıcının beyanı" | ADR-0114, K8 |
| Saatli extern'in içinden geçen yol | E4007 | Yok | Yok | Yok: özet boş sayılır | ADR-0114 sınırı |
| `@false_path(from, to)` | Zamanlama analizi (SDC/XDC'ye yazılır) | Yok | Yok | E6003 tanımlı, üretilmiyor | ADR-0054 |
| `@multicycle(...)` | Zamanlama analizi | Yok | Yok | E6004 tanımlı, üretilmiyor | ADR-0054 |
| `@no_protocol_check` | El sıkışma protokol kontratları (E4007 değil) | Yok | Yok | Yok | ADR-0050 |
| `@no_auto_contracts` | Otomatik FSM/sayaç kontratları | Yok | Yok | Yok | ADR-0066 |

Üç extern satırı kullanıcının bilinçli bir işareti değildir: bir
SystemVerilog modülünü `extern` olarak bildirmek, Volt'un o modülün içine
güvenmesi demektir, ama bu güven hiçbir yerde görünmez. Bir PR'ı
inceleyen kişi, tasarıma yeni bir denetimsiz geçiş eklendiğini göremez.

İki uç da kötüdür. Kaçışı kolay olan dilde denetimler zamanla susturulur
(ticari CDC araçlarındaki "waiver" deneyimi). Kaçışı olmayan dilde
kullanıcı gayriresmî bir yol bulur; Volt'ta bu yol `extern`'dir ve
`declassify`'dan daha görünmezdir.

## Karar

### 1. Kaçışın yedi kuralı

Her kaçış, bugünkü ve ileride eklenecek olanlar, şu kurallara uyar:

- **K1 — Dar.** Kaçış hangi denetimi ve hangi hedef için atladığını
  söyler. İçindeki her şeyi susturan blok biçimi (`unsafe { … }`) yoktur.
- **K2 — Bilinçli.** Kaçış yalnız kullanıcının yazdığı açık bir
  sözdizimiyle devreye girer; derleyici hiçbir şeyi kendiliğinden
  "beyan" saymaz. Tanının `help` satırı önce düzeltmeyi, sonra kaçışı
  gösterir.
- **K3 — Gerekçeli.** Kaçış bir gerekçe metni taşır. Gerekçesiz kaçış
  derleme hatasıdır (`declassify` için bugünkü E0016 gibi).
- **K4 — İz bırakan.** Güvenlik denetimini (saat alanı, güven seviyesi,
  el sıkışma) atlayan kaçış her kullanımda bir uyarı verir (W3008
  gibi). Extern'in açık kaçış işareti (§2) bu kaçışlardandır. Öteki
  kaçışlar (zamanlama, kontrat kapatma) uyarı vermez ama K5'teki
  raporda listelenir.
- **K5 — Toplu raporlanabilir.** `volt check` her koşuda kaçışların tek
  satırlık özetini basar (`3 escapes: 2 extern, 1 declassify`; Türkçe
  kipte `3 kaçış: 2 extern, 1 declassify`). `--escapes` bayrağı bütün
  kaçışları türü, hedefi, gerekçesi ve konumuyla listeler: insan için
  metin, `--format json` ile CI için JSON. Bir PR'ın eklediği ve
  kaldırdığı kaçışlar iki raporun farkıyla görülür.
- **K6 — Politikayla sınırlanabilir.** Volt.toml'un `[escapes]`
  bölümünde her kaçış türü için proje politikası yazılır: `"allow"`
  (raporda listelenir), `"warn"` (her kullanım uyarı), `"deny"` (her
  kullanım hata). Anahtarlar kaçış türünün adıdır. Değerler `[lint]
  unenforced_attributes`'teki gibi dizgedir (ADR-0048); `"deny"` bu
  ADR'yle eklenir. Tanınmayan bir değer yapılandırma hatasıdır (çıkış
  kodu 4, cli-contract.md §2); `[lint]`'teki gibi `"warn"`a düşmez,
  çünkü yanlış yazılmış bir `"deny"` sessizce gevşek bir politikaya
  dönmemelidir:

  ```toml
  [escapes]
  extern_domains = "warn"
  declassify = "deny"
  false_path = "allow"
  ```

  Varsayılan, K4'teki davranıştır.
- **K7 — Kanıtlanabilen kanıtlanır, kalanı sınanır.** Kaçış denetimi
  tümden kapatmaz, yalnız derleyicinin kanıtlayamadığı kısmı varsayım
  olarak kabul eder. Kanıtlanabilen kısım yine denetlenir (ör.
  `@false_path` için yolun yapısal olarak var olup olmadığı, E6003).
  Kalan varsayım, üretilen SystemVerilog'a bir doğrulama ifadesi
  (assertion) olarak yazılır; benzetim ve biçimsel doğrulama varsayım
  bozulduğunda yakalar. Bir kaçışta ikisi de yapılamıyorsa, nedeni o
  kaçışı getiren ADR'de yazılır (ör. `declassify`, Sınırlar).

### 2. Var olan kaçışlara uygulanışı: v0.2.0

ADR-0100'e göre son hane sürümü yeni tanı kodu ve yeni seçenek getirmez,
önceden kabul edilen girdiyi yalnız bir sessiz yanlışı düzeltmek için
reddeder. Bu ADR'nin her adımı bunlardan birini ister (yeni uyarı ve hata
kodları, `--escapes`, `[escapes]`, işaretsiz extern'in reddi); bu yüzden
uygulama tek adımdır ve v0.2.0'dadır. O zamana kadar bugünkü davranış
geçerlidir.

| Kaçış | v0.2.0'da |
|---|---|
| Portları birden çok saat alanında olan extern | Açık kaçış işareti zorunludur (K2), gerekçe taşır (K3). İşaret yoksa derleme hatası; tanı, işareti ekleyen otomatik uygulanabilir bir öneri taşır ve öneri ADR-0099'un gidiş-dönüş testinden geçer. İşaret varsa her kullanımda bir iz uyarısı (W3008 gibi: "Volt bu modülün içindeki saat alanı geçişini denetlemiyor") ve rapor. Derleyicinin kendi CDC primitifleri (`sync()`, `AsyncFifo`, …) kaçış değildir, işaret istemez. |
| Saatsiz extern'in anotasyonlu çıkışı | Aynı kural: açık kaçış işareti zorunlu ve gerekçeli; işaret yoksa otomatik düzeltme önerili hata; işaret varsa iz uyarısı ("bu çıkışın bağımlılıkları denetlenmiyor") ve rapor. |
| Saatli extern'in içinden geçen yol | Rapora girer (uyarı yok: her saatli extern'de çıkardı). |
| `declassify` | Rapora girer; davranış aynı. |
| `@false_path`, `@multicycle`, `@no_protocol_check`, `@no_auto_contracts` | Gerekçe ister (K3) ve rapora girer. |
| Kaçış raporu (K5) | Tek satırlık özet her `volt check`'te; ayrıntı `--escapes` ile, metin ve JSON. |
| Proje politikası (K6) | `[escapes]` bölümü. |
| Yapısal kanıt (K7) | E6003 ve E6004'ün üreticisi. Yanlış bir `@false_path` bugün sessizce yanlış zamanlama kısıtı üretir; v0.2.0'a kadar kitaptaki bilinen sınırlar sayfası bunu söyler. |

İşaretin ve gerekçenin sözdizimi uygulamada kesinleşir. Yeni kodların
numaraları uygulama PR'ında (#292, #293), ailelerinin sıradaki boş
numaralarından verilir: saat alanı, sıfırlama ve güven seviyesi tanıları
`E3xxx`/`W3xxx` ailesidir (uyarılarda W3001–W3011 kullanımda). Her durum
kendi kodunu alır, çünkü düzeltmeleri ve `volt explain` metinleri
farklıdır. Kullanılmış ya da geri çekilmiş bir numara yeniden verilmez.

Bugün derlenen ve işaretsiz bir çok alanlı ya da anotasyonlu çıkışlı
saatsiz extern içeren tasarım v0.2.0'da hata alır; sürüm notunun
"Behavior changes" bölümüne girer ve otomatik düzeltme tek adımda
uygulanır.

### 3. UX Anayasası ile ilişkisi

UX Anayasası (İlke 2 ve "Yasak Kalıplar") kontrat ya da anotasyon
eksikliği için uyarıyı yasaklar. §2'deki extern kuralı bu yasağa
girmez:

- İşaretsiz extern için verilen bir eksiklik uyarısı değil, bir
  sözdizimi kuralının hatasıdır: gerekçesiz `declassify`'ın E0016'sı
  gibi. Kural, Volt'un denetlemediği bir yerde kullanıcının bunu bildiğini
  yazmasını ister; tasarım hakkında eksik bir bilgiyi değil.
- Hata çözümü gösterir ve çözüm tek adımda otomatik uygulanır (İlke 2:
  "Hata mesajı çözümü gösterir").
- İşaretli kullanımın uyarısı bir eksikliği değil, kullanıcının yazdığı
  bir kaçışı bildirir (W3008 gibi).
- Volt'un kendisi denetlediği extern'ler (tek saatli, anotasyonsuz
  saatsiz) işaret istemez.

### 4. Yeni kaçış ekleme şartı

Yeni bir kaçış ancak şu üç koşulla eklenir:

1. Yedi kurala uyar ve rapora girer.
2. İhtiyaç gerçek bir tasarımdan gelir: kullanıcının yazamadığı, doğru
   olduğu gösterilebilen bir yapı.
3. Standart kitaplıkta güvenli bir yapı o ihtiyacı karşılayamıyordur.

Saat alanı geçişi için genel bir kaçış bu ADR ile **eklenmez**. Önce
yaygın ve doğru kalıplar standart kitaplığa güvenli yapılar olarak
girer: açılışta bir kez yazılıp sonra sabit kalan yapılandırma yazmaçları
(yarı statik sinyaller) ve senkronlanmış bir "veri hazır" sinyaliyle
birlikte taşınan çok bitli veri.

## Sınırlar

- `declassify`'ın varsayımı ("bu bilgi açılabilir") kanıtlanamaz ve
  sınanamaz; K7 ona uygulanmaz.
- Rapor, extern'in içinde ne olduğunu bilmez; yalnız Volt'un nereye
  güvendiğini söyler.
- `@allow(unenforced)` ve `[lint] unenforced_attributes` (ADR-0048) bu
  ADR'deki anlamda kaçış değildir. Volt'un ayrıştırdığı ama henüz
  uygulamadığı bir nitelik (ör. `@budget(lut = 5000)`) her kullanımda
  W0021 verir: "bu nitelikten hiçbir denetim üretilmiyor". `@allow`
  yalnız bu hatırlatmayı susturur. Derleyici hiçbir şeye kullanıcının
  sözüyle güvenmeye başlamaz; tasarım öncesinde nasıl denetleniyorsa
  yine öyle denetlenir.

## Reddedilenler

- **Kaçışsız dil.** Gerçek tasarımlarda doğru ama kanıtlanamayan yapılar
  vardır; resmi yol yoksa kullanıcı gayriresmî olanı (`extern`) kullanır
  ve kaçış görünmez olur.
- **Genel `unsafe` bloğu** (Veryl'in `unsafe (cdc)` biçimi). Bloğun
  içindeki her şey susar; K1'e aykırı.
- **İşaret yazılınca susan extern uyarısı.** İşaretsiz extern'e uyarı
  verip işaret yazılınca susmak, UX Anayasası'nın yasakladığı "anotasyon
  eksikliği için uyarı" biçimidir; uyarı da görmezden gelinebildiği için
  güven yine görünmez kalır. Yerine işaret zorunludur ve eksikliği
  otomatik düzeltmeli bir hatadır (§2, §3).
- **Görünürlüğü son hane sürümünde vermek.** Yeni uyarı kodları,
  `--escapes` ve JSON raporu ADR-0100'e göre orta hane ister; hepsi
  v0.2.0'dadır.
- **Her kaçışa her kullanımda uyarı.** Zamanlama ve kontrat kaçışları
  sık ve çoğunlukla zararsızdır; uyarı gürültüsü güvenlik uyarılarını
  boğar. Bunlar raporda listelenir.
- **Raporu yalnız bayrakla göstermek.** Kimse bayrağı yazmazsa kaçışlar
  yine görünmez olur. Tek satırlık özet her koşuda basılır; ayrıntı
  bayrakla gelir.
- **E6003'ü son hane sürümünde hata yapmak.** Bugün `@false_path`
  kullanan her tasarımı birden derlenmez yapabilir; v0.2.0'ı bekler
  (§2).
