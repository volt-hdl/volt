//! Deyim ve blok yürüyüşü (§3 adım 4-6): her deyimi ilgili kurala
//! (K5 yayılım, K6/K7 atama, K8 örnekleme) yönlendirir.

use volt_ast::{Block, BlockStmt, ElseBranch, Idx, IfStmt, MatchArmBody, Stmt, StmtKind};
use volt_span::Span;

use super::edgeless::EdgeUse;
use super::{DomainId, Inferencer};

impl Inferencer<'_> {
    pub(super) fn walk_stmt(&mut self, stmt_idx: Idx<Stmt>) {
        match &self.ast.stmts[stmt_idx].kind {
            StmtKind::Reg(r) => {
                // init sabittir; yine de yayılım için hesaplanır.
                self.expr_domain(r.init);
            }
            StmtKind::Let(l) => self.walk_let(l),
            StmtKind::Wire(w) => {
                // Açıklama varsa alan odur (ADR-0088) — sürücüler K6 ile
                // ona göre denetlenir (E3001); yoksa çoklu saatte kısıt
                // değişkeni, tek saatte varsayılan alan.
                if let Some(def) = self.decl_def(&w.name) {
                    let dom = match &w.domain {
                        Some(ann) => self.signal_annotation_domain(ann),
                        None if self.multi_clock => self.fresh_var(),
                        None => self.default_domain,
                    };
                    self.signal_domains.insert(def, dom);
                }
            }
            StmtKind::Instance(inst) => self.check_instance(inst),
            StmtKind::On(on) => {
                let (dom, span) = self.on_block_domain(&on.trigger);
                let dom = self.check_on_trigger(&on.trigger, dom);
                self.check_edgeless_use(dom, span, EdgeUse::On);
                self.stmt_anchor = Some(self.ast.stmts[stmt_idx].span);
                self.walk_block(on.body, Some((dom, span)), None);
                self.stmt_anchor = None;
            }
            StmtKind::Comb(block) => {
                self.stmt_anchor = Some(self.ast.stmts[stmt_idx].span);
                self.walk_block(*block, None, None);
                self.stmt_anchor = None;
            }
            StmtKind::Assign(a) => self.check_assign(&a.lhs, a.rhs, None, None),
            StmtKind::For(f) => {
                self.expr_domain(f.start);
                self.expr_domain(f.end);
                self.stmt_anchor = Some(self.ast.stmts[stmt_idx].span);
                self.walk_block(f.body, None, None);
                self.stmt_anchor = None;
            }
            StmtKind::Expr(e) => {
                self.expr_domain(*e);
            }
            StmtKind::Error => {}
        }
    }

    /// `let` (modül ya da blok): alanı değerinin alanıdır. `@Alan`
    /// açıklaması (ADR-0088) değerle K6 kuralıyla denetlenir — çelişki
    /// E3001 (sabit değer her alana uyar) — ve bağlamanın alanı olur.
    fn walk_let(&mut self, l: &volt_ast::LetDecl) {
        let dom = self.expr_domain(l.value);
        let dom = match &l.domain {
            Some(ann) => {
                let declared = self.signal_annotation_domain(ann);
                let conflict = matches!(
                    (self.resolve_dom(declared), self.resolve_dom(dom)),
                    (DomainId::Explicit(x), DomainId::Explicit(y)) if x != y
                );
                self.fix_src = Some(l.value);
                self.check_compat(declared, dom, ann.span, self.ast.exprs[l.value].span);
                self.fix_src = None;
                // Çelişki bir kez raporlanır; kullanımlar kaskad üretmez.
                if conflict {
                    DomainId::Error
                } else {
                    declared
                }
            }
            None => dom,
        };
        if let Some(def) = self.decl_def(&l.name) {
            self.signal_domains.insert(def, dom);
        }
    }

    /// `ctx`: 'on' bloğunun alanı (K7, E3012). `pc`: 'on' dışındaki
    /// (`comb`, modül `for`'u) dalların koşul alanı — koşula bağlı her
    /// atama koşulun alanını taşır (K5, ADR-0083 Karar 8).
    fn walk_block(
        &mut self,
        block_idx: Idx<Block>,
        ctx: Option<(DomainId, Span)>,
        pc: Option<(DomainId, Span)>,
    ) {
        let block = &self.ast.blocks[block_idx];
        for stmt in &block.stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, rhs, .. }
                | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                    self.check_assign(lhs, *rhs, ctx, pc);
                }
                BlockStmt::If(if_stmt) => self.walk_if(if_stmt, ctx, pc),
                BlockStmt::Match(mt) => {
                    let dom = self.expr_domain(mt.scrutinee);
                    let s_span = self.ast.exprs[mt.scrutinee].span;
                    self.fix_src = Some(mt.scrutinee);
                    let pc = self.branch_pc(dom, s_span, ctx, pc);
                    self.fix_src = None;
                    for arm in &mt.arms {
                        let pc = match arm.guard {
                            Some(g) => {
                                let gd = self.expr_domain(g);
                                self.branch_pc(gd, self.ast.exprs[g].span, ctx, pc)
                            }
                            None => pc,
                        };
                        match &arm.body {
                            MatchArmBody::Block(b) => self.walk_block(*b, ctx, pc),
                            MatchArmBody::Expr(e) => {
                                self.expr_domain(*e);
                            }
                        }
                    }
                }
                BlockStmt::Let(l) => self.walk_let(l),
                BlockStmt::For(f) => {
                    self.expr_domain(f.start);
                    self.expr_domain(f.end);
                    self.walk_block(f.body, ctx, pc);
                }
                BlockStmt::Error => {}
            }
        }
        if let Some(tail) = block.tail {
            self.expr_domain(tail);
        }
    }

    fn walk_if(
        &mut self,
        if_stmt: &IfStmt,
        ctx: Option<(DomainId, Span)>,
        pc: Option<(DomainId, Span)>,
    ) {
        let dom = self.expr_domain(if_stmt.cond);
        self.fix_src = Some(if_stmt.cond);
        let pc = self.branch_pc(dom, self.ast.exprs[if_stmt.cond].span, ctx, pc);
        self.fix_src = None;
        self.walk_block(if_stmt.then_block, ctx, pc);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.walk_block(*b, ctx, pc),
            Some(ElseBranch::If(nested)) => self.walk_if(nested, ctx, pc),
            None => {}
        }
    }

    /// Dal koşulu (`if` koşulu, sınanan, muhafız): 'on' bloğunda yabancı
    /// alan E3012 (K7); dışında koşulun alanı `pc`'ye `join` edilir (K5 —
    /// iç içe koşulların karışması da E3001'dir).
    fn branch_pc(
        &mut self,
        dom: DomainId,
        span: Span,
        ctx: Option<(DomainId, Span)>,
        pc: Option<(DomainId, Span)>,
    ) -> Option<(DomainId, Span)> {
        if let Some(ctx) = ctx {
            self.check_foreign_read(dom, span, ctx);
            return None;
        }
        match pc {
            None => Some((dom, span)),
            Some((p, p_span)) => Some((self.join(p, dom, p_span, span), span)),
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
