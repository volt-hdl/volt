# ADR-0027: Stdlib Mimarisi — CDC Primitifleri Derleyicide Yerleşik

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-08
> Etkilenen: volt-syntax (InstanceDecl generic args), volt-hir (builtin
> çözümleme, W3005, E2025), volt-sv-emit (primitif SV/SVA üretimi),
> volt-driver (multiclock .sby), tests/ui/pass/27-29
> Uygulama aşaması: F5 başlangıcı

## Sorun

`sync()`/`sync3()` tek bitlik CDC geçişini çözüyor; çok bitli veride
W3003 uyarısı veriliyor ama derleyici bir alternatif SUNMUYOR. Standart
CDC primitifleri (AsyncFifo, HandshakeSync, PulseSync) bir stdlib
gerektirir. İki mimari seçenek var; DAHA AZ KARMAŞIK olan seçilmelidir.

## Seçenekler

**Seçenek A — `crates/volt-stdlib/` + `.volt` dosyaları**
Stdlib Volt dilinde yazılır, derleyici otomatik yükler,
`import volt::cdc::AsyncFifo;` ile kullanılır. Dil kendi kendini ifade
eder (doğru uzun vadeli hedef) ama BUGÜN imkânsıza yakın karmaşıklıkta:

- Generic'ler yalnız AYRIŞTIRILIYOR; monomorfizasyon yok (F3 işi).
  `AsyncFifo<T, const DEPTH: usize>` Volt'ta yazılamaz.
- `import`/paket çözümlemesi yok (F2 işi); çok dosyalı derleme yok.
- Kullanıcı modülü örneklemesinin SV üretimi bile F0'da `future()`
  (E0003) — stdlib modülleri üretilemezdi.
- Gray kod pointer'ları `sync()`'ten geçirmek W3003'ü tetiklerdi:
  stdlib kendi uyarısını bastırmak için ayrıca mekanizma isterdi.

**Seçenek B — Rust'ta yerleşik tanım (sync() gibi)**
Primitifler derleyicide tanınır: isim çözümleme `AsyncFifo`,
`HandshakeSync`, `PulseSync` adlarını yerleşik modül olarak bilir,
SV gövdeleri volt-sv-emit'te şablon olarak üretilir (tıpkı
`try_emit_sync_bridge` gibi). Stdlib Volt'ta yazılamıyor olması
dezavantaj; ama mevcut F0-F4 altyapısının TAMAMI yeniden kullanılıyor.

## Karar: Seçenek B — Rust'ta Yerleşik Primitifler

Gerekçe:

- **Bağımlılık zinciri.** Seçenek A üç eksik altyapıya (monomorfizasyon,
  import, instance SV üretimi) aynı anda ihtiyaç duyar; Seçenek B
  hiçbirine ihtiyaç duymaz. "Daha az karmaşık" ölçütünde açık ara önde.
- **Emsal: sync().** Tek meşru CDC köprüsü zaten yerleşik; primitifler
  aynı deseni izler. Verilog'un kendi primitifleri de dilde değil
  araçtadır.
- **Geri dönüşü olan karar.** Kullanıcıya görünen sözdizimi
  (`let f = AsyncFifo<u8, 16> { ... }`) grammar-full.ebnf §10'daki
  `InstanceDecl = "let" Ident "=" Path [GenericArgs] "{...}"` biçiminin
  ta kendisi. F3'te generics + import gelince stdlib Volt'a taşınabilir;
  kullanıcı kodu DEĞİŞMEZ. Bu ADR o taşımayı engellemez, sıralar.

### Kullanıcıya görünen yüzey

```volt
let fifo = AsyncFifo<u8, 16> {
    wr_clk: fast_clk, wr_data: data, wr_en: push,
    rd_clk: slow_clk, rd_en: pop,
}
full  = fifo.wr_full
value = fifo.rd_data
```

- `AsyncFifo<T, DEPTH>` — gray kod pointer'lı asenkron FIFO.
  DEPTH iki kuvveti olmalı; değilse **E2025** (mevcut "geçersiz
  genişlik/boyut" kodu bu kullanım için doğal genişleme — DEPTH bir
  boyut parametresidir).
- `HandshakeSync<T>` — req/ack ile tek transfer; veri kaynak alanda
  ack gelene kadar stabil tutulur.
- `PulseSync` — toggle + kenar sezimi ile tek darbe geçişi.

### Yeni tanı kodu: W3005 (bu ADR tanım kaynağıdır)

**W3005 — PulseSync minimum darbe aralığı.** Toggle protokolü, ardışık
kaynak darbeleri hedef alanda çözülemeyecek kadar sıksa darbe YUTAR.
Statik olarak saat oranı bilinmediğinden derleyici her PulseSync
örneklemesinde kullanım kısıtını hatırlatan W3005 uyarısı üretir:
kaynak darbeler arasında en az 3 hedef saat çevrimi bulunmalıdır.
(Spec salt okunur olduğundan yeni kod ADR ile tanımlanır — W2013 /
ADR-0025 emsali.)

### Formal kontratlar

Her primitif, `volt verify` akışında (SvaMode::Immediate) kendi
kontratlarını üretir ve SvaProp olarak kaydeder:

- AsyncFifo: `count <= DEPTH` invariant'ı (ikili pointer farkı),
  `wr_full` ve `rd_empty` cover'ları. Hedefteki
  `!(wr_full && rd_empty)` invariant'ı ZAYIFLATILDI: iki-flop
  senkronizasyon gecikmesi nedeniyle boş→dolu geçişte iki bayrak
  GEÇİCİ olarak aynı anda doğru olabilir (gerçek FIFO davranışı);
  bunun yerine senkronize görünümün gerçek doluluk aralığında kaldığı
  kanıtlanır.
- HandshakeSync: req yüksekken (ack tamamlanana dek) verinin stabil
  kaldığı invariant'ı; transfer tamamlanma cover'ı.
- PulseSync: toggle bütünlüğü invariant'ı (toggle yalnız pulse_in'e
  yanıt olarak değişir); darbe iletim cover'ı. İlk tasarımdaki
  "pulse_out iki ardışık dst çevrimi yüksek kalamaz" invariant'ı
  ZAYIFLATILDI: serbest saat oranları altında kaynak darbeleri dst
  periyodundan sık gelirse ihlal GERÇEKTİR (tam W3005'in uyardığı
  durum) ve hiçbir kaynak-alan aralık varsayımı dst saatinin keyfî
  yavaşlığını dışlayamaz; dst-alan varsayımı ise özelliğin kendisini
  varsaymak olurdu (döngüsel). Kullanım kısıtı W3005 uyarısıyla
  kullanıcıya bırakılır.

İki saatli tasarımlar için üretilen `.sby` dosyasına `multiclock on`
eklenir (Yosys clk2fflogic akışı); tek saatli tasarımların çıktısı
değişmez.

Immediate modda her primitif iki formal ortam kurulumu daha üretir
(yalnız `volt verify` çıktısında — normal build bunları hiç görmez):

- **Başlangıç durumu**: `initial begin ... end` bloğu tüm iç durum ve
  gözlemci register'larını reset değerlerinden başlatır. Gerekçe:
  clk2fflogic altında `initial assume (rst)` reset'i yalnız 0. zaman
  adımında sabitler; o aralıkta kenar örneklemeyen saat alanı hiç
  sıfırlanmaz ve pointer'lar keyfî değerle başlayıp sahte karşı örnek
  üretirdi.
- **İz ortası reset yok varsayımı**: her alanda
  `always @(kenar clk) assume (!reset)`. Gerekçe: `multiclock on`
  altında reset serbest bir girdidir ve tek alanın kenarında bir
  çevrim yüksek kalabilir — KISMİ reset (ör. rbin sıfırlanır, wbin
  kalır) tüm pointer kontratlarını gerçekten ihlal eder. Gerçek
  donanımda reset iki alan da görene dek tutulur; sync reset yalnız
  örneklenen kenarda etkili olduğundan kenar bazlı assume yeterlidir
  ve `initial assume` ile çelişmez.

## Sonuçlar

- **volt-syntax**: `let x = Yol<...> { ... }` örneklemesinde generic
  argümanlar ayrıştırılır (grammar §10 zaten tanımlıyordu; [N3]
  yeniden sınıflandırması `<` için sınırlı ileri bakışla genişletildi).
- **volt-hir**: `builtin.rs` primitif imzalarını tanımlar; isim
  çözümleme/tip denetimi/alan denetimi yerleşikleri tanır. E2025
  (DEPTH iki kuvveti değil) ve W3005 (PulseSync aralık kısıtı) burada
  üretilir.
- **volt-diagnostics**: W3005 kod tablosuna, messages/{en,tr} ve
  explain/{en,tr}'ye eklendi.
- **volt-sv-emit**: primitif başına inline SV üretimi (gray kod
  pointer'ları, iki-flop senkronizasyon, `logic [W-1:0] mem
  [DEPTH-1:0]` belleği) + Immediate modda kontrat üretimi.
- **W3003 yardım metni** üç alternatifi de sayar: AsyncFifo (veri
  akışları), HandshakeSync (tek transferler), gray kodlama (sayaçlar).
- **tests/ui/pass/27-29**: üç fixture; Verilator `--lint-only -Wall`
  temiz, `volt verify` Docker (hdlc/formal) ile kanıtlanıyor.

## Emsal

FIRRTL'de `mem` ve `AsyncQueue` benzeri yapılar derleyici/araç
tarafında çözülür; Verilog primitifleri dilde değil araçtadır. LLVM'de
`memcpy` gibi "yerleşik ama kütüphane görünümlü" intrinsic'ler aynı
kalıptır: önce intrinsic, olgunlaşınca kütüphaneye taşınır.
