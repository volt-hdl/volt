# Volt → SystemVerilog Eşleme Spesifikasyonu

> STATÜ: BAĞLAYICI SPESİFİKASYON
> Her Volt yapısının ürettiği SystemVerilog burada tanımlıdır.
> Hedef standart: IEEE 1800-2017

---

## 0. Temel İlkeler

```
İ1. İSİM KORUNUMU
    Volt'taki her tanımlayıcı SV'de aynı isimle görünür.
    → Constraints dosyaları (SDC/XDC) çalışmaya devam eder.

İ2. 1:1 MODÜL EŞLEMESİ
    Bir Volt modülü → bir SV modülü.
    → ECO mühendisi kaynağı tanıyabilir.

İ3. always_ff / always_comb
    Çıplak `always` ASLA kullanılmaz.
    → Lint araçları latch/race uyarısı vermez.

İ4. AÇIK GENİŞLİK
    Her sinyal genişliği açıkça yazılır: logic [7:0]
    → Örtük genişlik çıkarımı yok.

İ5. YORUM KORUNUMU
    /// belgeleme yorumları SV'ye aktarılır.
```

---

## 1. Modül

### Volt
```volt
/// 8-bit sayaç
module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8
}
```

### SystemVerilog
```systemverilog
// 8-bit sayaç
module Counter (
    input  logic       clk,
    input  logic       rst,
    input  logic       enable,
    output logic [7:0] count
);
endmodule
```

**Kurallar:**
- `clock` tipi → `input logic clk`
- Reset portu **otomatik eklenir** (bkz. §7)
- Port sırası: clock → reset → diğer inputlar → outputlar
- Belgeleme yorumu `///` → `//` olarak aktarılır

---

## 2. Tipler

| Volt | SystemVerilog | Not |
|------|---------------|-----|
| `bool` | `logic` | tek bit |
| `u8` | `logic [7:0]` | işaretsiz |
| `u16` | `logic [15:0]` | |
| `u32` | `logic [31:0]` | |
| `u64` | `logic [63:0]` | |
| `i8` | `logic signed [7:0]` | işaretli |
| `i16` | `logic signed [15:0]` | |
| `i32` | `logic signed [31:0]` | |
| `bits<N>` | `logic [N-1:0]` | aritmetiksiz |
| `clock` | `logic` | port bağlamında |
| `[T; N]` | `T_sv [N-1:0]` | dizi |

**Örnek:**
```volt
in  data  : u8
in  flag  : bool
out sum   : i16
in  bus   : bits<4>
in  mem   : [u8; 16]
```
```systemverilog
input  logic       [7:0]  data,
input  logic              flag,
output logic signed [15:0] sum,
input  logic       [3:0]  bus,
input  logic       [7:0]  mem [15:0]
```

---

## 3. Register Bildirimi

### Volt
```volt
reg count : u8 = 0
```

### SystemVerilog
```systemverilog
logic [7:0] count;
```

**Kural:** Başlangıç değeri bildirimde değil, reset bloğunda
üretilir (bkz. §4).

---

## 4. Sıralı Blok (`on`)

### Volt
```volt
reg count : u8 = 0

on clk {
    if enable {
        count <= count + 1
    }
}
```

### SystemVerilog
```systemverilog
logic [7:0] count;

always_ff @(posedge clk) begin
    if (rst) begin
        count <= 8'd0;
    end else begin
        if (enable) begin
            count <= count + 8'd1;
        end
    end
end
```

**Kurallar:**
- Reset bloğu **otomatik eklenir**, `reg` bildirimindeki
  başlangıç değeri kullanılır
- Varsayılan: senkron reset, aktif-yüksek
- Sayısal literaller genişliğe göre boyutlandırılır: `8'd1`
- Her `if` bloğu `begin`/`end` ile sarılır (lint dostu)

### Asenkron Reset Varyantı

```volt
domain AsyncDomain {
    clock = posedge
    reset = async active_low
}

module Foo {
    in clk : clock @AsyncDomain
    reg x : u8 = 0
    on clk { x <= x + 1 }
}
```
```systemverilog
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        x <= 8'd0;
    end else begin
        x <= x + 8'd1;
    end
end
```

---

## 5. Kombinasyonel Atama

### 5.1 Basit Bağlantı

```volt
out result : u8
result = count
```
```systemverilog
assign result = count;
```

### 5.2 `let` Bağlaması

```volt
let sum = a + b;
let doubled = sum << 1;
```
```systemverilog
wire [8:0] sum = a + b;
wire [9:0] doubled = sum << 1;
```

**Kural:** `let` → `wire`, genişlik tip çıkarımından gelir.

### 5.3 Koşullu İfade

```volt
out y : u8
y = if sel { a } else { b }
```
```systemverilog
assign y = sel ? a : b;
```

### 5.4 Çok Dallı Koşul

```volt
y = if a { 1 } else if b { 2 } else { 3 }
```
```systemverilog
assign y = a ? 8'd1 : (b ? 8'd2 : 8'd3);
```

---

## 6. Operatörler

| Volt | SystemVerilog | Not |
|------|---------------|-----|
| `+` `-` `*` | `+` `-` `*` | doğrudan |
| `/` `%` | `/` `%` | sentez uyarısı üretilir |
| `&` `\|` `^` | `&` `\|` `^` | bit düzeyi |
| `~` | `~` | bit değilleme |
| `!` | `!` | mantıksal değilleme |
| `&&` `\|\|` | `&&` `\|\|` | mantıksal |
| `==` `!=` | `==` `!=` | |
| `<` `>` `<=` `>=` | `<` `>` `<=` `>=` | |
| `<<` `>>` | `<<` `>>` | mantıksal kaydırma |
| `a as u16` | `{{8{1'b0}}, a}` | zero-extend |
| `a as i16` | `{{8{a[7]}}, a}` | sign-extend |
| `a[3]` | `a[3]` | bit seçimi |
| `a[7:4]` | `a[7:4]` | aralık seçimi |

**Genişlik kuralı:**
```volt
let s = a + b;   // a: u8, b: u8 → s: u9
```
```systemverilog
wire [8:0] s = {1'b0, a} + {1'b0, b};
```
Taşma genişlemesi açık zero-extend ile yapılır.

---

## 7. Reset Portu Üretimi

Reset portu kullanıcı yazmadan otomatik eklenir:

```
Domain reset ayarı          Üretilen port      always_ff
─────────────────────────────────────────────────────────────
sync, active_high (vars.)   input logic rst    @(posedge clk)
                                                if (rst)
sync, active_low            input logic rst_n  @(posedge clk)
                                                if (!rst_n)
async, active_high          input logic rst    @(posedge clk or
                                                  posedge rst)
async, active_low           input logic rst_n  @(posedge clk or
                                                  negedge rst_n)
none                        (port yok)         @(posedge clk)
                                                (reset bloğu yok)
```

**Açık reset kontrolü:**
```volt
in rst : reset(async, active_low)   // isim: rst_n olur
```

---

## 8. CDC Köprüsü

### Volt
```volt
out slow_data : u8
slow_data = sync(fast_data, slow_clk)
```

### SystemVerilog
```systemverilog
// CDC senkronizatörü: fast_clk -> slow_clk
logic [7:0] sync_fast_data_stage0;
logic [7:0] sync_fast_data_stage1;

always_ff @(posedge slow_clk) begin
    if (rst) begin
        sync_fast_data_stage0 <= 8'd0;
        sync_fast_data_stage1 <= 8'd0;
    end else begin
        sync_fast_data_stage0 <= fast_data;
        sync_fast_data_stage1 <= sync_fast_data_stage0;
    end
end

assign slow_data = sync_fast_data_stage1;
```

**Kurallar:**
- İsimlendirme: `sync_<kaynak_sinyal>_stage<N>`
- İki flip-flop varsayılan (3 için `sync3()` kullanılır)
- Yorum satırı otomatik eklenir (SDC için ipucu)
- SDC'de `set_false_path` üretilir (F4'te)

---

## 9. Modül Örnekleme

### Volt
```volt
let u = Uart { clk: clk, data: tx_data };
out busy : bool
busy = u.busy;
```

### SystemVerilog
```systemverilog
logic u_busy;

Uart u (
    .clk  (clk),
    .rst  (rst),
    .data (tx_data),
    .busy (u_busy)
);

assign busy = u_busy;
```

**Kurallar:**
- İsimli port bağlama (positional asla kullanılmaz)
- Çıkış sinyalleri için `<örnek>_<port>` wire üretilir
- Clock ve reset otomatik bağlanır

---

## 10. Sayısal Literaller

| Volt | SystemVerilog | Not |
|------|---------------|-----|
| `0` | `8'd0` | hedef genişliğe göre |
| `42` | `8'd42` | |
| `0xFF` | `8'hFF` | |
| `0b1010` | `4'b1010` | |
| `-1` (i8) | `-8'sd1` | işaretli |
| `true` | `1'b1` | |
| `false` | `1'b0` | |

**Kural:** Literal genişliği bağlamdan çıkarılır. Belirsizse
E2005 hatası verilir.

---

## 11. Üretilmeyecek SV Yapıları (Yasak Liste)

```
✗ always            → always_ff veya always_comb
✗ reg               → logic
✗ wire (reg için)   → logic
✗ initial           → hiç kullanılmaz
✗ #delay            → hiç kullanılmaz
✗ pozisyonel bağlama → isimli bağlama
✗ örtük genişlik    → her zaman açık
✗ x veya z değeri   → hiç üretilmez
✗ casex / casez     → case ile açık karşılaştırma
```

---

## 12. Dosya Başlığı

Her üretilen SV dosyası şu başlıkla başlar:

```systemverilog
// Bu dosya Volt tarafından otomatik üretilmiştir.
// Kaynak: counter.volt
// Volt sürümü: 0.1.0
// Üretim tarihi: (deterministik build'de üretilmez)
//
// DÜZENLEMEYİN — değişiklikler kaynak dosyada yapılmalıdır.

`default_nettype none
```

**Not:** `--release` modunda tarih yazılmaz (determinizm için).
`` `default_nettype none `` örtük wire bildirimini engeller.

---

## 13. Doğrulama

Üretilen her SV dosyası şu kontrollerden geçmelidir:

```bash
verilator --lint-only -Wall design.sv
# Sıfır uyarı bekleniyor

iverilog -t null design.sv
# Sözdizimi kontrolü

yosys -p "read_verilog -sv design.sv; hierarchy -check; check"
# Sentezlenebilirlik kontrolü
```

CI bu üç kontrolü her commit'te çalıştırır.
