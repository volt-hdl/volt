//! Tanı veri modeli.
//!
//! CLAUDE.md 5 parça kuralı: her tanı kod + konum + açıklama + çözüm (help)
//! + spec referansı (`explain_url`) taşır. `validate()` bunu denetler.

use volt_span::Span;

use crate::code::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    /// cli-contract.md §5 JSON'daki değer.
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

/// Etiketli kaynak aralığı; `primary` olan hatanın asıl konumudur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabeledSpan {
    pub span: Span,
    pub label: String,
    pub primary: bool,
}

impl LabeledSpan {
    pub fn primary(span: Span, label: impl Into<String>) -> Self {
        Self {
            span,
            label: label.into(),
            primary: true,
        }
    }

    pub fn secondary(span: Span, label: impl Into<String>) -> Self {
        Self {
            span,
            label: label.into(),
            primary: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// İnsan çıktısında "= neden:" satırı.
    Reason,
    /// İnsan çıktısında "= not:" satırı.
    Note,
    /// İnsan çıktısında "= karşı örnek:" satırı (F4b, `volt verify`).
    Counterexample,
}

impl NoteKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NoteKind::Reason => "reason",
            NoteKind::Note => "note",
            NoteKind::Counterexample => "counterexample",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub kind: NoteKind,
    pub text: String,
}

/// cli-contract.md §5'teki applicability değerleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

impl Applicability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Applicability::MachineApplicable => "machine-applicable",
            Applicability::MaybeIncorrect => "maybe-incorrect",
            Applicability::HasPlaceholders => "has-placeholders",
            Applicability::Unspecified => "unspecified",
        }
    }
}

/// Otomatik uygulanabilir düzeltme önerisi (LSP quick-fix temeli).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub span: Span,
    pub replacement: String,
    pub applicability: Applicability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: ErrorCode,
    pub severity: Severity,
    pub message: String,
    /// Primary + secondary konumlar; en az bir primary zorunlu.
    pub spans: Vec<LabeledSpan>,
    /// "= neden:" / "= not:" satırları.
    pub notes: Vec<Note>,
    /// "= çözüm:" satırı — 5 parça kuralı gereği zorunlu.
    pub help: Option<String>,
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// Beş parçanın tamamını isteyen tek kurucu: kod, konum, açıklama,
    /// çözüm; spec referansı (`explain_url`) koddan türetilir.
    pub fn new(
        code: ErrorCode,
        severity: Severity,
        message: impl Into<String>,
        primary: LabeledSpan,
        help: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            spans: vec![LabeledSpan {
                primary: true,
                ..primary
            }],
            notes: Vec::new(),
            help: Some(help.into()),
            suggestions: Vec::new(),
        }
    }

    pub fn error(
        code: ErrorCode,
        message: impl Into<String>,
        primary: LabeledSpan,
        help: impl Into<String>,
    ) -> Self {
        Self::new(code, Severity::Error, message, primary, help)
    }

    pub fn warning(
        code: ErrorCode,
        message: impl Into<String>,
        primary: LabeledSpan,
        help: impl Into<String>,
    ) -> Self {
        Self::new(code, Severity::Warning, message, primary, help)
    }

    pub fn with_secondary(mut self, span: Span, label: impl Into<String>) -> Self {
        self.spans.push(LabeledSpan::secondary(span, label));
        self
    }

    pub fn with_note(mut self, kind: NoteKind, text: impl Into<String>) -> Self {
        self.notes.push(Note {
            kind,
            text: text.into(),
        });
        self
    }

    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Hatanın asıl konumu.
    pub fn primary_span(&self) -> Option<&LabeledSpan> {
        self.spans.iter().find(|s| s.primary)
    }

    pub fn explain_url(&self) -> String {
        self.code.explain_url()
    }

    /// 5 parça kuralı denetimi: kod (yapısal), konum, açıklama, çözüm,
    /// spec referansı (koddan türetildiği için yapısal).
    pub fn validate(&self) -> Result<(), String> {
        if self.message.trim().is_empty() {
            return Err(format!("{}: açıklama (message) boş", self.code.as_str()));
        }
        if self.primary_span().is_none() {
            return Err(format!("{}: primary konum (span) yok", self.code.as_str()));
        }
        match &self.help {
            Some(h) if !h.trim().is_empty() => Ok(()),
            _ => Err(format!(
                "{}: çözüm (help) eksik — 5 parça kuralı",
                self.code.as_str()
            )),
        }
    }
}
