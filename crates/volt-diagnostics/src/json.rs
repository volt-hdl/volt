//! JSON çıktı — cli-contract.md §5 "JSON Çıktısı" şemasıyla birebir.
//!
//! Alan adları şemadan değiştirilemez; tests/diagnostics_tests.rs doğrular.

use serde::Serialize;
use volt_span::{SourceMap, Span};

use crate::diagnostic::Diagnostic;

#[derive(Serialize)]
struct JsonPos {
    line: u32,
    col: u32,
    byte: u32,
}

#[derive(Serialize)]
struct JsonLabeledSpan {
    file: String,
    start: JsonPos,
    end: JsonPos,
    label: String,
    primary: bool,
}

#[derive(Serialize)]
struct JsonSpan {
    file: String,
    start: JsonPos,
    end: JsonPos,
}

#[derive(Serialize)]
struct JsonNote {
    kind: &'static str,
    text: String,
}

#[derive(Serialize)]
struct JsonSuggestion {
    span: JsonSpan,
    replacement: String,
    applicability: &'static str,
}

#[derive(Serialize)]
struct JsonDiagnostic {
    code: &'static str,
    severity: &'static str,
    message: String,
    spans: Vec<JsonLabeledSpan>,
    notes: Vec<JsonNote>,
    help: Option<String>,
    suggestions: Vec<JsonSuggestion>,
    explain_url: String,
}

fn json_pos(map: &SourceMap, span: Span, byte: u32) -> JsonPos {
    let (line, col) = map.line_col_at(span.file, byte);
    JsonPos { line, col, byte }
}

fn json_span(map: &SourceMap, span: Span) -> JsonSpan {
    JsonSpan {
        file: map.path(span.file).display().to_string(),
        start: json_pos(map, span, span.start),
        end: json_pos(map, span, span.end),
    }
}

/// Tek tanıyı şemaya uygun `serde_json::Value`'ya çevirir.
pub fn to_json_value(diag: &Diagnostic, map: &SourceMap) -> serde_json::Value {
    let json = JsonDiagnostic {
        code: diag.code.as_str(),
        severity: diag.severity.as_str(),
        message: diag.message.clone(),
        spans: diag
            .spans
            .iter()
            .map(|ls| JsonLabeledSpan {
                file: map.path(ls.span.file).display().to_string(),
                start: json_pos(map, ls.span, ls.span.start),
                end: json_pos(map, ls.span, ls.span.end),
                label: ls.label.clone(),
                primary: ls.primary,
            })
            .collect(),
        notes: diag
            .notes
            .iter()
            .map(|n| JsonNote {
                kind: n.kind.as_str(),
                text: n.text.clone(),
            })
            .collect(),
        help: diag.help.clone(),
        suggestions: diag
            .suggestions
            .iter()
            .map(|s| JsonSuggestion {
                span: json_span(map, s.span),
                replacement: s.replacement.clone(),
                applicability: s.applicability.as_str(),
            })
            .collect(),
        explain_url: diag.explain_url(),
    };
    serde_json::to_value(json).expect("tanı JSON'a çevrilemedi")
}

/// Tek tanıyı JSON metnine çevirir.
pub fn to_json_string(diag: &Diagnostic, map: &SourceMap) -> String {
    serde_json::to_string_pretty(&to_json_value(diag, map)).expect("tanı JSON'a çevrilemedi")
}
