# Volt Stdlib — Yerleşik Primitifler

Volt stdlib'i derleyicide yerleşiktir (ADR-0027 Seçenek B, ADR-0029):
primitifler isim çözümlemede tanınır, SV gövdeleri volt-sv-emit'te
şablon olarak üretilir ve `volt verify` akışında her primitif kendi
formal kontratlarını kanıtlar. Kullanım yüzeyi her yerde aynıdır:

```volt
let ornek = PrimitifAdi<T, BOYUT> {
    port: sinyal,
    ...
}
cikis = ornek.cikis_portu
```

- Girişler örnekleme bloğunda bağlanır; çıkışlar alan erişimiyle okunur.
- Saat portları modülün basit bir saat portuna bağlanmalıdır.
- Kullanıcı aynı adla modül tanımlarsa kullanıcı modülü kazanır.

| Primitif | Saat | Generic | Tanı |
|---|---|---|---|
| [SyncFifo](#syncfifo) | tek | `<T, DEPTH>` | E2025 |
| [Ram](#ram) | tek | `<T, DEPTH>` | E2025 |
| [DualPortRam](#dualportram) | tek | `<T, DEPTH>` | E2025, W3006 |
| [Counter](#counter) | tek | `<WIDTH>` | E2025 |
| [ShiftRegister](#shiftregister) | tek | `<T, LEN>` | E2025 |
| [RoundRobinArbiter](#roundrobinarbiter) | tek | `<N>` | E2025 |
| [PriorityArbiter](#priorityarbiter) | tek | `<N>` | E2025 |
| [EdgeDetect](#edgedetect) | tek | — | — |
| [AsyncFifo](#asyncfifo-cdc) | çift (CDC) | `<T, DEPTH>` | E2025 |
| [HandshakeSync](#handshakesync-cdc) | çift (CDC) | `<T>` | — |
| [PulseSync](#pulsesync-cdc) | çift (CDC) | — | W3005 |

Sabit generic kuralları: `DEPTH` 2..=65536 arası bir iki kuvveti
(E2025); `WIDTH` 1..=64; `LEN` ve `N` 2..=64. Sabit argüman derleme
zamanı tam sayı literali olmalıdır (E2008).

---

## SyncFifo

```volt
SyncFifo<T, DEPTH>   // DEPTH: iki kuvveti, 2..=65536
```

Tek saat alanında, doluluk sayaçlı FIFO.

| Port | Yön | Tip | Açıklama |
|---|---|---|---|
| `clk` | in | clock | Tek saat |
| `wr_data` | in | `T` | Yazılacak veri |
| `wr_en` | in | bool | Yazma isteği (doluysa yok sayılır) |
| `full` | out | bool | `count == DEPTH` |
| `rd_data` | out | `T` | Senkron okuma (bir çevrim gecikmeli) |
| `rd_en` | in | bool | Okuma isteği (boşsa yok sayılır) |
| `empty` | out | bool | `count == 0` |

**Kontratlar** (`volt verify`):

- `invariant: count <= DEPTH` — doluluk sınırı.
- `invariant: !(full && empty)` — tek saatte İKİ bayrak birlikte doğru
  olamaz. AsyncFifo'da bu kontrat iki-flop senkronizasyon gecikmesi
  yüzünden zayıflatılmıştır; SyncFifo'da tam haliyle KANITLANIR.
- `cover: full`, `cover: empty`.

**Ne zaman kullanılmalı:** aynı saat alanında üretici/tüketici
ayrıştırması, ani yük (burst) yumuşatma, boru hattı geri basıncı.

**Ne zaman kullanılmamalı:** yazma ve okuma FARKLI saat alanlarındaysa
`AsyncFifo` kullanın. Tersine, tek saat alanındaysanız AsyncFifo yerine
SyncFifo kullanın — gray kod pointer ve senkronizatör maliyeti
gereksizdir ve `!(full && empty)` garantisini kaybedersiniz.

```volt
let fifo = SyncFifo<u8, 16> { clk: clk, wr_data: din, wr_en: push, rd_en: pop }
dout  = fifo.rd_data
full  = fifo.full
empty = fifo.empty
```

## Ram

```volt
Ram<T, DEPTH>   // DEPTH: iki kuvveti, 2..=65536
```

Tek portlu senkron RAM, okuma-önce (read-first): aynı çevrimde yazılan
adres okunursa ESKİ değer döner. Bellek içeriği resetlenmez.

| Port | Yön | Tip | Açıklama |
|---|---|---|---|
| `clk` | in | clock | Tek saat |
| `addr` | in | `bits<clog2(DEPTH)>` | Okuma/yazma adresi |
| `wr_data` | in | `T` | Yazılacak veri |
| `wr_en` | in | bool | Yazma etkin |
| `rd_data` | out | `T` | Senkron okuma (bir çevrim gecikmeli) |

**Kontratlar:** `invariant: addr < DEPTH` — DEPTH iki kuvveti
olduğundan clog2(DEPTH) bitlik adres yapısal olarak aralıktadır;
kontrat bu garantiyi kanıt olarak belgeler.

**Ne zaman kullanılmalı:** tablo/tampon depolama, tek erişimcili
scratchpad. FPGA sentezinde block RAM'e eşlenir.

**Ne zaman kullanılmamalı:** aynı çevrimde iki bağımsız erişim
gerekiyorsa `DualPortRam`; birkaç kelimelik durum için `reg` yeterlidir.

```volt
let spad = Ram<u16, 256> { clk: clk, addr: addr, wr_data: wdata, wr_en: we }
rdata = spad.rd_data
```

## DualPortRam

```volt
DualPortRam<T, DEPTH>   // DEPTH: iki kuvveti, 2..=65536
```

Aynı saatte iki bağımsız okuma/yazma portu. Her iki port da
okuma-önce davranır. **W3006:** iki port aynı çevrimde AYNI adrese
yazarsa B portu sessizce kazanır; derleyici adres çakışmasını statik
dışlayamadığından her örneklemede uyarır. Portların ayrık adres
bölgelerine yazdığını yapısal olarak garanti edin ya da yazıcıları
tek portlu `Ram` önünde arbitre edin.

| Port | Yön | Tip |
|---|---|---|
| `clk` | in | clock |
| `a_addr`, `b_addr` | in | `bits<clog2(DEPTH)>` |
| `a_wr_data`, `b_wr_data` | in | `T` |
| `a_wr_en`, `b_wr_en` | in | bool |
| `a_rd_data`, `b_rd_data` | out | `T` |

**Kontratlar:** `invariant: a_addr < DEPTH`, `invariant: b_addr < DEPTH`.

**Ne zaman kullanılmalı:** aynı çevrimde iki erişimci (ör. CPU + DMA
okuması, ping-pong tamponu).

**Ne zaman kullanılmamalı:** tek erişimci varsa `Ram`; yazıcılar aynı
bölgeyi paylaşıyorsa önce arbitrasyon.

## Counter

```volt
Counter<WIDTH>   // WIDTH: 1..=64
```

Enable/clear'lı sarmalı (wrap-around) sayaç; `clear` `enable`'dan
önceliklidir. `overflow`, sayaç tüm birlerden sıfıra sardığı çevrimde
BİR çevrim yüksek kalan darbedir.

| Port | Yön | Tip |
|---|---|---|
| `clk` | in | clock |
| `enable` | in | bool |
| `clear` | in | bool |
| `count` | out | `bits<WIDTH>` |
| `overflow` | out | bool |

**Kontratlar:** `invariant: count < (1 << WIDTH)`; `cover: overflow`.

**Ne zaman kullanılmalı:** olay sayma, zaman bölme, adres üretme.

**Ne zaman kullanılmamalı:** keyfî bir üst sınırda sarmak (örneğin
mod-10) gerekiyorsa sayaç deseni elle yazılır (bkz.
`tests/ui/pass/23_provable_invariant.volt`).

```volt
let cnt = Counter<8> { clk: clk, enable: en, clear: clr }
ticks = cnt.count
wrap  = cnt.overflow
```

## ShiftRegister

```volt
ShiftRegister<T, LEN>   // LEN: 2..=64
```

Seri-paralel dönüşüm: her `shift_en` çevriminde `data_in` en genç
aşamaya girer, `data_out` en eski aşamayı verir, `taps` tüm LEN
aşamayı düzleştirilmiş `bits<LEN * width(T)>` olarak sunar
(en eski aşama en üst dilimde).

| Port | Yön | Tip |
|---|---|---|
| `clk` | in | clock |
| `data_in` | in | `T` |
| `shift_en` | in | bool |
| `data_out` | out | `T` |
| `taps` | out | `bits<LEN * width(T)>` |

**Kontratlar:** `cover: shift_en` (kaydırma erişilebilir).

**Ne zaman kullanılmalı:** seri hat alıcıları (UART/SPI benzeri),
sabit gecikme hatları, örüntü sezimi için pencere.

**Ne zaman kullanılmamalı:** rastgele erişimli tampon gerekiyorsa
`Ram`; tek çevrimlik gecikme için tek `reg` yeterlidir.

```volt
let sr = ShiftRegister<bool, 8> { clk: clk, data_in: bit_in, shift_en: strobe }
oldest = sr.data_out
window = sr.taps
```

## RoundRobinArbiter

```volt
RoundRobinArbiter<N>   // N: 2..=64
```

Dönen öncelikli arbiter. Grant aynı çevrimde (kombinasyonel) üretilir;
kayıtlı maske son grant edilen bitin ÜSTÜNÜ öncelikli tutar, en üst
bit grant edilince sarar. Düşük indeks önce gelir.

| Port | Yön | Tip |
|---|---|---|
| `clk` | in | clock |
| `req` | in | `bits<N>` |
| `grant` | out | `bits<N>` |

**Kontratlar:**

- `invariant: popcount(grant) <= 1` — `(grant & (grant-1)) == 0`
  biçiminde kanıtlanır (bir-sıcak ya da sıfır).
- `invariant: grant & req == grant` — grant yalnız isteyene verilir.
- `cover: grant[k]` her k için — her istekçi grant alabilir (sınırlı
  erişilebilirlik).

**Ne zaman kullanılmalı:** eşit öncelikli çok istekçili paylaşım
(bellek portu, veri yolu) — açlık (starvation) istenmediğinde.

**Ne zaman kullanılmamalı:** istekçiler doğal öncelik taşıyorsa
`PriorityArbiter`.

```volt
let rr = RoundRobinArbiter<4> { clk: clk, req: req }
grant_rr = rr.grant
```

## PriorityArbiter

```volt
PriorityArbiter<N>   // N: 2..=64
```

Sabit öncelikli, tamamen kombinasyonel arbiter: `req[0]` en yüksek
öncelik. Durum tutmaz; `clk` yalnız kontrat örneklemesi içindir.
Kontratlar RoundRobinArbiter ile aynıdır.

**Ne zaman kullanılmalı:** doğal öncelik sırası olan istekler
(ör. hata işleyici > normal trafik).

**Ne zaman kullanılmamalı:** düşük öncelikli istekçinin açlığı kabul
edilemezse `RoundRobinArbiter`.

## EdgeDetect

```volt
EdgeDetect   // generic argüman yok
```

Tek saat alanında kenar algılama: bir çevrimlik geçmiş register'ından
`rising`/`falling`/`both` türetilir.

| Port | Yön | Tip |
|---|---|---|
| `clk` | in | clock |
| `signal` | in | bool |
| `rising` | out | bool |
| `falling` | out | bool |
| `both` | out | bool |

**Kontratlar:** `invariant: both == (rising | falling)`;
`cover: rising`, `cover: falling`.

**Ne zaman kullanılmalı:** aynı saat alanındaki bir seviyeden darbe
türetme (buton, durum değişimi).

**Ne zaman kullanılmamalı:** `signal` BAŞKA bir saat alanından
geliyorsa EdgeDetect senkronizasyon İÇERMEZ — darbeyi alanlar arası
taşımak için `PulseSync` kullanın; çok bitli veri için
`AsyncFifo`/`HandshakeSync`.

```volt
let ed = EdgeDetect { clk: clk, signal: btn }
pressed = ed.rising
```

---

## CDC primitifleri (ADR-0027)

### AsyncFifo (CDC)

```volt
AsyncFifo<T, DEPTH>   // DEPTH: iki kuvveti, 2..=65536
```

Gray kod pointer'lı çift saatli FIFO. Portlar: `wr_clk`, `wr_data`,
`wr_en`, `wr_full` (yazma/kaynak alanı); `rd_clk`, `rd_data`, `rd_en`,
`rd_empty` (okuma/hedef alanı). Kontratlar: doluluk sınırı invariant'ı,
`wr_full`/`rd_empty` cover'ları. `!(full && empty)` iki-flop
senkronizasyon gecikmesi nedeniyle ZAYIFLATILMIŞTIR (ADR-0027) — tek
saat alanındaysanız bu garantiyi veren `SyncFifo`'yu kullanın.

**Kullanım:** farklı saat alanları arasında veri AKIŞI.

### HandshakeSync (CDC)

```volt
HandshakeSync<T>
```

4-fazlı req/ack ile tek transfer; veri kaynak alanda ack gelene dek
stabil tutulur. Portlar: `src_clk`, `data_in`, `send`, `busy`;
`dst_clk`, `data_out`, `valid`. Kontrat: req yüksekken veri stabilite
invariant'ı; transfer cover'ı.

**Kullanım:** seyrek, tek seferlik alanlar arası transferler.

### PulseSync (CDC)

```volt
PulseSync
```

Toggle + kenar sezimi ile tek darbe geçişi. Portlar: `src_clk`,
`pulse_in`; `dst_clk`, `pulse_out`. **W3005:** ardışık kaynak
darbeleri arasında en az 3 hedef saat çevrimi gerekir; daha sık
darbeler YUTULUR. Kontrat: toggle bütünlüğü invariant'ı (ADR-0027'de
zayıflatma gerekçesi).

**Kullanım:** alanlar arası tek bitlik darbe taşıma. Aynı alan içinde
kenar/darbe türetmek için `EdgeDetect` yeterlidir ve 2-3 çevrimlik
senkronizatör gecikmesi taşımaz.
