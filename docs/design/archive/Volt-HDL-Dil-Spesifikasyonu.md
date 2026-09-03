> UYARI: Bu belge geçersizdir.
> Güncel sürüm: docs/design/ altında.

# Volt HDL — Resmî Dil Spesifikasyonu (v0.1)

> **Kapsam:** Bu belge Volt'un sözdizimini (EBNF gramer) ve statik anlambilimini
> (resmî tip kuralları) tanımlar. Hedef, hem derleyici uygulayıcıları hem de
> formal doğrulama (K-framework) entegrasyonu için tek bir referans noktasıdır.
> Notasyon, Volt-IR'ye lowering'den önceki **kaynak dil** seviyesini tanımlar.

---

## 1. Notasyon

- `::=` üretim kuralı, `|` alternatif, `[...]` opsiyonel, `{...}` sıfır-veya-çok
  tekrar, `(...)` gruplama, `'...'` terminal.
- Tip yargıları: `Γ ⊢ e : τ` ("Γ bağlamında e ifadesi τ tipindedir").
- Çıkarım kuralları: premisler çizginin üstünde, sonuç altında.
- Bağlamlar:
  - `Γ` — değişken/sinyal → tip eşlemesi.
  - `Δ` — tanımlı saat alanları kümesi ve özellikleri.
  - `Σ` — modül imzaları (port tipleri).
  - `Π` — pipeline aşama bağlamı (aşama indeksi `k`).
- `n, m` doğal sayı (bit genişliği); `d` doğal sayı (cycle gecikmesi).

---

## 2. Sözcüksel Yapı (Lexical)

### 2.1 Anahtar Kelimeler

```
domain  module  in  out  inout  reg  wire  on  if  else  match
fsm  state  init  pipeline  stage  rule  let  const  enum  struct
bundle  newtype  extern  pub  import  as  test  assert  invariant
cover  expect  tick  forall  sync  async  posedge  negedge  active_high
active_low  after  cycles  true  false  eventually  within
```

### 2.2 Tanımlayıcılar (Identifiers)

```
ident      ::= (letter | '_') { letter | digit | '_' }
type_ident ::= upper_letter { letter | digit | '_' }   (* tipler büyük harfle başlar *)
letter     ::= 'a'..'z' | 'A'..'Z'
upper_letter ::= 'A'..'Z'
digit      ::= '0'..'9'
```

### 2.3 Literaller

```
int_lit    ::= dec_lit | hex_lit | bin_lit | sized_lit
dec_lit    ::= digit { digit | '_' }
hex_lit    ::= '0x' hex_digit { hex_digit | '_' }
bin_lit    ::= '0b' ('0' | '1') { '0' | '1' | '_' }
sized_lit  ::= dec_lit "'" ('d' dec | 'h' hex | 'b' bin)   (* örn. 8'd42, 4'hF *)
bool_lit   ::= 'true' | 'false'
time_lit   ::= dec_lit '.' 'cycles'
```

> **Kural:** `sized_lit` genişliği literalden çıkar; `dec_lit` genişliği bağlamdan
> çıkarılır (tip çıkarımı, Bölüm 4.3). Bağlamdan çıkarılamıyorsa derleme hatası.

### 2.4 Operatörler (öncelik düşükten yükseğe)

```
|| , && , |-> , |=>            (* mantıksal / temporal *)
== , != , < , <= , > , >=
| , ^ , &                       (* bit düzeyi *)
<< , >>                         (* kaydırma *)
+ , +% , +| , - , -% , -|       (* toplama/çıkarma varyantları *)
* , *% , /                      (* çarpma/bölme *)
! , ~ , unary-                  (* tekli *)
```

Atama/bağlantı operatörleri (ifade değil, deyim seviyesinde):
```
=    kombinasyonel atama / bağlantı
<=   kayıtlı güncelleme (yalnızca 'on D' içinde)
>>   stream bağlantısı (yön: producer → consumer)
<>   çift-yönlü bundle bağlantısı
```

Diğer: `::` yol erişimi, `@` saat alanı niteleyici, `.` üye erişim.

---

## 3. Sözdizimsel Gramer (EBNF)

### 3.1 Derleme Birimi

```
compilation_unit ::= { import_decl } { top_decl }
import_decl      ::= 'import' path [ 'as' ident ]
path             ::= ident { '::' ident }

top_decl ::= domain_decl | module_decl | type_decl
           | extern_decl | const_decl | test_decl
```

### 3.2 Saat Alanı

```
domain_decl ::= 'domain' type_ident '{' domain_field { ',' domain_field } '}'
domain_field ::= 'clock' '=' edge
              | 'reset' '=' reset_kind reset_pol
edge        ::= 'posedge' | 'negedge'
reset_kind  ::= 'sync' | 'async'
reset_pol   ::= 'active_high' | 'active_low'
```

### 3.3 Tip Tanımları

```
type_decl   ::= enum_decl | struct_decl | bundle_decl | newtype_decl
enum_decl   ::= [ 'pub' ] 'enum' type_ident '{' ident { ',' ident } '}'
struct_decl ::= [ 'pub' ] 'struct' type_ident '{' field { ',' field } '}'
bundle_decl ::= [ 'pub' ] 'bundle' type_ident '{' dir_field { ',' dir_field } '}'
newtype_decl::= [ 'pub' ] 'newtype' type_ident '=' type
field       ::= ident ':' type
dir_field   ::= direction ident ':' type
direction   ::= 'in' | 'out' | 'inout'
```

### 3.4 Tipler

```
type ::= scalar_type
       | type '@' type_ident                 (* saat alanı niteliği *)
       | 'Delayed' '<' type ',' const_expr '>'
       | 'Stream' '<' type '>' | 'Flow' '<' type '>'
       | type_ident [ '<' type_args '>' ]    (* enum/struct/bundle/newtype/jenerik *)

scalar_type ::= 'bool'
              | 'u' const_expr                 (* u8, u<N> *)
              | 'i' const_expr                 (* i16, i<N> *)
              | 'bits' '<' const_expr '>'
              | 'fixed' '<' const_expr ',' const_expr '>'
type_args   ::= type_arg { ',' type_arg }
type_arg    ::= type | const_expr
const_expr  ::= int_lit | ident | const_expr binop const_expr
```

### 3.5 Modül

```
module_decl ::= [ 'pub' ] 'module' type_ident [ generics ] '{' { module_item } '}'
generics    ::= '<' generic_param { ',' generic_param } '>'
generic_param ::= type_ident                       (* tip parametresi *)
                | 'const' ident ':' scalar_type     (* const-jenerik *)

module_item ::= port_decl | reg_decl | wire_decl | connect_stmt
              | on_block | rule_block | fsm_decl | pipeline_decl
              | inst_decl | assert_stmt | invariant_stmt | cover_stmt

port_decl   ::= direction ident ':' type
reg_decl    ::= 'reg' '(' type_ident ')' ident ':' type '=' expr   (* reset değeri ZORUNLU *)
wire_decl   ::= 'wire' ident ':' type
inst_decl   ::= 'let' ident '=' type_ident [ '<' type_args '>' ] '(' [ args ] ')'
```

### 3.6 Deyimler ve Bloklar

```
on_block    ::= 'on' type_ident '{' { seq_stmt } '}'
seq_stmt    ::= reg_update | if_stmt | match_stmt | block
reg_update  ::= lvalue '<=' expr
connect_stmt::= lvalue '=' expr | lvalue '>>' lvalue | lvalue '<>' lvalue
lvalue      ::= ident { ('.' ident) | ('[' expr ']') }

if_stmt     ::= 'if' expr block [ 'else' (if_stmt | block) ]
match_stmt  ::= 'match' expr '{' match_arm { match_arm } '}'
match_arm   ::= pattern '=>' (expr | block)
pattern     ::= path | int_lit | '_'
block       ::= '{' { seq_stmt | connect_stmt | let_bind } '}'
let_bind    ::= 'let' ident [ ':' type ] '=' expr

rule_block  ::= 'rule' ident [ 'when' expr ] '{' { seq_stmt } '}'
```

### 3.7 FSM ve Pipeline

```
fsm_decl    ::= 'fsm' type_ident 'on' type_ident '{'
                  'state' ident { ',' ident }
                  'init' ident
                  { transition }
                '}'
transition  ::= ident '=>' ident [ 'after' time_lit ] [ 'when' expr ]

pipeline_decl ::= 'pipeline' type_ident 'on' type_ident '{'
                    { stage_decl } 'out' ident '=' expr
                  '}'
stage_decl   ::= 'stage' ident ':' (let_bind | connect_stmt)
```

### 3.8 Doğrulama ve Test

```
assert_stmt   ::= 'assert' 'on' type_ident ':' temporal_expr
invariant_stmt::= 'invariant' 'on' type_ident ':' expr
cover_stmt    ::= 'cover' 'on' type_ident ':' expr
temporal_expr ::= expr
                | expr '|->' temporal_expr
                | expr '|=>' temporal_expr
                | 'eventually' '(' expr [ 'within' time_lit ] ')'

test_decl ::= 'test' string_lit '{' { test_stmt } '}'
test_stmt ::= inst_decl | connect_stmt | tick_stmt | expect_stmt | forall_stmt
tick_stmt ::= 'tick' '(' type_ident ',' expr ')'
expect_stmt::= 'expect' expr
forall_stmt::= 'forall' ident ':' type 'in' range block
range     ::= expr '..' expr
```

### 3.9 İfadeler

```
expr ::= int_lit | bool_lit | ident | path
       | expr binop expr
       | unop expr
       | expr '.' ident                      (* üye erişim *)
       | expr '[' expr [ ':' expr ] ']'       (* bit dilimi *)
       | ident '(' [ args ] ')'               (* çağrı / primitif *)
       | expr '.' 'truncate' '(' ')'
       | expr '.' 'resize' '(' ')'
       | expr 'as' type                       (* açık dönüşüm *)
       | '(' expr ')'
args ::= expr { ',' expr } | named_arg { ',' named_arg }
named_arg ::= ident '=' expr
```

---

## 4. Tip Sistemi (Statik Anlambilim)

### 4.1 Tipler

```
τ ::= bool
    | u<n>            unsigned, genişlik n
    | i<n>            signed, genişlik n
    | bits<n>         ham bit vektörü (aritmetiksiz)
    | fixed<n,f>      sabit ondalık (tam n, kesir f)
    | τ @ D           saat alanı D ile nitelenmiş
    | Delayed<τ, d>   d cycle gecikmeli (L1 zamanlama tipi)
    | Stream<τ>       geri-basınçlı akış
    | Flow<τ>         geri-basınçsız akış
    | N               newtype (taban tipi base(N))
    | enum E          sonlu durum/etiket kümesi
    | struct S        kayıtlı bileşik
    | bundle B        yönlü bileşik (arayüz)
```

Yardımcı yüklemler:
- `domain(τ)` — bir sinyal tipinin ait olduğu saat alanı (yoksa ⊥ = alansız/sabit).
- `width(τ)` — bit genişliği (`u<n>`→n, `i<n>`→n, `bits<n>`→n).
- `signed(τ)` — `i<n>` için doğru, diğer aritmetik tipler için yanlış.

### 4.2 Yargılar

```
Γ; Δ; Σ; Π ⊢ e : τ        (* ifade tiplemesi *)
Γ; Δ; Σ ⊢ stmt ok          (* deyim iyi-tanımlı *)
Δ ⊢ D domain               (* D tanımlı bir saat alanı *)
```

Kısalık için bağlam taşıyıcıları gerektiğinde gizlenir.

### 4.3 Aritmetik, Genişlik ve İşaretlilik Kuralları

**Genişleyen toplama (`+`)** — taşma korunur, sonuç bir bit geniş:
```
Γ ⊢ e1 : u<n>     Γ ⊢ e2 : u<m>
──────────────────────────────────────  (T-AddWiden-U)
Γ ⊢ e1 + e2 : u<max(n,m)+1>

Γ ⊢ e1 : i<n>     Γ ⊢ e2 : i<m>
──────────────────────────────────────  (T-AddWiden-I)
Γ ⊢ e1 + e2 : i<max(n,m)+1>
```

**Wrapping toplama (`+%`)** — genişlik korunur, üst bit atılır:
```
Γ ⊢ e1 : u<n>     Γ ⊢ e2 : u<m>
──────────────────────────────────────  (T-AddWrap)
Γ ⊢ e1 +% e2 : u<max(n,m)>
```

**Doygunluklu toplama (`+|`)** — genişlik korunur, sınırda doyurulur:
```
Γ ⊢ e1 : u<n>     Γ ⊢ e2 : u<m>
──────────────────────────────────────  (T-AddSat)
Γ ⊢ e1 +| e2 : u<max(n,m)>
```

**Çarpma (`*`)** — genişlikler toplanır:
```
Γ ⊢ e1 : u<n>     Γ ⊢ e2 : u<m>
──────────────────────────────────────  (T-Mul)
Γ ⊢ e1 * e2 : u<n+m>
```

**İşaretlilik karışımı YASAK** — `u` ve `i` arası işlemi tipleyen *hiçbir kural
yoktur*; bu, böyle bir ifadenin tip hatası olduğu anlamına gelir. Açık dönüşüm gerekir:
```
Γ ⊢ e : u<n>
─────────────────────  (T-CastSign)   [taban için n+1 işaret biti eklenir]
Γ ⊢ (e as i<n+1>) : i<n+1>
```

**Karşılaştırma** — eşit genişlik ve aynı işaretlilik gerektirir, sonuç `bool`:
```
Γ ⊢ e1 : u<n>     Γ ⊢ e2 : u<n>
──────────────────────────────────────  (T-Cmp)
Γ ⊢ (e1 == e2) : bool
```
(Farklı genişlikte karşılaştırma için açık `resize()` gerekir.)

### 4.4 Atama, Bağlantı ve Tek-Sürücü

**Atanabilirlik (`assignable`):** Yalnızca *aynı* skaler tip atanabilir; daraltma/
genişletme açık `truncate()`/`resize()` ister.
```
assignable(τ, τ)                                  (refleksif)
¬ assignable(u<n>, u<m>)  eğer n ≠ m               (daraltma/genişletme yasak)
¬ assignable(u<n>, i<m>)                           (işaretlilik uyuşmazlığı)
```

**Kombinasyonel bağlantı (`=`):**
```
Γ ⊢ lv : τ_l     Γ ⊢ e : τ_r     assignable(τ_r, τ_l)
       domain(τ_r) = domain(τ_l)  veya  domain(τ_r) = ⊥
─────────────────────────────────────────────────────────  (T-Connect)
Γ ⊢ (lv = e) ok
```

**Tek-sürücü kuralı (modül-genel statik kontrol):** Bir `lvalue`'nin kök sinyali,
modül içinde **en fazla bir** sürücüye (`=` veya `<=`) sahip olabilir.
```
∀ sinyal s ∈ module:  |drivers(s)| ≤ 1
─────────────────────────────────────────  (WF-SingleDriver)
module ok  (bu açıdan)
```
İhlal → derleme hatası `E0301: multiple drivers for 's'`.

**Birleşimsel döngü yasağı:** Kombinasyonel bağlantı grafiği (register'lar düğüm
keser) **döngüsüz** olmalıdır.
```
acyclic( comb_dependency_graph(module) )
─────────────────────────────────────────  (WF-NoCombLoop)
module ok  (bu açıdan)
```

### 4.5 Saat Alanları ve CDC

**Alan tutarlılığı:** Kombinasyonel bir ifadede karışan tüm sinyaller aynı alanda
(ya da alansız) olmalıdır:
```
Γ ⊢ e1 : τ1 @ D     Γ ⊢ e2 : τ2 @ D
─────────────────────────────────────  (T-SameDomain)
Γ ⊢ (e1 op e2) : τ @ D

Γ ⊢ e1 : τ1 @ D1   Γ ⊢ e2 : τ2 @ D2   D1 ≠ D2
─────────────────────────────────────────────────  (T-CrossDomain-Error)
        ⊥   (E0401: clock-domain crossing without synchronizer)
```

**CDC primitifi:** Yalnızca onaylı senkronizörler alan değiştirebilir. İmzaları:
```
cdc::sync_2ff  : ∀τ. (τ @ Dsrc) → (τ @ Ddst)        [yalnızca width(τ)=1 veya toggle-safe]
cdc::handshake : ∀τ. (Stream<τ> @ Dsrc) → (Stream<τ> @ Ddst)
cdc::gray_fifo : ∀τ,N. (Stream<τ> @ Dsrc, const N) → (Stream<τ> @ Ddst)
```
```
Γ ⊢ e : τ @ Dsrc      width(τ) = 1
────────────────────────────────────────────  (T-CDC-2FF)
Γ ⊢ cdc::sync_2ff(from=Dsrc, to=Ddst, e) : τ @ Ddst
```
Çok-bitlik düz `sync_2ff` çağrısı → `E0402: multi-bit signal needs gray_fifo/handshake`.

### 4.6 Register ve Sıralı Bloklar

**Register tanımı:** Bir alana aittir; reset değeri tip-uyumlu olmalıdır.
```
Δ ⊢ D domain     Γ ⊢ e_reset : τ
──────────────────────────────────────────────  (T-RegDecl)
Γ, (r : τ @ D) ⊢ (reg(D) r : τ = e_reset) ok
```

**Kayıtlı güncelleme (`<=`):** Yalnızca eşleşen `on D` bloğunda; hedef register o
alanda olmalı:
```
within(on D)     Γ ⊢ r : τ @ D     Γ ⊢ e : τ_r     assignable(τ_r, τ)
─────────────────────────────────────────────────────────────────────  (T-RegUpdate)
Γ ⊢ (r <= e) ok
```
`<=` bir `on` bloğu dışında → `E0210`. Hedef alan ≠ blok alanı → `E0211`.

`=` ile `<=`'nin aynı sinyal için karışması → `E0212: mixed comb/seq assignment`.

### 4.7 Latch Önleme — Kesin Atama (Definite Assignment)

Kombinasyonel bir bağlamda atanan her sinyal, **tüm kontrol-akış yollarında**
atanmalıdır; aksi halde örtük latch oluşur ve reddedilir.
```
defAssigned(s, then-branch)     defAssigned(s, else-branch)
─────────────────────────────────────────────────────────────  (DA-If)
defAssigned(s, if c { then } else { else })

∀ arm_i ∈ match:  defAssigned(s, arm_i)     match exhaustive
─────────────────────────────────────────────────────────────  (DA-Match)
defAssigned(s, match e { arms })
```
Bir dal `s`'yi atamıyorsa → `E0220: signal 's' not assigned on all paths (implicit latch)`.
(Sıralı `on D` bloğunda atanmayan register **önceki değerini korur** — bu latch
değildir, normal register davranışıdır; DA yalnızca kombinasyonel bağlama uygulanır.)

### 4.8 Kademeli Zamanlama Tipleri (L0 / L1 / L2)

**L0 — Pipeline aşama hizalama (zorunlu).** Pipeline bağlamı `Π` mevcut aşama
indeksi `k`'yı taşır. Bir `stage k`'da tanımlanan sinyal yalnızca `k`'da ham olarak
okunabilir; sonraki aşamada okumak gecikme gerektirir:
```
Π = stage k     Γ ⊢ x : τ @ D     x stage j'de tanımlı     j = k
──────────────────────────────────────────────────────────────────  (T-Stage-Same)
Γ; Π ⊢ x : τ @ D

Π = stage k     x stage j'de tanımlı     j < k
──────────────────────────────────────────────────────────────────  (T-Stage-Delayed)
Γ; Π ⊢ x : Delayed<τ, k−j> @ D
```
`Delayed<τ, d>` (d>0) bir değeri *ham* `τ` bekleyen yere bağlamak → `E0230:
pipeline stage misalignment (signal is d cycles behind)`. Derleyici otomatik
register ekleyerek hizalamayı önerir.

**L1 — Gecikme tipi (hafif, varsayılan açık).** `Delayed<τ, d>` birinci sınıf
tiptir; `d` cycle bekletme `delay()` primitifiyle açıkça artırılır:
```
Γ ⊢ e : τ @ D
─────────────────────────────────────  (T-Delay)
Γ ⊢ delay(e) : Delayed<τ, 1> @ D
```
İki gecikmeli değeri toplamak için gecikmeler eşit olmalı:
```
Γ ⊢ e1 : Delayed<u<n>, d>     Γ ⊢ e2 : Delayed<u<m>, d>
────────────────────────────────────────────────────────────  (T-Delayed-Add)
Γ ⊢ e1 + e2 : Delayed<u<max(n,m)+1>, d>
```
Eşit olmayan gecikme → `E0231: temporal misalignment (d1 ≠ d2)`.

**L2 — Tam timeline tipleri (opt-in, `#[timeline]`).** Filament/Anvil esinli olay
ve aralık tipleri. Bir port, geçerlilik penceresini `'G+[a, b]` ile bildirir
("olay G'den a..b cycle aralığında geçerli"). Bir donanım kaynağı (örn. çarpan)
çakışan pencerelerle kullanılırsa **yapısal tehlike** statik olarak reddedilir:
```
#[timeline]
module Mac {
    in  a : u8  @['G+[0,1])      // G+0..G+1 aralığında geçerli
    in  b : u8  @['G+[0,1])
    out p : u16 @['G+[1,2))      // bir cycle sonra geçerli
}
```
Çağrı (invocation) ve başlatma aralığı (initiation interval, II) kontrolü:
```
inst m = Mac()
m.invoke@T0(a0, b0)     m.invoke@T1(a1, b1)     T1 − T0 ≥ II(Mac)
──────────────────────────────────────────────────────────────────  (T-Timeline-II)
   ok   (aksi halde E0240: structural hazard, II ihlali)
```

> Bu üç seviye geriye uyumludur: L0 her zaman aktiftir; L1 yalnızca `Delayed`
> kullanan yerlerde devreye girer; L2 modül `#[timeline]` ile işaretlenmedikçe
> hiçbir maliyet getirmez.

### 4.9 FSM İyi-Tanımlılığı

```
init ∈ states     ∀ (s1 => s2) ∈ transitions:  s1, s2 ∈ states
∀ s ∈ states:  reachable(s, init)            (ulaşılabilirlik)
──────────────────────────────────────────────────────────────  (WF-FSM)
fsm F on D ok
```
Ulaşılamaz durum → `E0250: unreachable FSM state`. Hiç çıkış geçişi olmayan
non-final durum → `E0251: deadlock state` uyarısı.

### 4.10 Modüller, Jenerikler, Const-Jenerikler

**Modül örnekleme:** Tip ve const argümanları imzayla eşleşmeli:
```
Σ(M) = ⟨ generics: [T, const N : u32], ports: ... ⟩
Γ ⊢ τ_arg type     Γ ⊢ n_arg : u32  (derleme-zamanı sabit)
────────────────────────────────────────────────────────────  (T-Inst)
Γ ⊢ M<τ_arg, n_arg>() : instance(M[T:=τ_arg, N:=n_arg])
```
Const argümanın derleme zamanında sabit olmaması → `E0260`. Yanlış genişlikte port
bağlantısı → `E0261` (assignable kuralından).

---

## 5. İyi-Tanımlılık (Well-Formedness) Özeti

Bir modül, **tüm** şu koşulları sağlarsa iyi-tanımlıdır:

1. Her ifade tiplenir (Bölüm 4.3–4.5).
2. Her sinyalin ≤ 1 sürücüsü vardır (WF-SingleDriver).
3. Kombinasyonel bağımlılık grafiği döngüsüzdür (WF-NoCombLoop).
4. Her kombinasyonel sinyal tüm yollarda atanır (DA, Bölüm 4.7).
5. Alanlar-arası geçiş yalnızca CDC primitifiyleidir (Bölüm 4.5).
6. Her register bir alana aittir ve tip-uyumlu reset değeri vardır (Bölüm 4.6).
7. `<=` yalnızca eşleşen `on D` içindedir; comb/seq karışmaz.
8. Pipeline aşama hizalaması tutarlıdır (L0, Bölüm 4.8).
9. FSM ulaşılabilir ve kapalıdır (Bölüm 4.9).
10. `#[timeline]` modüllerde II/yapısal-tehlike kontrolü geçer (L2).

---

## 6. İşlemsel Anlambilim (Özet — Cycle Semantiği)

Volt tek bir **senkron cycle anlambilimi** kullanır (sim = sentez):

- Bir cycle'da: (a) tüm kombinasyonel `=` bağlantıları sabit-nokta değerine kadar
  hesaplanır (döngü olmadığından sonlu), (b) tüm `on D` blokları **aynı anda**
  okudukları register değerleriyle çalışır, (c) saat kenarında `<=` ile hesaplanan
  yeni değerler register'lara *eşzamanlı* yazılır.
- Bu, Verilog'un blocking/non-blocking + sensitivity-list belirsizliğini ortadan
  kaldırır: `<=` her zaman "kenar sonu eşzamanlı güncelleme", `=` her zaman
  "kombinasyonel anlık bağlantı"dır; başka mod yoktur.
- Reset davranışı alan tipinden gelir: `sync` reset saat kenarında, `async` reset
  kenardan bağımsız uygulanır; polarite `active_high/low` ile sabittir.

Bu anlambilim K-framework'te resmîleştirilerek CIRCT lowering'inin doğruluğu
(çeviri korumalılığı) kanıtlanabilir kılınır.

---

## 7. Uyumluluk Seviyeleri (Conformance)

| Seviye | İçerir | Hedef |
|---|---|---|
| **Core** | Bölüm 2–7 hariç L2; tüm WF kuralları + L0/L1 | MVP derleyici, FPGA/öğrenci |
| **Verify** | Core + `assert`/`invariant`/`cover` + formal köprü | Doğrulama ekipleri |
| **Timeline** | Verify + L2 `#[timeline]` | Yüksek-güvence ASIC |

Bir uygulama hangi seviyeyi desteklediğini bildirmelidir; üst seviye alt seviyeyi
kapsar.

---

*Bu spesifikasyon v0.1'dir; gramer ve kurallar referans uygulamayla birlikte
olgunlaştırılacaktır. Hata kodları (`E02xx`/`E03xx`/`E04xx`) derleyici tanı
kataloğuyla senkron tutulur.*
