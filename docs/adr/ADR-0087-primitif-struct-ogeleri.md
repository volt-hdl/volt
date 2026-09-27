# ADR-0087: Yerleşik Primitiflerde Struct / Enum Öğe Tipi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-27
> Etkilenen: volt-sv-emit (`structs/prim.rs` yeni, `builtin_data.rs` yeni,
> `builtin_prim.rs`, `structs/mod.rs`), volt-hir (`typeck/instance.rs`),
> volt-diagnostics (E2009 açıklaması, `stdlib` konusu), `templates/fifo`
> Kaynak: ADR-0084 §5 bulgu 1; ADR-0077 (struct, bit düzeni), ADR-0074
> (enum kodlaması), ADR-0027/0029/0049 (primitifler)

## Sorun

`SyncFifo<Packet, 8>` E0003 ("a whole struct value in this position");
`fifo` şablonu `in_pkt as u12` ile paketleyip `fifo.rd_data as Packet`
ile açıyordu.

### Tarama — hangi primitif generic `T` alıyor?

Sonda `build/c3/gen.py` (7 primitif × {struct, enum, enum alanlı struct,
`u12`}), main 9fb467e:

| Primitif | `T` portları | `u12` | enum | struct | enum alanlı struct |
|---|---|---|---|---|---|
| `SyncFifo<T, DEPTH>` | wr_data → rd_data | ✓ | ✓ | E0003 | E0003 |
| `AsyncFifo<T, DEPTH>` | wr_data → rd_data | ✓ | ✓ | E0003 | E0003 |
| `Ram<T, DEPTH>` | wr_data → rd_data | ✓ | ✓ (!) | E0003 | E0003 |
| `DualPortRam<T, DEPTH>` | a/b_wr_data → a/b_rd_data | ✓ | ✓ (!) | E0003 | E0003 |
| `AsyncDualPortRam<T, DEPTH>` | wr_data → rd_data | ✓ | ✓ (!) | E0003 | E0003 |
| `ShiftRegister<T, LEN>` | data_in → data_out, `taps` | ✓ | ✓ | E0003 | E0003 |
| `HandshakeSync<T>` | data_in → data_out | ✓ | ✓ | E0003 | E0003 |
| `Handshake<T>` (bundle, ADR-0050) | port bundle, bellek değil | ✓ | ✓ | alan erişimi ✓, bütün `req.data` E0003 (ADR-0077 Karar 1) | aynı |

`T`'siz primitifler: `PulseSync`, `Counter<WIDTH>`, arbiter'lar,
`EdgeDetect`. HIR struct `T`'yi zaten tipliyordu; E0003 sv-emit'in
struct indirgemesinden (ADR-0077 Karar 5) geliyordu. Yan bulgular:
`ShiftRegister<Struct>.taps` HIR'da `width_of` ile 1 bit sayılıyordu;
RAM ailesi enum `T`'yi (işaretli "!") sessizce kabul ediyordu — aşağıda.

## Karar

**Struct (ve enum) `T` bütün generic primitiflerde desteklenir; `T`
bellekte ve register'larda TEK paketlenmiş sözcüktür.**

1. **Paketleme — ADR-0077 Karar 3 düzeni** (ilk alan MSB, `W` = yaprak
   genişlikleri toplamı). İndirgemeden önce (`structs/prim.rs`) veri
   girişi bağlaması `wr_data: e` → `e as uint<W>`, veri çıkışı okuması
   `m.rd_data` → `(m.rd_data) as Packet`; ADR-0077'nin mevcut `p as uN` ve
   `raw as P` kuralları birleştirme/dilime indirger. Emitter `T`'yi `W`
   bitlik vektör görür: `logic [W-1:0] mem [DEPTH]` — **AoS**, alan başına
   bellek (SoA) değil. `fifo` şablonunun SV'si elle `as u12` / `as Packet`
   sürümüyle **bayt aynı**.
2. **Reset değeri `T`'nin varsayılan kodlaması:** veri register'ları
   (`rd_data`, `data_q`, `data_out`, ShiftRegister aşamaları, formal
   init) sıfır yerine her enum yaprağı ilk varyantının koduyla reset'lenir
   (`enum Lvl : u4 { Lo = 2, Hi = 8 }` → `8'h20`, ShiftRegister
   `{3{8'h20}}`). İlk kod 0 ise (açık değersiz her enum) çıktı eskisiyle
   aynı.
3. **RAM ailesinde enum/`Trit` yapraklı `T` E2009** (aşağıda gerekçe).
4. `ShiftRegister.taps` genişliği `LEN × signal_width(T)`.

### Enum alanlı struct — `bits → struct` yasağı iç işleyişte güvenli mi?

ADR-0077: enum alanlı struct'a ham bit dökmek yasak (E2009), çünkü
"enum tipli bir değer inşa gereği geçerli bir varyanttır; F1 ve
kapsayıcı `match` buna dayanır". Primitif `rd_data`'yı açtığında
derleyici kendi yazdığı sözcüğü okur — ama yalnız o değil:

| Okunan değer | FIFO ailesi (SyncFifo, AsyncFifo, ShiftRegister, HandshakeSync) | RAM ailesi (Ram, DualPortRam, AsyncDualPortRam) |
|---|---|---|
| yazılmış değer | tipli `T` girişinden → geçerli | geçerli |
| reset / başlangıç | çıkış register'ının reset değeri — önce **0** (ilk kodu 0 olmayan enum'da geçersiz); artık `T`'nin varsayılanı → **geçerli** | hiç yazılmamış adres: belleğin başlangıç içeriği — donanımda tanımsız, formal'de serbest, Verilator'da 0/rastgele → **geçersiz olabilir** |
| sonuç | **güvenli** (Karar 2 ile) | **güvenli değil** → E2009 |

Böylece yasak iç işleyişte delinmez: FIFO ailesi yalnız geçerli `T`
gösterir; RAM ailesinde enum yapraklı öğe yasak, çözüm ADR-0077'ninki —
bitleri sakla (`Ram<u8, ...>`), okuduktan sonra alanı `match` ile çöz.
Bu, bugün sessizce kabul edilen `Ram<EnumTipi>`'ni de kapsar (korpusta
kullanım yok, golden değişmedi). Değerlendirilen seçenekler:

- *RAM'i `initial` ile varsayılana başlatmak* — reddedildi: FPGA'da BRAM
  INIT olur ama ASIC akışında yok sayılır; formal ile donanım ayrışır.
- *Yazılmamış okumaya kontrat (`assume`)* — reddedildi (ADR-0074 Karar 3,
  ADR-0077: sessiz `assume` ispatı yanlışlar).

## Ölçüm — BRAM çıkarımı

`build/c3/bram/` (Docker `hdlc/formal`, Yosys): aynı tasarım `Packet`
(12 bit) ve `u12` öğeli.

| Tasarım | `synth_xilinx -noiopad` | `synth_ice40` |
|---|---|---|
| `Ram<Packet, 1024>` | 3 hücre: 1 RAMB18E1, 1 LUT2, 1 BUFG | 64 hücre: 3 SB_RAM40_4K, 35 SB_DFF, 2 SB_DFFSR, 24 SB_LUT4 |
| `Ram<u12, 1024>` | aynı | aynı |
| `SyncFifo<Packet, 512>` | 70 hücre: 1 RAMB18E1, 28 FDRE, 9 CARRY4, ... | 155 hücre: 2 SB_RAM40_4K, ... |
| `SyncFifo<u12, 512>` | aynı | aynı |

Struct öğeli bellek `u12` ile **hücre hücre aynı**. ADR-0077'nin struct
dizisi ölçümündeki SoA kaybı (iCE40'ta 5 kat BRAM, xc7'de RAMB18 yerine
53 hücre) paketli yazımda yok — primitif belleği zaten eleman başına tek
sözcüktür.

## Doğrulama

- `prim_struct_tests.rs`: ui/pass 128 (SyncFifo, AsyncFifo, Ram,
  ShiftRegister<Cmd>, HandshakeSync<Cmd>) SV biçimi — tek bellek dizisi,
  `({pkt_tag, pkt_data})`, yaprak dilimleri; ui/fail 184 E2009 işaretli
  satırda; açık değerli enum varsayılan reset'i; sıfır varsayılan eski
  literal. `stdlib_tests.rs`: struct öğe temiz, RAM ailesi (üçü) E2009,
  `taps` 36 bit. Verilator `-Wall` (ui/pass 128) temiz.
- Şablon zinciri (Docker `volt-eng`): `fifo` lint temiz, `volt test` 3/3,
  `volt verify` bmc/prove/cover 7/7; dört şablon üç kipte geçti.
- Golden (`build/c5`, 2048 kaynak): değişen yalnız yeni fixture'lar;
  `templates/fifo` SV'si bayt aynı.
- Mutasyon (`build/c3/mutate.py`): 7/7 düştü — girişi paketlememek,
  çıkışı açmamak, varsayılan reset'i sıfırlamak, E2009'u kaldırmak,
  `taps`'i `width_of` ile saymak, ShiftRegister reset'ini sıfırlamak,
  AsyncDualPortRam'i RAM ailesinden çıkarmak.

## Sonuçlar

- (+) `SyncFifo<Packet, 8>` ve eşleri paketleme yazmadan; BRAM korunur.
- (+) FIFO ailesinde enum alanlı struct güvenle saklanır; reset'te de
  geçerli varyant (önce ilk kodu 0 olmayan enum'da değildi).
- (−) `Ram<EnumTipi>` artık E2009 (önce sessizce kabul — geçersiz kod
  okuyabiliyordu); korpusta kullanım yok.
- (−) Bütün `Handshake<T>` payload'ı değer olarak hâlâ E0003 (bundle
  düzleştirmesi, ADR-0077 Karar 1 — bu ADR belleklerle sınırlı).

## Gelecek İş

1. `Handshake<Struct>` bütün payload değeri (`req.data` → `p`).
2. Başlatılmış RAM (`Ram<T, D> = [init; D]`) — o zaman enum yapraklı `T`
   RAM ailesinde de güvenli olur.
3. Örnek adı + port adı çakışması (`hs` + `valid` → `hs_valid` kullanıcı
   portuyla aynı SV adı; ui/pass 128 yazılırken Verilator "Duplicate
   declaration" ile görüldü) — ayrı iş, `volthdl-soc-kesif` bulgusuyla aynı.
