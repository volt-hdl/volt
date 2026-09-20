//! Deyim seviyesi kontrol (type-inference.md §6): modül gövdesi,
//! reg/let/wire bildirimleri, bloklar ve atamalar. Atamalar sürücü
//! tablosuna (§11.1) buradan kaydedilir; analiz `crate::drivers`'tadır.

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExternDecl, ForStmt, Idx, IfStmt, LValue, LValueSuffix,
    LetDecl, MatchArm, MatchArmBody, ModuleDecl, RegDecl, Stmt, StmtKind,
};
use volt_diagnostics::{lstr, ErrorCode};

use super::TypeChecker;
use crate::resolve::{DefId, DefKind};
use crate::ty::{Ty, TypeId};

impl TypeChecker<'_, '_> {
    pub(super) fn check_module(&mut self, m: &ModuleDecl) {
        for p in &m.ports {
            let ty = self.resolve_type_ref(p.ty);
            self.record_def_type(&p.name, ty);
        }
        for &stmt in &m.body {
            self.check_stmt(stmt);
        }
        // Kontratlar gövdeden SONRA: register/let tipleri artık kayıtlı.
        for c in &m.contracts {
            self.check_contract(c);
        }
        let mut diags = Vec::new();
        self.drivers.check_undriven_outputs(m, self.res, &mut diags);
        self.diagnostics.extend(diags);
    }

    /// Extern modül portlarını tipler (ADR-0047): domain çıkarımı saat
    /// portlarını `Ty::Clock` üzerinden tanır, K8 haritası kurulabilir.
    pub(super) fn type_extern_ports(&mut self, x: &ExternDecl) {
        for p in &x.ports {
            let ty = self.resolve_type_ref(p.ty);
            self.record_def_type(&p.name, ty);
        }
    }

    fn check_stmt(&mut self, stmt_idx: Idx<Stmt>) {
        let ast = self.ast;
        match &ast.stmts[stmt_idx].kind {
            StmtKind::Reg(r) => self.handle_reg(r),
            StmtKind::Let(l) => self.handle_let(l),
            StmtKind::Wire(w) => {
                let ty = self.resolve_type_ref(w.ty);
                self.record_def_type(&w.name, ty);
            }
            StmtKind::Instance(inst) => self.handle_instance(inst),
            StmtKind::On(on) => self.check_driver_block(on.body),
            StmtKind::Comb(block) => self.check_driver_block(*block),
            StmtKind::Assign(a) => self.check_assign(&a.lhs, a.rhs),
            StmtKind::For(f) => self.check_for(f),
            StmtKind::Expr(e) => {
                self.synth(*e);
            }
            StmtKind::Error => {}
        }
    }

    /// on/comb bloğu kendi sürücü grubunda denetlenir (§11.1).
    fn check_driver_block(&mut self, block: Idx<Block>) {
        let group = self.new_group();
        let prev = self.current_group.replace(group);
        self.check_block(block);
        self.current_group = prev;
    }

    /// §6: `reg` tipi ya bildirilir ya init'ten çıkarılır; salt literal
    /// init belirsizdir (E2012).
    fn handle_reg(&mut self, r: &RegDecl) {
        let ty = match r.ty {
            Some(t) => {
                let ty = self.resolve_type_ref(t);
                self.check(r.init, ty);
                ty
            }
            None => {
                let inferred = self.synth(r.init);
                if self.types.is_int_lit(inferred) {
                    self.error(
                        ErrorCode::E2012,
                        r.name.span,
                        lstr!(en: "cannot determine register type"; tr: "register tipi belirlenemiyor"),
                        lstr!(en: "type of literal initializer is ambiguous"; tr: "literal başlangıç tipi belirsiz"),
                        lstr!(en: "write the type as reg {} : u8 = ...", r.name.text; tr: "reg {} : u8 = ... şeklinde tip yazın", r.name.text),
                    );
                    self.types.error()
                } else {
                    // Register depolaması somut genişlik ister — esnek
                    // aritmetik sonucu doğal genişliğe sabitlenir.
                    self.types.concrete(inferred)
                }
            }
        };
        self.record_def_type(&r.name, ty);
    }

    /// §6: `let` tipi bildirilmişse check, değilse synth; soneksiz
    /// literal i32 varsayılır (W2012).
    fn handle_let(&mut self, l: &LetDecl) {
        let ty = match l.ty {
            Some(t) => {
                let ty = self.resolve_type_ref(t);
                self.check(l.value, ty);
                ty
            }
            None => {
                let ty = self.synth(l.value);
                if self.types.is_int_lit(ty) {
                    self.warning(
                        ErrorCode::W2012,
                        l.name.span,
                        lstr!(en: "type not specified, i32 assumed"; tr: "tip belirtilmedi, i32 varsayıldı"),
                        lstr!(en: "literal type could not be resolved from context"; tr: "literal tipi bağlamdan çözülemedi"),
                        lstr!(en: "make it explicit by writing let {} : i32 = ...", l.name.text; tr: "let {} : i32 = ... yazarak açık belirtin", l.name.text),
                    );
                    self.types.intern(Ty::SInt { width: 32 })
                } else {
                    ty
                }
            }
        };
        self.record_def_type(&l.name, ty);
    }

    fn check_for(&mut self, f: &ForStmt) {
        self.synth(f.start);
        self.synth(f.end);
        // Döngü değişkeni derleme zamanı tamsayısıdır (const-eval.md §8).
        let ty = self.types.int_lit();
        self.record_def_type(&f.var, ty);
        self.check_block(f.body);
    }

    fn check_block(&mut self, block_idx: Idx<Block>) {
        let ast = self.ast;
        let block = &ast.blocks[block_idx];
        for stmt in &block.stmts {
            self.check_block_stmt(stmt);
        }
        if let Some(tail) = block.tail {
            self.synth(tail);
        }
    }

    fn check_block_stmt(&mut self, stmt: &BlockStmt) {
        match stmt {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => self.check_assign(lhs, *rhs),
            BlockStmt::If(if_stmt) => self.check_if(if_stmt),
            BlockStmt::Match(m) => {
                self.synth(m.scrutinee);
                for arm in &m.arms {
                    self.check_arm(arm);
                }
            }
            BlockStmt::Let(l) => self.handle_let(l),
            BlockStmt::For(f) => self.check_for(f),
            BlockStmt::Error => {}
        }
    }

    fn check_if(&mut self, if_stmt: &IfStmt) {
        let bool_ty = self.types.bool_ty();
        self.check(if_stmt.cond, bool_ty);
        self.check_block(if_stmt.then_block);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.check_block(*b),
            Some(ElseBranch::If(nested)) => self.check_if(nested),
            None => {}
        }
    }

    pub(super) fn check_arm(&mut self, arm: &MatchArm) {
        if let Some(guard) = arm.guard {
            let bool_ty = self.types.bool_ty();
            self.check(guard, bool_ty);
        }
        match &arm.body {
            MatchArmBody::Block(b) => self.check_block(*b),
            MatchArmBody::Expr(e) => {
                self.synth(*e);
            }
        }
    }

    // ═══ Atama ve sürücü kaydı (§6, §11) ══════════════════════════

    fn check_assign(&mut self, lhs: &LValue, rhs: Idx<Expr>) {
        let (target, lhs_ty) = self.lvalue_type(lhs);
        self.check(rhs, lhs_ty);
        let Some(def) = target else { return };
        if !matches!(
            self.res.def_kind(def),
            DefKind::Port { .. } | DefKind::Register | DefKind::Wire | DefKind::LocalBinding
        ) {
            return;
        }
        let group = match self.current_group {
            Some(g) => g,
            None => self.new_group(),
        };
        self.drivers
            .record(def, lhs.span, group, !lhs.suffixes.is_empty());
    }

    fn lvalue_type(&mut self, lv: &LValue) -> (Option<DefId>, TypeId) {
        let def = self.res.use_spans.get(&lv.base.span).copied();
        let mut ty = match def {
            Some(d) => self.def_type(d),
            None => self.types.error(),
        };
        for suffix in &lv.suffixes {
            ty = match suffix {
                LValueSuffix::Index(e) => {
                    self.synth(*e);
                    self.index_result(ty, *e, lv.span)
                }
                LValueSuffix::Range { hi, lo } => self.range_result(ty, *hi, *lo, lv.span),
                LValueSuffix::PartSelect {
                    start,
                    width,
                    ascending,
                } => {
                    self.synth(*start);
                    self.synth(*width);
                    self.part_select_result(ty, *start, *width, *ascending, lv.span)
                }
                LValueSuffix::Field(name) => self.field_result(ty, name, lv.span),
            };
        }
        (def, ty)
    }

    fn new_group(&mut self) -> u32 {
        self.next_group += 1;
        self.next_group
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, def_ty};

    #[test]
    fn register_type_comes_from_annotation_or_initializer() {
        let src = "module M {\n    in  clk : clock\n    in  a : u8\n    out y : u8\n\n    reg r : u8 = 0\n    reg q = a\n\n    on clk {\n        r <= a\n        q <= a\n    }\n    y = r\n}\n";
        assert_eq!(def_ty(src, "r"), "u8");
        assert_eq!(def_ty(src, "q"), "u8");
    }

    #[test]
    fn bare_literal_register_is_ambiguous_e2012() {
        let src = "module M {\n    in  clk : clock\n    out y : u8\n\n    reg r = 0\n\n    on clk { r <= r }\n    y = 0\n}\n";
        assert!(codes(src).contains(&"E2012"));
    }

    #[test]
    fn bare_literal_let_defaults_to_i32_with_w2012() {
        let src = "module M {\n    in  a : u8\n    out y : u8\n\n    let _x = 42\n\n    y = a\n}\n";
        assert!(codes(src).contains(&"W2012"));
        assert_eq!(def_ty(src, "_x"), "i32");
    }

    #[test]
    fn assignment_checks_rhs_against_target_and_if_condition_is_bool() {
        let narrow = "module M {\n    in  a : u16\n    out y : u8\n\n    y = a\n}\n";
        assert!(codes(narrow).contains(&"E2001"));
        let cond = "module M {\n    in  a : u8\n    out y : u8\n\n    comb {\n        if a { y = a } else { y = 0 }\n    }\n}\n";
        assert!(codes(cond).contains(&"E2003"));
    }

    #[test]
    fn assignments_in_two_blocks_are_recorded_as_separate_drivers() {
        let src = "module M {\n    in  a : u8\n    out y : u8\n\n    comb { y = a }\n    comb { y = a }\n}\n";
        assert!(codes(src).contains(&"E4001"));
    }
}
