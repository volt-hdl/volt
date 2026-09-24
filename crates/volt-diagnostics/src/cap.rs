//! Tanı üst sınırı (ADR-0068 §4, W0023; ADR-0070 §3): katlama kök
//! nedeni çözer, bu bilinmeyen patlama sınıflarına karşı yedek
//! güvencedir. Sürücü (`--max-diagnostics`, varsayılan 1000) ve LSP
//! (sabit [`LSP_MAX_DIAGNOSTICS`]) aynı işlevi farklı sınır ve çözüm
//! metniyle çağırır.

use crate::{lstr, Diagnostic, ErrorCode, LabeledSpan, Severity};

/// Editörde belge başına gösterilen en çok tanı (ADR-0070 §3).
pub const LSP_MAX_DIAGNOSTICS: usize = 200;

/// Sınır aşılırsa hatalar önce (JSON zarfıyla aynı sıra), ilk `limit`
/// tanı kalır, sonuna gizlenen sayıyı ve `help` çözümünü söyleyen W0023
/// eklenir. `limit == 0` sınırsızdır.
pub fn cap_diagnostics(diagnostics: &mut Vec<Diagnostic>, limit: usize, help: &str) {
    if limit == 0 || diagnostics.len() <= limit {
        return;
    }
    let total = diagnostics.len();
    let mut ordered = std::mem::take(diagnostics);
    ordered.sort_by_key(|d| d.severity != Severity::Error);
    ordered.truncate(limit);
    let hidden = total - limit;
    let Some(anchor) = ordered
        .last()
        .and_then(|d| d.primary_span())
        .map(|s| s.span)
    else {
        *diagnostics = ordered;
        return;
    };
    ordered.push(Diagnostic::warning(
        ErrorCode::W0023,
        lstr!(
            en: "too many diagnostics: {limit} shown, {hidden} hidden";
            tr: "çok fazla tanı: {limit} gösterildi, {hidden} gizlendi"
        ),
        LabeledSpan::primary(
            anchor,
            lstr!(en: "last diagnostic shown"; tr: "gösterilen son tanı"),
        ),
        help,
    ));
    *diagnostics = ordered;
}
