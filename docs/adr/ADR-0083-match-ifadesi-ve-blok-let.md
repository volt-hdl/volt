# ADR-0083: `match` İfadesi ve Blok İçi `let`

> Statü: KABUL EDİLDİ — Aşama 1 (tasarım), Aşama 2 (uygulama, bkz.
> "Aşama 2 — uygulama notları"). Aşama 3 (örnekler + eşdeğerlik kanıtı)
> bu belgeye not olarak eklenecek.
> Tarih: 2026-09-26
> Etkilenen (plan): volt-syntax (ifade `match`'inde E0014), volt-hir
> (match ifadesinin tip kuralları ve kapsayıcılığı, `comb` koşulunun
> saat alanı — §2.2 açığı, muhafızın örtük akışı, blok `let`'ine atama
> E4001, consteval'de `match`, açılım bütçesine sınanan kopyası),
> volt-sv-emit (match ifadesi: kök konumda `case`, iç konumda üçlü
> zincir; blok `let`'i süreç içi yerel değişken; ad ayırma ve E1013;
> `structs/` match), volt-diagnostics (yeni kod yok; E0014 ve E2003
> metinleri ifadeyi de anlatır), `tests/`, `docs/spec/` (Aşama 2),
> `examples/` (Aşama 3).
> Aşama 1'de DOKUNULMADI: kod, docs/spec/; hiçbir aşamada: docs/research/.
> Genişletir: ADR-0032 (match deyimi), ADR-0041 (check kipi), ADR-0052
> (K6/K7 örtük akış), ADR-0074 (Karar 4 — son kol), ADR-0081 (fn gövdesinde
> `match`, açılım bütçesi, ad ayırma), `domain-inference.md` K5/K6/K7,
> `name-resolution.md` §2/§6.

## Sorun

`match` yalnız sıralı/kombinasyonel blokta **deyim** olarak SV'ye iniyor
(ADR-0032, `case`). Değer üreten seçim — ALU sonucu, çoklayıcı, kod
çözme, ROM tablosu — `if`/`else` zinciriyle ya da her kolda aynı hedefe
yazan bir deyim `match`'iyle yazılmak zorunda. ADR-0081 fn gövdesinde
`match`'i erteledi; bu yüzden `riscv_core`'da ALU işlemi ve dallanma
koşulu fn'e taşınamadı. Ayrıca `on`/`comb` bloğunda ara hesaplamaya ad
verilemiyor: blok içi `let` parser, çözümleme ve tip denetiminden geçip
emitter'da E0003 alıyor.

Enum (ADR-0074), struct (ADR-0077) ve fn (ADR-0081) aynı yöntemle
eklendi: önce ölçüm ve tasarım, sonra uygulama, sonra örnekler ve
donanımın değişmediğinin kanıtı. Bu ADR birinci aşamadır.

## 1. Mevcut durum (ölçüm)

Sondalar `build/m1/` (gitignore'da; üretici `build/m1/gen.py`),
`target/debug/volt.exe check`, `main` @ 7a71c98. Her satır bir komut
çıktısıdır; `W1001` (kullanılmayan port) satırları ayıklandı.

### 1.1 `match` ifadesi

| Sonda | Kaynak (özet) | `check` |
|---|---|---|
| `m01` | modül `let r = match op { 0 => a, 1 => b, _ => a + b }` | `E0003 not supported yet: 'match' expressions` |
| `m02` | enum, dört varyant, `_` yok | E0003 (yalnız) |
| `m03` / `m04` | `on` / `comb` içinde sağ taraf | E0003 |
| `m05` | fn gövdesinin son ifadesi | E0003 |
| `m06` | sayısal, `_` yok (`0..3` hepsi yazılı) | **yalnız E0003 — E0014 yok** |
| `m07` | `0 => a, _ => true` (u8 / bool kol) | **yalnız E0003 — tip hatası yok** |
| `m14` | enum, eksik varyant, `_` yok | **yalnız E0003 — E0014 yok** |
| `m13` | bağlama deseni `x => a` | yalnız E0003 |
| `m10` / `m12` / `m15` | `1 \| 2`, muhafız `0 if a > b`, iç içe | yalnız E0003 |
| `m11` | aralık `1..=2` | `E0001 unexpected '..': expected '=>' in the match arm` |
| `m16` | blok kol `0 => { a }` | `E0001 unexpected '{': expected arm body` |
| `m09` | kontratta `invariant: (match …) == (match …)` | `check` ve varsayılan `build` temiz; `build --emit=sva` ve `volt verify` `E0003 … 'match' expressions` (kontrat yalnız SVA üretilirken iner — ADR-0070 "A sınıfı" SVA'ya özgü fark, ADR-0081 `p06b` ile aynı) |
| `z01` | test dilinde `let x = match 1 { … };` | `E0001 unexpected 'match': expected a test expression` |

E0003'ün tek kaynağı emitter'dır: `crates/volt-sv-emit/src/expr.rs:710`
(`'match' expressions`); ADR-0070 ortak boru hattı onu `check`'te de
gösterir. Parser (`parser/expr.rs:539` `parse_match_expr`) ve çözümleme
kabul ediyor; **tip denetimi ifadeyi denetlemiyor**:

```rust
// crates/volt-hir/src/typeck/synth.rs:300
fn synth_match(&mut self, scrutinee: Idx<Expr>, arms: &[MatchArm]) -> TypeId {
    self.synth(scrutinee);
    for arm in arms { self.check_arm(arm); }   // kol gövdesi yalnız synth
    self.types.error()                         // sonuç tipi: Error
}
```

Sonuç tipi `Error` olduğu için kol uyuşmazlığı (`m07`) ve beklenen tip
(`m08`) hiç sınanmıyor; E0014 deyim parser'ında (`parser/stmt.rs:611`
sayısal) ve tip denetiminde (enum, `typeck/matching.rs`,
`typeck/gated.rs`) yalnız `MatchStmt` için üretiliyor. Deyim düzeyinde
emitter'ın desteklediği desenler (`sv-emit/src/lib.rs:1780-1900`):
literal, `_`, `A | B`, argümansız enum varyantı; muhafız, ifade gövdeli
deyim kolu, bağlama/tuple/argümanlı yol E0003.

### 1.2 `if` ifadesi — emsal

| Sonda | Kaynak | Sonuç |
|---|---|---|
| `i01` | modül `let r = if op == 0 { a } else { b }` | temiz; SV `wire [7:0] r = (op == 2'd0) ? a : b;` |
| `i02` | `on` içinde `q <= if … ` | temiz; `q <= (op == 2'd0) ? a : b;` |
| `i03` / `i04` | fn gövdesi / kontrat | temiz; SVA `((op == 2'd0) ? a : b) >= 8'd0` |
| `y01` | kollar `u8` / `u9`, beklenen tip yok | `E2003 if/else branches have different types: 'u8' and 'u9'` |
| `y02` / `y06` | `u8`/`bool`, `u8`/`i8` | E2003 aynı biçimde |
| `y03` | `let r : u9 = if c { a + b } else { b }` | `wire [8:0] r = c ? (9'(a) + 9'(b)) : 9'(b);` — beklenen tip kollara itiliyor, taşma korunuyor |
| `y04` | aynı, tip yazılmamış | `wire [7:0] r = c ? (a + b) : b;` (sentez kipi: 8 bit) |
| `y07` | `y = if c { a + b } else { b }` (y: u9) | `assign y = c ? (9'(a) + 9'(b)) : 9'(b);` |
| `k01` | `const N : u32 = if true { 1 } else { 2 }` | temiz (`consteval.rs:305` `ExprKind::If`) |
| `k02` | struct tipli `if` | alan başına: `wire [7:0] p_x = c ? a : 8'd0;` |
| `k03` / `k04` | enum tipli; örnek port bağlamasında | `s <= go ? S_Run : S_Idle;` / `.x(c ? a : 8'd0)` |

`if` ifadesi her konumda çalışıyor: sentez kipinde kollar **aynı tip**
(E2003), check kipinde beklenen tip kollara itiliyor (ADR-0041), SV'de
satır içi üçlü. `match` ifadesi bunun genellemesidir.

### 1.3 Blok içi `let`

| Sonda | Kaynak | `check` |
|---|---|---|
| `l01` / `l02` | `on clk { let t = a + b; q <= t }`, `comb` eşi | `E0003 not supported yet: 'let t' inside a block` |
| `l03` | `if` dalında `let t`, dal dışında `t` | `E1001 undefined name: 't'` (kapsam doğru) |
| `l04` | `on` içinde `let a = b` (port `a`'yı gölgeler) | `W1002 'a' shadows a definition in an outer scope` + E0003 |
| `l07` | dış blokta `let t`, `if` dalında yine `let t` | W1002 + E0003 |
| `u01` | `q <= t` sonra `let t = a` | `E1002 't' is not yet defined at this point` |
| `u02` | aynı blokta iki `let t` | `E1003 't' is already defined in this scope` |
| `l06` | `comb { let t = a; t = b; … }` | **yalnız E0003** (modül `let`'ine atama `a01` → `E4001 't' is already driven`) |
| `l09` | `let t : u9 = a + b` | E0003 |
| `r02` | blokta `let begin = a` | **yalnız E0003 — E1013 yok** (modül `let begin` `r01` → `E1013 'begin' is a SystemVerilog keyword`) |

E0003'ün kaynağı yine yalnız emitter (`sv-emit/src/lib.rs:1744`).
Çözümleme `name-resolution.md` §2/§6'yı zaten uyguluyor (blok kapsamı,
sıralı bildirim, iç kapsamda W1002, aynı kapsamda E1003). Açık kalanlar:
blok `let`'ine atama sürücü analizinde yakalanmıyor (`l06`) ve E1013'ün
kesin denetimi (`sv-emit/src/sv_names.rs:201` `audit_module_names`)
yalnız modül düzeyi deyimleri tarıyor.

### 1.4 Kontrat ve test dili

Kontrattaki `if` ifadesi (`v01`: `invariant: q == (if q == 0 { 0 } else { q })`,
`assert: y == (if op == 0 { a } else { q })`):

```
$ VOLT_SBY=build/sby-docker.cmd volt verify --mode prove build/m1/v01_ifexpr_contract.volt
     [1/1] V (2 properties) ... ok (0.31s)
      Result 2 properties verified in 1.2s (16 jobs; prove, depth 20)
$ … v02 (assert'te kasıtlı hata: else kolu a)
      V.ast_0  E5001 contract violated at cycle 2
      Result 1 of 2 properties failed in 1.0s
```

`--emit=sva` (ayrı dosya + `bind`) çıktısı `q == ((q == 8'd0) ? 8'd0 : q);`:
`verilator --lint-only -Wall --assert V.sv v.sva` temiz; Yosys 0.36
`read -formal` `property` anahtar sözcüğünde `syntax error` veriyor — bu,
ADR-0081 §5.5'te de görülen, üçlüden bağımsız bir Yosys sınırıdır
(`volt verify` immediate kipini kullanır). Test dili (ADR-0058) ayrı bir
gramerdir (`TestPrimary`), `if`/`match` ifadesi yoktur (`z01`: E0001).

## 2. Güvenlik ve saat alanı: örtük bilgi akışı (ölçüm)

`match`'in seçimi, sonuç değeri sınananın kendisi olmasa da bilgi taşır
(`match secret_bit { 0 => 0, _ => 1 }` = `secret_bit`). Soru: bugün
`trust_level` (ADR-0052), domain (CDC) ve `Delayed` (ADR-0037) bu akışı
if/match **deyimlerinde** ve ifadelerinde izliyor mu?

### 2.1 Güven (E3009) — açık YOK; muhafızda gizli boşluk

Sondalar `t01`–`t11`: `key : u8 @Secret`, `dbg : u8 @Pub`.

| Sonda | Akış | Sonuç |
|---|---|---|
| `t01` | `on`: `if key[0] { d <= 1 } else { d <= 0 }`, `dbg = d` | `E3009 secret data flows to a public output` |
| `t02` | `on`: deyim `match key[0] { true => {d <= 1}, _ => {d <= 0} }` | E3009 |
| `t03` | `comb`: `if key[0] { dbg = 1 } else { dbg = 0 }` | E3009 ×2 (her dal) |
| `t04` | `dbg = if key[0] { 1 } else { 0 }` | E3009 |
| `t05` | `dbg = match key[0] { true => 1, _ => 0 }` | E3009 (analiz ifadeyi görüyor; SV E0003) |
| `t07` | `if key[0] { let t = a; d <= t }` | E3009 (blok `let`'i dal koşulunu taşıyor) |
| `t08` | `d <= a >> (key[2:0] as u3)` | E3009 |
| `t09` | `let s = if key[0] { a } else { a }` | E3009 (tutucu: iki kol aynı olsa da) |
| `t11` | `for i in 0..8 { if key[i] { d <= i } }` | E3009 |
| `t06` | `match a { 0 if key[0] => { d <= 1 }, _ => { d <= 0 } }` | **yalnız `E0003 … 'match' arm guards`** — E3009 YOK |
| `t06b` | `dbg = match a { 0 if key[0] => 1, _ => 0 }` | yalnız E0003 — E3009 YOK |

`crates/volt-hir/src/trust.rs:479-520` `walk_block` koşul etiketini `pc`
olarak her yazmaya, blok `let`'ine ve iç içe dallara taşıyor (K7);
`expr_tag` `If`'te `cond ⊔ then ⊔ else`, `Match`'te `scrutinee ⊔ kollar`
(`trust.rs:358-384`). **Bugün erişilebilir bir örtük akış açığı yok.**

**Gizli boşluk:** muhafız (`arm.guard`) ne deyimde (`trust.rs:486`) ne
ifadede (`trust.rs:369`) etikete katılıyor. Bugün her muhafız E0003
aldığı için donanım üretilemiyor — ama `check` sızıntıyı bildirmiyor
(`t06`) ve muhafız desteği açıldığı anda gerçek açık olur. Aşama 2
ADIM 2.1'de kapatılır (Karar 8).

### 2.2 Saat alanı (E3001/E3012) — MEVCUT AÇIK: `comb` koşulu

Sondalar `d*`/`x*`: `fs : bool @Fast`, `sa : u8 @Slow`, `y : u8 @Slow`.

| Sonda | Akış | Sonuç |
|---|---|---|
| `d01` / `d02` / `x08` | `on sclk { if fs { q <= sa } }`, deyim `match fs`, ifade gövdeli kol | `E3012 a signal from a foreign clock domain is read in an 'on' block` |
| `d03` | `y = if fs { sa } else { 0 }` | `E3001 different clock domains cannot be combined combinationally` |
| `d04` | `y = match fs { true => sa, _ => 0 }` | E3001 |
| **`d06`** | `comb { if fs { y = sa } else { y = 0 } }` | **temiz, `build` rc=0** |
| **`x03`** | `comb { match fs { true => { y = sa }, _ => { y = 0 } } }` | **temiz** |
| **`x04`** | `comb { y = 0; if fs { y = sa } }` | **temiz** |

`d06` ile `d03` aynı donanımdır (`y`, `fs`'e kombinasyonel bağlı; `fs`
`Slow` saatine göre eşzamansız) ama biri E3001 alıyor, öteki temiz
derleniyor. Neden: `crates/volt-hir/src/domain/walk.rs:39`
`StmtKind::Comb(block) => self.walk_block(*block, None)` — `comb`
bloğunda bağlam (`ctx`) yok, `walk_if` (`walk.rs:93`) koşulun alanını
yalnız `ctx` varken (`on` bloğu, E3012) denetliyor; `comb` atamasının
K6 denetimi (`assign.rs`) yalnız sağ tarafın alanına bakıyor. Bu,
`domain-inference.md` K5'in (koşul da bir operanddır) `comb` deyimlerinde
uygulanmamasıdır: **mevcut bir CDC açığı.** Düzeltme Aşama 2 ADIM 2.1'de,
match ifadesinden önce (Karar 8).

### 2.3 Zamanlama (E5010) — açık değil, bilinçli sınır

| Sonda | Akış | Sonuç |
|---|---|---|
| `e03` | `y = if x != 0 { s1 } else { x }` (kollar 1 / 0 çevrim) | `E5010 timing misalignment` |
| `e02` | `y = if s1 != 0 { x } else { x }` (koşul 1 çevrim) | temiz |
| `x05` / `x07` | `on`/`comb` içinde `if s1 != 0 { … }` | temiz |

ADR-0037 §4: "`if`/`match` koşulları ve `match` scrutinee'si kontrol
sinyalidir, denetim dışıdır; yalnız veri yolları izlenir"
(`timing.rs:425` `// Koşul kontrol sinyalidir — yalnız dallar birleştirilir.`). Davranış belgelenmiş
karardır; match ifadesi aynı kuralı izler (Karar 8).

## 3. `match` ifadesinin anlamı

### Karar 1: Desenler — deyimle aynı küme

| Desen | Bu tur | Gerekçe |
|---|---|---|
| literal (`0`, `true`, `3'b101`) | ✓ | ADR-0032 |
| alternatif `A \| B` | ✓ | ADR-0032; `_` içeren alternatif joker sayılır (parser `pattern_has_wildcard`) |
| joker `_` | ✓ | ADR-0032 |
| argümansız enum varyantı `S::Idle` | ✓ | ADR-0074 |
| aralık `1..=3` | ✗ (bugün E0001, `m11`) | gramerde yok; deyim ve ifadeye birlikte, ayrı iş |
| bağlama `x`, `x @ …` | ✗ E0003 | kol başına kapsam ve SV'de ad gerekir; deyimde de E0003 |
| struct / tuple / argümanlı yol | ✗ E0003 | payload'lı enum yok (ADR-0074), struct deseni ayrı iş |
| muhafız `if cond` | ✗ E0003 | deyimde de E0003 (ADR-0032); açıldığında kapsayıcılığa sayılmaz ve koşulu örtük akışa katılır — Karar 8'in kuralı şimdiden uygulanır |
| blok kol `=> { … }` | ✗ (bugün E0001, `m16`) | gramerde `MatchArmExpr = … "=>" Expr`; blok `let`'i (§4) ara ad ihtiyacını karşılar |

**Gerekçe:** ifade ve deyim aynı desen çözümlemesini, kapsayıcılık
analizini ve emitter kodunu (`case` öğesi üretimi) paylaşır; bir desen
ikisine birlikte açılır. Böylece "deyimde çalışan desen ifadede
çalışmıyor" durumu hiç oluşmaz.

### Karar 2: Tipler — `if` ifadesinin kuralı

- **Sınanan** sentezlenir. İzin verilen tipler deyimle aynı: `uN`/`iN`,
  `bits<N>`, `bool`, enum. Desen literali sınanan tipine `check` edilir
  (deyimle aynı sığma kuralı); enum yolu sınananın enum'unun varyantı
  olmalıdır (ADR-0074 kodları).
- **Check kipi** (beklenen tip var — tipli `let`, atama hedefi, port
  bağlaması, fn dönüş tipi, argüman): beklenen tip **her kola itilir**,
  sonuç o tiptir. `let r : u9 = match op { 0 => a + b, _ => b }` —
  `a + b` 9 bitte hesaplanır, taşma korunur (`y03`'ün birebir eşi).
- **Sentez kipi** (beklenen tip yok — tipsiz `let`, karşılaştırma
  operandı): ilk kolun tipi sentezlenir, diğer kollar ona `check` edilir;
  uyuşmazlık **E2003** "match arms have different types: 'u8' and 'bool'"
  (`if`'in "if/else branches have different types" iletisinin eşi; kod
  aynı, yeni kod yok). Hepsi tipsiz literal olan kollar `if`'teki gibi
  davranır (W2012 + i32).
- Struct tipli sonuç: `if` gibi alan başına indirgenir (`k02`); bugünkü
  `structs/mod.rs:876` E0003'ü kalkar.

**Gerekçe:** kullanıcı açısından `match` çok kollu `if`'tir; iki yapının
genişlik davranışının ayrışması (biri taşmayı korurken öteki kesmesi)
ADR-0041'in kapattığı hata sınıfını geri açar.

### Karar 3: Kapsayıcılık — deyimle AYNI kurallar

- Sayısal (`bool` dahil) sınanan: muhafızsız bir `_` kolu zorunlu,
  yoksa **E0014** — bütün değerler literal olarak yazılmış olsa da
  (`m06`: `u2`'nin 0..3'ü). ADR-0032 ile birebir; `bool` için `if`
  ifadesi zaten var.
- Enum sınanan: bütün varyantlar muhafızsız kollarda ya da `_`; eksikse
  E0014 (ADR-0074 Karar 4 tablosu, `typeck/matching.rs`
  `enum_not_exhaustive`, çözümleme hatasında `typeck/gated.rs` yedeği).
- İfadede "boş `_`" yoktur: her kol bir değer verir. E0014 iletisi
  bağlama göre "'match' expression has no '_' arm" der; `volt explain
  E0014` ifade örneğini de içerir.

### Karar 4: Geçersiz kodlar — ifadede de son kol

Bütün varyantları adlı, `_`'sız enum match'inde geçersiz kod (3
varyantlı 2 bitlik enum'da `3`) **son adlı kolun değerini** alır
(ADR-0074 Karar 4: son kol `default:`). İfadede de aynı: `case`
biçiminde son kol `default:` olur; üçlü zincirde son kol zincirin
`else`'idir — iki biçim aynı donanımdır (§5.4: A↔B, `default` ↔ son
`else`; ADR-0081 §5.4: enum'lu match, Bt↔B 129/129). **Gerekçe:** deyimi
ifadeye (ya da tersine) çevirmek geçersiz kod davranışını
değiştirmemeli; Aşama 3'ün eşdeğerlik kanıtı buna dayanır.

### Karar 5: Nerede — bu tur

| Konum | Bu tur | Not |
|---|---|---|
| Modül `let`, modül ataması (`y = …`) | ✓ | |
| `on`/`comb` içinde atama sağ tarafı, `if` koşulu, deyim `match`'inin sınananı | ✓ | |
| Blok `let`'i sağ tarafı (§4) | ✓ | |
| Örnek port bağlaması, indeks, operand (herhangi bir donanım ifadesi) | ✓ | |
| fn gövdesi (ADR-0081'de ertelenen) | ✓ | açılım `inline/` üzerinden; ADR-0081 E0003 satırı kalkar |
| Kontratlar (immediate ve `--emit=sva`) | ✓ | üçlü biçimde (Karar 10); `v01`/`v02` kanıtı `if` ile |
| Const bağlamı | ✓ | `consteval` `If`'i değerlendiriyor (`k01`); `Match` aynı yorumlayıcıya eklenir: desen eşleşmesi derleme zamanında |
| Test dili | ✗ | ayrı gramer ve yorumlayıcı (ADR-0058); fn çağrısıyla birlikte ortak sabit yorumlayıcı işi (ADR-0081 Gelecek iş 2) |

## 4. Blok içi `let`

### Karar 6: `let` saf bir ara addır, register değildir

`on clk { let t = a + b; r <= t }` — `t` kombinasyonel bir değerin
adıdır; bir çevrim gecikme, durum ya da sürücü değildir. `t`'ye atama
(`t = …`, `t <= …`) **E4001** (modül `let`'inde olduğu gibi — `a01`,
`a02`); bugünkü sessiz kabul (`l06`) kapanır. Tipli biçim modül
`let`'iyle aynı: `let t : u9 = a + b` check kipidir (ADR-0041).

### Karar 7: Değer, bildirim noktasındaki değerdir

Blok `let`'i, bloğun sıralı yürütmesinde **bildirildiği noktada**
değerlendirilir:

- **`on` bloğu:** blokta yalnız `<=` vardır (E0006); bir register'ı
  okuyan `let` her zaman **güncellenmeden önceki** değeri görür —
  `r <= …`'dan sonra yazılmış olsa da. Bu, `on` bloğundaki her sağ
  tarafın bugünkü anlamıdır (`q <= q + 1; w <= q` eski `q`'yu okur).
  Ölçüm (§5.5): `q <= a; let t = q; w <= t` iki SV biçimde de eski değer
  — `rL <-> rH: Of those cells 24 are proven and 0 are unproven`.
- **`comb` bloğu:** Volt `comb` bloğu SV blocking anlamındadır — ölçüm
  (`build/m1/c/`): `comb { y = 0; z = y; if c { y = a } }` kabul ediliyor
  ve `always_comb begin y = 8'd0; z = y; if (c) … end` olarak iniyor
  (`z` ara değeri okur); `comb { y = a; y = y + 1 }` tanısız. Bu yüzden
  `comb` içindeki `let`, bildirim noktasına kadar yapılmış blocking
  atamaları görür. Bu karar SV eşlemesini belirler (Karar 11).

### Karar 8: Kapsam, gölgeleme ve bilgi akışı

- **Kapsam:** bildirimden içinde bulunduğu `{ }` bloğunun sonuna kadar
  (`name-resolution.md` §2 "Block scope"). `if`/`else`/`match` kolu
  içinde bildirilen `let` dal dışında görünmez (`l03` E1001 — zaten
  uygulanıyor); bildirimden önce kullanım E1002 (`u01`).
- **Gölgeleme:** `name-resolution.md` §6 aynen: iç kapsamda izinli +
  **W1002** (modül portu/`let`'i ya da dış blok `let`'i — `l04`, `l07`);
  aynı kapsamda **E1003** (`u02`; Rust'taki aynı kapsamda yeniden
  bağlama Volt'ta yok). Gölgeleme SV'de ad yakalamaya yol açar (§5.6) —
  emitter yeniden adlandırır (Karar 11).
- **Güven:** blok `let`'i sağ tarafının etiketini ve içinde bulunduğu
  dalın koşul etiketini (`pc`) taşır — `trust.rs:498-502` bugün böyle
  (`t07`). Değişiklik yok.
- **Örtük akış — Aşama 2 ADIM 2.1 (önce, ayrı commit):**
  1. **Domain (açık, §2.2):** `comb` bloğu da `pc` alanıyla yürünür:
     `if` koşulunun / deyim `match`'i sınananının alanı, dal içindeki her
     atamanın sağ tarafına K5 `join` ile katılır ve K6 denetimi E3001'i
     verir. `on` bloğundaki E3012 değişmez.
  2. **Trust (gizli boşluk, §2.1):** muhafız ifadesinin etiketi, deyimde
     kolun `pc`'sine, ifadede sonuca katılır (`walk_block` Match kolu ve
     `expr_tag` Match kolu). Domain'de muhafız alanı aynı biçimde
     (deyimde E3012/`comb`'da E3001, ifadede K5).
  3. match ifadesi: sonuç = sınanan ⊔ (muhafızlar) ⊔ kollar — trust ve
     domain'de bugün böyle (`t05`, `d04`); ifade SV'ye inince davranış
     değişmez.
  4. Zamanlama: ADR-0037 kuralı — sınanan ve muhafız kontrol sinyali,
     kollar `combine` (E5010). Değişiklik yok.
- **Sınıflandırma:** ADIM 2.1'den sonra mevcut `examples/`,
  `tests/ui/**`, `tests/fixtures/**` ve gömülü test kaynaklarında yeni
  çıkan her E3001 A (gerçek CDC → raporlanır) / B (yanlış alarm → kural
  daraltılır) olarak sınıflanır; tablo PR açıklamasında. ADIM 2.1 yalnız
  TANI ekler, SV'yi değiştirmez (golden ile doğrulanır).

## 5. SV eşlemesi (ölçüm)

### Deney

`build/m1/sv/` (üretici `gen_sv.py`, betik `run.sh`): RV32I ALU (10 işlem
`op : u4`, `_ => 0`) + dallanma koşulu (6 `funct3`, `_ => false`) +
`on clk { let s = alu; let t = s ^ a; if en { q <= t } }`. Aynı arayüzde
biçimler:

- **A** — match ifadesi → iç içe üçlü, sürekli atama (`wire [31:0] alu_r = (op == 4'd0) ? a + b : … : 32'd0;`)
- **B** — match ifadesi → `always_comb` + `case` + ara `logic`
- **D** — deyim biçimi (ADR-0032): `comb`'da `case`, `q` için `always_ff` içinde kol başına `case`
- **LA** — blok `let`'i modül teline taşınmış (A'nın içinde: `wire [31:0] on0_s = alu_r;`)
- **LB** / **LBc** — blok `let`'i `always_ff` / `always_comb` içinde yerel değişken (ayrı bildirim + atama)
- **LBn** — LB, adlı blok (`begin : on_clk`)
- **LBi** — LB, bildirimde ilk değer (`logic [31:0] s = alu_r;`)
- **Am** — A + kasıtlı hata (SRA → SRL)

C (fn içinde match) ayrı bir SV biçimi değildir: ADR-0081 Karar 12'nin
açılımı fn gövdesini çağrı yerine taşır; gövdedeki match çağrı yerinde
A/B'den biri olur.

Araçlar: `verilator/verilator:latest` (5.050), `hdlc/yosys:latest`
(Yosys 0.66), `hdlc/formal:latest` (Yosys 0.36+42, `volt verify`'ın
imajı). `MSYS_NO_PATHCONV=1 docker run --rm --entrypoint sh -v
C:/Dev/volthdl/build/m1/sv:/work <imaj> /work/run.sh <lint|yread|synth|eq> …`.

### 5.1 Verilator `-Wall`

```
== A    - Verilator: Built from 0.031 MB sources in 2 modules, into 0.026 MB …
== B    - Verilator: Built from 0.031 MB sources … into 0.025 MB …
== D    - Verilator: Built from 0.031 MB sources … into 0.029 MB …
== LB   - Verilator: Built from 0.031 MB sources … into 0.027 MB …
== LBc  - Verilator: Built from 0.031 MB sources … into 0.028 MB …
== LBn  - Verilator: Built from 0.031 MB sources … into 0.027 MB …
== LBi  %Warning-IMPLICITSTATIC: /work/LBi/Alu.sv:38:22: Variable's lifetime implicitly set to static (IEEE 1800-2023 6.21)
        %Warning-IMPLICITSTATIC: /work/LBi/Alu.sv:39:22: …
== Am   - Verilator: Built from 0.031 MB sources …
```

`always_ff` içinde yerel değişkene blocking atama (LB) `-Wall`'da uyarı
üretmiyor; bildirimde ilk değer (LBi) üretiyor.

### 5.2 Yosys okuma

`read_verilog -sv; prep` (0.66) ve `read -formal; prep` (0.36): A, B, D,
LB, LBc, LBn temiz; **LBi iki sürümde de**
`/work/LBi/Alu.sv:38: ERROR: Invalid nesting of always blocks and/or initializations.`
(ADR-0081 §5.1 `yf/19` ile aynı bulgu). Yerel değişken yalnız ayrı
bildirim + atama biçiminde kullanılabilir.

### 5.3 Sentez (Yosys 0.66)

`run.sh synth`: `synth -top Alu -flatten; stat` + `synth_ice40` +
`synth_xilinx -flatten -noiopad`:

| | genel | iCE40 | xc7 |
|---|---|---|---|
| A | 1410 | 739: 644 SB_LUT4, 63 SB_CARRY, 32 SB_DFFESR | 674: 230 LUT6, 79 LUT5, 84 LUT4, 29 LUT3, 35 LUT2, 28 CARRY4, 32 FDRE, 103 MUXF7, 21 MUXF8 |
| B | 1386 | 734: 639 SB_LUT4, 63 SB_CARRY, 32 SB_DFFESR | 717: 215 LUT6, 65 LUT5, 76 LUT4, 52 LUT3, 92 LUT2, 20 LUT1, 28 CARRY4, 32 FDRE, 84 MUXF7, 20 MUXF8 |
| D | 1439 | 1058: 963 SB_LUT4, 63 SB_CARRY, 32 SB_DFFESR | 761: 269 LUT6, 73 LUT5, 69 LUT4, 68 LUT3, 75 LUT2, 8 LUT1, 28 CARRY4, 32 FDRE, 89 MUXF7, 16 MUXF8 |
| LB | **A ile aynı** (1410 / 739 / 674, aynı dağılım) | | |
| LBc | **A ile aynı** | | |

- Blok `let`'inin tel (LA, A'nın içinde) ve yerel değişken (LB, LBc)
  biçimleri üç akışta **aynı hücre dağılımı**.
- A-B farkı `case` ile üçlü zincirin ABC başlangıç ağından (ADR-0081
  §5.3 ve ADR-0077 bulgusu: sayım eşdeğerlik kanıtı değildir); iCE40'ta
  B, xc7'de A küçük.
- D büyük: deyim biçiminde `q`'nun `case`'i ALU'yu `y`'ninkinden ayrı
  yazıyor (kol başına `(v) ^ a`), Yosys paylaştırmıyor. Bu, ifade
  biçiminin (ALU bir kez adlandırılıp iki yerde kullanılır) somut
  kazancıdır.

### 5.4 Eşdeğerlik (Yosys 0.66, ADR-0077 yöntemi)

`run.sh eq` (`prep; flatten; equiv_make gold gate equiv; equiv_simple
-seq 1; equiv_induct -seq 1; equiv_status -assert`):

```
A <-> B:    rc=0   Of those cells 194 are proven and 0 are unproven.
A <-> D:    rc=0   Of those cells 97 are proven and 0 are unproven.
B <-> D:    rc=0   Of those cells 97 are proven and 0 are unproven.
A <-> LB:   rc=0   Of those cells 130 are proven and 0 are unproven.
A <-> LBc:  rc=0   Of those cells 130 are proven and 0 are unproven.
B <-> LB:   rc=0   Of those cells 130 are proven and 0 are unproven.
D <-> LBc:  rc=0   Of those cells 97 are proven and 0 are unproven.
A <-> LBn:  rc=0   Of those cells 130 are proven and 0 are unproven.
A <-> Am:   rc=1   Of those cells 101 are proven and 93 are unproven.
                   ERROR: Found 93 unproven $equiv cells in 'equiv_status -assert'.
```

İfade biçimleri (A, B), deyim biçimi (D) ve blok `let`'inin iki eşlemesi
aynı donanım; kasıtlı hata (Am) denetimi düşürüyor.

### 5.5 `comb` karşı örneği: tel taşıma anlamı değiştirir

Karar 7'nin `comb` anlamıyla iki kaynak, iki eşleme:

- `comb { y = 0; let t = y; if c { y = a }; z = t }` — **cH**: `wire [7:0] cmb_t = y;` (modül teli), **cL**: `always_comb` içinde yerel `t`.
- `comb { y = a; let t = y + 1; y = t; z = a }` — **kH** tel, **kL** yerel.

```
$ run.sh lint cH cL kH kL
== cH  - Verilator: Built from 0.030 MB sources …          (temiz ama YANLIŞ)
== cL  - Verilator: Built from 0.030 MB sources …
== kH  %Warning-UNOPTFLAT: /work/kH/Alu.sv:2:68: Signal unoptimizable: Circular combinational logic: 'y'
== kL  (yalnız kasıtlı kullanılmayan 'c' için UNUSEDSIGNAL)
$ run.sh eq cL cH
cL <-> cH: rc=1   Of those cells 8 are proven and 8 are unproven.
$ run.sh eq kL kH
kL <-> kH: rc=1   Of those cells 8 are proven and 8 are unproven.
$ yosys -p "read_verilog -sv kH/Alu.sv; prep -top Alu; check -assert"
Warning: found logic loop in module Alu:
```

Tel biçimi `let`'i bloğun **son** değerine bağlar: `cH`'de `z` 0 yerine
son `y`'yi okur (sessizce farklı donanım), `kH`'de kombinasyonel döngü
kurar. Yerel değişken sıralı anlamı korur. `on` bloğunda iki biçim
eşdeğerdir (`rL <-> rH`: 24/24; blokta blocking yazma yoktur).

### 5.6 Dalga formu ve gölgeleme

`verilator --cc --trace` ile üretilen izleme bildirimlerindeki adlar:

```
A    "alu_r" "br_take" "on0_s" "on0_t" "q_r" …           (tel: modül düzeyi)
LB   "alu_r" "br_take" "q_r" "s" "t" "unnamedblk1" …      (yerel: adsız blok kapsamı)
LBn  "alu_r" "br_take" "on_clk" "q_r" "s" "t" …           (yerel: adlı blok kapsamı)
```

Yerel değişkenler dalga formunda görünür; adsız blokta kapsam adı
Verilator'un sayacıdır (`unnamedblk1`, başka bir yerel bloğun eklenmesiyle
kayar), adlı blokta kararlıdır.

Gölgeleme: LB'de yerellerin adı `en` (giriş portu) yapılınca
(`LBh`):

```
%Warning-VARHIDDEN: /work/LBh/Alu.sv:39:22: Declaration of signal hides declaration in upper scope: 'en'
%Warning-WIDTHTRUNC: /work/LBh/Alu.sv:45:13: Logical operator IF expects 1 bit on the If, but If's VARREF 'en' generates 32 bits.
```

İkinci satır asıl tehlikedir: aynı süreçte sonraki `if (en)` artık yerele
bağlanıyor. Volt çözümü `DefId` ile yapar, SV adla; yerel ad modül adını
ya da başka bir yereli gölgelediğinde emitter **yeniden adlandırmak
zorundadır** (Karar 11).

### 5.7 Kontrat

Kontrat ifadesinin içinde `case` yazılamaz. `if` ifadesinin üçlü
biçimi immediate kipte kanıtlanıyor ve kasıtlı hatada düşüyor (`v01`/
`v02`, §1.4); `--emit=sva` bind modülünde Verilator `--assert` temiz.
Match ifadesi kontratta aynı üçlü biçimi kullanır.

### 5.8 Ölçeklenme: çok kollu match

Tek sınananlı, `n` kollu match (`op : u16`, kol `a ^ k`) — **üçlü**
(`deep<n>`) ve **`case`** (`deepc<n>`):

```
verilator deep256   rc=0   92ms
verilator deep1024  rc=0  108ms
verilator deep2048  rc=0  158ms
verilator deep4096  rc=1   85ms %Error: /work/deep4096/Alu.sv:2500:33: memory exhausted
verilator deepc4096 rc=0  337ms
yosys     deep256   rc=0  264ms
yosys     deep512   rc=0 1533ms
yosys     deep1024  rc=0 8781ms
yosys     deep4096  rc=0 705187ms
yosys     deepc4096 rc=0 1667ms   (ikinci koşuda 2016ms)
```

Üçlü zincir, kol sayısı kadar derin bir SV ifadesidir: Verilator'un
ayrıştırıcı yığını 4096'da tükeniyor, Yosys'in okuma süresi kol sayısıyla
süper-doğrusal büyüyor (256 → 1024: 33×, 1024 → 4096: 80×; 4096 kol
11,8 dakika). `case` 4096 kolda iki araçta da iki saniyenin altında. ROM tablosu (`let v = match addr { 0 => …, 1023 => … }`)
gerçekçi bir kullanım olduğu için bu sınır tasarımı belirler.

### Karar 10: match ifadesi — kökte `case`, içeride üçlü

**A (yalnız üçlü)** kontratta ve iç içe konumda zorunludur ama §5.8'de
ölçeklenmiyor. **B (her zaman `always_comb` + `case`)** her iç konumda
bir ara ad ve bir süreç gerektirir: iç içe ifade, port bağlaması, `else if`
koşulu için hesaplamanın sıralı noktada ayrı bir süreçte yapılması —
`comb`'da Karar 7'yi bozma riskiyle (§5.5'in aynısı). Seçilen **karma
kural, yere göre (eşiğe göre değil)**:

1. **Kök konum → `case`.** match ifadesi bir atamanın ya da `let`'in
   **tüm** sağ tarafıysa (fn açılımından sonra), deyim biçimine
   (ADR-0032) iner — hedef her kolda tekrarlanır:
   - modül `let r = match …` → `logic [W-1:0] r;` + `always_comb case … r = v; default: r = vd; endcase`
   - modül `y = match …` → `always_comb` içinde `y = v`
   - `on` içinde `q <= match …` → süreç içinde `case (sel) k: q <= v; …`
   - `comb` içinde `y = match …` → `case (sel) k: y = v; …` (bildirim noktasında, Karar 7)
   - blok `let t = match …` → yerel `t` (Karar 11) + `case … t = v`
   - fn tel kipinde (ADR-0081 Karar 12) sonuç teli / fn `let` teli kök konumdur.
   Son kol kuralı Karar 4; struct tipli hedef kol başına alan atamaları.
2. **İç konum → üçlü zincir.** Operand, koşul, indeks, port bağlaması,
   kontrat, ikame kipindeki fn açılımı: `(sel == k0) ? v0 : (sel == k1)
   ? v1 : … : v_son` — `if` ifadesinin bugünkü biçimi; alternatif
   `(sel == k1 || sel == k2)`, enum `S_Idle` `localparam`'ı.
3. **Derinlik:** iç konumdaki üçlü zincirin yüksekliği kol sayısıdır;
   ADR-0081 ikame kipi kuralı genişler — emit edilen ifade yüksekliği
   `MAX_DEPTH` (256) aşılırsa **E0018** (yeni kod yok), `= help:` "match'i
   bir `let`'e verin (kök konum `case` olur)". 256 kol §5.8'de iki araçta
   da güvenli (92 ms / 264 ms).
4. **Kopya maliyeti:** üçlü zincirde sınanan her literal için bir kez
   yazılır (`Σ desen literali × |sınanan|`); bu maliyet ADR-0081 Karar
   11 açılım bütçesine (`MAX_EXPANSION_NODES`, E2027) eklenir — iç içe
   sınanan (`match (match x {…}) {…}`) üstel kopyaya gidemez. Sınanan
   yeni bir ada bağlanmaz (üretilen ad sayısı sıfır kalır); tipik sınanan
   yalın ad ya da dilimdir (`instr[6:0]`).

**Gerekçe:** kök konum pratikte büyük match'lerin (kod çözücü, ROM)
yeridir ve oraya ADR-0032'nin ölçülmüş, ölçeklenen `case` biçimi gelir —
üretilen ek ad yok, hedef adı dalga formunda aynı. İç konum `if`
ifadesinin tutarlı genellemesidir ve kontratta tek seçenektir. İkisi aynı
donanım (§5.4). Deyim ↔ ifade dönüşümü SV'de de birbirine yakın kalır;
Aşama 3'ün "SV bayt-aynı mı" sorusu kök konumdaki taşımalarda
cevaplanabilir.

### Karar 11: Blok `let`'i — süreç içi yerel değişken

- Her blok `let`'i, içinde bulunduğu `always_ff`/`always_comb`
  sürecinin başında **ayrı bildirim** (`logic [W-1:0] t;`) ve bildirim
  noktasında **blocking atama** (`t = …;`) olur. Bildirimde ilk değer
  YOK (§5.1 IMPLICITSTATIC, §5.2 Yosys hatası).
- Yerel içeren süreç adlı bloktur: `begin : on_<k>` / `begin : comb_<k>`
  (`k` modüldeki `on`/`comb` bloklarının kaynak sırası) — dalga formunda
  kararlı kapsam (§5.6). Adlı blok etiketi modül ad alanındadır;
  ADR-0081 ad ayırmasına girer.
- **Ad:** `let`'in adı; ama modül düzeyi bir SV adıyla (port, reg, tel,
  modül `let`'i, örnek çıkışı, fn açılım telleri, enum `localparam`'ı)
  ya da aynı süreçteki başka bir yerelle çakışırsa ADR-0081 Karar 12 kuralı:
  `<ad>_2`, `<ad>_3`… (VARHIDDEN ve §5.6'daki ad yakalama). `for` içindeki
  `let` her iterasyonda aynı yereli yeniden atar (sıralı anlam korunur).
- **E1013:** blok `let` adları `audit_module_names`'e girer (bugün
  `r02`'de yalnız çıktı metni taramasına kalıyor); kesin tanı `let`'in
  yerinde.
- `on` bloğunda yerel yazma sıfırlama dalının dışındadır: Volt'un
  `if (rst) … else begin <gövde> end` iskeletinde gövde sırası korunur.

**Reddedilen — A (modül teline taşıma):** `on` bloğunda eşdeğer
(§5.4, §5.5 rL/rH) ve modül düzeyi adla dalga formunda doğrudan görünür;
ama `comb`'da anlamı değiştiriyor ya da döngü kuruyor (§5.5 cH/kH). İki
blok türüne iki eşleme bakım ve kanıt yükünü ikiye katlar; tek eşleme
(yerel) iki blokta da doğru ve sentezde A ile aynı (§5.3).

## 6. Diğer katmanlarla etkileşim

- **Sürücü analizi (ADR-0073):** match ifadesi okumadır. Kök `case`
  biçimi hedefi kol başına yazar ama kaynakta tek atamadır; sürücü
  tablosu değişmez. Blok `let`'i sürücü değildir; ona atama E4001
  (Karar 6).
- **FSM oto kontrat (ADR-0066):** tanıma sözdizimseldir ve deyim
  `match`'ine bağlıdır. `s <= match s { … }` biçimi tanınmaz → kontrat
  üretilmez (güvenli yön: yanlış kontrat yok). Deyim biçimindeki FSM'ler
  etkilenmez. Tanımanın ifade biçimine genişlemesi gelecek iş.
- **Açılım bütçesi (ADR-0068 §6) ve derinlik (ADR-0080):** `for` içinde
  match ifadesi parser'ın `AstWriter` kapısından zaten sayılır; iç içe
  match'in Volt ağacı yüksekliği ADR-0080'le sınırlı. SV'ye özgü iki
  büyüme (üçlü zincir yüksekliği, sınanan kopyası) Karar 10.3/10.4 ile
  emitter doğrulamasında (ADR-0070 C seçeneği: `check`'te de).
- **Parite (ADR-0070):** yeni sondalar `tests/fixtures/parity/`'ye:
  E0014 ifade, E2003 kol, E3001 `comb` koşulu, E3009 muhafız, E4001 blok
  `let`, E1013 blok `let`, E0018 derin üçlü, E2027 sınanan kopyası.
- **Ayrılmış sözcükler (ADR-0078):** kök `case` biçimi yeni ad üretmez;
  üçlü zincir de. Yeni üretilen adlar: blok `let` yerelleri (kullanıcı
  adı — E1013 kapsamına alınır) ve adlı blok etiketleri `on_<k>`/`comb_<k>`
  (anahtar sözcük olamaz; çakışmada `_2`).
- **LSP:** ek yetenek yok; blok `let`'i hover'da tip (modül `let`'i gibi).

## Tanılar

Yeni kod yok.

| Kod | Değişiklik | Konum |
|---|---|---|
| E0014 | match **ifadesi** için de (sayısal `_`, enum kapsayıcılık); ileti bağlama göre "expression" | match |
| E2003 | "match arms have different types" (sentez kipi) | uyuşmayan kol |
| E3001 | `comb`'da koşul / sınanan / muhafız alanı (ADIM 2.1) | atama |
| E3009 | muhafız etiketi (ADIM 2.1) | atama / ifade |
| E3012 | `on`'da muhafız alanı | muhafız |
| E4001 | blok `let`'ine atama | atama |
| E1013 | blok `let` adı SV anahtar sözcüğü | `let` |
| E0018 | iç konumdaki üçlü zincir yüksekliği > 256 | match |
| E2027 | sınanan kopyası açılım bütçesini aşıyor | match |
| E0003 | kalır: muhafız, bağlama/struct/tuple deseni, test dili (E0001) | desen / kol |

## Reddedilenler

- **Yalnız üçlü (A):** §5.8 — Verilator 4096 kolda çöküyor, Yosys
  süper-doğrusal (4096 kol 705 sn); ROM tabloları bu yolda kullanılamaz.
- **Her zaman `case` (B):** iç konumlarda ara ad + ayrı süreç + sıralı
  yerleştirme (`else if` koşulu); kontratta kullanılamaz (§5.7).
- **Kol sayısı eşiğine göre biçim seçimi:** aynı kaynak kol eklendikçe
  biçim değiştirir (SV diff'i ve dalga formu kararsız); yere göre kural
  deterministik ve kaynaktan okunur.
- **Sınananı otomatik tele bağlama:** her match'e üretilmiş ad; blok
  içinde sıralı yerleştirme sorunu (§5.5). Maliyet bütçeyle sınırlandı.
- **Blok `let`'i modül teli (A):** §5.5 — `comb`'da yanlış donanım /
  döngü.
- **Bildirimde ilk değerli yerel:** §5.1/§5.2.
- **Aynı kapsamda yeniden bağlama (`let t = t + 1`):** spec §6 E1003;
  donanımda ad = sinyal ilkesi.
- **Blok gövdeli match ifadesi kolu:** gramerde yok; blok `let`'i ara ad
  ihtiyacını karşılar.

## Gelecek iş

1. Muhafız (`if` desen koşulu) deyim ve ifadede birlikte; akış kuralı
   ADIM 2.1'de hazır.
2. Aralık deseni `a..=b`, bağlama deseni, struct deseni.
3. Test dilinde `if`/`match` ifadesi ve fn çağrısı (ortak sabit
   yorumlayıcı).
4. ADR-0066 FSM tanımasının `s <= match s { … }` biçimine genişlemesi.
5. Uzun `else if` zincirli `if` ifadesinin kökte `if` deyimine inmesi
   (aynı §5.8 derinlik sınırı; bugün E0018 kuralı kapsar).

## Uygulama planı

### Aşama 2 — uygulama (dal `feat/match-expr-let`, main'den)

1. **ADIM 2.1 — örtük akış (önce, tek başına commit):** Karar 8 madde
   1-2 (domain `comb` `pc`'si, muhafız trust/domain). Önce/sonra tanı
   karşılaştırması (ADR-0081 ADIM 2.1'in golden betiği deseni):
   `examples/**`, `tests/ui/**`, `tests/fixtures/**`, gömülü Rust test
   kaynakları; yeni E3001/E3009 sınıflama tablosu (A/B) PR'da. SV
   golden'ı byte-aynı.
2. Parser: ifade `match`'inde sayısal E0014 (`parse_match_expr`, deyim
   kuralının paylaşılan yardımcısı); enum yolu ertelenir (ADR-0074
   Karar 4, `gated.rs` yedeği ifadeyi de yürür).
3. Typeck: `synth_match` / `check_match` (Karar 2, 3); E2003 iletisi;
   `structs/` indirgemesi; `consteval` `Match`.
4. Sürücü: blok `let`'ine atama E4001.
5. Emitter: Karar 10 (kök `case`, iç üçlü, yükseklik E0018, kopya
   maliyeti bütçeye — hesap volt-hir'de, `check` = `build` = LSP),
   Karar 11 (yerel bildirim, adlı blok, ad ayırma, E1013), fn `inline/`
   içinde match; `expr.rs:710` ve `lib.rs:1744` E0003'leri kalkar.
6. Tanılar: iki dil + `volt explain` (E0014 ve E2003 ifade örnekleri,
   E0018 match yardımı); parite sondaları (§6).
7. Spec (ADR kaynaklı): `grammar-full.ebnf` (MatchExpr yorumu: E0014,
   desen kümesi), `type-inference.md` (match kuralları), `domain-inference.md`
   K5/K6 (`comb` koşulu, muhafız), `name-resolution.md` §2 (blok `let`),
   `sv-mapping.md` (kök/iç, yerel), `const-eval.md` (`match`).
8. Testler — `tests/ui/pass/`: match ifadesi modül/`on`/`comb`/fn/
   kontrat/const, sayısal ve enum sınanan, beklenen tip itmesi, struct
   tipli sonuç, iç içe match, `for` içinde match; blok `let` `on`/`comb`/
   `if` dalı/`match` kolu/`for` gövdesi, gölgeleme (W1002 + yeniden
   adlandırma), `r <= …` sonrası okuma. `tests/ui/fail/`: kapsamsız
   (sayısal, enum), kol tip uyuşmazlığı, E3009 (muhafız + ifade), E3001
   (`comb` koşulu, ifade sınananı), kapsam dışı `let` (E1001), blok
   `let`'ine atama (E4001), E1013, E0018. `ui/pass` sayımı beş yerde.
   Simülasyon (`volt test`, Docker) ve formal (`volt verify`, Docker,
   boolector) — kontratta match ifadesi, kasıtlı hatayla. Çıktı ağı
   (ADR-0079): yeni `ui/pass` dosyalarının SV'si Verilator `-Wall` +
   Yosys. Yosys `equiv`: deyim match ↔ ifade match (kök ve iç biçim).
   §5.5'in `comb` karşı örneği ui/pass + simülasyon testi olarak kalır.
9. Mutasyon (tek tek, `CARGO_BUILD_JOBS=2`, `--test-threads=2`):
   örtük akış (`comb` `pc` / muhafız join'i) kaldır → E3001/E3009 testi
   düşmeli; kapsayıcılık denetimini kaldır → E0014 testi düşmeli;
   `let` kapsam kuralını boz (dal `let`'i dışarı sızsın) → E1001 testi
   düşmeli; yerel yerine tel → `comb` karşı örneği düşmeli; yeniden
   adlandırmayı kaldır → gölgeleme testi (Verilator) düşmeli.
10. Golden: PR #49 sonrası referans; yeni yapıları kullanmayan her
    tasarımın SV'si byte-aynı; ADIM 2.1 yalnız tanı ekler.

ADR'den sapma gerekirse (örn. kök konum kuralının bir bağlamda
uygulanamaması) uygulama durur ve ADR güncellenir.

### Aşama 3 — örnekler ve kanıt (dal `feat/match-examples`)

`examples/riscv_core.volt`: ALU işlemi ve dallanma koşulu fn + match
ifadesiyle (ADR-0081'de ertelenen taşıma); fn'lerin yeri (ortak
`riscv_imm.volt` mı ayrı `riscv_alu.volt` mı) Aşama 3'te karar;
`riscv_pipeline` aynı fn'leri kullanabiliyorsa kullanır. Başka adaylar
yalnız okunabilirliği artırıyorsa; donanımı değiştiren taşıma yapılmaz.
Kanıt ADR-0077/0081 yöntemiyle: `volt test` (riscv_core 59/59 ve
diğerleri), `volt verify` (boolector), Verilator `-Wall`, Yosys
`equiv_make`/`equiv_induct` eski ↔ yeni (tüm hücreler + kasıtlı hata),
Yosys `stat` karşılaştırması, mümkünse SV bayt karşılaştırması. README
"Limitations" ve CHANGELOG.

## Aşama 2 — uygulama notları (2026-09-27, dal `feat/match-expr-let`)

Karar 1-11 uygulandı; yeni tanı kodu yok. Aşağıdaki maddeler kararın
uygulamada netleşen biçimidir (anlam değişmedi) ya da ölçümle bulunan
bir sınırdır. Golden referansı PR #50 sonrası `main` (36fd93d; PR #50
yalnız belge, kod PR #49 ile aynı). Araçlar `build/m2/` (gitignore'da):
`golden.py` + `golden_cmp.py` + `golden_diff.py`, `corpus.py` (gömülü
Rust test kaynakları), `p21/gen.py` (ADIM 2.1 taraması), `t/gen.py`
(emitter sondaları), `sv/` (Verilator + Yosys), `eq/` (eşdeğerlik),
`sim/` (volt test + verify), `mutate.py`.

### ADIM 2.1 — örtük akış (ayrı commit 93db141)

Karar 8 madde 1-2: `domain/walk.rs` `on` dışındaki blokları koşul alanı
(`pc`) ile yürür; `if` koşulu, deyim `match`'inin sınananı ve kol
muhafızı `pc`'ye `join` edilir (iç içe koşulların karışması da E3001),
atamanın sağ tarafı `pc` ile birleşir, K6 E3001'i verir. Sağ taraf sabitse
ikincil etiket koşulu gösterir. `on` bloğunda aynı yol E3012'dir (değişmedi).
Muhafız: trust `walk_block` (kolun `pc`'si) ve `expr_tag`/`match_domain`
(ifade sonucu).

Tarama (`build/m2/p21/gen.py`, önce = `main`, sonra = 93db141):

| Sonda | Önce | Sonra |
|---|---|---|
| `comb { for … { if fs { y = sa } … } }` | temiz | E3001 |
| `comb { if sb { y = sa } else if fs { … } }` | temiz | E3001 |
| `comb { if sb { if fs { … } } }` (iç içe) | temiz | E3001 |
| `comb { match fv { 0 => { y = sa }, … } }` | temiz | E3001 |
| `comb { y = 0; if fs { y = 1 } }` | temiz | E3001 |
| `comb { match sa { 0 if fs => … } }` | E0003 | E3001 |
| `on sclk { match sa { 0 if fs => … } }` | E0003 | E3012 |
| `y = match sa { 0 if fs => 1, _ => 0 }` | E0003 | E3001 |
| muhafız `key[0]` (deyim / ifade / comb) | E0003 | E3009 |
| `comb { if sb { y = sa } … }` (aynı alan) | temiz | temiz |
| modül `for` içinde `if` | E0003 | E0003 (yalnız ifade biçimi var) |

Sınıflandırma: önce/sonra golden 1966 dosya — `tests/ui`,
`tests/fixtures`, `examples`, `tests/fuzz_regressions`, gömülü Rust test
kaynakları (`build/m2/corpus.py`: `module` içeren 1358 dizge sabiti) ve
`build/domain_corpus.py`'nin 88 iki saatli kaynağı. **Tanı ve SV çıktısı
bayt-aynı: yeni E3001 / E3009 yok**; A (gerçek CDC) / B (yanlış alarm)
tablosu boş, kural daraltılmadı.

### Uygulamada netleşenler

1. **E0018 / E2027 emitter'da** (§6 "emitter doğrulamasında"). Uygulama
   planı madde 5'teki "hesap volt-hir'de" ifadesinin amacı `check = build
   = LSP` idi; ADR-0070 ortak boru hattı emit'i üçünde de koştuğu için
   hesap, konumu kesin bilen emitter'da (`match_expr.rs`
   `check_ternary_limits`). Yalnız en dıştaki iç match denetlenir (tek
   tanı). Yükseklik: iç match kol sayısı kadar derin; kopya: sınanan
   düğümü × desen literali (iç içe match kendi kopyasıyla).
2. **Kısmi hedefli modül ataması iç biçimde.** `y[3:0] = match …` üçlü
   zincirle `assign` kalır: aynı sinyalin öteki parçaları `assign` ile
   sürülürken bir parçayı `always_comb`'a almak değişkeni iki süreç
   türüne böler. Kök `case` bütün sinyale atamada.
3. **`comb` yerelleri süreç başında sıfırlanır.** `comb { if c { let t =
   a + b; y = t } else { y = b } }` ilk uygulamada Yosys 0.66'da
   `ERROR: Latch inferred for signal '\M.\comb_0.t' from always_comb
   process`, Verilator 5.050'de `%Warning-LATCH` verdi (dal içindeki
   yerel her yolda atanmıyor). Süreç başına `t = 8'd0;` eklendi — değer
   hiç okunmaz (`let` bildiriminden önce görünmez; Karar 7 değişmez).
   Bildirimde ilk değer değil (Karar 11, §5.1/§5.2). `always_ff`'te
   gerekmiyor (`l07`, `n10`: iki araç temiz).
4. **Adlı blok etiketi türe göre sayılır:** `on_<k>` k'ıncı `on` bloğu,
   `comb_<k>` k'ıncı `comb` bloğu (kaynak sırası). Yerel içermeyen
   süreç adsız kalır — yeni yapıyı kullanmayan tasarımın SV'si değişmez.
5. **İfadede ilk joker koldan sonrası yazılmaz.** Deyim biçimi (ADR-0032)
   değişmedi. Kapsayıcı `_`'sız enum'da son adlı kol `default` / zincirin
   son `else`'i (Karar 4); W2014'lü kol yazılmaz (deyimle aynı
   `match_cover` kuralı, artık kollar üzerinden).
6. **Yerel ad çakışması kümesi:** sembol tablosu (port, reg, tel, modül
   `let`'i, fn açılım telleri), örnek adları, `<örnek>_<port>` çıkış
   telleri, `<Enum>_<Varyant>` `localparam`'ları ve süreçteki yereller.
7. **Sabit match iki katmanda:** HIR `consteval` (tip/genişlik bağlamı)
   ve emitter'ın kendi sabit katlayıcısı (SV'ye literal). Bulgu (bu işin
   dışında, düzeltilmedi — golden kuralı): emitter katlayıcısı `if`
   ifadesini değerlendirmiyor; `const N : u32 = if true { 20 } else { 3 }`
   bugün `assign z = N;` (tanımsız ad) üretiyor. Gelecek iş 6.
8. **Match'i bilmeyen gezginler:** SVA modülünün port toplayıcısı
   (`sva.rs`), `prev()` toplayıcısı (`past.rs`), SDC saat izleme
   (`constraints/walk.rs`) ve `enum_of_expr` `Match`'i görmüyordu —
   kontrattaki match'in adları bağlanan SVA modülüne port olmuyordu
   (ölçüldü: `m09` `m_sva (clk, rst)`); dördü de alt ifadeleri gezer.
9. **Kontrat kol sınırı (Aşama 1 notu 4):** ek sınır ya da uyarı yok.
   Kontrat iç konumdur; E0018 (256 kol) orada da geçerli ve §5.8'de 256
   kol iki araçta da güvenli (Verilator 92 ms, Yosys 264 ms). Daha büyük
   seçim modül `let`'ine yazılıp kontratta adıyla kullanılır.
10. **Eski beklentisi değişen testler** (ADR kararının sonucu, silme yok):
    `parser_tests::match_exhaustiveness_not_checked_in_parser` →
    `match_expr_missing_wildcard_is_e0014` (Karar 3); resolve testlerinde
    `match x { n => n }` `_` kolu aldı, enum desenli sınanan enum tipli
    oldu; E0003 örneği olarak match kullanan fixture'lar muhafıza geçti
    (`ui/fail/160`, `parity/fn11`, `p42`, `p43`, iki W5001 testi —
    niyetleri korundu); `parity/d13` (blok `let`'ine atama) `drivers: ok`
    → E4001 11/10 (Karar 6).

### Ölçümler

- **Golden:** önceki 1966 dosyanın 1950'si bayt-aynı; farklı 16'nın hepsi
  match ifadesi ya da blok `let`'i kullanıyor (E0003 → temiz/yeni tanı;
  `build/m2/golden_diff.py`).
- **Araçlar** (`build/m2/sv/`): 33 sonda tasarımı (m1 sondaları + 20 yeni)
  Verilator 5.050 `--lint-only -Wall` (UNUSED dışında) temiz, Yosys 0.66
  `prep; check -assert` temiz, mandal yok.
- **Çıktı ağı** (ADR-0079, `volt-net` Docker, `VOLT_REQUIRE_TOOLS=
  verilator,yosys`): 13/13. İlk koşuda tek bulgu fixture'ın kendi
  kullanılmayan biti idi (`typed[8:1]`), düzeltildi.
- **Simülasyon** (`build/m2/sim/mx_sim.volt`, Docker `volt test`): 2/2 —
  kök/iç match, fn tel kipi, enum son kol, `on`'da eski register değeri,
  `comb`'da §5.5 karşı örneği (`z == 0`).
- **Formal** (`volt verify`, boolector): 6 kontrat prove ve bmc kanıtlı;
  kontrattaki match'in kolu bozulunca `MxSim.inv_4 E5001 contract violated
  at cycle 2`.
- **Eşdeğerlik** (Yosys 0.66, `build/m2/eq/`; RV32I ALU 10 işlem + 5 kollu
  enum dallanma + `on`'da ALU): deyim biçimi S, kök ifade E1 (`let alu =
  match`, blok `let s = match`), iç ifade E2 (`if k { match … }`,
  `(match …) ^ a`):

  ```
  S  <-> E1: rc=0   Of those cells 97 are proven and 0 are unproven.
  S  <-> E2: rc=0   Of those cells 97 are proven and 0 are unproven.
  E1 <-> E2: rc=0   Of those cells 97 are proven and 0 are unproven.
  S  <-> Em: rc=1   Of those cells 66 are proven and 31 are unproven.   (SRA → SRL)
  E2 <-> Em: rc=1   Of those cells 66 are proven and 31 are unproven.
  ```

### Mutasyon (tek tek, `CARGO_BUILD_JOBS=2`, `--test-threads=2`; `build/m2/mutate.py`)

| # | Kaldırılan koruma | Düşen test |
|---|---|---|
| M1 | comb koşulunun alanı atamaya katılmaz (ADIM 2.1) | `domain_tests`: comb_constant_under_a_foreign_condition_is_e3001, comb_for_body_carries_the_condition, comb_if_condition_from_a_foreign_domain_is_e3001, comb_match_scrutinee_from_a_foreign_domain_is_e3001 |
| M2 | muhafız güven etiketine katılmaz (deyim + ifade) | `trust_tests`: implicit_flow_through_a_match_guard_is_e3009 |
| M3 | ifade match'inde sayısal E0014 yok (parser) | `parser_tests`: match_expr_missing_wildcard_is_e0014 |
| M4 | ifade match'inde enum kapsayıcılığı yok (typeck) | `match_expr_tests`: every_fail_fixture_reports_its_code_on_the_marked_line |
| M5 | blok let'i dal dışına sızar (çözümleme kapsamı) | `match_expr_tests`: every_fail_fixture_reports_its_code_on_the_marked_line, shadowing_block_let_is_renamed_in_sv |
| M6 | emitter yerel kapsamı kapanmaz | `match_expr_tests`: shadowing_block_let_is_renamed_in_sv |
| M7 | yerel yerine tel anlamı (let bloğun son değerini okur) | `match_expr_tests`: block_let_is_a_process_local_in_a_named_block, comb_block_let_keeps_the_value_at_its_declaration, shadowing_block_let_is_renamed_in_sv |
| M8 | gölgeleyen yerel yeniden adlandırılmaz | `match_expr_tests`: shadowing_block_let_is_renamed_in_sv |
| M9 | blok let'ine atama E4001 değil (aynı sürücü grubu) | `match_expr_tests`: every_fail_fixture_reports_its_code_on_the_marked_line |
| M10 | blok let adı E1013 denetimine girmez | `match_expr_tests`: every_fail_fixture_reports_its_code_on_the_marked_line |
| M11 | iç match yükseklik/kopya sınırı yok | `match_expr_tests`: every_fail_fixture_reports_its_code_on_the_marked_line |
| M12 | comb yerelleri süreç başında sıfırlanmaz | `match_expr_tests`: block_let_is_a_process_local_in_a_named_block |
| M13 | kök konum case değil (her yer üçlü) | `match_expr_tests`: contract_match_lowers_to_a_ternary_in_sva, every_fail_fixture_reports_its_code_on_the_marked_line, expected_type_is_pushed_into_every_arm_and_structs_split_per_field, match_in_a_function_body_follows_the_call_mode, match_in_blocks_repeats_the_target_per_arm |
| M14 | enum geçersiz kodları son kola gitmez (son kol etiketli) | `match_expr_tests`: match_in_blocks_repeats_the_target_per_arm, whole_right_hand_side_is_a_case_and_an_operand_is_a_ternary |
| M15 | match ifadesi kol tipleri denetlenmez (E2003) | `match_expr_tests`: every_fail_fixture_reports_its_code_on_the_marked_line |
| M16 | yerel anahtarı yalnız span (struct yaprakları tek yerele düşer) | `match_expr_tests`: struct_block_let_splits_into_one_local_per_leaf |

16/16 yakalandı (M16, uygulama sırasında bulunan struct yaprağı hatasının — iki yaprak aynı bildirim span'ini paylaşıp tek yerele düşüyordu — regresyon testi). Her koşudan sonra kaynak geri yüklendi (`git diff --stat crates` değişmedi).

### Aşama 1 sondalarının yeniden koşusu (Aşama 1 notu 5)

`build/m1/gen.py` uygulamayla (§1-§2 tabloları):

| Sonda | Aşama 1 | Aşama 2 |
|---|---|---|
| `m01`-`m05`, `m08`, `m08b`, `m10`, `m15` | E0003 | temiz, build rc=0 |
| `m06` (sayısal, `_` yok) | yalnız E0003 | E0014 "'match' expression has no '_' arm" |
| `m07` (u8/bool kol, beklenen u8) | yalnız E0003 | E2003 (check kipi, kola itilen tip) |
| `m14` (enum eksik) | yalnız E0003 | E0014 "missing Op::And, Op::Or" |
| `m13` (bağlama `x => a`) | E0003 | E0014 (bağlama joker sayılmaz — deyimle aynı) |
| `m12` (muhafız) | E0003 | E0003 "'match' arm guards" |
| `m09` (kontrat) | `--emit=sva`'da E0003 | SVA'da üçlü, bağlanan modül portları tam |
| `m11`, `m16`, `t10`, `z01` | E0001 | E0001 (gramer değişmedi) |
| `l01`, `l02`, `l04`, `l05`, `l07`-`l09` | E0003 | temiz; `l04`/`l07` W1002 + SV'de `a_2`/`t_2` |
| `l03` | E1001 | E1001 |
| `l06` (blok `let`'ine atama) | yalnız E0003 | E4001 |
| `t01`-`t11`, `t06`/`t06b` (muhafız) | t06/t06b E0003 | hepsi E3009 |
| `d06`, `x03`, `x04` (`comb` koşulu) | temiz | E3001 |
| `d01`, `d02`, `x08` | E3012 | E3012 |
| `e04` (sınanan gecikmeli) / kollar farklı gecikme | E0003 | temiz / E5010 (ADR-0037 kuralı) |

4096 kollu kök match (`let v = match op { 0 => a ^ 0, … }`): `volt build`
363 ms, Verilator 5.050 `-Wall` 0,34 sn (walltime), Yosys 0.66 `prep`
~2,7 sn (Docker açılışı dahil) — §5.8 `deepc4096` ile tutarlı; aynı match
operand konumunda E0018.

### Gelecek iş (Aşama 2 eki)

6. ~~Emitter'ın sabit katlayıcısında `if` ifadesi (`const N = if … `
   bugün SV'de tanımsız ad bırakıyor; HIR `consteval` doğru hesaplıyor).~~
   Aşama 3'te kapandı (aşağıda).

## Aşama 3 — örnekler ve kanıt (2026-09-27, dal `feat/match-examples`)

### Taşınanlar ve kararlar

- **Yeni `examples/riscv_alu.volt`:** `alu_of(f3, is_r, alt, a, b) -> u32`
  ve `branch_taken(f3, a, b) -> bool`, ikisi de `funct3` üzerinde `match`.
  Ayrı dosya kararı: `riscv_imm.volt` tek iş yapar (immediate çözme) ve
  `riscv_pipeline` bu iki fn'i kullanamaz (aşağıda) — ortak dosyada
  birleştirmenin bir kullanıcısı yok. `riscv_core` ikisini de `use` eder.
- **`riscv_core.volt`:** ALU ve dallanma koşulu fn çağrısı; ayrıca aynı
  biçimdeki dört doğal aday (bir kod üzerinde değer seçimi): `sh_ok`
  (`f3`), `csr_rdata` (`csr_addr`, 12 kol), `csr_wdata` (`csr_op`),
  `load_val` (`f3`). Taşınmayanlar farklı boole'lar arasında öncelik
  seçimi — `match` değil: `exc_cause`, `wb_val`, `alu_b`. Kaynak 524 → 509
  satır (+ `riscv_alu.volt` 38).
- **`riscv_pipeline.volt`:** yalnız `alu_op` `match` oldu. `alu_of` /
  `branch_taken` kullanılamaz: pipeline'ın ALU'su alt küme (ADD/SUB, XOR,
  OR, AND; `f3` 1/2/3/5 AND'e düşer), dallanması yalnız BEQ/BNE — ortak
  fn donanımı değiştirirdi.
- **Diğer örnekler** (`uart_tx`, `i2c`, `soc/timer`): değer üreten `else
  if` zincirleri farklı boole'lara öncelik veriyor (`tick`, `stalling`,
  `rw_r`, `enable_r`) — `match`'e taşınmadı.

### Aşama 2 notu 1 — `const` başlangıcında `if` (Gelecek iş 6, kapandı)

HIR `consteval` `if`/`match`'i hesaplıyordu; SV üreticisinin katlayıcısı
(`expr.rs` `eval_const`) yalnız `+ - * /`, dizi elemanı ve `match`
biliyordu. Katlanamayan skaler `const` çıplak adıyla basılıyordu — SV'de
bildirilmediği için tanımsız ad ("Volt tamam, çıktı geçersiz"). Ölçüm
(`build/m3/probe/`, önce → sonra):

| Sonda | Önce | Sonra |
|---|---|---|
| `c1` `const N : u32 = if true { 20 } else { 3 }` | `assign z = N;` | `assign z = 32'd20;` |
| `c2` `bits<M>`, `M = if W > 4 && !(W == 6) {…}` | E2005 ×2 (genişlik) | `[15:0]` portlar |
| `c3` `const F : bool = 3 > 2`, `if !F … else if F -> …` | `assign b = F;` / `= Q;` | `1'd1` / `8'd2` |
| `c4` `const S : u32 = 1 << 3` | `assign z = S;` | `32'd8` |
| `c5` `const S : u32 = 5 as u32` | `assign z = S;` | E0003 "constant 'S' whose value the SystemVerilog emitter cannot fold" |

Düzeltme: katlayıcı `if`, bool literali, karşılaştırmalar, `&&`/`||`/`->`/
`!`, `%` ve negatif olmayan değerde `& | ^ << >>` hesaplar (HIR ile aynı
seçim; koşul çözülemezse `None`). Hâlâ katlanamayan skaler `const` değer
olarak kullanılınca **E0003** alır (`future()`; `check` = `build`,
ADR-0070) — tanımsız ad bir daha yazılmaz. `volt explain E0003` iki dilde
güncellendi. `tests/ui/pass/127_const_if_match.volt` çıktı ağına
(ADR-0079) girer: `volt-net` Docker, Verilator + Yosys 13/13.

Yan bulgu (aynı düzeltmeyle kapandı): `enum S : u4 { A = K, B = 1 << 3 }`
düzeni hesaplanamayınca portlar **1 bit** iniyordu, tanısız
(`build/m2/corpus/sim_f8bcd864a2.volt`: `input logic cmd` → `input logic
[3:0] cmd`). Test: `enum_value_with_shift_keeps_declared_width`.

Golden (PR #51 sonrası `main` ↔ bu dal, `build/m3/golden.py`: check insan
+ JSON + build SV/SVA/SDC): önceki 2004 dosyanın 1994'ü bayt-aynı. Farklı
10: 5 sonda, yukarıdaki enum korpus dosyası ve taşınan örnekler
(`riscv_core`, `riscv_core_test`, `riscv_sw/hello_soc` — `RiscvCore`'u
içerir — ve `riscv_pipeline`).

Mutasyon (tek tek, `build/m3/mutate.py`): `if` katlaması kaldırıldı →
`const_if_expression_folds_to_literal` ve `const_condition_forms_fold`
düştü; E0003 kaldırıldı → `unfoldable_const_is_explicit_e0003` düştü. 2/2.

### Üretilen SV

`RiscvCore.sv` 264 → 344 satır, **bayt-aynı değil**: altı blok iç içe
üçlüden `always_comb` + `case`'e iner (Karar 10, atamanın tüm sağ tarafı).
`alu_of`/`branch_taken` tel kipinde açılır (ADR-0081: `alu_of_0_a` …
parametre telleri, sonuç `alu_out` doğrudan `case` hedefi); `shamt` ve
`alt_op` telleri fn içine girdiği için kalktı. `UartTx.sv` bayt-aynı.
`RiscvPipeline.sv` 214 → 224 satır (yalnız `alu_op`).

### Eşdeğerlik kanıtı

1. **Modüler (Yosys 0.66 `equiv_simple`, `build/m3/blocks/gen.py`):** fark
   tam altı blok; bloklar konumla çıkarılınca kalan 244 satır iki dosyada
   birebir (`// Source:` ve boş satırlar hariç). Her blok serbest girişli
   ayrı modülde (girişler iki tarafta aynı adlı, değişmemiş teller):
   `sh_ok` 2/2, `alu_out` 64/64, `br_taken` 2/2, `csr_rdata` 64/64,
   `csr_wdata` 64/64, `load_val` 88/88 `$equiv` hücresi, toplam 1,6 sn.
   Kasıtlı hata (her bloğa bir: kol kodu ya da işleç) **6/6 yakalandı**
   (`alu_out` `^`→`|` 64 kanıtsız, `csr_rdata` `12'hB02`→`12'hB03` 64,
   `load_val` `3'd4`→`3'd6` 32, …).
2. **Tam çekirdek (ABC `dprove`, `build/m3/cec/seq.ys`):** eski ve yeni
   `RiscvCore`+`UartTx` → `prep; memory_map; flatten; async2sync;
   dffunmap` → `miter -equiv` → `techmap; dffunmap; aigmap; setundef
   -zero -init` → AIGER; `yosys-abc "dprove"`: 67 giriş, 3012 latch, 65564
   AND — **"Networks are equivalent", 32 sn** (latch eşlemesi 3979 →
   1917, fraig). Kasıtlı hata (`alu_out` `^`→`|`): `bmc3 -F 8` **2.
   çerçevede karşı örnek** (118 sn; yükle → XOR → sakla), `dprove` 631
   sn'de UNDECIDED (kanıtlamadı). x sabitleri iki tarafta aynı (827'şer,
   hepsi değişmemiş `+:` parça seçiminin `$shiftx` eşlemesi); `setundef`
   bu yüzden simetrik. İlk denemede `write_aiger -zinit` başlangıcı
   olmayan her FF'ye ayrı giriş ekledi (3079 giriş) → iki kopya farklı
   durumdan başladı, 0. çerçevede "not equivalent"; Yosys `sat -seq 1
   -set-init-zero` aynı miter'da SUCCESS verdi, `-init` ile düzeldi.
3. **ADR-0081 yöntemi tam çekirdekte bitmedi (ölçüm):** `equiv_make` +
   `equiv_simple -seq 1` + `equiv_induct -seq 1` (ADR-0081 Aşama 3'te SV
   bayt-aynı olduğundan saniyeler sürmüştü) `alu_out[21]`de; `-seq 1`'siz
   (`eq2`) aynı bitte, `equiv_struct` + `equiv_simple -short` (`eq3`)
   `alu_out[3]`te ~24 dk ilerlemedi, durduruldu. Aynı ALU bloğu serbest
   girişle 1 sn'nin altında kanıtlanıyor: tıkanma ALU'nun kendisinde
   değil, konisindeki register dosyası okuma mux'u (32×32) üstünde.
   Bu yüzden asıl kanıt 1 + 2.
4. **`riscv_pipeline` (Yosys, ADR-0081 betiği):** 2103/2103 `$equiv`,
   2,7 sn; kasıtlı hata (`3'd4` `^`→`|`) 32 kanıtsız hücre.

### Diğer kanıtlar

- **`volt test`** (Docker, taze Linux derlemesi): `riscv_core` 59/59
  (C programı dahil), `riscv_pipeline` 15/15, `uart_tx` 4, `axi4lite_slave`
  5, `fir_filter` 10, `soc` 5, `i2c` 12, `vga` 7, `hybrid_accel` 6 — hepsi
  geçti. Not: `volt-rustup` biriminde araç zinciri yoktu, ilk koşu bayat
  ikiliyle E0003 verdi; `build/m3/docker_test.sh` artık gerekirse
  `rustup default stable` yapıyor.
- **`volt verify`** (boolector, `-j 4`, önce ↔ sonra aynı):
  `riscv_core` prove 3 44/44, bmc 10 44/44, cover 12'de 2 başarısız
  (aynı ikisi önce de: `UartTx.cov_9`, `RiscvCore.cov_4` 12 adıma sığmaz);
  `riscv_pipeline` prove/bmc/cover 14/14.
- **Verilator 5.050 `--lint-only -Wall`:** `RiscvCore`+`UartTx` ve
  `RiscvPipeline` 0 uyarı.
- **Yosys `stat`** (FF, CARRY, DSP birebir; fark yalnız seçme mantığı):

  | | önce | sonra |
  |---|---|---|
  | `RiscvCore` iCE40 `SB_LUT4` | 7408 | 7370 |
  | `RiscvCore` xc7 LUT1-6 | 3169 | 3096 |
  | `RiscvCore` xc7 MUXF7 / MUXF8 | 724 / 219 | 723 / 205 |
  | `RiscvPipeline` iCE40 `SB_LUT4` | 2815 | 2811 |
  | `RiscvPipeline` xc7 LUT1-6 | 1630 | 1524 |
  | `RiscvPipeline` xc7 MUXF7 / MUXF8 | 143 / 35 | 140 / 4 |

  Açıklama: aynı mantık fonksiyonu farklı başlangıç yapısından eşlenir —
  `case` Yosys `proc`'ta tek `$pmux` (sabit kod karşılaştırmaları tek
  seçicide), üçlü zincir sıralı `$mux` zinciri; ABC teknoloji eşlemesi
  farklı ağdan farklı LUT kümesi bulur. Register ve aritmetik hücreleri
  aynı; eşdeğerlik yukarıda kanıtlı. Fark iki tasarımda da küçülme
  yönünde, ama iki tasarımda ölçüldü — genel bir iddia değil.
- `cargo test --all`: 3015 geçti, 0 başarısız; tutarlılık 150 kod, 3292
  test (baseline güncellendi); `ui/pass` sayımı 113 (dört assert).
