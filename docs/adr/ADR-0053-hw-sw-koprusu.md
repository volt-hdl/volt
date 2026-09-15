# ADR-0053: HW-SW Köprüsü — `@mmio` Haritasından Sürücü, Başlık, `regmap.json` ve Belge Üretimi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-15
> Etkilenen: volt-ast (`mmio.rs` YENİ: `RegMap`/`RegDesc`/`FieldDesc`;
> `MmioFieldDecl.doc`), volt-syntax (item.rs alan doc yorumu; mmio.rs
> `RegInfo.doc/offset`, `FieldInfo.doc`, `regmap()` kopyası;
> `ParseResult.regmaps`), volt-sw-emit (YENİ crate: rust.rs, c.rs,
> json.rs, markdown.rs, names.rs), volt-driver (`--emit=rust,c,regmap,
> regmap-md`, `Compiled.regmaps`, `write_sw_outputs`), docs/spec/
> cli-contract.md §4/§5 (bu ADR kaynaklı), examples/soc/README.md,
> tests/ui/pass/72, crate testleri volt-syntax/tests/regmap_tests.rs,
> volt-sw-emit/tests/generators_tests.rs, volt-driver/tests/sw_emit_tests.rs.
> DOKUNULMADI: crates/volt-sv-emit, crates/volt-hir, README.md.

## Sorun

ADR-0044 `@mmio` register haritasını RTL'e çevirdi: bus adaptörü,
adres çözümleme, okuma çoklayıcısı, erişim denetimi. Ama bir SoC
projesinde aynı harita ÜÇ ayrı yerde yaşar: RTL, C/Rust sürücüsü ve
belge. Üçü elle senkron tutulur; bir register'ın offset'i RTL'de
kayınca sürücü eski adrese yazmaya devam eder, belge üçüncü bir
değeri gösterir. "Adres uyuşmazlığı" bu yüzden klasik bir hata
sınıfıdır ve simülasyon onu görmez (sürücü simüle edilmez).

Volt'ta bilgi ZATEN vardır: `@reg(offset, access, volatile)` ve
alanların tipleri/nitelikleri parser'da `RegInfo` olarak durur. Eksik
olan yalnız farklı çıktılardır.

### Durum tespiti (bu ADR'nin 1. adımı)

`@mmio` bilgisinin derleyicideki yolculuğu:

| Bilgi | Nerede? | Sürücü için yeterli mi? |
|---|---|---|
| `base`, `bus` | parser `mmio_header()` → `render()` argümanı | evet |
| Register adı, `offset`, `access` | `RegInfo { name, addr = base+offset, access }` | evet (offset `addr - base`) |
| `volatile` | `RegInfo.owner == Owner::Hardware` | evet (türetilir) |
| Alan adı, `lsb`, genişlik, tip ailesi | `FieldInfo { name, lo, ty: Bool/Bits/UInt }` | evet |
| `@reserved` | `FieldInfo.name == None` | evet |
| `@self_clearing`, `@w1c` | `FieldInfo` bayrakları | evet |
| Register doc yorumu | `MmioRegDecl.doc` (AST'de vardı, `RegInfo`'ya kopyalanmıyordu) | kopyalandı |
| Alan doc yorumu | YOKTU — `MmioFieldDecl`'de `doc` alanı yok, parser `///`'ı alan önünde kabul etmiyordu | eklendi |
| Sıfırlama / reset değeri | yok (ADR-0044: hep 0) | belgeye "reads 0" olarak yansır; sabit kimlik sözcüğü hâlâ ertelenmiş |

Kritik bulgu: ADR-0044 "silme ilkesi" gereği `ModuleDecl.mmio_regs`
desugar sonunda BOŞALIR ve HIR'ın hiçbir geçidi (resolve, typeck,
domain, sv-emit) haritayı görmez. Yani bilgi HIR'da DURMAZ; yalnız
parser'ın `desugar_mmio` çağrısı sırasında geçici `RegInfo` listesinde
yaşar. Sürücü üretimi için iki seçenek vardı: (a) HIR'a bir yan tablo
eklemek, (b) parser'ın ürettiği listeyi dil bağımsız bir özete kopyalayıp
`ParseResult` ile dışarı vermek. (b) seçildi: yazılım çıktıları anlamsal
analiz gerektirmez, HIR'a dokunmak silme ilkesini bozardı ve LSP aynı
`ParseResult`'ı kullandığından harita editörde de erişilebilir olur.

## Karar

### 1. Veri modeli: `volt_ast::mmio::RegMap`

```rust
pub struct RegMap { module, base: u64, bus, doc: Option<String>, registers: Vec<RegDesc> }
pub struct RegDesc { name, offset: u64, access: RegAccess, volatile: bool, doc, fields: Vec<FieldDesc> }
pub struct FieldDesc { name /* @reserved → "_reserved" */, lsb, width, kind: Bool|Bits|UInt,
                       reserved, self_clearing, w1c, doc }
```

Saf veridir: span, AST indeksi ya da `mmio_` sentetik ad taşımaz.
Parser `desugar_mmio` içinde, RTL metnini üreten `render()` ile AYNI
`RegInfo` listesinden `regmap()` ile kurar ve `ParseResult.regmaps`'e
ekler (`parse_unit` birimdeki tüm haritaları toplar). Hatalı (E0009/
E0015) register düşer; hem RTL hem harita onsuz üretilir — ikisi
birbirinden ayrılamaz. Yerleşim tamamen hatalıysa (`parse_generated`
başarısız) harita da üretilmez.

Alan doc yorumu: `MmioFieldDecl.doc` eklendi; `parse_mmio_reg` alan
niteliklerinden ÖNCE `collect_doc_comments()` çağırır (`/// açıklama`
satırları `pins : bits<8>` üstünde). Modül doc'u `Item.doc`'tan gelir.

### 2. CLI: `volt build --emit=rust,c,regmap,regmap-md`

| `--emit` | Dosya | İçerik |
|---|---|---|
| `rust` | `build/sw/<modül>.rs` | `no_std` Rust sürücüsü |
| `c` | `build/sw/<modül>.h` | C başlığı (`#define` + `static inline`) |
| `regmap` | `build/sw/<modül>.json` | `volt-regmap/1` şeması |
| `regmap-md` | `build/docs/<modül>.md` | register haritası tabloları |

`<modül>` modül adının snake_case halidir (`GpioRegs` → `gpio_regs`;
Rust modül adı ve C makro öneki ile tutarlı). Değerler virgülle
birleşir, yinelenen tür bir kez yazılır, `sva` ile birlikte kullanılır.
Yazılan yollar JSON zarfının `artifacts` listesine RTL ve SVA'dan sonra
girer; insan biçiminde `Output` satırı olarak görünür. Birimde `@mmio`
modülü yoksa dosya üretilmez, insan biçiminde bir `Note` düşer (hata
DEĞİL: sürücüsü olmayan tasarım geçerlidir). Derleme hatası varsa RTL
gibi yazılım çıktısı da üretilmez. Üretim `crates/volt-sw-emit`'tedir:
saf `RegMap → String` fonksiyonları, dosya sistemi sürücüde.

### 3. Rust sürücüsü (`--emit=rust`)

```rust
#[derive(Debug)]
pub struct Gpio { base: *mut u32 }
impl Gpio {
    pub const BASE: usize = 0x0000_0000;
    pub const DIR_OFFSET: usize = 0x04;      // her register
    pub const DIR_MASK: u32 = 0x0000_00FF;   // adlandırılmış alan bitleri
    /// # Safety ...
    pub const unsafe fn new(base: *mut u32) -> Self
    pub unsafe fn at_default_base() -> Self  // without_provenance_mut(BASE)
    pub fn dir_raw(&self) -> u32             // okunabilir register: sözcük & MASK
    pub fn set_dir_raw(&mut self, word: u32) // yazılabilir register: sözcük & MASK
    pub fn dir(&self) -> u8                  // alan getter (readable)
    pub fn set_dir(&mut self, pins: u8)      // alan setter (writable)
    pub fn trigger_control_reset(&mut self)  // @self_clearing
    pub fn clear_status_expired(&mut self)   // @w1c
}
```

Kurallar:

- `no_std`: yalnız `core::ptr::{read_volatile, write_volatile}`;
  `unsafe` iki özel yardımcıda (`read`/`write`, `base.byte_add(offset)`)
  toplanır, güvenlik sözleşmesi `new`'in `# Safety` bölümündedir.
- **Erişim hakkı derleme zamanında**: ReadOnly register'a setter,
  WriteOnly register'a getter ÜRETİLMEZ — yanlış yönde erişim Rust'ta
  "method not found" hatasıdır, çalışma zamanında sessiz bir SLVERR
  değil.
- Alan adı: tek adlandırılmış alanı olan register'da alan adı düşer
  (`direction()`, `set_direction(pins)`); çok alanlıda `reg_field`
  (`control_enable()`, `trigger_control_reset()`). Tip en dar tam sayı:
  `bool`, ≤8 → `u8`, ≤16 → `u16`, aksi `u32`.
- Bit alanları shift+mask ile: getter `((word >> lsb) & mask) as T`;
  setter okuma-değiştirme-yazma. RMW'de korunan bitler = adlandırılmış
  alanlar − bu alan − `@self_clearing`/`@w1c` bitleri (bir alanı
  yazarken darbe alanı yeniden tetiklenmez); korunacak bit yoksa okuma
  atlanır. WriteOnly register geri okunamadığından setter diğer alanlara
  0 yazar (yorumla belirtilir).
- `@reserved` bitler okumada maskelenir (`MASK` sabiti), yazmada 0.
- `@self_clearing` → `trigger_*` (RMW + bit), `@w1c` → `clear_*`
  (yalnız o bit yazılır; register ReadOnly+volatile olduğundan başka
  alanı etkilemez).
- Volt doc yorumları `///` olarak modül → struct, register → raw
  erişimciler, alan → getter/setter üstüne aktarılır. Getter'lar
  `#[must_use]`, yardımcılar `#[inline]`. Çıktı `clippy::pedantic`
  altında temizdir (doğrulandı).

### 4. C başlığı (`--emit=c`)

```c
#ifndef GPIO_H
#define GPIO_H
#include <stdint.h>
#include <stdbool.h>
#define GPIO_BASE                 0x00000000U
#define GPIO_DIR                  (GPIO_BASE + 0x04U)
#define GPIO_DIR_OFFSET           0x04U
#define GPIO_DIR_MASK             0x000000FFU
#define GPIO_DIR_PINS_SHIFT       0U
#define GPIO_DIR_PINS_MASK        0xFFU
static inline uint32_t gpio_dir_read(void);
static inline void     gpio_dir_write(uint32_t word);
static inline uint8_t  gpio_get_dir_pins(void);
static inline void     gpio_set_dir_pins(uint8_t pins);
/* @self_clearing → <mod>_trigger_<reg>_<alan>(void); @w1c → <mod>_clear_<reg>_<alan>(void) */
#endif
```

Taban adres derleme zamanı sabitidir (`<MOD>_BASE`); her erişim
`*(volatile uint32_t *)ADRES`. C tarafında adlar HER ZAMAN
`<mod>_<get|set>_<reg>_<alan>` biçimindedir (makro dilinde kısaltma
karışıklık yaratır). ReadOnly alana setter, WriteOnly alana getter
üretilmez. `extern "C"` koruması C++'tan kullanıma izin verir. Çıktı
`gcc -std=c99 -Wall -Wextra -Werror -pedantic -fsyntax-only`, C11 ve
`g++ -std=c++17` ile temizdir (gcc 16.2, Docker).

### 5. `regmap.json` şeması — `volt-regmap/1`

```json
{
  "schema": "volt-regmap/1",
  "generator": "volt 0.1.0",
  "source": "gpio.volt",
  "name": "Gpio",
  "base": 0,
  "bus": "AXI4Lite",
  "doc": "8-bit GPIO ..." | null,
  "registers": [
    {
      "name": "dir", "offset": 4, "address": 4,
      "access": "rw" | "ro" | "wo", "volatile": false, "doc": "..." | null,
      "fields": [
        { "name": "pins", "lsb": 0, "width": 8, "type": "uint" | "bits" | "bool",
          "reserved": false, "self_clearing": false, "w1c": false, "doc": null },
        { "name": "_reserved", "lsb": 8, "width": 24, "type": "bits", "reserved": true, ... }
      ]
    }
  ]
}
```

| Anahtar | Tip | Anlam |
|---|---|---|
| `schema` | string | `volt-regmap/1`; uyumsuz değişiklikte sayı artar |
| `generator`, `source` | string | üretici sürümü, ana kaynak dosya adı |
| `name`, `base`, `bus` | string, u64, string | modül adı, taban adres (ondalık), bus |
| `doc` | string \| null | doc yorumu, satırlar `\n` |
| `registers[].offset` / `address` | u64 | bayt offset'i / `base + offset` |
| `registers[].access` | `rw`/`ro`/`wo` | ADR-0044 erişim türü |
| `registers[].volatile` | bool | donanım günceller (yalnız `ro`) |
| `fields[].lsb`, `width` | u32 | alanlar ardışık, toplam ≤ 32 |
| `fields[].type` | `bool`/`bits`/`uint` | Volt tipi ailesi |
| `fields[].reserved` | bool | `@reserved` (ad `_reserved`) |
| `fields[].self_clearing`, `w1c` | bool | alan nitelikleri |

Sıra bildirim sırasıdır; anahtarlar alfabetik yazılır (serde_json
varsayılanı, deterministik). Sayılar ondalıktır (JSON hex bilmez).

### 6. Markdown belge (`--emit=regmap-md`)

`# <Modül> register map`, modül doc'u, taban/bus/kaynak satırları,
`| Offset | Name | Access | Description |` özet tablosu (Access:
`RW`/`RO`/`WO` + `, volatile`), sonra her register için
`### <reg> — offset, address, access` başlığı, register doc'u ve
`| Bits | Field | Type | Attributes | Description |` tablosu — bitler
yüksekten düşüğe (donanım belgesi alışkanlığı), rezerve satırı "Reads
as 0; write 0.", nitelikler `reserved`/`self-clearing`/`w1c`.
Açıklama sütunu doc'un ilk satırıdır, `|` kaçırılır.

### 7. Tutarlılık testi (KRİTİK)

Sürücü ile RTL aynı `RegInfo`'dan türese de, `volt-driver/tests/
sw_emit_tests.rs::regmap_offsets_match_rtl_address_decode_for_every_design`
bunu BAĞIMSIZ yoldan doğrular: üç tasarımı (`examples/soc/gpio.volt`,
`ui/pass/58`, `ui/pass/72`) `--emit=regmap` ile derler, üretilen
`<Modül>.sv`'yi metin olarak okur ve:

1. `wire mmio_whit = aw_addr == 32'hA || ...` / `mmio_rhit` satırlarındaki
   adres kümesi = JSON'daki `{base + offset}` kümesi (tam eşitlik —
   fazla ya da eksik adres yok);
2. `case (ar_addr)` kolları = okunabilir (`rw`/`ro`) register adresleri;
3. `case (aw_addr)` kolları = bus'ın yazdığı (`rw`/`wo`) register adresleri;
4. bus'a ait RW register için `mmio_<reg>_rd = mmio_<reg> & 32'hM`
   maskesi = JSON alanlarından hesaplanan rezerve-dışı maske;
5. her `@w1c` alanının biti okuma görünümü satırında literal olarak var.

`rtl_consistency_check_detects_a_shifted_offset` denetimin kendisini
sınar: JSON'da bir adres kaydırılınca fonksiyon panik etmelidir.
`rust_constants_match_regmap_json` Rust/C sabitlerini (`BASE`,
`*_OFFSET`, `*_SHIFT`) JSON'a karşı doğrular. Böylece "sürücü ile RTL
ayrıştı" hatası üç katmanda yakalanır: aynı kaynaktan üretim (tasarım),
SV metni ile karşılaştırma (test), sabitlerin çapraz denetimi (test).

### 8. Derlenebilirlik testleri

- `generated_rust_drivers_pass_cargo_check_in_a_no_std_crate`: üç
  sürücüyü geçici `#![no_std] #![deny(warnings)]` crate'ine koyar,
  `cargo check --offline` + `RUSTFLAGS=-D warnings` koşar (yerelde ve
  CI'da gerçek).
- `generated_c_headers_pass_c_compiler_syntax_check`: `CC`/gcc/cc/clang
  bulunursa `-std=c99 -Wall -Wextra -Werror -fsyntax-only` ile üç başlığı
  içeren bir `main.c` derler (CI ubuntu'da gcc var); bulunamazsa
  (Windows geliştirici makinesi) guard + parantez/küme dengesi denetimi
  yapar ve not düşer.
- `generated_json_is_valid_and_matches_schema`: §5 şemasını el
  yordamıyla (tip, ardışık `lsb`, ≤ 32 bit, `_reserved` adı) doğrular.
- `readonly_registers_get_no_setter_in_rust_or_c`,
  `writeonly_registers_get_no_getter_in_rust_or_c`: erişim hakkı testleri.

## Sonuçlar

`examples/soc/gpio.volt` için `volt build --emit=rust,c,regmap,regmap-md`:

| Çıktı | Dosya | Boyut |
|---|---|---|
| RTL | `build/rtl/Gpio.sv` | 130 satır (değişmedi) |
| Rust | `build/sw/gpio.rs` | 3 register → 1 struct, 6 sabit, 2 kurucu, 9 erişimci |
| C | `build/sw/gpio.h` | 16 `#define`, 9 `static inline` |
| JSON | `build/sw/gpio.json` | 3 register, 6 alan |
| Belge | `build/docs/gpio.md` | 1 özet + 3 bit tablosu |

Sürücüye yansıyan `@reg` özellikleri: `offset`, `access = ReadWrite |
ReadOnly | WriteOnly` (getter/setter varlığı), `volatile` (doc + JSON
bayrağı), `@reserved` (maske), `@self_clearing` (`trigger_*`), `@w1c`
(`clear_*`), alan tipleri `bool`/`bits<N>`/`uN` (en dar tam sayı tipi),
`base`, `bus`, üç seviyede doc yorumu. `no_std` uyumu: yalnız `core`,
`cargo check` testle doğrulanır.

Testler: +9 (regmap_tests) +23 (generators_tests) +17 (sw_emit_tests)
+1 (`ui/pass/72`) = +50; ui/pass sayım assert'leri 61.

## Sınırlar / Ertelenen

- Tek bus, 32 bitlik sözcük; JSON `bus` alanı ileride başka değer alabilir.
- Sıfırlama değeri yok (hep 0); sabit kimlik sözcüğü (`= 0x5C01`)
  gelince `reset` anahtarı eklenecek (`volt-regmap/2` değil — eklemeli).
- Rust sürücüsü tek örnekli (`&mut self` ile yazma); `Send`/`Sync`,
  `embedded-hal` ya da `svd2rust` benzeri tip durum makinesi yok.
- C tarafında çalışma zamanı taban adresi yok (`<MOD>_BASE` sabit);
  birden çok örnek isteyen kullanıcı makroyu yeniden tanımlayabilir.
- `regmap.json` için JSON Schema dosyası yok; şema bu ADR'de, testte
  el yordamıyla doğrulanır.
- SVD (CMSIS) ve IP-XACT üretimi bu ADR'nin dışındadır; `regmap.json`
  bunlara dönüştürücü yazmak için yeterli bilgiyi taşır.
- Üretilen dosyalar `build/` altındadır (`.gitignore`); depoya alınacak
  sürücü için kullanıcı kopyalar.
