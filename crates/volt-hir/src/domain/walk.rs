//! Deyim ve blok yürüyüşü (§3 adım 4-6): her deyimi ilgili kurala
//! (K5 yayılım, K6/K7 atama, K8 örnekleme) yönlendirir.

use volt_ast::{Block, BlockStmt, ElseBranch, Idx, IfStmt, MatchArmBody, Stmt, StmtKind};
use volt_span::Span;

use super::{DomainId, Inferencer};

impl Inferencer<'_> {
    pub(super) fn walk_stmt(&mut self, stmt_idx: Idx<Stmt>) {
        match &self.ast.stmts[stmt_idx].kind {
            StmtKind::Reg(r) => {
                // init sabittir; yine de yayılım için hesaplanır.
                self.expr_domain(r.init);
            }
            StmtKind::Let(l) => {
                let dom = self.expr_domain(l.value);
                if let Some(def) = self.decl_def(&l.name) {
                    self.signal_domains.insert(def, dom);
                }
            }
            StmtKind::Wire(w) => {
                // Anotasyon sözdizimi yok: çoklu saatte kısıt değişkeni,
                // tek saatte varsayılan alan.
                if let Some(def) = self.decl_def(&w.name) {
                    let dom = if self.multi_clock {
                        self.fresh_var()
                    } else {
                        self.default_domain
                    };
                    self.signal_domains.insert(def, dom);
                }
            }
            StmtKind::Instance(inst) => self.check_instance(inst),
            StmtKind::On(on) => {
                let (dom, span) = self.on_block_domain(&on.trigger);
                self.walk_block(on.body, Some((dom, span)));
            }
            StmtKind::Comb(block) => self.walk_block(*block, None),
            StmtKind::Assign(a) => self.check_assign(&a.lhs, a.rhs, None),
            StmtKind::For(f) => {
                self.expr_domain(f.start);
                self.expr_domain(f.end);
                self.walk_block(f.body, None);
            }
            StmtKind::Expr(e) => {
                self.expr_domain(*e);
            }
            StmtKind::Error => {}
        }
    }

    fn walk_block(&mut self, block_idx: Idx<Block>, ctx: Option<(DomainId, Span)>) {
        let block = &self.ast.blocks[block_idx];
        for stmt in &block.stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, rhs, .. }
                | BlockStmt::BlockAssign { lhs, rhs, .. } => self.check_assign(lhs, *rhs, ctx),
                BlockStmt::If(if_stmt) => self.walk_if(if_stmt, ctx),
                BlockStmt::Match(mt) => {
                    let dom = self.expr_domain(mt.scrutinee);
                    if let Some(ctx) = ctx {
                        self.check_foreign_read(dom, self.ast.exprs[mt.scrutinee].span, ctx);
                    }
                    for arm in &mt.arms {
                        match &arm.body {
                            MatchArmBody::Block(b) => self.walk_block(*b, ctx),
                            MatchArmBody::Expr(e) => {
                                self.expr_domain(*e);
                            }
                        }
                    }
                }
                BlockStmt::Let(l) => {
                    let dom = self.expr_domain(l.value);
                    if let Some(def) = self.decl_def(&l.name) {
                        self.signal_domains.insert(def, dom);
                    }
                }
                BlockStmt::For(f) => {
                    self.expr_domain(f.start);
                    self.expr_domain(f.end);
                    self.walk_block(f.body, ctx);
                }
                BlockStmt::Error => {}
            }
        }
        if let Some(tail) = block.tail {
            self.expr_domain(tail);
        }
    }

    fn walk_if(&mut self, if_stmt: &IfStmt, ctx: Option<(DomainId, Span)>) {
        let dom = self.expr_domain(if_stmt.cond);
        if let Some(ctx) = ctx {
            self.check_foreign_read(dom, self.ast.exprs[if_stmt.cond].span, ctx);
        }
        self.walk_block(if_stmt.then_block, ctx);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.walk_block(*b, ctx),
            Some(ElseBranch::If(nested)) => self.walk_if(nested, ctx),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};
    use super::super::DomainId;

    #[test]
    fn let_takes_the_domain_of_its_value() {
        let r = inferred(&two_clock("    in a : u8 @Slow\n    let x = a + 1"));
        assert_eq!(r.domain_name_of("x"), Some("Slow"));
    }

    #[test]
    fn wire_uses_the_default_domain_with_a_single_clock() {
        let r = inferred(
            "module M {\n    in clk : clock\n    in a : u8\n    wire w : u8\n    w = a\n}\n",
        );
        assert_eq!(r.domain_name_of("w"), Some("clk"));
    }

    #[test]
    fn wire_is_a_constraint_variable_with_several_clocks() {
        let r = inferred(&two_clock(
            "    in a : u8 @Slow\n    wire w : u8\n    w = a",
        ));
        assert!(matches!(r.domain_of("w"), DomainId::Unresolved(_)));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn constrained_wire_conflicts_with_the_other_domain() {
        let r = inferred(&two_clock(
            "    in a : u8 @Slow\n    out q : u8 @Fast\n    wire w : u8\n    w = a\n    q = w",
        ));
        assert_eq!(r.codes(), ["E3001"]);
    }

    #[test]
    fn comb_block_assignments_are_checked() {
        let r = inferred(&two_clock(
            "    in a : u8 @Slow\n    out q : u8 @Fast\n    comb {\n        q = a\n    }",
        ));
        assert_eq!(r.codes(), ["E3001"]);
    }
}
