//! K5 — kombinasyonel yayılım: ifadenin alanı alt ifadelerinin
//! `join`'idir. Değerlendirme sırası tanı sırasını belirler.

use volt_ast::{ArrayLitKind, Expr, ExprKind, Idx, MatchArm, MatchArmBody};
use volt_span::Span;

use super::{DomainId, Inferencer};
use crate::resolve::BuiltinKind;

impl Inferencer<'_> {
    pub(super) fn expr_domain(&mut self, e: Idx<Expr>) -> DomainId {
        if let Some(&d) = self.expr_domains.get(&e) {
            return d;
        }
        let d = self.compute_expr_domain(e);
        self.expr_domains.insert(e, d);
        d
    }

    fn compute_expr_domain(&mut self, e: Idx<Expr>) -> DomainId {
        let expr = &self.ast.exprs[e];
        match &expr.kind {
            ExprKind::IntLit { .. }
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::Todo { .. }
            | ExprKind::Concat(_)
            | ExprKind::Error => DomainId::Timeless,
            ExprKind::Path(_) => self.path_domain(e, expr.span),
            ExprKind::Binary { lhs, rhs, .. } => self.join_pair(*lhs, *rhs),
            ExprKind::Unary { operand, .. } => self.expr_domain(*operand),
            ExprKind::Cast { expr: inner, .. } => self.expr_domain(*inner),
            ExprKind::Index { base, index } => self.join_pair(*base, *index),
            ExprKind::Range { base, hi, lo } => {
                self.expr_domain(*hi);
                self.expr_domain(*lo);
                self.expr_domain(*base)
            }
            // Başlangıç indeksi çalışma zamanı okumasıdır; genişlik sabittir.
            ExprKind::PartSelect {
                base, start, width, ..
            } => {
                self.expr_domain(*width);
                self.join_pair(*base, *start)
            }
            ExprKind::Field { base, field } => self.field_domain(*base, &field.text),
            ExprKind::Call { callee, args } => self.call_domain(*callee, &args.clone(), expr.span),
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => self.if_domain(*cond, *then_expr, *else_expr),
            ExprKind::Match { scrutinee, arms } => self.match_domain(*scrutinee, arms),
            ExprKind::StructLit { fields, .. } => {
                let exprs: Vec<Idx<Expr>> = fields.iter().filter_map(|f| f.value).collect();
                self.join_list(&exprs, expr.span)
            }
            ExprKind::ArrayLit(ArrayLitKind::List(items)) | ExprKind::TupleLit(items) => {
                self.join_list(&items.clone(), expr.span)
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                self.expr_domain(*count);
                self.expr_domain(*value)
            }
        }
    }

    /// Ad okuması: sinyalin alanı; çift yönlü port ADR-0051'e düşer.
    fn path_domain(&mut self, e: Idx<Expr>, span: Span) -> DomainId {
        match self.res.resolutions.get(&e) {
            Some(&def) => {
                if self.bidir_ports.contains(&def) {
                    return self.bidir_read(def, span);
                }
                self.def_domain(def)
            }
            None => DomainId::Timeless,
        }
    }

    /// İki alt ifadenin `join`'i — önce sol, sonra sağ değerlendirilir.
    fn join_pair(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>) -> DomainId {
        let a = self.expr_domain(lhs);
        let b = self.expr_domain(rhs);
        self.join(a, b, self.ast.exprs[lhs].span, self.ast.exprs[rhs].span)
    }

    /// `base.field`: instance port okuması K8 haritasından (uart.busy),
    /// aksi halde tabanın alanı.
    fn field_domain(&mut self, base: Idx<Expr>, field: &str) -> DomainId {
        if let Some(&base_def) = self.res.resolutions.get(&base) {
            if let Some(ports) = self.instance_ports.get(&base_def) {
                return ports.get(field).copied().unwrap_or(DomainId::Timeless);
            }
        }
        self.expr_domain(base)
    }

    /// Çağrı: `sync()` köprüdür (K9), `prev(x)` x'in alanındadır
    /// (ADR-0040), diğerleri argümanlarının `join`'idir.
    fn call_domain(&mut self, callee: Idx<Expr>, args: &[Idx<Expr>], span: Span) -> DomainId {
        if let Some(kind) = self.builtin_of(callee) {
            if matches!(kind, BuiltinKind::Sync | BuiltinKind::Sync3) {
                // ADR-0051: sync() kaynağı olan çift yönlü okuma
                // meşru köprüdür — W3007 üretmez.
                self.in_sync_source = true;
                let d = self.sync_domain(args, span);
                self.in_sync_source = false;
                return d;
            }
            // prev(x) x-in alanında değerlendirilir (ADR-0040).
            if kind == BuiltinKind::Prev {
                if let Some(&x) = args.first() {
                    return self.expr_domain(x);
                }
            }
        }
        self.join_list(args, span)
    }

    fn if_domain(
        &mut self,
        cond: Idx<Expr>,
        then_expr: Idx<Expr>,
        else_expr: Idx<Expr>,
    ) -> DomainId {
        let c = self.expr_domain(cond);
        let t = self.expr_domain(then_expr);
        let e2 = self.expr_domain(else_expr);
        let ct = self.join(
            c,
            t,
            self.ast.exprs[cond].span,
            self.ast.exprs[then_expr].span,
        );
        self.join(
            ct,
            e2,
            self.ast.exprs[cond].span,
            self.ast.exprs[else_expr].span,
        )
    }

    fn match_domain(&mut self, scrutinee: Idx<Expr>, arms: &[MatchArm]) -> DomainId {
        let mut dom = self.expr_domain(scrutinee);
        let s_span = self.ast.exprs[scrutinee].span;
        let arm_exprs: Vec<Idx<Expr>> = arms
            .iter()
            .filter_map(|a| match &a.body {
                MatchArmBody::Expr(e) => Some(*e),
                MatchArmBody::Block(_) => None,
            })
            .collect();
        for a in arm_exprs {
            let d = self.expr_domain(a);
            dom = self.join(dom, d, s_span, self.ast.exprs[a].span);
        }
        dom
    }

    fn join_list(&mut self, exprs: &[Idx<Expr>], fallback_span: Span) -> DomainId {
        let mut dom = DomainId::Timeless;
        let mut dom_span = fallback_span;
        for &e in exprs {
            let d = self.expr_domain(e);
            let e_span = self.ast.exprs[e].span;
            dom = self.join(dom, d, dom_span, e_span);
            if matches!(self.resolve_dom(d), DomainId::Explicit(_)) {
                dom_span = e_span;
            }
        }
        dom
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};
    use super::super::DomainId;

    const PORTS: &str = "    in a : u8 @Fast\n    in b : u8 @Slow\n    in c : bool @Fast\n";

    #[test]
    fn literals_are_timeless_and_take_the_domain_of_the_other_operand() {
        let r = inferred(&two_clock(&format!(
            "{PORTS}    let k = 3\n    let x = a + 1"
        )));
        assert_eq!(r.domain_of("k"), DomainId::Timeless);
        assert_eq!(r.domain_name_of("x"), Some("Fast"));
    }

    #[test]
    fn binary_mix_of_two_domains_is_e3001_and_yields_error() {
        let r = inferred(&two_clock(&format!("{PORTS}    let x = a + b")));
        assert_eq!(r.codes(), ["E3001"]);
        assert_eq!(r.domain_of("x"), DomainId::Error);
    }

    #[test]
    fn error_domain_suppresses_cascades() {
        let r = inferred(&two_clock(&format!(
            "{PORTS}    let x = a + b\n    let y = x + b\n    let z = y + a"
        )));
        assert_eq!(r.codes(), ["E3001"]);
    }

    #[test]
    fn if_expression_joins_condition_and_both_branches() {
        let ok = inferred(&two_clock(&format!(
            "{PORTS}    let x = if c {{ a }} else {{ 0 }}"
        )));
        assert_eq!(ok.domain_name_of("x"), Some("Fast"));
        let bad = inferred(&two_clock(&format!(
            "{PORTS}    let x = if c {{ a }} else {{ b }}"
        )));
        assert_eq!(bad.codes(), ["E3001"]);
    }

    #[test]
    fn index_joins_base_and_index() {
        let r = inferred(&two_clock(&format!(
            "{PORTS}    in i : u3 @Slow\n    let x = a[i]"
        )));
        assert_eq!(r.codes(), ["E3001"]);
    }

    #[test]
    fn call_arguments_are_joined_left_to_right() {
        let r = inferred(&two_clock(&format!("{PORTS}    let x = max(a, b)")));
        assert_eq!(r.codes(), ["E3001"]);
        let d = &r.dom.diagnostics[0];
        assert_eq!(d.spans[0].label, "@Fast");
        assert_eq!(d.spans[1].label, "@Slow");
    }

    #[test]
    fn instance_port_read_uses_the_instance_map() {
        let r = inferred(&format!(
            "module Inner {{\n    in clk : clock\n    out busy : bool\n    busy = true\n}}\n{}",
            two_clock("    let u = Inner { clk: slow_clk }\n    let x = u.busy")
        ));
        assert_eq!(r.domain_name_of("x"), Some("Slow"));
    }
}
