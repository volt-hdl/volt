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
| [Handshake](#handshake-tek-saat-bundle) | tek (bundle) | `<T>` | E4007 |
| [AsyncFifo](#asyncfifo-cdc) | çift (CDC) | `<T, DEPTH>` | E2025 |
| [HandshakeSync](#handshakesync-cdc) | çift (CDC) | `<T>` | — |
| [PulseSync](#pulsesync-cdc) | çift (CDC) | — | W3005 |
| [AsyncDualPortRam](#asyncdualportram-cdc) | çift (CDC) | `<T, DEPTH>` | E2025, W3006 |

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
bölgeyi paylaşıyorsa önce arbitrasyon. İki port FARKLI saat
alanlarındaysa DualPortRam kullanılamaz (tek `clk` portu; öteki alandan
bağlanan her port E3001) — `AsyncDualPortRam` kullanın, karşılaştırma
[aşağıda](#dualportram-mı-asyncdualportram-mı).

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

## Handshake (tek saat, bundle)

```volt
in  rx : Handshake<T>     // tüketici: data/valid giriş, ready çıkış
out tx : Handshake<T>     // üretici:  data/valid çıkış, ready giriş
```

Tek saat alanında valid/ready el sıkışması (ADR-0050). Diğer
primitiflerden farklı olarak örneklenen bir modül DEĞİL, yerleşik bir
`struct port` bundle'ıdır (ADR-0039): port tipi olarak yazılır, parser
düz portlara açar.

| Alan | Yön (`out tx`) | Tip | Açıklama |
|---|---|---|---|
| `data` | out | `T` | Taşınan veri; `T` sade bir `struct` ise alanları `tx_data_<alan>` düz portlarına açılır (`tx.data.addr`) |
| `valid` | out | bool | Üretici verisi hazır; `ready` gelene dek yüksek kalır |
| `ready` | in | bool | Tüketici bu çevrimde alabilir |
| `fired` | (sanal) | bool | `valid && ready` — transfer bu çevrimde gerçekleşti |
| `stalled` | (sanal) | bool | `valid && !ready` — üretici bekliyor |

`in rx : Handshake<T>` her alanın yönünü tersler. Üretilen SV düzdür:
`tx_data`, `tx_valid`, `tx_ready` (struct payload: `tx_data_addr`, ...).
Kullanıcı aynı adla `struct port Handshake` tanımlarsa kullanıcı tanımı
kazanır.

**Kontratlar** (`volt verify`, OTOMATİK — her Handshake portu için):

| Kural | `out` (üretici) | `in` (tüketici) |
|---|---|---|
| `prev(valid) && !prev(ready) -> valid` | invariant | assume |
| `prev(valid) && !prev(ready) -> data == prev(data)` (düz veri alanı başına) | invariant | assume |

Tüketici tarafta `assume`: modül kendi girişini kanıtlayamaz, ortamdan
bekler (ADR-0040 ile aynı gerekçe). Dizi/tuple payload için veri kuralı
üretilmez. Kapatmak için `@no_protocol_check` (port ya da modül
düzeyi) — protokolü bilerek konuşmayan bir izleme çıkışı gibi.

**E4007:** üretici tarafta `valid`, `ready`'ye kombinasyonel bağımlı
olamaz (sürekli atama, `let`, `comb` bloğu üzerinden izlenir; `on`
bloğu yolu keser). Tüketici `ready`yi `valid`den türetebilir.

```volt
module Producer {
    in  clk : clock
    out tx  : Handshake<u8>
    reg valid_r : bool = false
    on clk {
        if tx.fired { valid_r <= false } else { valid_r <= true }
    }
    tx.data  = 42
    tx.valid = valid_r
}
module Consumer {
    in  clk : clock
    in  rx  : Handshake<u8>
    reg acc : u8 = 0
    on clk { if rx.fired { acc <= acc + rx.data } }
    rx.ready = true
}
```

**Ne zaman kullanılmalı:** aynı saat alanında iki modül arasında geri
basınçlı veri aktarımı; AXI4-Lite/AXI-Stream benzeri kanallar
(`examples/axi4lite_slave.volt`: beş kanal = beş `Handshake<Payload>`).

**Ne zaman kullanılmamalı:** iki taraf FARKLI saatteyse — `Handshake<T>`
senkronizasyon içermez, alan denetimi bağlamayı E3001 ile reddeder.
Alanlar arası tek transfer için `HandshakeSync<T>`, akış için
`AsyncFifo`.

## Handshake mı, HandshakeSync mı?

| | `Handshake<T>` | `HandshakeSync<T>` |
|---|---|---|
| Ne | Port tipi (bundle) | Örneklenen modül |
| Saat | tek alan | iki alan (`src_clk` → `dst_clk`) |
| Protokol | valid/ready, her çevrim transfer olabilir | 4 fazlı req/ack, transfer başına birkaç çevrim (iki yönde 2-flop) |
| Geri basınç | `ready` | `busy` |
| Kontrat | otomatik tutma + veri kararlılığı (invariant/assume) | req yüksekken veri kararlılığı, transfer cover'ı |
| Kullan | modüller arası kanal, bus arayüzü | seyrek, tek seferlik CDC transferi |

Karar kuralı: **aynı saatteyse `Handshake<T>`, değilse
`HandshakeSync<T>`** (akış için `AsyncFifo`). Farklı saatte `Handshake`
kullanmak mümkün değildir: derleyici karşı alandan bağlanan her alanı
reddeder (E3001/E3013).

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

**Kullanım:** seyrek, tek seferlik alanlar arası transferler. Aynı
saat alanı içinde valid/ready kanalı için `Handshake<T>` bundle'ı
(bkz. [Handshake mı, HandshakeSync mı?](#handshake-mı-handshakesync-mı)).

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

### AsyncDualPortRam (CDC)

```volt
AsyncDualPortRam<T, DEPTH>   // DEPTH: iki kuvveti, 2..=65536
```

Domain-aware çift saatli bellek (ADR-0049): yazma portu `wr_clk`
alanında, okuma portu `rd_clk` alanında yaşar. Senkronizatör, gray kod
ya da FIFO YOKTUR — her portun adresi ve verisi kendi alanında kalır,
**bellek dizisinin kendisi CDC sınırıdır**. Bu tam olarak sentez
araçlarının gerçek çift saatli (true dual-port) block RAM'e eşlediği
kalıptır; üretilen SV'de yazma portu resetsiz saf `always_ff`, okuma
portu bir çevrim gecikmeli register'dır (Yosys `synth_xilinx`: tek
RAMB18E1, hiç FDRE yok).

| Port | Yön | Tip | Alan | Açıklama |
|---|---|---|---|---|
| `wr_clk` | in | clock | @Src | Yazma saati |
| `wr_addr` | in | `bits<clog2(DEPTH)>` | @Src | Yazma adresi |
| `wr_data` | in | `T` | @Src | Yazılacak veri |
| `wr_en` | in | bool | @Src | Yazma etkin |
| `rd_clk` | in | clock | @Dst | Okuma saati |
| `rd_addr` | in | `bits<clog2(DEPTH)>` | @Dst | Okuma adresi (her çevrim okunur, `rd_en` yok) |
| `rd_data` | out | `T` | @Dst | Senkron okuma (bir `rd_clk` gecikmeli) |

`@Src`/`@Dst` ADR-0047'nin sembolik alanlarıdır: her örneklemede
`wr_clk`/`rd_clk` bağlantıları onları gerçek alanlara bağlar, diğer
portlar o haritaya göre denetlenir. Yanlış alandan bağlanan port
**E3001**, `rd_data` okuması `rd_clk`'nin alanını taşır.

**Kontratlar** (`volt verify`, her adres KENDİ saatinde örneklenir):

- `invariant: wr_addr < DEPTH` (`wr_clk`'ta) ve `invariant: rd_addr <
  DEPTH` (`rd_clk`'ta) — DEPTH iki kuvveti olduğundan yapısal garanti.
- `cover: wr_en` — okuma portu her çevrim okuduğundan bu "eş zamanlı
  okuma ve yazma"nın erişilebilirliğidir.
- Alanlar arası kontrat YAZILMAZ (kullanıcı kodunda da E3001 olurdu).

**Eş zamanlı erişim — W3006 (çift saatli biçim):** yazma ile AYNI
adresin diğer saatten okunması çakışırsa okunan değer TANIMSIZDIR —
eski değer, yeni değer ya da (gerçek donanımda) metastabil bir örnek
olabilir. Bunu hiçbir senkronizatör sıralayamaz; iki saat arasında
"aynı çevrim" tanımlı değildir. Derleyici adres çakışmasını statik
dışlayamadığından her örneklemede W3006 üretir. Sözleşme kullanıcıda:
okuyucuyu yazılmakta olan adreslerden uzak tutun (ping-pong bölgeler,
okumadan önce `HandshakeSync`/`PulseSync` ile "hazır" sinyali) ya da
tek bayat örneğe tahammül edin (kare tamponu: piksel bir kare geç
görünür).

**Ne zaman kullanılmalı:** bir alanda yazılıp diğerinde RASTGELE
ERİŞİMLE okunan veri — kare tamponları, arama tabloları, örnek
tamponları, DMA hedef bellekleri.

**Ne zaman kullanılmamalı:** veri SIRALI akıyorsa `AsyncFifo` (geri
basınç ve tam/boş bayraklarıyla); tek sözcük seyrek geçiyorsa
`HandshakeSync`; iki port aynı saatteyse `DualPortRam` (iki tarafta
da okuma+yazma, aynı-çevrim davranışı tanımlı).

```volt
let fb = AsyncDualPortRam<bool, 8192> {
    wr_clk: sys_clk, wr_addr: wr_addr, wr_data: wr_bit, wr_en: wr_en,
    rd_clk: pix_clk, rd_addr: rd_addr,
}
pixel = fb.rd_data
```

## DualPortRam mı, AsyncDualPortRam mı?

| | `DualPortRam<T, DEPTH>` | `AsyncDualPortRam<T, DEPTH>` |
|---|---|---|
| Saat | tek `clk` | `wr_clk` + `rd_clk` |
| Portlar | A ve B: her ikisi okuma+yazma | yazma portu + okuma portu |
| Aynı adrese aynı çevrimde | tanımlı: B kazanır (W3006) | tanımsız okuma (W3006) |
| CDC | yok — öteki alandan bağlanan port E3001 | bellek dizisi sınırdır; adresler alan denetiminden geçer |
| Sentez | 1 BRAM (TDP, tek saat) | 1 BRAM (TDP, iki saat) |
| Kullan | CPU + DMA, ping-pong, aynı saatte iki erişimci | kare tamponu, LUT, bir alanda yaz / ötekinde oku |

Karar kuralı: **portlar aynı saatteyse DualPortRam, değilse
AsyncDualPortRam.** Aynı saatte AsyncDualPortRam kullanmak hata
değildir ama okuma portunun yazma yeteneğini ve tanımlı çakışma
davranışını kaybedersiniz. Farklı saatte DualPortRam kullanmak mümkün
değildir: derleyici her yanlış alanlı bağlamayı reddeder.

Çok bitli CDC karar tablosu (W3003'ün önerdiği dört yol):

| Veri biçimi | Primitif |
|---|---|
| Sıralı akış (stream) | `AsyncFifo<T, N>` |
| Tek transfer | `HandshakeSync<T>` |
| (aynı saat — CDC değil) | `Handshake<T>` bundle'ı |
| Sayaç | gray kodlama (`AsyncFifo` içinde hazır) |
| Rastgele erişim | `AsyncDualPortRam<T, N>` |
