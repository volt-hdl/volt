# Çip İçi Ağ (Network-on-Chip) — Tasarım Notu

> Statü: **TASARIM NOTU — BU TURDA UYGULANMADI.** Kod, gramer, spec ve ADR
> değişmedi. GÜNCELLEME (ADR-0056, 2026-09-17): §1'deki iki dil sınırı —
> modül seviyesi `for` içinde örnekleme ve bundle dizisi portu — kaldırıldı;
> dizi tipli port/wire SV eşlemesi eklendi; §3'teki "sentetik span bütçesi"
> riski `for` açılımı için geçersiz (klonlar kaynak span'ini korur, yalnız
> `ctx` değişir; 4×4 = 28 ctx). 4×4 mesh artık iç içe `for` ile yazılabilir
> (tahmin ~150–200 satır); `noc` desugar'ı (§2-3) hâlâ opsiyon. Bu belge opsiyon değeri taşır: talep gelirse hazır plan, gelmezse
> sıfır maliyet. §6'daki koşullardan biri oluşmadan UYGULANMAMALIDIR.
> Tarih: 2026-09-16
> Dayanak: ADR-0038 (pipeline desugar), ADR-0039 (bundle), ADR-0040 (`prev`),
> ADR-0041 (const generic mono), ADR-0050 (`Handshake<T>`), ADR-0055 (paralel
> verify), docs/stdlib.md, examples/soc/

---

## 1. Mevcut altyapıyla bugün elle NoC yazılabilir mi? (2×2 mesh, 4 router)

Kağıt üzerinde değerlendirme; kod yazılmadı, iddialar kaynak koda ve ADR'lere
dayanır.

**Flit ve port tipi.** `struct NocFlit { dest_x: u1, dest_y: u1, payload: bits<128> }`
sade bir struct'tır; `Handshake<NocFlit>` ADR-0050 §1 kuralıyla
`<port>_data_dest_x`, `<port>_data_dest_y`, `<port>_data_payload`, `<port>_valid`,
`<port>_ready` düz portlarına açılır. **Evet, router portları yazılabilir.**

**Router.** `module Router<const X: u32, const Y: u32>` (ADR-0041 mono) beş giriş
+ beş çıkış `Handshake<NocFlit>` (N/E/S/W/L). İç yapı bugünkü stdlib ile kurulur:
giriş başına `SyncFifo<NocFlit, 4>` (struct `T`'nin SyncFifo'da açıldığı
doğrulanmadı — Aşama 1 riski), XY kararı `comb` blokta `dest_x`/`X`
karşılaştırması, çıkış başına `RoundRobinArbiter<5>` (stdlib, kontratlı), çıkış
muxu `match`. E4007 (valid ← ready kombinasyonel yolu) FIFO'lu tasarımda
kendiliğinden sağlanır; tamponsuz "bypass" router yazılmak istenirse E4007
buna izin vermez — bu, kilitlenme açısından da istenen yöndür (§4).

**Dört router elle örneklenir mi?** Evet, ama iki dil sınırı maliyeti yükseltir:
- Modül seviyesi `for` yalnız atama açar; gövdede `let` (dolayısıyla
  örnekleme) `future` olarak reddedilir (volt-sv-emit/generate.rs). 4 router
  elle; 4×4 mesh (16 router) elle yazılamaz düzeyde uzundur.
- Bundle dizisi (`in ports : [Handshake<NocFlit>; 5]`) düzleşmez
  (parser/bundle.rs dizi tipini ele almaz; ADR-0039 "generic struct port"
  sınırıyla aynı sınıf). Her yön ayrı port olarak yazılır.
- Gövdeler yukarıdan aşağı çözümlendiğinden karşılıklı bağlı routerlar için
  bağlantı tellerinin yarısı `wire` ile öne bildirilmelidir (SocTop deseni,
  examples/soc/top.volt satır 47-72).

**Boilerplate tahmini (2×2, tek saat):**

| Parça | Satır | Not |
|---|---|---|
| `NocFlit` + Router gövdesi | ~120–150 | FIFO/arbiter/mux, XY kararı, tie-off |
| Router portları | 10 satır → 50 SV portu | Handshake ×10 × 5 düz sinyal |
| Mesh top: `wire` ön bildirimi | ~25 | 4 kenar × 2 yön × 5 sinyal, yarısı |
| Mesh top: 4 örnekleme | ~4 × 40 = 160 | her router 50 bağlama |
| Kenar tie-off | ~40 | 2×2'de her routerın 2 yönü boş: `valid=false`, `ready=true`; boş çıkışlar Verilator `-Wall` için tüketilmeli (top.volt satır 172 notu) |
| **Toplam** | **~350–400** | bunun ~230'u salt bağlama/tie-off |

Karşılaştırma: SocTop 175 satırla 4 periferiği tek veri yoluna bağlıyor; 2×2 mesh
iki katı, 4×4 mesh (~1.400 satır) pratikte yazılmaz. **Elle NoC bugün 2×2 için
mümkündür, 4×4 için değildir.** Bu, Aşama 2'nin (desugar) tek gerçek gerekçesidir.

**Bugün yazılabilen kontratlar:**
- Otomatik (ADR-0050 §3): port başına 1 tutma + 3 veri kararlılığı = 4;
  router başına 40, 2×2 mesh'te 160 kural — sıfır satır yazılarak.
- Arbiter kontratları stdlib'den gelir (bir-sıcak grant, `grant & req == grant`).
- Elle yönlendirme değişmezleri: `invariant: out_e.valid -> out_e.data.dest_x > X`
  (X const generic; kontratta sabit okuma serbest). Her yön için bir tane, 4 satır.
- Elle sınırlı canlılık: yardımcı `wait_r` sayacı + `invariant: wait_r <= K`
  (ayrıntı §4). "Sonunda ulaşır" doğrudan yazılamaz: kontrat türleri yalnız
  requires/ensures/invariant/cover/assert/assume'dur (grammar §5); `eventually`
  yoktur.
- `cover: out_e.fired && ...` giriş→çıkış çifti başına (router başına 20).

**Domain sistemi çok saatli NoC'de ne yapar?** Tek saatli mesh'te görünmez.
GALS (her düğüm kendi saatinde) kurulmak istenirse `Handshake` portu karşı
alandan bağlanamaz: E3001 (alan uyuşmazlığı) ve bundle alanları bölünürse E3013.
Yani derleyici "saat adasını geçen bağlantı `AsyncFifo` üzerinden gitmeli"
kuralını **yazılmadan** dayatır. Bedeli: `AsyncFifo` valid/ready konuşmaz
(`wr_en/full`, `rd_en/empty`); her ada sınırında ~20 satırlık iki adaptör
gerekir ve `!(full && empty)` garantisi zayıftır (ADR-0027). `HandshakeSync`
4 fazlı olduğundan flit akışı için çok yavaştır. Sonuç: domain sistemi CDC'yi
yakalar, GALS adaptörünü vermez — Aşama 1'de `HandshakeToFifo` benzeri bir
stdlib köprüsü değerlendirilmelidir.

## 2. Önerilen sözdizimi taslağı (bağlayıcı değil)

```volt
noc Mesh<4, 4> SystemBus {
    protocol   = Handshake<NocFlit>   // tek desteklenen protokol (V0)
    routing    = xy                   // V0: yalnız xy
    flit_width = 128                  // payload; başlık (dest_x, dest_y) otomatik
    buffering  = 4                    // giriş FIFO derinliği
}

module Top {
    in clk : clock
    let bus = SystemBus { clk }                  // üretilen modül; portlar n<x>_<y>_send/recv
    let c0  = Core { clk, recv: bus.n0_0_recv }  // Handshake<NocFlit> (in)
    bus.n0_0_send = c0.send                      // düz alanlar bağlanır (ADR-0039 kuralı)
    ...
}
```

Karar noktaları:
- **`@node(SystemBus, 0, 0)` niteliği V0'da yok.** Modülü ağa niteliğiyle
  bağlamak "gizli bağlantı" yaratır ve modülü tek ağ örneğine sabitler;
  SocTop'un açık örnekleme deseni korunur. `@node` sonradan şeker olabilir.
- **Paket = tek flit (V0).** Çok flit'li (wormhole) paket ertelenir; flit
  sırası sorunu V0'da yoktur (§4). Başlık (`dest_x/dest_y : bits<clog2(·)>`)
  desugar tarafından eklenir; kullanıcı yalnız payload'ı görür.
- `noc` bağlamsal anahtar kelime olur (ADR-0023 deseni; bugün rezerve değil).

**Topolojiler:**

| Topoloji | Yönlendirme | Kilitlenme | Kapsam önerisi |
|---|---|---|---|
| Mesh | XY | sarma yok → XY yeterli | **V0** |
| Crossbar | yok (tek atlama) | arbiter ile yok | **V0** (BusDecoder genellemesi, ucuz) |
| Ring | tek yön | çevrim var → VC ya da kabarcık gerekir | V1 |
| Torus | XY + sarma | sarma çevrim yaratır → dateline/VC gerekir | kapsam dışı |

Torus ve ring, sanal kanal (VC) gerektirir; VC hem Router'ı hem `Handshake`'i
(kanal başına ayrı ready) değiştirir. V0'da yok.

## 3. Desugar planı — pipeline deseni geçerli mi?

ADR-0038: `pipeline(5)` parser içinde sıradan `ModuleDecl`'e iner; isim
çözümleme, tip denetimi, CDC, zamanlama, SVA ve SV üretimi pipeline'ı görmez.
Aynı yol NoC için: `noc Mesh<W,H> Ad {...}` → parser'da `noc.rs`:
1. `NocFlit` struct'ı (başlık + payload) sentezlenir.
2. W×H `Router` örneği (stdlib `Router<T, DEPTH>`, koordinatlar `my_x/my_y`
   giriş portu olarak literal bağlanır — mono ile 16 ayrı `Router_x_y` modülü
   üretmekten kaçınılır) ve 2·(2WH−W−H) tek yönlü kanal `wire`'ı üretilir.
3. Kenar portları tie-off, düğüm portları modül arayüzüne çıkarılır.
4. Sonuç sıradan `ModuleDecl`; bundle düzleştirme (`flatten_bundles`) ardından
   zaten koşar.

**Varsayım ("tip sistemi / CDC / SV üretimi değişmez") tek saatli mesh için
geçerlidir.** Riskler:
- **Sentetik span bütçesi.** ADR-0039 bildirim span'lerinin öğe içindeki tek
  karakterlik konumlardan, pipeline'ın ise öğe başından sayıldığını not eder;
  4×4 mesh ~300 wire + ~800 bağlama üretir ve bir `noc` bildirimi bu kadar
  benzersiz konum taşımaz. Önce sıfır genişlikli/`Span.ctx` tabanlı sentetik
  span mekanizması gerekir. **En büyük teknik risk.**
- **Üretilen kodda tanı.** Kullanıcının yazmadığı bağlamalarda E1001/E3001
  çıkarsa mesaj `noc` bildirimine işaret etmelidir; ADR-0038 bunu E5011–E5016
  ön denetimiyle çözdü. NoC için eşdeğer ön denetim kümesi gerekir (boyut
  ≥ 2, düğüm koordinatı menzil içi, `flit_width` ≥ 1, `buffering` ≥ 1,
  tek saat portu).
- **Çok saat.** `noc` tek saat kabul eder; GALS istenirse desugar ada
  sınırına `AsyncFifo` + adaptör yerleştirmeli — bu CDC analizini
  değiştirmez ama desugar'ı ikiye katlar. V0 dışı.
- **Formal ölçek.** SocTop `bmc 12` ≈ 9 s, `cover 48` ≈ 6 dk (ADR-0055).
  4×4 mesh ×5 giriş ×4 flit ×~136 bit ≈ 43 k durum biti; tek modül BMC'si
  algoritmik sınırdır (ADR-0055 "tek dev modülde kazanım yok"). Kanıt
  router başına yapılmalı, üst seviyede yalnız cover kalmalı.
- `W1001` fix-it'i ve E4007 ikincil etiketleri sentetik ada işaret eder
  (ADR-0039/0050 bilinen sınırı) — NoC'de daha sık görünür.

## 4. Kilitlenme (deadlock)

**XY mesh'te neden kilitlenmez:** XY, kanal bağımlılık grafiğinde çevrim
oluşturmaz (dönüş modeli; sarma yok). Bu bir yönlendirme teoremi; derleyici
yönlendirme fonksiyonunu kendisi ürettiğinden **yapısal olarak** garanti eder,
ADR-0038'in "stall muhafızını unutmak dilde ifade edilemez" ilkesiyle aynı
güçtedir. Ancak teoremin üç ön koşulu derleyicinin görüş alanı dışındadır:
1. **Uç nokta boşaltması:** her `recv` portu sonunda `ready` vermeli. Düğüm
   isteğe yanıt beklerken gelen isteği kabul etmezse **protokol kilitlenmesi**
   XY'ye rağmen olur (OpenPiton bu yüzden üç ayrı fiziksel ağ kullanır;
   BaseJump "XY ağ içi kilitlenmeyi önler, uç nokta kilitlenmesini önlemez"
   der). Volt bunu kanıtlayamaz; `assume: recv.ready` ya da sınırlı hazırlık
   varsayımı olarak ortama bırakır.
2. Tek flit ya da atlama başına tampon (wormhole'da bağımlı çevrim yok) — V0
   tek flit olduğundan sağlanır.
3. Kombinasyonel `ready` çevrimi yok — E4007 örnekler arasını görmez
   (ADR-0050 sınırı); desugar routerları FIFO'lu ürettiğinden yapısal olarak
   sağlanır.

**Otomatik üretilebilecek kontratlar:**

| Kontrat | Tür | Kanıt |
|---|---|---|
| `out_e.valid -> dest_x > my_x` (dört yön, XY: önce x) | invariant, router | `prove 3` ucuz |
| FIFO doluluk `count <= DEPTH` | invariant | stdlib'den hazır |
| `wait_r <= K` (giriş valid && !ready sayacı, `fired`'da sıfırlanır) | invariant + `assume` (uç nokta hazırlığı) | yalnız `bmc ≥ K`; K = atlama × arbiter turu × DEPTH; 2×2/DEPTH 2 için onlar, 4×4 için yüzler → **BMC pratik değil** |
| Flit sırası (V0 tek flit) | — | sorun yok; wormhole gelirse `lock_r -> sel == lock_sel` router başına invariant |
| `cover: in_<a>.fired && out_<b>.fired` çifti | cover, router | 20/router, ucuz |
| Her düğüm çifti yolu | cover, top | W·H·(W·H−1): 2×2'de 12, 4×4'te 240 → dakikalar; **bayrakla isteğe bağlı** |

**Canlılık sınırı (dürüst ifade):** Volt'ta ADR-0040/0050 akışı sınırlıdır —
`prove` k-tümevarım (derinlik 3), `bmc` (12), `cover`; `eventually`/`until`
yoktur ve `prev(x, N)` literal derinlik ister. "Paket sonunda hedefe ulaşır"
yalnız (a) sayaç sınırı olarak ve (b) küçük konfigürasyonda BMC ile
gösterilebilir. Kilitlenme özgürlüğü bir **kanıt değil, yapı + varsayım**
argümanıdır; belge ve tanı metni bunu böyle söylemelidir, "deadlock-free
kanıtlandı" denmemelidir.

## 5. Kapsam ve maliyet — üç aşama

Kalibrasyon: ADR-0038 (09-10), ADR-0039 (09-13), ADR-0050 (09-14) her biri
1–3 günlük tek tur olarak indi. Tahminler bu tempoya göre "odaklı gün"dür.

| Aşama | İçerik | Süre | Bağımlılık | Risk |
|---|---|---|---|---|
| **1 — stdlib `Router<T, DEPTH>` + elle 2×2** | Handshake portlu ilk stdlib primitifi (mevcut primitifler düz port), `my_x/my_y` girişli XY router şablonu + kontratları; `examples/noc/mesh2x2.volt` + sim testi + `verify`; docs/stdlib.md; gerekirse `HandshakeToFifo` GALS köprüsü | 2–3 gün | SyncFifo/Router'da struct `T`; stdlib şablonunda bundle desteği | E4007 ile arbiter etkileşimi; formal süresi; 4×4 örneği elle yazılamaz (kabul) |
| **2 — `noc` sözdizimi + desugar** | Bağlamsal anahtar kelime, `parser/noc.rs`, ön denetim tanıları (yeni E kodu ailesi, 5 parça + `explain` iki dil), sentetik span mekanizması, grammar/ast-nodes/sv-mapping için **yeni ADR** (spec salt okunur), mesh + crossbar, pass/fail ui testleri, 4×4 örneği | 4–6 gün | Aşama 1; sentetik span altyapısı; (isteğe bağlı) `for` içinde örnekleme — varsa desugar kısalır | Span bütçesi; üretilen kodda tanı kalitesi; mono/SV şişmesi; formal ölçek |
| **3 — kilitlenme analizi + kontratlar** | Yönlendirme değişmezleri otomatik; `wait_r` sınırlı canlılık + uç nokta `assume`; yol cover'ları `--cover=paths` bayrağı; (wormhole gelirse) sıra kilidi invariant'ı; "yapı + varsayım" tanı metni | 3–5 gün | Aşama 2; ADR-0055 paralel verify | BMC yalnız 2×2'de anlamlı; "kanıtlandı" yanılsaması |

Toplam ≈ 9–14 odaklı gün. Aşama 1 tek başına da değerlidir ve Aşama 2 olmadan
durabilir (emsallerin tamamı kütüphane düzeyinde kalmıştır, §7).

## 6. Değerlendirme koşulları — NE ZAMAN?

**Aşağıdakilerden biri somut olarak oluşmadan bu plan UYGULANMAMALIDIR.**
Bekleme maliyeti sıfırdır: hiçbir mevcut özellik NoC'ye bağımlı değildir.

| Tetikleyici | Kanıt sayılan şey | Açılan aşama |
|---|---|---|
| Volt ile çok çekirdekli SoC tasarlayan kullanıcı | Bir kullanıcı `BusDecoder` tarzı tek veri yolunun darboğaz olduğu ≥ 4 düğümlü noktadan noktaya trafiği Volt'ta elle yazmaya başlar (issue/PR/örnek) | 1 |
| AI hızlandırıcı (sistolik dizi + NoC) talebi | Somut mimari ve düğüm sayısı içeren talep; not: sistolik dizi önce `for` içinde örnekleme ister — o özellik NoC'den bağımsız ön koşuldur | 1 → 2 |
| Akademik grup ilgisi | Ders/tez/makale Volt'un NoC formal kontratlarını kullanmak ister | 1 → 3 |
| Rakip HDL'de dil düzeyi NoC | Spade/Veryl/Clash benzeri bir HDL NoC'yi dil yapısı olarak sunar ve karşılaştırma noktası olur | 2 |
| `for` içinde örnekleme gelir | 4×4 elle yazılabilir hale gelir; Aşama 1 öne çekilebilir, Aşama 2 aciliyeti düşer | 1 |
| ADR-0053 (HW-SW köprüsü) çok master'lı kullanım | Birden çok `@mmio` master'ı ve arbitrasyon talebi — önce crossbar, mesh değil | 1 (crossbar) |

Tetikleyici yoksa: belge olduğu gibi durur; yılda bir bu tablo gözden geçirilir.

## 7. Emsaller

| Ekosistem | NoC desteği | Kilitlenme | Ders |
|---|---|---|---|
| Chisel / rocket-chip | Dilde yok. TileLink bir veri yolu protokolü (Diplomacy ile parametre müzakeresi). NoC ayrı üreteç: **Constellation** (ucb-bar) — wormhole, sanal ağlar, keyfi topoloji | Yönlendirme doğrulayıcı + yönlendirme tablosu derleyicisi; dateline/katmanlı yönlendirmede VC sayısı "en fazla dateline geçişi + 1" şartı | Üreteç zamanı (Scala) çalışır; kilitlenme denetimi yönlendirme ilişkisi üzerinde, RTL üzerinde değil |
| Bluespec (BSV) | Dilde yok; `replicateM` statik elaborasyon + kural tabanlı router (ör. RECONNECT) | Kütüphane/tasarımcı sorumluluğu | Kural zamanlayıcı arbitrasyonu üretir; Volt'un `Handshake` otomatik kontratı bunun tip/kontrat karşılığıdır |
| SystemVerilog (elle) | OpenPiton (3 fiziksel ağ, XY wormhole), BaseJump `bsg_mesh_router` (XY), ESP | Kağıt üzerinde argüman; OpenPiton protokol kilitlenmesini üç ağ + öncelikle çözer | Uç nokta kilitlenmesi ağ dışı sorundur — hiçbir emsal onu araçla çözmez |
| Spade / Clash / Veryl | Yok (Clash'te Haskell üreteci yazılabilir) | — | Hiçbir modern HDL NoC'yi dil yapısı yapmadı |

**Volt'un farkı ne olurdu?** (1) Her link ADR-0050 ile kontratlı gelir —
emsallerde protokol kontratı elle yazılır ya da yoktur. (2) Domain sistemi
GALS sınırını derleme hatası yapar — emsallerde CDC lint ayrı araçtır.
(3) Yönlendirme değişmezleri ve sınırlı canlılık `volt verify -j` ile aynı
akışta koşar. (4) `noc` dil yapısı yeni olurdu; ama emsallerin tamamı
kütüphane düzeyinde kaldığından bu aynı zamanda YAGNI riskidir — Aşama 1
(kütüphane) emsallerle hizalıdır, Aşama 2 ancak §6 kanıtıyla savunulur.

Kaynaklar: [Constellation README](https://github.com/ucb-bar/constellation/blob/master/README.md), [Constellation Routing](https://constellation.readthedocs.io/en/latest/Configuration/RoutingSpec.html),
[BSV Reference Guide](https://csg.csail.mit.edu/6.375/6_375_2019_www/resources/bsv-reference-guide.pdf), [OpenPiton mikro mimari](https://parallel.princeton.edu/openpiton/docs/micro_arch.pdf),
[BaseJump Manycore Network](https://arxiv.org/pdf/1808.00650).

## 8. Özet

Elle NoC bugün 2×2 için evet (~350–400 satır, ~230'u bağlama/tie-off), 4×4
için hayır. Desugar planı tek saatli mesh için geçerli; sentetik span bütçesi
ve üretilen kodda tanı kalitesi çözülmeden başlanmamalı. XY yapısal garanti;
uç nokta kilitlenmesi ve gerçek canlılık kanıt dışı ("yapı + varsayım").
**Bu turda uygulanmadı; §6 koşulu oluşana dek uygulanmaz.**
