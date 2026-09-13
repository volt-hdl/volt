# ADR-0041: Aritmetik Ergonomisi — Aynı-İşaret Genişleme, Const Diziler, Generic Örnekleme

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-13
> Etkilenen: type-inference.md §5, const-eval.md §8, sv-mapping.md §2/§5/§9/§16,
> grammar-full.ebnf (PrimitiveType), ast-nodes.md (TypeRefKind),
> volt-ast (`TypeRefKind::UIntN/SIntN`), volt-syntax (parser `uint<N>`/`sint<N>`,
> `parser/mono.rs` monomorfizasyon), volt-hir (typeck.rs check modu itme,
> expect_assignable), volt-sv-emit (let genişliği, const dizi, `for` açma,
> `comb`/`wire`, kullanıcı modülü örnekleme, boyut dönüşümü `W'(x)`),
> examples/fir_filter.volt, tests/ui/pass/50-52, tests/ui/fail/39-40

## Sorun

`examples/fir_filter.volt` keşfi (rapor 6a-6g) üç ergonomi açığı gösterdi:

1. **Örtük genişleme yasağı** (type-inference.md §5) 8 tap'lık bir FIR'da
   20'den fazla elle `as i32` cast'i gerektiriyor. `let p : i32 = t * c`
   gibi hedefi AÇIKÇA yazılmış bir bağlamda bile `t : i16` reddediliyordu;
   üstelik emitter anotasyonu değil `max(lhs, rhs)` genişliğini kullandığı
   için cast'siz yazım sessizce 16 bitlik çarpan üretiyordu.
2. **Const dizi** (`const COEFFS : [i16; 8] = [...]`) tip denetiminden geçiyor
   ama SV'ye çıplak isim olarak sızıyordu; kullanıcı 8 ayrı skaler const +
   `match` yazmak zorundaydı.
3. **Generic modül örnekleme**: `module FirFilter<const TAPS: u32, ...>` ayrışıp
   çözümleniyor, ama tip konumunda E2021 alıyor ve `FirFilter<8, 16> { ... }`
   örneklemesi kullanıcı modülleri için üretilemiyordu (E0003). Kullanıcı
   modülü örneklemesinin SV üretimi de yoktu.

## Karar

### 1. Aynı-işaret örtük genişleme — hedef tip açıkça yazılmışsa

type-inference.md §5 atanabilirlik kuralı ŞÖYLE değişir:

```
kaynak S(a bit), hedef T(b bit), hedef tip AÇIKÇA yazılmış:
  işaret aynı ve b >= a   → örtük genişleme İZİNLİ
  işaret aynı ve b <  a   → E2001 (daraltma; değişmedi)
  işaret farklı           → E2002 (değişmedi)
```

"Açıkça yazılmış" = check moduna giren her bağlam: `let x : T`, `reg x : T`,
`wire x : T`, port tipi (atama hedefi ve port bağlama), `const x : T`.
Anotasyonsuz `let c = a + b` (a: i18, b: i19) synth modundadır ve E2001
vermeye devam eder.

**Beklenen tip aşağı itilir.** `let c : i19 = a + b` ifadesinde `a + b`
önce i19'a genişletilir, sonra toplanır — sonuç taşımasız ve tam. Kural:
check modunda `+ - * / %` ikili işleminin operandları sentezlenir; literal
olmayan her operand int tipliyse, işareti beklenen tiple aynıysa ve doğal
genişliği beklenen genişliğe sığıyorsa, işlem beklenen tipte yapılır
(`arith_result(op, T, T)`). Aksi halde (Trit, bits<N>, operand hedeften
geniş, işaret farklı) eski synth yolu ve eski tanılar geçerlidir.

| Yazım | Sonuç |
|---|---|
| `let acc : i19 = a + b` (a, b: i18) | OK — i19'da toplama |
| `let c : i20 = a18 + b19` | OK — her operand i20'ye |
| `let c = a18 + b19` | E2001 (anotasyon yok) |
| `let x : i32 = y` (y: i16) | OK |
| `let z : u16 = w` (w: i16) | E2002 |
| `let v : i8 = u` (u: i16) | E2001 |
| `let p : i32 = t * c` (t, c: i16) | OK — 32×32 çarpan |

İlke korunuyor: genişleme bedava değildir, ama hedef tipi yazan kullanıcı
maliyetin farkındadır. Bit düzeyi (`& | ^`), kaydırma ve karşılaştırma
kuralları değişmedi.

**SV üretimi açık genişletme basar.** SystemVerilog'un bağlam-belirlenimli
genişliği aynı sonucu verir ama Verilator `-Wall` WIDTHEXPAND uyarır.
Emitter aritmetik operandları etkin bağlamla (`max(ifade genişliği, hedef)`)
basar; bağlamdan dar ATOM operandlar boyut dönüşümüyle sarılır:
`wire signed [31:0] p = 32'(t) * 32'sd3;`. Boyut dönüşümü işaretli sinyalde
işaret, işaretsizde sıfır genişletir; Verilator ve Yosys destekler. Aynı
dönüşüm bileşik ifade cast'lerinde (`(a + b) as i32` → `32'(a + b)`) E2005
"yalnız basit sinyal" kısıtını kaldırır. `let` teli her zaman anotasyon
genişliğiyle bildirilir (eski `max(lhs, rhs)` davranışı kaldırıldı).

### 2. Const diziler

`const COEFFS : [i16; 8] = [1, 2, 3, 4, 4, 3, 2, 1]` (ve `[v; N]`) tip
denetiminden zaten geçiyordu (ConstValue::Array); eksik olan SV üretimiydi.

| Erişim | SV |
|---|---|
| `COEFFS[k]`, k derleme zamanı sabiti (literal, sabit ifade, açılmış döngü değişkeni) | eleman literale katlanır: `16'sd3` |
| `COEFFS[idx]`, idx sinyal | modül başında tablo işlevi `function automatic logic signed [15:0] COEFFS_at(input logic [2:0] i) ... case ... endfunction` ve `COEFFS_at(idx)` |
| çıplak `COEFFS` reg başlatıcısında | eleman atamaları (`taps[0] <= 16'sd1; ...`) |
| çıplak `COEFFS` başka konumda | E2005 (sessiz sızıntı yok) |

Değişken indeks biçimi: tablo işlevi (`function ... case`) seçildi; ölçüm
ve gerekçe "Ölçüm notu" bölümünde. Alternatif unpacked `localparam` dizisi
emitter'da `ConstArrayStyle::LocalparamArray` ile seçilebilir durumda
tutulur (`emit_full_opts`).

Negatif sabitler (`const C : i16 = -2`) artık `(-16'sd2)` olarak katlanır;
emitter sabit değerlendirmesi i128'e taşındı.

### 3. Generic argümanlı kullanıcı modülü örnekleme — monomorfizasyon

```volt
module FirFilter<const TAPS: u32, const WIDTH: u32> { ... }
let f8 = FirFilter<8, 16> { clk, sample, valid_in }
let f4 = FirFilter<4, 16> { clk, sample, valid_in }
```

Her farklı argüman demeti için modül şablonu KLONLANIR ve parametreler
literale ikame edilir: `FirFilter_8_16`, `FirFilter_4_16`. Aynı argümanlarla
ikinci örnek aynı modülü kullanır. Şablonun kendisi, en az bir kez
örneklendiyse, öğe listesinden çıkar (SV'de generic modül yoktur); hiç
örneklenmemiş generic modül dokunulmadan kalır ve tip konumunda bugünkü
gibi E2021 üretir.

**Katman:** geçit `volt-syntax/parser/mono.rs`'tedir ve pipeline desugar'ından
(ADR-0038) hemen sonra koşar. Gerekçe: isim çözümleme, tip denetimi, alan
çıkarımı, zamanlama ve SV üretimi tümüyle AST düğümlerine anahtarlıdır; klon
AST düzeyinde yapılınca sonraki her geçit somut modül görür, hiçbir geçit
"generic bağlam" taşımak zorunda kalmaz. (HIR seviyesinde bir ikame ortamı,
her geçidin sonuç tablolarını örnek başına ayrıştırmasını gerektirirdi.)

Kurallar:
- Argüman sayısı ≠ parametre sayısı → E2003 (yerleşik primitiflerle aynı biçim).
- `const` parametreye argüman tam sayı LİTERALİ olmalı → aksi E2008
  (ADR-0027/0029 yerleşik kuralıyla tutarlı; `Mod<TAPS2>` gibi isimle
  argüman bu ADR'de yok).
- Tip parametresine argüman → E0003 (ileride; bu ADR yalnız const generics).
- Generic olmayan modüle argüman → E2003.
- İç içe generic (klon gövdesinde yeni örnekleme) worklist ile çözülür;
  64 turdan sonra hâlâ yeni örnek üretiliyorsa E2003 "instantiation depth".
- `Delayed<T, N>` / `delay<K>` yan tabloları (ADR-0037) klonla birlikte taşınır.
- Klonlar kaynak konumlarını korur; `Span`'e eklenen `ctx: u16` alanı
  (monomorf başına benzersiz, elle yazılmış kaynakta 0) span anahtarlı
  tabloların (`decl_spans`/`use_spans`, timing `pinned`) klonlar arasında
  çakışmasını önler — tanılar yine şablonun satırını gösterir.
- `test "..."` bloklarında generic DUT yazılamaz; test somut bir sarmalayıcı
  modülü hedefler (`examples/fir_filter.volt` → `Fir8`, `Fir4`).

**Tip yapıcıları `uint<N>` / `sint<N>`.** Bir genişlik parametresinin tam
sayı tipine girebilmesi için (`in sample : sint<WIDTH>`) `bits<N>` ile aynı
yoldan çözülen iki tip yapıcısı eklendi (grammar PrimitiveType,
`TypeRefKind::UIntN/SIntN`). `sint<16>` ≡ `i16`; SV eşlemesi
`logic signed [N-1:0]`.

### 4. SV üretimi ön koşulları (sv-mapping.md §9, §16)

Generic FIR'ın gövdesi TAPS'a göre değişebilsin diye emitter şu yapıları
üretir hale geldi:

| Volt | SV |
|---|---|
| `let f = Mod { clk, x: e }` (kullanıcı modülü) | çıkış telleri gövde başında `logic ... f_out;` + isimli bağlama; saat kısayolu, reset hedef modülün alan yapılandırmasından üst modülün aynı adlı reset portuna; bağlanmamış giriş E2005 |
| `f.out` | `f_out` |
| `for i in a..b { ... }` (on/comb/stage gövdesi, sınırlar sabit) | gövde her iterasyon için AÇILIR, `i` literale ikame; sabit olmayan sınır E2005 |
| modül seviyesi `for` | her iterasyon için `assign` satırları |
| `wire x : T` | `logic ... x;` |
| `comb { ... }` | `always_comb begin ... end` |

Blok içi `let`, dizi tipli `wire`/port ve `comb` içinde `match` dışı yapılar
hâlâ E0003.

## Sonuçlar

- `examples/fir_filter.volt`: 20+ cast → 0, 8 skaler const → 1 dizi,
  generic `FirFilter<TAPS, WIDTH>` iki farklı argümanla örneklenir
  (`FirFilter_8_16`, `FirFilter_4_16`), 10 simülasyon testi, Verilator
  `-Wall` temiz, kontratlar bmc/prove/cover ile kanıtlanır.
- Daraltma HÂLÂ E2001, işaret uyumsuzluğu HÂLÂ E2002 —
  `tests/ui/fail/39_widening_sign_mismatch.volt`,
  `tests/ui/fail/40_narrowing_still_error.volt`.
- `volt explain E2001` örneği daraltma örneğine çevrildi.
- Yeni tanı kodu açılmadı (E2001/E2002/E2003/E2005/E2008/E0003 yeniden kullanıldı).

## Alternatifler

- **Genişlemeyi tamamen serbest bırakmak** (synth modunda da): reddedildi —
  anotasyonsuz `a18 + b19`'da hangi genişlikte toplanacağı belirsizdir ve
  "genişleme görünür olmalı" ilkesi (§5) yalnız hedef yazıldığında sağlanır.
- **HIR seviyesinde monomorfizasyon**: reddedildi (yukarıda, §3 Katman).
- **Const diziyi `case` fonksiyonu olarak üretmek**: ölçüm notuna göre
  yedek olarak tutuldu.

## Ölçüm notu

`tests/ui/pass/51_const_array.volt` (4 elemanlı i16 tablo, sinyal indeksli
`COEFFS[sel]`) iki biçimde üretilip ölçüldü (2026-09-13, Docker
`hdlc/formal` Yosys, `verilator/verilator:latest` 5.050):

| Biçim | Verilator `--lint-only -Wall` | Yosys `read_verilog -sv; synth` |
|---|---|---|
| `localparam logic signed [15:0] COEFFS [0:3] = '{...}` | temiz | **REDDEDİLDİ**: `syntax error, unexpected '['` (unpacked localparam dizisi desteklenmiyor) |
| `function automatic ... COEFFS_at(...) case ... endfunction` | temiz | temiz — 648 hücre (tablo + 4-tap MAC + kaydırma hattı) |

Karar: tablo işlevi varsayılan. Formal akış (`volt verify` → sby → Yosys)
ilk biçimle hiç çalışmazdı; Verilator ikisini de kabul ettiğinden seçim
Yosys tarafından belirlendi. Aynı ölçümde `32'(x)` boyut dönüşümleri ve
`always_comb` akümülasyonu iki araçta da temiz geçti; `Fir8` +
`FirFilter_8_16` Yosys `synth` ile 1044 hücre.

Bilinen semantik sınır: `W'(a << k)` SV'de genişletmeyi kaydırmadan ÖNCE
yapar; Volt'ta sol kaydırma genişlemez ve taşan bitler düşer. Bu yüzden
emitter kaydırmanın sol operandına dış bağlamı itmez ve sonucu
`W'(a << k)` olarak sarar — Volt semantiğine (önce kaydır, sonra genişlet)
eşdeğer olması için kaydırma sonucunun genişliği kaynağın genişliğinde
tutulmalıdır; bu durum yalnız `let y : u16 = x8 << k` gibi açık genişlemeli
sol kaydırmada ortaya çıkar ve sonraki bir ADR'de ele alınacaktır.
