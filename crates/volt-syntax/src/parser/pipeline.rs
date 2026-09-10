//! Pipeline bildirimi ayrıştırma (ADR-0038): `pipeline(N) Ad { ... }`,
//! `stage`, `stall`, `flush` ve `stage(X).y` referansları.
//!
//! Ayrıştırılan `PipelineDecl` hemen desugar'a (desugar.rs) verilir ve
//! sıradan bir `ModuleDecl`'e iner — AST arenasına Pipeline öğesi hiç
//! girmez, sonraki aşamalar pipeline'ı görmez.

use std::collections::HashMap;

use volt_ast::{
    BlockContext, Expr, FlushDecl, Idx, ItemKind, Name, PipelineDecl, StageDecl, StallDecl,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use crate::token::TokenKind::*;

use super::Parser;

/// `stage(X).y` ara kaydı: ifade düğümü desugar'da yeniden yazılana dek
/// `ExprKind::Error` yer tutucusudur, bilgi bu yan kayıtta taşınır.
#[derive(Debug)]
pub(crate) struct StageRefData {
    pub(crate) target: StageRefTarget,
    pub(crate) field: Name,
    pub(crate) span: Span,
    /// Yazıldığı aşamanın indeksi; modül seviyesinde None.
    pub(crate) stage: Option<usize>,
}

#[derive(Debug)]
pub(crate) enum StageRefTarget {
    Named(Name),
    Relative(i64),
}

pub(crate) type StageRefMap = HashMap<Idx<Expr>, StageRefData>;

impl Parser<'_> {
    /// `pipeline "(" IntLit ")" Ident "{" PipelineBody "}"` — grammar §4a.
    pub(crate) fn parse_pipeline(&mut self) -> ItemKind {
        let item_start = self.pos;
        self.bump_any(); // 'pipeline'
        let prev_in_pipeline = std::mem::replace(&mut self.in_pipeline, true);

        let open_paren = self.current_span();
        self.expect(
            LParen,
            &lstr!(en: "'(' after 'pipeline'"; tr: "'pipeline' sonrası '('"),
            &lstr!(en: "write it as pipeline(5) Name {{ ... }}"; tr: "pipeline(5) Ad {{ ... }} biçiminde yazın"),
        );
        let depth_span = self.current_span();
        let depth = if self.at(IntLit) {
            let text = self.current_text().replace('_', "");
            self.bump_any();
            text.parse::<u32>().unwrap_or_else(|_| {
                self.push_error(pipeline_err(
                    ErrorCode::E5011,
                    depth_span,
                    lstr!(en: "invalid pipeline depth"; tr: "geçersiz pipeline derinliği"),
                    lstr!(en: "the depth must be a small decimal literal"; tr: "derinlik küçük bir ondalık literal olmalı"),
                ));
                0
            })
        } else {
            self.error_expected(
                &lstr!(en: "stage count in 'pipeline(N)'"; tr: "'pipeline(N)' içinde aşama sayısı"),
                &lstr!(en: "write it as pipeline(5) Name {{ ... }}"; tr: "pipeline(5) Ad {{ ... }} biçiminde yazın"),
            );
            0
        };
        self.expect_closing(RParen, ")", open_paren);

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "pipeline name"; tr: "pipeline adı"),
                &lstr!(en: "write it as pipeline(5) Name {{ ... }}"; tr: "pipeline(5) Ad {{ ... }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the pipeline body"; tr: "pipeline gövdesi için '{{'"),
            &lstr!(en: "write it as pipeline(5) Name {{ ... }}"; tr: "pipeline(5) Ad {{ ... }} biçiminde yazın"),
        );

        let mut decl = PipelineDecl {
            name,
            depth,
            depth_span,
            ports: Vec::new(),
            contracts: Vec::new(),
            body: Vec::new(),
            stages: Vec::new(),
            stalls: Vec::new(),
            flushes: Vec::new(),
        };

        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            let doc = self.collect_doc_comments();
            let attrs = self.parse_attributes();
            match self.current() {
                Some(KwIn) | Some(KwOut) | Some(KwInout) => {
                    if let Some(port) = self.parse_port(attrs, doc) {
                        decl.ports.push(port);
                    }
                }
                Some(k) if super::item::contract_kind(k).is_some() => {
                    decl.contracts.push(self.parse_contract());
                }
                Some(KwStage) => self.parse_stage(&mut decl),
                Some(KwStall) => {
                    let stall = self.parse_stall(None);
                    decl.stalls.push(stall);
                }
                Some(KwFlush) => {
                    let flush = self.parse_flush(None);
                    decl.flushes.push(flush);
                }
                Some(RBrace) | None => break,
                _ => {
                    decl.body.push(self.parse_stmt(attrs));
                }
            }
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.expect_closing(RBrace, "}", open);
        self.in_pipeline = prev_in_pipeline;

        // Sentetik bildirim span'leri pipeline metninin TAMAMI içinde
        // dağıtılır — benzersizlik için geniş aralık gerekir.
        let full_span = self.span_from(item_start);
        self.desugar_pipeline(decl, full_span)
    }

    /// `stage Ad { ... }` — gövde ardışık bağlamdır; `stall`/`flush`
    /// deyimleri aşamaya bağlanır (in_stage), gerisi blok deyimidir.
    fn parse_stage(&mut self, decl: &mut PipelineDecl) {
        self.bump_any(); // 'stage'
        let stage_idx = decl.stages.len();

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "stage name"; tr: "aşama adı"),
                &lstr!(en: "write it as stage Fetch {{ ... }}"; tr: "stage Fetch {{ ... }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let prev_stage = self.current_stage.replace(stage_idx);
        let start = self.pos;
        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the stage body"; tr: "aşama gövdesi için '{{'"),
            &lstr!(en: "write it as stage {} {{ ... }}", name.text; tr: "stage {} {{ ... }} biçiminde yazın", name.text),
        );

        let mut stmts = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            match self.current() {
                Some(KwStall) => {
                    let stall = self.parse_stall(Some(stage_idx));
                    decl.stalls.push(stall);
                }
                Some(KwFlush) => {
                    let flush = self.parse_flush(Some(stage_idx));
                    decl.flushes.push(flush);
                }
                _ => stmts.push(self.parse_block_stmt(BlockContext::Sequential)),
            }
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        self.expect_closing(RBrace, "}", open);
        self.current_stage = prev_stage;

        let span = self.span_from(start);
        let body = self.ast.blocks.alloc(volt_ast::Block {
            span,
            stmts,
            tail: None,
            context: BlockContext::Sequential,
        });
        decl.stages.push(StageDecl { name, body });
    }

    /// `stall [S1, S2] when koşul` — liste 'when' bağlamsal kelimesine
    /// kadar okunur; modül seviyesindeki listesiz biçim E5013'ü desugar
    /// üretir (aşama çıpası orada denetlenir).
    fn parse_stall(&mut self, in_stage: Option<usize>) -> StallDecl {
        let start = self.pos;
        self.bump_any(); // 'stall'
        let stages = self.parse_stage_list();
        let cond = self.parse_when_cond("stall");
        self.eat(Semi);
        StallDecl {
            span: self.span_from(start),
            stages,
            cond,
            in_stage,
        }
    }

    /// `flush S1, S2 when koşul`.
    fn parse_flush(&mut self, in_stage: Option<usize>) -> FlushDecl {
        let start = self.pos;
        self.bump_any(); // 'flush'
        let stages = self.parse_stage_list();
        let cond = self.parse_when_cond("flush");
        self.eat(Semi);
        FlushDecl {
            span: self.span_from(start),
            stages,
            cond,
            in_stage,
        }
    }

    /// `S1, S2, ...` — 'when' görülene dek aşama adları.
    fn parse_stage_list(&mut self) -> Vec<Name> {
        let mut stages = Vec::new();
        while self.at(Ident) && self.current_text() != "when" {
            stages.push(self.parse_name());
            if !self.eat(Comma) {
                break;
            }
        }
        stages
    }

    /// Bağlamsal `when` + koşul ifadesi (if başlığı kuralı: StructLit yasak).
    fn parse_when_cond(&mut self, what: &str) -> Idx<Expr> {
        if self.at(Ident) && self.current_text() == "when" {
            self.bump_any();
        } else {
            self.error_expected(
                &lstr!(en: "'when' in the {what} statement"; tr: "{what} deyiminde 'when'"),
                &lstr!(en: "write it as {what} Fetch, Decode when cond"; tr: "{what} Fetch, Decode when koşul biçiminde yazın"),
            );
        }
        if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected(
                &lstr!(en: "{what} condition"; tr: "{what} koşulu"),
                &lstr!(en: "write a bool expression after 'when'"; tr: "'when' sonrasına bool ifade yazın"),
            );
            self.alloc_error_expr(self.current_span())
        }
    }

    /// `stage "(" (Ad | ±k) ")" "." alan` — ifade konumu (expr.rs'ten).
    /// Yer tutucu `ExprKind::Error` düğümü döner; çözüm bilgisi
    /// `stage_refs` yan kaydına yazılır, desugar düğümü yeniden yazar.
    pub(crate) fn parse_stage_ref(&mut self, start: usize) -> Idx<Expr> {
        self.bump_any(); // 'stage'

        let open = self.current_span();
        self.expect(
            LParen,
            &lstr!(en: "'(' after 'stage'"; tr: "'stage' sonrası '('"),
            &lstr!(en: "write it as stage(Execute).alu_out or stage(+1).x"; tr: "stage(Execute).alu_out veya stage(+1).x biçiminde yazın"),
        );

        let target = match self.current() {
            Some(Ident) => Some(StageRefTarget::Named(self.parse_name())),
            Some(Plus) | Some(Minus) => {
                let negative = self.at(Minus);
                self.bump_any();
                if self.at(IntLit) {
                    let text = self.current_text().replace('_', "");
                    self.bump_any();
                    let k = text.parse::<i64>().unwrap_or(i64::MAX);
                    Some(StageRefTarget::Relative(if negative { -k } else { k }))
                } else {
                    self.error_expected(
                        &lstr!(en: "stage offset after the sign"; tr: "işaret sonrası aşama uzaklığı"),
                        &lstr!(en: "write it as stage(+1).x or stage(-1).x"; tr: "stage(+1).x veya stage(-1).x biçiminde yazın"),
                    );
                    None
                }
            }
            _ => {
                self.error_expected(
                    &lstr!(en: "stage name or ±offset"; tr: "aşama adı veya ±uzaklık"),
                    &lstr!(en: "write it as stage(Execute).x or stage(+1).x"; tr: "stage(Execute).x veya stage(+1).x biçiminde yazın"),
                );
                None
            }
        };
        self.expect_closing(RParen, ")", open);

        self.expect(
            Dot,
            &lstr!(en: "'.' after 'stage(...)'"; tr: "'stage(...)' sonrası '.'"),
            &lstr!(en: "write it as stage(Execute).alu_out"; tr: "stage(Execute).alu_out biçiminde yazın"),
        );
        let field = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "value name after '.'"; tr: "'.' sonrası değer adı"),
                &lstr!(en: "write it as stage(Execute).alu_out"; tr: "stage(Execute).alu_out biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let span = self.span_from(start);
        let idx = self.alloc_error_expr(span);

        if !self.in_pipeline {
            self.push_error(pipeline_err(
                ErrorCode::E5012,
                span,
                lstr!(en: "stage reference outside a pipeline"; tr: "pipeline dışında aşama referansı"),
                lstr!(en: "stage(...).x is only meaningful inside a pipeline declaration"; tr: "stage(...).x yalnız pipeline bildirimi içinde anlamlıdır"),
            ));
            return idx;
        }
        if let Some(target) = target {
            self.stage_refs.insert(
                idx,
                StageRefData {
                    target,
                    field,
                    span,
                    stage: self.current_stage,
                },
            );
        }
        idx
    }
}

/// ADR-0038 tanıları için kısa kurucu (5 parça: kod, konum, açıklama,
/// öneri; spec referansı `volt explain` üzerinden).
pub(crate) fn pipeline_err(
    code: ErrorCode,
    span: Span,
    message: String,
    help: String,
) -> Diagnostic {
    let label = message.clone();
    Diagnostic::error(code, message, LabeledSpan::primary(span, label), help)
}
