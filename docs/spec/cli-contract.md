# Volt CLI Sözleşmesi

> STATÜ: BAĞLAYICI
> İlgili: UX Anayasası, `error-recovery.md`
> Aşama: F0'dan itibaren — CI için ÖNKOŞUL
> Karar kayıtları: ADR-0021 (çıkış kodları §2, `build/` dizini §4, stdout/stderr §11), ADR-0004 (`--lang` §3, tanı biçimi §5), ADR-0015 (determinizm İ3, `--release` planı), ADR-0019 (E9001), ADR-0022 (E7001/E7002), ADR-0063 (`check-regmap` §6a, E9003/E9004)

---

## 0. Tasarım İlkeleri

```
İ1. UNIX GELENEĞİ
    Başarıda sessiz, hatada gürültülü.
    stdout = veri, stderr = tanılar.

İ2. MAKİNE + İNSAN
    Aynı bilgi iki formatta: renkli metin ve JSON.
    CI JSON okur, insan metin okur.

İ3. ÖNGÖRÜLEBİLİR ÇIKTI
    Aynı girdi → aynı çıktı (--release modunda bit-bit).

İ4. TEK KOMUT YETERLİ
    volt run design.volt → derle + simüle + sonuç göster.
    Kurulum, yapılandırma, ön adım yok.
```

---

## 1. Komut Listesi

```
volt new <isim>              Yeni proje oluştur
volt init                    Mevcut dizinde proje başlat

volt build [dosya]           Derle → SystemVerilog
volt check [dosya]           Sadece kontrol (çıktı üretme)
volt check-regmap <dosya> --against <sürücü>
                             Sürücü ↔ register haritası denetimi (ADR-0063)
volt run [dosya]             Derle + simüle
volt test [filtre]           Test çalıştır
volt fmt [dosya]             Biçimlendir
volt explain <KOD>           Hata kodunu açıkla

volt sim [dosya]             Simülasyon                      [F4]
volt verify [dosya]          Formal doğrulama                [F4]
volt doc [dosya]             Belge üret                      [F5]
volt add <paket>             Bağımlılık ekle                 [V1]
volt diff <v1> <v2>          Anlamsal fark                   [V1]
volt migrate <dosya.v>       Verilog → Volt                  [V1]
```

---

## 2. Çıkış Kodları

**Kritik:** CI bu kodlara güveniyor.

```
KOD  ANLAM                          NE ZAMAN
────────────────────────────────────────────────────────────
0    Başarı                         Hata yok (uyarı olabilir)
1    Derleme hatası                 E-kodu üretildi
2    Kullanım hatası                Geçersiz argüman/bayrak
3    G/Ç hatası                     Dosya bulunamadı, yazma hatası
4    Yapılandırma hatası            Volt.toml bozuk/eksik
5    Test başarısızlığı             Testler çalıştı, bazıları kaldı
6    Doğrulama başarısızlığı        Formal karşı örnek buldu
101  İç hata (panic)                Derleyici hatası — bug report
────────────────────────────────────────────────────────────

Uyarılar çıkış kodunu ETKİLEMEZ (--deny-warnings hariç).
```

```rust
#[repr(i32)]
pub enum ExitCode {
    Success       = 0,
    CompileError  = 1,
    UsageError    = 2,
    IoError       = 3,
    ConfigError   = 4,
    TestFailure   = 5,
    VerifyFailure = 6,
    InternalError = 101,
}
```

---

## 3. Genel Bayraklar

Tüm komutlarda geçerli:

```
-h, --help              Yardım göster
-V, --version           Sürüm bilgisi
-v, --verbose           Ayrıntılı çıktı (-vv daha ayrıntılı)
-q, --quiet             Sadece hatalar
    --format=<f>        Çıktı formatı: human | json | short
    --lang=<l>          Tanı dili: en | tr (öncelik: bayrak > VOLT_LANG > Volt.toml [ui] lang > en)
    --color=<c>         Renk: auto | always | never
    --no-color          --color=never kısayolu
-j, --jobs=<N|auto>     Paralel iş sayısı (varsayılan: auto = CPU sayısı; bugün `verify`, ADR-0055)
    --manifest=<yol>    Volt.toml konumu
    --target-dir=<yol>  Çıktı dizini (varsayılan: build/)
```

### Renk Davranışı

```
--color=auto (varsayılan):
  stdout terminal ise → renkli
  boru/dosya ise      → renksiz
  NO_COLOR ortam değişkeni varsa → renksiz
  CI=true ise         → renksiz
```

---

## 4. Çıktı Dizini Yapısı

```
build/
├── rtl/
│   ├── counter.sv
│   └── uart.sv
├── sim/                          [F4]
│   ├── counter.vcd
│   └── counter_tb
├── formal/                       [F4]
│   ├── counter.sby
│   └── counter.sva
├── constraints/                  ADR-0054 (uygulandı): --emit=sdc,xdc
│   ├── VgaTop.sdc                modül başına (create_clock, senkronizör kısıtları; ADR-0065)
│   └── VgaTop.xdc                Vivado lehçesi (+ ASYNC_REG)
├── sw/                           ADR-0053 (uygulandı): --emit=rust,c,regmap
│                                 (her dosya regmap-hash imzalı, ADR-0063)
│   ├── gpio.rs                   no_std Rust sürücüsü (@mmio modülü başına)
│   ├── gpio.h                    C başlığı
│   └── gpio.json                 volt-regmap/1 register haritası
├── docs/                         ADR-0053 (uygulandı): --emit=regmap-md
│   └── gpio.md                   register haritası tabloları
└── .volt-cache/                  artımlı derleme
```

`--target-dir` ile değiştirilebilir. `.gitignore`'a eklenmeli.

---

## 5. `volt build`

> ADR-0024 + ADR-0042 (uygulandı): girdi dosyasının `use` bağımlılıkları
> otomatik yüklenir; çıktı **modül başına** `build/rtl/<Modül>.sv`'dir
> (başlıkta `// Module:` ve modülün kendi kaynak dosyası). `--single-file`
> aşağıdaki `build/rtl/<kaynak>.sv` düzenini korur. Bu bölümdeki örnekler
> `--single-file` çıktısını gösterir.

```bash
volt build [SEÇENEKLER] [DOSYA]
```

```
SEÇENEKLER:
    --release             Deterministik build + volt.lock
    --emit=<tür,...>      sva | rust | c | regmap | regmap-md   (uygulandı)
                          sdc | xdc                           (uygulandı, ADR-0054)
                          json-ast | json-hir | none          [V1]
    --check-regmap        Üretilen sürücü ↔ üretilen RTL adres çözümlemesi
                          (uygulandı, ADR-0063; bkz. §6a)
    --sdc-style=<stil>    targeted (varsayılan) | clock-groups — alanlar
                          arası kısıtlar (uygulandı, ADR-0065 §4.3)
    --target=<hedef>      generic | fpga-xilinx | fpga-intel |
                          asic-generic | asic-sky130
    --optimize=<seviye>   structure | aggressive
    --out=<dosya>         Tek dosya çıktısı
    --deny-warnings       Uyarıları hata say
```

### Yazılım çıktıları — `--emit=rust,c,regmap,regmap-md` (ADR-0053)

> ADR-0053 (uygulandı). Birimdeki her `@mmio` modülü (ADR-0044) için
> register haritasından yazılım tarafı üretilir; dosya adı modül adının
> snake_case halidir (`GpioRegs` → `gpio_regs`).

| `--emit` | Dosya | İçerik |
|---|---|---|
| `rust` | `build/sw/<modül>.rs` | `no_std` Rust sürücüsü: `struct <Modül> { base: *mut u32 }`, `BASE`/`*_OFFSET`/`*_MASK` sabitleri, `read_volatile`/`write_volatile` erişimciler; ReadOnly alana setter, WriteOnly alana getter ÜRETİLMEZ; `@self_clearing` → `trigger_*`, `@w1c` → `clear_*` |
| `c` | `build/sw/<modül>.h` | `#define <MOD>_BASE`, `<MOD>_<REG>`, `_OFFSET`, `_MASK`, alan başına `_SHIFT`/`_MASK`; `static inline <mod>_get_/_set_/_trigger_/_clear_<reg>_<alan>()` |
| `regmap` | `build/sw/<modül>.json` | `volt-regmap/1` şeması (ADR-0053 §5): `name`, `base`, `bus`, `registers[{name, offset, address, access: rw\|ro\|wo, volatile, fields[{name, lsb, width, type, reserved, self_clearing, w1c, doc}]}]` |
| `regmap-md` | `build/docs/<modül>.md` | `\| Offset \| Name \| Access \| Description \|` özet tablosu + register başına bit alanı tablosu; açıklamalar Volt `///` doc yorumlarından |

Kurallar:

- Değerler virgülle birleşir (`--emit=sva,rust,c`); yinelenen tür bir
  kez yazılır. Yazılan yollar JSON zarfının `artifacts` listesine RTL
  ve SVA dosyalarından SONRA girer; insan biçiminde `Output` satırı.
- Derleme hatası varsa yazılım çıktısı da üretilmez (RTL ile aynı kural).
- Birimde `@mmio` modülü yoksa dosya üretilmez; çıkış kodu 0, insan
  biçiminde `Note: no @mmio module in the unit; --emit=... produced nothing`.
- Doc yorumları (`///`) modül, register ve alan seviyesinde dört çıktıya
  aktarılır.
- ADR-0063: `.h` ve `.rs` ilk iki satırı imzadır —
  `// Generated by Volt <sürüm> from <kaynak>` ve `// regmap-hash: <16 hex>`;
  JSON'da `regmap_hash` anahtarı. Register başına reset sabiti (C
  `<MOD>_<REG>_RESET`, Rust `<REG>_RESET`, JSON `reset`); Rust'ta alan
  başına `<REG>_<ALAN>_SHIFT`/`_MASK`. Hash yalnız denetlenen olgulardan
  hesaplanır (doc yorumu, kaynak adı, sürüm ve sıra girmez).
- `--check-regmap`: her `@mmio` modülü için istenen sürücü türleri
  (hiçbiri istenmediyse `c`, `rust`, `regmap` bellekte) yeniden okunur ve
  modülün SV'sindeki adres çözümlemesiyle (`mmio_whit`/`mmio_rhit`,
  `case (ar_addr)`/`case (aw_addr)`, okuma maskesi, W1C biti, reset)
  karşılaştırılır. Ayrışma derleyici hatasıdır: E9003, çıkış 1. Uyumda
  insan biçiminde `Regmap drivers match the RTL address decode (N @mmio
  module(s))`; `@mmio` modülü yoksa `Note: ... --check-regmap checked nothing`.

### Zamanlama kısıtları — `--emit=sdc,xdc` (ADR-0054, ADR-0065)

> ADR-0054 (uygulandı). Saat portu olan her modül için, modül üst modül
> kabul edilerek `build/constraints/<Modül>.sdc` (Synopsys/Quartus/
> OpenSTA) ve/veya `.xdc` (Vivado) yazılır; dosya adı ADR-0024 ile aynı
> (modül adı). Alt modül örnekleri `örnek/` önekiyle düzleştirilir.
>
> ADR-0065 (uygulandı): varsayılan `--sdc-style=targeted` saat grubu
> YAZMAZ; yalnız üretilen senkronizörlerin ilk aşamasına giden yol ve ham
> reset portunun etkinleşme yolu kısıtlanır. Alanlar arası başka her yol
> zamanlanır (senkronizörsüz geçiş zamanlama aracında ihlal olarak
> görünür), reset bırakma yolları recovery/removal olarak analiz edilir.
> `--sdc-style=clock-groups` ADR-0054 çıktısını üretir ve başlığa
> `# Style: clock-groups — paths between domains are NOT timed; see ADR-0065`
> yazar.

| Kaynak | Üretilen satır (`targeted`) |
|---|---|
| `domain D { frequency = 100.mhz }` + `in clk : clock @D` | `create_clock -name clk -period 10.000 [get_ports clk]` |
| iki+ farklı alan | `set_clock_groups` YOK; açıklayıcı yorum (`clock-groups` stilinde `set_clock_groups -asynchronous -group [get_clocks {a}] -group [get_clocks {b}]`) |
| `sync()` / `sync3()`, `HandshakeSync` req/ack, `PulseSync` toggle | `.sdc`: `set_false_path -from ... -to [get_cells {<ilk aşama>_reg*}]`; `.xdc`: `set_max_delay -datapath_only <T_kaynak> -from ... -to ...` |
| `AsyncFifo` gray işaretçiler | `.sdc`: `set_max_delay -ignore_clock_latency <T_kaynak>`; `.xdc`: `set_max_delay -datapath_only <T_kaynak>` + `set_bus_skew <T_kaynak>` |
| `AsyncFifo` / `AsyncDualPortRam` belleği, `HandshakeSync` verisi | `set_false_path` (iki lehçe) |
| ham reset portu `in r : reset(...)` | `set_false_path -from [get_ports r]`; zincir `rst_sync_<saat>_stage<i>` yorumla listelenir |
| XDC, her senkronizör ve reset zinciri | `set_property ASYNC_REG TRUE [get_cells {..._reg*}]` |
| `@timing(clk = F)` / `@timing(clk >= F)` | alan frekansı yoksa `create_clock` kaynağı; varsa tutarlılık denetimi (E0017) |
| `@timing(max_delay(a, b) <= 5.ns)` / `min_delay(a, b) >= 1.ns` | `set_max_delay 5.000 -from ... -to ...` / `set_min_delay` |
| `@false_path(from = a, to = b)` (modül, port, `reg`) | `set_false_path -from ... -to ...` |
| `@multicycle(from = a, to = b, cycles = N)` / `reg` üstünde `@multicycle(N)` | `set_multicycle_path N -setup` + `N-1 -hold` |

Kurallar:

- Dosya başlığı `# Generated by Volt <sürüm>` / `# Source: <dosya>` /
  `# DO NOT EDIT` / `# Module: <Modül>`; çıktı deterministiktir.
- Frekansı bildirilmemiş alan **W0022** (alan başına bir kez, yalnız
  `--emit=sdc,xdc` istendiğinde) — `create_clock` üretilmez, yorum satırı
  yazılır; o saat `set_clock_groups`'a girmez. `T_kaynak` geçişin kaynak
  saatinin periyodudur (AsyncFifo okuma işaretçisi ve HandshakeSync ack
  için hedef saat); kaynak saatin frekansı yoksa `set_false_path` yazılır
  ve bir yorum satırı nedenini söyler; kaynak saat yoksa (alansız giriş
  portu) `set_false_path` doğru kısıttır.
- Otomatik reset portu (`rst`/`rst_n`) hiçbir kısıt almaz; zincir çıkışından
  register temizleme pinlerine giden yollar hiçbir stilde kapatılmaz.
- Tek alanlı modülün çıktısı iki stilde aynıdır (yalnız `# Style:` satırı).
- Desteklenmeyen nitelik biçimi ya da alan frekansıyla çelişki **E0017**
  (her zaman, `volt check` dâhil); hata varsa kısıt dosyası yazılmaz.
- Yollar JSON `artifacts` listesine RTL ve SVA'dan sonra girer; saat
  portlu modül yoksa `Note: no module with a clock port in the unit;
  --emit=sdc produced nothing`, çıkış 0.
- Register hücre adları Vivado / Design Compiler geleneğiyle
  `get_cells {ad_reg*}`; Quartus kullanıcısı deseni uyarlar.

### İnsan Çıktısı — Başarı

```
$ volt build counter.volt
   Derleniyor counter.volt
    Tamamlandı 0.12s
     Çıktı build/rtl/counter.sv (34 satır)
```

### İnsan Çıktısı — Hata

```
$ volt build design.volt
   Derleniyor design.volt

error[E3001]: iki farklı saat alanı doğrudan bağlanamaz
  ┌─ design.volt:12:14
   │
12 │     result = data
   │              ^^^^ 'data' → fast_clk alanında (satır 4)
   │     ^^^^^^ 'result' → slow_clk alanında (satır 5)
   │
   = neden: sinyal kararsız bir anda yakalanabilir
   = çözüm: result = sync(data, slow_clk)
   = daha fazla: volt explain E3001

warning[W1001]: kullanılmayan sinyal: 'temp'
  ┌─ design.volt:8:9
   │
 8 │     let temp = a + b
   │         ^^^^
   │
   = çözüm: '_temp' olarak yeniden adlandırın

     Hata: 1 hata, 1 uyarı nedeniyle derleme başarısız
```

Çıkış kodu: 1

### JSON Çıktısı

```bash
volt build --format=json design.volt
```

```json
{
  "version": "1",
  "command": "build",
  "success": false,
  "diagnostics": [
    {
      "code": "E3001",
      "severity": "error",
      "message": "iki farklı saat alanı doğrudan bağlanamaz",
      "spans": [
        {
          "file": "design.volt",
          "start": { "line": 12, "col": 14, "byte": 245 },
          "end":   { "line": 12, "col": 18, "byte": 249 },
          "label": "'data' → fast_clk alanında",
          "primary": true
        },
        {
          "file": "design.volt",
          "start": { "line": 12, "col": 5, "byte": 236 },
          "end":   { "line": 12, "col": 11, "byte": 242 },
          "label": "'result' → slow_clk alanında",
          "primary": false
        }
      ],
      "notes": [
        { "kind": "reason", "text": "sinyal kararsız bir anda yakalanabilir" }
      ],
      "help": "result = sync(data, slow_clk)",
      "suggestions": [
        {
          "span": {
            "file": "design.volt",
            "start": { "line": 12, "col": 14, "byte": 245 },
            "end":   { "line": 12, "col": 18, "byte": 249 }
          },
          "replacement": "sync(data, slow_clk)",
          "applicability": "machine-applicable"
        }
      ],
      "explain_url": "https://volthdl.org/errors/E3001"
    }
  ],
  "summary": { "errors": 1, "warnings": 1 },
  "artifacts": [],
  "duration_ms": 118
}
```

**`applicability` değerleri:**
```
machine-applicable  → otomatik uygulanabilir (LSP quick-fix)
maybe-incorrect     → öneri, kullanıcı kontrol etmeli
has-placeholders    → şablon, doldurulması gerekiyor
unspecified         → sadece bilgi
```

### Short Format

CI logları için tek satır:

```bash
volt build --format=short design.volt
```
```
design.volt:12:14: error[E3001]: iki farklı saat alanı doğrudan bağlanamaz
design.volt:8:9: warning[W1001]: kullanılmayan sinyal: 'temp'
```

---

## 6. `volt check`

Çıktı üretmeden sadece doğrulama. `build`'den hızlı.

```bash
$ volt check design.volt
    Kontrol design.volt
    Tamamlandı 0.08s
       Sonuç 0 hata, 2 uyarı

  todo! bulundu (2):
    design.volt:14  "LRU mu FIFO mu?"       [tip: bits<3>]
    design.volt:22  "politika belirlenecek" [kontrat]
```

**`todo!` listesi burada gösteriliyor** — `build --release`
bunları hata sayıyor (E9001).

## 6a. `volt check-regmap` — sürücü ↔ register haritası (ADR-0063)

> ADR-0063 (uygulandı). Daha önce Volt'un ürettiği bir sürücü dosyasını
> (bayat olabilir, elle düzenlenmiş olabilir) tasarımın BUGÜNKÜ `@mmio`
> haritasıyla karşılaştırır. Firmware deposundaki başlığın CI denetimi
> içindir.

```bash
volt check-regmap [SEÇENEKLER] <DOSYA.volt> --against <sürücü> [--against <sürücü> ...]
```

```
SEÇENEKLER:
    --against <dosya>   .h (--emit=c) | .rs (--emit=rust) | .json (--emit=regmap);
                        tekrarlanabilir, en az bir tane zorunlu
    --format <f>        human | json | short
```

Karşılaştırılanlar: taban adres; register adı, offset, erişim, bit
maskesi, reset değeri; alan adı, bit konumu, genişlik/maske, davranış
(`normal` | `W1C` | `self-clearing`); eksik (RTL'de var, dosyada yok) ve
fazla (dosyada var, RTL'de yok) register/alan. Erişim erişimci
fonksiyonlarının varlığından okunur. **Yorum, boşluk ve bildirim sırası
ayrışma değildir.** Aynı Volt sürümünün ürettiği dosyada erişimci kodu
da yorumsuz belirteç belirteç karşılaştırılır.

Önce içerikten yeniden hesaplanan `regmap-hash`'e bakılır: tasarımınkine
eşitse ayrıntılı fark listesi atlanır (hızlı yol). Beyan edilen hash
yalnız nedeni sınıflandırır (bayat / üretildikten sonra düzenlenmiş).

```
$ volt check-regmap gpio.volt --against firmware/gpio.h
    Checking gpio.volt against 1 file(s)
error[E9003]: register map drift in firmware/gpio.h
  ┌─ firmware/gpio.h:2:1
  │
2 │ // regmap-hash: 5a1c09e2b7d34f60
  │ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ the design's regmap-hash is 0c9e41d7a2b3f815
  │
  = note: GPIO_DIR offset: file 0x0C, RTL 0x04
  = note: missing in file: GPIO_DATA_IN (0x08)
  = reason: the file is stale: it was generated from an older register map (its regmap-hash matches its own content)
  = help: regenerate with volt build --emit=c gpio.volt
  = for more: volt explain E9003

    Finished 0.01s
      Result 0 of 1 file(s) match the register map
```

Uyumlu dosya için insan biçiminde `Match <yol> (regmap-hash <h>, hash match)`.

Volt'un üretmediği dosya (uzantı `.h`/`.rs`/`.json` değil, imza yok,
imzadan önceki Volt sürümü, imzalı ama zorunlu bildirimi silinmiş) sessizce
geçmez: **E9004**. Elle yazılmış başlıklar ayrıştırılmaz.

Çıkış kodları (§2):

| Kod | Durum |
|---|---|
| 0 | Tüm `--against` dosyaları uyumlu |
| 1 | En az bir E9003/E9004, ya da tasarımda derleme hatası |
| 2 | `--against` verilmedi, ya da tasarımda `@mmio` modülü yok |
| 3 | Tasarım ya da `--against` dosyası okunamadı |

`--format=json`: §5 zarfı (`"command": "check-regmap"`) + `regmap_check`:

```json
"regmap_check": {
  "design_hashes": { "Gpio": "0c9e41d7a2b3f815" },
  "files": [
    { "path": "firmware/gpio.h", "format": "c", "status": "drift",
      "module": "GPIO", "declared_hash": "5a1c09e2b7d34f60",
      "content_hash": "5a1c09e2b7d34f60", "design_hash": "0c9e41d7a2b3f815",
      "generator_version": "0.1.0", "fast_path": false, "code_compared": false,
      "cause": "stale",
      "drift": [ { "kind": "offset", "subject": "GPIO_DIR", "file": "0x0C", "rtl": "0x04" } ] }
  ]
}
```

- `status`: `match | drift | unsupported` (`unsupported` → `reason`:
  `extension | not_volt | old_volt | malformed`).
- `drift[].kind`: `module | base | offset | access | mask | reset |
  missing_register | extra_register | missing_field | extra_field |
  field_lsb | field_mask | field_access | inconsistent | code`.
- `cause`: `stale | edited | stale_and_edited | null`.

---

## 7. `volt run`

```bash
$ volt run counter.volt
   Derleniyor counter.volt
   Simüle ediliyor (100 döngü)

   döngü  enable  count
   -----  ------  -----
       0       0      0
       1       1      1
       2       1      2
       ...

    Tamamlandı 0.34s
```

```
SEÇENEKLER:
    --cycles=<N>      Simülasyon döngü sayısı (varsayılan: 100)
    --vcd=<dosya>     Dalga formu kaydet
    --top=<modül>     Üst modül (varsayılan: tek modül)
```

---

## 8. `volt test`

```bash
$ volt test
   Derleniyor 12 test dosyası

running 39 tests
test ui::pass::01_minimal_module ... ok
test ui::pass::02_register_basic ... ok
test ui::fail::01_cdc_violation ... ok
test ui::fail::02_width_mismatch ... FAILED

failures:

---- ui::fail::02_width_mismatch ----
  beklenen: E2001
  alınan:   E2003
  konum:    tests/ui/fail/02_width_mismatch.volt:10

test result: FAILED. 38 passed; 1 failed
```

Çıkış kodu: 5

```
SEÇENEKLER:
    --filter=<desen>   Sadece eşleşen testler
    --update-snapshots Snapshot güncelle (dikkatli!)
    --nocapture        Test çıktısını göster
```

---

## 8a. `volt verify` — paralel formal doğrulama (ADR-0055)

> ADR-0055 (uygulandı). Birimdeki kontratlı her modül bir SymbiYosys
> GÖREVİDİR: tek `build/formal/<iş>.sby` dosyası `[tasks]` bölümüyle
> üretilir (`<iş>` = girdi dosyasının kök adı), tek `sby -j N -f <iş>.sby`
> süreci görevleri kendi görev döngüsünde paralel koşturur. Paralellik
> birimi MODÜLDÜR, kontrat değil: bir modülün tüm kontratları tek BMC
> koşusunda birlikte denetlenir (gerekçe ve ölçüm ADR-0055).

```bash
$ volt verify -j 8 examples/soc/top.volt
   Verifying examples/soc/top.volt
     [1/8] UartTx (7 properties) ... ok (0.40s)
     [2/8] BusDecoder (24 properties) ... ok (0.78s)
     ...
     [8/8] SocTop (19 properties) ... ok (10.87s)
    Finished 12.25s
      Result 137 properties verified in 12.3s (8 jobs; bmc, depth 12)
```

```
SEÇENEKLER:
    -j, --jobs=<N|auto>  Paralel sby işi (varsayılan auto = CPU sayısı; 1 = sıralı)
        --fail-fast      İlk karşı örnekte dur (varsayılan: her görev tamamlanır)
        --mode=<m>       bmc | prove | cover           (varsayılan: bmc)
        --depth=<N>      Arama derinliği               (varsayılan: 20)
        --engine=<e>     z3 | boolector | yices        (varsayılan: z3)
```

Çıktı dizini (`--target-dir`, §4):

```
build/formal/
├── <iş>.sv               tüm birimin SV'si (SVA gömülü, tek dosya)
├── <iş>.sby              [tasks] = kontratlı modüller, kaynak sırasında
├── <iş>_<görev>/         sby çalışma dizini (görev = küçük harf modül adı)
└── <görev>_cex.vcd       karşı örnek izi (yalnız FAIL'de)
```

Tek görevi elle yinelemek: `sby -f <iş>.sby <görev>` (`build/formal/` içinde).

### İlerleme ve determinizm

- İlerleme satırları `[ k/N] <Modül> (<n> properties) ... ok|FAIL|error (<süre>)`
  TAMAMLANMA sırasında akar; `k` tamamlanan görev sayısıdır. `-j 1` ile
  sıra kaynak sırasına eşittir.
- RAPOR her zaman KAYNAK SIRASINDADIR: E5001 tanıları, `Failures:`
  listesi, JSON `modules`/`properties`. Tamamlanma sırası raporu
  değiştirmez; aynı girdi → aynı rapor (yalnız süre alanları değişir).
  `-j 1` ve `-j N` aynı sonucu verir (testle doğrulanır).

### Hata durumu

- Bir modülün karşı örneği DİĞER görevleri durdurmaz; tüm başarısızlıklar
  sonda kaynak sırasında listelenir; çıkış kodu 6.
- `--fail-fast`: ilk `DONE (FAIL)` satırında sby süreci sonlandırılır,
  bitmemiş görevler `skipped` olur; çıkış kodu 6.
- Görev `DONE` satırı basmadan biterse araç hatası (çıkış 3; karşı örnek
  de varsa 6 baskındır) ve `sby -f <iş>.sby <görev>` ipucu yazılır.
- `-j 0` ya da sayı/`auto` dışı değer kullanım hatasıdır (çıkış 2).

### JSON (`--format=json`)

§5 zarfına `verify` nesnesi eklenir:

```json
"verify": {
  "mode": "bmc", "depth": 12, "engine": "z3", "jobs": 8, "fail_fast": false,
  "modules": [
    { "module": "SocTop", "task": "soctop", "status": "pass",
      "properties": 19, "duration_ms": 11614 }
  ],
  "properties": [
    { "module": "SocTop", "name": "inv_0", "keyword": "invariant",
      "status": "pass", "duration_ms": 11614 }
  ]
}
```

- `modules[].status`: `pass | fail | error | skipped`.
- `properties[].status`: `pass | fail | unproven | error | skipped`;
  `unproven` = aynı modülde başka bir kontrat ihlal edildi, BMC o döngüde
  durdu (bu kontrat o döngüden sonra denetlenmedi).
- `duration_ms`: kontratın ait olduğu GÖREVİN süresi (modülün kontratları
  tek koşuda birlikte kanıtlanır); `skipped`/`error` görevlerde `null`.

---

## 9. `volt explain`

UX Anayasası'nın "= daha fazla" satırının hedefi:

```bash
$ volt explain E3001
```

```
E3001: Saat Alanı Geçişi (CDC) İhlali

İki farklı saat alanındaki sinyaller doğrudan bağlanamaz.

NEDEN SORUN

Hedef flip-flop, kaynak sinyali kurulum (setup) veya tutma
(hold) penceresi içinde yakalarsa metastabil duruma girer.
Çıkış bir süre kararsız kalır ve sonra rastgele 0 veya 1'e
yerleşir.

Bu hata Verilog'da sessizce derlenir ve genellikle silisyumda
ortaya çıkar — hata ayıklaması en pahalı noktada.

ÖRNEK

  domain Fast { clock = posedge }
  domain Slow { clock = posedge }

  module Bad {
      in  data   : u8 @Fast
      out result : u8 @Slow

      result = data          // ✗ E3001
  }

ÇÖZÜM

  result = sync(data, slow_clk)    // ✓ iki flip-flop

ÇOK BİTLİ VERİ

sync() her biti bağımsız senkronize eder. 8-bit veri için
bitler farklı saat kenarlarında yakalanabilir:

  0b11111111 → 0b11110000 (geçersiz ara değer)

Çok bitli veri için:
  - Gray kodlama (sayaçlar için)
  - AsyncFifo (veri akışı için)
  - Handshake protokolü (kontrol için)

DAHA FAZLA
  https://volthdl.org/errors/E3001
  https://volthdl.org/guide/cdc
```

---

## 10. Ortam Değişkenleri

```
VOLT_LOG=<seviye>       error | warn | info | debug | trace
VOLT_BACKTRACE=1        Panik durumunda yığın izi
VOLT_TARGET_DIR=<yol>   Varsayılan çıktı dizini
VOLT_COLOR=<mod>        --color varsayılanı
NO_COLOR=1              Renk kapalı (standart)
CI=true                 CI modu: renksiz, ilerleme çubuğu yok
```

---

## 11. stdout / stderr Ayrımı

```
stdout:
  Üretilen veri (--emit=json-ast çıktısı)
  Test sonuç özeti
  volt explain metni
  --out=- ile SV çıktısı

stderr:
  Tanılar (hata + uyarı)
  İlerleme mesajları
  "Derleniyor...", "Tamamlandı"

Neden:
  volt build --emit=json-ast x.volt | jq '.items'
  → Tanılar boruyu kirletmiyor
```

---

## 12. İç Hata (Panic) Davranışı

```
thread 'main' panicked at crates/volt-hir/src/ty.rs:142

Volt'ta bir iç hata oluştu. Bu bir derleyici hatasıdır.

Lütfen bildirin: https://github.com/volthdl/volt/issues/new

Bilgi:
  volt sürümü: 0.1.0 (a3f2b1c 2026-08-31)
  platform:    x86_64-unknown-linux-gnu
  komut:       volt build design.volt
  girdi:       design.volt (1.2 KB)

Yığın izi için: VOLT_BACKTRACE=1 ile tekrar çalıştırın
```

Çıkış kodu: 101

---

## 13. CI Entegrasyonu

```yaml
# .github/workflows/ci.yml
- name: Volt kontrol
  run: |
    volt check --format=short --deny-warnings

- name: Volt derleme
  run: |
    volt build --release
    verilator --lint-only -Wall build/rtl/*.sv

- name: Volt testleri
  run: |
    volt test --format=json > test-results.json
```

**`--deny-warnings`:** Uyarıları hata sayar, çıkış kodu 1 olur.

---

## 14. Yardım Metni

```
$ volt --help

Volt HDL — saat alanı güvenli donanım tanımlama dili

KULLANIM:
    volt <KOMUT> [SEÇENEKLER]

KOMUTLAR:
    new       Yeni proje oluştur
    build     Derle ve SystemVerilog üret
    check     Hızlı kontrol (çıktı üretmez)
    run       Derle ve simüle et
    test      Testleri çalıştır
    fmt       Kaynak kodu biçimlendir
    explain   Hata kodunu açıkla

SEÇENEKLER:
    -h, --help          Bu yardımı göster
    -V, --version       Sürüm bilgisi
    -v, --verbose       Ayrıntılı çıktı
    -q, --quiet         Sadece hatalar
        --format=<f>    human | json | short
        --color=<c>     auto | always | never

ÖRNEKLER:
    volt new my_design
    volt build counter.volt
    volt run counter.volt --cycles=50
    volt explain E3001

Daha fazla: https://volthdl.org
```

---

## 15. Uygulama Sırası

```
F0 (2 gün):
  build, check komutları
  Çıkış kodları 0/1/2/3
  --format=human ve --format=json
  --color desteği

F1 (1 gün):
  explain komutu (5 hata kodu ile başla)
  --verbose / --quiet

F4 (2 gün):
  run, test, sim komutları
  Çıkış kodu 5, 6

F5 (2 gün):
  fmt, doc, new
  Tam yardım metinleri
```

---

## 16. Test Vektörleri

```
KOMUT                                  ÇIKIŞ  ÇIKTI
──────────────────────────────────────────────────────────
volt build counter.volt                0      counter.sv
volt build cdc_violation.volt          1      E3001
volt build --bad-flag                  2      kullanım hatası
volt build yok.volt                    3      dosya yok
volt build (Volt.toml bozuk)           4      config hatası
volt test (1 test kaldı)               5      test raporu
volt verify (karşı örnek)              6      counter-example
volt verify -j 0 x.volt                2      kullanım hatası (ADR-0055)
volt check --deny-warnings (uyarı var) 1      uyarı → hata
volt explain E3001                     0      açıklama metni
volt explain E9999                     2      bilinmeyen kod
──────────────────────────────────────────────────────────
volt build --format=json | jq .success  → false
volt build --format=short               → tek satır/tanı
NO_COLOR=1 volt build                   → ANSI kodu yok
```

---

## 17. Ek Hata Kodları (Bütçe, Sürüm, Yapı)

> Taşındı: Bu kodlar önceden `docs/design/Volt-Dil-Spesifikasyonu-v3.md`
> içinde tanımlıydı; bağlayıcı tanım artık burasıdır.

```
E5xxx  Davranışsal Kontratlar
  E5001  Kontrat ihlali — formal doğrulama karşı örnek buldu (çıkış kodu 6)
  E5004  Kontrat ifadesi Bool değil

E6xxx  Bütçe ve Zamanlama
  E6001  Kaynak bütçesi aşıldı
  E6003  @false_path kanıtlanamadı (yol gerçekten var)
  E6004  @multicycle pipeline derinliğiyle uyuşmuyor

E7xxx  Sürüm
  E7001  SemVer ihlali: kırıcı değişiklik ama MAJOR bump yok
  E7002  abi_version değişmeden arayüz değişti

E9xxx  Yapı
  E9001  todo! ile release build yapılamaz
  E9002  Determinizm ihlali
  E9003  Sürücü dosyası ile RTL arasında register haritası ayrışması (ADR-0063, §6a)
  E9004  Volt'un üretmediği register haritası dosyası (ADR-0063, §6a)
```
