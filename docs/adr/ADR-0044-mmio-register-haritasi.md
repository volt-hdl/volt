# ADR-0044: `@mmio` Register Haritası — Bellek Eşlemeli Register Blokları

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-13
> Etkilenen: grammar-full.ebnf §2/§4/§18, sv-mapping.md §17,
> volt-ast (ModuleDecl.mmio_regs, MmioRegDecl, MmioFieldDecl),
> volt-syntax (parser/item.rs `@reg` bildirimi, parser/mmio.rs desugar,
> ParseResult.generated), volt-driver (unit.rs sentetik kaynak kaydı),
> volt-lsp (analysis.rs), volt-diagnostics (E0015, E4006),
> examples/soc/gpio.volt

## Sorun

`examples/soc/` keşfi (rapor 7g): her çevre biriminin register haritası
elle yazılıyordu. `gpio.volt` üç register için 40+ satır kalıp kod
taşıyordu — AXI köprüsünün 19 bağlama satırı, `match` ile adres
çözümleme, okuma çoklayıcısı, yazma maskesi, `rdata` register'ı ve
"bilinmeyen adres 0 okur" kuralı elle kodlanmıştı. Erişim izni (ReadOnly
register'a yazma) denetimi yoktu; SLVERR yalnız çözücüde üretiliyordu.
Timer ve UART aynı kalıbı tekrarlıyordu. Register haritası donanım ile
yazılımın tek ortak sözleşmesidir; v3 mimarisi (`docs/design/
Volt-Butunlesik-Mimari-v3.md §4.2`) bunun için `@mmio` bildirimini
tasarlamış ama uygulamamıştı.

## Karar

### 1. Yüzey sözdizimi

```volt
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module GpioRegs {
    in  clk        : clock
    in  pin_values : bits<8>
    out pins_out   : bits<8>

    @reg(offset = 0x00, access = ReadWrite)
    direction : {
        pins : bits<8>,
        @reserved : bits<24>,
    }

    @reg(offset = 0x08, access = ReadOnly, volatile)
    input : {
        pins : bits<8>,
        @reserved : bits<24>,
    }

    @reg(offset = 0x0C, access = ReadWrite)
    control : {
        enable : bool,
        reset  : bool @self_clearing,
        @reserved : bits<30>,
    }

    on clk { regs.input.pins <= pin_values }     // donanım tarafı yazar
    pins_out = regs.direction.pins & regs.input.pins
}
```

- `@mmio(base, bus)` bir MODÜL niteliğidir. `base` 32 bitlik tam sayı
  literali (varsayılan 0), `bus` yalnız `AXI4Lite` (varsayılan). Başka
  bus, başka argüman → E0009.
- `@reg(offset, access[, volatile]) ad : { alanlar }` modül gövdesinde
  bir register bildirimidir (gramer §4 `MmioRegDecl`). `offset` zorunlu
  literal, 4 bayta hizalı; `access` `ReadWrite` (varsayılan) |
  `ReadOnly` | `WriteOnly`; `volatile` bayrağı donanımın güncellediği
  register'ı işaretler.
- Alanlar en düşük bitten başlayarak sıralı yerleşir: `alan : Tip
  [@nitelik]`; tip `bool`, `bits<N>` ya da `uN` (N ≤ 32). `@reserved :
  bits<N>` adsız dolgu alanıdır; toplam ≤ 32 bit (eksik üst bitler
  örtük rezerve). Alan nitelikleri: `@self_clearing` (bus 1 yazınca bir
  döngü 1 kalır, sonra 0), `@w1c` (donanım kurar, yazılım 1 yazarak
  temizler).
- `reset` anahtar kelimesi alan adı olarak serbesttir (`regs.control.reset`
  erişimi için `.` sonrasında `reset` kabul edilir; `on clk.reset`
  tetikleyicisi ayrı yolda ayrışır).
- Kullanıcı tarafı erişim: `regs.<register>.<alan>` — `regs` her `@mmio`
  modülünün örtük register tutamacıdır; sağ tarafta, `on`/`comb`
  bloklarında ve kontratlarda kullanılır.

### 2. Sahiplik modeli

| Register | Sahip | Saklama | RTL yazar? | Bus yazar? |
|---|---|---|---|---|
| ReadWrite / WriteOnly, volatile değil | bus | `reg mmio_<reg> : u32` (tam sözcük) | HAYIR → E4006 | evet (byte strobe) |
| ReadOnly, volatile | donanım | alan başına `reg mmio_<reg>_<alan> : T` | evet (`<=`) | hayır; `@w1c` alanı 1 yazılınca temizlenir |
| ReadOnly, volatile değil | sabit | alan başına `let` (0) | HAYIR → E4006 | hayır (yok sayılır, OKAY) |

`volatile` yalnız `ReadOnly` ile geçerlidir (E0009): bus ve donanım
aynı saklayıcıyı yazsaydı çift sürücü olurdu. `@w1c` yalnız volatile
register'ın `bool` alanında, `@self_clearing` yalnız bus'a ait
register'ın `bool` alanında geçerlidir (E0009).

Neden tam sözcük saklama: dar alanlar ayrı saklansaydı `w_data`'nın
rezerve bitleri ve kullanılmayan `w_strb` şeritleri hiç okunmazdı;
Verilator `-Wall` bunu UNUSEDSIGNAL ile bildirir ve Volt'un lint
susturma pragması yoktur (ADR-0039 §Sınırlar). Bus'a ait register
rezerve bitleriyle birlikte saklanır, okuma görünümü `mmio_<reg> &
MASKE` ile rezerve bitleri 0 okur; sentez rezerve flopları düşürür.

### 3. Uygulama: parser'da desugar, ÜRETİLEN VOLT METNİ

ADR-0038/0039 silme ilkesi izlenir: `parse_source_file` /
`parse_unit` sonunda, bundle düzleştirmesinden ÖNCE, `desugar_mmio`
(parser/mmio.rs) her `@mmio` modülünü açar:

1. `@reg` bildirimleri doğrulanır (E0009/E0015), `RegInfo` listesine
   çevrilir; çakışan adres E0015.
2. Bus adaptörü, saklayıcılar, adres çözümleme, okuma çoklayıcısı,
   yazma mantığı ve otomatik kontratlar VOLT KAYNAK METNİ olarak
   üretilir (`module __VoltMmio { ... }`), kendi sentetik `FileId`'siyle
   (`<mmio:GpioRegs>`) ayrıştırılır ve gövdesi kullanıcı modülüne
   aktarılır: bildirimler (reg/let) kullanıcı gövdesinin ÖNÜNE (modül
   gövdeleri yukarıdan aşağı çözümlenir), `on` bloğu ve çıkış atamaları
   ARKASINA. Portlar `in aw : AxiWriteAddr, in w : AxiWriteData, in b :
   AxiWriteResp, in ar : AxiReadAddr, in r : AxiReadData` bundle'larıdır
   (ADR-0039); birimde tanımlı değillerse `examples/axi4lite_slave.volt`
   ile aynı beş `pub struct port` de üretilir. Bundle düzleştirmesi
   sonra koşar: SV portları `aw_addr`, `aw_valid`, ... düz adlarıyla
   çıkar, `examples/soc/top.volt` bağlamaları değişmez.
3. Kullanıcı gövdesi ve kontratlarındaki `regs.<reg>.<alan>` zincirleri
   yeniden yazılır: donanım/sabit alan → düz ad (`mmio_status_count`),
   bus alanı → dilim görünümü (`mmio_control[0]`, `mmio_direction[7:0]`,
   `mmio_x[15:0] as u16`). Bus'a ait ya da sabit alana `<=`/`=` ile
   yazma → E4006 (ikincil etiket: register bildirimi).
4. `@w1c` temizleyicisi, alanı yazan kullanıcı `on` bloğunun BAŞINA
   eklenir: aynı döngüde donanım kurması yazılım temizlemesini yener
   (durum bayrağı standardı). Böyle bir blok yoksa ayrı `on` bloğu
   kalır.

Neden metin üretimi: bundle/pipeline desugar'ları AST'yi elle kurar;
`@mmio` için ~80 satırlık bus adaptörü + register mantığı el
kurulumuyla okunmaz olurdu. Üretilen Volt metni okunabilirdir ve
`ParseResult.generated` ile sürücüye döner: `load_unit` sentetik
kaynağı SourceMap'e aynı kimlikle kaydeder, böylece üretilen koda düşen
bir tanı (`<mmio:GpioRegs>:61:41`) üretilen SATIRI gösterir. LSP aynı
kaydı yapar. Üretilen tüm isimler `mmio_` öneklidir (ayrılmış).

### 4. Üretilen RTL (özet; tamamı sv-mapping.md §17)

- Adres çözümleme: `aw_addr == base+offset || ...` (tam 32 bit
  karşılaştırma; `examples/soc` çözücüsü sayfa göreli adres verdiğinden
  `gpio.volt` `base = 0` kullanır).
- Protokol AxiToReg ile aynıdır: `aw` ve `w` birlikte kabul edilir, bir
  istek bir cevap, cevap master alana kadar tutulur, `prot != 0` →
  SLVERR ve yazma yok sayılır.
- Yazma: `if mmio_we { match aw_addr { A => { mmio_r <= (mmio_r &
  ~mmio_wmask) | (w_data & mmio_wmask) } _ => { } } }`; ReadOnly
  register'a yazma yok sayılır (OKAY), haritada olmayan adres → SLVERR
  (b_resp = 2).
- Okuma: `match ar_addr { A => { mmio_rdata <= mmio_r_rd } _ => {
  mmio_rdata <= 0 } }`; WriteOnly ve haritasız adres 0 okur, haritasız
  adres SLVERR.
- `@self_clearing`: her döngü `mmio_r[bit] <= false`, ardından gelen
  yazma bunu yener → bir döngülük darbe.
- `@w1c`: `if mmio_we && aw_addr == A && w_strb[k] && w_data[bit] {
  alan <= false }`.

### 5. Otomatik kontratlar

Her `@mmio` modülüne eklenir (kullanıcı kendi kontratlarını da yazar;
`regs.<reg>.<alan>` kontratlarda geçerlidir):

```volt
// haritasız adres → SLVERR (yazma ve okuma)
invariant: prev(aw_valid) && prev(w_valid) && !prev(mmio_bvalid)
           && !(prev(aw_addr) == A0 || ...) -> b_valid && b_resp == 2
invariant: prev(ar_valid) && !prev(mmio_rvalid)
           && !(prev(ar_addr) == A0 || ...) -> r_valid && r_resp == 2
// ReadOnly (sabit) register yazma ile değişmez: hep sıfırlama değerini okur
invariant: prev(ar_valid) && !prev(mmio_rvalid) && prev(ar_addr) == A -> r_data == 0
// her register'a en az bir erişim
cover: ar_valid && ar_ready && ar_addr == A        // okunabilir
cover: aw_valid && aw_ready && aw_addr == A        // WriteOnly
```

Volatile ReadOnly register için "yazma ile değişmez" kontratı
üretilmez: donanım onu aynı döngüde değiştirebilir, kontrat doğru
ifade edilemez.

### 6. Tanılar

- **E0015 — MMIO register haritası yerleşim hatası** (parser/mmio.rs):
  aynı offset'te iki register (ikincil etiket: ilk bildirim), hizasız /
  aralık dışı offset, alanlar toplamı > 32 bit, desteklenmeyen alan
  tipi, yinelenen register/alan adı, adlandırılmış alanı olmayan
  register, `@mmio` olmayan modülde `@reg`. Beş parça: kod, konum,
  açıklama, öneri, ADR referansı.
- **E4006 — bus'a ait MMIO register alanına RTL'den yazıldı**
  (parser/mmio.rs, yeniden yazımdan önce kaynak adlarıyla): volatile
  olmayan register alanına `<=`/`=`. Beş parça + ikincil etiket (register
  bildirimi) + gerekçe notu (üretilen bus mantığı tek yazar).
- **E0009** (mevcut kod, yeni kullanımlar): `@mmio`/`@reg` argümanı
  geçersiz, desteklenmeyen bus, `volatile` + `ReadWrite`, `@w1c`/
  `@self_clearing` yanlış yerde.
- `volt explain E0015` / `E4006` iki dilde.

## Sonuçlar

`examples/soc/gpio.volt`:

| | Keşif (elle) | ADR-0044 |
|---|---|---|
| Satır (yorumsuz) | 55 | 17 |
| Register haritası + bus köprüsü satırı | 40+ (19 bağlama + 21 mantık) | 8 (3 `@reg` + 1 `on`) |
| SV port sayısı | 22 | 22 (aynı adlar) |
| Kontrat | 1 cover | 1 cover + 2 invariant + 3 cover (otomatik) |

Uygulanan `@reg` özellikleri: `offset`, `access = ReadWrite | ReadOnly |
WriteOnly`, `volatile`, `@reserved`, `@self_clearing`, `@w1c`; alan
tipleri `bool` / `bits<N>` / `uN`; `base` adres tabanı; `bus = AXI4Lite`.

Doğrulama: `examples/soc/top_test.volt` sim testleri (Verilator,
Docker) geçer, SoC hiyerarşisi Verilator `-Wall` temiz, otomatik +
kullanıcı kontratları SymbiYosys ile prove/bmc/cover'da geçer
(sayılar CHANGELOG'da). Testler: `tests/ui/pass/57_mmio_basic.volt`,
`58_mmio_access_control.volt`, `tests/ui/fail/44_mmio_write_readonly.volt`
(E4006), `45_mmio_offset_overlap.volt` (E0015); crate testleri
volt-syntax/tests/mmio_tests.rs, volt-hir/tests/mmio_semantic_tests.rs,
volt-sv-emit/tests/mmio_emit_tests.rs.

## Sınırlar / Ertelenen

- Yalnız RTL tarafı: Rust/C sürücü, `regmap.json`/`regmap.md` üretimi
  sonraki tur (v3 §4.2).
- Tek bus: AXI4-Lite; APB/Wishbone → E0009. Veri yolu 32 bit; 64 bitlik
  register yok.
- ReadOnly + volatile olmayan register sıfırlama değeri 0'dır; sabit
  değerli kimlik sözcüğü (`= 0x5C01`) sonraki tur.
- `@w1c` ve `@self_clearing` yalnız `bool` alanlarda.
- Register'ın tamamına (`regs.control`) sözcük olarak erişim yok;
  alanlar tek tek okunur.
- Register haritası olmayan bir `@mmio` modülü (hiç `@reg`) yalnız
  SLVERR döner; yazılabilir register'ı olmayan bir blokta `w_data` /
  `w_strb` kullanılmaz (W1001 + Verilator UNUSED).
- Kullanıcı `mmio_` önekli ad ve AXI bundle portlarını (`aw`, `w`, `b`,
  `ar`, `r`) kendisi bildiremez (E1003 çift bildirim).
- Üretilen sentetik kaynak (`<mmio:Ad>`) diske yazılmaz; yalnız
  tanılarda görünür. `volt build --emit-mmio` gibi bir döküm sonraki tur.
