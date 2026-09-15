# ADR-0051: Çift Yönlü Portlar — `inout` Yazma Desteği ve `opendrain` Tipi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-15
> Etkilenen: volt-ast (PortDir::OpenDrain, BidirRegs), volt-syntax
> (parser/bidir.rs YENİ, item.rs `opendrain` bağlamsal anahtar kelime,
> stmt.rs yöntem çağrısı deyimi, pipeline.rs), volt-hir (resolve.rs
> E4008, domain.rs W3007), volt-sv-emit (lib.rs `inout wire` + üç durumlu
> tampon + `tri1`, instance.rs çift yönlü bağlama, expr.rs literal cast),
> volt-diagnostics (E4008, W3007, explain), volt-lsp, docs/spec
> (grammar-full.ebnf PortDir, sv-mapping.md §1/§11/§17,
> domain-inference.md §5, type-inference.md §11.7), examples/i2c
> (i2c_top.sv SİLİNDİ), tests/ui/pass/68-69, tests/ui/fail/53-54

## Sorun

`examples/i2c/` keşfi (rapor 8a, 8g): `inout sda : bool` ayrıştırılıyor,
tipleniyor ve `inout logic sda` olarak üretiliyor; ama dilde `z` değeri,
sürücü etkinleştirme yapısı ya da açık drenaj kavramı yok. Bir `inout`
porta yazılabilen tek şey sıradan bir 0/1 ifadesi — yani push-pull
sürücü — ve I2C veri yolunda bu, pull-up ve diğer aygıtlarla çekişme
demek. Sonuç: I2C master pad'i `sda_in` / `sda_oe` çiftine bölmek ve
üç durumlu tamponu 88 satırlık elle yazılmış `i2c_top.sv` sarmalayıcıda
tutmak zorunda kaldı. Çift yönlü veri yolu olan HİÇBİR tasarım (I2C,
1-Wire, SDRAM DQ, bellek arayüzleri) saf Volt'ta yazılamıyordu.
Üstüne: `inout` portu `on` bloğunda `<=` ile bile yazılabiliyor, kimse
uyarmıyordu; test tezgâhı `inout`a dokunamıyor (E8503/E8504); pad'in
öbür ucundaki asenkron aygıt K2 gereği modülün saat alanında sayılıyordu.

## Karar

### 1. Sürücü ifadesi: Seçenek B+ — yöntem çağrıları, tek mekanizma

Üç seçenek değerlendirildi (görev tanımındaki A/B/C):

| | A) Elle ayrı `oe` sinyali | C) Koşullu sürme `sda = if drive { v } else { released }` | B+) Yöntemler `drive()` / `drive_low()` / `release()` |
|---|---|---|---|
| Yeni dil kavramı | Yok — bugünkü durum, sarmalayıcı SV'de kalır | `released` literali (bağlamsal; yalnız inout hedefli atamada geçerli), tipi hedefin tipi | Yok — `p.yöntem()` zaten alan zinciri + çağrı sözdizimidir |
| Tip denetimi | — | `released`in tipi bağlama bağlı: check-modu özel yol, sürekli atama + `<=` iki biçim | Sıradan `bool`/`T` atamaları: parser desugar'dan sonra hiçbir aşağı akış aşaması yeni bir tip görmez |
| SV üretimi | Elle | İfade ağacı ikiye AYRILMALI: `oe` ifadesi (`released` → 0, yaprak → 1) ve `out` ifadesi (`released` → 0); iç içe `if`/`match` için özyineli dönüşüm | Tek satır: `assign p = enable ? data : 'z` — register'lar zaten var |
| Kontrat | `oe_r` register'ı elle | `sda.released` için `oe` ifadesinin yeniden hesabı | `p.released` = `!<enable>` — register adı |
| Kombinasyonel sürme | Mümkün | Mümkün | Yok (yalnız `on`; bkz. Sınırlar) |
| opendrain ile ortaklık | — | Ayrı mekanizma (opendrain'de `released` değil `drive_low`) | AYNI mekanizma: `inout` ve `opendrain` tek desugar |

**B+ seçildi**: en az karmaşık olan. `inout` ve `opendrain` aynı
parser desugar'ından geçer; isim çözümleme, tip denetimi, domain
çıkarımı ve SV üretimi yalnız sıradan port + register görür (ADR-0038 /
ADR-0050 silme ilkesi). Bedeli, sürücü durumunun register olması
(kombinasyonel `oe` yok) — I2C, 1-Wire ve SDRAM denetleyicilerinde
sürücü zaten register'lanmıştır ve çıkış pinini kombinasyonel sürmek
glitch demektir.

Görev tanımındaki "Seçenek C ile `assign sda = sda_oe ? sda_out : 1'bz`"
SV çıktısı korunmuştur: aynı `sda_oe` / `sda_out` adları, yalnız bunlar
kullanıcı ifadesinden değil `drive()`/`release()` çağrılarından beslenir.

### 2. Sözdizimi ve anlam

```volt
inout     dq  : bits<8>        // bool / uN / iN / bits<N>
opendrain sda : bool           // yalnız bool: tek kablolu-VE biti

on clk {
    dq.drive(value)             // dq_oe <= true; dq_out <= value
    dq.release()                // dq_oe <= false
    sda.drive_low()             // sda_drive_low <= true
    sda.release()               // sda_drive_low <= false
}
let level = sda.read()          // hattın çözümlenmiş seviyesi (= sda)
invariant: !busy -> sda.released          // = !sda_drive_low
invariant: sda.driving -> busy            // =  sda_drive_low
```

* `opendrain` bağlamsal anahtar kelimedir (ADR-0023 tarzı): yalnız port
  konumunda `opendrain ad : tip` deseninde tanınır; başka yerde sıradan
  isimdir. Gramer: `PortDir = "in" | "out" | "inout" | "opendrain"`.
* Sentetik sürücü register'ları (parser, `parser/bidir.rs`): `inout p`
  → `reg p_oe : bool = false` + `reg p_out : T = 0` (`bits<N>` için
  `0 as bits<N>`); `opendrain p` → `reg p_drive_low : bool = false`.
  Sıfırlama değeri "serbest" — reset'te hiçbir pad sürülmez. Adlar
  gerçektir (kullanıcı `p_oe`yi okuyabilir; aynı adı bildirirse E1003).
  Yalnız SÜRÜLEN portun register'ı vardır; hiç sürülmeyen ama
  `released`/`driving` ile gözlenen port `let p_oe : bool = false` sabit
  teli alır; yalnız okunan pad hiçbir şey almaz (`assign` de üretilmez).
* Yöntem çağrıları yalnız `on` bloğunda deyimdir; `dq.drive(v)` iki
  `<=` üretir (parser `pending` kuyruğu). `read()` ifadedir ve porta
  yeniden yazılır; `released`/`driving` her ifade bağlamında (kontrat,
  `let`, `on`) sürücü register'ına yazılır.
* Aynı çevrimde hem okuyup hem yazmak GEÇERLİDİR (I2C'de olağan:
  ACK yuvasında `release()` + `read()`).

### 3. E4008 — çift yönlü port yanlış kullanımı

Tek kod, bağlama göre mesaj (E0001 gibi):

| Durum | Yer | Mesaj |
|---|---|---|
| `p = e` / `p <= e` doğrudan atama | resolve (`check_bidir_assign`) | `cannot assign to opendrain port 'p' directly` + yön başına yöntem listesi + port bildirimi ikincil etiketi |
| `on` dışında `p.drive_low()` (comb/let) | parser | `only allowed inside an 'on' block` |
| bilinmeyen üye `p.foo` / `p.foo()` | parser | `'p' has no member 'foo'` + sunulan üyeler |
| opendrain'e `drive(v)`, inout'a `drive_low()` | parser | bilinmeyen üye |
| argüman sayısı; `read()` deyim olarak; `drive_low()` değer olarak | parser | özel mesajlar |
| `opendrain` bool değil; `inout` dizi/tuple/struct | parser (port span) | tip kuralı |

Beş parça: kod, konum, açıklama, öneri, gerekçe notu ("çift yönlü pad
başka aygıtlarla paylaşılır … üç durumlu tamponu derleyici üretir
(ADR-0051)"). `volt explain E4008` iki dilde.

### 4. SV üretimi (sv-mapping.md §17; §11 istisnası)

```systemverilog
inout  wire        sda,                 // IEEE 1800 23.2.2.3: inout NET olmalı
inout  wire [7:0]  dq,
...
logic sda_drive_low;                    // sıradan reg: always_ff + reset
logic dq_oe; logic [7:0] dq_out;
// opendrain pad (ADR-0051): driven only while sda_drive_low is high
assign sda = sda_drive_low ? 1'b0 : 1'bz;
assign dq  = dq_oe ? dq_out : {8{1'bz}};
```

* Okuma: `sda.read()` → `sda` (net doğrudan okunur; ayrı `sda_in` teli
  gereksiz).
* Port sırası: clock → reset → in → inout/opendrain → out (§1).
* **§11 istisnası**: "x veya z değeri hiç üretilmez" kuralı `'z` için
  YALNIZ çift yönlü port sürücüsünde delinir. Gerekçe: üç durumlu
  tampon SV'de başka türlü ifade edilemez; `'z` tek bir yerde, derleyici
  tarafından, sabit kalıpla üretilir — kullanıcı ifadesinde asla
  görünmez. `x` hâlâ üretilmez.
* Örnekleme: örneğin çift yönlü portu üst modülün bir `wire`ına ya da
  kendi çift yönlü portuna ADIYLA bağlanır (`.sda(sda_bus)`); ifade
  bağlamak ya da bağlamamak E2005. Bağlanan üst tel net olur: `inout`
  için `wire`, `opendrain` için `tri1` (pull-up + kablolu-VE; birden çok
  pad aynı tele bağlanabilir). `<örnek>_<port>` çıkış teli üretilmez.
* Formal (Immediate) çıktı: Yosys serbest bırakılmış `'z` netini SABİT
  0 okur (probe: `assert (drive || !sda)` geçti) — SCL'nin pull-up'ını
  bekleyen bir FSM formal modelde sonsuza dek takılırdı. Bu modda dış
  aygıt `(* anyseq *) logic sda_ext;` ile modellenir:
  `assign sda = sda_drive_low ? 1'b0 : sda_ext;` — serbestken serbest
  değer, sürülürken modülün değeri. Yosys `tri1` de tanımadığından
  veri yolu teli formal çıktıda düz `wire`dır.

### 5. Kontratlar: `released` / `driving` sürücü NİYETİDİR

`sda.released` = `!sda_drive_low`, `sda.driving` = `sda_drive_low` —
kontratlar `z` değeri hakkında değil, modülün kendi sürücü register'ı
hakkında konuşur. Bu ayrım bilinçlidir: (1) `z` bir değer değildir;
Yosys'te yoktur, Verilator'da iki durumlu; (2) "hat serbest" bir
niyettir ve kanıtlanabilir olan budur — hattın gerçek seviyesi diğer
aygıtlara bağlıdır ve formal modelde serbesttir (anyseq). Hattın
seviyesi kontratta `sda.read()` ile ayrıca kullanılabilir (W3007
kontratta üretilmez). I2C: `invariant: !busy_r -> (sda.released &&
scl.released)` — "boşta master hattı asla çekmez" — `prove 3` ile
k-tümevarımla kanıtlandı.

### 6. W3007 — domain kuralı: çift yönlü okuma HARİCİDİR

`in` portu K2 ile modülün alanında sayılır (pinlere güvenilir). Çift
yönlü pad ise tanımı gereği başka bir aygıtça sürülür: `sda.read()`
(ya da çıplak `sda`) okuması harici sayılır ve `sync()` kaynağı değilse
**W3007** "external bidirectional signal read without synchronization"
üretir (uyarı; okuma modülün alanında kalır ki E3012 kaskadı olmasın).
`sync(sda.read(), clk)` içinde kaynak alan `Timeless` döner: hedefle
aynı alan sayılmaz, W3002 "gereksiz sync" çıkmaz. Kontrat içindeki
okuma uyarı üretmez (formal gözlem metastabilite taşımaz). Uyarı,
hata değil: bir test tezgâhı ya da karşı tarafın aynı saati paylaştığı
bilinen bir tasarım (senkron SRAM, `tests/ui/pass/68`) hattı doğrudan
okuyabilir. Clock stretching gibi durumlar için gerekli: I2C master
SCL'nin gerçekten yükseldiğini senkronize hattan görür.

## Sonuçlar

`examples/i2c/` (ADR-0050 durumu → ADR-0051):

| | Önce | Sonra |
|---|---|---|
| Elle yazılan SV | `i2c_top.sv` 69 satır (iki üç durumlu tampon + 20 satır port yönlendirme) | **0** — extern sarmalayıcı KALKTI |
| Pad arayüzü | 4 port (`sda_in`/`sda_oe`/`scl_in`/`scl_oe`) | 2 port (`opendrain sda`, `opendrain scl`) |
| Test tezgâhı veri yolu | Volt'ta elle kablolu-VE (`!(m.sda_oe \|\| s.sda_oe)`) + 2 `out` yönlendirme | iki `wire`, SV'de `tri1`; her aygıt kendi tamponuyla sürer |
| Hat okuma | doğrudan (`sda_in`), asenkronluk görünmez | `sync(sda.read(), clk)`; doğrudan okuma W3007 |
| Toplam satır | 328 + 541 + 69 = 938 | 343 + 555 = 898 (−40; `if x { release } else { drive_low }` +15, sarmalayıcı −69, bus modeli −14) |
| Sim | 12/12 | 12/12 (Docker Verilator, ilk denemede) |
| Formal | 12 özellik bmc 12 / prove 3 / cover 90 | 13 özellik bmc 12 / prove 3 / cover 190 (7/7 cover) |
| Verilator `-Wall` | temiz (sarmalayıcı dâhil) | temiz — `I2cTb` (tri1 + 3 modül) ve `I2cMaster` |

Tasarım değişikliği: iki-flop senkronizasyon 2 çevrim gecikme getirir;
faz 1 ("SCL serbest") senkronize SCL yüksek okunana dek biter. Standart
modda faz 8 çevrim olduğundan görünmez; hızlı modda faz 2 çevrim +1
bekleme = bit başına 9 çevrim (355 kHz). `stretched` bayrağı ikinci
ardışık bekleme çevriminde kalkar (senkronizatör gecikmesinden fazla).
Sistem saati 3.2 MHz (32 / 8 clk-bit) seçildi: 100 kHz tam, 400 kHz
2 clk-faz ile iki tarafın senkronizatör gecikmesine yeter (1.6 MHz'de
1 clk-faz, köle ACK'ını master örneklemesine yetiştiremezdi — hesap
i2c_test.volt başlığında).

Verilator üç durumlu davranışı: `--lint-only -Wall` `1'bz`, `{8{1'bz}}`
ve `tri1` için hiçbir uyarı vermez; `--cc` ile İÇ tri-state netleri
(alt modül tamponları + üst `tri1`) doğru çözülür — 12 test bunun
üzerinde koştu. Üst seviye `inout` portu için ise `--cc` varsayılanda
tamponu sessizce atar (`--pins-inout-enables` gerekir); `volt test`
zaten üst seviye `inout`a erişemez (E8503/E8504), tezgâh üstünde
`tri1` tel kullanılır.

Testler: `tests/ui/pass/68_inout_bidirectional.volt` (bits<8> SRAM
portu, 1-bit dinleyici, üst tahta), `69_opendrain_basic.volt` (pad +
iki pad'li veri yolu; bmc/prove/cover 3/3), `tests/ui/fail/53_opendrain_direct_assign.volt`
(E4008), `54_inout_unsynchronized.volt` (W3007);
volt-syntax/tests/bidir_tests.rs (+19), volt-hir/tests/
bidir_semantic_tests.rs (+16), volt-sv-emit/tests/bidir_emit_tests.rs
(+14), volt-diagnostics explain (116 kod). Yan kazanım: `0 as bits<8>`
literal cast'i emitter'da E2005 veriyordu; soneksiz literal kaynağı
artık hedef genişliğinde boyutlandırılır.

## Sınırlar / Ertelenen

- Sürücü durumu yalnız register: `comb`/`let` içinde `drive_low()` E4008.
  Kombinasyonel `oe` (sürücü durumunu `let` ile üretmek) ayrı ADR;
  glitch'li çıkış pini olduğundan bilinçli ertelendi.
- `dq.drive(v)` sürülen değeri register'lar: veri çevrim sonra pad'de
  görünür (`dq_out <= v`). Kombinasyonel veri (`assign dq = oe ? v : 'z`)
  aynı erteleme.
- Bundle (`struct port`) alanı `inout` olabilir ve düzleşir, ama
  `aw.dq.drive(v)` biçimi tanınmaz (parser yöntem çağrısını yalnız düz
  port adında görür); düz ad (`aw_dq.drive(v)`) çalışır.
- `opendrain` yalnız `bool`; çok bitli açık drenaj veri yolu için hat
  başına port ya da `inout`.
- `volt test` üst seviye `inout`/`opendrain` portuna erişemez
  (E8503/E8504 değişmedi); tezgâh modülünde `wire` + `out` yönlendirme
  gerekir. `dut.sda_pull_low` benzeri tezgâh sürücüsü V1.
- Formal: bir üst modülün sahip olduğu ve ≥2 örneğin sürdüğü veri yolu
  teli Immediate çıktıda push-pull anyseq sürücülerin çakışmasına yol
  açar (Yosys "multiple drivers"); pad'i olan modülü doğrulayın, tezgâhı
  değil. `tri1` formal çıktıda düz `wire`.
- Bir port hem `p.driving` ile gözlenip hem hiç sürülmezse sabit `let`
  üretilir; SV'de `wire p_oe = 1'b0` + `assign p = 1'b0 ? … : 'z` —
  Verilator/Yosys sabit katlar.
- Sentetik ad span'leri port bildiriminin son karakterlerinden alınır;
  `inout a:b` gibi 9 karakterlik bir bildirimde iki sentetik ad (oe +
  out) yine sığar, LSP referans araması bu konumları gösterir.
- `extern module` çift yönlü port bildirebilir (yön tablosuna girer);
  ADR-0047'nin sınırı sürer: sv-emit extern örneğini üretemez (E0003).
- `x` hâlâ üretilmez; `'z` yalnız derleyici kalıbında (§11).
