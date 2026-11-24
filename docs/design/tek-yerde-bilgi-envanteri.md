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
| G | `volt-hir/src/ty.rs:282` `signal_width` | TD, alan | kendi | Enum kodlama, struct düzen genişliği; Bool ve Trit `None`. |
| G+İ | `volt-hir/src/ty.rs:188` `int_range`; `:170` `uint_flex`; `:178` `sint_flex`; `:212` `concrete` | TD | kendi | (işaret, lo, hi) görünümü. |
| G+İ | `volt-hir/src/typeck/type_ref.rs:24` `resolve_type_ref_uncached`; `:75` `resolve_sized`; `:103` `widened_int` | TD | kendi + TD consteval | TypeRef → Ty; `<N>` 1..=65535. |
| G+İ | `volt-hir/src/typeck/synth.rs:20` `synth`; `:57` `synth_uncached`; `:101` `synth_int_lit`; `:501` `suffix_ty` | TD | kendi | `expr_types`'ı yazar; soneksiz literal `IntLit`. |
| G+İ | `volt-hir/src/typeck/synth.rs:260` `synth_unary`; `:360` `synth_if`; `:409` `synth_match`; `:163` `synth_call`; `:229` `synth_array_lit` | TD | kendi | |
| G+İ | `volt-hir/src/typeck/binop.rs:53` `arith_of`; `:150` `arith_result`; `:187` `trit_arith_result`; `:242` `synth_bitwise`; `:299` `synth_shift`; `:400` `unify_for_comparison` | TD | kendi | `+ -` esnek lo..hi+1, `*` lo..2·hi, `/ %` hi. |
| G+İ | `volt-hir/src/typeck/width.rs:28` `meet_int_ranges`; `:52` `operand_range`; `:73` `flex`; `:104` `clamp_width` | TD | kendi | Aralık kesişimi. |
| G | `volt-hir/src/typeck/select.rs:15` `index_result`; `:61` `range_result`; `:139` `part_select_result`; `:213` `field_result`; `:263` `struct_field_type` | TD | kendi + `try_const_eval` | |
| G+İ | `volt-hir/src/typeck/check.rs:18` `check`; `:149` `check_arith`; `:195` `widen_operands` | TD | kendi | Check kipi: literal, `if`, `match` beklenen tipi alır; ADR-0041'de operandların kaydı hedef tip olur. |
| G+İ | `volt-hir/src/typeck/cast.rs:12` `synth_cast` | TD | `resolve_type_ref` | |
| G+İ | `volt-hir/src/typeck/stmt.rs:85` `handle_reg`; `:116` `handle_let`; `:342` `lvalue_type`; `:375` `suffix_bits`; `:458` `bit_width` | TD | kendi; `StructLayout::field_bits` | `bit_width` Bool 1, Trit 2, dizi eleman×uzunluk: `signal_width`'ten farklı. |
| G | `volt-hir/src/typeck/enums.rs:23` `check_enum_decls`; `structs.rs:23` `check_struct_decls`; `:172` `struct_layout` | TD | ORTAK `enum_layout`, `struct_layout` | Genişlik tablolarını arenaya yazar. |
| G | `volt-hir/src/typeck/instance.rs:127` `builtin_port_type` | TD | kendi | Addr = log2(DEPTH), Taps = dim·T. |
| G+İ | `volt-hir/src/typeck/arith_cut.rs:149` `arith_width`; `:169` `value_bounds`; `:194` `derived_bounds`; `:254` `type_bounds` | TD (W2016) | kendi | |
| G+İ (sığma) | `width.rs:109` `uint_fits`, `:114` `sint_fits`; `check.rs:233`, `:284`; `cast.rs:35` `warn_literal_cut`; `literal_fold.rs:363` `literal_range`; `compare_width.rs:29` `fits`; `arith_cut.rs:291` `fits`; `matching.rs:508` `int_fits` | TD | kendi (7 kopya) | Bir (işaret, genişlik) tipinin değer aralığı her dosyada yeniden yazılmış. |
| G+İ | `volt-hir/src/typeck/mod.rs:257` `compute_def_type` | TD | `resolve_type_ref` | Döngü değişkeni `IntLit`. |
| G | `volt-hir/src/consteval.rs:180` `eval_type_arg`; `:132` `check_type` | TD tanıları | kendi | Genişlik 1..=65536 (E2025). |

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
| G+İ | `volt-sv-emit/src/let_width.rs:184` `int_range`; `:255` `binary_range`; `:305` `operand_meet`; `:137` `natural_sig`; `:146` `operation_sig`; `:164` `comparison_ctx`; `:71` `untyped_let_sig` | SV | **kendi** | TD'nin esnek aralık kurallarını (ADR-0025) yeniden yazar; `MAX_WIDTH` kopyası (`:20`). |
| G+İ | `volt-sv-emit/src/lib.rs:2513` `sig_of_typeref` | SV | ORTAK `const_eval::width`, `enum_layout`, takma ad | TypeRef → genişlik; `<N>` için üst sınır yok (TD 65535). |
| G | `volt-sv-emit/src/lib.rs:2462` `array_reg_sig`; `:2416` `signal_sig`; `:2442` `port_sig`; `:2612` `leaf_sig`; `:2375` `lvalue_sig` | SV | `sig_of_typeref`, `eval_const` | Paketli dizi eleman×uzunluk. |
| G+İ | `volt-sv-emit/src/lib.rs:1056` `emit_module` (sembol tablosu); `:1742` `emit_let_sync`; `:1775` `sync_source_sig`; `match_expr.rs:470` `emit_block_let` | SV | `sig_of_typeref`, `untyped_let_sig` | |
| G+İ | `volt-sv-emit/src/enums.rs:28` `enum_sig`; `:107` `enum_variant_sig`; `waves.rs:254` `wave_signal_of` | SV | ORTAK `enum_layout` | |
| G+İ | `volt-sv-emit/src/trit.rs:102` `trit_sum_sig` | SV | kendi | TD `trit_arith_result`'ın kopyası. |
| G | `volt-sv-emit/src/builtin_prim.rs:49` `addr_width`; `:83` `builtin_inst_info`; `builtin_data.rs:20` `struct_width` | SV | kendi; ORTAK `struct_layout` | DEPTH `eval_const`'la (TD yalnız literal kabul eder). |
| G | `volt-sv-emit/src/const_array.rs:33` `array_const_info`; `:158` `case_function` | SV | `array_reg_sig`; kendi clog2 | |
| G+İ | `volt-sv-emit/src/inline/types.rs:38` `canon_depth`; `:71` `widened`; `:137` `DeclTypes::type_of` | SV (fn açılımı) | kendi | Açılımın kendi tip gösterimi. |
| G | `volt-sv-emit/src/sim_width.rs:21` `port_bits`; `:25` `bits_at`; `sim.rs:63` `collect_sim_ports`; `past.rs:73` `past_reg_block` | SV testbench, formal | ORTAK `const_eval::width`, `enum_layout` | Trit 2. |

### Üretilen sürücüler, kısıtlar, test simülasyonu

| Bilgi | Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|---|
| G | `volt-syntax/src/parser/mmio.rs:524` `field_ty`; `:105` `FieldTy::width` | SW (`RegMap`) | kendi | Yalnız literal `N` (1..=32); const yok. |
| G | `volt-sw-emit/src/names.rs:9` `rust_type`; `:19` `c_type`; `check/mod.rs:182` `field_mask` | SW | `FieldDesc.width` | |
| — | `volt-sdc-emit/src/*` | SDC | — | Genişlik, işaret, sabit hesaplamaz. |
| G+İ | `volt-hir/src/sim_port.rs:110` `scalar_width`; `:145` `enum_width`; `:160` `port_type_width`; `:193` `test_port_width` | SIM | ORTAK `const_eval::width`, `alias_chain_end`, `enum_layout` | Trit ve dizi "skaler değil". |
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
| `volt-ast/src/enum_layout.rs:99` `follow_aliases`; `struct_layout.rs:162` `struct_of_type`; `:222`; `:289`; `const_eval.rs:389` `is_array` | ORTAK | `alias_chain_end` | Döngüde `None`. |
| `volt-sv-emit/src/alias.rs:22` `resolve` | SV | `alias_chain_end` | Döngüde de son tipi verir (`follow_aliases`'tan farklı). Yaklaşık 30 fonksiyon çağırır: `flop_audit.rs:74` `is_clock_type`, `reset_sync.rs:37`, `lib.rs:789` `clock_ports_of`, `lib.rs:2513` `sig_of_typeref`, `inline/types.rs:38`, `structs/mod.rs:303` vb. |
| `volt-hir/src/typeck/type_ref.rs:118` `resolve_named_type` | TD | **kendi** | DefId ve kapsam tabanlı, derinlik sınırı yok (`alias_stack`). `Ty::Clock`, `Ty::Struct` buradan gelir. |
| `volt-hir/src/sim_port.rs:110` `scalar_width` | SIM | `alias_chain_end` | |

**Takma adı hiç çözmeyen yerler** (saat ya da reset portunu `TypeRefKind`'a
bakarak sınıflar; `type Clk = clock` ile yazılan port saat sayılmaz):

| Yer | Kullanan |
|---|---|
| `volt-hir/src/domain/rdc/facts.rs:93` `clock_facts` (satır 102), `:157` `raw_facts` (satır 163) | TD (RDC) |
| `volt-hir/src/domain/rdc/mod.rs:81` `is_raw_reset` | TD (RDC) |
| `volt-hir/src/constraints/walk.rs:78` `Scope::new` (satır 90), `:1248` `is_raw_reset` | SDC |
| `volt-hir/src/timing.rs:151` `ModuleTiming::run` (satır 165) | TD (L1 zamanlama) |
| `volt-hir/src/sim.rs:272` `check_set_port` (satır 284) | SIM (E8503) |
| `volt-lsp/src/analysis.rs:334` `clock_ports` | LSP (`on` tamamlama) |

Bunlardan birinin etkisi doğrulandı (rapordaki "Açık bulgular"): saat portu
takma adla yazılınca `volt build --emit=sdc` kısıt dosyası üretmez.

## 4. Saat alanı, saat ve reset sınıflaması

| Yer | Kullanan | Yardımcı | Not |
|---|---|---|---|
| `volt-hir/src/domain/tables.rs:16` `collect_domain_decls`; `:59` `implicit_clock_domain`; `:128` `def_domain`; `:107` `is_clock_def`; `:115` `is_clock_port` | TD | **başvuru**; `Ty::Clock` | |
| `volt-hir/src/domain/module.rs:12` `infer_module`; `:55` `scan_clock_ports`; `:179` `assign_port_domains` | TD | başvuru | `@clk` zinciri; kontrat saati ilk saat (satır 43). |
| `volt-hir/src/domain/annotation.rs:12`; `trust_alias.rs:23`, `:39`; `reg.rs:17`, `:70`, `:236`; `expr.rs:11`, `:20`, `:90`, `:101`; `walk.rs:64`; `sync.rs:14`; `bidir.rs:15`; `edgeless.rs:74` | TD | başvuru | |
| `volt-hir/src/domain/instance.rs:111`, `:150`, `:245`, `:278`, `:304`, `:652`; `builtin_instance.rs:77` | TD | başvuru; `DomainRole` | |
| `volt-hir/src/domain/rdc/facts.rs:93`, `:137`, `:157`, `:187`, `:263`; `rdc/mod.rs:81`, `:126`; `rdc/converge.rs:43`, `:184` | TD (RDC) | kendi + `clock_resets_flops` | |
| `volt-hir/src/summary.rs:272` `Clocks::is_clock_port` | TD | `Ty::Clock` | Takma adı çözer. |
| `volt-hir/src/timing.rs:151` `ModuleTiming::run` | TD | kendi | |
| `volt-hir/src/sim.rs:272` `check_set_port` | SIM | kendi | |
| `volt-sv-emit/src/lib.rs:828` `collect_domains` | SV | **kendi** | `domain` bildirimi → kenar ve reset; son alan kazanır. |
| `volt-sv-emit/src/flop_audit.rs:48` `port_domain` | SV | **kendi** | TD `scan_clock_ports`'un ada göre kopyası. |
| `volt-sv-emit/src/lib.rs:789` `clock_ports_of`; `:224` `domain_of_trigger` | SV | `port_domain`, `reset_sync` | |
| `volt-sv-emit/src/lib.rs:1790` `try_emit_sync_bridge`; `flop_audit.rs:176` `audit_sync`; `:98` `audit_module` | SV | **kendi** (iki kopya) | Yakalama saati iki yerde ayrı yazılmış. |
| `volt-sv-emit/src/reset_sync.rs:47` `feeding_raw`; `:87` `reset_free_clock`; `instance.rs:404` `parent_synced_reset` | SV, SIM | kendi + `clock_resets_flops` | Yorumlar TD `rdc` ve SDC `feeding_raw` ile "aynı kural" der; ortak kod yok. |
| `volt-sv-emit/src/builtin_prim.rs:83` `builtin_inst_info` | SV | `DomainRole` | |
| `volt-sv-emit/src/sva.rs:148`, `:225`, `:358`; `sim_contract.rs:112` | SV, SIM | kendi | Kontrat saati = ilk saat (TD `module.rs:43` ile aynı kural, ayrı kod). |
| `volt-sv-emit/src/lib.rs:1041` `chain_consumed`; `sim.rs:63` `collect_sim_ports` | SV, SIM | ORTAK `reset_chain`; `clock_ports_of` | |
| `volt-ast/src/reset_chain.rs:22` `chain_consumed`; `:86` `syncs_to`; `builtin.rs:294` `clock_resets_flops` | ORTAK | — | SV, SDC ve RDC kullanır. |
| `volt-hir/src/constraints/walk.rs:78` `Scope::new`; `:146` `clock_of_domain`; `:159` `driver_clock_depth` | SDC | **kendi** | Ayrı sürücü saati izleyicisi (derinlik 8); `@clockport` zincirini izlemez. |
| `volt-hir/src/constraints/walk.rs:335` `collect_domains`; `:379` `collect_module` | SDC | **kendi** | `domain` bildiriminin üçüncü ayrıştırıcısı; `has_reset` "son alan kazanır" yerine `any`. |
| `volt-hir/src/constraints/walk.rs:945` `sync_bridge`; `:1017` `builtin_bridge`; `:1113` `user_instance` | SDC | kendi | `builtin_bridge` `DomainRole` yerine port adlarını (`wr_clk`, `rd_clk`, `src_clk`, `dst_clk`) sabit yazar. |
| `volt-hir/src/constraints/walk.rs:1167` `reset_chains`; `:1201` `has_reset`; `:1212` `feeding_raw`; `:1248` `is_raw_reset` | SDC | `reset_chain`; gerisi kendi | `feeding_raw` SV `reset_sync::feeding_raw`'ın kopyası. |
| `volt-lsp/src/analysis.rs:334` `clock_ports` | LSP | kendi | |
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

| Bilgi | Ayrışma |
|---|---|
| Genişlik üst sınırı | TD `<N>`'i 65535'te (`resolve_sized`), TD consteval 65536'da keser; `volt_ast::const_eval::width` ve SV `sig_of_typeref` sınırsız. |
| Sabitte `as` | TD consteval `as`'ı yok sayar; SV `shift_amount` `as uN`'yi keser. |
| `clog2` ve `~` | TD consteval değerlendirir; `volt_ast::const_eval`'da kolları yok, SV değer alamaz. |
| Yerleşik DEPTH/N | TD yalnız literal kabul eder; SV herhangi bir `eval_const` değerini. |
| Bool/Trit genişliği | `signal_width` Bool ve Trit için `None`; `bit_width` Bool 1, Trit 2; SV `bits_at` Trit 2; SIM `scalar_width` Trit'i skaler saymaz. |
| Test dilinde tasarım sabiti | Yalnız düz (ya da eksi) literal sabit sayılır; hesaplanan `const` sayılmaz. |
| Takma adlı saat/reset | Yedi yer takma adı çözmez (bölüm 3). |
| `domain` bildirimi | Üç ayrıştırıcı; reset alanında "son alan kazanır" ile `any` ayrışır. |
| Sürücü saati | SDC `@clockport` zincirini izlemez; TD ve SV izler. |
| Struct yaprak adı | `Leaf::suffix` dışında üç ayrı yazım. |
| Değer aralığı | "Sığar mı" TD'de yedi kez, SIM'de bir kez yazılmış. |
