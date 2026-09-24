//! Volt parser'ı — F0 kapsamı.
//!
//! Elden yazılmış LL(2) iniş parser'ı + Pratt ifade parser'ı.
//! İlkeler (error-recovery.md §1): asla panik yok, her girdide AST,
//! her kurtarma en az bir token tüketir, kaskadlar bastırılır.

mod auto_contract;
mod bidir;
mod bundle;
mod desugar;
mod expr;
mod handshake;
pub(crate) mod item;
mod mmio;
pub(crate) mod mono;
mod pattern;
mod pipeline;
pub(crate) mod recovery;
mod stmt;
mod struct_lit;
mod test;
mod test_expr;
mod type_graph;

use std::collections::HashSet;

use volt_ast::mmio::RegMap;
use volt_ast::{Expr, ExprKind, Idx, Pattern, PatternKind, SourceFile, TypeRef, TypeRefKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::{FileId, Span};

use crate::lexer::tokenize;
use crate::token::{Token, TokenKind};
pub use mono::monomorphize;
use recovery::same_kind;

/// Ayrıştırma sonucu: AST + tüm tanılar (lexer + parser) + parser'ın
/// ürettiği sentetik kaynaklar (ADR-0044 `@mmio`).
#[derive(Debug)]
pub struct ParseResult {
    pub ast: SourceFile,
    pub diagnostics: Vec<Diagnostic>,
    /// Desugar'ın ürettiği ve kendi `FileId`'siyle ayrıştırdığı Volt
    /// metinleri. Sürücü bunları SourceMap'e AYNI kimlikle kaydeder ki
    /// üretilen koda düşen bir tanı üretilen satırı göstersin.
    pub generated: Vec<GeneratedSource>,
    /// `@mmio` modüllerinin register haritaları (ADR-0053): RTL'e açılan
    /// bilginin yazılım tarafı için dil bağımsız kopyası. Sürücü
    /// `--emit=rust,c,regmap,regmap-md` çıktılarını bundan üretir.
    pub regmaps: Vec<RegMap>,
}

/// Parser'ın ürettiği sentetik kaynak dosya (ADR-0044).
#[derive(Debug, Clone)]
pub struct GeneratedSource {
    /// Birimdeki dosyaların ardından sırayla atanan kimlik.
    pub file: FileId,
    /// Görünen ad (`<mmio:Gpio>`).
    pub name: String,
    pub text: String,
}

impl ParseResult {
    pub fn error_codes(&self) -> Vec<&'static str> {
        self.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }
}

/// Kaynak dosyayı tam AST'ye ayrıştırır. Hiçbir girdide panik etmez.
pub fn parse(file: FileId, source: &str) -> ParseResult {
    let mut parser = Parser::new(file, source);
    // Monomorfizasyon (ADR-0041) ve `for` açılımı (ADR-0056) pipeline
    // desugar'ı gibi parser katmanında, bundle düzleştirmesinden önce
    // (`finish_unit_desugar`); alt geçitler somut, açılmış modül görür.
    parser.parse_source_file();
    parser.finish()
}

/// Birden çok dosyayı TEK derleme birimine ayrıştırır (ADR-0042).
/// Dosyalar sırayla aynı arena'lara eklenir; her düğümün span'i kendi
/// dosyasını taşır. Monomorfizasyon tüm dosyalar okunduktan sonra bir
/// kez koşar — generic tanım ile örneklemesi farklı dosyalarda olabilir.
pub fn parse_unit(files: &[(FileId, &str)]) -> ParseResult {
    let mut ast = SourceFile::default();
    let mut diagnostics = Vec::new();
    let mut generated = Vec::new();
    let mut regmaps = Vec::new();
    let next_synthetic = files.iter().map(|(f, _)| f.0 + 1).max().unwrap_or(0);
    for (i, &(file, source)) in files.iter().enumerate() {
        let mut parser = Parser::new(file, source);
        parser.ast = std::mem::take(&mut ast);
        parser.parse_items_only();
        if i + 1 == files.len() {
            // ADR-0044 @mmio desugar'ı, sonra mono + `for` açılımı +
            // ADR-0039 bundle düzleştirmesi — hepsi tüm birim üzerinde
            // bir kez (generic tanım ile örneklemesi farklı dosyada olabilir).
            parser.next_synthetic = next_synthetic;
            parser.desugar_mmio();
            parser.finish_unit_desugar();
        }
        let result = parser.finish();
        ast = result.ast;
        diagnostics.extend(result.diagnostics);
        generated.extend(result.generated);
        regmaps.extend(result.regmaps);
    }
    ParseResult {
        ast,
        diagnostics,
        generated,
        regmaps,
    }
}

/// Ayrıştırma sonucu desugar/mono ÖNCESİ (test yardımcısı): `for`
/// gibi parser katmanında açılan yapıların ham AST'si.
pub fn parse_items_only_for_tests(file: FileId, source: &str) -> ParseResult {
    let mut parser = Parser::new(file, source);
    parser.parse_items_only();
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
    /// `stage(X).y` ara kayıtları (ADR-0038) — desugar tüketir.
    pub(crate) stage_refs: pipeline::StageRefMap,
    /// Ayrıştırılmakta olan aşamanın indeksi (göreli stage refleri için).
    pub(crate) current_stage: Option<usize>,
    /// Pipeline gövdesi içinde miyiz? (stage(...) yalnız burada geçerli.)
    pub(crate) in_pipeline: bool,
    /// Bir sonraki sentetik kaynak kimliği (ADR-0044). Tek dosyada
    /// `file + 1`; birimde en büyük kimlik + 1 (parse_unit ayarlar).
    pub(crate) next_synthetic: u32,
    /// Bu ayrıştırmada üretilen sentetik kaynaklar.
    pub(crate) generated: Vec<GeneratedSource>,
    /// Açılan `@mmio` modüllerinin register haritaları (ADR-0053).
    pub(crate) regmaps: Vec<RegMap>,
    /// Çift yönlü port durumu (ADR-0051): öğenin `inout`/`opendrain`
    /// portları ve `p.drive(v)`'nin beklettiği ikinci deyim.
    pub(crate) bidir: bidir::BidirState,
    /// Üst düzey const'lar (ad → değer ifadesi) — bundle dizisi
    /// indekslerinin sabit değerlendirmesi için (ADR-0056).
    pub(crate) consts: std::collections::HashMap<String, Idx<Expr>>,
    /// Bir döngüye ulaşan tip adları (ADR-0069, `type_graph`): bundle ve
    /// Handshake açılımı bunları açmaz.
    pub(crate) recursive_types: HashSet<String>,
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
            stage_refs: pipeline::StageRefMap::new(),
            current_stage: None,
            in_pipeline: false,
            next_synthetic: file.0 + 1,
            generated: Vec::new(),
            regmaps: Vec::new(),
            bidir: bidir::BidirState::default(),
            consts: std::collections::HashMap::new(),
            recursive_types: HashSet::new(),
        }
    }

    pub(crate) fn finish(self) -> ParseResult {
        ParseResult {
            ast: self.ast,
            diagnostics: self.diagnostics,
            generated: self.generated,
            regmaps: self.regmaps,
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

    /// `pos + n` konumundaki tokenın metni; dosya sonunda boş.
    pub(crate) fn text_at(&self, pos: usize) -> &'s str {
        self.tokens.get(pos).map_or("", |t| self.text_of(t.span))
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
        self.reject_bare_struct_lit();
        expr
    }

    /// `if p == P { a: 1 } { ... }` / `cover: p == P { a: 1 }`: başlıkta
    /// `{` bloğu başlatır (Rust'taki belirsizlik), literal yazılamaz.
    /// `Ad { alan:` görülürse E0001 parantez önerisiyle verilir ve
    /// literal atlanır (ADR-0077 Karar 6; kaskad yok).
    fn reject_bare_struct_lit(&mut self) {
        let prev_is_name = self
            .pos
            .checked_sub(1)
            .and_then(|i| self.tokens.get(i))
            .is_some_and(|t| t.kind == TokenKind::Ident);
        if !(prev_is_name
            && self.at(TokenKind::LBrace)
            && self.peek(1) == Some(TokenKind::Ident)
            && self.peek(2) == Some(TokenKind::Colon))
        {
            return;
        }
        let name = self.text_at(self.pos - 1).to_string();
        let start = self.pos;
        let mut depth = 0usize;
        while !self.at_eof() {
            match self.current() {
                Some(TokenKind::LBrace) => depth += 1,
                Some(TokenKind::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        self.bump_any();
                        break;
                    }
                }
                _ => {}
            }
            self.bump_any();
        }
        let span = self.span_from(start);
        self.push_error(Diagnostic::error(
            ErrorCode::E0001,
            lstr!(en: "a struct literal here must be in parentheses: '{{' would start the block";
                  tr: "burada struct literali parantez içinde olmalı: '{{' bloğu başlatırdı"),
            LabeledSpan::primary(
                span,
                lstr!(en: "read as the start of a block"; tr: "blok başlangıcı olarak okundu"),
            ),
            lstr!(en: "wrap the struct literal in parentheses: ({name} {{ ... }})";
                  tr: "struct literalini parantez içine alın: ({name} {{ ... }})"),
        ));
    }
}
