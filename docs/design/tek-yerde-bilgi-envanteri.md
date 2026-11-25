# Tek Yerde Bilgi — Envanter (#407 Aşama 0)

> Kaynak: ADR-0136 ("Arka uçlar anlamsal bilgiyi tek yerden okur"), Aşama 0.
> Tarih: 2026-11-24, `main` 2bff047d üzerinde. Satır numaraları o günkü
> koddandır; her satır fonksiyonun `fn` satırını gösterir.

Bu belge bir anlamsal bilginin birden çok yerde **hesaplandığı** (yalnız
taşındığı değil) her noktayı sayar: genişlik, işaret, sabit değer, takma ad
çözümü, saat alanı, struct sinyalleri. Her satır: yer, kullanan arka uç,
ortak yardımcıyı mı kullanıyor yoksa kendi hesabını mı yapıyor.

Kısaltmalar: **TD** tip denetimi ve HIR (`volt-hir`, başvuru kaynağı),
**SV** SV üretimi (`volt-sv-emit`), **SW** üretilen sürücüler
(`volt-sw-emit`), **SDC** kısıtlar (`volt-sdc-emit`; kısıt modeli
`volt-hir/src/constraints/`'te kurulur, SDC onu yalnız yazar), **SIM** test
dilinin simülasyonu (`volt-hir/src/sim*.rs`, `volt-driver/src/sim_*.rs`,
SV tarafındaki testbench), **LSP** dil sunucusu, **ORTAK** `volt-ast`'teki
ortak yardımcılar.

## Özet

| Bilgi | Yer | Hesaplayan (yer sayısı) | Ortak yardımcıyı kullanan | Kendi hesabı | Yardımcının kendisi |
|---|---|---|---|---|---|
| Genişlik | 87 | TD 34, SV 40, SW 2, SIM 7, ORTAK 4 | 18 | 65 | 4 |
| İşaret | 41 | TD 21, SV 15, SIM 4, ORTAK 1 | 1 | 39 | 1 |
| Sabit değer | 30 | TD 8, SV 13, SW 2, SIM 5, ORTAK 2 | 12 | 16 | 2 |
| Takma ad çözümü | 12 | ORTAK 4, SV 6, TD 1, SIM 1 | 10 | 1 (TD) | 1 |
| Saat alanı, saat/reset sınıflaması | ≈37 | TD ≈16 (SIM'in E8503 denetimi dahil), SV ≈10, SDC ≈9 (kısıt modeli), LSP 2 | ≈6 | ≈29 | 2 |
| Struct sinyalleri | 25 | ORTAK 3, SV 9, TD 5, SIM 5, SDC 1, LSP 2 | 18 | 4 | 3 |

Ayrıca takma adı hiç çözmeyen 7 yer vardır (bölüm 3).

Toplam: altı bilgi, yaklaşık 232 yer. Sayılar fonksiyon düzeyindedir (bir
fonksiyon birden çok bilgiyi hesaplıyorsa her bilgide sayılır).

Göze çarpanlar:

- **Genişlik ve işaret:** SV üreticisi tip denetiminin kurallarının çoğunu
  yeniden yazar: ifade genişliği (`width_of`), esnek aralık (`let_width.rs`
  `int_range`, `binary_range`: TD `binop.rs` `arith_of` ve `width.rs`'in
  kopyası), literal soneki (`suffix_sig` ↔ TD `suffix_ty`), `Trit ± Trit`
  (`trit_sum_sig` ↔ TD `trit_arith_result`), yerleşik primitif portları
  (`builtin_field_sig` ↔ TD `builtin_port_type`), literal aritmetiği
  (`fold_literal_arith` ↔ TD `literal_fold`). İşareti hesaplayan ortak
  yardımcı yok.
- **Sabit değer:** iki değerlendirici var: TD'nin `ConstEvaluator`'ı
  (`volt-hir/src/consteval.rs`) ve ortak `volt_ast::const_eval` (SV, SIM ve
  düzenler). TD ortak değerlendiriciyi hiç çağırmaz. Kapsamları farklı
  (aşağıda "Bilinen ayrışmalar").
- **Takma ad:** ortak `alias_chain_end` TD dışında herkesin; TD kendi
  (DefId ve kapsam tabanlı) çözümünü yapar. Asıl boşluk takma adı hiç
  çözmeyen yedi yerdir (aşağıda).
- **Saat alanı:** üç bağımsız motor: TD `domain/**` (başvuru), SV'nin kendi
  türetmesi (`flop_audit`, `clock_ports_of`, `reset_sync`) ve kısıt modeli
  (`constraints/walk.rs`). SV ve SDC TD'nin `DomainResult`'ını okumaz;
  yalnız LSP ve `trust.rs` okur.
- **Struct sinyalleri:** bit düzeni tamamen ortak (`struct_layout`,
  `struct_signals`); yaprak adı kuralı (`i_x`) üç yerde yeniden yazılmış.

## 1. Genişlik ve işaret

### Tip denetimi (başvuru)

| Bilgi | Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|---|
| G | `volt-hir/src/ty.rs:237` `TypeArena::width_of` | TD, alan | kendi | UInt/SInt/Bits genişliği; esnekte `hi`. Bool/Trit/Enum/Struct/Array `None`. |
| G | `volt-hir/src/ty.rs:309` `bit_width` | TD, alan | ORTAK `bit_width::Scalar` | Sinyalin genişliği, TD'nin tek kaynağı (#419): skaler tablo (Bool, Clock, Reset 1; Trit 2), sayısalda `width_of`, enum kodlama, struct düzen genişliği (bundle `None`: tek sinyal değil; E4012 kapsamı düzenin `W`sini okur), dizi eleman×uzunluk, `Delayed` iç tipinki; demet, örnek, literal, hata `None`. `taps`, W3003, `is_one_bit`, E4012 kapsamı, E2009 önerisi, genişlik gölgesi okur. `signal_width` ve `stmt.rs` `bit_width` kopyaları kaldırıldı. |
| G+İ | `volt-hir/src/ty.rs:188` `int_range`; `:170` `uint_flex`; `:178` `sint_flex`; `:212` `concrete` | TD | kendi | (işaret, lo, hi) görünümü. |
| G+İ | `volt-hir/src/typeck/type_ref.rs:30` `resolve_type_ref_uncached`; `:85` `resolve_sized`; `:117` `widened_int` | TD | kendi + TD consteval; ORTAK `MAX_WIDTH` | TypeRef → Ty; `<N>` 1..=`MAX_WIDTH` (65535); değeri bilinen aralık dışı `<N>` E2025 (`eval_type_arg`, #418). `u16` gösterimi `MAX_WIDTH`'e derleme zamanında bağlı. |
| G+İ | `volt-hir/src/typeck/synth.rs:20` `synth`; `:57` `synth_uncached`; `:101` `synth_int_lit`; `:501` `suffix_ty` | TD | kendi | `expr_types`'ı yazar; soneksiz literal `IntLit`. |
| G+İ | `volt-hir/src/typeck/synth.rs:260` `synth_unary`; `:360` `synth_if`; `:409` `synth_match`; `:163` `synth_call`; `:229` `synth_array_lit` | TD | kendi | |
| G+İ | `volt-hir/src/typeck/binop.rs:53` `arith_of`; `:150` `arith_result`; `:187` `trit_arith_result`; `:242` `synth_bitwise`; `:299` `synth_shift`; `:400` `unify_for_comparison` | TD | kendi | `+ -` esnek lo..hi+1, `*` lo..2·hi, `/ %` hi. |
| G+İ | `volt-hir/src/typeck/width.rs:28` `meet_int_ranges`; `:52` `operand_range`; `:73` `flex`; `:106` `clamp_width` | TD | kendi; ORTAK `MAX_WIDTH` | Aralık kesişimi. |
| G | `volt-hir/src/typeck/select.rs:15` `index_result`; `:61` `range_result`; `:139` `part_select_result`; `:213` `field_result`; `:263` `struct_field_type` | TD | kendi + `try_const_eval` | |
| G+İ | `volt-hir/src/typeck/check.rs:18` `check`; `:149` `check_arith`; `:195` `widen_operands` | TD | kendi | Check kipi: literal, `if`, `match` beklenen tipi alır; ADR-0041'de operandların kaydı hedef tip olur. |
| G+İ | `volt-hir/src/typeck/cast.rs:12` `synth_cast` | TD | `resolve_type_ref` | |
| G+İ | `volt-hir/src/typeck/stmt.rs:85` `handle_reg`; `:116` `handle_let`; `:342` `lvalue_type`; `:375` `suffix_bits` | TD | `TypeArena::bit_width`; `StructLayout::field_bits` | Kendi `bit_width` kopyası #419 ile kaldırıldı. |
| G | `volt-hir/src/typeck/enums.rs:23` `check_enum_decls`; `structs.rs:23` `check_struct_decls`; `:172` `struct_layout` | TD | ORTAK `enum_layout`, `struct_layout` | Genişlik tablolarını arenaya yazar. |
| G | `volt-hir/src/typeck/instance.rs:129` `builtin_port_type`; `:168` `taps_width`; `:185` `check_taps_width` | TD | `TypeArena::bit_width`, ORTAK `MAX_WIDTH` | Addr = log2(DEPTH), Taps = dim·`bit_width(T)`; genişliği olmayan T hata tipi (kendi tanısı var), sınırı aşan çarpım E2025 (#419; önce `unwrap_or(1)` ve `as u16`). |
| G+İ | `volt-hir/src/typeck/arith_cut.rs:149` `arith_width`; `:169` `value_bounds`; `:194` `derived_bounds`; `:254` `type_bounds` | TD (W2016) | kendi | |
| G+İ (sığma) | `width.rs:109` `uint_fits`, `:114` `sint_fits`; `check.rs:233`, `:284`; `cast.rs:35` `warn_literal_cut`; `literal_fold.rs:363` `literal_range`; `compare_width.rs:29` `fits`; `arith_cut.rs:291` `fits`; `matching.rs:508` `int_fits` | TD | kendi (7 kopya) | Bir (işaret, genişlik) tipinin değer aralığı her dosyada yeniden yazılmış. |
| G+İ | `volt-hir/src/typeck/mod.rs:257` `compute_def_type` | TD | `resolve_type_ref` | Döngü değişkeni `IntLit`. |
| G | `volt-hir/src/consteval.rs:185` `eval_type_arg`; `:137` `check_type` | TD tanıları | kendi; ORTAK `MAX_WIDTH` | Genişlik 1..=65535 (E2025); ifade başına bir tanı. |

### SV üretimi

| Bilgi | Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|---|
| G+İ | `volt-sv-emit/src/expr.rs:240` `width_of` → `own_width` | SV | **kendi** | "Kaba" genişlik: karşılaştırma 1, kaydırma sol, ikili max(l, r), işaret l && r; dizin 1 ya da eleman; `if`/`match` ilk bilinen kol; soneksiz literal `None`. Aşama 1'in gölgesi bunu ölçer. |
| G+İ | `volt-sv-emit/src/expr.rs:1945` `suffix_sig` | SV | kendi | TD `suffix_ty`'nin kopyası. |
| G | `volt-sv-emit/src/expr.rs:401` `builtin_field_sig` | SV | kendi | TD `builtin_port_type`'ın kopyası. |
| G+İ | `volt-sv-emit/src/expr.rs:54` `widen_sig`; `:1843` `fmt_int`; `:770` `emit_prec` | SV | kendi | Literal bağlam genişliğinde basılır; genişlik yoksa E2005. |
| G+İ | `volt-sv-emit/src/expr.rs:1139` `emit_binary`; `:1455` `arith_ctx`; `:605` `emit_assigned` | SV | `let_width` | İşlenen bağlamı (ADR-0041 `W'(x)`). |
| G | `volt-sv-emit/src/expr.rs:1223` `clamped_signed_shift`; `:1331` `address_width`; `:1343` `index_width_mismatch`; `:1363` `sized_index`; `:1261` `select_from_value`; `:1393` `part_select_from_value`; `:1435` `cut_value`; `:1730` `width_dependent` | SV | kendi; `eval_const` | Kendi clog2'si. |
| G+İ | `volt-sv-emit/src/expr.rs:1547` `emit_cast` | SV | `sig_of_typeref`, `width_of` | |
| G+İ | `volt-sv-emit/src/expr.rs` `value_arms`; `match_expr.rs` `scrutinee_sig` | SV | kendi | #215'ten sonra: `if`'in iki dalı ve match ifadesinin ifade kolları tek listede; `width_of`, `int_range`, `is_unsized_literal`, `width_dependent`, `tree_width_dependent`, `is_trit`, `enum_of_expr` ikisini bu listeden okur (type-inference.md §3.9). Deyim `match`'i ve match ifadesinin sınananı doğal genişlikte (`natural_sig`). |
| G+İ | `volt-sv-emit/src/let_width.rs:184` `int_range`; `:255` `binary_range`; `:305` `operand_meet`; `:137` `natural_sig`; `:146` `operation_sig`; `:164` `comparison_ctx`; `:71` `untyped_let_sig` | SV | **kendi**; ORTAK `MAX_WIDTH` | TD'nin esnek aralık kurallarını (ADR-0025) yeniden yazar. |
| G+İ | `volt-sv-emit/src/lib.rs:2513` `sig_of_typeref` | SV | ORTAK `const_eval::width`, `enum_layout`, `bit_width::Scalar`, takma ad | TypeRef → genişlik; `<N>`'in üst sınırını E2025 uygular (ORTAK `MAX_WIDTH`), aşan birim üreticiye ulaşmaz. |
| G | `volt-sv-emit/src/lib.rs:2462` `array_reg_sig`; `:2416` `signal_sig`; `:2442` `port_sig`; `:2612` `leaf_sig`; `:2375` `lvalue_sig` | SV | `sig_of_typeref`, `eval_const` | Paketli dizi eleman×uzunluk. |
| G+İ | `volt-sv-emit/src/lib.rs:1056` `emit_module` (sembol tablosu); `:1742` `emit_let_sync`; `:1775` `sync_source_sig`; `match_expr.rs:470` `emit_block_let` | SV | `sig_of_typeref`, `untyped_let_sig` | |
| G+İ | `volt-sv-emit/src/enums.rs:28` `enum_sig`; `:107` `enum_variant_sig`; `waves.rs:254` `wave_signal_of` | SV | ORTAK `enum_layout` | |
| G+İ | `volt-sv-emit/src/trit.rs:102` `trit_sum_sig` | SV | kendi | TD `trit_arith_result`'ın kopyası. |
| G | `volt-sv-emit/src/builtin_prim.rs:49` `addr_width`; `:83` `builtin_inst_info`; `builtin_data.rs:20` `struct_width` | SV | kendi; ORTAK `struct_layout` | DEPTH `eval_const`'la (TD yalnız literal kabul eder). |
| G | `volt-sv-emit/src/const_array.rs:33` `array_const_info`; `:158` `case_function` | SV | `array_reg_sig`; kendi clog2 | |
| G+İ | `volt-sv-emit/src/inline/types.rs:38` `canon_depth`; `:71` `widened`; `:137` `DeclTypes::type_of` | SV (fn açılımı) | kendi | Açılımın kendi tip gösterimi. |
| G | `volt-sv-emit/src/sim.rs:63` `collect_sim_ports`; `past.rs:73` `past_reg_block` | SV testbench, formal | ORTAK `bit_width::type_ref_bits` | `sim_width.rs`'in kendi `bits_at` kopyası #419 ile kaldırıldı. |

### Üretilen sürücüler, kısıtlar, test simülasyonu

| Bilgi | Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|---|
| G | `volt-syntax/src/parser/mmio.rs:524` `field_ty`; `:105` `FieldTy::width` | SW (`RegMap`) | kendi | Yalnız literal `N` (1..=32); const yok. |
| G | `volt-sw-emit/src/names.rs:9` `rust_type`; `:19` `c_type`; `check/mod.rs:182` `field_mask` | SW | `FieldDesc.width` | |
| — | `volt-sdc-emit/src/*` | SDC | — | Genişlik, işaret, sabit hesaplamaz. |
| G+İ | `volt-hir/src/sim_port.rs:110` `scalar_width`; `:155` `port_type_width`; `:187` `test_port_width` | SIM | ORTAK `bit_width::type_ref_bits`, `alias_chain_end`, `enum_of_type` | Genişlik ORTAK'tan (#419); burada yalnız değerin test dilinde yazılışı seçilir: Clock, Reset, Trit, dizi ve demet "skaler değil" (sayı olarak yazılmaz), struct bilinmiyor. `enum_width` kopyası kaldırıldı. |
| G+İ | `volt-hir/src/sim_port.rs:44-92` `PortWidth` yöntemleri; `:288` `check_value_fits` | SIM | kendi | Değer aralığı (TD'nin sığma kopyalarından ayrı). |
| G | `volt-hir/src/sim_struct.rs:40` `struct_port_layout`; `sim_load.rs:46` `check_load` | SIM | ORTAK `struct_layout`, `scalar_width` | |
| G+İ | `volt-driver/src/sim_lower.rs:288` `set_port`; `:326` `port_width`; `:332` `fits_without_check`; `sim_struct.rs:86` `labels`; `:136` `packed_literal` | SIM | `test_port_width`, `StructLayout` | |

## 2. Sabit değer

| Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|
| `volt-hir/src/consteval.rs:289` `const_eval`; `:298` `const_eval_uncached`; `:235` `eval_const_def`; `:640` `eval_binop`; `:791` `eval_unop`; `:828` `eval_builtin_call` | TD | **kendi** (tam değerlendirici) | Int/Bool/Array/Tuple/Enum/Struct, döngü E2020, `clog2`, `~`; `as` değeri kesmez. |
| `volt-hir/src/typeck/mod.rs:301` `try_const_eval`; `stmt.rs:184` `check_for` | TD | TD consteval | |
| `volt-hir/src/typeck/literal_fold.rs:105` `literal_sign_mag`; `:138` `eval_literal`; `:448` `literal_op`; `:210` `check_literal_expr` | TD | kendi | Soneksiz literal aritmetiği i128'de (#144). |
| `volt-hir/src/typeck/instance.rs:257` `builtin_const_arg` | TD | kendi | DEPTH/N yalnız literal. |
| `volt-hir/src/typeck/compare_width.rs:65`; `matching.rs:212`, `:397` | TD | TD consteval | |
| `volt-sv-emit/src/expr.rs:169` `eval_const`; `scope.rs:109` `Scope::bind` | SV | ORTAK `const_eval::eval` | Üreticinin bütün sabitleri. |
| `volt-sv-emit/src/expr.rs:183` `fold_literal_arith`; `:206` `literal_value`; `:123` `literal_op` | SV | **kendi** | TD `literal_fold`'un kopyası. |
| `volt-sv-emit/src/expr.rs:1237` `shift_amount` | SV | `eval_const` + kendi | `as uN` 2^N modunda kesilir (TD consteval kesmez). |
| `volt-sv-emit/src/expr.rs:1490` `fold_const_path`; `lib.rs:2248` `match_arm_label`; `generate.rs:23` `for_bounds`; `past.rs:63` `prev_depth` | SV | `eval_const` | |
| `volt-sv-emit/src/const_array.rs:59` `const_array_element`; `:65` `const_array_elements`; `:72`; `:113` | SV | ORTAK `const_eval::array_element(s)` | |
| `volt-sv-emit/src/structs/mod.rs:31` `const_int`; `structs/value.rs:352` `pack_array`; `builtin_data.rs:31` `default_literal` | SV | ORTAK `const_int` (üst düzey kapsam) | Düzen hesapları üreticinin kapsamını değil üst düzey kapsamı kullanır. |
| `volt-sw-emit/src/check/rtl.rs:252` `sv_literal`; `:198` `reset_values` | SW denetimi | kendi | Reset değerlerini üretilen SV metninden geri okur. |
| `volt-hir/src/sim_const.rs:40` `TestConsts::new`; `:144` `TestConsts::eval`; `:209` `plain_literal`; `sim_port.rs:208` `fold_binary`; `:182` `const_value` | SIM | kendi; ORTAK `const_int` | u64 sarmalı aritmetik; tasarım sabiti yalnız düz literalse sabittir. |
| `volt-driver/src/sim_lower.rs:127` `lower_value` | SIM | `TestConsts` | |
| `volt-ast/src/const_eval.rs:85` `eval`; `:92` `const_int`; `:99` `width`; `:334` `element`; `:345` `elements`; `:329` `variant_code` | ORTAK | — | Derinlik 64; `as`, `clog2`, `~`, alan erişimi yok. |

## 3. Takma ad çözümü

| Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|
| `volt-ast/src/alias.rs:21` `alias_chain_end` | ORTAK | — | Ada göre, derinlik 64; döngüde `Err(son)`. |
| `volt-ast/src/bit_width.rs:29` `Scalar::bits`; `:66` `type_ref_bits` | ORTAK | `alias_chain_end`, `const_eval`, `enum_layout`, `struct_layout` | Skaler genişlik tablosu (Bool, Clock, Reset 1; Trit 2) ve AST tipinin genişliği (#419). TD `bit_width`, SV `sig_of_typeref`, `struct_layout` yaprağı, SV testbench ve SIM okur. |
| `volt-ast/src/enum_layout.rs:99` `follow_aliases`; `struct_layout.rs:162` `struct_of_type`; `:222`; `:289`; `const_eval.rs:389` `is_array` | ORTAK | `alias_chain_end` | Döngüde `None`. |
| `volt-sv-emit/src/alias.rs:22` `resolve` | SV | `alias_chain_end` | Döngüde de son tipi verir (`follow_aliases`'tan farklı). Yaklaşık 30 fonksiyon çağırır: `flop_audit.rs:74` `is_clock_type`, `reset_sync.rs:37`, `lib.rs:789` `clock_ports_of`, `lib.rs:2513` `sig_of_typeref`, `inline/types.rs:38`, `structs/mod.rs:303` vb. |
| `volt-hir/src/typeck/type_ref.rs:118` `resolve_named_type` | TD | **kendi** | DefId ve kapsam tabanlı, derinlik sınırı yok (`alias_stack`). `Ty::Clock`, `Ty::Struct` buradan gelir. |
| `volt-hir/src/sim_port.rs:110` `scalar_width` | SIM | `alias_chain_end` | |

**Saat ve reset sınıflaması** (#234 ile düzeltildi; önceden yedi yer ve
ayrıştırıcının dört geçidi portu `TypeRefKind`'a bakarak sınıflıyordu,
`type Clk = clock` ile yazılan port saat sayılmıyordu). Sınıf
(`SignalClass`: saat, reset türüyle, veri) iki yoldan okunur; ikisi aynı
türü döndürür:

| Yol | Yardımcı | Kim çağırır | Neden bu yol |
|---|---|---|---|
| Tip denetiminin çözdüğü tip (`Ty::Clock`, `Ty::Reset`) | `volt-hir/src/port_class.rs:40` `DefTypes::class` (`:60` `port`, `:66` `is_clock_port`, `:72` `is_raw_reset`) | RDC `rdc/facts.rs:98` `clock_facts`, `:163` `raw_facts`, `rdc/mod.rs:125` `is_raw_reset_port`; SDC `constraints/walk.rs:81` `Scope::new`, ham reset süzgeci (`reset_chains`, `feeding_raw`); L1 `timing.rs:152` `ModuleTiming::run`; TD `typeck/reset_binding.rs:268` `is_raw_reset_def` (E2003); alan çıkarımı `domain/tables.rs:107`, `:112`; özet `summary.rs:272`; güven `trust.rs:132`; LSP `analysis.rs:340` `clock_ports` (tip denetimi koştuysa) | Tip denetiminden sonra koşar. Tip denetimi port tiplerini gövdelerden önce kaydeder (`typeck/stmt.rs:47` `type_ports`), böylece E2003 kuralı dosyada sonra bildirilen çocuğun portunu da sınıflar. |
| Takma ad zinciri (ada göre) | `volt-ast/src/alias.rs:37` `signal_class` (`alias_chain_end`) | SIM `sim.rs:273` `check_set_port` (E8503); RDC `rdc/mod.rs:63` `without_raw_reset_unused` (W1001 süzgeci); LSP `analysis.rs:340` (yarım kod); ayrıştırıcı `desugar.rs:1015` `single_clock` (E5011), `mmio.rs:829` `module_clock`, `register_assign.rs:55` `reg_clocks` (E0019 önerisi), `auto_contract/scan.rs:141` `scan_module` | Tip denetimine ulaşamaz: test blokları kardeş dosyayla birim olmadan denetlenir; W1001 süzgeci çözümleme hatalıyken de (tip denetimi koşmadan) çalışır; LSP tamamlaması çoğunlukla ayrıştırma hatalı yarım kodda istenir; ayrıştırıcı tip denetiminden önce koşar. |

Sınır: ayrıştırma sırasında koşan iki geçit (`single_clock`, `reg_clocks`)
yalnız kendisinden önce bildirilen takma adı görür (#415). `@mmio` ve
otomatik kontratlar dosya bütünüyle ayrıştırıldıktan sonra koşar; `use`
ile içe aktarılan takma ad da görülür. `volt-sv-emit` saat ve reset
portlarını kendi `alias::resolve`'u üzerinden zaten çözüyordu (yukarıdaki
satır); Aşama 2'de onun okuması da `port_class`'a geçer.

ADR-0137 (#412): takma ad zinciri yolunun yedi çağıranı nedenleri ve onları
kaldıracak adımla bir izin listesindedir
(`crates/volt-driver/tests/signal_class_callers_tests.rs`); listede
olmayan yeni çağıran testi düşürür. Sınıflama gölgesi
(`crates/volt-driver/tests/class_shadow_tests.rs`) derlenen her birimin
her portunda iki yolun aynı sınıfı verdiğini sınar; bilinen tek fark
#415'tir.

## 4. Saat alanı, saat ve reset sınıflaması

| Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|
| `volt-hir/src/domain/tables.rs:16` `collect_domain_decls`; `:59` `implicit_clock_domain`; `:128` `def_domain`; `:107` `is_clock_def`; `:115` `is_clock_port` | TD | **başvuru**; `Ty::Clock`; `domain` alanları ORTAK `domain_fields` (#420) | `collect_domain_decls` kenarı, reset'i ve `trust_level`'ı ortak okuyucudan alır. |
| `volt-ast/src/domain_fields.rs:42` `DomainDecl::field`, `:49` `repeated_fields`, `:61` `clock_edge`, `:72` `reset`, `:87` `trust_level`, `:96` `frequency` (#420'nin dalında) | ORTAK | — | `domain` bildiriminin tek okuyucusu: anahtarın ilk yazımı. İkinci yazım E1003'tür; onu ayrıştırıcı bildirimi kurduğu yerde raporlar (`volt-syntax/src/parser/item.rs` `repeated_domain_fields`, ADR-0140). TD, RDC, SDC ve SV okur. |
| `volt-hir/src/domain/module.rs:12` `infer_module`; `:55` `scan_clock_ports`; `:179` `assign_port_domains` | TD | başvuru | `@clk` zinciri; kontrat saati ilk saat (satır 43). |
| `volt-hir/src/domain/annotation.rs:12`; `trust_alias.rs:23`, `:39`; `reg.rs:17`, `:70`, `:236`; `expr.rs:11`, `:20`, `:90`, `:101`; `walk.rs:64`; `sync.rs:14`; `bidir.rs:15`; `edgeless.rs:74` | TD | başvuru | |
| `volt-hir/src/domain/instance.rs:111`, `:150`, `:245`, `:278`, `:304`, `:652`; `builtin_instance.rs:77` | TD | başvuru; `DomainRole` | |
| `volt-hir/src/domain/rdc/facts.rs:93`, `:137`, `:157`, `:187`, `:263`; `rdc/mod.rs:81`, `:126`; `rdc/converge.rs:43`, `:184` | TD (RDC) | kendi + `clock_resets_flops`; saat ve ham reset portu `port_class` (#234); `domain` reset'i ORTAK `domain_fields` (#420) | `declared_reset` alanı ortak okuyucudan alır; önce "son alan kazanır". |
| `volt-hir/src/summary.rs:272` `Clocks::is_clock_port` | TD | `port_class` | Takma adı çözer. |
| `volt-hir/src/timing.rs:152` `ModuleTiming::run` | TD | `port_class` (#234) | |
| `volt-hir/src/sim.rs:273` `check_set_port` | SIM | `signal_class` (#234) | |
| `volt-sv-emit/src/lib.rs:828` `collect_domains` | SV | ORTAK `domain_fields` (#420) | `domain` bildirimi → kenar ve reset; yazılmamış alan spec varsayılanı. Önce kendi döngüsü, "son alan kazanır". |
| `volt-sv-emit/src/flop_audit.rs:48` `port_domain` | SV | **kendi** | TD `scan_clock_ports`'un ada göre kopyası. |
| `volt-sv-emit/src/lib.rs:789` `clock_ports_of`; `:224` `domain_of_trigger` | SV | `port_domain`, `reset_sync` | |
| `volt-sv-emit/src/lib.rs:1790` `try_emit_sync_bridge`; `flop_audit.rs:176` `audit_sync`; `:98` `audit_module` | SV | **kendi** (iki kopya) | Yakalama saati iki yerde ayrı yazılmış. |
| `volt-sv-emit/src/reset_sync.rs:47` `feeding_raw`; `:87` `reset_free_clock`; `instance.rs:404` `parent_synced_reset` | SV, SIM | kendi + `clock_resets_flops` | Yorumlar TD `rdc` ve SDC `feeding_raw` ile "aynı kural" der; ortak kod yok. |
| `volt-sv-emit/src/builtin_prim.rs:83` `builtin_inst_info` | SV | `DomainRole` | |
| `volt-sv-emit/src/sva.rs:148`, `:225`, `:358`; `sim_contract.rs:112` | SV, SIM | kendi | Kontrat saati = ilk saat (TD `module.rs:43` ile aynı kural, ayrı kod). |
| `volt-sv-emit/src/lib.rs:1041` `chain_consumed`; `sim.rs:63` `collect_sim_ports` | SV, SIM | ORTAK `reset_chain`; `clock_ports_of` | |
| `volt-ast/src/reset_chain.rs:22` `chain_consumed`; `:86` `syncs_to`; `builtin.rs:294` `clock_resets_flops` | ORTAK | — | SV, SDC ve RDC kullanır. |
| `volt-hir/src/constraints/walk.rs:78` `Scope::new`; `:146` `clock_of_domain`; `:159` `driver_clock_depth` | SDC | **kendi** | Ayrı sürücü saati izleyicisi (derinlik 8); `@clockport` zincirini izlemez. |
| `volt-hir/src/constraints/walk.rs:335` `collect_domains`; `:379` `collect_module`; `apply_edges` | SDC | ORTAK `domain_fields` (#420) | `has_reset`, `frequency` ve kenar ortak okuyucudan. Önce `domain` bildiriminin üçüncü ayrıştırıcısıydı: `has_reset` "son alan kazanır" yerine `any`, `apply_edges` kenarda ilk alan (SV son alanı okuyordu). |
| `volt-hir/src/constraints/walk.rs:945` `sync_bridge`; `:1017` `builtin_bridge`; `:1113` `user_instance` | SDC | kendi | `builtin_bridge` `DomainRole` yerine port adlarını (`wr_clk`, `rd_clk`, `src_clk`, `dst_clk`) sabit yazar. |
| `volt-hir/src/constraints/walk.rs:1167` `reset_chains`; `:1201` `has_reset`; `:1212` `feeding_raw`; `:1248` `is_raw_reset` | SDC | `reset_chain`; gerisi kendi | `feeding_raw` SV `reset_sync::feeding_raw`'ın kopyası. |
| `volt-lsp/src/analysis.rs:340` `clock_ports` | LSP | `port_class`, yarım kodda `signal_class` (#234) | |
| `volt-lsp/src/hover.rs:307`; `inlay.rs:228`, `:237` | LSP | TD `DomainResult` | Yalnız okur. |

## 5. Struct sinyalleri

| Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|
| `volt-ast/src/struct_layout.rs:171` `layout`; `:193`; `:222`; `:289`; `:150`; `:162`; `:325`; `:318`; `:56` `Leaf::dotted`; `:61` `Leaf::suffix`; `:99`; `:107` | ORTAK | — | Yapraklar, genişlikler, LSB'ler, ad kuralları. |
| `volt-ast/src/struct_signals.rs:49` `StructTypes::new`; `:95`; `:127` `module_signals`; `:245` `struct_of_expr` | ORTAK | `struct_layout` | |
| `volt-sv-emit/src/structs/mod.rs:165` `leaf_name` | SV | **kendi** | `Leaf::suffix`'i yeniden yazar. |
| `volt-sv-emit/src/structs/mod.rs:95` `lower`; `:260`; `:448`; `:498`; `:568`; `:616`; `:781`; `structs/value.rs:198`, `:229`, `:397`, `:412`, `:431` | SV | ORTAK `StructTypes`, düzen | |
| `volt-sv-emit/src/structs/prim.rs:29` `pack_struct_prims` | SV | `struct_layout` + kendi | Veri çıkışlarını kendisi bulur (`prim_data_outs`'un kopyası). |
| `volt-sv-emit/src/inline/types.rs:205` `struct_field`; `:224` `struct_leaves`; `inline/expand.rs:688`, `:708` | SV | kısmen kendi | Struct'ı adıyla kendisi arar; sonek `path.join("_")`. |
| `volt-sv-emit/src/sim.rs:121` `struct_leaves`; `builtin_data.rs:20`, `:31`; `lib.rs:2462`; `alias.rs:85` | SV, SIM | ORTAK `struct_layout` | |
| `volt-hir/src/typeck/structs.rs:23`, `:172`, `:266`, `:316`; `select.rs:263`; `stmt.rs:295`, `:316`, `:375`, `:458`; `instance.rs:168` | TD | ORTAK `struct_layout` (TD `try_const_eval` ile) | Öteki tüketiciler `const_eval::const_int` kullanır. |
| `volt-hir/src/sim_struct.rs:40`, `:51`, `:232`; `sim_top.rs:19` | SIM | ORTAK `struct_layout` | |
| `volt-hir/src/sim_struct.rs:70` `sv_port_name` | SIM | **kendi** | `replace('.', "_")`: sonek kuralının ayrı kopyası. |
| `volt-hir/src/constraints/walk.rs:934` `struct_leaf_suffixes` | SDC | ORTAK `module_signals` + `suffix` | |
| `volt-driver/src/sim_struct.rs:86`, `:122` `packed_port`, `:136` `packed_literal`, `:163`; `sim_lower.rs:426`, `:462` | SIM | düzen yaprakları | `packed_port`/`packed_literal` `leaf.lsb` yerine kaydırma ve veya ile paketler. |
| `volt-lsp/src/hover.rs:150`, `:179`; `completion.rs:217`, `:286` | LSP | ORTAK `struct_layout`; `Ty::Struct` | |

## Bilinen ayrışmalar (iki hesabın bugün farklı cevap verdiği yerler)

Bunlar envanterin yan ürünüdür; Aşama 2'de ilgili bilgi geçmeden önce
çözülür ya da bakımcının kararıyla belgelenir.

**Ayıklama (#412, ADR-0137 §5; 2026-11-24, `main` d185a50c).** Her satır için en küçük örnek `volt
check`, `volt build` (SV, SVA, SDC, XDC, sürücüler) ve uygulanıyorsa
`volt test` ile çalıştırıldı. Sınıflar: **sessiz yanlış** (çıktı ya da
test sonucu spec'ten farklı ve tanı yok), **reddedilen** (bir taraf tanı
veriyor, kullanıcı ayrışmaya ulaşamıyor), **gizli** (bugün kullanıcının
görebileceği bir fark yok). Reddedilen ve gizli satırların bugünkü
davranışı `crates/volt-driver/tests/divergence_triage_tests.rs`'te
`rowN_` önekli testlerle sabittir. Açık sessiz yanlış satırların testi
yoktur; issue'su son sütundadır (#421); kapanan satır 1'in (#418), 5'in
(#419) ve 8'in (#420) testleri düzeltmeleriyle girdi. Ayıklamanın sessiz
olmayan bulguları (yanlış W2013, yanlış E2006, yanıltıcı E2005, okunmayan
`DomainInfo.reset`) #422'dedir.

| # | Bilgi | Ayrışma | Sınıf (#412) | Gözlenen | Test ya da issue |
|---|---|---|---|---|---|
| 1 | Genişlik üst sınırı | #418 ile giderildi: sınır tek sabittir, `volt_ast::const_eval::MAX_WIDTH` = 65535; TD consteval (E2025), `resolve_sized`, `widened_int`, `clamp_width`, `enum_layout`, `struct_layout` ve SV `let_width` onu okur. `const_eval::width` ve `sig_of_typeref` sınırı denetlemez; aşan birim E2025 ile durur, üreticiye ulaşmaz. | kapandı (#418) | Önce `bits<65536>` E2025'ten geçiyor, tip denetiminde sessizce hata tipi oluyordu: `bits<8>`'e atama E2001'siz derleniyor, SV `assign y = 8'(x)` kesiyordu. Şimdi E2025; takma ad hedefi, blok `let`'i ve fonksiyon imzası da. | `width_limit_tests.rs`; `tests/real_bugs/418_width_above_limit.volt` |
| 2 | Sabitte `as` | TD consteval `as`'ı yok sayar; SV `shift_amount` `as uN`'yi keser. | reddedilen | `as`'lı sabit üreticide E0003; dizi indeksinde yanlış E2006 (kesilmemiş değer). Satır içi `x << (9 as u3)` derlenir, SV doğru, W2013 yanlış ("hep 0"; açık bulgu). | `row2_*`; #422 |
| 3 | `clog2` ve `~` | TD consteval değerlendirir; `volt_ast::const_eval`'da kolları yok, SV değer alamaz. | reddedilen | genişlikte E2005, değerde E0003. | `row3_*`; #422 |
| 4 | Yerleşik DEPTH/N | TD yalnız literal kabul eder; SV herhangi bir `eval_const` değerini. | reddedilen | adlı sabit E2008; generic parametre monomorflamada literal, iki taraf aynı. | `row4_*` |
| 5 | Bool/Trit genişliği | #419 ile giderildi: TD'de tek fonksiyon `TypeArena::bit_width`, skaler tablo ORTAK `volt_ast::bit_width::Scalar` (Bool, Clock, Reset 1; Trit 2); AST tarafı `type_ref_bits` (SV testbench, SIM). `signal_width`, TD `bit_width` kopyası, `unwrap_or(1)`, SV `bits_at` ve SIM `enum_width` kaldırıldı; genişlik gölgesi de `bit_width`'i okur. SIM'in Trit'i "skaler değil" sayması genişlik değil, test dilinde yazılış kuralıdır. | kapandı (#419) | Önce `ShiftRegister<Trit, 4>`'ün `taps`'ı TD'de `bits<4>`, SV'de 8 bit; `bits<4>`'e atama SV'de kesiliyordu, Trit `sync()`'i W3003 almıyordu. Şimdi `taps` `bits<8>`, Trit `sync()`'i W3003; `taps` genişlik sınırını aşarsa E2025. | `bit_width_tests.rs`; `ui/pass/163_scalar_width_uses.volt`; `tests/real_bugs/419_shift_register_trit_taps.volt` |
| 6 | Test dilinde tasarım sabiti | Yalnız düz (ya da eksi) literal sabit sayılır; hesaplanan `const` sayılmaz. | reddedilen | hesaplanan const E8506; düz literal const E8512 sığma denetimine girer. | `row6_*` |
| 7 | Takma adlı saat/reset | #234 ile giderildi: sınıf `port_class` ya da `signal_class`'tan okunur (bölüm 3); kalan sınır #415. | kapandı (#416) | #234'ün örnekleri doğrudan yazılanla aynı tanı ve dosyalar; kalan #415 reddedilen (E5011; E0019 önerisinde `on <clock>`). | `aliased_clock_reset_tests.rs`; `row7_issue_415_*`; sınıflama gölgesi (ADR-0137 §4) |
| 8 | `domain` bildirimi | #420 ile giderildi: bir alan anahtarı bildirimde en çok bir kez yazılır, ikinci yazım E1003 (iki alanı gösterir; ADR-0140). Alanları okuyan beş yer (TD `collect_domain_decls`, RDC `declared_reset`, SDC `collect_domains` ve `apply_edges`, SV `collect_domains`) ORTAK `volt_ast::domain_fields`'ı okur; "son alan kazanır", `any` ve kenarda "ilk alan" kodda kalmadı. | kapandı (#420) | Önce `reset = none` ardından `reset = sync active_high` tanısız derleniyordu: SV ve RDC son alanı alıyor, senkronizör üretiliyor; XDC onun `ASYNC_REG` satırını yazmıyordu. Şimdi E1003. | `domain_field_once_tests.rs`; `ui/fail/317_domain_field_written_twice.volt`; `ui/pass/164_domain_every_field_once.volt`; `tests/real_bugs/420_domain_reset_written_twice.volt` |
| 9 | Sürücü saati | SDC `@clockport` zincirini izlemez; TD ve SV izler. | sessiz yanlış | `@a_clk` açıklamalı sinyalden `let` ile `sync()`: XDC `set_max_delay -datapath_only` yerine `set_false_path`. Yakalama register'ı da (`sync_<x>_src`) yalnız `@Alan` yazımında üretilir. Saat portu başka bir saat portuyla açıklanınca (`in c1 : clock @c0`) SDC yanlış W0022 verir, `create_clock` yazmaz. | #421 |
| 10 | Struct yaprak adı | `Leaf::suffix` dışında üç ayrı yazım. | gizli | SV, SDC ve test dili `p.i.x`'i `p_i_x` yazar; çakışma E1003. | `row10_*` |
| 11 | Değer aralığı | "Sığar mı" TD'de yedi kez, SIM'de bir kez yazılmış. | gizli | yedi TD kopyası 1..=64 bitte aynı sınırı verir (E2010, W2010); SIM'in bit deseni kabulü ADR-0059'un kuralı. | `row11_*` |

Ayıklamanın yan bulgusu (gizli, #422): TD'nin `DomainInfo.reset`'i
(`domain/tables.rs`) yazılmamış reset alanına `none` verir; spec'in
varsayılanı `sync active_high`'tır (sv-mapping.md) ve RDC ile SV onu
kullanır. Alan hiçbir yerde okunmadığı için bugün görünmez.
