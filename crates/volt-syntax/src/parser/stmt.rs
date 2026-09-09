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
use volt_diagnostics::{
    lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion,
};

use crate::token::TokenKind::*;

use super::expr::ABOVE_COMPARISON_BP;
use super::recovery::{BLOCK_STMT_START, STMT_START};
use super::{Parser, MAX_DEPTH};

/// `let` değeri iki üretimden birine gider (grammar §10 + §19 [N3]):
/// düz bağlama ya da generic argümanlı modül örneklemesi.
pub(crate) enum LetOrInstance {
    Let(LetDecl),
    Instance(InstanceDecl),
}

impl Parser<'_> {
    // ═══ Modül gövdesi deyimleri ══════════════════════════════════

    /// Nitelikler çağıran (modül gövdesi döngüsü) tarafından ayrıştırılıp
    /// buraya aktarılır — SADECE saklanır, yorumlanmaz.
    pub(crate) fn parse_stmt(&mut self, attrs: Vec<Attribute>) -> Idx<Stmt> {
        let start = self.pos;

        let kind = match self.current() {
            Some(KwReg) => self.parse_reg(),
            Some(KwLet) => match self.parse_let_impl(true) {
                // [N3]: '=' sonrası yapı literali görülünce InstanceDecl'e
                // yeniden sınıflandırma — geri izleme DEĞİL. Generic
                // argümanlı örnekleme (`let f = AsyncFifo<u8, 16> { ... }`)
                // ise sınırlı token ileri bakışıyla doğrudan ayrıştırılır
                // (grammar §10 InstanceDecl [GenericArgs]); ileri bakış
                // `> {` görmezse normal ifade yolu (karşılaştırma) sürer.
                Some(LetOrInstance::Instance(inst)) => StmtKind::Instance(inst),
                Some(LetOrInstance::Let(decl)) => self.reclassify_let(decl),
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
                                    &lstr!(en: "expression on the right-hand side of the assignment"; tr: "atamanın sağında ifade"),
                                    &lstr!(en: "write it as name = expression"; tr: "isim = ifade biçiminde yazın"),
                                );
                                self.alloc_error_expr(self.current_span())
                            };
                            self.eat(Semi);
                            StmtKind::Assign(AssignStmt { lhs, rhs })
                        }
                        None => {
                            self.error_expected(
                                &lstr!(en: "name/index/field as the assignment target"; tr: "atama hedefi olarak isim/indeks/alan"),
                                &lstr!(en: "the left-hand side must be name, name[i] or name.field"; tr: "sol taraf isim, isim[i] veya isim.alan olmalı"),
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
                    lstr!(en: "unexpected '{}', expected a statement", self.current_text(); tr: "beklenmeyen '{}', deyim bekleniyor", self.current_text()),
                    LabeledSpan::primary(
                        self.current_span(),
                        lstr!(en: "expected a statement"; tr: "deyim bekleniyor"),
                    ),
                    lstr!(en: "expected reg, let, wire, on, comb, for or an assignment"; tr: "reg, let, wire, on, comb, for veya atama bekleniyor"),
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
                    &lstr!(en: "clock name for the reg domain"; tr: "reg domaini için saat adı"),
                    &lstr!(en: "write it as reg(clk) name = 0"; tr: "reg(clk) isim = 0 biçiminde yazın"),
                );
                None
            };
            self.expect(
                RParen,
                &lstr!(en: "closing ')'"; tr: "kapanış ')'"),
                &lstr!(en: "write it as reg(clk)"; tr: "reg(clk) biçiminde yazın"),
            );
            d
        } else {
            None
        };

        if !self.at(Ident) {
            self.error_expected(
                &lstr!(en: "register name"; tr: "register adı"),
                &lstr!(en: "write it as reg name : u8 = 0"; tr: "reg isim : u8 = 0 biçiminde yazın"),
            );
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
                self.error_expected(
                    &lstr!(en: "initial value"; tr: "başlangıç değeri"),
                    &lstr!(en: "write it as reg name : u8 = 0"; tr: "reg isim : u8 = 0 biçiminde yazın"),
                );
                self.alloc_error_expr(self.current_span())
            }
        } else {
            self.error_expected(
                &lstr!(en: "'=' for the reg initial value"; tr: "reg başlangıç değeri için '='"),
                &lstr!(en: "write it as reg {} : u8 = 0", name.text; tr: "reg {} : u8 = 0 biçiminde yazın", name.text),
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

    /// `let isim [: tip] = ifade [;]` — blok bağlamı (örnekleme yok).
    pub(crate) fn parse_let(&mut self) -> Option<LetDecl> {
        match self.parse_let_impl(false)? {
            LetOrInstance::Let(decl) => Some(decl),
            // allow_instance=false ile erişilmez.
            LetOrInstance::Instance(_) => None,
        }
    }

    /// `let` gövdesi. `allow_instance` yalnız modül gövdesinde true:
    /// değer konumu `Yol<...> {` ile başlıyorsa (sınırlı ileri bakış,
    /// AST kurmadan) doğrudan InstanceDecl ayrıştırılır — grammar §10.
    fn parse_let_impl(&mut self, allow_instance: bool) -> Option<LetOrInstance> {
        self.bump_any(); // 'let'

        if !self.at(Ident) {
            self.error_expected(
                &lstr!(en: "name for the let binding"; tr: "let bağlaması için isim"),
                &lstr!(en: "write it as let name = expression"; tr: "let isim = ifade biçiminde yazın"),
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
            if allow_instance && ty.is_none() && self.instance_generics_ahead() {
                return Some(LetOrInstance::Instance(self.parse_generic_instance(name)));
            }
            if self.at_expr_start() {
                self.parse_expr()
            } else {
                self.error_expected(
                    &lstr!(en: "let value"; tr: "let değeri"),
                    &lstr!(en: "write it as let name = expression"; tr: "let isim = ifade biçiminde yazın"),
                );
                self.alloc_error_expr(self.current_span())
            }
        } else {
            self.error_expected(
                &lstr!(en: "'=' for the let binding"; tr: "let için '='"),
                &lstr!(en: "write it as let {} = expression", name.text; tr: "let {} = ifade biçiminde yazın", name.text),
            );
            self.alloc_error_expr(self.current_span())
        };

        self.eat(Semi);
        Some(LetOrInstance::Let(LetDecl { name, ty, value }))
    }

    /// Sınırlı ileri bakış: mevcut konum `Ident (:: Ident)* <` ile
    /// başlayıp `<`/`>` yuvası kapandığında `{` geliyorsa true. AST
    /// kurulmaz, geri izleme yoktur; yalnız ham token'lara bakılır.
    /// `>>` iki kapanış sayılır (Fifo<Entry<8>> deseni). Tarama ~32
    /// token ile sınırlıdır — `let a = b < c` gibi karşılaştırmalar
    /// hızla elenir ve normal ifade yoluna düşer.
    fn instance_generics_ahead(&self) -> bool {
        const SCAN_LIMIT: usize = 32;
        let kind_at = |i: usize| self.tokens.get(i).map(|t| t.kind);

        let mut i = self.pos;
        if kind_at(i) != Some(Ident) {
            return false;
        }
        i += 1;
        while kind_at(i) == Some(ColonColon) && kind_at(i + 1) == Some(Ident) {
            i += 2;
        }
        if kind_at(i) != Some(Lt) {
            return false;
        }

        let mut depth = 0usize;
        let limit = i + SCAN_LIMIT;
        while i < limit {
            match kind_at(i) {
                Some(Lt) => depth += 1,
                Some(Gt) => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                    if depth == 0 {
                        return kind_at(i + 1) == Some(LBrace);
                    }
                }
                Some(Shr) => {
                    if depth < 2 {
                        return false;
                    }
                    depth -= 2;
                    if depth == 0 {
                        return kind_at(i + 1) == Some(LBrace);
                    }
                }
                // Generic argüman gövdesinde beklenen token'lar: isimler,
                // sabitler, virgül, yol ayırıcı ve yerleşik tip anahtar
                // kelimeleri. Başka bir şey görülürse ifade yoluna düşülür.
                Some(
                    Ident | IntLit | Comma | ColonColon | KwBool | KwClock | KwReset | UIntType
                    | SIntType | KwTrit | KwBits,
                ) => {}
                _ => return false,
            }
            i += 1;
        }
        false
    }

    /// `Yol<Args> { port: değer, ... }` — ileri bakış doğruladıktan
    /// sonra doğrudan örnekleme ayrıştırması (yapı literali sapağı yok).
    fn parse_generic_instance(&mut self, name: Name) -> InstanceDecl {
        let module_path = self.parse_path();
        let generic_args = self.parse_generic_args();

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the port bindings"; tr: "port bağlamaları için '{{'"),
            &lstr!(en: "write it as let name = Module<...> {{ port: value }}"; tr: "let isim = Modul<...> {{ port: değer }} biçiminde yazın"),
        );

        let mut bindings = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            if self.at(Ident) {
                let bstart = self.pos;
                let port_name = self.parse_name();
                let value = if self.eat(Colon) {
                    if self.at_expr_start() {
                        Some(self.parse_expr())
                    } else {
                        self.error_expected(
                            &lstr!(en: "port value"; tr: "port değeri"),
                            &lstr!(en: "write it as port: expression"; tr: "port: ifade biçiminde yazın"),
                        );
                        Some(self.alloc_error_expr(self.current_span()))
                    }
                } else {
                    None // `clk` kısayolu — yerel isim port adıyla aynı
                };
                bindings.push(PortBinding {
                    span: self.span_from(bstart),
                    port_name,
                    value,
                });
            } else {
                self.error_expected(
                    &lstr!(en: "port name"; tr: "port adı"),
                    &lstr!(en: "write it as Module<...> {{ port: value }}"; tr: "Modul<...> {{ port: değer }} biçiminde yazın"),
                );
            }
            if !self.eat(Comma) && self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        self.expect_closing(RBrace, "}", open);
        self.eat(Semi);

        InstanceDecl {
            name,
            module_path,
            generic_args,
            bindings,
        }
    }

    /// `wire isim : tip [;]` — bildirim; atama ayrı AssignStmt ile.
    fn parse_wire(&mut self) -> StmtKind {
        self.bump_any(); // 'wire'

        if !self.at(Ident) {
            self.error_expected(
                &lstr!(en: "wire name"; tr: "wire adı"),
                &lstr!(en: "write it as wire name : u8"; tr: "wire isim : u8 biçiminde yazın"),
            );
            self.recover_silent(STMT_START);
            return StmtKind::Error;
        }
        let name = self.parse_name();

        self.expect(
            Colon,
            &lstr!(en: "':' in the wire declaration"; tr: "wire bildiriminde ':'"),
            &lstr!(en: "write it as wire {} : u8", name.text; tr: "wire {} : u8 biçiminde yazın", name.text),
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
            self.error_expected(
                &lstr!(en: "loop variable"; tr: "döngü değişkeni"),
                &lstr!(en: "write it as for i in 0..N {{ }}"; tr: "for i in 0..N {{ }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        self.expect(
            KwIn,
            &lstr!(en: "'in' for the 'for' loop"; tr: "'for' için 'in'"),
            &lstr!(en: "write it as for i in 0..N {{ }}"; tr: "for i in 0..N {{ }} biçiminde yazın"),
        );

        let start = if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected(
                &lstr!(en: "range start"; tr: "aralık başlangıcı"),
                &lstr!(en: "write it as for i in 0..N"; tr: "for i in 0..N biçiminde yazın"),
            );
            self.alloc_error_expr(self.current_span())
        };

        self.expect(
            DotDot,
            &lstr!(en: "'..' for the range"; tr: "aralık için '..'"),
            &lstr!(en: "write it as for i in 0..N"; tr: "for i in 0..N biçiminde yazın"),
        );

        let end = if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected(
                &lstr!(en: "range end"; tr: "aralık sonu"),
                &lstr!(en: "write it as for i in 0..N"; tr: "for i in 0..N biçiminde yazın"),
            );
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
                        &lstr!(en: "'reset' after '.'"; tr: "'.' sonrasında 'reset'"),
                        &lstr!(en: "write it as on clk.reset {{ }}"; tr: "on clk.reset {{ }} biçiminde yazın"),
                    );
                    OnTrigger::Error
                }
            } else {
                OnTrigger::Clock(name)
            }
        } else {
            self.error_expected(
                &lstr!(en: "clock name for 'on'"; tr: "'on' için saat adı"),
                &lstr!(en: "write it as on clk {{ }}"; tr: "on clk {{ }} biçiminde yazın"),
            );
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
                lstr!(en: "block is nested too deeply"; tr: "blok çok derin iç içe"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "nesting depth limit exceeded"; tr: "derinlik sınırı aşıldı"),
                ),
                lstr!(en: "split the nested blocks into separate modules"; tr: "iç içe blokları ayrı modüllere bölün"),
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
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the block"; tr: "blok için '{{'"),
            &lstr!(en: "write it as {{ ... }}"; tr: "{{ ... }} biçiminde yazın"),
        );

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
                    lstr!(en: "unexpected '{}', expected a block statement", self.current_text(); tr: "beklenmeyen '{}', blok deyimi bekleniyor", self.current_text()),
                    LabeledSpan::primary(
                        self.current_span(),
                        lstr!(en: "expected a statement"; tr: "deyim bekleniyor"),
                    ),
                    lstr!(en: "expected an assignment, if, match, for or let"; tr: "atama, if, match, for veya let bekleniyor"),
                );
                self.recover(BLOCK_STMT_START, err);
                BlockStmt::Error
            }
        }
    }

    /// `match ifade { desen [if koşul] => gövde, ... }`
    /// Enum varyantları üzerinden tam exhaustiveness F3'ün işi; o zamana
    /// dek deyim bağlamındaki match'te joker '_' kolu zorunludur —
    /// eksikse E0014 (ADR-0032).
    pub(crate) fn parse_match_stmt(&mut self, ctx: BlockContext) -> MatchStmt {
        let start = self.pos;
        self.bump_any(); // 'match'

        let scrutinee = if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected(
                &lstr!(en: "match scrutinee"; tr: "match konusu"),
                &lstr!(en: "write it as match x {{ pattern => ... }}"; tr: "match x {{ desen => ... }} biçiminde yazın"),
            );
            self.alloc_error_expr(self.current_span())
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the match body"; tr: "match gövdesi için '{{'"),
            &lstr!(en: "write it as match x {{ ... }}"; tr: "match x {{ ... }} biçiminde yazın"),
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
        let span = self.span_from(start);

        let has_wildcard = arms
            .iter()
            .any(|arm| arm.guard.is_none() && self.pattern_has_wildcard(arm.pattern));
        if !has_wildcard {
            self.push_error(
                Diagnostic::error(
                    ErrorCode::E0014,
                    lstr!(en: "'match' statement has no '_' arm"; tr: "'match' deyiminde '_' kolu yok"),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "every value must be covered"; tr: "her değer kapsanmalı"),
                    ),
                    lstr!(en: "add a final '_ => {{ }}' arm"; tr: "sona '_ => {{ }}' kolu ekleyin"),
                )
                .with_note(
                    NoteKind::Note,
                    lstr!(en: "exhaustiveness analysis over enum variants arrives with F3 (ADR-0032); in a sequential block an empty '_' arm keeps the registers' values"; tr: "enum varyantları üzerinden kapsayıcılık analizi F3 ile gelecek (ADR-0032); sıralı blokta boş '_' kolu register değerlerini korur"),
                ),
            );
        }

        MatchStmt {
            span,
            scrutinee,
            arms,
        }
    }

    /// Desen `_` içeriyor mu? `A | _` gibi alternatifler de sayılır.
    fn pattern_has_wildcard(&self, idx: Idx<volt_ast::Pattern>) -> bool {
        use volt_ast::PatternKind;
        match &self.ast.patterns[idx].kind {
            PatternKind::Wildcard => true,
            PatternKind::Or(alts) => alts.iter().any(|&p| self.pattern_has_wildcard(p)),
            _ => false,
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
                self.error_expected(
                    &lstr!(en: "guard condition"; tr: "koruma koşulu"),
                    &lstr!(en: "write it as pattern if cond => ..."; tr: "desen if koşul => ... biçiminde yazın"),
                );
                None
            }
        } else {
            None
        };

        self.expect(
            FatArrow,
            &lstr!(en: "'=>' in the match arm"; tr: "match kolunda '=>'"),
            &lstr!(en: "write it as pattern => result"; tr: "desen => sonuç biçiminde yazın"),
        );

        let body = match ctx {
            Some(block_ctx) if self.at(LBrace) => MatchArmBody::Block(self.parse_block(block_ctx)),
            _ => {
                let expr = if self.at_expr_start() {
                    self.parse_expr()
                } else {
                    self.error_expected(
                        &lstr!(en: "arm body"; tr: "kol gövdesi"),
                        &lstr!(en: "write it as pattern => expression"; tr: "desen => ifade biçiminde yazın"),
                    );
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
                    &lstr!(en: "assignment target"; tr: "atama hedefi"),
                    &lstr!(en: "the left-hand side must be name, name[i] or name.field"; tr: "sol taraf isim, isim[i] veya isim.alan olmalı"),
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
                        lstr!(en: "'=' cannot be used in a sequential block"; tr: "sıralı blokta '=' kullanılamaz"),
                        LabeledSpan::primary(
                            op_span,
                            lstr!(en: "should be '<='"; tr: "'<=' olmalı"),
                        ),
                        lstr!(en: "use '<='"; tr: "'<=' kullanın"),
                    )
                    .with_note(
                        NoteKind::Note,
                        lstr!(en: "assignments inside an 'on' block happen on the clock edge"; tr: "'on' bloğu içindeki atamalar saat kenarında olur"),
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
                        lstr!(en: "'<=' cannot be used in a combinational block"; tr: "kombinasyonel blokta '<=' kullanılamaz"),
                        LabeledSpan::primary(
                            op_span,
                            lstr!(en: "should be '='"; tr: "'=' olmalı"),
                        ),
                        lstr!(en: "use '='"; tr: "'=' kullanın"),
                    )
                    .with_note(
                        NoteKind::Note,
                        lstr!(en: "a comb block contains immediate assignments; there is no clock edge"; tr: "comb bloğu anlık atama içerir, saat kenarı yoktur"),
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
                    &lstr!(en: "assignment operator {expected}"; tr: "atama operatörü {expected}"),
                    &lstr!(en: "write it as target {expected} expression"; tr: "hedef {expected} ifade biçiminde yazın"),
                );
                self.recover_silent(BLOCK_STMT_START);
                return BlockStmt::Error;
            }
        };

        let rhs = if self.at_expr_start() {
            self.parse_expr()
        } else {
            self.error_expected(
                &lstr!(en: "expression on the right-hand side of the assignment"; tr: "atamanın sağında ifade"),
                &lstr!(en: "write it as target <= expression"; tr: "hedef <= ifade biçiminde yazın"),
            );
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
            self.error_expected(
                &lstr!(en: "'if' condition"; tr: "'if' koşulu"),
                &lstr!(en: "write it as if cond {{ ... }}"; tr: "if koşul {{ ... }} biçiminde yazın"),
            );
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
