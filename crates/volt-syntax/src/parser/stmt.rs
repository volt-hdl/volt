//! Deyim ve blok ayrıştırma (F1): RegDecl, LetDecl, WireDecl,
//! InstanceDecl, OnBlock, Comb, ForStmt, AssignStmt; blok içi
//! NonBlockAssign/BlockAssign/If/Match/Let/For.
//!
//! E0006/E0007 blok bağlamı kontrolleri error-recovery.md §6.1'e göre:
//! yanlış operatör raporlanır ama ayrıştırma doğru operatör gibi sürer.

use volt_ast::{
    AssignStmt, Attribute, Block, BlockContext, BlockStmt, ElseBranch, Expr, ExprKind, ForStmt,
    Idx, IfStmt, InstanceDecl, LValue, LValueSuffix, LetDecl, MatchArm, MatchArmBody, MatchStmt,
    Name, OnBlock, OnTrigger, PortBinding, RegDecl, Stmt, StmtKind, WireDecl,
};
use volt_diagnostics::{Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion};

use crate::token::TokenKind::*;

use super::expr::ABOVE_COMPARISON_BP;
use super::recovery::{BLOCK_STMT_START, STMT_START};
use super::{Parser, MAX_DEPTH};

impl Parser<'_> {
    // ═══ Modül gövdesi deyimleri ══════════════════════════════════

    /// Nitelikler çağıran (modül gövdesi döngüsü) tarafından ayrıştırılıp
    /// buraya aktarılır — SADECE saklanır, yorumlanmaz.
    pub(crate) fn parse_stmt(&mut self, attrs: Vec<Attribute>) -> Idx<Stmt> {
        let start = self.pos;

        let kind = match self.current() {
            Some(KwReg) => self.parse_reg(),
            Some(KwLet) => match self.parse_let() {
                // [N3]: '=' sonrası yapı literali görülünce InstanceDecl'e
                // yeniden sınıflandırma — geri izleme DEĞİL.
                Some(decl) => self.reclassify_let(decl),
                None => StmtKind::Error,
            },
            Some(KwWire) => self.parse_wire(),
            Some(KwOn) => self.parse_on(),
            Some(KwComb) => {
                self.bump_any(); // 'comb'
                StmtKind::Comb(self.parse_block(BlockContext::Combinational))
            }
            Some(KwFor) => StmtKind::For(self.parse_for_stmt(BlockContext::Combinational)),
            _ if self.at_expr_start() => {
                // AssignStmt (LValue '=' Expr) veya ExprStmt
                let expr = self.parse_expr();
                if self.at(Eq) {
                    match self.expr_to_lvalue(expr) {
                        Some(lhs) => {
                            self.bump_any(); // '='
                            let rhs = if self.at_expr_start() {
                                self.parse_expr()
                            } else {
                                self.error_expected(
                                    "atamanın sağında ifade",
                                    "isim = ifade biçiminde yazın",
                                );
                                self.alloc_error_expr(self.current_span())
                            };
                            self.eat(Semi);
                            StmtKind::Assign(AssignStmt { lhs, rhs })
                        }
                        None => {
                            self.error_expected(
                                "atama hedefi olarak isim/indeks/alan",
                                "sol taraf isim, isim[i] veya isim.alan olmalı",
                            );
                            self.bump_any(); // '='
                            let _ = self.parse_expr();
                            self.eat(Semi);
                            StmtKind::Error
                        }
                    }
                } else {
                    self.eat(Semi);
                    StmtKind::Expr(expr)
                }
            }
            None => StmtKind::Error,
            _ => {
                let err = Diagnostic::error(
                    ErrorCode::E0001,
                    format!("beklenmeyen '{}', deyim bekleniyor", self.current_text()),
                    LabeledSpan::primary(self.current_span(), "deyim bekleniyor"),
                    "reg, let, wire, on, comb, for veya atama bekleniyor",
                );
                self.recover(STMT_START, err);
                StmtKind::Error
            }
        };

        let span = self.span_from(start);
        self.ast.stmts.alloc(Stmt { span, attrs, kind })
    }

    /// [N3] yeniden sınıflandırma: `let u = Uart { clk: clk }` yapı
    /// literali olarak ayrıştırıldı; tip anotasyonu yoksa modül
    /// örneklemesine dönüştürülür (grammar-full.ebnf §19 N3).
    fn reclassify_let(&mut self, decl: LetDecl) -> StmtKind {
        if decl.ty.is_none()
            && matches!(self.ast.exprs[decl.value].kind, ExprKind::StructLit { .. })
        {
            let kind = std::mem::replace(&mut self.ast.exprs[decl.value].kind, ExprKind::Error);
            let ExprKind::StructLit { path, fields } = kind else {
                unreachable!()
            };
            let bindings = fields
                .into_iter()
                .map(|f| PortBinding {
                    span: f.span,
                    port_name: f.name,
                    value: f.value,
                })
                .collect();
            return StmtKind::Instance(InstanceDecl {
                name: decl.name,
                module_path: path,
                generic_args: Vec::new(),
                bindings,
            });
        }
        StmtKind::Let(decl)
    }

    /// `reg [(domain)] isim [: tip] = ifade [;]`
    fn parse_reg(&mut self) -> StmtKind {
        self.bump_any(); // 'reg'

        let domain = if self.eat(LParen) {
            let d = if self.at(Ident) {
                Some(self.parse_name())
            } else {
                self.error_expected(
                    "reg domaini için saat adı",
                    "reg(clk) isim = 0 biçiminde yazın",
                );
                None
            };
            self.expect(RParen, "kapanış ')'", "reg(clk) biçiminde yazın");
            d
        } else {
            None
        };

        if !self.at(Ident) {
            self.error_expected("register adı", "reg isim : u8 = 0 biçiminde yazın");
            self.recover_silent(STMT_START);
            return StmtKind::Error;
        }
        let name = self.parse_name();

        let ty = if self.eat(Colon) {
            Some(self.parse_type_or_error())
        } else {
            None
        };

        let init = if self.eat(Eq) {
            if self.at_expr_start() {
                self.parse_expr()
            } else {
                self.error_expected("başlangıç değeri", "reg isim : u8 = 0 biçiminde yazın");
                self.alloc_error_expr(self.current_span())
            }
        } else {
            self.error_expected(
                "reg başlangıç değeri için '='",
                &format!("reg {} : u8 = 0 biçiminde yazın", name.text),
            );
            self.alloc_error_expr(self.current_span())
        };

        self.eat(Semi);
        StmtKind::Reg(RegDecl {
            name,
            domain,
            ty,
            init,
        })
    }

    /// `let isim [: tip] = ifade [;]`
    pub(crate) fn parse_let(&mut self) -> Option<LetDecl> {
        self.bump_any(); // 'let'

        if !self.at(Ident) {
            self.error_expected(
                "let bağlaması için isim",
                "let isim = ifade biçiminde yazın",
            );
            self.recover_silent(STMT_START);
            return None;
        }
        let name = self.parse_name();

        let ty = if self.eat(Colon) {
            Some(self.parse_type_or_error())
        } else {
            None
        };

        let value = if self.eat(Eq) {
            if self.at_expr_start() {
                self.parse_expr()
            } else {
                self.error_expected("let değeri", "let isim = ifade biçiminde yazın");
                self.alloc_error_expr(self.current_span())
            }
        } else {
            self.error_expected(
                "let için '='",
                &format!("let {} = ifade biçiminde yazın", name.text),
            );
            self.alloc_error_expr(self.current_span())
        };

        self.eat(Semi);
        Some(LetDecl { name, ty, value })
    }

    /// `wire isim : tip [;]` — bildirim; atama ayrı AssignStmt ile.
    fn parse_wire(&mut self) -> StmtKind {
        self.bump_any(); // 'wire'

        if !self.at(Ident) {
            self.error_expected("wire adı", "wire isim : u8 biçiminde yazın");
            self.recover_silent(STMT_START);
            return StmtKind::Error;
        }
        let name = self.parse_name();

        self.expect(
            Colon,
            "wire bildiriminde ':'",
            &format!("wire {} : u8 biçiminde yazın", name.text),
        );
        let ty = self.parse_type_or_error();
        self.eat(Semi);

        StmtKind::Wire(WireDecl { name, ty })
    }

    /// `for i in başlangıç..bitiş { ... }` — derleme zamanı generate
    /// döngüsü; açma F2'de, burada yalnız ayrıştırılır. Gövde bağlamı
    /// çevreleyen bloktan miras alınır.
    pub(crate) fn parse_for_stmt(&mut self, ctx: BlockContext) -> ForStmt {
        self.bump_any(); // 'for'

        let var = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected("döngü değişkeni", "for i in 0..N { } biçiminde yazın");
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        self.expect(KwIn, "'for' için 'in'", "for i in 0..N { } biçiminde yazın");

        let start = if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected("aralık başlangıcı", "for i in 0..N biçiminde yazın");
            self.alloc_error_expr(self.current_span())
        };

        self.expect(DotDot, "aralık için '..'", "for i in 0..N biçiminde yazın");

        let end = if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected("aralık sonu", "for i in 0..N biçiminde yazın");
            self.alloc_error_expr(self.current_span())
        };

        let body = self.parse_block(ctx);
        ForStmt {
            var,
            start,
            end,
            body,
        }
    }

    /// `on tetik { ... }` — tetik: saat adı veya `saat.reset`.
    fn parse_on(&mut self) -> StmtKind {
        self.bump_any(); // 'on'

        let trigger = if self.at(Ident) {
            let name = self.parse_name();
            if self.eat(Dot) {
                if self.eat(KwReset) {
                    OnTrigger::Reset(name)
                } else {
                    self.error_expected(
                        "'.' sonrasında 'reset'",
                        "on clk.reset { } biçiminde yazın",
                    );
                    OnTrigger::Error
                }
            } else {
                OnTrigger::Clock(name)
            }
        } else {
            self.error_expected("'on' için saat adı", "on clk { } biçiminde yazın");
            OnTrigger::Error
        };

        let body = self.parse_block(BlockContext::Sequential);
        StmtKind::On(OnBlock { trigger, body })
    }

    // ═══ Bloklar ══════════════════════════════════════════════════

    pub(crate) fn parse_block(&mut self, ctx: BlockContext) -> Idx<Block> {
        let start = self.pos;

        // Derinlik sınırı — patolojik iç içe blokta yığın koruması
        if self.depth >= MAX_DEPTH {
            let span = self.bump();
            self.push_error(Diagnostic::error(
                ErrorCode::E0001,
                "blok çok derin iç içe",
                LabeledSpan::primary(span, "derinlik sınırı aşıldı"),
                "iç içe blokları ayrı modüllere bölün",
            ));
            return self.ast.blocks.alloc(Block {
                span,
                stmts: Vec::new(),
                tail: None,
                context: ctx,
            });
        }
        self.depth += 1;

        let open = self.current_span();
        self.expect(LBrace, "blok için '{'", "{ ... } biçiminde yazın");

        let mut stmts = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            stmts.push(self.parse_block_stmt(ctx));
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi — sonsuz döngü koruması
            }
        }

        self.expect_closing(RBrace, "}", open);
        self.depth -= 1;

        let span = self.span_from(start);
        self.ast.blocks.alloc(Block {
            span,
            stmts,
            tail: None,
            context: ctx,
        })
    }

    fn parse_block_stmt(&mut self, ctx: BlockContext) -> BlockStmt {
        match self.current() {
            Some(KwIf) => BlockStmt::If(self.parse_if_stmt(ctx)),
            Some(KwLet) => match self.parse_let() {
                Some(decl) => BlockStmt::Let(decl),
                None => BlockStmt::Error,
            },
            Some(KwMatch) => BlockStmt::Match(self.parse_match_stmt(ctx)),
            Some(KwFor) => BlockStmt::For(self.parse_for_stmt(ctx)),
            _ if self.at_expr_start() => self.parse_block_assign(ctx),
            _ => {
                let err = Diagnostic::error(
                    ErrorCode::E0001,
                    format!(
                        "beklenmeyen '{}', blok deyimi bekleniyor",
                        self.current_text()
                    ),
                    LabeledSpan::primary(self.current_span(), "deyim bekleniyor"),
                    "atama, if, match, for veya let bekleniyor",
                );
                self.recover(BLOCK_STMT_START, err);
                BlockStmt::Error
            }
        }
    }

    /// `match ifade { desen [if koşul] => gövde, ... }`
    /// Exhaustiveness kontrolü F2'ye aittir — burada yalnız ayrıştırılır.
    pub(crate) fn parse_match_stmt(&mut self, ctx: BlockContext) -> MatchStmt {
        let start = self.pos;
        self.bump_any(); // 'match'

        let scrutinee = if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected("match konusu", "match x { desen => ... } biçiminde yazın");
            self.alloc_error_expr(self.current_span())
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            "match gövdesi için '{'",
            "match x { ... } biçiminde yazın",
        );

        let mut arms = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            arms.push(self.parse_match_arm(Some(ctx)));
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.expect_closing(RBrace, "}", open);
        MatchStmt {
            span: self.span_from(start),
            scrutinee,
            arms,
        }
    }

    /// Tek match kolu. `ctx` Some ise blok gövdesine izin verilir
    /// (deyim bağlamı); None ise yalnız ifade (ifade bağlamı, §13).
    pub(crate) fn parse_match_arm(&mut self, ctx: Option<BlockContext>) -> MatchArm {
        let start = self.pos;
        let pattern = self.parse_pattern();

        let guard = if self.eat(KwIf) {
            if self.at_expr_start() {
                Some(self.parse_expr_no_struct_lit())
            } else {
                self.error_expected("koruma koşulu", "desen if koşul => ... biçiminde yazın");
                None
            }
        } else {
            None
        };

        self.expect(
            FatArrow,
            "match kolunda '=>'",
            "desen => sonuç biçiminde yazın",
        );

        let body = match ctx {
            Some(block_ctx) if self.at(LBrace) => MatchArmBody::Block(self.parse_block(block_ctx)),
            _ => {
                let expr = if self.at_expr_start() {
                    self.parse_expr()
                } else {
                    self.error_expected("kol gövdesi", "desen => ifade biçiminde yazın");
                    self.alloc_error_expr(self.current_span())
                };
                MatchArmBody::Expr(expr)
            }
        };
        self.eat(Comma);

        MatchArm {
            span: self.span_from(start),
            pattern,
            guard,
            body,
        }
    }

    /// Blokta atama: LHS kısıtlı bp ile ayrıştırılır ki `<=`
    /// karşılaştırma olarak tüketilmesin.
    fn parse_block_assign(&mut self, ctx: BlockContext) -> BlockStmt {
        let start = self.pos;
        let lhs_expr = self.parse_expr_bp(ABOVE_COMPARISON_BP);
        let lhs = match self.expr_to_lvalue(lhs_expr) {
            Some(lv) => lv,
            None => {
                self.error_expected(
                    "atama hedefi",
                    "sol taraf isim, isim[i] veya isim.alan olmalı",
                );
                self.recover_silent(BLOCK_STMT_START);
                return BlockStmt::Error;
            }
        };

        let op_span = self.current_span();
        let non_blocking = match (ctx, self.current()) {
            (BlockContext::Sequential, Some(Le)) => {
                self.bump_any();
                true
            }
            (BlockContext::Sequential, Some(Eq)) => {
                // E0006 — ama '<=' yazılmış gibi DEVAM et (error-recovery.md §6.1)
                self.push_error(
                    Diagnostic::error(
                        ErrorCode::E0006,
                        "sıralı blokta '=' kullanılamaz",
                        LabeledSpan::primary(op_span, "'<=' olmalı"),
                        "'<=' kullanın",
                    )
                    .with_note(
                        NoteKind::Note,
                        "'on' bloğu içindeki atamalar saat kenarında olur",
                    )
                    .with_suggestion(Suggestion {
                        span: op_span,
                        replacement: "<=".to_string(),
                        applicability: Applicability::MachineApplicable,
                    }),
                );
                self.bump_any();
                true
            }
            (BlockContext::Combinational, Some(Eq)) | (BlockContext::Function, Some(Eq)) => {
                self.bump_any();
                false
            }
            (BlockContext::Combinational, Some(Le)) | (BlockContext::Function, Some(Le)) => {
                // E0007 — '=' yazılmış gibi devam et
                self.push_error(
                    Diagnostic::error(
                        ErrorCode::E0007,
                        "kombinasyonel blokta '<=' kullanılamaz",
                        LabeledSpan::primary(op_span, "'=' olmalı"),
                        "'=' kullanın",
                    )
                    .with_note(
                        NoteKind::Note,
                        "comb bloğu anlık atama içerir, saat kenarı yoktur",
                    )
                    .with_suggestion(Suggestion {
                        span: op_span,
                        replacement: "=".to_string(),
                        applicability: Applicability::MachineApplicable,
                    }),
                );
                self.bump_any();
                false
            }
            _ => {
                let expected = match ctx {
                    BlockContext::Sequential => "'<='",
                    BlockContext::Combinational | BlockContext::Function => "'='",
                };
                self.error_expected(
                    &format!("atama operatörü {expected}"),
                    &format!("hedef {expected} ifade biçiminde yazın"),
                );
                self.recover_silent(BLOCK_STMT_START);
                return BlockStmt::Error;
            }
        };

        let rhs = if self.at_expr_start() {
            self.parse_expr()
        } else {
            self.error_expected("atamanın sağında ifade", "hedef <= ifade biçiminde yazın");
            self.alloc_error_expr(self.current_span())
        };
        self.eat(Semi);

        let span = self.span_from(start);
        if non_blocking {
            BlockStmt::NonBlockAssign { lhs, rhs, span }
        } else {
            BlockStmt::BlockAssign { lhs, rhs, span }
        }
    }

    pub(crate) fn parse_if_stmt(&mut self, ctx: BlockContext) -> IfStmt {
        let start = self.pos;
        self.bump_any(); // 'if'

        let cond = if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected("'if' koşulu", "if koşul { ... } biçiminde yazın");
            self.alloc_error_expr(self.current_span())
        };

        let then_block = self.parse_block(ctx);

        let else_branch = if self.eat(KwElse) {
            if self.at(KwIf) {
                Some(ElseBranch::If(Box::new(self.parse_if_stmt(ctx))))
            } else {
                Some(ElseBranch::Block(self.parse_block(ctx)))
            }
        } else {
            None // deyim if'inde else opsiyonel (E0008 yalnız if-İFADESİ için)
        };

        IfStmt {
            span: self.span_from(start),
            cond,
            then_block,
            else_branch,
        }
    }

    // ═══ LValue dönüşümü ══════════════════════════════════════════

    /// İfadeyi atama hedefine yeniden yorumlar (grammar-full.ebnf §19 N3
    /// tarzı yeniden sınıflandırma — geri izleme yok).
    pub(crate) fn expr_to_lvalue(&self, idx: Idx<Expr>) -> Option<LValue> {
        let mut suffixes = Vec::new();
        let mut current = idx;
        let span = self.ast.exprs[idx].span;

        let base = loop {
            match &self.ast.exprs[current].kind {
                ExprKind::Path(path) => {
                    if path.segments.len() != 1 {
                        return None; // çok parçalı yol atama hedefi olamaz
                    }
                    break path.segments[0].clone();
                }
                ExprKind::Index { base, index } => {
                    suffixes.push(LValueSuffix::Index(*index));
                    current = *base;
                }
                ExprKind::Range { base, hi, lo } => {
                    suffixes.push(LValueSuffix::Range { hi: *hi, lo: *lo });
                    current = *base;
                }
                ExprKind::Field { base, field } => {
                    suffixes.push(LValueSuffix::Field(field.clone()));
                    current = *base;
                }
                _ => return None,
            }
        };

        suffixes.reverse();
        Some(LValue {
            span,
            base,
            suffixes,
        })
    }
}
