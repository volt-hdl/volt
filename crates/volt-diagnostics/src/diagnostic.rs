//! Tanı veri modeli.
//!
//! CLAUDE.md 5 parça kuralı: her tanı kod + konum + açıklama + çözüm (help)
//! + spec referansı (`explain_url`) taşır. `validate()` bunu denetler.

use volt_span::{FileId, Span};

use crate::code::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// Bir düzeltmenin tek düzenlemesinin türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// `span`'ı metinle değiştirir (boş span: araya ekler).
    Replace,
    /// Metni, `span.start`'ın bulunduğu satırın ÜSTÜNE, o satırın
    /// girintisiyle yeni bir satır olarak ekler. Girinti tanı üretilirken
    /// bilinmez (HIR kaynak metni tutmaz); [`Edit::resolve`] kaynaktan
    /// çözer.
    LineAbove,
}

/// Düzeltmenin bir düzenlemesi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub span: Span,
    pub text: String,
    pub kind: EditKind,
}

impl Edit {
    /// Somut düzenleme: değiştirilecek aralık ve yeni metin. `source`,
    /// `span.file`'ın metnidir.
    pub fn resolve(&self, source: &str) -> (Span, String) {
        match self.kind {
            EditKind::Replace => (self.span, self.text.clone()),
            EditKind::LineAbove => {
                let at = (self.span.start as usize).min(source.len());
                let line_start = source[..at].rfind('\n').map_or(0, |i| i + 1);
                let indent: String = source[line_start..]
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();
                let start = line_start as u32;
                (
                    Span {
                        start,
                        end: start,
                        ..self.span
                    },
                    format!("{indent}{}\n", self.text),
                )
            }
        }
    }
}

/// Düzeltme önerisi (LSP quick fix temeli): birlikte uygulanan bir ya da
/// daha çok düzenleme. İlki birincildir (JSON `span`/`replacement`).
/// Her öneri bir gidiş-dönüş fikstürüyle sınanır (tests/suggestions/).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub edits: Vec<Edit>,
    pub applicability: Applicability,
}

impl Suggestion {
    /// `span`'ı `text` ile değiştiren öneri.
    pub fn replace(span: Span, text: impl Into<String>, applicability: Applicability) -> Self {
        Self {
            edits: vec![Edit {
                span,
                text: text.into(),
                kind: EditKind::Replace,
            }],
            applicability,
        }
    }

    /// `span`'ın satırının üstüne `text` satırını ekleyen öneri.
    pub fn line_above(span: Span, text: impl Into<String>, applicability: Applicability) -> Self {
        Self {
            edits: vec![Edit {
                span,
                text: text.into(),
                kind: EditKind::LineAbove,
            }],
            applicability,
        }
    }

    /// Birlikte uygulanacak bir değiştirme daha.
    pub fn and_replace(mut self, span: Span, text: impl Into<String>) -> Self {
        self.edits.push(Edit {
            span,
            text: text.into(),
            kind: EditKind::Replace,
        });
        self
    }

    /// Birlikte uygulanacak bir satır ekleme daha.
    pub fn and_line_above(mut self, span: Span, text: impl Into<String>) -> Self {
        self.edits.push(Edit {
            span,
            text: text.into(),
            kind: EditKind::LineAbove,
        });
        self
    }

    /// Birincil düzenleme.
    pub fn primary(&self) -> &Edit {
        &self.edits[0]
    }

    /// Somut düzenlemeler, öneri sırasıyla. `source` dosyanın metnini
    /// verir; metni olmayan dosyadaki düzenleme atlanır.
    pub fn resolve<'s>(&self, source: impl Fn(FileId) -> Option<&'s str>) -> Vec<(Span, String)> {
        self.edits
            .iter()
            .filter_map(|e| source(e.span.file).map(|src| e.resolve(src)))
            .collect()
    }
}

/// Somut düzenlemeleri tek dosyanın metnine uygular. Aynı konumdaki
/// eklemeler öneri sırasıyla yazılır; düzenlemeler çakışmamalıdır.
pub fn apply_edits(text: &str, edits: &[(Span, String)]) -> String {
    let mut order: Vec<usize> = (0..edits.len()).collect();
    // Sondan başa uygula; eşit başlangıçta sonraki önce (öneri sırası korunur).
    order.sort_by(|&a, &b| {
        (edits[b].0.start, edits[b].0.end, b).cmp(&(edits[a].0.start, edits[a].0.end, a))
    });
    let mut out = text.to_string();
    for i in order {
        let (span, new) = &edits[i];
        out.replace_range(span.start as usize..span.end as usize, new);
    }
    out
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
    /// Bu tanıya katlanmış özdeş kopyaların birincil `Span.ctx`'leri
    /// (ADR-0068): açılmış `for` yinelemeleri, generic örneklemeler ya da
    /// bundle dizisi elemanları aynı hatayı yeniden üretince kopya
    /// burada sayılır, bir kez raporlanır. Kimliğe DAHİL DEĞİLDİR.
    pub folded_ctxs: Vec<u16>,
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
            folded_ctxs: Vec::new(),
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

    pub fn explain_url(&self) -> Option<String> {
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
