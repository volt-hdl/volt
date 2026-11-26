# İkili Varsayımlar — Envanter (ADR-0139 Aşama 0)

> Kaynak: ADR-0139 ("Değer kümesi, kodlama ve zaman modeli ayrı
> bilgilerdir"), Aşama 0.
> Tarih: 2026-11-25, `main` 2503284d üzerinde. Satır numaraları o günkü
> koddandır; her satır fonksiyonun `fn` satırını gösterir (tip, sabit ya
> da alan olan yerlerde bildirim satırını; notta yazılı).
> Biçim: `docs/design/tek-yerde-bilgi-envanteri.md` (ADR-0136 Aşama 0).

Bu belge derleyicinin "her şey ikilidir" varsayımını üç eksende sayar:

- **a. Değer kümesi:** değerin 0/1 bitlerinden oluştuğunu varsayan yerler
  (genişliğin bit sayısı olması, 2^N aralıkları, sabit değerlendirme,
  ikili ve onaltılı literaller, bit işlemleri, kaydırma, seçim, sığma
  denetimleri).
- **b. Kodlama:** bir tipin fiziksel temsilini seçen yerler (skaler
  genişlik, enum kodu, struct ve dizi paketleme, Trit'in kodları, test
  dili, dalga biçimi, formal, MMIO ve yazılım genişlikleri).
- **c. Zaman modeli:** saatli eşzamanlı devreyi varsayan yerler (saat ve
  reset, register, `on` blokları, `prev()`, `Delayed`, `sync()`, CDC ve
  RDC, SDC/XDC, simülasyon adımı, formal derinlik).

Her satır:

- **Yer**, **Kullanan** ve kısa bir **Not**;
- **Sınıf:** `dil` (dilin anlamı gereği ikili ya da saatli: spec ya da bir
  ADR bunu tanımlar; doğrudur ve kalır), `arka uç` (yalnız bugünkü ikili
  SV/Verilator/C++ arka ucunun gereği; bir kodlama katmanının arkasına
  taşınabilir), `belirsiz` (kesin değil; neden notta yazılı; sınıfı
  bakımcının kararıdır);
- **Bilgi:** `tek` (bilgi bir ortak yerden okunur ya da tek yerde
  hesaplanır), `dağınık` (bu yer bilgiyi kendisi hesaplar ve aynı bilgi
  başka yerlerde de hesaplanır; kopyalar "Dağınık bilgiler"de).

Zaman ekseninde `dil`, bugünkü dilin saatli anlamıdır (bir `reg` alanının
kenarında güncellenir); olay güdümlü bir modelin bunlardan hangisini
değiştireceği bu envanterin sorusu değildir.

Kısaltmalar: **TD** tip denetimi ve alan çıkarımı (`volt-hir`), **SV**
SV üretimi (`volt-sv-emit`), **SW** üretilen sürücüler (`volt-sw-emit`),
**SDC** kısıt modeli (`volt-hir/src/constraints/`) ve `volt-sdc-emit`,
**SIM** test dilinin simülasyonu (`volt-hir/src/sim*.rs`,
`volt-driver/src/sim*`, SV tarafındaki testbench ve C++ betiği), **LSP**
dil sunucusu, **ORTAK** `volt-ast`'teki ortak yardımcılar, **SYN**
`volt-syntax`, **DRV** `volt-driver`'ın öteki kısımları, **DIFF**
`volt-difftest` (ADR-0108'in derleyiciden bilerek bağımsız başvuru
modeli).

Yöntem: kod okuması; hiçbir şey koşturulmadı. Her satır numarası
2503284d'de dosya okunarak doğrulandı. Sonradan kapanan satırlar
"Kapandı (#N, PR #M)" ile işaretlidir; satır numaraları ve Özet'in
sayıları 2503284d'deki hâldir. Davranış hakkındaki bir iddia
koşturulmadığı için "Açık bulgular" bölümünde "koşturulmadı" diye
işaretlidir. Bir yer iki eksende görünebilir (ör. `past.rs`
`past_reg_block`: genişliği b'de, ilk çevrim anlamı c'de); her eksen onu
kendi açısından sayar.

## Özet

| Eksen | Satır | dil | arka uç | belirsiz | dağınık satır | Dağınık bilgi |
|---|---|---|---|---|---|---|
| a. Değer kümesi | 88 | 42 | 35 | 11 | 53 | 12 |
| b. Kodlama | 87 | 54 | 28 | 5 | 42 | 16 |
| c. Zaman modeli | 114 | 70 | 37 | 7 | 42 | 16 |
| Toplam | 289 | 166 | 100 | 23 | 137 | 44 |

Sayılar fonksiyon düzeyindedir: bir satır bir fonksiyon ya da aynı
dosyada birlikte okunan birkaç küçük fonksiyondur.

Göze çarpanlar:

- **Kodlama dilden görünür.** ADR-0077 Karar 3 (normatif) struct'ın bit
  düzenini `p as uN` ve `raw as P` ile gözlenebilir kılar; yaprak içinde
  "`Trit` ADR-0003'ün 2 bitlik kodu", enum ADR-0074 kodu, dizi alanı
  ADR-0056'nın paketlenmiş vektörüdür. Bu yüzden Trit'in `01`/`11`
  kodları, enum kodu ve dizi paketleme sırası bu envanterde `dil`
  sınıfındadır: bugün yalnız SV'nin değil dilin bilgisidir. Kodlamayı bir
  katmanın arkasına almak struct dönüşümleri için bir dil kararı ister.
- **`Scalar` yalnız bit sayısını tutar.** #419 "Trit 2 bittir" bilgisini
  `volt_ast::bit_width::Scalar` ve `TypeArena::bit_width`'te topladı.
  Trit'in işaretli olduğu, `01`/`11` kodları ve kullanılmayan `10` kodu
  için ortak yer yoktur: bunları bilen 16 yer (bölüm b.E ve "Trit'in
  temsilini bilen yerler"). `bool`, `clock` ve `reset`'in 1 bit olduğu
  `Scalar`'ın dışında 15 yerde sabit yazılıdır.
- **Değer kümesinin bilgisi en dağınık olandır.** Bir (işaret, N)
  tipinin 2^N aralığı yaklaşık 20 yerde yazılmıştır (tip denetiminde 9);
  sekiz sabit değerlendirici vardır ve bit anlamları ayrışır (`~`,
  negatif sayıda `&`, bit kaybeden `<<`, `as`); clog2'nin on uygulaması
  vardır.
- **Olay güdümlü bir tetik için yuva yoktur.** `OnTrigger` `Clock`,
  `Reset` ve `Error`'dur; spec'in `Spike` tetiği (grammar-full.ebnf
  §11, `on spike(Ident)` [V2]) kodda yalnız sözcük rezervasyonudur
  (E0003). "Zaman = çevrim" dört bağımsız sayaçta yaşar: alan çıkarımı,
  L1 zamanlama (register = +1 çevrim, hangi saatin olduğuna bakmadan),
  test dilinin `step(n)`'i (bütün saatler birlikte çevrilir) ve formalin
  `--depth`'i.
- **Kenarsız alan (`clock = none`) bugünkü tek zamansız alan
  kavramıdır.** Kimliği vardır, CDC denetimi onun için de çalışır, flop
  E3016 ile yasaktır. Kuralı yalnız tip denetimi ve SV'nin `flop_audit`'i
  uygular (c.4).

## a. Değer kümesi

### a.1 Genişlik = bit sayısı

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/const_eval.rs:37` `MAX_WIDTH` (sabit); `:108` `width` | ORTAK | 65535 bit; `width` ≥ 1 ve u32'ye sığan değer verir. Birim (bit) dilden, sınır tip denetiminin `u16` gösteriminden (#418). | belirsiz | tek |
| `volt-ast/src/lib.rs:935` `TypeRefKind` (tip) | ORTAK | `UInt(u8)`, `SInt(u8)`, `Bits`, `UIntN`, `SIntN`, `Trit`; `uN` 1..=64 `u8`'de. | dil | tek |
| `volt-syntax/src/token.rs:118`, `:121` `UIntType`/`SIntType`; `parser/item.rs:1539` `parse_int_type` | SYN | `u[1-9][0-9]*`; 1..=64 doğrudan tip, büyüğü Path. | dil | dağınık |
| `volt-hir/src/ty.rs:86` `Ty` (tip); `:209` `int_range`; `:258` `width_of` | TD | `UInt/SInt/Bits { width: u16 }`. | dil | tek |
| `volt-hir/src/typeck/type_ref.rs:15` (`MAX_WIDTH == u16::MAX` derleme denetimi); `:30` `resolve_type_ref_uncached`; `:85` `resolve_sized`; `:117` `widened_int`; `resolve/prelude.rs:36` `is_widened_int_type` | TD | `<N>` → `width: u16`; `u9`/`i33` adlarını kendisi çözer. | dil | dağınık |
| `volt-hir/src/typeck/width.rs:101` `MAX_INT_WIDTH` (sabit, 64); `:106` `clamp_width` | TD | W2016'nın yardımı "en geniş tam sayı 64" der. Geçici sınır mı dil kuralı mı kod söylemez. | belirsiz | dağınık |
| `volt-sv-emit/src/lib.rs:974` `wide_int_type` | SV | 64 bitten geniş `uN` E0003. | arka uç | dağınık |
| `volt-sv-emit/src/inline/types.rs:38` `canon_depth`; `:71` `widened` | SV | fn açılımı `uN` adını kendisi ayrıştırır. | arka uç | dağınık |
| `volt-sv-emit/src/expr.rs:21` `Sig` (tip); `:34` `decl_type`; `:43` `wire_prefix` | SV | `logic [w-1:0]`, `logic signed`. | arka uç | tek |

### a.2 Değer aralığı 2^N, sığma, taşma, kesme

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/typeck/width.rs:111` `uint_fits`; `:116` `sint_fits` | TD | `value >> w == 0`; `value < 2^(w-1)`. | dil | dağınık |
| `volt-hir/src/typeck/check.rs:233` `check_int_lit`; `:284` `check_literal_fits` | TD | E2010; Trit literali E2011. | dil | tek |
| `volt-hir/src/typeck/diag.rs:233` `literal_overflow` | TD | E2010 iletisindeki en büyük değer `(1<<w)-1` yeniden hesaplanır. | dil | dağınık |
| `volt-hir/src/typeck/cast.rs:35` `warn_literal_cut` | TD | W2010: alt N bit; negatifte ikiye tümleyen; iletide `0x…`. | dil | dağınık |
| `volt-hir/src/typeck/literal_fold.rs:363` `literal_range`; `:387` `literal_out_of_range`; `:210` `check_literal_expr` | TD | (i128, u128) aralığı; Trit −1..=1. | dil | dağınık |
| `volt-hir/src/typeck/compare_width.rs:29` `fits`; `:43` `labels`; `:65` `check_comparison_widths` | TD | E2031; "N bitte sarar". | dil | dağınık |
| `volt-hir/src/typeck/arith_cut.rs:291` `fits`; `:254` `type_bounds`; `:149` `arith_width`; `:55` `warn_cut_arith_operands` | TD | W2016; dosyada iki ayrı aralık hesabı. | dil | dağınık |
| `volt-hir/src/typeck/binop.rs:112` `warn_cut_division_operands` | TD | W2015: işlenen hi bite kesilir. | dil | tek |
| `volt-hir/src/typeck/matching.rs:508` `int_fits`; `:212` `check_const_pattern_fits` | TD | Desendeki sabit için E2010. | dil | dağınık |
| `volt-hir/src/typeck/enums.rs:70` `report_layout_issue` | TD | E2010: enum değeri `(1<<w)-1`'i aşar. | dil | dağınık |
| `volt-syntax/src/parser/mono/mod.rs:141` `ParamRange::max`; `:804` `err_arg_out_of_range` | SYN | Generic argümanında E2010. | dil | dağınık |
| `volt-syntax/src/parser/auto_contract/mod.rs:149` `max_value`; `counter.rs:27` `specs`, `:66` `recognize`; `fsm.rs:197` `recognize` | SYN | Sayaç sarma kontratı `2^w - 1`. | dil | dağınık |
| `volt-hir/src/sim_port.rs:44` `max_pattern`, `:53` `min_signed`, `:61` `fits_pattern`, `:68` `accepts`, `:77` `to_pattern`, `:92` `write_range` | SIM | ADR-0059: değer 64 bitlik desen; negatif sayı 64 bitte sarar. Kural test dilinin, gerekçesi C++ `uint64`. | belirsiz | dağınık |
| `volt-hir/src/sim_port.rs:260` `check_set_port_value`; `:282` `check_value_fits`; `:308` `check_assert_compare` | SIM | E8512; port bit deseni olarak okunur. Gerekçe yukarıdaki gibi. | belirsiz | tek |
| `volt-sv-emit/src/sim_script.rs:68` `PORT_PRELUDE` (sabit, C++); `:50` `LOAD_PRELUDE` | SIM | `volt_port_fits`; yorumu "`PortWidth::accepts` ile aynı" der. | arka uç | dağınık |
| `volt-hir/src/sim_load.rs:46` `check_load` | SIM | `info.max >> bits != 0`. | dil | dağınık |
| `volt-driver/src/sim_lower.rs:288` `set_port`; `:332` `fits_without_check` | SIM | "0 ve 1 her porta sığar". | dil | tek |
| `volt-sv-emit/src/builtin_prim.rs:965` `builtin_contracts` | SV | Counter `count < 2^W`; AsyncFifo işaretçi farkı mod 2^pw; adres genişliği satır içi `trailing_zeros` (iki kez). | arka uç | dağınık |

### a.3 Sabit değerlendirme (i128, u128, u64)

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/const_eval.rs:93` `eval`; `:100` `const_int`; `:164` `Eval::int`; `:245` `Eval::bool`; `:433` `arith` | ORTAK | i128; `& \| ^` yalnız negatif olmayanda; bit kaybeden `<<` `None`; `~` ve `as` yok; bool 0/1. Ana makinenin i128'i: ne dil kuralı ne SV gereği. | belirsiz | dağınık |
| `volt-hir/src/consteval.rs:304` `const_eval_uncached` (`:400` Cast kolu); `:646` `eval_binop`; `:769` `literal_overflow`; `:797` `eval_unop`; `:980` `shift_overflow` | TD | i128, E2022; `as` kesmez; `~a` = i128 `!a`; `checked_shl` yalnız miktarı denetler. | belirsiz | dağınık |
| `volt-hir/src/typeck/literal_fold.rs:105` `literal_sign_mag`; `:138` `eval_literal`; `:448` `literal_op` | TD | Soneksiz literal aritmetiği i128 (#144). | belirsiz | dağınık |
| `volt-sv-emit/src/expr.rs:167` `literal_op`; `:227` `fold_literal_arith`; `:250` `literal_value` | SV | TD `literal_fold`'un kopyası. | belirsiz | dağınık |
| `volt-sv-emit/src/expr.rs:213` `eval_const` | SV | ORTAK `eval`'i sarar. | belirsiz | tek |
| `volt-sv-emit/src/expr.rs:1269` `shift_amount` | SV | `as uN` = mod 2^N (ADR-0115); TD consteval kesmez (tek-yerde envanteri, ayrışma 2). | dil | dağınık |
| `volt-syntax/src/parser/mono/unroll.rs:580` `eval_const`; `:592` `eval_const_scoped` | SYN | Ayrıştırıcının değerlendiricisi: i128, `checked_shl/shr`, `& \| ^` yok. | belirsiz | dağınık |
| `volt-syntax/src/parser/mono/clone.rs:307` `fold_binary` | SYN | İki literalin `+ - * / %`'si. | belirsiz | dağınık |
| `volt-hir/src/sim_const.rs:144` `TestConsts::eval`; `:209` `plain_literal` | SIM | u64; `-1` → 2^64−1; "anlam üretilen C++ ile aynı". | arka uç | dağınık |
| `volt-hir/src/sim_port.rs:202` `fold_binary` | SIM | u64 sarma; 64 ve üstü kaydırma 0. | arka uç | dağınık |
| `volt-sv-emit/src/sim_script.rs:141` `cpp_value`; `:155` `cpp_binary` | SIM | C++ `unsigned long long` üzerinde `& \| ^`, `volt_shl`. | arka uç | dağınık |

### a.4 clog2 ve ikili adres

Yerleşik primitiflerin adres portu (`bits<clog2(DEPTH)>`) ve DEPTH'in iki
kuvveti olması kodlama ekseninde (b.F) sayılır.

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/consteval.rs:834` `eval_builtin_call` (Clog2 kolu) | TD | `128 - lz(n-1)`; yerleşik `clog2`'nin anlamı ikilidir. | dil | dağınık |
| `volt-sv-emit/src/expr.rs:1363` `address_width` | SV | `32 - lz(len-1)` (Verilator WIDTH, #123). | arka uç | dağınık |
| `volt-sv-emit/src/expr.rs:1425` `part_select_from_value` | SV | Satır içi `128 - lz(down)` ve `down < (1<<w)`. | arka uç | dağınık |
| `volt-sv-emit/src/const_array.rs:158` `case_function` | SV | `iw = clog2(len)`. | arka uç | dağınık |

### a.5 Literaller

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-syntax/src/token.rs:42` `IntLit`, `:46` `InvalidNumber` (`TokenKind` varyantları); `lexer.rs:69` `check_token` | SYN | `0x`/`0b`/`0o` ve `u8..i64` sonekleri; E0005. | dil | tek |
| `volt-syntax/src/parser/expr.rs:992` `parse_int_lit`; `:1023` `parse_int_text` | SYN | u128; sığmayan E0005. | dil | dağınık |
| `volt-syntax/src/parser/test.rs:307` `parse_int_text`; `test_expr.rs:142` `parse_test_primary` | SYN | Test dilinin ikinci ayrıştırıcısı: u64, büyük harfli önekleri de kabul eder. | arka uç | dağınık |
| `volt-syntax/src/parser/pipeline.rs:280` `parse_stage_ref` | SYN | `expr.rs`'inkini kullanır. | dil | tek |
| `volt-syntax/src/parser/auto_contract/print.rs:102` `int_text` | SYN | Literali kendi tabanında geri yazar. | dil | tek |
| `volt-ast/src/lib.rs:1315` `NumBase`; `:1323` `IntSuffix` (tip) | ORTAK | Dec, Hex, Bin, Oct. | dil | tek |
| `volt-hir/src/typeck/synth.rs:101` `synth_int_lit`; `:501` `suffix_ty` | TD | Sonek → (işaret, 8/16/32/64). | dil | dağınık |
| `volt-sv-emit/src/expr.rs:1972` `suffix_sig` | SV | `suffix_ty`'nin kopyası. | dil | dağınık |
| `volt-sv-emit/src/expr.rs:1870` `fmt_int`; `:1937` `LitValue` (tip); `:98` `widen_sig` | SV | `W'[s]d/h/b/o`; en küçük negatif `W'shPAT` (ikiye tümleyen); E2005. | arka uç | dağınık |
| `volt-sv-emit/src/expr.rs:1522` `fold_const_path`; `const_array.rs:72` `try_emit_const_array_index` | SV | Sabiti boyutlu literale katlar (`fmt_int`). | arka uç | tek |
| `volt-sv-emit/src/lib.rs:287` `zero_of` | SV | `1'b0`, `W'd0`, `W'sd0`. | arka uç | dağınık |
| `volt-sv-emit/src/enums.rs:176` `enum_localparams`; `match_expr.rs:396` `end_process` | SV | Elle yazılmış `W'dV`, `W'd0`. | arka uç | dağınık |
| `volt-sv-emit/src/builtin_prim.rs:768` `emit_counter` ve öteki `emit_*` şablonları | SV | Elle yazılmış yaklaşık 59 `'d` ve `{W{1'b1}}`. | arka uç | dağınık |
| `volt-sw-emit/src/check/rtl.rs:252` `sv_literal`; `:138` `hex_literals` | SW | Üretilen SV literalini geri okur (`d/h/b`; `o` ve `s` yok). | arka uç | dağınık |
| `volt-lsp/src/hover.rs:242` `enum_hover` | LSP | `= W'dV` gösterir; değeri yalnız `IntLit`'ten okur. | arka uç | dağınık |

### a.6 Bit işlemleri, kaydırma, seçim, birleştirme, işaret genişletme

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/lib.rs:1335` `BinOp`; `:1394` `UnOp`; `:1287` `Concat` (tip) | ORTAK | `& \| ^ << >> ~`; `>>>`, indirgeme ve XNOR yok; `Concat` kaynakta yazılamaz, struct indirgemesi kurar. | dil | tek |
| `volt-syntax/src/parser/expr.rs:21` `infix_binding_power`; `:52` `token_binop`; `:82` `w0010_pair`; `:271` `parse_prefix`; `:664` `parse_postfix` | SYN | `[i]`, `[h:l]`, `+:`, `-:`. | dil | tek |
| `volt-syntax/src/parser/test_expr.rs:23` `binary_op` | SYN | Test dilinde `& \| ^ << >>` u64 üzerinde; `~` yok. | arka uç | tek |
| `volt-hir/src/typeck/binop.rs:19` `synth_binary`; `:242` `synth_bitwise`; `:280` `bitwise_with_literal` | TD | Genişleme yok; `bits<a>` ile `bits<b>` eşit (E2001); Bool & Bool → Bool. | dil | tek |
| `volt-hir/src/typeck/binop.rs:299` `synth_shift` | TD | W2013; işaretli `>>` işaret bitiyle dolar. | dil | tek |
| `volt-hir/src/typeck/binop.rs:53` `arith_of`; `:150` `arith_result` | TD | `bits<N>` aritmetiği E2004; `+ -` +1 bit, `*` ×2 (ADR-0025). | dil | dağınık |
| `volt-hir/src/typeck/binop.rs:187` `trit_arith_result` | TD | Trit ± Trit → i3, Trit × iN → iN. | dil | dağınık |
| `volt-hir/src/typeck/synth.rs:260` `synth_unary` | TD | `~` işlenenin tipini korur (spec type-inference.md §3.4: `UnOp::BitNot => ot`), Trit dahil; `-iN` → i(N+1). | dil | tek |
| `volt-hir/src/typeck/select.rs:15` `index_result`; `:61` `range_result`; `:94` `const_range_result`; `:139` `part_select_result`; `:177` `part_select_width`; `:203` `const_select_in_bounds` | TD | Bit seçiminin sonucu Bool; aralık `bits<h-l+1>`; E2006, E2007. | dil | tek |
| `volt-hir/src/typeck/stmt.rs:379` `suffix_bits` | TD | Sol değerde indeks tek bit seçer. | dil | tek |
| `volt-hir/src/typeck/synth.rs:163` `synth_call`; `resolve/prelude.rs:10` `BUILTINS` (sabit) | TD | `zext/sext/trunc/concat/replicate/popcount` adları prelude'da, tipleri hata tipi, SV karşılığı yok. | dil | tek |
| `volt-sv-emit/src/expr.rs:111` `sv_prec`; `:128` `is_bitwise`; `:1171` `emit_binary` | SV | İşaretli `>>` → `>>>`. | arka uç | tek |
| `volt-sv-emit/src/expr.rs:1255` `clamped_signed_shift` | SV | Miktar ≥ w ise w−1 yazılır (Verilator). | arka uç | tek |
| `volt-sv-emit/src/expr.rs:802` `emit_prec` | SV | `~`'nin genişliği yalıtılır; Bool → `1'b1`/`1'b0`. | arka uç | tek |
| `volt-sv-emit/src/expr.rs:507` `widen_if_narrow`; `:621` `fit_bitwise_operand`; `:1762` `width_dependent`; `:1843` `isolate_if_dependent` | SV | `W'($unsigned(..))`. | arka uç | tek |
| `volt-sv-emit/src/expr.rs:1293` `select_from_value`; `:1315` `is_selectable_base`; `:1352` `scalar_select`; `:1375` `index_width_mismatch`; `:1395` `sized_index`; `:1411` `is_scalar_base`; `:1467` `cut_value` | SV | Seçim = kaydırma + boyut dönüşümü. | arka uç | tek |
| `volt-sv-emit/src/expr.rs:1579` `emit_cast` | SV | `{{n{x[msb]}}, x}`, `{{n{1'b0}}, x}`, `x[w-1:0]`. | arka uç | tek |
| `volt-sv-emit/src/expr.rs:291` `own_width` | SV | Karşılaştırma ve bit seçimi 1 bit. | arka uç | dağınık |
| `volt-sv-emit/src/let_width.rs:183` `int_range`; `:254` `binary_range`; `:304` `operand_meet` | SV | +1 / ×2 kuralının kopyası. | arka uç | dağınık |
| `volt-sv-emit/src/builtin_prim.rs:865` `emit_round_robin_arbiter`; `:320` `emit_async_fifo` | SV | `x & (~x + 1)`; Gray kod `(b>>1)^b`. | arka uç | tek |
| `volt-sv-emit/src/lib.rs:1687` `emit_bidir_drivers`; `instance.rs:319` `bidir_binding` | SV | `1'bz`: dört değerli SV'nin Z'si. | arka uç | tek |

### a.7 Değer biçimleme

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-sv-emit/src/sim_width.rs:40` `WIDE_PRELUDE` (sabit, C++) | SIM | 64 bitten genişi onaltılık sözcüklerle yazar. | arka uç | tek |
| `volt-hir/src/sim_port.rs:232` `describe_value` | SIM | u64 → i64 gösterimi. | arka uç | dağınık |
| `volt-driver/src/sim_struct.rs:30` `LeafLabel::value`; `:39` `render` | SIM | Maske ve ikiye tümleyen işaret genişletme. | arka uç | dağınık |
| `volt-hir/src/testdata.rs:152` `parse_hex_token`; `:184` `parse_readmemh` | SIM | Basamak başına 4 bit; x/z reddedilir; 64 bit. | arka uç | tek |

### a.8 Fark testinin başvuru modeli (ADR-0108)

DIFF bilerek bağımsızdır (crate'in `volt-*` bağımlılığı yoktur); oradaki
kopya bir kusur değil, yöntemin kendisidir. Değer kümesi değişirse onun
modeli de değişir.

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-difftest/src/value.rs:5` `mask`; `:15` `conv`; `:28` `to_i128`; `:38` `fits` | DIFF | u128 desen; ikiye tümleyen. | dil | dağınık |
| `volt-difftest/src/model.rs:264` `synth`; `:402` `binary`; `:552` `int_arith`; `:607` `lit_arith`; `:650` `cast` | DIFF | Tip kurallarının bağımsız uygulaması. | dil | dağınık |
| `volt-difftest/src/model.rs:626` `arith_bits`; `:693` `eval`; `:790` `eval_bin` | DIFF | Sarma, maske, kaydırma; x `None`. | dil | dağınık |
| `volt-difftest/src/gen.rs:44` `floor_log2`; `:89` `lit`; `:586` `wide_lit` | DIFF | | dil | dağınık |
| `volt-difftest/src/harness.rs:684` `vectors` | DIFF | Köşe değerleri: 0, hepsi 1, yalnız MSB. | dil | tek |
| `volt-difftest/src/harness.rs:739` `BitVec::hex`; `print.rs:42` `lit_text` | DIFF | | arka uç | tek |

## b. Kodlama

Sınıflamanın kaynakları: ADR-0003 (Trit {−1, 0, +1}, 2 bit işaretli
depolama), ADR-0062 (kodlar +1 = `01`, 0 = `00`, −1 = `11`; `10`
kullanılmaz), ADR-0077 Karar 3 (struct düzeni, normatif; yaprak içinde
"`Trit` ADR-0003'ün 2 bitlik kodu"), ADR-0074 Karar 2 (enum ikili, 0'dan
sıralı, `W = max(1, clog2 n)`; one-hot ve Gray ertelendi), ADR-0056 §4
(dizi port ve teli paketlenmiş vektör, eleman 0 en düşük bitlerde;
gerekçesi Yosys), ADR-0029 (yerleşik primitifler: adres `bits<clog2(DEPTH)>`,
DEPTH iki kuvveti, `taps = LEN × genişlik(T)`), ADR-0044 (MMIO alanı en
çok 32 bit), ADR-0058 ve ADR-0059 (test değerleri u64), ADR-0087 (RAM
öğesinde enum ya da Trit olmaz).

### b.A Skaler genişlik (Bool, Clock, Reset 1 bit; Trit 2 bit)

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/bit_width.rs:29` `Scalar::bits`; `:37` `Scalar::of` | ORTAK | Tablonun kendisi; yalnız bit sayısı (işaret ve kod tablosu yok). | dil | tek |
| `volt-ast/src/bit_width.rs:66` `type_ref_bits` (`:70` `bits_at`) | ORTAK | AST tipinin genişliği; SV testbench ve SIM okur. | dil | tek |
| `volt-hir/src/ty.rs:310` `TypeArena::bit_width` | TD | Tip denetiminin tek kaynağı (#419). | dil | tek |
| `volt-hir/src/ty.rs:319` `TypeArena::is_signed` (Trit kolu `:321`) | TD | Eklendi (#413, PR #440; satır numarası o daldan, Özet'in sayılarına girmez). İşaretin tek fonksiyonu, `bit_width`'in yanında (ADR-0141). Trit kolu `true`: Trit'in 2 bitlik ikiye tümleyen kodlamasının bilgisi (ADR-0062), değer kümesi {−1, 0, +1} değil (ADR-0139). Aynı bilgi ORTAK `leaf_or_nested` ve SV `sig_of_typeref`'te de yazılı; #436'nın izin listesine girer. | arka uç | dağınık |
| `volt-ast/src/checked.rs:40` `CheckedTypes` (`:24` `CheckedValue`) | ORTAK | Eklendi (#413, PR #440). TD'nin cevabını SV'ye taşıyan tablo (ADR-0141): `Bits { width, signed }` `bit_width` ve `is_signed`'dan gelir; tablo hesap yapmaz, Trit'i ayrıca tanımaz (Trit `Bits { width: 2, signed: true }` olarak gelir: kodlama bilgisi). ADR-0136 §3'ün ara temsili değildir; IR kararıyla taşınır ya da kaldırılır. | arka uç | tek |
| `volt-hir/src/typeck/checked.rs:19` `TypeckResult::checked_types` (`:32` `checked_value`) | TD | Eklendi (#413, PR #440). Tablonun tek doldurucusu: her tip kolu `bit_width` ve `is_signed`'dan, struct alanları `struct_fields`'tan. | arka uç | tek |
| `volt-hir/src/ty.rs:258` `TypeArena::width_of` | TD | Yalnız sayısal tipler; Bool ve Trit `None`. | dil | tek |
| `volt-ast/src/struct_layout.rs:224` `leaf_or_nested` | ORTAK | Genişlik `Scalar`'dan; Trit'in `signed: true`'su ve `LeafKind::Trit` burada yazılır. | dil | dağınık |
| `volt-ast/src/struct_layout.rs:291` `array_elem_width` | ORTAK | `TypeRefKind::Bool => Ok(1)` sabit (satır 303); aynı dosyanın 239. satırı `Scalar::Bool.bits()` okur. | dil | dağınık |
| `volt-sv-emit/src/lib.rs:2513` `sig_of_typeref` | SV | Genişlik `Scalar`'dan; Trit `logic signed [1:0]` (işaret burada). | arka uç | dağınık |
| `volt-sv-emit/src/expr.rs:26` `impl Sig` (`:28` `Sig::BIT`); `lib.rs:1056` `emit_module` | SV | Reset portları ve senkronizör aşamaları sabit 1 bit. | arka uç | dağınık |
| `volt-sv-emit/src/lib.rs:2442` `port_sig` | SV | Ham reset `Sig::BIT`. | arka uç | dağınık |
| `volt-sv-emit/src/instance.rs:147` `emit_user_instance` | SV | Saat bağlaması sabit 1 bit. | arka uç | dağınık |
| `volt-sv-emit/src/lib.rs:2303` `emit_if`; `sva.rs:140` `ONE_BIT` (sabit) | SV | Koşul ve kontrat 1 bit. | arka uç | dağınık |
| `volt-sv-emit/src/let_width.rs:254` `binary_range` | SV | Karşılaştırma `Sig::BIT`; Trit için `trit_sum_sig`. | arka uç | dağınık |
| `volt-sv-emit/src/sim.rs:63` `collect_sim_ports` | SIM | Port bitleri `type_ref_bits`'ten; otomatik `rst` `Some(1)` sabit. | arka uç | dağınık |
| `volt-hir/src/timing.rs:736` `stage_register_help` (`:897` `register_cost`) | TD | E5010 önerisinin kendi genişlik tablosu: Bool 1, sayısal N; Trit, enum, struct, dizi `None` (bedel yazılmaz). `bit_width`'i okumaz. Bedelin birimi flop. | belirsiz | dağınık |
| `volt-syntax/src/parser/mmio.rs:106` `FieldTy::width` | SYN, SW | `Bool => 1` sabit. | dil | dağınık |
| `volt-difftest/src/ast.rs:14` `PortTy::width` | DIFF | `Bool => 1`; bilerek bağımsız. | belirsiz | dağınık |
| `volt-hir/src/typeck/stmt.rs:316` `note_coverage`; `:379` `suffix_bits` | TD | E4012; `bit_width` ve düzenin W'si. | dil | tek |
| `volt-hir/src/domain/assign.rs:265` `is_one_bit`; `domain/sync.rs:14` `sync_domain` | TD | W3003: `bit_width == 1`. | dil | tek |
| `volt-hir/src/typeck/instance.rs:168` `taps_width`; `:185` `check_taps_width` | TD | `Taps = bits<dim × bit_width(T)>`; E2025. | dil | tek |
| `volt-sv-emit/src/builtin_prim.rs:83` `builtin_inst_info` | SV | T'si olmayan primitif 1 bit. | arka uç | dağınık |

### b.B Enum kodu

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/enum_layout.rs:180` `layout`; `:164` `bit_len`; `:169` `width_for_count` | ORTAK | İkili, bildirim sırasıyla 0'dan; `max(1, clog2 n)`; one-hot ve Gray yok. | dil | tek |
| `volt-ast/src/enum_layout.rs:106` `repr_of`; `:260` `valid_layout`; `:25` `EnumLayout::is_dense` | ORTAK | Taban tipi işaretsiz; "dense": 2^W kodun hepsi kullanılıyor. | dil | tek |
| `volt-hir/src/typeck/enums.rs:23` `check_enum_decls`; `ty.rs:279` `set_enum_width`; `:284` `enum_width` | TD | | dil | tek |
| `volt-hir/src/typeck/cast.rs:142` `check_enum_cast` | TD | `E as uN` yalnız N ≥ W; `uN as E` E2009. | dil | tek |
| `volt-hir/src/consteval.rs:628` `variant_value` | TD | Kodu kendisi hesaplar, `enum_layout`'u çağırmaz; ürettiği `discriminant` hiçbir yerde okunmaz. | dil | dağınık |
| `volt-ast/src/const_eval.rs:338` `variant_code` (`:293` `match_key`) | ORTAK | SV ve SIM okur. | dil | tek |
| `volt-sv-emit/src/enums.rs:23` `enum_layout`; `:28` `enum_sig`; `:107` `enum_variant_sig` | SV | `logic [W-1:0]`; `localparam`. | arka uç | tek |
| `volt-sv-emit/src/enums.rs:372` `enum_match_plan` | SV | Dense değilse `default` kolu (ADR-0074 K4). | dil | tek |
| `volt-sv-emit/src/builtin_data.rs:31` `default_literal` (`:51` `first_code`, `:57` `set`, `:66` `literal`) | SV | Reset değeri ilk varyantın kodu (ADR-0087); bit dizisini kendisi kurar. | dil | tek |
| `volt-sv-emit/src/waves.rs:320` `enum_table`; `:134` `binary`; `:140` `filter_text` | SV | Dalga süzgeci: kodlar ikili basamakla; 2^W kodun hepsi listelenir (ADR-0092). | arka uç | tek |
| `volt-hir/src/sim_const.rs:40` `TestConsts::new` | SIM | Varyant kodları `valid_layout`'tan. | dil | tek |
| `volt-hir/src/sim_port.rs:110` `scalar_width` | SIM | Enum kod genişliğinde işaretsiz sayı; Trit, saat, reset, dizi, demet "sayı değil". | dil | tek |
| `volt-driver/src/sim_lower.rs:107` `EnumLabels::label` | SIM | Koddan ada; eşleşmezse "invalid code". | arka uç | tek |
| `volt-syntax/src/parser/auto_contract/scan.rs:235` `enum_width`; `fsm.rs:38` `specs`; `mod.rs:107` `value_of` | SYN | F1 kontratı yalnız dense olmayan enum'a (ADR-0074 K6). | dil | tek |
| `volt-lsp/src/hover.rs:242` `enum_hover` | LSP | `State::Run = 2'd1`. | dil | tek |

### b.C Struct paketleme

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/struct_layout.rs:173` `layout` | ORTAK | İlk yaprak MSB'de; W yaprakların toplamı (ADR-0077 K3). | dil | tek |
| `volt-ast/src/struct_layout.rs:109` `field_bits`; `:68` `msb`; `:73` `Leaf::bits`; `:96` `has_enum_or_trit` | ORTAK | | dil | tek |
| `volt-hir/src/typeck/structs.rs:23` `check_struct_decls`; `:172` `struct_layout` | TD | | dil | tek |
| `volt-hir/src/typeck/structs.rs:266` `check_struct_cast`; `:316` `check_bits_to_struct` | TD | `p as uN` N ≥ W; `raw as P` N == W ve P'de enum ya da Trit yaprağı yok (geçersiz kod). | dil | tek |
| `volt-hir/src/typeck/instance.rs:217` `check_memory_element` | TD | RAM öğesinde enum ya da Trit E2009 (ADR-0087). | dil | tek |
| `volt-sv-emit/src/structs/value.rs:229` `project`; `:397` `concat_of` | SV | `leaf.lsb`/`msb` dilimleri. | dil | tek |
| `volt-sv-emit/src/builtin_data.rs:20` `struct_width` | SV | | dil | tek |
| `volt-sv-emit/src/sim.rs:122` `struct_leaves` | SIM | Yaprak başına bir testbench portu. | arka uç | tek |
| `volt-hir/src/sim_struct.rs:40` `struct_port_layout`; `:51` `find_port`; `:431` `check_struct_compare` | SIM | Bütün struct için 64 bit sınırı. | dil | tek |
| `volt-driver/src/sim_struct.rs:122` `packed_port`; `:136` `packed_literal` | SIM | Kaydır ve OR ile MSB'den paketler; `leaf.lsb`'yi okumaz. | dil | dağınık |
| `volt-driver/src/sim_struct.rs:86` `labels` | SIM | lsb ve genişlik düzenden; işaretli yaprağı kendisi çözer (b.E). | arka uç | tek |
| `volt-lsp/src/hover.rs:105` `def_hover`; `:179` `layout_fields`; `:197` `field_hover` | LSP | `field_hover` `[msb:lsb]` metnini kendisi yazar. | dil | tek |

### b.D Dizi, `Delayed`, demet

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-sv-emit/src/lib.rs:2416` `signal_sig`; `:2462` `array_reg_sig`; `:2615` `leaf_sig` | SV | `elem.width * len` üç yerde; `type_ref_bits` okunmaz. ADR-0056 §4'ün gerekçesi Yosys, ama ADR-0077 K3 ve test dili paketlemeyi gözlenebilir kılar. | belirsiz | dağınık |
| `volt-sv-emit/src/lib.rs:2681` `packed_select` | SV | `[W*i +: W]`, eleman 0 LSB'de; gerekçe yukarıdaki gibi. | belirsiz | dağınık |
| `volt-sv-emit/src/structs/value.rs:352` `pack_array` | SV | `[a, b]` → `{b, a}` (ADR-0077 K3). | dil | dağınık |
| `volt-hir/src/sim_port.rs:155` `port_type_width` | SIM | Paketli dizi portu tek bir değer (`[u8; 4]` 32 bitlik sayı). | belirsiz | tek |
| `volt-syntax/src/parser/item.rs:1489` `desugar_delayed_type` | SYN | `Delayed<T, N>` T'ye iner; genişlik T'ninki (ADR-0037). | dil | tek |

Demet tipinin bit düzeni tanımlı değildir: `bit_width` ve `type_ref_bits`
`None` verir, SV E0003 verir, SIM onu "sayı değil" sayar.

### b.E Trit

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-sv-emit/src/trit.rs:17`–`:18` `TRIT_POS`, `TRIT_NEG` (sabit) | SV | `2'sb01`, `2'sb11`. ADR-0077 K3 kodu struct dönüşümünde gözlenebilir kılar. | dil | dağınık |
| `volt-sv-emit/src/trit.rs:113` `try_emit_trit_mul` | SV | Çarpansız seçici; `10` `else` dalında 0'a gider (ADR-0062 K2). | arka uç | dağınık |
| `volt-sv-emit/src/trit.rs:102` `trit_sum_sig` | SV | Trit ± Trit = 3 bit işaretli; TD `trit_arith_result`'ın kopyası. | dil | dağınık |
| `volt-sv-emit/src/trit.rs:22` `is_trit_typeref`; `:41` `note_trit`; `:50` `is_trit`; `lib.rs:858` `Emitter` (`:873` `trits` alanı); `lib.rs:1426` `emit_body` | SV | Trit-lik AST'den yeniden çıkarılır (ADR-0062 K4); tanınmayan biçimde jenerik `*`, yalnız i2 ikiye tümleyende doğru. | arka uç | dağınık |
| `volt-sv-emit/src/expr.rs:802` `emit_prec` | SV | `try_emit_trit_mul`'u çağırır. | arka uç | tek |
| `volt-sv-emit/src/waves.rs:172` `trit_table`; `:164` `invalid_label` | SV | `width: 2`, `00`/`01`/`11`; `10` "invalid −2"; kodları `TRIT_POS`/`TRIT_NEG`'den okumaz. | arka uç | dağınık |
| `volt-sv-emit/src/waves.rs:254` `wave_signal_of`; `:275` `instance_wave_outputs` | SV | Trit için sabit `width: 2` (iki yer). | arka uç | dağınık |
| `volt-hir/src/typeck/binop.rs:187` `trit_arith_result` | TD | `*` kapalı, `±` i3, `Trit * iN` iN (ADR-0003). | dil | tek |
| `volt-hir/src/typeck/cast.rs:83` `check_cast_legal` | TD | `Trit → iN` yalnız `width >= 2` (satır 119); sayısal → Trit E2009; `Bool ↔ u1`; 2 ve 1 sabit, `Scalar` okunmaz. | dil | dağınık |
| `volt-hir/src/typeck/check.rs:233` `check_int_lit`; `literal_fold.rs:387` `literal_out_of_range`; `width.rs:86` `is_literal_adaptable` | TD | E2011; değer kümesi {−1, 0, 1} iki yerde yazılı (kodlama değil, değer kümesi). | dil | dağınık |
| `volt-hir/src/typeck/select.rs:15` `index_result` | TD | Trit bit seçilemez; kod dışarı açılmaz. | dil | tek |
| `volt-syntax/src/token.rs:171`–`:172` `Spike`, `PTrit` (`:192` `Reserved`) | SYN | Yalnız rezerve (ADR-0028); temsil yok. | dil | tek |
| `volt-driver/src/sim_struct.rs:39` `LeafLabel::render` | SIM | Struct içindeki Trit'i 2 bit ikiye tümleyen çözer (`11` → −1, `10` → −2). | arka uç | dağınık |

### b.F Adres ve indeks genişliği, tek-sıcak

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/builtin.rs:363` `const_rule`; `:86` `ConstRule::allows` | ORTAK | DEPTH iki kuvveti, 2..=65536 (ADR-0029); gerekçesi ikili Gray işaretçi. | dil | tek |
| `volt-hir/src/typeck/instance.rs:129` `builtin_port_type` | TD | Addr = `bits<dim.trailing_zeros()>`; Taps `bit_width`'ten. | dil | dağınık |
| `volt-sv-emit/src/expr.rs:438` `builtin_field_sig` | SV | TD `builtin_port_type`'ın kopyası: Bool ve Clock 1 bit, Addr `trailing_zeros`, Taps `dim × width`. | arka uç | dağınık |
| `volt-sv-emit/src/builtin_prim.rs:49` `BuiltinInst::addr_width` | SV | `trailing_zeros`. | arka uç | dağınık |
| `volt-sv-emit/src/builtin_prim.rs:1492` `builtin_init_lines` | SV | Satır içi `trailing_zeros`. | arka uç | dağınık |
| `volt-sv-emit/src/builtin_prim.rs:908` `emit_priority_arbiter` | SV | Grant tek-sıcak; kontrat `(g & (g-1)) == 0` (ADR-0029). | dil | tek |

### b.G Test dili

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/sim_port.rs:20` `SCRIPT_VALUE_BITS`; `sim_struct.rs:30` `MAX_WHOLE_STRUCT_BITS`; `sim_load.rs:22` `MAX_LOAD_ELEM_BITS`; `volt-sv-emit/src/sim_width.rs:14` `MAX_SCALAR_BITS` (sabitler) | SIM | Dört ayrı 64 sabiti (C++ u64/`QData`). | arka uç | dağınık |
| `volt-hir/src/sim_const.rs:209` `plain_literal`; `volt-driver/src/sim_lower.rs:127` `lower_value`; `:166` `array_data` | SIM | bool → 0/1 (ADR-0058). | dil | dağınık |
| `volt-driver/src/sim_lower.rs:584` `port_check` | SIM | `PortWidth` okur. | dil | tek |
| `volt-sv-emit/src/sim_width.rs:18` `may_be_wide`; `:24` `show_width`; `:29` `drive` | SIM | 64 bitten geniş port `VlWide<N>`, 32 bitlik sözcükler (ADR-0095 §5). | arka uç | tek |
| `volt-hir/src/sim_load.rs:161` `resolve_load_target` | SIM | Eleman bitleri `scalar_width`'ten, en çok 64. | dil | tek |
| `volt-driver/src/sim/test_waves.rs:105` `session_hint`; `volt-driver/src/waves.rs:129` `with_plain_traces` | DRV | GTKWave adı `name[W-1:0]`. | arka uç | dağınık |
| `volt-sv-emit/src/waves.rs:125` `gtkwave_name` | SV | Aynı ad kuralı. | arka uç | dağınık |

### b.H Formal

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-sv-emit/src/past.rs:73` `past_reg_block` | SV, SIM | Genişlik `self.width_of(x).map(..).unwrap_or(1)` (satır 100): #419'un kaldırdığı "bilinmiyorsa 1 bit" kalıbı; yardımcı register `logic [W-1:0]` işaretsiz. **Kapandı (#413, PR #440):** genişlik ve işaret `prev_sig`'den (`CheckedTypes`, TD'nin `bit_width` ve `is_signed`'ı); işaretli tip `logic signed`; genişlik yoksa E2005, 1 bit yok. | arka uç | dağınık |

`sby.rs` ve `sva.rs` (`ONE_BIT` dışında) kodlama seçmez. Sürücü VCD
okumaz, yalnız kopyalar.

### b.I MMIO ve yazılım tarafı

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-syntax/src/parser/mmio.rs:525` `field_ty` (`:44` `WORD_BITS`) | SYN | Yalnız literal N, 1..=32; takma ad, enum, Trit yok (ADR-0044). | dil | dağınık |
| `volt-syntax/src/parser/mmio.rs:150` `FieldInfo::mask` | SYN | `((1<<w)-1)<<lo`. | dil | dağınık |
| `volt-syntax/src/parser/mmio.rs:207` `regmap` | SYN, SW | `FieldDesc.width`. | dil | tek |
| `volt-ast/src/mmio.rs:131` `FieldDesc::mask` | ORTAK, SW | Aynı maske. | dil | dağınık |
| `volt-sw-emit/src/check/mod.rs:182` `field_mask` | SW | Kaydırmasız maske. | dil | dağınık |
| `volt-sw-emit/src/rust.rs:181` `extract`; `:201` `place` | SW | Maske satır içinde; bool `!= 0`. | dil | dağınık |
| `volt-sw-emit/src/c.rs:48` `register` | SW | `#define _MASK` `(1<<w)-1`. | dil | dağınık |
| `volt-sw-emit/src/names.rs:9` `rust_type`; `:19` `c_type` | SW | En dar u8/u16/u32 ya da bool. | arka uç | tek |

## c. Zaman modeli

### c.1 Sözcük ve gramer

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-syntax/src/token.rs:18` `TokenKind` (tip) | SYN | `on`, `clock`, `reset`, `posedge`/`negedge`/`none`: zaman sözlüğü kenardan oluşur. | dil | tek |
| `volt-syntax/src/lexer.rs:69` `check_token` | SYN | `Spike` ve `spike` rezerve, E0003. | dil | tek |
| `volt-syntax/src/parser/stmt.rs:531` `parse_on` | SYN | Tetik yalnız `Ident` ya da `Ident.reset`; `on spike(x)` "saat adı" hatasına düşer. | dil | tek |
| `volt-syntax/src/parser/stmt.rs:143` `parse_reg` | SYN | `reg(clk)`: açık alan bir saat adıdır. | dil | tek |
| `volt-syntax/src/parser/stmt.rs:830` `parse_block_assign` | SYN | Sıralı bağlamda `<=`, kombinasyonelde `=` (E0006, E0007). | dil | tek |
| `volt-syntax/src/parser/item.rs:1561` `parse_domain`; `:1600` `parse_domain_field`; `:1712` `parse_domain_value` | SYN | clock, frequency, reset, trust_level; `posedge`/`negedge`/`none`. | dil | tek |
| `volt-syntax/src/parser/item.rs:1670` `unsupported_domain_key` | SYN | `reset_cycles`, `reset_sequence` ayrılmış (E0003). | dil | tek |
| `volt-syntax/src/parser/test.rs:186` `parse_test_reset`; `:237` `parse_test_call` | SYN | Test dilinin `reset()` ve `step(n)`'i. | dil | tek |

### c.2 Ortak AST modeli

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/lib.rs:775` `ClockEdge`; `:782` `ResetSpec`; `:788` `ResetSync` (tip) | ORTAK | `ClockEdge::None` kenarsız alan. | dil | tek |
| `volt-ast/src/lib.rs:708` `DomainDecl`; `:721` `DomainKey` (tip) | ORTAK | `Frequency`, `ResetCycles`, `ResetSequence`. | dil | tek |
| `volt-ast/src/lib.rs:990` `RegDecl`; `:1038` `OnBlock`; `:1044` `OnTrigger` (tip) | ORTAK | `OnTrigger` = `Clock` \| `Reset` \| `Error`; spec'in `Spike` tetiği kodda yok. | dil | tek |
| `volt-ast/src/lib.rs:1104` `BlockContext`; `:1114` `BlockStmt` (tip) | ORTAK | `Sequential` ⇒ `<=`. | dil | tek |
| `volt-ast/src/lib.rs:577` `TestStmt` (tip) | ORTAK | `step` ve `reset` çağrıları. | dil | tek |
| `volt-ast/src/alias.rs:37` `signal_class` | ORTAK | Tip denetimi olmadan saat/reset sınıfı (ADR-0137 izin listesi). | dil | dağınık |
| `volt-ast/src/builtin.rs:69` `DomainRole` ve port tabloları (`:120`–`:227`) | ORTAK | `wr_clk`/`rd_clk`, `src_clk`/`dst_clk`, `clk`. | dil | tek |
| `volt-ast/src/builtin.rs:294` `clock_resets_flops`; `:309` `comb_inputs`; `:423` `has_dst_clock` | ORTAK | Saat bağlaması flop sürer mi; primitif çıkışı register mı kombinasyonel mi. | dil | tek |
| `volt-ast/src/reset_chain.rs:22` `chain_consumed`; `:86` `syncs_to` | ORTAK | Reset zinciri (SV, SDC). | arka uç | tek |

### c.3 Tip ve saat/reset sınıfı

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/ty.rs:86` `Ty` (`Clock`, `Reset`) | TD | | dil | tek |
| `volt-hir/src/port_class.rs:40` `class`; `:54` `is_clock`; `:66` `is_clock_port`; `:72` `is_raw_reset` | TD, SDC, LSP | Anlamsal yol (ADR-0137); SV okumaz. | dil | dağınık |
| `volt-hir/src/typeck/reset_binding.rs:94` `reject_reset_as_data`; `:143` `check_sync_source`; `:174` `check_reset_binding` | TD | Reset veri değil; `sync(reset)` E2003. | dil | tek |
| `volt-hir/src/resolve/prelude.rs:8` `init_prelude` | TD | `sync`, `sync3`, `prev`. | dil | tek |

### c.4 Alan bildirimi ve kenarsız alan

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/domain/mod.rs:72` `DomainId`; `:85` `ClockSpec`; `:91` `DomainInfo` (tip) | TD | `Explicit` \| `Timeless` \| `Unresolved` \| `Error`. | dil | tek |
| `volt-hir/src/domain/tables.rs:16` `collect_domain_decls` | TD | Okuyucu 1: kenarda son alan kazanır; reset varsayılanı `None` (#422). **Kapandı (#420, PR #433):** kenarı ve reset'i ORTAK `volt_ast::domain_fields` verir, ikinci yazım E1003 (ADR-0140); reset varsayılanının ayrışması açık (#422). | dil | dağınık |
| `volt-hir/src/domain/tables.rs:59` `implicit_clock_domain` | TD | Açıklamasız saat = örtük posedge alan. | dil | dağınık |
| `volt-hir/src/domain/edgeless.rs:23` `EdgeUse` (tip); `:50` `check_edgeless_use`; `:74` `edgeless_domain` | TD | E3016; kenar isteyen altı kullanım. | dil | dağınık |
| `volt-hir/src/domain/instance.rs:86` `check_child_clock_edges`; `:111` `child_port_has_edge` | TD | Çocuk saat portu kenarlı alanda. | dil | tek |
| `volt-hir/src/domain/rdc/facts.rs:19` `DEFAULT_RESET` (sabit); `:98` `clock_facts`; `:143` `declared_reset` | TD | Okuyucu 2 (yalnız reset); varsayılan `sync active_high`. **Kapandı (#420, PR #433):** `declared_reset` alanı ORTAK `domain_fields`'tan alır; varsayılanın ayrışması açık (#422). | dil | dağınık |
| `volt-sv-emit/src/lib.rs:828` `collect_domains` | SV | Okuyucu 3: son alan kazanır. **Kapandı (#420, PR #433):** kenar ve reset ORTAK `domain_fields`'tan; "son alan" kodda kalmadı. | arka uç | dağınık |
| `volt-sv-emit/src/lib.rs:111` `ResetCfg::DEFAULT`; `:165` `DomainInfo::SPEC_DEFAULT` (sabit) | SV | posedge + `sync active_high`; kural sv-mapping.md'de, dil kuralı mı SV eşlemesi mi spec ayırmaz. | belirsiz | dağınık |
| `volt-hir/src/constraints/walk.rs:348` `collect_domains` | SDC | Okuyucu 4: `has_reset = !any(reset = none)` (#420). **Kapandı (#420, PR #433):** `has_reset` ORTAK `domain_fields`'tan; `any` kodda kalmadı. | arka uç | dağınık |
| `volt-hir/src/constraints/walk.rs:472` `apply_edges` | SDC | Okuyucu 5: `clock =` için ilk alan kazanır (`find_map`); TD ve SV'de son. **Kapandı (#420, PR #433):** kenar ORTAK `domain_fields`'tan; alan bir kez yazıldığından ilk/son ayrışması kalmadı. | arka uç | dağınık |

### c.5 Register, `on`, `<=`, reset değeri

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-syntax/src/parser/register_assign.rs:36` `clock_of`; `:55` `reg_clocks`; `:74` `check_register_assigns` | SYN | E0019. | dil | tek |
| `volt-hir/src/typeck/output_register.rs:66` `check_output_nonblocking` | TD | E0020; aynası SV `flop_audit` `audit_reset_targets`. | dil | dağınık |
| `volt-hir/src/domain/reg.rs:17` `assign_reg_domains`; `:53` `scan_writers`; `:70` `reg_domain_from_writers` | TD | Register'ın alanı yazan `on` bloğunun saati; E3011. | dil | dağınık |
| `volt-hir/src/domain/reg.rs:195` `check_on_trigger`; `:236` `on_block_domain` | TD | Tetik saat portu (E3002). | dil | dağınık |
| `volt-hir/src/domain/walk.rs:11` `walk_stmt` | TD | `On` → alan; `Comb` ayrı. | dil | tek |
| `volt-hir/src/domain/assign.rs:17` `check_foreign_read`; `:88` `check_assign` | TD | E3001, E3012. | dil | tek |
| `volt-sv-emit/src/lib.rs:224` `domain_of_trigger` | SV | `on` tetiğinden alan. | arka uç | dağınık |
| `volt-sv-emit/src/lib.rs:1944` `emit_on_block` | SV | `always_ff @(edge clk [or edge rst])`; `on clk.reset` E0003. | arka uç | dağınık |
| `volt-sv-emit/src/lib.rs:2011` `reset_assignments`; `:2633` `collect_written` | SV | Reset dalı = yazılan reg'lerin `init` değeri; bu dil anlamı yalnız burada. | dil | tek |
| `volt-sv-emit/src/lib.rs:206` `reset_port_set`; `:1335` `emit_ports` | SV | Kaynakta yazılmayan `rst`/`rst_n` portu eklenir; arayüzü değiştiren bir kural, yalnız SV'de. | belirsiz | tek |
| `volt-sv-emit/src/lib.rs:141` `async_sensitivity`; `:173` `flop_edge` | SV | `None` kenar `unreachable!`. | arka uç | tek |
| `volt-sv-emit/src/flop_audit.rs:98` `audit_module`; `:232` `audit_edge`; `:253` `audit_reset_targets` | SV | TD kurallarının üretimden önceki aynası (E3016, E0020, E3002). | arka uç | dağınık |
| `volt-hir/src/summary.rs:329` `CombGraph::build` | TD | `on`, `reg`, `sync` kombinasyonel yolu keser. | dil | tek |

### c.6 Alan çıkarımı ve CDC

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/domain/module.rs:12` `infer_module` | TD | Kontrat saati = ilk saat portu (satır 43–50). | dil | dağınık |
| `volt-hir/src/domain/module.rs:55` `scan_clock_ports`; `:179` `assign_port_domains` | TD | `@clk` zinciri izlenir. | dil | dağınık |
| `volt-sv-emit/src/flop_audit.rs:48` `port_domain`; `:74` `is_clock_type` | SV | `scan_clock_ports`'un ada göre kopyası. | arka uç | dağınık |
| `volt-sv-emit/src/lib.rs:789` `clock_ports_of` | SV | Kendi saat testi. | arka uç | dağınık |
| `volt-hir/src/domain/join.rs:48` `join` | TD | `Timeless` birim eleman; iki alan E3001. | dil | tek |
| `volt-hir/src/domain/instance.rs:150` `build_domain_mapping`; `:304` `clockless_outputs` | TD | E3014; saatsiz hedefin çıkışı. | dil | tek |
| `volt-hir/src/domain/extern_decl.rs:16` `check_extern_decl` | TD | Çok saatli extern açıklama ister. | dil | tek |
| `volt-hir/src/domain/bundle.rs:12` `check_bundle_domains`; `bidir.rs:15` `bidir_read` | TD | E3013; W3007. | dil | tek |
| `volt-hir/src/summary.rs:235` `extern_summary`; `:272` `Clocks::is_clock_port` | TD | Saatli extern opak, saatsiz extern en kötü durum. | dil | tek |
| `volt-hir/src/handshake.rs:36` `check_handshakes` | TD | E4007. | dil | tek |

### c.7 `sync()` ve yerleşik CDC primitifleri

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/domain/sync.rs:55` `check_sync_capture` | TD | Kaynak yakalama kuralı, kopya 1. | dil | dağınık |
| `volt-hir/src/typeck/synth.rs:209` `check_sync_arity` | TD | `sync(src, dst_clk)`. | dil | tek |
| `volt-sv-emit/src/lib.rs:1790` `try_emit_sync_bridge` | SV | `sync` 2, `sync3` 3 aşama (satır 1801); yakalama kopya 2. | arka uç | dağınık |
| `volt-sv-emit/src/lib.rs:296` `sync_always_ff` | SV | Senkronizörün `always_ff` metni. | arka uç | dağınık |
| `volt-sv-emit/src/flop_audit.rs:176` `audit_sync`; `:208` `audit_builtin` | SV | Yakalama kopya 3. | arka uç | dağınık |
| `volt-hir/src/constraints/walk.rs:958` `sync_bridge` | SDC | Aşama 2/3 (satır 970); yakalama `driver_clock` ile (#421). | arka uç | dağınık |
| `volt-hir/src/domain/builtin_instance.rs:25` `check_builtin_instance`; `:77` `builtin_clock_domains` | TD | `DomainRole`. | dil | tek |
| `volt-sv-emit/src/builtin_prim.rs:320` `emit_async_fifo` … `:931` `emit_edge_detect` | SV | Primitiflerin çevrim davranışı yalnız SV şablonunda; TD portların alanını bilir. | belirsiz | tek |
| `volt-sv-emit/src/builtin_prim.rs:1575` `edge_of`; `:1582` `builtin_always_ff_no_reset`; `:1596` `builtin_always_ff` | SV | `always_ff` metni. | arka uç | dağınık |
| `volt-hir/src/constraints/walk.rs:1030` `builtin_bridge` | SDC | Port adları `DomainRole` yerine sabit. | arka uç | dağınık |
| `volt-sv-emit/src/sv_collisions.rs:181` `reset_origins`; `:218` `sync_origins` | SV | Üretilen senkronizör adlarının kökeni. | arka uç | dağınık |

### c.8 Reset senkronizörü ve RDC

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/domain/rdc/mod.rs:39` `check_rdc`; `:125` `is_raw_reset_port` | TD | | dil | tek |
| `volt-hir/src/domain/rdc/facts.rs:195` `reset_free_bindings`; `:271` `feeds_of` | TD | Reset örneklemeyen saat kullanımı; ham reset → saat. | dil | dağınık |
| `volt-hir/src/domain/rdc/converge.rs:43` `release_clocks`; `rdc/share.rs:17`, `:51` | TD | E3003, W3009, W3010. | dil | tek |
| `volt-sv-emit/src/reset_sync.rs:21` `RESET_SYNC_STAGES` (sabit, 2); `:32` `stage_name` | SV, SIM | ADR-0065'te; dil yüzeyinde yazılamaz. | belirsiz | dağınık |
| `volt-sv-emit/src/reset_sync.rs:37` `is_raw_reset`; `:47` `feeding_raw`; `:87` `reset_free_clock`; `:166` `synchronizer_block` | SV | `port_class` ve RDC kurallarının kopyaları. | arka uç | dağınık |
| `volt-sv-emit/src/lib.rs:1041` `chain_consumed`; `instance.rs:404` `parent_synced_reset` | SV | ORTAK `reset_chain`. | arka uç | tek |
| `volt-hir/src/constraints/walk.rs:1180` `reset_chains`; `:1214` `has_reset`; `:1225` `feeding_raw` | SDC | `feeding_raw` kopya 3. | arka uç | dağınık |
| `volt-hir/src/constraints/mod.rs:133` `RESET_SYNC_STAGES` (sabit, 2) | SDC | SV sabitinin kopyası. | arka uç | dağınık |

### c.9 `prev`, kontratlar, formal

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/resolve/expr.rs:253` `check_prev_call` | TD | `prev(x, N)`: N ≥ 1 çevrim (E5017). | dil | tek |
| `volt-sv-emit/src/past.rs:39` `emit_prev`; `:63` `prev_depth` | SV | `$past(x, N)`. | arka uç | tek |
| `volt-sv-emit/src/past.rs:73` `past_reg_block` | SV, SIM | Reset sonrası ilk çevrimde `prev(x) == 0`; öteki kiplerde `$past`. İki biçimin ilk çevrimde aynı olduğu doğrulanmadı. | belirsiz | tek |
| `volt-sv-emit/src/sva.rs:148` `sva_properties`; `:225` `sva_immediate`; `:358` `sva_file` | SV | `@(edge clk) disable iff (rst)`; `clocks.first()`. | arka uç | dağınık |
| `volt-sv-emit/src/sva.rs:306` `unclocked_contracts` | SV | Saatsiz modülün kontratı (E5005). | dil | tek |
| `volt-sv-emit/src/sim_contract.rs:112` `sva_simulation`; `:189` `sim_monitor` | SV, SIM | İlk saat. | arka uç | dağınık |
| `volt-sv-emit/src/sby.rs:102` `sby_config`; `:159` `sby_config_tasks`; `lib.rs:474` `emit_unit` | SV | `depth N` çevrim; `multiclock on` yalnız ≥ 2 saatte. | arka uç | tek |
| `volt-driver/src/verify.rs:495` `unclocked_diagnostics` | DRV | Metin "ilk saat kenarı" kuralını yineler. | dil | dağınık |
| `volt-driver/src/verify.rs:621` `sby_tasks`; `:988` `counterexample_diagnostic`; `:1102` `unproven_diagnostic`; `verify_depth.rs:58` `auto_unreached_at` | DRV | "violated at cycle N"; `--depth` çevrim (varsayılan 20). | arka uç | tek |
| `volt-ast/src/lib.rs:835` `AutoReach` (tip); `volt-syntax/src/parser/auto_contract/counter.rs:111` `min_steps_to_bound`; `fsm.rs:168` `distances`; `:190` `transition_reach` | SYN, ORTAK | Erişilebilirlik = resetten en az kenar sayısı. | dil | tek |
| `volt-syntax/src/parser/auto_contract/scan.rs:141` `scan_module` | SYN | Yalnız ilk saatin `on` blokları. | dil | dağınık |
| `volt-syntax/src/parser/handshake.rs:337` `handshake_contracts`; `:418` `prev` | SYN | Kural çevrim başına. | dil | tek |
| `volt-syntax/src/parser/mmio.rs:829` `module_clock`; `:855` `render`; `:1488` `render_contracts`; `:1621` `render_logic` | SYN | İlk saat portu (yoksa `clk` eklenir). | dil | dağınık |

### c.10 L1 zamanlama ve pipeline

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-ast/src/lib.rs:199` `TimingInfo` (tip) | ORTAK | | dil | tek |
| `volt-syntax/src/parser/expr.rs:889` `parse_delay_expr` | SYN | `delay<K>(x)`. | dil | tek |
| `volt-hir/src/ty.rs:124` `Ty::Delayed` | TD | Varyant tanımlı, hiçbir yerde kurulmaz. | dil | tek |
| `volt-hir/src/timing.rs:152` `ModuleTiming::run`; `:362` `compute_def`; `:496` `delay_of` | TD | Register = +1 çevrim; hangi saatin register'ı olduğuna bakılmaz. | dil | tek |
| `volt-hir/src/timing.rs:318` `declared_of`; `:430` `check_declared` | TD | E5010. | dil | tek |
| `volt-ast/src/lib.rs:334` `PipelineDecl` (tip); `volt-syntax/src/parser/pipeline.rs:43` `parse_pipeline` | SYN, ORTAK | `pipeline(N)`. | dil | tek |
| `volt-syntax/src/parser/desugar.rs:88` `desugar_pipeline`; `:1015` `single_clock`; `:1035` `build_on_block` | SYN | Aşama = 1 çevrim; tek saat (E5011). | dil | tek |

### c.11 Kısıtlar (SDC/XDC)

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/constraints/mod.rs:62` `ClockConstraint` (tip); `:80` `period_ps`; `:189` `async_groups`; `:242` `missing_frequency` | SDC | Saat = periyot; W0022. | arka uç | tek |
| `volt-hir/src/constraints/parse.rs:90` `parse_frequency` | SDC | `frequency = 100.mhz` yalnız burada okunur; kaynakta yazılır, anlamı yalnız STA'da. | belirsiz | tek |
| `volt-hir/src/constraints/parse.rs:188` `parse_timing`; `:356` `parse_false_path`; `:381` `parse_multicycle` | SDC | `@multicycle(cycles = N)`; gerekçe yukarıdaki gibi. | belirsiz | tek |
| `volt-hir/src/constraints/walk.rs:81` `Scope::new` | SDC | Register → ilk `on` saati. | arka uç | dağınık |
| `volt-hir/src/constraints/walk.rs:150` `clock_of_domain`; `:159` `driver_clock`; `:163` `driver_clock_depth` | SDC | Ayrı sürücü saati izleyicisi (#421). | arka uç | dağınık |
| `volt-hir/src/constraints/walk.rs:392` `collect_module` | SDC | Saat tipli her port bir `ClockConstraint`; kenar süzgeci yok. | arka uç | dağınık |
| `volt-sdc-emit/src/render.rs:56` `create_clock`; `:79` `groups_section`; `:152` `async_reg`; `:166` `crossing_rule`; `:208` `resets_section`; `:249` `path_rule` | SDC | `ASYNC_REG`, `set_max_delay`, `set_multicycle_path`. | arka uç | tek |

### c.12 Simülasyon ve test dili

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-hir/src/sim.rs:24` `TEST_BUILTINS` (sabit); `:217` `check_call`; `:485` `bad_step_arg` | SIM | `step(n ≥ 1)` çevrim sayısı. | dil | tek |
| `volt-hir/src/sim.rs:273` `check_set_port` | SIM | Saat portu yazılamaz: "saati `step()` sürer" (E8503). | dil | tek |
| `volt-driver/src/sim_lower.rs:356` `call` | SIM | `step` → `TbStep::Step`, `reset` → `TbStep::Reset`. | dil | tek |
| `volt-sv-emit/src/sim.rs:156` `TbStep` (tip) | SIM | | dil | tek |
| `volt-sv-emit/src/sim.rs:282` `cycle_fn` | SIM | Bütün saat portları birlikte 1 → eval → 0 → eval; alan frekansları ve fazları yok sayılır. | arka uç | tek |
| `volt-sv-emit/src/sim.rs:313` `reset_fn` | SIM | `reset()` = 2 çevrim + `RESET_SYNC_STAGES`. | arka uç | tek |
| `volt-sv-emit/src/sim.rs:407` `run_testbench_cpp_with`; `volt-driver/src/sim/run_cmd.rs:43` `run_inner` | SIM, DRV | `volt run --cycles N`. | arka uç | tek |
| `volt-sv-emit/src/sim_script.rs:299` `cycle_body`; `:325` `emit_step` | SIM | `step` → `run_cycle`. | arka uç | tek |
| `volt-driver/src/sim/contracts.rs:104` `parse_contract_fail` | SIM | İhlal raporunda `cycle=N`. | arka uç | tek |

### c.13 LSP ve araçlar

| Yer | Kullanan | Not | Sınıf | Bilgi |
|---|---|---|---|---|
| `volt-lsp/src/analysis.rs:340` `clock_ports` | LSP | `port_class`, yarım kodda `signal_class`. | dil | dağınık |
| `volt-lsp/src/completion.rs:36` `context_at`; `:127` `completions` | LSP | `on ` yazılınca saat portları önerilir. | dil | tek |
| `volt-lsp/src/hover.rs:307` `domain_line`; `inlay.rs:84` `inlay_hints`; `:228` `domain_label`; `:237` `distinct_clock_domains`; `docs.rs:9` `keyword_doc` | LSP | "reg — Updated on clock edge"; Timeless "constant / combinational". | dil | tek |
| `volt-difftest/src/harness.rs:463` `build`; `:624` `testbench`; `run.rs:34` `verilator_args` | DIFF | `in clk : clock`, `#1 clk = 1'b1`, `--timing`. | arka uç | tek |
| `volt-sw-emit/src/c.rs:127` `field_fns`; `rust.rs:245` `field_methods` | SW | `@self_clearing`: "bir çevrim yüksek". | dil | tek |

### Kodda bugün var olan zamansız kavramlar

- `DomainId::Timeless` (`domain/mod.rs:72`): sabitler ve saf
  kombinasyonel değerler; `join`'de birim eleman.
- Kombinasyonel bağlam: `comb {}`, sürekli atama, `let`; SV'de
  `always_comb` ve `assign`.
- `fn` saf kombinasyoneldir: `reg`, `on`, `comb` (E2016) ve `sync`
  yasak.
- `clock = none` (`ClockEdge::None`): kimliği olan, kenarı olmayan alan;
  CDC denetimi çalışır, flop E3016; saat portu olmadan ondan `sync()`
  geçerlidir. Bugün bir "kenarsız zaman tabanı"na en yakın kavram.
- Ham `reset(async, …)` portu: dildeki tek asenkron olay
  (`or posedge rst`).
- Saatsiz extern (en kötü durum kombinasyonel özet), primitiflerin
  kombinasyonel çıkışları (`comb_inputs`), alansız giriş portundan
  `sync()` (SDC'de false path).
- Gramerde olup kodda yarım kalan iki tetik: `on clk.reset` (tip
  denetiminden geçer, SV'de E0003) ve `on spike(x)` (yalnız sözcük
  rezervasyonu).

## Dağınık bilgiler

Aynı ikili ya da saatli bilginin birden çok yerde hesaplandığı yerler.

### Değer kümesi

1. **(İşaret, N) tipinin aralığı 2^N, 2^(N−1)**, yaklaşık 20 yer: TD 9
   (`width.rs` `uint_fits`/`sint_fits`, `cast.rs` `warn_literal_cut`,
   `literal_fold.rs` `literal_range`, `compare_width.rs` `fits`,
   `arith_cut.rs` `fits` ve `type_bounds`, `matching.rs` `int_fits`,
   `diag.rs` `literal_overflow`, `enums.rs` `report_layout_issue`); SYN 2
   (`ParamRange::max`, `max_value`); SV 3 (`fmt_int`,
   `part_select_from_value`, `builtin_contracts`); SIM 5 (`PortWidth`,
   C++ `PORT_PRELUDE` ve `LOAD_PRELUDE`, `check_load`, `LeafLabel`); DIFF
   1. Ortak yardımcı yok. (`tek-yerde-bilgi-envanteri.md` TD'de 7 sayar;
   `diag.rs`, `enums.rs` ve `type_bounds` onda yok.)
2. **Sabit değerlendirici**, derleyicide 8: ORTAK `const_eval`, TD
   `ConstEvaluator`, TD `literal_fold`, SV `literal_value`, SYN
   `eval_const_scoped`, SYN `fold_binary`, SIM `TestConsts::eval` ve
   `fold_binary`; ayrıca C++ `cpp_binary` ve DIFF `lit_arith`. Bit
   anlamları ayrışır: `~` ORTAK'ta yok, TD'de i128 `!a`; negatif sayıda
   `&` ORTAK `None`, TD sonsuz ikiye tümleyen; bit kaybeden `<<` ORTAK
   `None`, TD ve SYN kaybı görmez; `as` SV keser, TD kesmez; test dili
   u64'te sarar.
3. **clog2**, derleyicide 10 uygulama: `consteval.rs:834`,
   `enum_layout.rs:164`/`:169`, `instance.rs:129`, `expr.rs:438`,
   `expr.rs:1363`, `expr.rs:1425`, `const_array.rs:158`,
   `builtin_prim.rs:49`, `builtin_prim.rs` `:965` ve `:1492` içinde iki
   satır içi; DIFF `floor_log2`. `trailing_zeros` sürümleri yalnız iki
   kuvvetinde doğrudur.
4. **`uN`/`iN` adının çözülmesi**, 6 yer: `token.rs:118`,
   `item.rs:1539`, `prelude.rs:36`, `type_ref.rs:117`, SV `lib.rs:974`,
   `inline/types.rs:71`.
5. **64 bit tavanı**, 7 yer: `item.rs:1539`, `MAX_INT_WIDTH`,
   `wide_int_type`, `SCRIPT_VALUE_BITS`, `MAX_SCALAR_BITS`, `test.rs`
   `parse_int_text`, `testdata.rs`.
6. **Sonek → (işaret, genişlik)**, 5 yer: `suffix_ty`, `suffix_sig`,
   `expr.rs` `SUFFIXES`, `test.rs`'in listesi, `print.rs` `int_text`.
7. **Literal metninin ayrıştırılması**, 2 yer: `expr.rs:1023` (u128),
   `test.rs:307` (u64).
8. **SV boyutlu literal yazımı:** genel yardımcı `fmt_int`; elle yazanlar
   `zero_of`, `builtin_data::literal`, `enum_localparams`, `end_process`,
   `builtin_prim`'deki yaklaşık 59 yer, LSP `enum_hover`; geri okuyan SW
   `sv_literal`.
9. **Genişleme kuralı (+1 bit, ×2)**, 4 yer: TD `arith_result`, TD
   `arith_width`, SV `binary_range`, DIFF `int_arith`.
10. **Literal aritmetiği**, 4 yer: TD `literal_op`, SV `literal_op`, SYN
    `fold_binary`, DIFF `lit_arith`.
11. **İkiye tümleyenle gösterim**, 3 yer: `describe_value`,
    `LeafLabel::render`, DIFF `to_i128`.
12. **Porta sığma kuralı**, 3 yer: `PortWidth::accepts`, C++
    `volt_port_fits`, `LOAD_PRELUDE`.

### Kodlama

13. **Bool, Clock, Reset 1 bit:** kaynak `Scalar::bits`; ayrıca 15 yerde
    sabit: ORTAK `struct_layout.rs:303`; SV `Sig::BIT` (`emit_module`,
    `port_sig`), `expr.rs:291`, `expr.rs:438`, `lib.rs:2303`,
    `sva.rs:140`, `instance.rs:147`, `builtin_prim.rs:83`,
    `let_width.rs:254`, `sim.rs:63` (otomatik `rst`); TD `timing.rs:736`,
    `cast.rs:83` (`Bool ↔ u1`); `mmio.rs:106`; DIFF `ast.rs:14`.
14. **Trit 2 bit:** kaynak `Scalar::bits`; ayrıca 6 yer: `waves.rs:172`,
    `:254`, `:275` (`width: 2`), `trit.rs:17`–`:18` (`2'sb`),
    `cast.rs:83` (`width >= 2`), `trit.rs:102` (2 + 1 = 3).
15. **Trit işaretli:** ortak yer yok; 6 yer: `struct_layout.rs:224`
    (`signed: true`), `sig_of_typeref`, `cast.rs:83`, `binop.rs:187`
    (yalnız iN ile çarpılır), `trit.rs:102`, `waves.rs:164` (`10` = −2).
16. **Trit kodları `01` ve `11`:** 2 yer, `trit.rs:17`–`:18` ve
    `waves.rs:172`.
17. **Trit ± Trit = i3:** 2 yer, `binop.rs:187` ve `trit.rs:102`.
18. **Bir ifadenin Trit olduğu:** iki motor, TD `Ty::Trit` ve SV'nin AST
    çıkarımı (`trit.rs:22`, `:50`, `trits` kümesi).
19. **Enum varyant kodu:** 2 yer, `enum_layout::layout` ve
    `consteval.rs:628`.
20. **Struct paketleme sırası:** 2 yer, `struct_layout.rs:173` (lsb) ve
    `volt-driver/src/sim_struct.rs:122`/`:136` (kaydır ve OR).
21. **Paketli dizide eleman 0 LSB'de:** 2 yer, `lib.rs:2681` ve
    `structs/value.rs:352`.
22. **Dizi genişliği = eleman × uzunluk:** 6 yer, `ty.rs:310`,
    `bit_width.rs:70` (`bits_at`), `struct_layout.rs:224`,
    `lib.rs:2416`, `:2442`, `:2615`.
23. **Taps = LEN × genişlik(T):** 2 yer, `instance.rs:168` ve
    `expr.rs:438`.
24. **Adres genişliği = clog2(DEPTH):** 4 yer, `instance.rs:129`,
    `expr.rs:438`, `builtin_prim.rs:49`, `:320` (madde 3'ün parçası).
25. **MMIO alan maskesi `(2^w − 1) << lsb`:** 5 yer, `mmio.rs:150`,
    `volt-ast/src/mmio.rs:131`, `check/mod.rs:182`, `rust.rs:181`,
    `c.rs:48`.
26. **64 bitlik test değeri sınırı:** 4 sabit (b.G).
27. **bool → 0/1 kodu:** ORTAK `const_eval.rs:164`, `:293`;
    `sim_const.rs:144`, `:209`; `sim_lower.rs:127`, `:166`; SV
    `expr.rs:802`.
28. **GTKWave `name[W-1:0]` adı:** 2 yer, `waves.rs:125` ve
    `volt-driver/src/waves.rs:129`.

### Zaman modeli

29. **Saat/reset sınıfı:** üç yol, `port_class` (TD, SDC, RDC, özet,
    L1, LSP), `signal_class` (SYN, test dili, LSP yedeği) ve SV'nin kendi
    `alias::resolve` + `TypeRefKind::Clock/Reset` testi (`lib.rs:789`,
    `:1335`, `flop_audit.rs:74`, `sim.rs:63`, `reset_sync.rs:37`).
30. **`domain` bildirimini okuyan 5 yer:** `tables.rs:16`,
    `facts.rs:143`, SV `lib.rs:828`, SDC `walk.rs:348` ve `:472`. Kenarda
    TD ve SV son alanı, SDC `apply_edges` ilk alanı alır; reset
    varsayılanı TD'de `None`, RDC ve SV'de `sync active_high`, SDC'de
    `any(reset = none)` (#420, #422). Yinelenen anahtar için tanı yok.
    **Beş okuyucu kapandı (#420, PR #433):** hepsi ORTAK
    `volt_ast::domain_fields`'ı okur, ikinci yazım E1003'tür (ADR-0140).
    Reset varsayılanının ayrışması açıktır (#422).
31. **Örtük alan varsayılanı (posedge + reset)**, 4 yer: `tables.rs:59`,
    SV `lib.rs:165`, RDC `facts.rs:19`, SDC `walk.rs` (`has_reset`
    varsayılanı).
32. **Saat portunun alanı (`@clk` zinciri)**, 3 yer: TD `module.rs:55`,
    SV `flop_audit.rs:48`, SDC `walk.rs:150` (zinciri izlemez, #421).
33. **Register → saat**, 2 yer: TD `reg.rs:53`/`:70` (E3011), SDC
    `walk.rs:81` (ilk `on` bloğu kazanır).
34. **`on` tetiği bir saat portu mu**, 2 yer: TD `reg.rs:195`, SV
    `flop_audit.rs:98`.
35. **Kenar isteyen kullanımlar (E3016)**, 2 yer: TD `edgeless.rs`, SV
    `flop_audit.rs`.
36. **Kontrat saati = ilk saat portu**, 6 hesap: TD `module.rs:12`; SV
    `sva.rs:148`, `:225`, `:358`, `sim_contract.rs:112`,
    `flop_audit.rs:98`; akraba kurallar `scan.rs:141`, `mmio.rs:829`,
    `verify.rs:495`'in metni.
37. **`sync()` kaynak yakalama kuralı**, 4 yer: TD `sync.rs:55`, SV
    `lib.rs:1790`, SV `flop_audit.rs:176`, SDC `walk.rs:958`.
38. **`sync`/`sync3` aşama sayısı**, 2 yer: SV `lib.rs:1801`, SDC
    `walk.rs:970`.
39. **Reset senkronizörü aşama sayısı 2**, 2 sabit: `reset_sync.rs:21`,
    `constraints/mod.rs:133`.
40. **Ham reset → saat (`feeding_raw`)**, 3 yer: RDC `facts.rs:271`, SV
    `reset_sync.rs:47`, SDC `walk.rs:1225`.
41. **Reset örneklemeyen saat kullanımı**, 2 yer: RDC `facts.rs:195`, SV
    `reset_sync.rs:87`; ortak tablo `builtin.rs:294`.
42. **`always_ff @(edge …)` metni**, 8 yer (yalnız SV): `lib.rs:296`,
    `:1944`, `builtin_prim.rs:1582`, `:1596`, `reset_sync.rs:166`,
    `past.rs:73`, `sim_contract.rs:189`, `sva.rs:225`.
43. **Primitif saat port adları**, 2 yer: `builtin.rs` (`DomainRole`) ve
    SDC `walk.rs:1030`.
44. **Üretilen senkronizör adları**, 3 yer: SV `lib.rs` ve
    `reset_sync.rs:32`, SDC `walk.rs`, `sv_collisions.rs`.

## Trit'in temsilini bilen yerler

`Scalar::bits` ve `TypeArena::bit_width` dışında Trit'in kodlamasını
(2 bit, işaretli, `01`/`11` kodları, kullanılmayan `10`) bilen yerler.
Aşama 1'in izin listesi testinin başlangıç listesi budur.

| Yer | Bildiği |
|---|---|
| `volt-ast/src/struct_layout.rs:224` `leaf_or_nested` | işaretli (`signed: true`), `LeafKind::Trit` |
| `volt-hir/src/ty.rs:321` `TypeArena::is_signed` (Trit kolu; eklendi #413, PR #440) | işaretli |
| `volt-ast/src/struct_layout.rs:31` `LeafKind` (tip), `:96` `has_enum_or_trit` | `10` geçersiz kod |
| `volt-sv-emit/src/lib.rs:2513` `sig_of_typeref` | işaretli; yorumda `01`/`00`/`11` |
| `volt-sv-emit/src/trit.rs:17`–`:18` `TRIT_POS`, `TRIT_NEG` | 2 bit, işaretli, `01`, `11` |
| `volt-sv-emit/src/trit.rs:113` `try_emit_trit_mul` | `01`, `11`; `10` → 0 |
| `volt-sv-emit/src/trit.rs:102` `trit_sum_sig` | i2 + i2 = 3 bit işaretli |
| `volt-sv-emit/src/trit.rs:50` `is_trit` | jenerik `*` ikiye tümleyen i2'de doğru |
| `volt-sv-emit/src/waves.rs:172` `trit_table` | 2 bit; `00`, `01`, `11` |
| `volt-sv-emit/src/waves.rs:164` `invalid_label` | `10` işaretli okununca −2 |
| `volt-sv-emit/src/waves.rs:254` `wave_signal_of`, `:275` `instance_wave_outputs` | 2 bit |
| `volt-hir/src/typeck/cast.rs:83` `check_cast_legal` | 2 bit ve işaretli (`width >= 2`); sayısal → Trit yasağı |
| `volt-hir/src/typeck/structs.rs:316` `check_bits_to_struct`; `typeck/instance.rs:217` `check_memory_element` | geçersiz kod var (E2009) |
| `volt-hir/src/typeck/binop.rs:187` `trit_arith_result` | Trit yalnız işaretli tiple çarpılır |
| `volt-hir/src/sim_port.rs:110` `scalar_width` | test dilinde "sayı değil" (#430) |
| `volt-driver/src/sim_struct.rs:39` `LeafLabel::render` | struct içinde 2 bit ikiye tümleyen |
| `volt-diagnostics/src/explain/topics.rs:376`, `:439` | kullanıcı metni: "invalid −2" |

Ters durum: `volt-hir/src/timing.rs:736` `stage_register_help` Trit'in
genişliğini bilmez (`None`) ve öneride bedel yazmaz.

## Açık bulgular

Envanterin yan ürünüdür. Hiçbiri koşturulmadı: kod ve spec okunarak
bulundu. Sınıflar AGENTS.md'nin rapor sözlüğündendir.

| # | Bulgu | Sınıf | Dayanak |
|---|---|---|---|
| 1 | `~t` (t: Trit) Trit tiplidir: spec type-inference.md §3.4 `UnOp::BitNot => ot` der, `synth.rs:260` öyle uygular. SV kodlamasında `~2'sb01 = 2'sb10` (−2), ADR-0003'ün değer kümesi {−1, 0, +1} dışında. Bit işleminin değer kümesi yerine kodlama üzerinde tanımlandığı bir yer. Bunu yakalayan test bulunamadı. | karar gerektiren (spec ile ADR-0003 çelişiyor; sessiz yanlış olabilir) | spec §3.4; ADR-0003; ADR-0062 K1 |
| 2 | `past.rs:73` `past_reg_block` genişliği `width_of(x).map(..).unwrap_or(1)` ile alır (satır 100): #419'un kaldırdığı "bilinmiyorsa 1 bit" kalıbı; yardımcı register işaretsiz bildirilir. `tek-yerde-bilgi-envanteri.md` bu fonksiyonun `type_ref_bits` kullandığını yazar; bugün değil. **Kapandı (#413, PR #440):** koşturuldu, işaret ve doğal genişlik için sessiz yanlıştı (`tests/real_bugs/413_prev_width_and_sign.volt`); 1 bit yoluna düşen girdiler başka bir tanı da alıyordu. Genişlik ve işaret TD'den (ADR-0141); envanter satırı düzeltildi. | kapandı (#413); önce sessiz yanlış şüphesi | ADR-0137 §1; ADR-0141 |
| 3 | SDC `apply_edges` (`constraints/walk.rs:472`) yinelenen `clock =` alanında ilk alanı, TD ve SV son alanı alır; yinelenen anahtar için tanı yok. #420 ile aynı kök (yinelenen `domain` alanı). **Kapandı (#420, PR #433):** `apply_edges` kenarı ORTAK `domain_fields`'tan okur; yinelenen `clock =` E1003'tür. | kapandı (#420); önce sessiz yanlış şüphesi (#420 ailesi) | #420; ADR-0140 |
| 4 | Sabit değerlendiriciler bit kaybeden `<<`'de ayrışır: TD `consteval.rs:646` `checked_shl` yalnız miktarı denetler (`3 << 127` bit kaybeder), ORTAK `arith` `None` verir. | gizli ya da reddedilen (koşturulmadı) | `tek-yerde-bilgi-envanteri.md` ayrışma 2–3 ile aynı aile |
| 5 | SDC `collect_module` (`walk.rs:392`) saat tipli her portu `ClockConstraint` yapar, kenara bakmaz; `clock = none` alanının saati için `create_clock` yazılabilir. SIM `cycle_fn` kenarsız saati de çevirir. | belirsiz (koşturulmadı) | E3016'nın kapsamı |
| 6 | `timing.rs:736` `stage_register_help` `bit_width` yerine kendi tablosunu kullanır; Trit, enum, struct, dizi için öneride bedel yazılmaz. `struct_layout.rs:303` `Bool => Ok(1)` sabittir (aynı dosya 239'da `Scalar` okur). | iyileştirme | ADR-0137 §1 |
| 7 | Test dilinde Trit portu: `sim_port.rs:110` onu "sayı değil" sayar; struct içindeki Trit ise `LeafLabel::render` ile işaretli çözülür. İki yol tutarsız. | iyileştirme (#430'un kapsamı) | #430 |

## Sayıların yöntemi

Bu belgenin tablolarındaki satırlar sayıldı: her tablo satırı bir yer.
"dağınık satır" Bilgi sütunu `dağınık` olan satırlardır; "Dağınık bilgi"
yukarıdaki numaralı listedir (bir bilginin birden çok yerde yazılması).
