//! Modül düzeyinde ifade deyimi (grammar-full.ebnf `ExprStmt`).
//!
//! Modül gövdesinde tek başına duran bir ifade donanımda hiçbir şey
//! yapmaz ve SV üretiminde düşer. Kullanıcının niyeti hemen her zaman bir
//! atamadır: `r <= r + 1` (`on` dışında, karşılaştırma olarak ayrışır)
//! ya da `y == a` (`y = a` yazım hatası). Bloklarda aynı yazım zaten
//! E0001/E0007'dir; modül düzeyi aynı kurala bağlanır (error-recovery.md
//! §6.1 bağlam denetimi). Hedef türünü (reg mi, tel mi) bilmek için
//! denetim modül gövdesinin tamamı ayrıştırıldıktan sonra yapılır.

use volt_ast::{BinOp, Expr, ExprKind, Idx, Stmt, StmtKind};
use volt_diagnostics::{
    lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion,
};
use volt_span::Span;

use super::register_assign::RegClocks;
use super::Parser;

impl Parser<'_> {
    pub(super) fn check_module_expr_stmts(&mut self, regs: &RegClocks, body: &[Idx<Stmt>]) {
        for &s in body {
            let StmtKind::Expr(e) = self.ast.stmts[s].kind else {
                continue;
            };
            let diag = match self.ast.exprs[e].kind {
                ExprKind::Binary {
                    op: BinOp::Le,
                    lhs,
                    rhs,
                } => match self.expr_to_lvalue(lhs) {
                    Some(target) => {
                        let clock = regs
                            .clock_of(&target.base.text)
                            .map(|c| c.unwrap_or_else(|| "<clock>".to_string()));
                        self.module_le_diag(lhs, rhs, clock)
                    }
                    None => self.module_expr_diag(e),
                },
                ExprKind::Binary {
                    op: BinOp::Eq,
                    lhs,
                    rhs,
                } if self.expr_to_lvalue(lhs).is_some() => self.module_eq_diag(e, lhs, rhs),
                _ => self.module_expr_diag(e),
            };
            // Her deyim kendi hatasıdır (kaskad değil): bastırma penceresi
            // uygulanmaz, gövde zaten ayrıştırıldı.
            self.diagnostics.push(diag);
        }
    }

    /// İki operand arasındaki operatörün konumu (`<=`, `==`).
    fn operator_span(&self, lhs: Idx<Expr>, rhs: Idx<Expr>, op: &str) -> Span {
        let (l, r) = (self.ast.exprs[lhs].span, self.ast.exprs[rhs].span);
        let between = Span {
            start: l.end,
            end: r.start,
            ..l
        };
        let offset = self.text_of(between).find(op).unwrap_or(0) as u32;
        Span {
            start: l.end + offset,
            end: l.end + offset + op.len() as u32,
            ..l
        }
    }

    /// `hedef <= değer` modül düzeyinde: reg ise `on` bloğu, değilse `=`.
    fn module_le_diag(&self, lhs: Idx<Expr>, rhs: Idx<Expr>, clock: Option<String>) -> Diagnostic {
        let op = self.operator_span(lhs, rhs, "<=");
        let target = self.text_of(self.ast.exprs[lhs].span);
        let value = self.text_of(self.ast.exprs[rhs].span);
        let message = lstr!(
            en: "'<=' cannot be used outside an 'on' block";
            tr: "'<=' 'on' bloğu dışında kullanılamaz"
        );
        let note = lstr!(
            en: "at module level '{target} <= {value}' reads as a comparison; the statement would do nothing";
            tr: "modül düzeyinde '{target} <= {value}' bir karşılaştırma olarak okunur; deyim hiçbir şey yapmaz"
        );
        let diag = match clock {
            Some(clk) => Diagnostic::error(
                ErrorCode::E0007,
                message,
                LabeledSpan::primary(
                    op,
                    lstr!(en: "register update outside an 'on' block"; tr: "'on' bloğu dışında register güncellemesi"),
                ),
                lstr!(
                    en: "a register takes its next value in an 'on' block: on {clk} {{ {target} <= {value} }}";
                    tr: "register sonraki değerini bir 'on' bloğunda alır: on {clk} {{ {target} <= {value} }}"
                ),
            ),
            None => Diagnostic::error(
                ErrorCode::E0007,
                message,
                LabeledSpan::primary(op, lstr!(en: "should be '='"; tr: "'=' olmalı")),
                lstr!(
                    en: "outside 'on' blocks assign with '=': {target} = {value}";
                    tr: "'on' blokları dışında '=' ile atayın: {target} = {value}"
                ),
            )
            .with_suggestion(Suggestion {
                span: op,
                replacement: "=".to_string(),
                applicability: Applicability::MachineApplicable,
            }),
        };
        diag.with_note(NoteKind::Note, note)
    }

    /// `hedef == değer` modül düzeyinde: büyük olasılıkla `=` yazım hatası.
    fn module_eq_diag(&self, e: Idx<Expr>, lhs: Idx<Expr>, rhs: Idx<Expr>) -> Diagnostic {
        let op = self.operator_span(lhs, rhs, "==");
        let target = self.text_of(self.ast.exprs[lhs].span);
        let value = self.text_of(self.ast.exprs[rhs].span);
        Diagnostic::error(
            ErrorCode::E0001,
            lstr!(
                en: "an expression on its own is not a statement";
                tr: "tek başına bir ifade deyim değildir"
            ),
            LabeledSpan::primary(
                self.ast.exprs[e].span,
                lstr!(en: "this comparison is never used"; tr: "bu karşılaştırma hiç kullanılmıyor"),
            ),
            lstr!(
                en: "to assign, write '=': {target} = {value}";
                tr: "atamak için '=' yazın: {target} = {value}"
            ),
        )
        .with_suggestion(Suggestion {
            span: op,
            replacement: "=".to_string(),
            applicability: Applicability::MaybeIncorrect,
        })
    }

    fn module_expr_diag(&self, e: Idx<Expr>) -> Diagnostic {
        Diagnostic::error(
            ErrorCode::E0001,
            lstr!(
                en: "an expression on its own is not a statement";
                tr: "tek başına bir ifade deyim değildir"
            ),
            LabeledSpan::primary(
                self.ast.exprs[e].span,
                lstr!(en: "this value is never used"; tr: "bu değer hiç kullanılmıyor"),
            ),
            lstr!(
                en: "assign the value to a signal: name = expression";
                tr: "değeri bir sinyale atayın: ad = ifade"
            ),
        )
    }
}
