//! Kaynak metni token akışına çeviren üst düzey lexer arayüzü.
//!
//! Tanılar `volt-diagnostics` modeliyle üretilir (5 parça kuralı).

use logos::Logos;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
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
                    lstr!(en: "unexpected character: {:?}", lexer.slice(); tr: "beklenmeyen karakter: {:?}", lexer.slice()),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "this character is not part of the Volt vocabulary"; tr: "bu karakter Volt sözlüğünde yok"),
                    ),
                    lstr!(en: "remove this character; see grammar-full.ebnf §15 for valid tokens"; tr: "bu karakteri kaldırın; geçerli tokenlar için grammar-full.ebnf §15'e bakın"),
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
                lstr!(en: "'{text}' is a reserved keyword and is not supported yet"; tr: "'{text}' ayrılmış anahtar kelimedir, henüz desteklenmiyor"),
                LabeledSpan::primary(span, lstr!(en: "reserved word"; tr: "ayrılmış kelime")),
                lstr!(en: "choose a different name (e.g. '{text}_')"; tr: "farklı bir isim seçin (ör. '{text}_')"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "these names will gain meaning as the language grows in V1; using them now would break your code"; tr: "V1'de dil genişlediğinde bu isimler anlam kazanacak; şimdi kullanılırsa kod kırılır"),
            ),
        ),
        TokenKind::InvalidNumber => Some(Diagnostic::error(
            ErrorCode::E0005,
            lstr!(en: "invalid numeric literal: '{text}'"; tr: "geçersiz sayısal literal: '{text}'"),
            LabeledSpan::primary(span, lstr!(en: "invalid literal"; tr: "geçersiz literal")),
            lstr!(en: "the prefix must be followed by at least one digit valid in that base (e.g. 0xFF, 0b1010, 0o755)"; tr: "önekten sonra tabana uygun en az bir rakam gelmeli (ör. 0xFF, 0b1010, 0o755)"),
        )),
        TokenKind::BlockComment(false) => Some(Diagnostic::error(
            ErrorCode::E0013,
            lstr!(en: "unterminated block comment"; tr: "kapanmamış blok yorumu"),
            LabeledSpan::primary(
                span,
                lstr!(en: "comment starts here but is never closed"; tr: "yorum burada başlıyor ama kapanmıyor"),
            ),
            lstr!(en: "add the missing '*/' (in nested comments every '/*' must be closed separately)"; tr: "eksik '*/' ekleyin (iç içe yorumlarda her '/*' ayrıca kapanmalı)"),
        )),
        _ => None,
    }
}
