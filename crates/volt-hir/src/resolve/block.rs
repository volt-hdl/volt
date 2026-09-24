//! Bloklar (name-resolution.md §2, §5): blok kapsamı, blok içi
//! deyimler, if/match kolları ve desen bağlamaları.

use std::collections::HashMap;

use volt_ast::{
    Block, BlockStmt, ElseBranch, Idx, MatchArm, MatchArmBody, Pattern, PatternArgs, PatternKind,
};
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

    fn resolve_pattern(&mut self, pat_idx: Idx<Pattern>, scope: ScopeId) {
        let pat = &self.ast.patterns[pat_idx];
        match &pat.kind {
            PatternKind::Wildcard | PatternKind::Error => {}
            PatternKind::Literal(e) => self.resolve_expr(*e, scope),
            PatternKind::Binding(name) => {
                self.declare_checked(&name.clone(), DefKind::PatternBinding, scope, false);
            }
            PatternKind::Path { path, args } => {
                let def = self.resolve_path(&path.clone(), scope);
                self.pattern_resolutions.insert(pat_idx, def);
                match args {
                    Some(PatternArgs::Tuple(pats)) => {
                        for &p in pats {
                            self.resolve_pattern(p, scope);
                        }
                    }
                    Some(PatternArgs::Struct(fields)) => {
                        for f in fields {
                            match f.pattern {
                                Some(p) => self.resolve_pattern(p, scope),
                                // `Foo { x }` kısayolu x'i bağlar.
                                None => {
                                    self.declare_checked(
                                        &f.name.clone(),
                                        DefKind::PatternBinding,
                                        scope,
                                        false,
                                    );
                                }
                            }
                        }
                    }
                    None => {}
                }
            }
            PatternKind::Tuple(pats) | PatternKind::Or(pats) => {
                for &p in pats.clone().iter() {
                    self.resolve_pattern(p, scope);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, resolved};
    use super::super::DefKind;

    #[test]
    fn match_arm_binding_is_scoped_to_its_arm() {
        let r =
            resolved("module M { in x : u8 out y : u8 y = match x { n if n > 4 => n, _ => 0 } }");
        assert!(r.error_codes().is_empty(), "{:?}", r.error_codes());
        assert_eq!(
            r.def_by_name("n").expect("n").1.kind,
            DefKind::PatternBinding
        );
    }

    #[test]
    fn loop_variable_is_not_visible_after_the_loop() {
        let src = "module M {\n    in clk : clock\n    out y : u8\n    reg line : [u8; 4] = [0; 4]\n    \
                   on clk {\n        for i in 1..4 { line[i] <= line[i - 1] }\n        line[0] <= i\n    }\n    \
                   y = line[3]\n}\n";
        assert!(codes(src).contains(&"E1001"), "{:?}", codes(src));
    }
}
