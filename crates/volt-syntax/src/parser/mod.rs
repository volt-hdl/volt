//! Volt parser'ı — F0 kapsamı.
//!
//! Elden yazılmış LL(2) iniş parser'ı + Pratt ifade parser'ı.
//! İlkeler (error-recovery.md §1): asla panik yok, her girdide AST,
//! her kurtarma en az bir token tüketir, kaskadlar bastırılır.

mod expr;
mod item;
mod pattern;
pub(crate) mod recovery;
mod stmt;
mod test;

use std::collections::HashSet;

use volt_ast::{Expr, ExprKind, Idx, Pattern, PatternKind, SourceFile, TypeRef, TypeRefKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::{FileId, Span};

use crate::lexer::tokenize;
use crate::token::{Token, TokenKind};
use recovery::same_kind;

/// Ayrıştırma sonucu: AST + tüm tanılar (lexer + parser).
#[derive(Debug)]
pub struct ParseResult {
    pub ast: SourceFile,
    pub diagnostics: Vec<Diagnostic>,
}

impl ParseResult {
    pub fn error_codes(&self) -> Vec<&'static str> {
        self.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }
}

/// Kaynak dosyayı tam AST'ye ayrıştırır. Hiçbir girdide panik etmez.
pub fn parse(file: FileId, source: &str) -> ParseResult {
    let mut parser = Parser::new(file, source);
    parser.parse_source_file();
    parser.finish()
}

/// Tek bir ifadeyi ayrıştırır (öncelik testleri için).
pub fn parse_expr(file: FileId, source: &str) -> (ParseResult, Idx<Expr>) {
    let mut parser = Parser::new(file, source);
    let root = parser.parse_expr();
    (parser.finish(), root)
}

/// İfade/blok iç içeliği sınırı — patolojik girdide yığın taşmasını önler.
const MAX_DEPTH: u32 = 200;

/// Kaskad bastırma penceresi (token sayısı, error-recovery.md §5).
const SUPPRESS_WINDOW: usize = 2;

pub(crate) struct Parser<'s> {
    source: &'s str,
    file: FileId,
    tokens: Vec<Token>,
    pos: usize,
    pub(crate) ast: SourceFile,
    diagnostics: Vec<Diagnostic>,
    last_error_pos: Option<usize>,
    pub(crate) depth: u32,
    eof_span: Span,
    /// StructLit'in yasak olduğu bağlamlar (if/match/for başlık ifadeleri):
    /// `if x { }` içindeki '{' blok başlangıcıdır, yapı literali değil.
    pub(crate) allow_struct_lit: bool,
    /// Parantezle sarılmış ifadeler — W0010 parantez önerisi bunları atlar.
    pub(crate) paren_exprs: HashSet<Idx<Expr>>,
}

impl<'s> Parser<'s> {
    pub(crate) fn new(file: FileId, source: &'s str) -> Self {
        let lexed = tokenize(file, source);
        // Lexer'ın Error tokenları zaten E0001 üretti; parser akışından
        // çıkarılır ki kaskad hata oluşmasın. InnerDoc F0'da kullanılmıyor.
        let tokens = lexed
            .tokens
            .into_iter()
            .filter(|t| !matches!(t.kind, TokenKind::Error | TokenKind::InnerDocComment))
            .collect();
        let len = source.len() as u32;
        Self {
            source,
            file,
            tokens,
            pos: 0,
            ast: SourceFile::default(),
            diagnostics: lexed.errors,
            last_error_pos: None,
            depth: 0,
            eof_span: Span::new(file, len, len),
            allow_struct_lit: true,
            paren_exprs: HashSet::new(),
        }
    }

    pub(crate) fn finish(self) -> ParseResult {
        ParseResult {
            ast: self.ast,
            diagnostics: self.diagnostics,
        }
    }

    // ─── Token imleci ───

    pub(crate) fn current(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos).map(|t| t.kind)
    }

    pub(crate) fn peek(&self, n: usize) -> Option<TokenKind> {
        self.tokens.get(self.pos + n).map(|t| t.kind)
    }

    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.current().is_some_and(|c| same_kind(c, kind))
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    pub(crate) fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or(self.eof_span)
    }

    pub(crate) fn text_of(&self, span: Span) -> &'s str {
        &self.source[span.start as usize..span.end as usize]
    }

    pub(crate) fn current_text(&self) -> &'s str {
        if self.at_eof() {
            ""
        } else {
            self.text_of(self.current_span())
        }
    }

    pub(crate) fn bump_any(&mut self) {
        if !self.at_eof() {
            self.pos += 1;
        }
    }

    /// Mevcut tokenı tüketir ve span'ini döndürür.
    pub(crate) fn bump(&mut self) -> Span {
        let span = self.current_span();
        self.bump_any();
        span
    }

    pub(crate) fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Üretim başlangıcından şimdiye kadar uzanan span.
    pub(crate) fn span_from(&self, start_tok: usize) -> Span {
        let start = self
            .tokens
            .get(start_tok)
            .map(|t| t.span.start)
            .unwrap_or(self.eof_span.start);
        let end = if self.pos > start_tok {
            self.tokens
                .get(self.pos - 1)
                .map(|t| t.span.end)
                .unwrap_or(self.eof_span.end)
        } else {
            start
        };
        Span::new(self.file, start, end)
    }

    // ─── Tanılar ───

    /// Kaskad bastırma penceresiyle hata ekler (error-recovery.md §5).
    pub(crate) fn push_error(&mut self, diag: Diagnostic) {
        if let Some(last) = self.last_error_pos {
            if self.pos.saturating_sub(last) < SUPPRESS_WINDOW {
                return;
            }
        }
        self.last_error_pos = Some(self.pos);
        self.diagnostics.push(diag);
    }

    /// Beklenen token yoksa E0001/E0011 üretir. Token tüketmez.
    pub(crate) fn expect(&mut self, kind: TokenKind, what: &str, help: &str) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.error_expected(what, help);
        false
    }

    pub(crate) fn error_expected(&mut self, what: &str, help: &str) {
        let (code, message) = if self.at_eof() {
            (
                ErrorCode::E0011,
                lstr!(en: "unexpected end of file: expected {what}"; tr: "beklenmeyen dosya sonu: {what} bekleniyor"),
            )
        } else {
            (
                ErrorCode::E0001,
                lstr!(en: "unexpected '{}': expected {what}", self.current_text(); tr: "beklenmeyen '{}': {what} bekleniyor", self.current_text()),
            )
        };
        let diag = Diagnostic::error(
            code,
            message,
            LabeledSpan::primary(
                self.current_span(),
                lstr!(en: "expected {what}"; tr: "{what} bekleniyor"),
            ),
            help,
        );
        self.push_error(diag);
    }

    /// Kapanış tokenı bekler; eksikse E0002 + açılış konumu (error-recovery.md §6.4).
    pub(crate) fn expect_closing(&mut self, kind: TokenKind, symbol: &str, open_span: Span) {
        if self.eat(kind) {
            return;
        }
        let diag = Diagnostic::error(
            ErrorCode::E0002,
            lstr!(en: "missing closing delimiter: '{symbol}'"; tr: "eksik kapanış: '{symbol}'"),
            LabeledSpan::primary(
                self.current_span(),
                lstr!(en: "expected '{symbol}'"; tr: "'{symbol}' bekleniyordu"),
            ),
            lstr!(en: "add '{symbol}'"; tr: "'{symbol}' ekleyin"),
        )
        .with_secondary(
            open_span,
            lstr!(en: "opening delimiter here"; tr: "açılış burada"),
        );
        self.push_error(diag);
    }

    // ─── AST yardımcıları ───

    pub(crate) fn alloc_error_expr(&mut self, span: Span) -> Idx<Expr> {
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::Error,
        })
    }

    pub(crate) fn alloc_error_type(&mut self, span: Span) -> Idx<TypeRef> {
        self.ast.types.alloc(TypeRef {
            span,
            kind: TypeRefKind::Error,
        })
    }

    pub(crate) fn alloc_error_pattern(&mut self, span: Span) -> Idx<Pattern> {
        self.ast.patterns.alloc(Pattern {
            span,
            kind: PatternKind::Error,
        })
    }

    /// Başlık konumundaki ifadeyi StructLit yasağıyla ayrıştırır
    /// (`if cond {`, `match x {`, `for i in a..b {`).
    pub(crate) fn parse_expr_no_struct_lit(&mut self) -> Idx<Expr> {
        let prev = self.allow_struct_lit;
        self.allow_struct_lit = false;
        let expr = self.parse_expr();
        self.allow_struct_lit = prev;
        expr
    }
}
