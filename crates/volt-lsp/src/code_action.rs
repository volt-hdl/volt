//! Quick fix (ADR-0091): tanının yapısal `suggestions` alanından —
//! `volt check --format=json`'daki aynı veri; metin ayrıştırması yok.
//!
//! "Kesin" filtresi: yalnız `machine-applicable` işaretli ve tanı başına
//! TEK olan öneri quick fix olur. `maybe-incorrect` (benzer ad tahmini),
//! `has-placeholders` (şablon) ve birden çok seçenek (sync / AsyncFifo)
//! yalnız tanı metninde kalır.

use volt_diagnostics::{lstr, Applicability, Diagnostic, Suggestion};

use crate::analysis::Analysis;

/// Uygulanabilir tek düzeltme: hangi tanıya ait, ne değişir.
pub struct QuickFix<'a> {
    pub diagnostic: &'a Diagnostic,
    pub suggestion: &'a Suggestion,
    pub title: String,
}

/// Tanının kesin düzeltmesi (yoksa `None`).
pub fn certain_fix(diag: &Diagnostic) -> Option<&Suggestion> {
    let mut fixes = diag
        .suggestions
        .iter()
        .filter(|s| s.applicability == Applicability::MachineApplicable);
    let only = fixes.next()?;
    fixes.next().is_none().then_some(only)
}

/// Birincil span'i `[start, end]` bayt aralığına değen tanıların quick
/// fix'leri (yayımlanan tanılar = `volt check` yolu).
pub fn quick_fixes(analysis: &Analysis, start: u32, end: u32) -> Vec<QuickFix<'_>> {
    analysis
        .diagnostics
        .iter()
        .filter(|d| {
            d.primary_span()
                .is_some_and(|p| p.span.start <= end && start <= p.span.end)
        })
        .filter_map(|d| {
            let s = certain_fix(d)?;
            (s.span.file == analysis.file_id).then(|| QuickFix {
                diagnostic: d,
                suggestion: s,
                title: title(d, s),
            })
        })
        .collect()
}

fn title(diag: &Diagnostic, s: &Suggestion) -> String {
    let code = diag.code.as_str();
    let r = &s.replacement;
    if s.span.start == s.span.end {
        lstr!(en: "{code}: insert '{r}'"; tr: "{code}: '{r}' ekle")
    } else if r.is_empty() {
        lstr!(en: "{code}: remove"; tr: "{code}: kaldır")
    } else {
        lstr!(en: "{code}: replace with '{r}'"; tr: "{code}: '{r}' ile değiştir")
    }
}

/// Düzeltmeyi metne uygular (testler ve istemcisiz araçlar için).
pub fn apply(text: &str, s: &Suggestion) -> String {
    let (a, b) = (s.span.start as usize, s.span.end as usize);
    format!("{}{}{}", &text[..a], s.replacement, &text[b..])
}
