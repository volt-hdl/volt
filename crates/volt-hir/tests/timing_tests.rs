//! L1 zamanlama denetimi testleri (ADR-0037, timing.rs).
//!
//! Kapsam: `@strict_timing` altında E5010 üretimi ve muafiyetler
//! (literal/sabit, koşul, tutma yazımı, açık anotasyon iddiası,
//! geri besleme döngüleri), `delay<K>` yeniden hizalama, çıkarım
//! zinciri ve `@strict_timing` olmayan modüllerin sessizliği.

use volt_hir::analyze;
use volt_syntax::{parse, FileId};

/// Kaynağı ayrıştırır, analiz eder, E5010 sayısını döndürür.
fn e5010_count(src: &str) -> usize {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hataları: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E5010")
        .count()
}

const TWO_STAGE_MISMATCH_BODY: &str = "
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = a + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
";

#[test]
fn two_stage_regs_mismatch_is_e5010() {
    let src = format!("@strict_timing\nmodule M {{ {TWO_STAGE_MISMATCH_BODY}");
    assert_eq!(e5010_count(&src), 1);
}

#[test]
fn non_strict_module_stays_silent() {
    // Geriye uyumluluk: @strict_timing yoksa aynı kaynak sessizdir.
    let src = format!("module M {{ {TWO_STAGE_MISMATCH_BODY}");
    assert_eq!(e5010_count(&src), 0);
}

#[test]
fn strictness_is_per_module() {
    // Katı modüldeki hata, katı olmayan komşuyu etkilemez ve tersi.
    let strict = format!("@strict_timing\nmodule S {{ {TWO_STAGE_MISMATCH_BODY}");
    let loose = format!("module L {{ {TWO_STAGE_MISMATCH_BODY}");
    assert_eq!(e5010_count(&format!("{strict}\n{loose}")), 1);
}

#[test]
fn equal_delays_combine_clean() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 1> = 0

    let sum = a + b

    on clk {
        a <= x
        b <= x
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn delay_k_realigns_clean() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = delay<1>(a) + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn nested_delay_accumulates() {
    // delay<1>(delay<1>(x)): 0 → 2 çevrim; 2 çevrimlik register'la uyumlu.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = delay<1>(delay<1>(x)) + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn delay_on_literal_pins_it() {
    // Zamanlamasız değere delay<K> sabitler: delay<2>(1) tam 2 çevrimdir.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = delay<2>(1) + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn literal_operand_is_exempt() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0

    let sum = a + 5

    on clk { a <= x }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn const_operand_is_exempt() {
    assert_eq!(
        e5010_count(
            "const BIAS : u32 = 7

@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0

    let sum = a + BIAS

    on clk { a <= x }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn undelayed_port_mix_is_e5010() {
    // Giriş portu tanım gereği 0 çevrimdir (ADR-0037 §3).
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg tmp : Delayed<u32, 1> = 0
    reg b   : Delayed<u32, 2> = 0

    let sum = x + b

    on clk {
        tmp <= x
        b   <= tmp
    }

    y = sum
}
"
        ),
        1
    );
}

#[test]
fn annotated_reg_write_mismatch_is_e5010() {
    // Delayed<_, 2> register'ı 0 çevrimlik porttan yazılamaz: register
    // bir çevrim ekler, kaynak 1 çevrim olmalıydı.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg b : Delayed<u32, 2> = 0

    on clk { b <= x }

    y = b
}
"
        ),
        1
    );
}

#[test]
fn annotated_reg_chain_clean() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0
    reg s3 : Delayed<u32, 3> = 0

    on clk {
        s1 <= x
        s2 <= s1
        s3 <= s2
    }

    y = s3
}
"
        ),
        0
    );
}

#[test]
fn reg_hold_write_is_exempt() {
    // `s2 <= s2` tutma yazımıdır (stall deseni) — gecikme denklemine
    // katılmaz ve hata üretmez.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    in  en  : bool
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    on clk {
        s1 <= x
        if en {
            s2 <= s1
        } else {
            s2 <= s2
        }
    }

    y = s2
}
"
        ),
        0
    );
}

#[test]
fn annotated_let_is_retiming_assertion() {
    // Açık anotasyonlu let, sonucun gecikmesini BİLDİRİR; başlatıcısı
    // aşamaları karıştırabilir (forwarding kaçış kapısı).
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let fwd : Delayed<u32, 1> = if s2 != 0 { s2 } else { s1 }

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = fwd
}
"
        ),
        0
    );
}

#[test]
fn inference_propagates_through_unannotated_defs() {
    // Anotasyonsuz let/reg gecikmeyi taşır: d = s1 (1), t = d + 1 çevrim
    // (2), t + s2 (2) uyumludur.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let d = s1
    reg t : u32 = 0

    let sum = t + s2

    on clk {
        s1 <= x
        s2 <= s1
        t  <= d
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn unannotated_reg_inferred_delay_mismatch_is_e5010() {
    // t, s1'den yazılır → 2 çevrim çıkarılır; s1 (1) ile karışamaz.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg t : u32 = 0

    let sum = t + s1

    on clk {
        s1 <= x
        t  <= s1
    }

    y = sum
}
"
        ),
        1
    );
}

#[test]
fn feedback_counter_is_unconstrained() {
    // Geri beslemeli register (sayaç) sabit gecikmeye oturmaz → serbest.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg cnt : u32 = 0

    let sum = cnt + x

    on clk { cnt <= cnt + 1 }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn if_branch_mismatch_is_e5010() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    in  sel : bool
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let v = if sel { s1 } else { s2 }

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = v
}
"
        ),
        1
    );
}

#[test]
fn if_condition_is_exempt() {
    // Koşul kontrol sinyalidir: 0 çevrimlik `sel`, 2 çevrimlik dallarla
    // aynı ifadede yaşayabilir (ADR-0037 §2).
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    in  sel : bool
    out y   : u32

    reg s2 : Delayed<u32, 2> = 0
    reg s2b : Delayed<u32, 2> = 0

    let v = if sel { s2 } else { s2b }

    on clk {
        s2  <= delay<1>(x)
        s2b <= delay<1>(x)
    }

    y = v
}
"
        ),
        0
    );
}

#[test]
fn annotated_array_reg_index_mix_is_e5010() {
    // 3 çevrimlik dizinin 0 çevrimlik indeksle okunması karışımdır;
    // bilinçliyse okuma açık anotasyonlu bir let'e alınır.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  i   : u2
    out y   : u32

    reg arr : Delayed<[u32; 4], 3> = [0; 4]

    let v = arr[i]

    y = v
}
"
        ),
        1
    );
}

#[test]
fn delayed_cycles_must_be_int_literal() {
    // V0 sınırı: Delayed<_, N> içindeki N tamsayı literali olmalı.
    assert_eq!(
        e5010_count(
            "const K : u32 = 2

@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg b : Delayed<u32, K> = 0

    on clk { b <= x }

    y = b
}
"
        ),
        1
    );
}

#[test]
fn delay_k_const_name_is_rejected() {
    // V0 sınırı: delay<K> içindeki K tamsayı literali olmalı.
    assert_eq!(
        e5010_count(
            "const K : u32 = 1

@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg b : Delayed<u32, 2> = 0

    let sum = delay<K>(x) + b

    on clk { b <= delay<1>(x) }

    y = sum
}
"
        ),
        1
    );
}

#[test]
fn output_port_assign_mismatch_is_e5010() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = s1 + s2
}
"
        ),
        1
    );
}

#[test]
fn unary_preserves_delay() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = (~a) + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
"
        ),
        1
    );
}

#[test]
fn annotated_input_port_overrides_default() {
    // Giriş portu açıkça Delayed<_, 2> bildirilebilir (dış boru hattından
    // gelen veri): 2 çevrimlik register'la doğrudan birleşir.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    in  late : Delayed<u32, 2>
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let sum = late + s2

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = sum
}
"
        ),
        0
    );
}

/// Ada göre kesin gecikme (ADR-0091: editör gecikme ipucunun kaynağı).
fn delays_by_name(src: &str) -> std::collections::BTreeMap<String, u32> {
    let parsed = parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let analysis = volt_hir::analyze(&parsed.ast);
    let res = &analysis.resolve;
    let timing = volt_hir::analyze_timing(&parsed.ast, res, &analysis.typeck);
    timing
        .delays
        .iter()
        .map(|(def, n)| (res.defs[def.0 as usize].name.clone(), *n))
        .collect()
}

const CHAIN_BODY: &str = "
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : u32 = 0
    reg b : u32 = 0
    let s = a + 1
    reg cnt : u8 = 0

    on clk {
        a <= x
        b <= s
        cnt <= cnt + 1
    }

    y = b
}
";

#[test]
fn analyze_timing_exposes_exact_delays_of_a_strict_module() {
    let d = delays_by_name(&format!("@strict_timing\nmodule M {{ {CHAIN_BODY}"));
    assert_eq!(d.get("x"), Some(&0));
    assert_eq!(d.get("a"), Some(&1));
    assert_eq!(d.get("s"), Some(&1));
    assert_eq!(d.get("b"), Some(&2));
    assert_eq!(d.get("y"), Some(&2));
    // Saat serbest, geri beslemeli sayaç sabit gecikmeye oturmaz.
    assert_eq!(d.get("clk"), None);
    assert_eq!(d.get("cnt"), None);
}

#[test]
fn analyze_timing_has_no_delays_outside_strict_modules() {
    let src = format!("module M {{ {CHAIN_BODY}");
    assert!(delays_by_name(&src).is_empty());
    // Tanılar check_timing ile aynı kaynaktan.
    let parsed = parse(FileId(0), &src);
    let analysis = volt_hir::analyze(&parsed.ast);
    assert!(volt_hir::check_timing(&parsed.ast, &analysis.resolve, &analysis.typeck).is_empty());
}

// ═══ Bildirilen gecikme gerçek gecikmeyle karşılaştırılır (#354) ═══
//
// `let`, `wire` ve çıkış portundaki `Delayed<T, N>`, değerin gecikmesi
// tam biliniyorsa N ile karşılaştırılır; uyuşmazlık E5010. Değer
// aşamaları karıştırıyorsa (ileri besleme) bildirim bugünkü gibi
// yeniden zamanlama iddiasıdır ve denetlenmez.

/// İki aşamalı modül: `a` 1, `b` 2 çevrim. `{decl}` gövdeye eklenir.
fn two_stage(ports: &str, decl: &str) -> String {
    format!(
        "@strict_timing
module M {{
    in  clk : clock
    in  x   : u32
{ports}
    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    on clk {{
        a <= x
        b <= a
    }}
{decl}
}}
"
    )
}

#[test]
fn let_declared_with_a_wrong_count_is_e5010() {
    // #354, tasarım 3: karışım yok, `b` tam 2 çevrim; bildirim 1 diyor.
    // Bildirim yanlış olduğu için aşağıdaki `s + a` hizalı görünüyordu.
    let src = two_stage(
        "    out y   : u32",
        "    let s : Delayed<u32, 1> = b\n    y = s + a",
    );
    assert_eq!(e5010_count(&src), 1);
}

#[test]
fn let_declared_with_the_right_count_passes() {
    let src = two_stage(
        "    out y   : u32",
        "    let s : Delayed<u32, 2> = b\n    y = s + b",
    );
    assert_eq!(e5010_count(&src), 0);
}

#[test]
fn let_declared_through_delay_k_counts_k() {
    // delay<K> bugün yalnız sayıya K ekler (#232): 1 + 1 = 2.
    let src = two_stage(
        "    out y   : u32",
        "    let s : Delayed<u32, 2> = delay<1>(a)\n    y = s + b",
    );
    assert_eq!(e5010_count(&src), 0);
}

#[test]
fn wire_declared_with_a_wrong_count_is_e5010() {
    let src = two_stage(
        "    out y   : u32",
        "    wire w : Delayed<u32, 1>\n    w = b\n    y = w + a",
    );
    assert_eq!(e5010_count(&src), 1);
}

#[test]
fn wire_declared_with_the_right_count_passes() {
    let src = two_stage(
        "    out y   : u32",
        "    wire w : Delayed<u32, 2>\n    w = b\n    y = w + b",
    );
    assert_eq!(e5010_count(&src), 0);
}

#[test]
fn output_port_declared_with_a_wrong_count_is_e5010() {
    // #354: çıkış portunun bildirimi çıkarımla sessizce eziliyordu.
    let src = two_stage("    out y   : Delayed<u32, 5>", "    y = a");
    assert_eq!(e5010_count(&src), 1);
}

#[test]
fn output_port_declared_with_the_right_count_passes() {
    let src = two_stage("    out y   : Delayed<u32, 1>", "    y = a");
    assert_eq!(e5010_count(&src), 0);
}

#[test]
fn register_declaration_is_unchanged() {
    // Register denetimi aynı kalır: kaynak + 1 bildirimle uyuşmalı.
    let right = two_stage(
        "    out y   : u32",
        "    reg c : Delayed<u32, 2> = 0\n    on clk { c <= a }\n    y = c + b",
    );
    assert_eq!(e5010_count(&right), 0);
    let wrong = two_stage(
        "    out y   : u32",
        "    reg c : Delayed<u32, 3> = 0\n    on clk { c <= a }\n    y = c",
    );
    assert_eq!(e5010_count(&wrong), 1);
}

#[test]
fn a_declaration_over_a_mix_stays_a_retiming_assertion() {
    // Başlatıcı aşamaları karıştırıyor: gecikmesi tam bilinmez, bildirim
    // denetlenmez (v0.2.0'da ADR-0118'e göre kaçış). N hiçbir şeye
    // uymasa da (7) bugünkü gibi geçer.
    for n in [1, 2, 7] {
        let src = two_stage(
            "    out y   : u32",
            &format!("    let s : Delayed<u32, {n}> = a + b\n    y = s"),
        );
        assert_eq!(e5010_count(&src), 0, "N = {n}");
    }
    // İç içe karışım da karışımdır: (a + b) + b'nin gecikmesi 2 değildir.
    let nested = two_stage(
        "    out y   : u32",
        "    let s : Delayed<u32, 7> = (a + b) + b\n    y = s",
    );
    assert_eq!(e5010_count(&nested), 0);
    // Kaynakları farklı aşamalardan gelen wire da karışımdır.
    let wire = two_stage(
        "    in  sel : bool\n    out y   : u32",
        "    wire w : Delayed<u32, 2>\n    comb {\n        if sel { w = a } else { w = b }\n    }\n    y = w",
    );
    assert_eq!(e5010_count(&wire), 0);
}

#[test]
fn a_declaration_over_a_timeless_value_is_not_checked() {
    // Literal zamanlama taşımaz: her bildirimle uyumludur.
    let src = two_stage(
        "    out y   : u32",
        "    let s : Delayed<u32, 3> = 7\n    y = s",
    );
    assert_eq!(e5010_count(&src), 0);
}

#[test]
fn pipeline_stage_lets_are_not_checked_as_declarations() {
    // stage(...) okuyan aşama-yerel let'ler desugar'da sabitlenir
    // (ADR-0038 §3): kullanıcının yazdığı bir bildirim değildir.
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/ui/pass/46_pipeline_forwarding.volt"
    ))
    .expect("fixture");
    assert_eq!(e5010_count(&src), 0);
}

#[test]
fn output_port_declaration_is_not_a_retiming_assertion() {
    // Çıkış portunun bildirimi sınırdaki sözdür, ileri besleme kapısı
    // değildir: sürücüsündeki karışım bildirimden önce olduğu gibi E5010.
    let src = two_stage("    out y   : Delayed<u32, 2>", "    y = a + b");
    assert_eq!(e5010_count(&src), 1);
}

/// Bildirim uyuşmazlığının okura dönük metni: ileti, etiketler, help.
fn declared_mismatch_text(src: &str) -> String {
    let parsed = parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let diags: Vec<_> = analyze(&parsed.ast)
        .diagnostics
        .into_iter()
        .filter(|d| d.code.as_str() == "E5010")
        .collect();
    assert_eq!(diags.len(), 1, "{diags:?}");
    let d = &diags[0];
    assert_eq!(d.message, "declared delay does not match the value");
    let mut t = d.help.clone().unwrap_or_default();
    for s in &d.spans {
        t.push('\n');
        t.push_str(&s.label);
    }
    t
}

#[test]
fn the_declared_mismatch_text_names_both_counts_and_the_way_out() {
    // N bir fazla: bir register aşaması daha; tekil "1 cycle".
    let t = declared_mismatch_text(&two_stage("    out y   : Delayed<u32, 2>", "    y = a"));
    assert!(t.contains("declared as 2 cycles here"), "{t}");
    assert!(t.contains("this value is 1 cycle old"), "{t}");
    assert!(
        t.contains("write the count as 1; if the value must be 2 cycles old, pass it through one more register stage"),
        "{t}"
    );
    // N dört fazla: dört register aşaması.
    let t = declared_mismatch_text(&two_stage("    out y   : Delayed<u32, 5>", "    y = a"));
    assert!(t.contains("pass it through 4 more register stages"), "{t}");
    // N bir eksik: bir çevrim önceki aşama; tekil "1 cycle".
    let t = declared_mismatch_text(&two_stage(
        "    out y   : u32",
        "    let s : Delayed<u32, 1> = b\n    y = s",
    ));
    assert!(t.contains("declared as 1 cycle here"), "{t}");
    assert!(t.contains("this value is 2 cycles old"), "{t}");
    assert!(
        t.contains("write the count as 2; if the value must be 1 cycle old, take it from the stage one cycle earlier"),
        "{t}"
    );
    // N iki eksik: iki çevrim önceki aşama (c 3 çevrim).
    let t = declared_mismatch_text(&two_stage(
        "    out y   : u32",
        "    reg c : Delayed<u32, 3> = 0\n    on clk { c <= b }\n    let s : Delayed<u32, 1> = c\n    y = s",
    ));
    assert!(t.contains("take it from the stage 2 cycles earlier"), "{t}");
    assert!(!t.contains("delay<"), "{t}");
}

#[test]
fn an_unannotated_register_write_that_mixes_stages_is_e5010() {
    // Anotasyonsuz register'ın yazım kaynağı da karışım denetiminden geçer:
    // a (1) + b (2), bildirim olmasa da E5010.
    let src = two_stage(
        "    out y   : u32",
        "    reg t : u32 = 0\n    on clk { t <= a + b }\n    y = t",
    );
    assert_eq!(e5010_count(&src), 1);
}

#[test]
fn the_register_write_text_names_the_cycle_it_would_hold() {
    // c 3 çevrim bildirilmiş, kaynağı a (1): register 2 çevrimlik değer tutar.
    let src = two_stage(
        "    out y   : u32",
        "    reg c : Delayed<u32, 3> = 0\n    on clk { c <= a }\n    y = c",
    );
    let parsed = parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let labels: Vec<String> = analyze(&parsed.ast)
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E5010")
        .flat_map(|d| d.spans.iter().map(|s| s.label.clone()))
        .collect();
    assert!(
        labels
            .iter()
            .any(|l| l.ends_with("register would hold a 2-cycle value")),
        "{labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|l| l == "register declared as 3 cycles here"),
        "{labels:?}"
    );
}

#[test]
fn a_counter_read_back_through_its_output_port_is_unconstrained() {
    // Çıkış portunun gecikmesi sürücüsünden hesaplanır, giriş portu gibi 0
    // çevrimle başlamaz (ADR-0037): `count` ile `c` birbirini besleyen bir
    // döngüdür, sabit gecikmeye oturmaz ve serbest kalır; `count + step`
    // bu yüzden aşama karıştırmaz. 0 ile başlasaydı döngü her turda bir
    // çevrim büyür ve `next` sahte bir E5010 alırdı.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk   : clock
    in  step  : u8
    out count : u8
    out next  : u8

    reg c : u8 = 0

    on clk { c <= count + 1 }

    count = c
    next = count + step
}
"
        ),
        0
    );
}
