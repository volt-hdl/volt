//! Quick fix (ADR-0091): "kesin" filtresi, aralık seçimi ve düzeltme
//! sonrası yeniden analiz — o tanı gider, yeni hata çıkmaz.

use volt_diagnostics::{Applicability, Diagnostic, ErrorCode, LabeledSpan, Severity, Suggestion};
use volt_lsp::analysis;
use volt_lsp::code_action::{apply, certain_fix, quick_fixes};
use volt_span::{FileId, Span};

const SEQ_EQ: &str = "\
module Count {
    in  clk : clock
    out q   : u8

    reg c : u8 = 0
    on clk {
        c = c + 1
    }
    q = c
}
";

fn suggestion(applicability: Applicability) -> Suggestion {
    Suggestion {
        span: Span::new(FileId(0), 0, 1),
        replacement: "x".into(),
        applicability,
    }
}

fn diag_with(suggestions: Vec<Suggestion>) -> Diagnostic {
    let span = Span::new(FileId(0), 0, 1);
    let mut d = Diagnostic::error(ErrorCode::E1001, "m", LabeledSpan::primary(span, "l"), "h");
    d.suggestions = suggestions;
    d
}

#[test]
fn only_a_single_machine_applicable_suggestion_is_certain() {
    use Applicability::*;
    assert!(certain_fix(&diag_with(vec![])).is_none());
    for uncertain in [MaybeIncorrect, HasPlaceholders, Unspecified] {
        assert!(certain_fix(&diag_with(vec![suggestion(uncertain)])).is_none());
    }
    assert!(certain_fix(&diag_with(vec![suggestion(MachineApplicable)])).is_some());
    // Kesin bir öneriyi kesin olmayan bir alternatif bozmaz.
    let mixed = diag_with(vec![
        suggestion(MaybeIncorrect),
        suggestion(MachineApplicable),
    ]);
    assert!(certain_fix(&mixed).is_some());
    // İki kesin seçenek = seçim: quick fix yok.
    let two = diag_with(vec![
        suggestion(MachineApplicable),
        suggestion(MachineApplicable),
    ]);
    assert!(certain_fix(&two).is_none());
}

#[test]
fn apply_splices_the_replacement() {
    let s = Suggestion {
        span: Span::new(FileId(0), 2, 3),
        replacement: "<=".into(),
        applicability: Applicability::MachineApplicable,
    };
    assert_eq!(apply("c = 1", &s), "c <= 1");
}

#[test]
fn quick_fix_is_offered_only_for_diagnostics_in_range() {
    let a = analysis::analyze("count.volt", SEQ_EQ);
    let eq = SEQ_EQ.find("c = c").unwrap() as u32 + 2;
    let fixes = quick_fixes(&a, eq, eq);
    assert_eq!(fixes.len(), 1);
    assert_eq!(fixes[0].diagnostic.code, ErrorCode::E0006);
    assert_eq!(fixes[0].suggestion.replacement, "<=");
    assert!(fixes[0].title.contains("E0006"), "{}", fixes[0].title);
    assert!(quick_fixes(&a, 0, 10).is_empty());
}

#[test]
fn applying_the_fix_removes_the_diagnostic_and_adds_no_error() {
    let a = analysis::analyze("count.volt", SEQ_EQ);
    let fix = &quick_fixes(&a, 0, SEQ_EQ.len() as u32)[0];
    let fixed = apply(SEQ_EQ, fix.suggestion);
    let b = analysis::analyze("count.volt", &fixed);
    assert!(
        b.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "{:?}",
        b.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>()
    );
}

#[test]
fn a_similar_name_guess_is_not_a_quick_fix() {
    let src = "module T {\n    in  data : u8\n    out y    : u8\n    y = dta\n}\n";
    let a = analysis::analyze("typo.volt", src);
    let e1001 = a
        .diagnostics
        .iter()
        .find(|d| d.code == ErrorCode::E1001)
        .expect("E1001");
    assert_eq!(e1001.suggestions.len(), 1, "veri tanıda kalır");
    assert!(quick_fixes(&a, 0, src.len() as u32).is_empty());
}

#[test]
fn a_fix_pointing_into_another_file_is_not_offered() {
    // Birim tanısı ana dosyaya taşınırken başka dosyadaki öneri düşer
    // (analysis::to_main_file); burada yapay tanıyla doğrudan denenir.
    let mut a = analysis::analyze("count.volt", SEQ_EQ);
    for d in &mut a.diagnostics {
        for s in &mut d.suggestions {
            s.span.file = FileId(7);
        }
    }
    assert!(quick_fixes(&a, 0, SEQ_EQ.len() as u32).is_empty());
}
