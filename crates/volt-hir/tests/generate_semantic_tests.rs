//! Modül seviyesi `for` açılımı (ADR-0056) — anlamsal geçit ve tanı
//! bağlamı: açılmış yinelemeye düşen tanı kaynak satırını gösterir ve
//! "for i = k yinelemesinde" notu taşır.

use volt_diagnostics::NoteKind;
use volt_hir::analyze;
use volt_span::SourceMap;
use volt_syntax::{parse, FileId};

fn analyze_src(src: &str) -> volt_hir::AnalysisResult {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

fn analyze_file(rel: &str) -> volt_hir::AnalysisResult {
    let path = format!("{}/../../tests/ui/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect("ui dosyası okunmalı");
    analyze_src(&src)
}

const PE: &str = "module Pe { in clk : clock\n in a : u8\n out c : u8\n reg r : u8 = 0\n on clk { r <= a }\n c = r }\n";

#[test]
fn ui_pass_75_for_instantiation_clean() {
    let r = analyze_file("pass/75_for_instantiation.volt");
    assert!(!r.has_errors(), "{:?}", r.error_codes());
}

#[test]
fn ui_pass_76_nested_for_instantiation_clean() {
    let r = analyze_file("pass/76_nested_for_instantiation.volt");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
}

#[test]
fn ui_pass_77_bundle_array_clean() {
    let r = analyze_file("pass/77_bundle_array.volt");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
}

#[test]
fn unrolled_instances_resolve_and_typecheck_like_hand_written_ones() {
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in bus : [u8; 4]\n out res : [u8; 4]\n for i in 0..4 {{ let pe = Pe {{ clk: clk, a: bus[i] }}\n res[i] = pe.c }} }}"
    );
    let r = analyze_src(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
}

#[test]
fn error_inside_iteration_points_at_source_line_with_iteration_note() {
    // 3. yinelemede tip hatası: `a: bus[i]` yerine bool bağlanıyor.
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in flag : bool\n out res : [u8; 3]\n for i in 0..3 {{\n let pe = Pe {{ clk: clk, a: flag }}\n res[i] = pe.c }} }}"
    );
    let parsed = parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty());
    let r = analyze(&parsed.ast);
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2003")
        .collect();
    // ADR-0068: üç yinelemede AYNI tanı → bir kez, katlanan kopyalar notta.
    assert_eq!(
        errs.len(),
        1,
        "özdeş tanı bir kez raporlanır: {:?}",
        r.error_codes()
    );
    let d = errs[0];
    assert_eq!(d.folded_ctxs.len(), 2);
    let mut map = SourceMap::new();
    map.add_file("t.volt", src.clone());
    let let_line = src.lines().position(|l| l.contains("let pe = Pe")).unwrap() as u32 + 1;
    let (line, _) = map.line_col_utf8(d.primary_span().unwrap().span);
    assert_eq!(line, let_line, "kaynak satırı gösterilmeli");
    let note = d
        .notes
        .iter()
        .find(|n| n.kind == NoteKind::Note && n.text.contains("i = "))
        .unwrap_or_else(|| panic!("yineleme notu yok: {:?}", d.notes));
    assert!(
        note.text
            .contains("occurs in 3 unrolled 'for' iterations (i = 0..2)"),
        "{}",
        note.text
    );
    // İkincil etiket `for` deyimini gösterir.
    assert!(d.spans.iter().any(|s| !s.primary));
}

#[test]
fn single_iteration_diagnostic_keeps_the_per_iteration_note() {
    // Yalnız i = 2'de hata (o yinelemede sabit indeks taşar): katlama
    // yok, eski "i = 2 yinelemesinde" notu kalır.
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in a : u8\n out res : [u8; 2]\n for i in 0..3 {{ let pe = Pe {{ clk: clk, a: a }}\n res[i] = pe.c }} }}"
    );
    let r = analyze_src(&src);
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.severity == volt_diagnostics::Severity::Error)
        .unwrap_or_else(|| panic!("taşma hatası bekleniyor: {:?}", r.error_codes()));
    assert!(d.folded_ctxs.is_empty(), "{d:?}");
    let note = d
        .notes
        .iter()
        .find(|n| n.text.contains("i = 2"))
        .unwrap_or_else(|| panic!("yineleme notu yok: {:?}", d.notes));
    assert!(
        note.text.contains("in the unrolled 'for' iteration i = 2"),
        "{}",
        note.text
    );
}

#[test]
fn nested_iteration_note_lists_both_variables() {
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in flag : bool\n out res : [u8; 4]\n for y in 0..2 {{ for x in 0..2 {{ let pe = Pe {{ clk: clk, a: flag }}\n res[y * 2 + x] = pe.c }} }} }}"
    );
    let r = analyze_src(&src);
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2003")
        .collect();
    assert_eq!(errs.len(), 1, "{:?}", r.error_codes());
    let note = errs[0]
        .notes
        .iter()
        .find(|n| n.text.contains("y = 0..1, x = 0..1"))
        .unwrap_or_else(|| panic!("iç içe not yok: {:?}", errs[0].notes));
    assert!(
        note.text.contains("4 unrolled 'for' iterations"),
        "{}",
        note.text
    );
}

#[test]
fn generic_instantiations_of_a_faulty_module_fold_into_one_diagnostic() {
    // ADR-0041 mono: aynı hatalı şablon 6 farklı argümanla → 6 klon,
    // gövdedeki hata 6 kez üretilir; tek tanı + "6 generic instantiations".
    let mut src = String::from(
        "module W<const K : u32> { in a : u8\n out o : i8\n o = a }\nmodule Top { in a : u8\n out o : i8\n",
    );
    for k in 1..=6 {
        src.push_str(&format!(" let w{k} = W<{k}> {{ a: a }}\n"));
    }
    src.push_str(" o = w1.o }");
    let r = analyze_src(&src);
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2002")
        .collect();
    assert_eq!(errs.len(), 1, "{:?}", r.error_codes());
    assert_eq!(errs[0].folded_ctxs.len(), 5);
    let note = errs[0]
        .notes
        .iter()
        .find(|n| n.text.contains("6 generic instantiations"))
        .unwrap_or_else(|| panic!("örnekleme notu yok: {:?}", errs[0].notes));
    assert!(note.text.contains("reported once"), "{}", note.text);
}

#[test]
fn for_over_generic_arguments_folds_iterations_and_instantiations() {
    // `W<i>` 8 yinelemede 8 farklı monomorf; W'nin gövdesindeki hata
    // her klonda: tek tanı, "8 generic instantiations".
    let src = "const N : u32 = 8\nmodule W<const K : u32> { in a : u8\n out o : i8\n o = a }\nmodule Top { in a : u8\n out os : [i8; N]\n for i in 0..N { let w = W<i> { a: a }\n os[i] = w.o } }";
    let r = analyze_src(src);
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2002")
        .collect();
    assert_eq!(errs.len(), 1, "{:?}", r.error_codes());
    assert_eq!(errs[0].folded_ctxs.len(), 7);
    assert!(
        errs[0]
            .notes
            .iter()
            .any(|n| n.text.contains("8 generic instantiations")),
        "{:?}",
        errs[0].notes
    );
}

#[test]
fn bundle_array_elements_fold_the_same_resolution_error() {
    // `[Bus; 16]` düzleştirmesi alan tipini her eleman için çözer (ctx 0):
    // bilinmeyen tip tek E1001 + "identical occurrences" notu.
    let src = "struct port Bus { in a : u8\n in b : Nope\n out c : u8 }\nmodule M { in x : [Bus; 16]\n out y : u8\n y = x[0].a }";
    let r = analyze_src(src);
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E1001")
        .collect();
    assert_eq!(errs.len(), 1, "{:?}", r.error_codes());
    assert!(
        errs[0].folded_ctxs.len() >= 15,
        "{}",
        errs[0].folded_ctxs.len()
    );
    assert!(
        errs[0]
            .notes
            .iter()
            .any(|n| n.text.contains("identical occurrences")),
        "{:?}",
        errs[0].notes
    );
}

#[test]
fn iteration_specific_errors_stay_separate_after_folding() {
    // res uzunluğu 2, döngü 0..4: i = 2 ve i = 3'te farklı mesajlı taşma
    // hataları — iki ayrı tanı, her biri kendi yineleme notuyla.
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in a : u8\n out res : [u8; 2]\n for i in 0..4 {{ let pe = Pe {{ clk: clk, a: a }}\n res[i] = pe.c }} }}"
    );
    let r = analyze_src(&src);
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.severity == volt_diagnostics::Severity::Error)
        .collect();
    assert_eq!(errs.len(), 2, "{:?}", r.error_codes());
    assert!(errs.iter().all(|d| d.folded_ctxs.is_empty()));
    let notes: Vec<String> = errs
        .iter()
        .flat_map(|d| d.notes.iter().map(|n| n.text.clone()))
        .collect();
    assert!(notes.iter().any(|n| n.contains("i = 2")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("i = 3")), "{notes:?}");
}

#[test]
fn diagnostics_outside_loops_carry_no_iteration_note() {
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in flag : bool\n out res : [u8; 2]\n out z : u8\n for i in 0..2 {{ let pe = Pe {{ clk: clk, a: 0 }}\n res[i] = pe.c }}\n z = flag }}"
    );
    let r = analyze_src(&src);
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E2003")
        .expect("z = flag tip hatası");
    assert!(d
        .notes
        .iter()
        .all(|n| !n.text.contains("yineleme") && !n.text.contains("iteration")));
}

#[test]
fn annotate_generate_is_idempotent() {
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in flag : bool\n out res : [u8; 2]\n for i in 0..2 {{ let pe = Pe {{ clk: clk, a: flag }}\n res[i] = pe.c }} }}"
    );
    let parsed = parse(FileId(0), &src);
    let r = analyze(&parsed.ast);
    let twice = volt_hir::annotate_generate(&parsed.ast, r.diagnostics.clone());
    for (a, b) in r.diagnostics.iter().zip(&twice) {
        assert_eq!(a.notes.len(), b.notes.len());
        assert_eq!(a.spans.len(), b.spans.len());
    }
}

#[test]
fn instance_name_from_loop_is_not_visible_after_the_loop() {
    // Döngü kapsamı: `pe` yalnız yineleme içinde; dışarıda E1001.
    let src = format!(
        "{PE}module Top {{ in clk : clock\n in a : u8\n out res : [u8; 2]\n out z : u8\n for i in 0..2 {{ let pe = Pe {{ clk: clk, a: a }}\n res[i] = pe.c }}\n z = pe.c }}"
    );
    let r = analyze_src(&src);
    assert!(r.error_codes().contains(&"E1001"), "{:?}", r.error_codes());
}

#[test]
fn array_ports_and_wires_typecheck_with_element_access() {
    let r = analyze_src("module M { in a : [i8; 4]\n out c : [i16; 4]\n wire w : [i16; 4]\n for i in 0..4 { w[i] = a[i] as i16\n c[i] = w[i] } }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
}
