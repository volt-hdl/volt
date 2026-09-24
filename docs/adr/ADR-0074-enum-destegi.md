# ADR-0074: Enum Desteği — Birim Varyantlı Enum'lar Donanıma İner

> Statü: KABUL EDİLDİ — Aşama 2 UYGULANDI (dal `feat/enum`; bkz. "Uygulama notları — Aşama 2"); Aşama 3 ayrı PR
> Tarih: 2026-09-24
> Etkilenen (plan): volt-syntax (`parser/stmt.rs` E0014 erteleme,
> `parser/test_expr.rs` `Enum::Varyant`, ADR-0066 otomatik kontratları),
> volt-hir (enum bildirim denetimi ve kodlama tablosu, tip kuralları,
> desen tiplemesi ve kapsayıcılık, `TypeArena::width_of`, tanılarda tip
> adı), volt-sv-emit (sinyal tipi, `localparam`, `case`), volt-diagnostics
> (E2xxx-A, W2xxx-A YENİ — numaralar Aşama 2'de tahsis edilir; `explain`
> iki dil), volt-driver (test dili, rapor), volt-lsp (hover, `::`
> tamamlama), `tests/`, `examples/` (Aşama 3), README.md, CHANGELOG.md.
> DOKUNULMADI: docs/spec/, docs/research/, kod.
> Genişletir: `grammar-full.ebnf` §7 (EnumDecl) ve §12 notu ("Enum
> kapsayıcılık analizi [F3]"), `sv-mapping.md` §2 (tip tablosu),
> ADR-0032 (E0014), ADR-0066 (F1, F3 joker kaynağı). Spec salt okunur;
> bu ADR ilgili bölümlerin güncel hâlidir.

## Sorun

ADR-0070 ölçümü: enum tipli port/reg/let `volt check`'te de artık
"not supported yet" verir (doğru), `volt build` E0003. Enum dilde var
(parser, çözümleme, sabit değerlendirme, tip denetiminin bir kısmı) ama
donanıma inmiyor. Sonuç: bütün örnek FSM'ler (`uart_tx`, `i2c`) durumları
`u2`/`u3` sayılarla yazıyor; `uart_tx.volt` bunu kendisi söylüyor:

```volt
// State encoding (an enum would be the natural fit once [F3]
// enum patterns land): 0 IDLE, 1 START, 2 DATA, 3 STOP.
reg state_r     : u2   = 0
```

ADR-0066 F1 kontratını ("durum geçerli") bu yüzden üretmedi: sayısal
durumda geçerli değer kümesi dilde yok. Enum bu bilgiyi dile taşır.

## Tespit (ADIM 1.1)

Ölçüm `main` 78083cb (PR #28 sonrası), `target/debug/volt.exe`. 37
sondalık külliyat (`build/enum-sv/probe.py <çıktı-dizini>` — `build/`
gitignore'da; her sonda `volt check` + `volt build --target-dir`).
Sondalar `enum State { Idle, Run, Done }` ve tek modül kullanır. Aynı
satırda check ve build aynı kodları verdi (parite tutuyor — ADR-0070).

### Katman katman

| Katman | Durum | Kanıt (sonda → sonuç) |
|---|---|---|
| Parser | TAM: birim, tuple, struct varyant; açık değer `= expr`; taban tipi `: T`; generic | `e01..e09` hepsi ayrışır (`crates/volt-syntax/src/parser/item.rs:1034`) |
| Çözümleme | Varyantlar kök kapsama bağlanmaz, yalnız `Enum::Varyant` (`resolve/collect.rs:55`); çok dosyalı `use` çalışır | `s05_bare_variant` → E1001 `undefined name: 'Idle'`; `t10` `State::Nope` → E1007; `p2/main.volt` (`use types::State`) yalnız sinyal tipi E0003'ü |
| Sabit değerlendirme | `ConstValue::EnumVariant { discriminant }`: açık değer ya da **sıra indeksi** (`consteval.rs:427`) | `{A = 5, B}` için B = 1 (SV'de 6 olurdu) — aşağıda Karar 2 |
| Bildirim denetimi | **YOK** | `e05` yinelenen varyant `{A, A}`, `e06` yinelenen değer `A = 1, B = 1`, `e07` `: u2 { B = 7 }`, `e08` generic, `e09` boş enum — hepsi **sessiz** (yalnız W1001) |
| Sinyal tipi | E0003 (sv-emit `sig_of_typeref` → `describe_user_type`) | `s01` reg, `s03` let, `s04` port: `not supported yet: enum type 'State' as a signal type (ports, reg, wire, let)` |
| Reset değeri | Parser zaten zorunlu kılar | `s02` `reg s : State` → E0001 `expected '=' for the reg initial value` |
| `==`/`!=` farklı enum | E2003 | `t02` → `comparison operands must have the same type: 'enum' and 'enum'` (ad yok!) |
| `==` tamsayı | E2003 | `t03` → `... 'enum' and 'tamsayı literali'` |
| Aritmetik | E2003 | `t01` `s + 1` → `incompatible arithmetic operands` |
| Atama tamsayı / başka enum | E2003 | `t04`, `t07`, `t09` |
| Bit seçimi | E2003 | `t11` `s[0]` → `bit selection is only allowed on numeric, bits or array types` |
| **Sıralama `<`** | **tip denetiminden GEÇER** | `t08` `s < State::Done` → yalnız sinyal tipi E0003 |
| Cast `enum → uN` | E2009 | `t05` `s as u2` → `cast 'enum' → 'u2' is invalid` |
| Cast `uN → enum` | E2009 | `t06` `raw as State` → `cast 'u2' → 'enum' is invalid` |
| E0014 | Parser'da, tip bilgisi olmadan, her `match` deyiminde | `m02` bütün varyantlar + `_` yok → E0014; `r01` sayısal `u2`, 4 değerin hepsi → yine E0014 (`parser/stmt.rs:645`) |
| Varyant deseni | typeck desen tipini **denetlemiyor**; sv-emit E0003 | `m04` `State` üzerinde `Other::X` → yalnız E0003 `binding, path and tuple patterns in 'match'`; `m05` `State` üzerinde `0` → E2005 (genişlik), tip hatası değil |
| Çıplak desen `Idle` | Bağlama deseni sayılır → E0003 | `m07` |
| Yinelenen kol | sessiz (sayısalda da) | `m09`; `p2/dup.volt` (`0 => …` iki kez) → yalnız W1001 |
| `match` ifadesi | E0003 (enum'dan bağımsız) | `m06` `not supported yet: 'match' expressions` |
| `const` enum | çözülür; kullanım sinyal tipinde E0003 | `f01` |
| `fn` argümanı | ifade içi çağrı zaten E0003 | `f02` |
| Domain W3003 | `TypeArena::width_of(Ty::Enum)` → `None` (`ty.rs:203`) | Enum `sync()`'ten geçseydi W3003 **sessiz** kalırdı |
| Test dili | `Enum::Varyant` ayrıştırılmaz; rapor sayı basar | `parser/test_expr.rs:204` (yalnız `dut.port`, `ad`, `ad[i]`, çağrı); `sim/tb_output.rs:54` `left=1 right=0` |
| LSP | hover `DefKind::Enum`/`EnumVariant` tanır; `::` sonrası tamamlama bağlamı YOK | `volt-lsp/src/completion.rs:14` (`Type`, `Domain`, `OnClock`, `Member`, `StmtStart`, `Expr`) |

Yanıtlar: parser tam; typeck karşılaştırma/atama/aritmetiği doğru
reddediyor ama sıralamayı kaçırıyor ve desenleri hiç tiplemiyor; varyant
yolu yalnız `State::Idle`; E0014 enum match'lerinde de "her match'te `_`"
biçiminde işliyor, çünkü parser'da ve tipsiz.

## Karar 1 — Kapsam (ADIM 1.2)

**Bu tur: birim varyantlı (C benzeri) enum.**

```volt
enum State { Idle, Start, Data, Stop }
enum Op : u7 { Load = 0b0000011, Store = 0b0100011, Alu = 0b0110011 }
```

Sinyal tipi olarak: port (modüller arası dahil), `reg`, `wire`, `let`,
`const`. Kontratlarda ve `prev()` içinde.

**Payload'lı varyant (tuple/struct) DESTEKLENMİYOR:** böyle bir enum
sinyal tipi olarak kullanıldığında E0003 `not supported yet: enum 'Cmd'
with data-carrying variants ('Cmd::Load')`. **Bildirim yasal kalır** —
yapıdaki ile aynı ayrım (struct bildirimi yasal, sinyal tipi E0003).
Gerekçe: `tests/ui/pass/91_type_graph_acyclic.volt` payload'lı `enum Cmd`
bildiriyor, `tests/ui/fail/77_recursive_enum.volt` ADR-0069 E4009'u
payload'lı enum'larla sınıyor; bildirim düzeyinde hata bu testleri
bozardı (test silmek yasak) ve hiçbir yanlış donanım üretmiyor.

Payload gelecek planı (ayrı ADR): etiket + en geniş payload birleşimi
(`{tag, union}` paketlenmiş vektör), yapıcı ifadesi `Cmd::Load(x)`,
bağlayan desenler (`Cmd::Load(v) => …`), SV'de `struct packed` +
`union packed`. Önkoşullar: struct sinyal tipi (bugün E0003) ve bağlama
desenleri (bugün E0003) — ikisi de enum'dan bağımsız işler.

Bu turda ayrıca E0003 kalanlar (sinyal tipi olarak): generic enum
(`enum E<T>`), enum dizisi (`[State; 4]`), struct alanı olarak enum
(struct zaten E0003), `@mmio` alanı olarak enum (Karar 6).

## Karar 2 — Kodlama (ADIM 1.3)

**Varsayılan: ikili, bildirim sırasıyla 0'dan, genişlik
`W = max(1, clog2(n))`** (`n` varyant sayısı; `n = 1` → 1 bit, `n = 3` →
2 bit, `n = 5` → 3 bit).

**Açık değer ve taban tipi: DESTEKLENİR.**

```volt
enum Op : u4 { Add = 0, Sub = 1, Jal = 8 }
```

Gerekçe: `enum → uN` yönünde (Karar 3) dış protokol kodunu birebir
taşımanın tek yolu (RISC-V opcode'u bir veri yoluna yazmak, bir
kontrol register'ının belgelenmiş kodu). Parser zaten ayrıştırıyor; eksik
olan yalnız denetim ve üretim. Kurallar:

| Kural | Tanı |
|---|---|
| Ya bütün varyantlar açık değerli ya hiçbiri | E2xxx-A |
| Açık değer sabit ifade (consteval), `≥ 0` | E2021 / E2010 |
| Değerler birbirinden farklı | E2xxx-A |
| Taban tipi `uN`, `uint<N>` ya da `bits<N>` (işaretsiz, ham) | E2xxx-A |
| Taban tipi varsa `W = N`; her değer `< 2^N` | E2010 |
| Taban tipi varsa `N ≥` gereken genişlik (açık değersizde `clog2(n)`) | E2xxx-A |
| Taban tipi yoksa `W = max(1, bitlen(en büyük değer))` (açık değerli) | — |
| Varyantsız enum (`enum E { }`) | E2xxx-A |
| Yinelenen varyant adı | E1003 |

"Hepsi ya da hiçbiri" kuralının nedeni ölçülmüş bir belirsizlik:
`consteval.rs:427` örtük varyantı **sıra indeksiyle** değerlendirir
(`{A = 5, B}` → B = 1), SV ve Rust ise önceki + 1 (B = 6). Karışık yazımı
yasaklamak iki anlamın hiçbirini seçmek zorunda bırakmaz ve okura da tek
anlam bırakır. Mevcut `consteval_tests.rs::enum_variant_positional_discriminant`
(`{Bekle = 0, Calis = 1, Bitti}`) consteval katmanında çalışır, bildirim
denetimine uğramaz — test değişmeden geçmeye devam eder; aynı kaynak
`volt check`'te E2xxx-A alır.

E4009 ile çakışma: `enum G : G { Idle }` (ADR-0069, ui/fail/77) önce
E4009 alır; taban tipi kullanıcı tipi olduğu için E2xxx-A da tetiklenirdi.
Taban tipi E4009 döngüsündeyse E2xxx-A verilmez — ui/fail/77 beklentisi
değişmez.

**One-hot / Gray (`@encoding(onehot)`): SONRA.** Gerekçe:
- Açık değerlerle bugün elle yazılabilir:
  `enum S : u4 { A = 1, B = 2, C = 4, D = 8 }` — kodlama, F1 ve `case`
  üretimi ikili ile aynı yoldan geçer, ek kural gerekmez.
- Asıl kazanç (varyant testinin tek bit `s[i]` olması, `unique case`
  yerine `case (1'b1)` kalıbı) `case` üretimini ve F1 biçimini değiştirir;
  bu turun "davranış ve donanım aynı kalmalı" kanıtını (Aşama 3) iki
  değişkene böler.
- Nitelik adı ve değerleri (`onehot`, `gray`, `onehot0`) ölçülecek bir
  sentez karşılaştırması ister (LUT/FF, Fmax) — ayrı ADR.

## Karar 3 — Tip kuralları (ADIM 1.4)

| İşlem | Karar | Tanı | Bugün |
|---|---|---|---|
| `==`, `!=` aynı enum | izinli, sonuç `bool` | — | tipte geçer |
| `==`, `!=` farklı enum ya da tamsayı | hata | E2003 | E2003 (ad yerine `'enum'`) |
| `<`, `<=`, `>`, `>=` | **hata** | E2003 | **geçiyor** (`t08`) |
| Aritmetik, bit düzeyi, kaydırma, tekli `!`/`-`/`~`, birleştirme | hata | E2003 | aritmetik E2003 |
| Bit/aralık seçimi `s[0]`, `s[1:0]` | hata | E2003 | E2003 |
| Atama: yalnız aynı enum'un değeri | — | E2003 | E2003 |
| `if s`, kontrat gövdesi `s` | `bool` değil | E2003 / E5004 | — |
| `e as uN`, `e as bits<N>`, `N ≥ W` | izinli, sıfır genişletme | — | E2009 |
| `e as uN`, `N < W` | hata (bilgi kaybı) | E2009 | E2009 |
| `e as iN`, `e as bool`, `e as Other` | hata | E2009 | E2009 |
| `uN as Enum` (sabit dahil) | **hata** | E2009 + öneri | E2009 |

Sıralama neden yasak: sıra kodlamaya bağlıdır; açık değerli enum'da
(`Jal = 8`, `Add = 0`) bildirim sırasıyla kod sırası ayrışır ve
`s < Op::Jal` okurun bekleyeceğini yapmaz. Gerekiyorsa `s as u4 < 8`
açık yazılır.

**`uN → enum`: YASAK (seçenek 1).** Değerlendirilen:

- *Yasak* — seçildi. Enum tipli bir değer yalnız varyant sabitlerinden,
  aynı enum'un sinyallerinden ve enum giriş portlarından gelir; iç
  sinyallerde "değer her zaman geçerli bir varyant" **inşa gereği**
  doğrudur. F1 (Karar 6) bu sayede tümevarımsal, kapsayıcı `match`
  (Karar 4) bu sayede sağlamdır. Ham bitten çözme açık yazılır ve
  geçersiz kodların ne olacağını yazar seçer:

  ```volt
  comb {
      match raw {
          0b0000011 => { op = Op::Load }
          0b0100011 => { op = Op::Store }
          _         => { op = Op::Illegal }
      }
  }
  ```

  E2009 önerisi (fix-it) bu kalıbı gösterir.
- *İzinli, kontrat üretir* (`raw as State` → otomatik `assert`) —
  reddedildi: kaynak bir giriş portuysa kontrat çevrenin davranışına
  bağlıdır ve formal'de karşı örnek her zaman vardır (port serbest);
  sessiz bir `assume` eklemek ise ispatı yanlışlayabilir. Kararı ikiye
  bölmek (iç kaynakta assert, portta assume) kaynağın izlenmesini ister —
  ölçülecek kazancı yok.
- *Yalnız sabit ve geçerli değerlerde* (`2 as State`) — reddedildi:
  `State::Run` zaten aynı şeyi adıyla söyler; tek kazanç kodu kopyalamak.

Güven sınırı: **modül giriş portu** enum tipliyse değer dışarıdan gelir;
üst seviye modülde çevre (testbench, elle yazılmış SV) geçersiz kod
sürebilir. Volt içi bağlantılarda kaynak yine enum tiplidir. Bu sınırın
sonuçları: F1 port kaynaklı register'lara üretilmez (Karar 6), kapsayıcı
`match`'in geçersiz kod davranışı tanımlıdır (Karar 4 — son kol).

**Reset değeri: zorunlu** — parser zaten her `reg` için başlangıç değeri
ister (`s02` → E0001); enum register'ında bu değer aynı enum'un sabiti
olmalı (`reg s : State = 0` → E2003, bugün de böyle: `t04`).

**Tanı metinlerinde enum adı:** E2003/E2009 mesajları `'enum'` yerine
`'State'` göstermeli (`t02`: `'enum' and 'enum'` iki farklı enum için
anlamsız). Hover zaten `TypeArena::display_named` kullanıyor (ADR-0070
§3.2); tip denetçisi mesajları da aynısını kullanacak.

## Karar 4 — `match` ve E0014 (ADIM 1.5)

İki seçenek:

- **(a) `_` enum'da da zorunlu.** Artı: SV'de her zaman `default`,
  geçersiz kodların davranışı açık, ADR-0032 ile birebir, kapsayıcılık
  analizi gerekmez. Eksi: enum büyüdüğünde unutulan varyantı derleyici
  yakalamaz — enum'un başlıca kazancı; 4 varyantlı (2'nin kuvveti) enum'da
  `_` ölü koddur ve ADR-0066 F3 ondan erişilemez bir cover üretir:
  sayısal eşdeğerde ölçüldü, doğru tasarım cover kipinde **E5001** alıyor:

  ```
  $ volt verify --mode cover --depth 12 f1_without.volt
     [1/1] Fsm (5 properties) ... FAIL (0.29s)
  error[E5001]: contract violated
  26 │             _ => { state_r <= 1 }
     = note: auto-generated FSM transition contract 'prev(state_r) != 0 &&
       prev(state_r) != 1 && prev(state_r) != 2 && state_r == 1', generated
       from match on state_r, transition _ -> 1
  ```
  (3 durumlu `u2` FSM, kod 3 erişilemez; `build/enum-sv/f1/f1_without.volt`.)

- **(b) Kapsayıcılık denetimi, `_` isteğe bağlı.** Seçildi.

**Karar (enum tipli sınanan için):**

| Kollar | Sonuç | SV |
|---|---|---|
| Bütün varyantlar muhafızsız kollarda adlı, `_` yok | **geçerli** | son adlı kol `default:` olur (yorumla) |
| Bütün varyantlar adlı + `_` var | geçerli — `_` açık **kurtarma kolu** (yalnız geçersiz kodlar) | adlı kollar + `default:` = `_` gövdesi |
| Eksik varyant, `_` var | geçerli (normal joker) | adlı kollar + `default:` |
| Eksik varyant, `_` yok | **E0014 hata**: `'match' on enum 'State' does not cover every variant: missing State::Stop` — fix-it eksik kolları ya da `_` kolunu ekler | — |
| Aynı varyant iki kolda | **W2xxx-A** erişilemez kol (ikincisi) | ikinci kol üretilmez |
| Çıplak `Idle` deseni, `Idle` sınananın varyantıysa | E0003 (bağlama deseni) + fix-it `State::Idle` | — |
| Başka enum'un varyantı ya da tamsayı deseni | E2003 | — |

Eksik varyant neden **hata**, uyarı değil: (1) bugünkü E0014 zaten hata;
(2) `comb` bloğunda eksik kol mandal (latch) demektir — ölçüldü (aşağıda
`E1`), Yosys `always_comb`'da **ERROR** veriyor; (3) sıralı blokta
"register değerini koru" niyeti `_ => { }` ile tek satırda açık yazılır.

Muhafızlı kollar (`if`) kapsamaya sayılmaz (muhafızlar zaten E0003,
ADR-0032).

**Geçersiz kodlar ve son kol:** 3 varyantlı enum 2 bittir, kod 3
geçersizdir. Kapsayıcı, `_`'sız `match`'te geçersiz kod son adlı kolun
davranışını alır — belgelenmiş, deterministik. İç sinyalde bu kod inşa
gereği oluşmaz (Karar 3) ve F1 bunu formal'de kanıtlar; giriş portunda
davranış tanımlı kalır. Ayrı bir kurtarma davranışı isteyen yazar `_`
kolunu yazar (tek olay bozulması — SEU — senaryosu).

**Neden `default` olarak son kol** (SV `case` biçimleri ölçüldü;
`build/enum-sv/case/`, aynı `always_comb` + `always_ff` gövdesi, 3
varyantlı enum):

```
$ verilator --lint-only -Wall --top-module E1 State_pkg.sv E1.sv   # default yok
%Warning-CASEINCOMPLETE: E1.sv:10:9: Case values incompletely covered (example pattern 0x3)
%Warning-CASEINCOMPLETE: E1.sv:20:13: Case values incompletely covered (example pattern 0x3)
%Warning-LATCH: E1.sv:9:5: Latch inferred for signal 'q' (not all control paths of combinational always assign a value)
$ yosys -p "read_verilog -sv State_pkg.sv E1.sv; synth -top E1; stat"
ERROR: Latch inferred for signal `\E1.\q' from always_comb process `\E1.$proc$E1.sv:9$1'.
```

| Biçim | Verilator 5.050 `-Wall` | Yosys 0.66 `synth` |
|---|---|---|
| E1 — bütün varyantlar, `default` yok | CASEINCOMPLETE ×2, LATCH | **ERROR: Latch inferred** |
| E3 — son varyant `default:` | temiz | temiz, **5 hücre** |
| E4 — `unique case`, bütün varyantlar | temiz | temiz, 7 hücre |
| E5 — bütün varyantlar + `default: ;` | temiz | **ERROR: Latch inferred** |

E3 hem temiz hem en küçük; `unique case` ek olarak simülasyonda
çalışma zamanı ihlal uyarısı ve X anlamı getirir (ADR-0008 x üretmez).

**Sayısal `match` DEĞİŞMEZ:** tamsayı sınananında E0014 bugünkü gibi her
`match` deyiminde `_` ister (`r01` 4 değerin hepsiyle bile) — enum
kullanmayan tasarımların tanıları ve SV'si byte-aynı kalır (golden).

**Uygulama sınırı:** E0014 bugün parser'da, tipsiz verilir
(`parser/stmt.rs:645`). Parser, muhafızsız kollarından biri yol deseni
(`A::B`) olan ve `_`'sız `match`'lerde E0014'ü **tip denetimine
erteler**; tip denetçisi sınanan enum ise kapsayıcılığa bakar, değilse
(yol deseni + sayısal sınanan, ör. `tests/fixtures/parity/p29b`) E2003
ve eski E0014'ü verir. Yalnız literal/joker desenli `match`'ler parser'da
kalır — sayısal yol dokunulmaz. Tip denetimi paylaşılan boru hattında
olduğundan check = build = LSP (ADR-0070) korunur.

## Karar 5 — SV eşlemesi (ADIM 1.6)

Seçenekler:

- **A** — `typedef enum logic [W-1:0] {...}` bir SV paketinde
  (`build/rtl/<Enum>_pkg.sv`), sinyaller `State_pkg::State`.
- **B** — modül başına `localparam logic [W-1:0] State_Idle = 2'd0;`,
  sinyaller düz `logic [W-1:0]`.

### Deney

`build/enum-sv/` (gitignore'da; üreteç `gen.py`). Aynı tasarım iki
biçimde: `Fsm` (enum durum register'ı, 3 varyant, 2'nin kuvveti değil,
enum çıkış portu) ve `Top` (`Fsm` örneği — enum portu modüller arası,
ADR-0024 modül başına dosya; `enum → logic`; A'da `State'(raw)`). A'nın
paket dosyası:

```systemverilog
`default_nettype none

package State_pkg;
    typedef enum logic [1:0] {
        Idle = 2'd0,
        Run  = 2'd1,
        Done = 2'd2
    } State;
endpackage
```

Araçlar: `verilator/verilator:latest` (5.050), `hdlc/yosys:latest`
(Yosys 0.66), `hdlc/formal` (SymbiYosys, Yosys 0.36+42 — `volt verify`'ın
imajı). Bütün komutlar `MSYS_NO_PATHCONV=1 docker run --rm -v
C:/Dev/volthdl/build/enum-sv:/work -w /work/<A|B> <imaj> …`.

**1. Verilator `-Wall`:**

```
$ verilator --lint-only -Wall --top-module Top State_pkg.sv Fsm.sv Top.sv   # A
- Verilator: Built from 0.058 MB sources in 4 modules, into 0.012 MB in 3 C++ files
$ verilator --lint-only -Wall --top-module Top Fsm.sv Top.sv                # B
- Verilator: Built from 0.044 MB sources in 3 modules, into 0.012 MB in 3 C++ files
```

İkisi de uyarısız. A'da dönüşümsüz `logic → enum` ataması (`Neg.sv`,
`s <= raw`):

```
%Error-ENUMVALUE: Neg.sv:9:35: Implicit conversion to enum 'enum{}State_pkg::State' from 'logic[1:0]' (IEEE 1800-2023 6.19.3)
```

A'da `enum → logic` genişletme `8'(s)` ve `2'(s)` `-Wall` temiz,
Yosys 0.36 `read -formal` temiz. B'de kullanılmayan `localparam`:

```
%Warning-UNUSEDPARAM: Unused.sv:6:28: Parameter is not used: 'State_Idle'
%Warning-UNUSEDPARAM: Unused.sv:7:28: Parameter is not used: 'State_Run'
%Error: Exiting due to 2 warning(s)
```

→ B'de modül yalnız **kullandığı** varyantları bildirmeli.

**2. Yosys sentezi (0.66):** A ve B'de
`read_verilog -sv …; synth -top Top -flatten; opt_clean -purge; rename -enumerate; write_verilog -noattr net.v; synth_ice40 -top Top; stat`:

| | genel hücreler | iCE40 |
|---|---|---|
| A | 3 `$_ANDNOT_`, 1 `$_NOR_`, 1 `$_ORNOT_`, 2 `$_SDFFE_PP0P_`, 2 `$_SDFF_PP0_` | 5 SB_LUT4, 2 SB_DFFESR, 2 SB_DFFSR |
| B | aynı | 5 SB_LUT4, 2 SB_DFFESR, 2 SB_DFFSR |

```
$ diff <(grep -v "^/\*" A/net.v) <(grep -v "^/\*" B/net.v) && echo IDENTICAL
IDENTICAL
```

**3. OpenSTA:** netlist üzerinde çalışır (ADR-0065 akışı: Yosys →
`write_verilog` → OpenSTA). A ve B netlistleri byte-aynı olduğundan
OpenSTA girdisi aynıdır; kodlama biçiminin zamanlama analizine etkisi
yok. Ayrı bir OpenSTA koşusu bu nedenle bilgi eklemez.

**4. Formal (Yosys 0.36, `read -formal`):** A tek dosyada (paket +
modül + immediate assert/cover, `volt verify`'ın tek dosya akışı) okundu:

```
SBY [fsm_a_cov] DONE (PASS, rc=0)
SBY [fsm_a_prv] summary: successful proof by k-induction.
SBY [fsm_a_prv] DONE (PASS, rc=0)
```

B `read -formal Fsm.sv Top.sv; prep -top Top` hatasız.

**5. Dosya sırası — ayırt edici ölçüm.** Kullanıcılar ve betikler
`rtl/*.sv`'yi glob'la okur; alfabetik sırada `Fsm.sv` < `State_pkg.sv`:

```
$ verilator --lint-only -Wall --top-module Top Fsm.sv State_pkg.sv Top.sv     # A
(temiz — Verilator sıraya duyarsız)
$ yosys -q -p "read_verilog -sv Fsm.sv State_pkg.sv Top.sv; hierarchy -top Top"   # A
Fsm.sv:7: ERROR: syntax error, unexpected TOK_PACKAGESEP, expecting ')' or ',' or '='
$ yosys -q -p "read_verilog -sv Fsm.sv; read_verilog -sv Top.sv; hierarchy -check -top Top"  # B
(temiz)
```

Projenin kendi OpenSTA CI betiği tam olarak bunu yapıyor
(`scripts/sta/run.py:166`: `files = sorted(p.name for p in rtl.glob("*.sv"))`,
dosya başına `read_verilog -sv`). A, bu betiği ve glob'la okuyan her
kullanıcı akışını **enum adı bir modül adından alfabetik olarak sonra
geldiğinde** kırar; düzeltme sıralı dosya listesi (`files.f`) ve
kullanıcıya yeni bir sözleşme demektir.

**6. Dalga formu (VCD):** A ve B, aynı testbench ile
`verilator --binary --trace --timing` → `wave.vcd` başlıkları:

```
A:  $var wire 2 % s [1:0] $end            (u_fsm)
B:  $var wire 2 * State_Idle [1:0] $end   (u_fsm ve dut'ta)
    $var wire 2 + State_Run [1:0] $end
    $var wire 2 , State_Done [1:0] $end
    $var wire 2 $ s [1:0] $end
```

**İkisinde de durum adları görünmüyor** (VCD'de enum tipi yok,
Verilator `$attrbegin` yazmadı); B `localparam`'ları sinyal olarak döküp
modül başına varyant sayısı kadar gürültü ekliyor (ama kodları dalga
formunda okunur kılıyor). FST ölçülmedi.

**7. Verilator C++ (`volt test`/`volt run` akışı):** A, `--cc` +
`public_flat_rw`: `CData/*1:0*/ Top__DOT__u_fsm__DOT__s;` — enum sinyali
düz `CData`; test donanımı (ADR-0058 `rootp`) iki seçenekte de aynı
tipi görür.

### Karar: **B — `localparam` + düz `logic`**

| Ölçüt | A | B |
|---|---|---|
| Verilator `-Wall` | temiz | temiz (yalnız kullanılan varyantlar) |
| Yosys sentez / netlist | aynı | aynı |
| OpenSTA | aynı girdi | aynı girdi |
| Formal (Yosys 0.36) | okur | okur |
| Glob/alfabetik okuma (Yosys) | **kırılır** | çalışır |
| Her `.sv` kendi başına yeterli (ADR-0024) | hayır (paket gerekir) | **evet** |
| SV sınırında tip koruması (ENUMVALUE) | var | yok |
| Okunabilirlik | `State_pkg::Idle` | `State_Idle` |
| VCD | ad yok | ad yok, `localparam` gürültüsü |

Gerekçe: ölçülen tek işlevsel fark dosya sırası kırılganlığıdır ve A'nın
aleyhinedir — projenin kendi CI betiğinde ve ADR-0024'ün "her modül bir
dosya, araçların dosya-başına-modül beklentisi" gerekçesinde. A'nın
artıları (ENUMVALUE, tipli port) yalnız Volt çıktısına **elle yazılmış
SV** bağlandığında devreye girer; Volt içi bağlantılarda enum tipi Volt
tip denetçisinde zaten korunur. Okunabilirlik (ADR-0012) B'de korunur:
varyant adı `case` etiketlerinde ve karşılaştırmalarda görünür (İ1 isim
korunumu: `<Enum>_<Varyant>`); netlist ikisinde aynı (ECO).
Geri dönülebilir: tipli SV arayüzü isteyen kullanıcılar için ileride
`--sv-enum=typedef` seçeneği (A, `files.f` ile) eklenebilir — ayrı ADR.

### B'nin kuralları

```systemverilog
module Fsm (
    input  logic       clk,
    input  logic       rst,
    input  logic       go,
    output logic [1:0] st      // State
);

    // enum State : Idle = 0, Run = 1, Done = 2
    localparam logic [1:0] State_Idle = 2'd0;
    localparam logic [1:0] State_Run  = 2'd1;
    localparam logic [1:0] State_Done = 2'd2;

    logic [1:0] state_r;   // State
    ...
            case (state_r)
                State_Idle: ...
                State_Run:  ...
                default: begin // State_Done (and invalid codes)
```

1. Modül, gövdesinde **adı geçen** varyantlar için `localparam` bildirir
   (UNUSEDPARAM); enum başına tek satırlık tam kodlama yorumu tabloyu
   yine eksiksiz gösterir. Bildirim sırası: enum'un bildirim sırası,
   varyantın bildirim sırası (determinizm, ADR-0015).
2. Adlandırma `<Enum>_<Varyant>`; bir modül içinde aynı ada sahip sinyal
   ya da sabit varsa E1003 (SV ad çakışması; fix-it yeniden adlandırma).
3. Enum tipli port/sinyal bildirimi sonuna `// <Enum>` yorumu (İ5).
4. `const START : State = State::Idle` kullanım yerinde `State_Idle`
   olarak iner (sabitler bugün de satır içi iner).
5. `e as uN` → `N'(e)` (`N == W`'de dönüşümsüz); `e as bits<N>` aynı.
6. Kapsayıcı `_`'sız `match` → son adlı kol `default:` (Karar 4).
7. Enum kullanmayan modüllerde çıktı byte-aynı.

## Karar 6 — Diğer katmanlar (ADIM 1.7)

### Domain / CDC

`TypeArena::width_of(Ty::Enum)` bugün `None` → çok bitli enum `sync()`'ten
geçse W3003 sessiz kalırdı. Karar: `width_of` enum için `W`'yi döndürür
(kodlama tablosu tip denetçisinde; arena enum genişliğini bilmediği için
yardımcı tablo üzerinden). `W > 1` enum `sync()` → **W3003**, sayısal ile
aynı risk, aynı öneri (Handshake / AsyncFifo). 2 varyantlı (1 bit) enum
uyarı almaz — doğru, tek bit. Enum'a özgü öneri yok: Gray kodlu açık
değerler yalnız ardışık geçişlerde yardımcı olur, genel bir öneri
değildir.

### Formal — ADR-0066 F1 ve F3

**F1 "durum geçerli": enum FSM'lerinde ÜRETİLİR.** Ölçüm: tümevarım
yardımcısı olarak değeri var. 3 durumlu FSM, geçersiz kodda bekleyen
(`_ => { }`) ve durum dışı serbest sayaçlı (`build/enum-sv/f1/hold_*.volt`
— `invariant: c_r <= 20`; F1'li sürümde ek olarak `invariant: state_r <= 2`,
sayısal FSM ile yazılabilen eşdeğer):

```
$ VOLT_SBY=build/sby-docker.cmd volt verify --mode prove --depth <d> hold_<x>.volt
prove depth 4  hold_without: DONE (UNKNOWN, rc=4)
prove depth 4  hold_with   : DONE (PASS, rc=0)
prove depth 10 hold_without: DONE (UNKNOWN, rc=4)
prove depth 10 hold_with   : DONE (PASS, rc=0)
prove depth 20 hold_without: DONE (UNKNOWN, rc=4)
prove depth 20 hold_with   : DONE (PASS, rc=0)
prove depth 24 hold_without: DONE (PASS, rc=0)
prove depth 24 hold_with   : DONE (PASS, rc=0)
```

F1 olmadan k-tümevarım, geçersiz kodda sayacın büyüdüğü zinciri ancak
sayaç sınırı kadar derinlikte eler (burada 21+); F1 ile derinlik 4
yeter. Geçersiz kodun öncülü yoksa (`_ => { s <= Run }`,
`f1_*.volt`) fark küçük: F1'siz derinlik 2 UNKNOWN, 3'ten itibaren iki
sürüm de PASS — k-tümevarım öncülsüz durumu kendisi eler.

Kurallar:
- Yalnız ADR-0066'nın tanıdığı FSM'lerde (bütün yazmalar sabit) ve durum
  register'ı enum tipliyse. Bütün yazmalar enum sabiti ve `uN → enum`
  yasak (Karar 3) → F1 tümevarımsaldır.
- `n = 2^W` (bütün kodlar geçerli) → F1 totolojidir, **üretilmez**.
- Biçim: `invariant: s == State::Idle || s == State::Run || s == State::Done`
  (SV `state_r == State_Idle || …`); kaynak "auto FSM state-valid
  invariant, generated from enum State". Kullanıcı aynı metni yazdıysa
  tekilleştirme (ADR-0066 §2) geçerli.
- `@no_auto_contracts` F1'i de kapatır.
- ADR-0064: F1 simülasyonda da denetlenir — testbench'in bir giriş
  portundan enum register'ına geçersiz kod sızdırması yakalanır (port
  kaynaklı register FSM sayılmaz ve F1 almaz; bu durum ADR-0066'nın
  "bütün yazmalar sabit" koşulunda zaten dışarıda kalır).

**F3 joker kaynağı (enum FSM):** ADR-0066'da joker kolu "adı geçen
hiçbir literal değil" kaynağıdır. Enum'da joker kaynağı **adı geçmeyen
varyantların kümesidir** (`prev(s) == State::Stop || …`); küme boşsa
(kapsayıcı `match` + kurtarma `_`) jokerden geçiş cover'ı **üretilmez** —
yukarıda ölçülen E5001 yanlış alarmı enum FSM'lerinde oluşmaz.

**Varyant başına cover:** ADR-0066 F2 kuralı aynen (hedef olmayan
durumlar için `cover: s == V`). Hiç yazılmayan ve hiç kolu olmayan
varyant için cover üretilmez.

**Giriş portları:** enum tipli giriş portuna formal'de otomatik `assume`
EKLENMEZ — çevre hakkında örtük varsayım ispatı yanlışlayabilir; port
serbest kalır (muhafazakâr). Gelecek iş olarak not edildi.

### Simülasyon / test dili

- Test ifadelerinde `State::Idle` (`parser/test_expr.rs`'e yol biçimi):
  birimin enum'larından varyant değerine iner (`TbValue::Lit`).
- `dut.cmd = State::Run` enum giriş portuna yazılabilir; **tamsayı da
  yazılabilir** (E8512 genişlik denetimiyle) — testler geçersiz kod
  enjeksiyonunu bilerek yapabilmeli.
- `assert_eq(dut.state, State::Idle)` başarısızlığında rapor adı da
  basar: `left=1 (State::Run) right=0 (State::Idle)`; geçersiz kod
  `left=3 (State: invalid code)`. Sürücü port tipini derlemeden bilir;
  testbench çıktısı (`VOLT-ASSERT-FAIL … left= right=`) değişmez, adlar
  raporda eklenir.
- Başka enum'un varyantıyla karşılaştırma → E8511.

### `@mmio` / HW-SW köprüsü

**SONRA.** Enum tipli `@mmio` alanı bu turda E0003
(`not supported yet: enum-typed register map field`). Gerekçe: Rust
`#[repr(uN)] enum` + `TryFrom`, C `enum` + doğrulama makrosu, regmap JSON
şeması ve ADR-0063 tutarlılık denetimi birlikte değişir; yazılımın
okuduğu geçersiz kod (bus hatası, sıfırlanmamış register) yazılım
tarafında tasarım ister. Ayrı ADR.

### LSP

- Hover: varyantta `State::Run = 2'd1 — enum State (2 bit)`; enum'da
  varyant listesi ve genişlik.
- Tamamlama: `State::` sonrası varyantlar (yeni `Context::Path(String)`).
- Tanılar paylaşılan boru hattından gelir; yeni bir LSP yolu yok.

### Parite (ADR-0070)

Yeni tanıların hepsi tip denetçisinde ya da çıktısız emit'te (E0003,
E1003 SV ad çakışması) — check = build = LSP korunur. Parite sondaları:
mevcut `p29b` (`u2` üzerinde `S::A` deseni) E0003 → E2003 olur, `p35c`
(enum portu) E0003 → temiz; beklentiler (`// parity:` başlığı)
güncellenir, sonda silinmez. Her yeni tanıya bir `e*` sondası.

### Sürücü analizi (ADR-0073)

Enum sinyalleri yalnız bütün olarak atanır (bit/aralık seçimi E2003) —
`DriverKind` ve bit aralığı mantığı değişmez. `d*` sondası: enum `let` +
atama → E4001; enum portu iki örnekten sürülmesi → E4001.

## Tanı özeti

| Kod | Durum | Tetik |
|---|---|---|
| E0003 | mevcut, kapsam daralır | payload'lı / generic enum, enum dizisi, `@mmio` enum alanı (sinyal tipi olarak); bağlama deseni (+ `State::Idle` fix-it) |
| E0014 | mevcut, enum'da anlamı değişir | enum `match`'i eksik varyantlı ve `_`'sız (mesaj eksik varyantları listeler); sayısal `match` değişmez |
| E1003 | mevcut | yinelenen varyant adı; SV ad çakışması `<Enum>_<Varyant>` |
| E2003 | mevcut, kapsam genişler | sıralama, bit düzeyi/tekli işlemler, desen tipi uyuşmazlığı; mesajlarda enum adı |
| E2009 | mevcut, kapsam daralır | `uN → enum`, dar `enum → uN`, `enum → iN/bool/enum` |
| E2010 | mevcut | açık değer taban tipine sığmıyor / negatif |
| E2021 | mevcut | açık değer sabit değil |
| **E2xxx-A** | **YENİ** — geçersiz enum kodlaması | karışık açık/örtük değer, yinelenen değer, varyantsız enum, geçersiz taban tipi, dar taban tipi |
| **W2xxx-A** | **YENİ** — erişilemez `match` kolu | aynı varyant ikinci kez (yalnız enum sınananı) |
| W3003 | mevcut, enum'a genişler | `W > 1` enum `sync()` |

İki yeni kodun numaraları Aşama 2'de E2029 / W2013'ten sonraki boş
numaralardan tahsis edilir (tasarım ADR'sinde numara yazılmaz —
`just consistency` ADR'deki her kodu tanı enum'unda arar). Her ikisi iki
dilde mesaj + `volt explain`.

## Reddedilenler

- **SV eşleme A (paket + `typedef enum`)** — Karar 5: glob/alfabetik
  okumada Yosys kırılır (ölçüldü), `.sv` dosyaları kendi başına yetmez.
- **`_` enum'da da zorunlu** — Karar 4: unutulan varyantı yakalamaz,
  2'nin kuvveti varyantlı enum'da ölü kol ve F3 yanlış alarmı.
- **Eksik varyant uyarı** — comb'da mandal (ölçüldü), E0014 zaten hata.
- **`uN → enum` kontratlı ya da sabitle** — Karar 3.
- **Çıplak varyant adları (`Idle`)** — iki enum aynı adı taşıyabilir,
  çözümleme tasarımı varyantları kök kapsama bağlamıyor
  (`resolve/collect.rs:55`); desende çıplak ad Rust'ta bile bağlamadır
  (sessiz joker tuzağı).
- **`unique case` üretimi** — Karar 4 ölçümü: daha çok hücre, X anlamı.
- **Karışık açık/örtük değer** — Karar 2: consteval ile SV/Rust anlamı
  ayrışıyor.
- **Payload'lı enum'da bildirim hatası** — ui/pass/91 ve ui/fail/77'yi
  bozar, yanlış donanım üretmez.

## Uygulama planı

### Aşama 2 — uygulama (dal `feat/enum`, main'den)

1. **Bildirim ve kodlama** (volt-hir): `typeck` içinde enum kodlama
   tablosu (`EnumLayout { width, values }`) — Karar 2 kuralları, E2xxx-A,
   E2010, E1003; E4009 döngüsündeki taban tipi atlanır.
2. **Tipler** (volt-hir): sıralama/bit düzeyi/tekli E2003; cast kuralları;
   mesajlarda `display_named`; `width_of` enum genişliği (W3003).
3. **match** (volt-syntax + volt-hir): parser E0014 ertelemesi (yol
   desenli, `_`'sız `match`); typeck desen tipleme, kapsayıcılık,
   W2xxx-A, çıplak desen fix-it.
4. **SV** (volt-sv-emit): `sig_of_typeref` enum → `Sig { width }`;
   payload/generic/dizi E0003; `localparam` (kullanılanlar + kodlama
   yorumu); `case` etiketleri, son kol `default`; `N'(e)`; ad çakışması
   E1003; `// <Enum>` yorumları.
5. **Otomatik kontratlar** (volt-syntax, ADR-0066): enum FSM tanıma,
   F1, F3 joker kaynağı = adı geçmeyen varyantlar.
6. **Test dili / rapor** (volt-syntax + volt-driver), **LSP** (volt-lsp),
   **@mmio** E0003.
7. **Tanılar** (volt-diagnostics): iki yeni kod, iki dil, `explain`;
   E0014 açıklaması enum kuralıyla güncellenir.
8. **Testler:** `tests/ui/pass/` — enum FSM, modüller arası enum portu,
   açık değerli + taban tipli enum, kapsayıcı `_`'sız match, `enum as uN`
   (ui/pass harness build'i de denetler — ADR-0071); `tests/ui/fail/` —
   her yeni/genişleyen tanı; parite `e*` sondaları + p29b/p35c
   beklentileri; `d*` enum sondası; `volt test` enum FSM (Docker);
   `volt verify` enum FSM, F1 dahil (Docker); Verilator `-Wall` (Docker).
9. **Mutasyon** (tek tek, `CARGO_BUILD_JOBS=2`, `--test-threads=2`):
   enum tip denetimini kaldır (enum == tamsayı geçsin); genişliği boz
   (`clog2` bir eksik/fazla); kapsayıcılığı boz (eksik varyantı kapsanmış
   say / `_` yeniden zorunlu). Her biri en az bir testi düşürmeli.
10. **Golden:** referans 78083cb (PR #28). Enum kullanmayan tasarımların
    SV'si ve tanıları byte-aynı (`build/` golden betikleri).
11. ADR-0074'e uygulama notları; CHANGELOG.

### Aşama 3 — örnekler ve kanıt (dal `feat/enum-examples`, main'den)

1. `examples/uart_tx.volt` (4 durum) ve `examples/i2c/` FSM'lerini enum'a
   taşı. `riscv_core`: `match` yok (ADR-0066), opcode'lar komut
   bitlerinden gelir ve `uN → enum` yasak — çözme `match`'i ek mantık
   demektir; değerlendirilir, varsayılan **dokunulmaz**.
2. Kanıt: `volt test` (Docker), `volt verify` (Docker), Verilator
   `-Wall`; Yosys sentezi sayılı ve enum'lu sürüm için LUT/FF tablosu
   (aynı kodlamada aynı olmalı — ölç).
3. ADR-0066 F1'in üretilip üretilmediği ve formal süresine etkisi
   (`uart_tx` 4 durum → `n = 2^W`, F1 üretilmez; i2c'de ölç).
4. README Limitations: enum satırı kaldırılır, struct kalır; CHANGELOG.

## Sınırlar / Ertelenen

- Payload'lı enum (Karar 1), `@encoding(onehot|gray)` (Karar 2),
  `@mmio` enum alanı (Karar 6), `--sv-enum=typedef` (Karar 5), enum
  dizisi, generic enum, `use State::*`.
- Dalga formunda varyant adları: VCD taşımıyor (ölçüldü); `volt run`
  için GTKWave çeviri dosyası ya da FST — ayrı iş.
- Enum giriş portları için formal `assume` (Karar 6).
- `match` ifadesi (enum'dan bağımsız E0003).

## Yan bulgular (enum dışı, bu ADR kapsamında düzeltilmez)

1. `volt verify --mode prove`, tümevarım başarısızlığını (sby
   `DONE (UNKNOWN, rc=4)`, günlükte `failed assertion …`) "SymbiYosys
   reported a tool error for module 'Fsm' (exit code Some(4))" diye
   raporluyor; karşı örnek konumu kayboluyor (`f1_without.volt`,
   derinlik 2).
2. ADR-0066 F3, erişilemez `_` kolu için de geçiş cover'ı üretiyor;
   sayısal FSM'de kullanılmayan kod varsa doğru tasarım cover kipinde
   E5001 alıyor (Karar 4 ölçümü). Enum FSM'lerinde Karar 6 ile oluşmaz;
   sayısal FSM'ler için ayrı düzeltme.
3. İngilizce E2003 mesajı tamsayı literalini Türkçe yazıyor:
   `incompatible arithmetic operands: 'enum' and 'tamsayı literali'`.
4. Sayısal `match`'te yinelenen literal kolu sessiz (`0 => …` iki kez).

## Uygulama notları — Aşama 2 (2026-09-24, dal `feat/enum`)

Kararların hepsi yazıldığı gibi uygulandı; aşağıdakiler tasarımın açık
bıraktığı ayrıntılar ve ölçüm sonuçlarıdır.

### Tanı numaraları

| Sembolik | Kod | İleti (EN) |
|---|---|---|
| E2xxx-A | **E2030** | Invalid enum encoding — karışık açık/örtük değer, yinelenen değer, varyantsız enum, işaretsiz olmayan ya da dar taban tipi |
| W2xxx-A | **W2014** | Unreachable match arm (variant already covered) |

İkisi de iki dilde ileti + `volt explain`. E0014'ün kısa başlığı "The
match statement does not cover every value" oldu; açıklaması enum
kapsayıcılığını ve "son kol default" kuralını anlatır.

### Katman yerleşimi

- **Kodlama tablosu tek yerde:** `volt_ast::enum_layout` (`layout`,
  `repr_of`, `valid_layout`, `enum_of_type`). HIR tanıları
  (`typeck/enums.rs`), sv-emit (`enums.rs`), otomatik kontratlar
  (`auto_contract/scan.rs`), test dili (`sim_const.rs`, `sim_port.rs`) ve
  LSP hover aynı fonksiyonu çağırır; her katman kendi sabit
  değerlendiricisini verir.
- **`width_of` değil `signal_width`:** Karar 6 "`width_of` enum için W
  döndürür" diyordu. `TypeArena::width_of` bit seçimini de besliyor
  (`select.rs`); enum'da `Some(W)` dönmesi `s[0]`'ı yasal yapardı (Karar 3
  ihlali). Davranış aynı kalacak biçimde ayrı `signal_width` eklendi
  (sayısalda `width_of`, enum'da kodlama genişliği); W3003 onu kullanır.
  Ölçüm: 3 varyantlı enum `sync()` → W3003, 2 varyantlı → uyarı yok.
- **E0014 ertelemesi:** parser, muhafızsız kolunda yol deseni olan `_`'sız
  `match`'te E0014 vermez; `typeck/matching.rs` sınanan enum ise
  kapsayıcılığa, değilse yol desenlerine E2003 + parser'ın E0014'ünü
  **aynı metinle** verir (golden). Deyim bağlamı dışındaki (tip denetimi
  koşmayan) yerlerde yol desenli match E0014 almaz — o konumlarda yol
  deseni zaten E0003'tür.
- **Desen tiplemesi:** çözümleme `pattern_resolutions` (desen → tanım)
  kaydeder; LSP de bu haritayla desen üzerinde hover verir.

### "Son kol default" — bilgi notu gerekli mi? (Aşama 1 notu 1)

**Hayır — tanı değil, SV yorumu + `volt explain`.** Kapsayıcı `_`'sız her
enum `match`'i bu durumdadır; her birine not basmak doğru tasarımlarda
kalıcı gürültü olur ve tanı üst sınırını (ADR-0068) boşa tüketir. Bilgi
üç yerde: üretilen SV'de `default: begin // State_Done (and invalid
codes)`, `volt explain E0014` metni ve E0014 tanısının notu ("its last arm
also takes the codes no variant uses").

### ADR-0066 düzeltmesi (Aşama 1 notu 2 — yan bulgu 2)

F3, sayısal FSM'de adı geçen literaller yazılan her değeri ve reset
değerini kapsıyorsa `_` kolundan geçiş cover'ı üretmez. Ölçüm
(`build/enum-sv/f1/f1_without.volt`, `--mode cover --depth 12`):

```
önce:  [1/1] Fsm (5 properties) ... FAIL   Fsm.cov_3  E5001 contract violated at cycle 11
sonra: [1/1] Fsm (4 properties) ... ok     Result 4 properties verified
```

Regresyon testleri `auto_contract_tests.rs`
(`numeric_unreachable_wildcard_gets_no_transition_cover`,
`numeric_reachable_wildcard_keeps_its_transition_cover`). Mevcut
`fsm_self_loop_is_not_a_transition` testinin beklentisi bu hatayı
kaydetmişti (yazılan değerler 0 ve 1 ikisi de adlı; `_ -> 0` cover'ı
erişilemez) — beklenti güncellendi, testin amacı (öz-döngü geçiş
sayılmaz) aynen denetleniyor. Ek: FSM tanıyıcı, desenleri register'ın
tipine uymayan (`match` enum register'ında tamsayı deseni ya da tersi —
zaten E2003) match'lerde kontrat üretmez; aksi hâlde kontrat ikinci bir
E2003 doğuruyordu.

### Diğer ayrıntılar

- W2014 yalnız kolun **bütün** varyantları önceki kollarca kapsanmışsa;
  `A | B` kolunda yalnız `A` yinelenirse kol erişilebilir, sessiz.
- Enum `const`'u kullanım yerinde varyant adıyla iner (`START` →
  `State_Run`).
- Ayrı `.sva` dosyası (`--emit=sva`) kendi `localparam`'larını taşır;
  modülün kümesine karışmaz (UNUSEDPARAM).
- `@mmio` enum alanı önce E0015 ("field type must be bool, bits<N> or uN")
  alıyordu; artık Karar 6'daki E0003 iletisi.
- Test dilinde bilinmeyen enum E8506, bilinmeyen varyant ve başka enum'un
  varyantıyla port karşılaştırması E8511; tamsayı enum portuna yazılabilir
  (E8512 genişlik denetimi).
- İngilizce E2003 iletilerinde tamsayı literali "integer literal" (Aşama
  1 notu 3).
- İnceleme bulguları (aynı PR): test dili ve port genişliği enum açık
  değerlerini `const` ve aritmetikle de değerlendirir (`A = K`,
  `B = 1 << 3`; önce yanlış E8506); tipsiz `let t = s` enum sınananı
  olarak tanınır (önce yanlış E0003); aynı satırda birden çok iddia varsa
  rapor enum adı eklemez (hangi iddianın düştüğü satırdan bilinemez).

### Sınırlar (bu tur)

- sv-emit ve test dili enum'u **ada göre** bulur (birleşik birimde öğe
  adları tekil — ADR-0042); test dili ve LSP hover açık değerleri yalnız
  literal/`const` literali olarak değerlendirir (`A = BASE + 1` biçiminde
  varyant test değeri olamaz, hover kodu göstermez; SV ve tip denetimi
  etkilenmez).
- Ertelenen E0014 tip denetimine bağlıdır: `fn` gövdesindeki `match` (tip
  denetimi `fn` gövdelerini koşmaz) ve çözümleme hatası olan birim (boru
  hattı tip denetiminden önce durur) yol desenli `match` için E0014
  almaz; önceki hatalar düzeltilince gelir. `fn` gövdeleri SV'ye zaten
  inmez.
- F1, ADR-0066'nın tanıdığı FSM'lere üretilir: durum register'ı sabit
  yazmalarla sürülmeli; `reg <= next` (comb `wire` üzerinden) FSM
  sayılmaz.

### Doğrulama

- `cargo test --all`: 2929 test (baseline 2850 → 2929), hepsi geçti;
  clippy `-D warnings` temiz.
- Golden (referans: `main` ef107ff = PR #28 + ADR belgesi; `build/parity/
  golden.py`, 370 dosya): enum kullanmayan bütün tasarımların `check`/
  `json`/`build --emit=sva` çıktısı byte-aynı; değişen yalnız beklenen iki
  parite sondası (`p29b` E0003 → E2003, `p35c` E0003 → temiz).
- Enum FSM (`tests/ui/pass/94_enum_fsm.volt`): `check`, `build`, Verilator
  `-Wall` temiz; `volt test` (Docker) 1/1 geçti; `volt verify` prove/cover/
  bmc (derinlik 16, Docker) 4/4 property, F1 dahil. Kasıtlı yanlış iddia
  raporu: `left:  1 (Phase::Go)` / `right: 2 (Phase::Hold)`.
- Mutasyon (tek tek, `CARGO_BUILD_JOBS=2`, `--test-threads=2`,
  `build/enum2/mutate.py`): 13/13 yakalandı — enum == tamsayı/başka enum,
  sıralama denetimi, `uN as Enum`, genişlik clog2 ±1, eksik varyantı
  kapsanmış sayma, `_`'ı yeniden zorunlu kılma, SV `default` kolu,
  kullanılmayan `localparam`, F1 yoğunluk koşulu, F3 sayısal erişilemez
  joker, F3 enum boş joker kaynağı, W2014.

