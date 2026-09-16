# ADR-0054: SDC/XDC Üretimi — `@timing` Uygulanıyor, Zamanlama Kısıtları Domain Bilgisinden Türetiliyor

> Statü: KABUL EDİLDİ (ADR-0048 Bölüm 2'yi gerçekler ve günceller)
> Tarih: 2026-09-16
> Etkilenen: volt-hir (`constraints/` YENİ: mod.rs, parse.rs, walk.rs;
> `attrs.rs` listesi ve W0021 notu; `lib.rs` `analyze`), volt-sdc-emit
> (YENİ crate: lib.rs, render.rs), volt-driver (`--emit=sdc,xdc`,
> `Compiled.constraints`, `write_constraint_outputs`, W0022 yalnız
> istek üzerine), volt-lsp (analysis.rs E0017), volt-diagnostics (E0017,
> W0022, W0021 explain), docs/spec/cli-contract.md §4/§5 (bu ADR
> kaynaklı), examples/vga (domain `frequency`, `@timing >=`),
> tests/ui/pass 73-74, tests/ui/fail 57, crate testleri
> volt-hir/tests/constraints_tests.rs, volt-sdc-emit/tests/render_tests.rs,
> volt-driver/tests/sdc_emit_tests.rs.
> DOKUNULMADI: crates/volt-sv-emit, crates/volt-syntax, README.md.

## Sorun

ADR-0048 `@timing`, `@false_path` ve `@multicycle` için "ayrıştırılıyor
ama uygulanmıyor" uyarısını (W0021) getirdi ve SDC üretimini plan olarak
bıraktı. Zamanlama kısıtları FPGA/ASIC akışının zorunlu parçasıdır:
kullanıcı `.xdc`/`.sdc` dosyasını elle yazıyor ve Volt'taki domain
bilgisiyle (hangi port hangi saat, hangi saatler birbirine asenkron,
hangi register'lar senkronizatör) elle senkron tutmak zorunda kalıyordu.
Oysa bu bilginin tamamı derleyicide ZATEN var — eksik olan yalnız
frekans sayısı ve bir yazıcı.

### Durum tespiti (bu ADR'nin 1. adımı)

| Bilgi | Nerede? | SDC için yeterli mi? |
|---|---|---|
| Saat portu → alan | `Port.domain` (`@Ad`) / anotasyonsuz portta örtük alan (K2) | evet |
| Alan kenarı | `DomainKey::Clock` → `ClockEdge` | evet (yorum) |
| Alan frekansı | `DomainKey::Frequency` gramerde ve ayrıştırılıyor, **hiçbir geçit okumuyordu** | evet — `100.mhz` bir `Field{IntLit, mhz}` ifadesi olarak zaten ayrışıyor |
| Reset | `DomainKey::Reset` → `ResetSpec` | SDC'ye girmez (reset yolu için ayrı kısıt yok) |
| İki alan asenkron mu | domain çıkarımı (ADR-0023): farklı alan = asenkron | evet |
| `sync()` köprüleri | sv-emit `try_emit_sync_bridge`: `sync_<kaynak>_stage<i>`, başka alandan porta `sync_<kaynak>_src` | evet — adlandırma deterministik |
| AsyncFifo / HandshakeSync / PulseSync / AsyncDualPortRam register'ları | sv-emit `builtin_prim.rs`: `<örnek>_<reg>` | evet |
| `@timing` biçimleri | `AttrArg::Named{ad, ifade}` / `Positional(ifade)`; `clk >= 100.mhz`, `max_delay(a, b) <= 5.ns`, `from = a, to = b`, `cycles = 3` **bugün hatasız ayrışıyor** | evet |
| Ondalık literal (`25.175.mhz`) | lexer'da yok (E0001) | hayır — `25_175.khz` ya da `25175000` ile yazılır |

Sonuç: yeni sözdizimi gerekmiyor. Yeni olan yalnız bir yorumlayıcı
(volt-hir `constraints/`) ve bir yazıcı (volt-sdc-emit).

## Karar

### 1. Kısıt modeli volt-hir'de, metin volt-sdc-emit'te

`volt_hir::collect_constraints(ast) -> ConstraintResult` çözümlemeden
bağımsızdır (yalnız AST): saat portu olan her modül için bir
`ModuleConstraints { clocks, groups, bridges, paths }` kurar ve E0017
tanılarını döndürür. `volt-sdc-emit` bu modeli `.sdc`/`.xdc` metnine
çevirir; saf fonksiyondur (ADR-0053 `volt-sw-emit` disiplini). Sürücü
`--emit=sdc,xdc` ile `build/constraints/<Modül>.sdc|xdc` yazar (modül
adı ADR-0024 ile aynı).

Her modül **üst modül kabul edilerek** kendi dosyasını alır: alt
modüllerin köprüleri ve yol kuralları `örnek/` önekiyle üst dosyaya
düzleştirilir (Vivado / Design Compiler hiyerarşik hücre adı), alt
modülün saat portları bağlama üzerinden üst modülün saat adlarına
eşlenir. Kullanıcı üst modülün dosyasını alır; alt modül dosyaları
bağımsız sentez için oradadır.

### 2. Otomatik kısıtlar (nitelik gerekmez)

| Kaynak | SDC |
|---|---|
| `domain D { frequency = F }` + `in clk : clock @D` | `create_clock -name clk -period <1/F ns, 3 ondalık> [get_ports clk]` |
| iki+ farklı alan | `set_clock_groups -asynchronous -group [get_clocks {a}] -group [get_clocks {b}]` — aynı alandaki saatler aynı grupta; frekansı olmayan saat gruba giremez (`get_clocks` tanımsız saatte başarısız olur); iki tanımlı gruptan az kaldıysa komut üretilmez |
| `y = sync(x, dst)` / `sync3` | `set_false_path -from <x> -to [get_cells {sync_x_stage0_reg*}]`; `<x>`: register → `get_cells {x_reg*}`, başka alandan port → `get_cells {sync_x_src_reg*}` (yakalama register'ı), aynı alandan port → `get_ports x`, `let` → ifadesinden izlenen kaynak saat `get_clocks` (tanımlıysa), aksi hâlde yalnız `-to` |
| `AsyncFifo f` | `f_rgray → f_rgray_s0`, `f_wgray → f_wgray_s0`, `f_mem → f_rd_data` (üçü de `set_false_path`) |
| `HandshakeSync h` | `h_req → h_req_s0`, `h_ack → h_ack_s0`, `h_data_q → h_data_out` |
| `PulseSync p` | `p_toggle → p_sync0` |
| `AsyncDualPortRam m` | `m_mem → m_rd_data` |
| XDC (Vivado) | ek olarak her senkronizatör zinciri: `set_property ASYNC_REG TRUE [get_cells {..._stage0_reg* ..._stage1_reg*}]` |

`set_clock_groups -asynchronous` zaten alanlar arası her yolu
zamanlama dışı bırakır; köprü başına `set_false_path` yine üretilir:
(a) kullanıcı grup satırını silse de köprüler korunur, (b) XDC'de
`ASYNC_REG` için zincir adları zaten hesaplanır, (c) dosya hangi
register'ın senkronizatör olduğunu belgeler.

Frekansı olmayan alan: `create_clock` ÜRETİLMEZ, dosyaya yorum satırı
yazılır ve **W0022** verilir — alan bildirimi başına bir kez (anotasyonsuz
saat portunda port başına). W0022 yalnız `--emit=sdc,xdc` istendiğinde
üretilir: SDC istemeyen bir tasarımdan frekans istenmez, mevcut
örnek/test çıktıları değişmez.

### 3. Nitelik biçimleri (desteklenenlerin tam listesi)

| Biçim | Konum | SDC | Denetim |
|---|---|---|---|
| `@timing(clk = F)` | modül | alan frekansı yoksa `create_clock` kaynağı | alan frekansı varsa eşit olmalı, yoksa **E0017** (çelişki; alan bildirimi ikincil etiket) |
| `@timing(clk >= F)` | modül | alan frekansı yoksa `create_clock` kaynağı | alan frekansı `F`'ten küçükse **E0017**; alt modül örneğinde üst saatin frekansına karşı da denetlenir |
| `@timing(max_delay(a, b) <= T)` | modül | `set_max_delay <T ns> -from <a> -to <b>` | |
| `@timing(min_delay(a, b) >= T)` | modül | `set_min_delay <T ns> -from <a> -to <b>` | |
| `@false_path(from = a, to = b)` | modül (en az biri), port (`in` → `-from`, `out` → `-to` örtük), `reg` (`-to` örtük) | `set_false_path` | |
| `@multicycle(from = a, to = b, cycles = N)` / `@multicycle(N)` | modül / port / `reg` | `set_multicycle_path N -setup` + `N-1 -hold` (N ≥ 2) | |

Birimler: frekans `25175000` (Hz), `.hz .khz .mhz .ghz`; süre `.ps .ns
.us` — süre birimi ZORUNLU. Uç noktalar aynı modülün portu (`get_ports`,
alt modülde `get_pins {örnek/port}`) ya da register'ıdır (`get_cells
{ad_reg*}`); `wire`/`let` ve örnek çıkışları uç nokta değildir.

Desteklenmeyen her şey **E0017** ("desteklenmeyen ya da tutarsız
zamanlama kısıtı"): bilinmeyen sinyal, birimsiz süre, `clk <= F` (üst
sınır bir zamanlama kısıtı değildir), `max_delay` ile `>=`, port ya da
register üstünde `@timing`, `reg` dışı deyimde `@false_path`, `cycles`
eksik, argüman iki kez. E0017 `volt check`, LSP ve `volt_hir::analyze`
ile her zaman koşar; generic klonlar ve birden çok üstten yürünen alt
modüller için konum başına BİR kez raporlanır.

`@false_path`/`@multicycle` yapısal kanıtı (ADR-0048 E6003/E6004) bu
ADR'de YOK: kullanıcı ne yazdıysa çevrilir. Kanıt katmanı ayrı karar.

### 4. W0021 güncellendi

`UNENFORCED_ATTRIBUTES`'tan `timing`, `false_path`, `multicycle`
çıktı; `reason_and_help` tablosundaki satırları silindi. W0021 artık
ikinci bir notla hâlâ uygulanmayanları listeler ("still unenforced:
@domain, @budget, @version, @abi_version, @dft, @debug_visible,
@debug_trace, @synthesis_target") ve zamanlama ailesinin `--emit=sdc`
ile uygulandığını söyler. `tests/ui/pass/63-64` örneği `@budget`'a geçti.

### 5. Çıktı düzeni ve dosya başlığı

```
build/constraints/<Modül>.sdc      --emit=sdc
build/constraints/<Modül>.xdc      --emit=xdc
```

```
# Generated by Volt 0.1.0
# Source: vga_top.volt
# DO NOT EDIT
# Module: VgaTop
# Dialect: SDC (Synopsys Design Constraints)
```

Bölümler sırayla: saatler, saat grupları, CDC köprüleri, yol kısıtları;
her komutun üstünde kaynağını söyleyen bir yorum. Yollar JSON zarfının
`artifacts` listesine RTL ve SVA'dan sonra girer. Saat portlu modül
yoksa `Note: no module with a clock port in the unit; --emit=sdc
produced nothing`, çıkış 0.

### 6. XDC farkı

Vivado SDC'nin bir üst kümesini okur; `create_clock`, `set_clock_groups`,
`set_false_path`, `set_max_delay`, `set_multicycle_path` birebir aynı.
Tek ek: senkronizatör zincirlerine `set_property ASYNC_REG TRUE` (Vivado
flop çiftini yan yana yerleştirir, metastabilite çözünürlük süresini
korur; SDC'de karşılığı yok). `-datapath_only` (Vivado'nun CDC için
önerdiği `set_max_delay` biçimi) üretilmez: `set_clock_groups` +
`set_false_path` ikilisi Vivado'da da geçerlidir ve iki lehçe arasında
tek satırlık farkı korur.

### 7. Doğrulama

- **Sözdizimi kapısı (CI, araçsız):** `volt_sdc_emit::syntax_check` —
  her mantıksal satır (Tcl `\` devamı birleştirilmiş) bilinen bir SDC
  komutuyla başlar, `[]`/`{}` dengeli, sayılar `d.ddd`, `create_clock`
  `-period` + `get_ports`, yol komutları `-from`/`-to` taşır. Üretilen
  her dosya bu kapıdan geçer (`render_tests`, `sdc_emit_tests`).
- **OpenSTA 3.1.0 (Docker `openroad/opensta`, yerel, 2026-09-16):**
  `examples/vga` SV'si Yosys 0.36 (`hdlc/formal`) ile hiyerarşi
  korunarak minimal bir Liberty'ye (DFF/AND/OR/XOR/MUX/NOT/BUF)
  sentezlendi; DFF hücreleri sürdükleri Q telinin adıyla `<ad>_reg` /
  `<ad>_reg[i]` olarak adlandırıldı (Yosys `rename -wire` yalnız tam
  tel süren hücreleri adlandırdığından bit dilimleri için küçük bir
  netlist son işlemcisi kullanıldı). Sonuç:
  - `read_sdc constraints/VgaTop.sdc` hatasız; `report_clock_properties`
    `sys_clk 10.00`, `pix_clk 39.72`.
  - Üretilen her `get_cells` deseni gerçek hücreyle eşleşti:
    `sync_invert_stage0_reg*` 1, `sync_done_r_stage0_reg*` 1,
    `sync_vs_active_stage0_reg*` 1, `done_r_reg*` 1,
    `fb/mem_rd_data_reg*` 1, `fb/mem_mem_reg*` 8192.
  - `report_checks -from [get_clocks sys_clk] -to [get_clocks pix_clk]`
    ve tersi: `No paths found.` (asenkron gruplar + false path'ler
    etkili); `-to sync_invert_stage0_reg/D`: `No paths found.`;
    `report_worst_slack -max` 7.70 ns (alan içi yollar zamanlanıyor).
  - Komutlar `scripts/`'e alınmadı (Docker + 8192 flop'luk sentez;
    CI `verify` işi gibi isteğe bağlı kalır).
- XDC dosyası OpenSTA ile okunamaz (`set_property` Vivado komutudur);
  XDC yalnız `syntax_check` kapısından ve SDC ile satır satır farkının
  `ASYNC_REG`'e indirgendiğini gösteren testten geçer.
- Yosys `read_sdc` komutu yoktur; Yosys tarafında yalnız hücre adı
  eşleşmesi doğrulanır.

## Sınırlar

- Ondalık literal hâlâ yok: `25.175.mhz` E0001; ADR bilinçli olarak
  gramer değiştirmez (`25_175.khz`).
- Register hücre adı `<ad>_reg*` Vivado/Design Compiler geleneğidir;
  Quartus `<ad>[i]` kullanır — Quartus kullanıcısı `get_cells` desenini
  uyarlamalı (ileride `--sdc-style=quartus`).
- `set_clock_groups` alan başına bir grup varsayar; `@timing` ile iki
  alanın senkron olduğunu söylemenin yolu yok (aynı `@Ad` kullanılır).
- `let` kaynaklı `sync()` için kaynak saat sığ izlenir (ifadedeki ilk
  saatli sinyal, 8 derinlik); bulunamazsa `-from` düşer, `-to` kalır.
- `@false_path`/`@multicycle` yolun VARLIĞI kanıtlanmaz (E6003/E6004
  ayrı ADR); `@multicycle` `-hold` her zaman `N-1`.
- `create_clock` `-waveform` üretilmez; negedge alanı yorumda belirtilir.
- Generic modülün monomorph klonları ayrı modül (`Delay_4`) olarak
  dosya alır; örnek adı öneki ise klonun adına değil örneğe bağlıdır.
- `extern` modüller ve dosya dışı hedefler yürünmez.

## Ölçütler

- `volt build --emit=sdc,xdc examples/vga/vga_top.volt` → `VgaTop.sdc`:
  iki `create_clock` (10.000 / 39.722), `set_clock_groups -asynchronous`,
  `fb/mem_mem_reg* → fb/mem_rd_data_reg*` ve üç `sync()` köprüsü için
  `set_false_path`; `VgaTop.xdc` ek `ASYNC_REG`; `volt check` VGA için
  `0 error(s), 1 warning(s)` (W3006 kaldı, W0021 gitti).
- `tests/ui/pass/73_sdc_single_clock`, `74_sdc_multi_clock` temiz;
  `fail/57_timing_unsupported_form` E0017 (satır eşleşmeli).
- `volt explain E0017`, `W0022`, güncel `W0021` iki dilde; 120 kod.
- `cargo test --all` yeşil; +30'dan fazla test (constraints_tests 39,
  render_tests 15, sdc_emit_tests 10, attrs/ui/parser güncellemeleri);
  baseline 1806 → 1879.
