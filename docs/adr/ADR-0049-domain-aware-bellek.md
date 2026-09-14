# ADR-0049: Domain-Aware Bellek — `AsyncDualPortRam<T, DEPTH>`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-14
> Etkilenen: volt-ast (builtin tablosu), volt-hir (domain.rs W3006 çift
> saatli biçim, W3003 dördüncü alternatif), volt-sv-emit (primitif
> SV/kontrat üretimi), volt-diagnostics (W3006 başlığı, W3003/W3006
> açıklamaları, stdlib konu sayfası), volt-lsp (docs.rs),
> docs/stdlib.md, examples/vga/frame_buffer.volt, tests/ui pass/65 +
> fail/51
> Önceki kararlar: ADR-0027 (CDC primitifleri), ADR-0029 (DualPortRam),
> ADR-0047 (sembolik `@Src`/`@Dst` alanları)

## Sorun

`examples/vga/` keşfi (rapor 8c, README §3/§7): stdlib `DualPortRam`
TEK saat varsayar — dokuz portu da `DomainRole::Src`, tek `clk`. Bir
kare tamponu gibi "bir alanda yaz, ötekinde oku" belleği için Volt'ta
tip sisteminin kabul ettiği hiçbir yol yoktu:

- `DualPortRam`'in A portunu öteki alandan beslemek 3× E3001.
- Elle `reg mem : [bool; N]` yazıp öteki `on` bloğundan okumak E3001
  (dizi okuması diziyi indeksin alanıyla birleştirir).
- `extern module` sarmalayıcı ADR-0047 ile SINIRDA denetlenir ama
  sv-emit onu üretemez (E0003).

Sonuç: geçiş yazma YOLUNA taşındı — (x, y, bit) tek sözcüğe
paketlenir, `AsyncFifo<u14, 16>` ile karşıya geçer, piksel tarafında
pop/valid register'ı ve sözcük çözme ile tek saatli `DualPortRam`'e
yazılır. 99 satır (49 kod satırı), ~6 çevrim yazma gecikmesi, 16
girişli FIFO ve yazıcının görmek zorunda olduğu `wr_full`. E3001 ihlali
YAKALIYOR ama doğru çözümü seçmek ("iki flop mu, gray kod mu, FIFO
mu?") hâlâ kullanıcının işi — tam olarak Volt'un çözmeyi vaat ettiği
düşünme yükü.

## Karar

### 1. Yeni primitif: `AsyncDualPortRam<T, DEPTH>` (mevcut DualPortRam'e DOKUNULMADI)

| Port | Yön | Tip | Rol |
|---|---|---|---|
| `wr_clk` | in | clock | Src |
| `wr_addr` | in | `bits<clog2(DEPTH)>` | Src |
| `wr_data` | in | `T` | Src |
| `wr_en` | in | bool | Src |
| `rd_clk` | in | clock | Dst |
| `rd_addr` | in | `bits<clog2(DEPTH)>` | Dst |
| `rd_data` | out | `T` | Dst |

Port rolleri ADR-0047'nin sembolik `@Src`/`@Dst` alanlarıdır; K8'in
yerleşik eşleniği (`check_builtin_instance`) saat bağlamalarından
haritayı çıkarır, diğer portları `check_compat` ile denetler → yanlış
alan **E3001** (bağlama satırında), `rd_data` okuması `rd_clk`'nin
alanını taşır. Yeni denetim kodu YAZILMADI: `DomainRole` tablosu
AsyncFifo için zaten vardı, bellek yalnız bir port tablosudur.

Okuma portu `rd_en` taşımaz: her `rd_clk` çevriminde okur (BRAM'in
doğal davranışı; enable, kullanıcı tarafında adresi tutarak elde
edilir). `DEPTH` kuralı DualPortRam ile aynı (iki kuvveti, 2..=65536,
E2025).

### 2. Üretilen SV — bellek dizisi CDC sınırıdır

```systemverilog
logic [W-1:0] m_mem [DEPTH];
logic [W-1:0] m_rd_data;

always_ff @(posedge wr_clk) begin          // resetsiz, saf yazma portu
    if (wr_en) m_mem[wr_addr] <= wr_data;
end

always_ff @(posedge rd_clk) begin          // hedef alanın reseti
    if (rst) m_rd_data <= '0;
    else     m_rd_data <= m_mem[rd_addr];
end
```

- **Adres senkronizasyonu GEREKMEZ.** AsyncFifo'nun gray pointer'ları
  iki alanın aynı sayaç üzerinde anlaştığı için vardır; burada iki port
  bağımsız adres üretir ve her adres kendi alanında kalır. Bellek
  hücresi iki saatin buluştuğu tek yerdir — sentez araçlarının gerçek
  çift saatli block RAM'e eşlediği kalıp tam budur.
- **BRAM çıkarımı korunur:** bellek dizisi resetlenmez, yazma bloğu
  reset dalı taşımaz (`builtin_always_ff_no_reset`), formal init
  diziye dokunmaz. Yosys `synth_xilinx` sonucu FrameBuffer =
  **1 RAMB18E1, 0 FDRE** (önceki tasarım: 1 RAMB18E1 + 3 RAM32M +
  53 FDRE).
- İki `always_ff` bloğu tek sürücülüdür (diziyi yalnız yazma bloğu
  sürer) — Verilator `-Wall` temiz.

### 3. Eş zamanlı erişim: TANIMSIZ okuma, W3006 çift saatli biçim

Yazma ile aynı adresin diğer saatten okunması çakışırsa okunan değer
tanımsızdır — iki saat arasında "aynı çevrim" tanımlı olmadığından bu
hiçbir senkronizatörle sıralanamaz. Bu bir kontrat olarak İFADE
EDİLEMEZ (alanlar arası `wr_addr == rd_addr` karşılaştırması kullanıcı
kodunda E3001, primitif içinde ise yapısal olarak reddedilen tasarımı
cover ile "erişilebilir" ilan etmek anlamsız olurdu). Bunun yerine:

- **W3006 her örneklemede** (W3005/DualPortRam kalıbı), DualPortRam'in
  "B kazanır" metninden AYRI metinle: "read of an address being written
  from the other clock is undefined"; öneri ping-pong bölgeler /
  okumadan önce el sıkışma / tek bayat örneğe tahammül.
- W3006'nın başlığı genellendi: "Same-address port collision is not
  detected (DualPortRam write-write, AsyncDualPortRam
  read-during-write)"; `volt explain W3006` iki biçimi anlatır.
- docs/stdlib.md "Eş zamanlı erişim" paragrafı ve karşılaştırma tablosu.

### 4. Kontratlar (Immediate SVA)

- `inv_0: wr_addr < DEPTH` — `wr_clk`'ta örneklenir.
- `inv_1: rd_addr < DEPTH` — `rd_clk`'ta örneklenir.
- `cov_0: wr_en` — okuma her çevrim olduğundan "eş zamanlı okuma ve
  yazma" = yazma etkinliğinin erişilebilirliği.
- ADR-0027'nin çift saatli "iz ortası reset yok" kenar varsayımı iki
  saatte de üretilir (`has_dst_clock() == true`).
- Formal init yalnız `rd_data`'yı sıfırlar.

### 5. W3003 dört alternatif gösterir

```
= help: for multi-bit data use one of:
           AsyncFifo<T, N>   — data streams
           HandshakeSync<T>  — single transfers
           gray coding       — counters
           AsyncDualPortRam<T, N> — random-access data
```

`volt explain W3003` ve `volt explain stdlib` (artık on iki bileşen)
aynı listeyi taşır.

### 6. `examples/vga/frame_buffer.volt`

| | Önce (AsyncFifo + DualPortRam) | Sonra (AsyncDualPortRam) |
|---|---|---|
| Kaynak | 99 satır / 49 kod satırı | 52 satır / 24 kod satırı |
| Elle CDC gövdesi (portlar hariç) | ~35 kod satırı: paketleme, FIFO, pop, valid register, çözme, RAM | 8 kod satırı: iki adres `let`, bir örnekleme, bir atama |
| Üretilen SV | 135 satır | 49 satır |
| Yazma gecikmesi | ~6 kenar | 1 `sys_clk` kenarı |
| Yazıcı arayüzü | `wr_full` geri basıncı | yok (asla dolmaz) |
| Sentez (synth_xilinx) | 1 RAMB18E1 + 3 RAM32M + 53 FDRE | 1 RAMB18E1 |
| Formal (`bmc 24`) | 15 özellik, 212 s | 11 özellik, 6 s |

Kasıtlı ihlal: `rd_addr: wr_addr` (SysDomain adresi okuma portuna) →
**E3001** bağlama satırında, iki alan etiketli; geri alındı. `VgaTop`
`wr_full`/`fb_dbg` bağımlılıklarını kaybetti (`advance = !done_r`);
7 simülasyon testi değişmeden geçiyor, Verilator `-Wall` temiz.

## Sonuçlar

- Volt artık "bir alanda yaz, ötekinde oku" belleğini tip denetimli
  tek primitifle ifade eder; kullanıcı senkronizasyon kararı vermez
  çünkü verilecek karar yoktur — adresler alanlarını terk etmez.
- ADR-0047'nin sembolik alan modeli ilk yerleşik-olmayan-FIFO müşterisini
  buldu: `DomainRole` tablosu tek başına yetti, K8 kodu paylaşıldı.
- Değişen dosyalar: volt-ast/builtin.rs (+1 varyant, +1 port tablosu),
  volt-hir/domain.rs (W3006 ikinci dal, W3003 dördüncü satır),
  volt-sv-emit/builtin_prim.rs (`emit_async_dual_port_ram`,
  kontratlar, init, `builtin_always_ff_no_reset`), volt-diagnostics
  (messages/explain/topics), volt-lsp/docs.rs, docs/stdlib.md,
  examples/vga (frame_buffer, vga_top, test, README), tests/ui 65/51,
  +33 test.

## Sınırlar

- **Tanımsız okuma kontratla ifade edilmez** (§3); W3006 + belge.
  Alanlar arası kontrat sözdizimi (README §7.5) ayrı bir ADR ister.
- `rd_en` yok: okuma her çevrim. Enable gerekiyorsa adresi tutun ya da
  `rd_data`'yı kullanıcı register'ında dondurun.
- Byte-enable, iki tarafta da yazma (true dual-port'un tam hali) ve
  farklı port genişlikleri (asimetrik BRAM) kapsam dışı — ihtiyaç
  çıkınca ayrı port tablosu.
- Simülasyon harness'ı iki saati aynı üreteçten sürer (README §5);
  gerçek asenkronluk (tanımsız okuma penceresi) simüle edilmez.
- Formal `prove` çok saatli modüllerde hâlâ sarmalayıcının reset
  sıralama boşluğuna takılır (README §6) — bu ADR'nin konusu değil.

## Test

- `tests/ui/pass/65_async_dual_port_ram.volt` — iki alanlı tampon,
  yalnız W3006; sv-emit `must_emit` listesinde (temiz SV), Verilator
  `-Wall` temiz.
- `tests/ui/fail/51_async_ram_domain_violation.volt` — `rd_addr`
  yazma alanından → E3001 (satır denetimli).
- `crates/volt-hir/tests/async_ram_tests.rs` — 15 test: temiz geçiş,
  W3006 metni/5 parça, her port için yanlış alan E3001, çıkışın alanı,
  E2025/E2003/E1009, DualPortRam regresyonu.
- `crates/volt-sv-emit/tests/async_ram_emit_tests.rs` — 14 test: iki
  always_ff, resetsiz yazma bloğu, dizinin yalnız iki satırda geçmesi,
  senkronizatör yokluğu, alan erişimi, multiclock işareti, kontratlar
  saat başına, cover, formal init, kenar varsayımları.
- domain_tests (W3003 dört alternatif), explain_tests (W3003 iki
  dilde AsyncDualPortRam), lsp (tamamlama + imza), ui_semantic (65/51).
- examples/vga: 7/7 simülasyon (Docker Verilator), `bmc 24` 11/11,
  `cover 12` erişildi, `synth_xilinx` 1 RAMB18E1.
