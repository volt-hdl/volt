//! Bloklar (name-resolution.md §2, §5): blok kapsamı, blok içi
//! deyimler ve if/match kolları (desenler `pattern`, ADR-0085).

use std::collections::HashMap;

use volt_ast::{Block, BlockStmt, ElseBranch, Idx, MatchArm, MatchArmBody};
use volt_span::Span;

use super::def::DefKind;
use super::scope::{ScopeId, ScopeKind};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn resolve_block(&mut self, block_idx: Idx<Block>, parent: ScopeId) {
        let block = &self.ast.blocks[block_idx];
        let scope = self.new_scope(ScopeKind::Block, Some(parent));

        let mut later: HashMap<String, Span> = HashMap::new();
        for stmt in &block.stmts {
            if let BlockStmt::Let(l) = stmt {
                later.entry(l.name.text.clone()).or_insert(l.name.span);
            }
        }
        self.pending.push(later);
        for stmt in &block.stmts {
            self.resolve_block_stmt(stmt, scope);
        }
        if let Some(tail) = block.tail {
            self.resolve_expr(tail, scope);
        }
        self.pending.pop();
    }

    fn resolve_block_stmt(&mut self, stmt: &BlockStmt, scope: ScopeId) {
        match stmt {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                self.resolve_lvalue(lhs, scope);
                self.resolve_expr(*rhs, scope);
            }
            BlockStmt::If(if_stmt) => self.resolve_if(if_stmt, scope),
            BlockStmt::Match(m) => {
                self.resolve_expr(m.scrutinee, scope);
                for arm in &m.arms {
                    self.resolve_arm(arm, scope);
                }
            }
            BlockStmt::Let(l) => {
                if let Some(ty) = l.ty {
                    self.resolve_type(ty, scope);
                }
                if let Some(domain) = &l.domain {
                    self.resolve_domain_ref(&domain.clone(), scope);
                }
                self.resolve_expr(l.value, scope);
                self.declare_local(&l.name.clone(), DefKind::LocalBinding, scope);
            }
            BlockStmt::For(f) => self.resolve_for(f, scope),
            BlockStmt::Error => {}
        }
    }

    fn resolve_if(&mut self, if_stmt: &volt_ast::IfStmt, scope: ScopeId) {
        self.resolve_expr(if_stmt.cond, scope);
        self.resolve_block(if_stmt.then_block, scope);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.resolve_block(*b, scope),
            Some(ElseBranch::If(nested)) => self.resolve_if(nested, scope),
            None => {}
        }
    }

    pub(super) fn resolve_arm(&mut self, arm: &MatchArm, scope: ScopeId) {
        let arm_scope = self.new_scope(ScopeKind::MatchArm, Some(scope));
        self.resolve_pattern(arm.pattern, arm_scope);
        if let Some(guard) = arm.guard {
            self.resolve_expr(guard, arm_scope);
        }
        match &arm.body {
            MatchArmBody::Block(b) => self.resolve_block(*b, arm_scope),
            MatchArmBody::Expr(e) => self.resolve_expr(*e, arm_scope),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, resolved};
    use super::super::DefKind;

    #[test]
    fn match_arm_name_is_a_value_not_a_binding() {
        // ADR-0085: `n` bağlanmaz — tanımsız ad tek E1001 (muhafız ve
        // gövdedeki kullanım kaskad üretmez), PatternBinding tanımı yok.
        let r =
            resolved("module M { in x : u8 out y : u8 y = match x { n if n > 4 => n, _ => 0 } }");
        assert_eq!(r.error_codes(), ["E1001"]);
        assert!(r.defs.iter().all(|d| d.kind != DefKind::PatternBinding));
    }

    #[test]
    fn loop_variable_is_not_visible_after_the_loop() {
        let src = "module M {\n    in clk : clock\n    out y : u8\n    reg line : [u8; 4] = [0; 4]\n    \
                   on clk {\n        for i in 1..4 { line[i] <= line[i - 1] }\n        line[0] <= i\n    }\n    \
                   y = line[3]\n}\n";
        assert!(codes(src).contains(&"E1001"), "{:?}", codes(src));
    }
}
