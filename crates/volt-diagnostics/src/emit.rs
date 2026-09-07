//! İnsan-okunabilir ve short format çıktı (cli-contract.md §5).

use std::ops::Range;

use codespan_reporting::diagnostic as cs;
use codespan_reporting::files::{Error as FilesError, Files};
use codespan_reporting::term;
use codespan_reporting::term::termcolor::NoColor;
use volt_span::{FileId, SourceMap};

use crate::diagnostic::{Diagnostic, NoteKind, Severity};

/// SourceMap'i codespan-reporting'in Files arayüzüne uyarlar.
struct MapFiles<'a>(&'a SourceMap);

impl<'a> Files<'a> for MapFiles<'a> {
    type FileId = FileId;
    type Name = String;
    type Source = &'a str;

    fn name(&'a self, id: FileId) -> Result<String, FilesError> {
        Ok(self.0.path(id).display().to_string())
    }

    fn source(&'a self, id: FileId) -> Result<&'a str, FilesError> {
        Ok(self.0.source(id))
    }

    fn line_index(&'a self, id: FileId, byte_index: usize) -> Result<usize, FilesError> {
        Ok(self.0.line_index(id, byte_index as u32))
    }

    fn line_range(&'a self, id: FileId, line_index: usize) -> Result<Range<usize>, FilesError> {
        Ok(self.0.line_range(id, line_index))
    }
}

fn to_cs_severity(severity: Severity) -> cs::Severity {
    match severity {
        Severity::Error => cs::Severity::Error,
        Severity::Warning => cs::Severity::Warning,
        Severity::Note => cs::Severity::Note,
    }
}

/// cli-contract.md §5 "İnsan Çıktısı" formatında (renksiz) metin üretir.
/// Anahtar satırlar aktif dile göre seçilir (GLOSSARY.md §7):
/// EN "= reason: / = help: / = note: / = for more:",
/// TR "= neden: / = çözüm: / = not: / = daha fazla:".
pub fn render_human(diag: &Diagnostic, map: &SourceMap) -> String {
    let labels = diag
        .spans
        .iter()
        .map(|ls| {
            let style = if ls.primary {
                cs::LabelStyle::Primary
            } else {
                cs::LabelStyle::Secondary
            };
            cs::Label::new(
                style,
                ls.span.file,
                ls.span.start as usize..ls.span.end as usize,
            )
            .with_message(ls.label.clone())
        })
        .collect();

    let keys = crate::messages::keys(crate::messages::lang());
    let mut notes: Vec<String> = diag
        .notes
        .iter()
        .map(|n| match n.kind {
            NoteKind::Reason => format!("{} {}", keys.reason, n.text),
            NoteKind::Note => format!("{} {}", keys.note, n.text),
            NoteKind::Counterexample => format!("{} {}", keys.counterexample, n.text),
        })
        .collect();
    if let Some(help) = &diag.help {
        notes.push(format!("{} {help}", keys.help));
    }
    notes.push(format!(
        "{} volt explain {}",
        keys.for_more,
        diag.code.as_str()
    ));

    let cs_diag = cs::Diagnostic::new(to_cs_severity(diag.severity))
        .with_code(diag.code.as_str())
        .with_message(&diag.message)
        .with_labels(labels)
        .with_notes(notes);

    let mut buffer = NoColor::new(Vec::new());
    let config = term::Config::default();
    term::emit(&mut buffer, &config, &MapFiles(map), &cs_diag).expect("tanı çıktısı üretilemedi");
    String::from_utf8(buffer.into_inner()).expect("tanı çıktısı UTF-8 olmalı")
}

/// CI logları için tek satır: `dosya:satır:sütun: severity[KOD]: mesaj`.
pub fn render_short(diag: &Diagnostic, map: &SourceMap) -> String {
    let primary = diag
        .primary_span()
        .or_else(|| diag.spans.first())
        .expect("short format için en az bir span gerekli");
    let (line, col) = map.line_col(primary.span);
    format!(
        "{}:{}:{}: {}[{}]: {}",
        map.path(primary.span.file).display(),
        line,
        col,
        diag.severity.as_str(),
        diag.code.as_str(),
        diag.message
    )
}
