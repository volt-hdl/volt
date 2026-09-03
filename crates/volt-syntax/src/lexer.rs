//! Kaynak metni token akışına çeviren üst düzey lexer arayüzü.
//!
//! Tanılar `volt-diagnostics` modeliyle üretilir (5 parça kuralı).

use logos::Logos;
use volt_diagnostics::{Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::{FileId, Span};

use crate::token::{Token, TokenKind};

/// Lexer çıktısı: token akışı + toplanan tanılar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub errors: Vec<Diagnostic>,
}

/// Trivia'yı (boşluk, sıradan yorumlar) atlayarak tokenize eder.
/// Doc yorumları gramer öğesi olduğu için akışta kalır.
pub fn tokenize(file: FileId, source: &str) -> LexOutput {
    let full = tokenize_with_trivia(file, source);
    let tokens = full
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia())
        .collect();
    LexOutput {
        tokens,
        errors: full.errors,
    }
}

/// Tüm tokenları (boşluk ve yorumlar dahil) span'leriyle üretir.
pub fn tokenize_with_trivia(file: FileId, source: &str) -> LexOutput {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut lexer = TokenKind::lexer(source);

    while let Some(result) = lexer.next() {
        let range = lexer.span();
        let span = Span::new(file, range.start as u32, range.end as u32);
        let kind = match result {
            Ok(kind) => {
                if let Some(diag) = check_token(kind, span, lexer.slice()) {
                    errors.push(diag);
                }
                kind
            }
            Err(()) => {
                errors.push(Diagnostic::error(
                    ErrorCode::E0001,
                    format!("beklenmeyen karakter: {:?}", lexer.slice()),
                    LabeledSpan::primary(span, "bu karakter Volt sözlüğünde yok"),
                    "bu karakteri kaldırın; geçerli tokenlar için grammar-full.ebnf §15'e bakın",
                ));
                TokenKind::Error
            }
        };
        tokens.push(Token { kind, span });
    }

    LexOutput { tokens, errors }
}

/// Geçerli biçimde eşleşen ama tanı gerektiren tokenlar.
fn check_token(kind: TokenKind, span: Span, text: &str) -> Option<Diagnostic> {
    match kind {
        TokenKind::Reserved => Some(
            Diagnostic::error(
                ErrorCode::E0003,
                format!("'{text}' ayrılmış anahtar kelimedir, henüz desteklenmiyor"),
                LabeledSpan::primary(span, "ayrılmış kelime"),
                format!("farklı bir isim seçin (ör. '{text}_')"),
            )
            .with_note(
                NoteKind::Reason,
                "V1'de dil genişlediğinde bu isimler anlam kazanacak; şimdi kullanılırsa kod kırılır",
            ),
        ),
        TokenKind::InvalidNumber => Some(Diagnostic::error(
            ErrorCode::E0005,
            format!("geçersiz sayısal literal: '{text}'"),
            LabeledSpan::primary(span, "geçersiz literal"),
            "önekten sonra tabana uygun en az bir rakam gelmeli (ör. 0xFF, 0b1010, 0o755)",
        )),
        TokenKind::BlockComment(false) => Some(Diagnostic::error(
            ErrorCode::E0013,
            "kapanmamış blok yorumu",
            LabeledSpan::primary(span, "yorum burada başlıyor ama kapanmıyor"),
            "eksik '*/' ekleyin (iç içe yorumlarda her '/*' ayrıca kapanmalı)",
        )),
        _ => None,
    }
}
