//! Özdeş tanı katlama (ADR-0068): açılmış kopyalarda (for açılımı,
//! monomorfizasyon, bundle dizisi) aynı tanı (kod + span + mesaj +
//! etiket + çözüm; `Span.ctx` HARİÇ) bir kez kalır, katlanan kopyaların
//! birincil `ctx`'leri `folded_ctxs`'e yazılır.

use volt_diagnostics::{fold_duplicates, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::{FileId, Span};

fn diag(code: ErrorCode, msg: &str, ctx: u16) -> Diagnostic {
    Diagnostic::error(
        code,
        msg,
        LabeledSpan::primary(Span::new(FileId(0), 10, 20).with_ctx(ctx), "here"),
        "fix it",
    )
}

#[test]
fn identical_diagnostics_differing_only_in_ctx_fold_into_the_first() {
    let mut diags = vec![
        diag(ErrorCode::E2005, "same", 1),
        diag(ErrorCode::E2005, "same", 2),
        diag(ErrorCode::E2005, "same", 3),
    ];
    fold_duplicates(&mut diags, 0);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].primary_span().unwrap().span.ctx, 1);
    assert_eq!(diags[0].folded_ctxs, vec![2, 3]);
}

#[test]
fn different_message_code_or_span_is_not_folded() {
    let mut diags = vec![
        diag(ErrorCode::E2005, "a", 1),
        diag(ErrorCode::E2005, "b", 2),
        diag(ErrorCode::E2028, "a", 3),
        Diagnostic::error(
            ErrorCode::E2005,
            "a",
            LabeledSpan::primary(Span::new(FileId(0), 30, 40).with_ctx(4), "here"),
            "fix it",
        ),
    ];
    fold_duplicates(&mut diags, 0);
    assert_eq!(diags.len(), 4);
    assert!(diags.iter().all(|d| d.folded_ctxs.is_empty()));
}

#[test]
fn secondary_span_label_and_note_are_part_of_the_identity() {
    let base = diag(ErrorCode::E2005, "m", 0);
    let with_secondary =
        diag(ErrorCode::E2005, "m", 1).with_secondary(Span::new(FileId(0), 1, 2), "there");
    let with_note = diag(ErrorCode::E2005, "m", 2).with_note(volt_diagnostics::NoteKind::Note, "n");
    let mut diags = vec![base, with_secondary, with_note];
    fold_duplicates(&mut diags, 0);
    assert_eq!(diags.len(), 3);
}

#[test]
fn help_text_is_not_part_of_the_identity_first_copy_keeps_its_help() {
    // Açılım gövde adlarını yeniden adlandırır (`t` → `t_0`); çözüm metni
    // bu üretilmiş adı taşıyabilir. Aynı konum + mesaj → katlanır.
    let mut a = diag(ErrorCode::W2012, "type not specified", 1);
    a.help = Some("write let t_0 : i32".into());
    let mut b = diag(ErrorCode::W2012, "type not specified", 2);
    b.help = Some("write let t_1 : i32".into());
    let mut diags = vec![a, b];
    fold_duplicates(&mut diags, 0);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].help.as_deref(), Some("write let t_0 : i32"));
    assert_eq!(diags[0].folded_ctxs, vec![2]);
}

#[test]
fn only_the_tail_from_the_given_index_is_folded() {
    let mut diags = vec![
        diag(ErrorCode::E2005, "same", 1),
        diag(ErrorCode::E2005, "same", 2),
        diag(ErrorCode::E2005, "same", 3),
    ];
    fold_duplicates(&mut diags, 1);
    assert_eq!(diags.len(), 2, "ilk eleman kuyruğun dışında");
    assert_eq!(diags[1].folded_ctxs, vec![3]);
}

#[test]
fn folding_twice_accumulates_previously_folded_ctxs() {
    let mut a = diag(ErrorCode::E2005, "same", 1);
    a.folded_ctxs = vec![7, 8];
    let mut diags = vec![a, diag(ErrorCode::E2005, "same", 2)];
    fold_duplicates(&mut diags, 0);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].folded_ctxs, vec![7, 8, 2]);
}

#[test]
fn ctx_zero_identical_copies_fold_too_and_order_is_preserved() {
    let mut diags = vec![
        diag(ErrorCode::E2005, "x", 0),
        diag(ErrorCode::E1001, "y", 0),
        diag(ErrorCode::E2005, "x", 0),
        diag(ErrorCode::E1001, "y", 5),
    ];
    fold_duplicates(&mut diags, 0);
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, vec!["E2005", "E1001"]);
    assert_eq!(diags[0].folded_ctxs, vec![0]);
    assert_eq!(diags[1].folded_ctxs, vec![5]);
}
