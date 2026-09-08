//! volt-diagnostics::Diagnostic → lsp_types::Diagnostic dönüşümü.
//!
//! DİKKAT: LSP Position UTF-16 kod birimi sayar (bayt DEĞİL, karakter
//! DEĞİL) — dönüşüm volt-span'deki `line_col_utf16` üzerinden yapılır.

// LSP tipi takma adla anılır: tanı struct literali yalnız
// volt-diagnostics kurucusuna aittir (consistency kontrol 3).
use tower_lsp::lsp_types::Diagnostic as LspDiagnostic;
use tower_lsp::lsp_types::{
    CodeDescription, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
    Position, Range, Url,
};
use volt_diagnostics::{Diagnostic, Severity};
use volt_span::{SourceMap, Span};

/// Span → LSP Range (0-tabanlı satır, UTF-16 sütun).
pub fn span_to_range(map: &SourceMap, span: Span) -> Range {
    let (sl, sc) = map.line_col_utf16(span.file, span.start);
    let (el, ec) = map.line_col_utf16(span.file, span.end);
    Range {
        start: Position::new(sl, sc),
        end: Position::new(el, ec),
    }
}

/// 5 parça kuralındaki alanların LSP eşlemesi: kod → `code`, spec
/// referansı → `code_description` (volthdl.org/errors/EXXXX), ikincil
/// span'ler → `related_information`, neden/not/çözüm → mesaj kuyruğu.
pub fn to_lsp_diagnostic(diag: &Diagnostic, map: &SourceMap, uri: &Url) -> LspDiagnostic {
    let range = diag
        .primary_span()
        .map(|s| span_to_range(map, s.span))
        .unwrap_or_default();

    let severity = match diag.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Note => DiagnosticSeverity::INFORMATION,
    };

    let related: Vec<DiagnosticRelatedInformation> = diag
        .spans
        .iter()
        .filter(|s| !s.primary)
        .map(|s| DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: span_to_range(map, s.span),
            },
            message: s.label.clone(),
        })
        .collect();

    let mut message = diag.message.clone();
    if let Some(primary) = diag.primary_span() {
        if !primary.label.is_empty() && primary.label != diag.message {
            message.push_str(&format!("\n{}", primary.label));
        }
    }
    for note in &diag.notes {
        message.push_str(&format!("\n{}: {}", note.kind.as_str(), note.text));
    }
    if let Some(help) = &diag.help {
        message.push_str(&format!("\nhelp: {help}"));
    }

    LspDiagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(diag.code.as_str().to_string())),
        code_description: Url::parse(&diag.explain_url())
            .ok()
            .map(|href| CodeDescription { href }),
        source: Some("volt".to_string()),
        message,
        related_information: (!related.is_empty()).then_some(related),
        tags: None,
        data: None,
    }
}
