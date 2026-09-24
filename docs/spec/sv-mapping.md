# Volt → SystemVerilog Eşleme Spesifikasyonu

> STATÜ: BAĞLAYICI SPESİFİKASYON
> Her Volt yapısının ürettiği SystemVerilog burada tanımlıdır.
> Hedef standart: IEEE 1800-2017
> Karar kayıtları: ADR-0008 (çıktı stili §0, yasak liste §11), ADR-0012 (1:1 modül, isim korunumu İ1/İ2), ADR-0010 (string template üretim, CIRCT ertelendi)

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
- Port sırası: clock → reset → diğer inputlar → inout/opendrain → outputlar
- `inout` / `opendrain` portu `inout wire` (IEEE 1800 23.2.2.3: inout bir
  net olmalı) — bkz. §17 (ADR-0051)
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
| `uint<N>` | `logic [N-1:0]` | genişliği sabit ifade (ADR-0041) |
| `sint<N>` | `logic signed [N-1:0]` | işaretli eşi (ADR-0041) |
| `clock` | `logic` | port bağlamında |
| `[T; N]` | `T_sv ad [0:N-1]` | unpacked dizi (ADR-0035); boyut isimden SONRA — sentez araçları BRAM/dağıtık RAM'e eşleyebilir. Yalnız `reg` bildirimlerinde |
| `[T; N]` port / `wire` | `logic [N*W-1:0] ad` | PAKETLENMİŞ vektör (ADR-0056): Yosys unpacked dizi portu kabul etmez. Eleman erişimi `ad[W*i +: W]` (literal indekste katlanır: `ad[8 +: 8]`), işaretli eleman `$signed(ad[...])`; bütün dizi ataması vektör kopyası |

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

**Kural:** `let` → `wire`; genişlik açık tip yazılmışsa ANOTASYONDAN,
yoksa tip çıkarımından gelir (ADR-0041). Anotasyon operanddan genişse
operandlar boyut dönüşümüyle açık genişletilir: `let p : i32 = t * c`
(t, c: i16) → `wire signed [31:0] p = 32'(t) * 32'(c);` (§16).

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
| `<<` | `<<` | sol kaydırma (işaretten bağımsız) |
| `>>` (işaretsiz sol operand) | `>>` | mantıksal kaydırma |
| `>>` (işaretli sol operand) | `>>>` | aritmetik kaydırma, işaret korunur (ADR-0036) |
| `a as u16` | `{{8{1'b0}}, a}` | zero-extend |
| `a as i16` | `{{8{a[7]}}, a}` | sign-extend |
| `a as i32` (a: u32, aynı genişlik) | `$signed(a)` | işaret yeniden yorumlama (ADR-0036); öz-belirlenimli sınır |
| `a as u32` (a: i32, aynı genişlik) | `$unsigned(a)` | işaret yeniden yorumlama (ADR-0036) |
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
- ADR-0041: çıkış telleri modül gövdesinin BAŞINDA bildirilir (kullanım
  sırasından bağımsız); reset, hedef modülün saat alanı yapılandırmasından
  üst modülün aynı adlı reset portuna bağlanır; bağlanmamış giriş portu
  E4011 (ADR-0072); generic argümanlı örnekleme monomorfizasyonla somut modül adına
  (`FirFilter<8, 16>` → `FirFilter_8_16`) çevrilmiş olarak gelir

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
✗ x veya z değeri   → hiç üretilmez (İSTİSNA: çift yönlü port tamponu
                       `assign p = en ? v : 'z`, yalnız derleyici
                       kalıbı — §17, ADR-0051; x hâlâ üretilmez)
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

---

## 14. Bundle Portları (ADR-0039)

Bundle (`struct port`) DÜZLEŞİR; SV `interface`/`modport` ÜRETİLMEZ.
Her alan `<port>_<alan>` adlı bağımsız bir port olur; iç içe bundle
`<port>_<alan>_<alt_alan>`. Yön alanın bildirilen yönüdür, port `in`
ile bildirilmişse tersine çevrilir. Port sırası kaynak sırasıdır
(bundle alanları bildirildikleri yerde açılır); §1'deki in/inout/out
gruplaması düz portlara uygulanır.

### Volt

```volt
struct port AxiWriteAddr {
    out addr  : u32
    out valid : bool
    in  ready : bool
}

module Slave {
    in aw : AxiWriteAddr
    aw.ready = true
}
```

### SystemVerilog

```systemverilog
module Slave (
    input  logic [31:0] aw_addr,
    input  logic        aw_valid,
    output logic        aw_ready
);
    assign aw_ready = 1'b1;
endmodule
```

Gerekçe: SV `interface` sentez ve lint araçlarında (özellikle modport
yönleri, parametreli arayüzler ve hiyerarşi düzleştirme) tutarsız
desteklenir; düz portlar her araçta çalışır ve üretilen RTL'nin
Volt kaynağıyla bire bir izlenebilirliğini korur. Test bloklarında ve
örnekleme bağlamalarında da düz ad kullanılır (`dut.aw_addr`).

---

## 15. Ardışık Kontratlar — `prev()` (ADR-0040)

`prev(x)` bir önceki döngüdeki, `prev(x, N)` N döngü önceki değerdir;
yalnız kontratlarda geçerlidir (RTL'de E5017) ve `x`'in saat alanında
değerlendirilir (başka alanla karışım E3001).

| Volt | `--emit=sva` (Inline/Separate) | `volt verify` (Immediate, Yosys) |
|---|---|---|
| `prev(x)` | `$past(x)` | `past_x_1` yardımcı register |
| `prev(x, 3)` | `$past(x, 3)` | `past_x_1 → past_x_2 → past_x_3` zinciri |

Yosys'in Verilog ön ucu `$past`'i bilmediğinden Immediate modda her
farklı argüman için bir register zinciri üretilir; reset değeri 0'dır
(reset sonrası ilk döngüde `prev(x) == 0`). Aynı argümanın tüm
derinlikleri tek zinciri paylaşır; bileşik argüman `past_e<k>_N` adını
alır.

```systemverilog
// prev() helper registers (ADR-0040): value N cycles ago, 0 after reset
logic past_b_valid_1;
always_ff @(posedge clk) begin
    if (rst) begin
        past_b_valid_1 <= '0;
    end else begin
        past_b_valid_1 <= b_valid;
    end
end
// invariant from axi4lite_slave.volt:83
always @(posedge clk)
    if (!(rst)) assert (!(past_b_valid_1 && !past_b_ready_1) || b_valid); // volt:inv_5
```

---

## 16. Genişleme, Const Diziler, Açılan Döngüler (ADR-0041)

### 16.1 Açık genişletme — boyut dönüşümü

Volt, hedef tipi açıkça yazılmış aynı-işaret genişlemeyi kabul eder
(type-inference.md §5). SV'de bağlam-belirlenimli genişlik aynı değeri
verir ama Verilator `-Wall` WIDTHEXPAND uyarır; bu yüzden emitter
genişlemeyi boyut dönüşümüyle AÇIK basar:

```volt
let p : i32 = t * c          // t, c : i16
result = s                   // result : i32, s : i16
```
```systemverilog
wire signed [31:0] p = 32'(t) * 32'(c);
assign result = 32'(s);
```

Kural: aritmetik operandın etkin bağlamı `max(ifade genişliği, hedef)`;
bağlamdan dar ATOM operand (`x`, `arr[i]`, `inst_out`) `W'(x)` ile sarılır,
bileşik alt ifadeye bağlam içeri aktarılır. Literaller zaten bağlam
genişliğiyle boyutlanır. Boyut dönüşümü işaretli operandda işaret,
işaretsizde sıfır genişletir. Kaydırmanın sol operandına dış bağlam
itilmez; karşılaştırma operandları kendi genişliklerinde kalır.

Aynı biçim bileşik ifade cast'lerinde de kullanılır: `(a + b) as i32` →
`32'(a + b)`, `(a + b) as u8` → `8'(a + b)`. Basit sinyal cast'leri §6'daki
`{{N{x[msb]}}, x}` / `x[W-1:0]` biçimlerini korur.

### 16.2 Const diziler

```volt
const COEFFS : [i16; 8] = [1, 2, 3, 4, 4, 3, 2, 1]
let p0 : i32 = taps[0] * COEFFS[0]      // sabit indeks
let c  : i16 = COEFFS[sel]              // sinyal indeks
reg   k : [i16; 8] = COEFFS             // reg başlatıcı
```
```systemverilog
function automatic logic signed [15:0] COEFFS_at(input logic [2:0] i);
    case (i)
        0: COEFFS_at = 16'sd1;
        1: COEFFS_at = 16'sd2;
        // ...
        7: COEFFS_at = 16'sd1;
        default: COEFFS_at = 16'sd0;
    endcase
endfunction

wire signed [31:0] p0 = 32'(taps[0]) * 32'sd1;   // katlandı
wire signed [15:0] c = COEFFS_at(sel);
// reset dalında: k[0] <= 16'sd1; k[1] <= 16'sd2; ...
```

- Sabit indeks (literal, sabit ifade, açılmış döngü değişkeni) elemanı
  LİTERALE katlar; SV'de dizi adı görünmez.
- Sinyal indeks: modül gövdesinin başında bir kez tablo işlevi bildirilir
  (`function automatic logic signed [15:0] COEFFS_at(input logic [2:0] i)`
  + `case`) ve erişim `COEFFS_at(idx)` olur. Ölçüm (ADR-0041): Yosys
  `read_verilog -sv` unpacked `localparam` dizisini reddeder, tablo işlevi
  Yosys ve Verilator -Wall'da temiz; `localparam` biçimi emitter'da
  `ConstArrayStyle::LocalparamArray` ile seçilebilir yedek olarak kalır.
- Çıplak dizi referansı yalnız reg başlatıcısında; başka konumda E2005.
- Negatif sabit değerler `(-16'sd2)` olarak basılır.

### 16.3 Döngü açma, `comb`, `wire`

```volt
reg taps : [i16; 4] = [0; 4]
wire acc : i32
on clk {
    for i in 1..4 { taps[i] <= taps[i - 1] }
    taps[0] <= sample
}
comb {
    acc = 0
    for i in 0..4 { acc = acc + taps[i] * COEFFS[i] }
}
```
```systemverilog
logic signed [15:0] taps [0:3];
logic signed [31:0] acc;

always_ff @(posedge clk) begin
    if (rst) begin
        for (int volt_i = 0; volt_i < 4; volt_i = volt_i + 1) taps[volt_i] <= 16'sd0;
    end else begin
        taps[1] <= taps[0];
        taps[2] <= taps[1];
        taps[3] <= taps[2];
        taps[0] <= sample;
    end
end

always_comb begin
    acc = 32'sd0;
    acc = acc + 32'(taps[0]) * 32'sd1;
    acc = acc + 32'(taps[1]) * 32'sd2;
    acc = acc + 32'(taps[2]) * 32'sd3;
    acc = acc + 32'(taps[3]) * 32'sd4;
end
```

- `for` sınırları derleme zamanı sabiti olmalı (const-eval.md §8); değilse
  E2021 (ADR-0072). Gövde her iterasyon için açılır, döngü değişkeni literale ikame
  edilir. Modül seviyesi `for` gövdesindeki `=` atamaları `assign`
  satırlarına açılır.
- `wire x : T` → `logic ... x;` bildirimi; sürücüsü `comb` ya da `assign`.
- `comb { }` → `always_comb begin ... end`; aynı blok içinde art arda
  tam atama tek sürücü sayılır (type-inference.md §11.2).

---

## 17. Çift Yönlü Portlar — `inout` / `opendrain` (ADR-0051)

### Volt
```volt
module Pad {
    in  clk : clock
    in  en  : bool
    opendrain sda : bool          // yalnız bool; pull-up harici, kablolu-VE
    inout     dq  : bits<8>       // bool / uN / iN / bits<N>
    out level : bool

    invariant: !en -> sda.released      // sürücü niyeti: !sda_drive_low

    wire sda_s : bool
    sda_s = sync(sda.read(), clk)       // dış aygıt: önce senkronize (W3007)

    on clk {
        if en { sda.drive_low() } else { sda.release() }
        if en { dq.drive(0 as bits<8>) } else { dq.release() }
    }
    level = sda_s
}
```

### SystemVerilog
```systemverilog
module Pad (
    input  logic       clk,
    input  logic       rst,
    input  logic       en,
    inout  wire        sda,
    inout  wire [7:0]  dq,
    output logic       level
);
    logic sda_drive_low;          // parser'ın sentezlediği sürücü register'ları
    logic dq_oe;
    logic [7:0] dq_out;
    ...                           // always_ff: reset'te serbest (1'b0), sonra <=

    // opendrain pad (ADR-0051): driven only while sda_drive_low is high
    assign sda = sda_drive_low ? 1'b0 : 1'bz;
    // inout pad (ADR-0051): driven only while dq_oe is high
    assign dq = dq_oe ? dq_out : {8{1'bz}};
endmodule
```

**Kurallar:**
- Sürücü durumu register'dır: `p.drive(v)` → `p_oe <= 1; p_out <= v`,
  `p.drive_low()` → `p_drive_low <= 1`, `p.release()` → enable `<= 0`;
  yalnız `on` bloğunda. Doğrudan atama (`p = e`, `p <= e`) E4008.
- Okuma `p.read()` net'in kendisidir (`sda`); ayrı `sda_in` teli yok.
- `p.released` = `!<enable>`, `p.driving` = `<enable>` — kontratlar
  sürücü NİYETİNİ kanıtlar, `z` değerini değil.
- Yalnız sürülen (ya da `released`/`driving` ile gözlenen) port tampon
  alır; yalnız okunan pad için `assign` üretilmez.
- Örnekleme: çift yönlü port üst modülün `wire`ına ya da kendi çift
  yönlü portuna ADIYLA bağlanır (`.sda(sda_bus)`); bağlanan tel net
  olur — `inout` için `wire`, `opendrain` için `tri1` (pull-up +
  kablolu-VE; birden çok pad aynı tele bağlanabilir). `<örnek>_<port>`
  çıkış teli üretilmez; ifade bağlamak / bağlamamak E4011 (ADR-0072).
- Formal (`volt verify`, Immediate) çıktısı: Yosys serbest `'z` netini
  sabit 0 okur; bu modda dış aygıt `(* anyseq *) logic p_ext;` +
  `assign p = enable ? value : p_ext;` ile modellenir (serbestken
  serbest değer), `tri1` yerine düz `wire` yazılır.
- Verilator: `-Wall` lint temiz; `--cc` iç tri-state netlerini çözer,
  üst seviye `inout` için `--pins-inout-enables` gerekir.
