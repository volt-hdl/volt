//! Modül gövdesi deyimleri (name-resolution.md §5): reg/let/wire/
//! inst bildirimleri, atama hedefleri ve hedef yönü denetimleri
//! (E4005 ADR-0039, E4008 ADR-0051).

use volt_ast::{
    GenericArg, Idx, InstanceDecl, LValue, LValueSuffix, Name, OnTrigger, PortDir, Stmt, StmtKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::def::{DefId, DefKind};
use super::instance::InstanceTarget;
use super::scope::{ScopeId, ScopeKind};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn resolve_stmt(&mut self, stmt_idx: Idx<Stmt>, scope: ScopeId, module_def: DefId) {
        let stmt = &self.ast.stmts[stmt_idx];
        match &stmt.kind {
            StmtKind::Reg(r) => {
                if let Some(ty) = r.ty {
                    self.resolve_type(ty, scope);
                }
                if let Some(domain) = &r.domain {
                    self.resolve_domain_ref(&domain.clone(), scope);
                }
                self.resolve_expr(r.init, scope);
                self.declare_local(&r.name.clone(), DefKind::Register, scope);
            }
            StmtKind::Let(l) => {
                if let Some(ty) = l.ty {
                    self.resolve_type(ty, scope);
                }
                self.resolve_expr(l.value, scope);
                self.declare_local(&l.name.clone(), DefKind::LocalBinding, scope);
            }
            StmtKind::Wire(w) => {
                self.resolve_type(w.ty, scope);
                self.declare_local(&w.name.clone(), DefKind::Wire, scope);
            }
            StmtKind::Instance(inst) => self.resolve_instance_stmt(inst, scope, module_def),
            StmtKind::On(on) => {
                match &on.trigger {
                    OnTrigger::Clock(name) | OnTrigger::Reset(name) => {
                        let _ = self.resolve_simple(&name.clone(), scope, true);
                    }
                    OnTrigger::Error => {}
                }
                self.resolve_block(on.body, scope);
            }
            StmtKind::Comb(block) => self.resolve_block(*block, scope),
            StmtKind::Assign(a) => {
                self.resolve_lvalue(&a.lhs, scope);
                self.resolve_expr(a.rhs, scope);
            }
            StmtKind::For(f) => self.resolve_for(f, scope),
            StmtKind::Expr(e) => self.resolve_expr(*e, scope),
            StmtKind::Error => {}
        }
    }

    /// `inst ad = Hedef { port: değer, ... }` — bağlamalar ÖNCE çözülür,
    /// örnek adı sonra bildirilir (kendi bağlamalarında görünmez).
    fn resolve_instance_stmt(&mut self, inst: &InstanceDecl, scope: ScopeId, module_def: DefId) {
        let target = self.resolve_instance_target(&inst.module_path.clone(), scope);
        for arg in &inst.generic_args {
            match arg {
                GenericArg::Type(ty) => self.resolve_type(*ty, scope),
                GenericArg::Const(e) => self.resolve_expr(*e, scope),
            }
        }
        for b in &inst.bindings {
            match target {
                InstanceTarget::Module(def) => {
                    self.check_port_exists(def, &b.port_name.clone());
                }
                InstanceTarget::Builtin(prim) => {
                    self.check_builtin_binding(prim, &b.port_name.clone());
                }
                InstanceTarget::Unknown => {}
            }
            match b.value {
                Some(e) => self.resolve_expr(e, scope),
                // `clk:` kısayolu — yerel isim port adıyla aynı.
                None => {
                    let _ = self.resolve_simple(&b.port_name.clone(), scope, true);
                }
            }
        }
        let inst_def = self.declare_local(&inst.name.clone(), DefKind::Instance, scope);
        match target {
            InstanceTarget::Module(t) => {
                self.instance_module.insert(inst_def, t);
                self.instance_edges.push((module_def, t));
            }
            InstanceTarget::Builtin(prim) => {
                self.instance_builtin.insert(inst_def, prim);
            }
            InstanceTarget::Unknown => {}
        }
    }

    pub(super) fn resolve_for(&mut self, f: &volt_ast::ForStmt, scope: ScopeId) {
        self.resolve_expr(f.start, scope);
        self.resolve_expr(f.end, scope);
        let loop_scope = self.new_scope(ScopeKind::Loop, Some(scope));
        self.declare_checked(&f.var.clone(), DefKind::LoopVar, loop_scope, false);
        self.resolve_block(f.body, loop_scope);
    }

    /// E4005 (ADR-0039) — etkin yönü giriş olan bundle alanına atama.
    /// Beş parça: kod, konum, açıklama, öneri, ADR referansı (not).
    fn check_bundle_direction(&mut self, def: DefId, base: &Name) {
        let Some(origin) = self.bundle_inputs.get(&def).cloned() else {
            return;
        };
        let (port, field) = (origin.port.text.clone(), origin.path.clone());
        let declared = origin.declared.keyword();
        let opposite = if origin.flipped { "out" } else { "in" };
        let mut diag = Diagnostic::error(
            ErrorCode::E4005,
            lstr!(en: "cannot assign bundle field '{port}.{field}': its direction is input";
                  tr: "'{port}.{field}' bundle alanına atanamaz: yönü giriş"),
            LabeledSpan::primary(
                base.span,
                lstr!(en: "this field is driven from outside the module";
                      tr: "bu alan modülün dışından sürülür"),
            ),
            lstr!(en: "drive the fields that face outward, or declare the port as '{opposite} {port} : {}'",
                      origin.bundle;
                  tr: "dışa bakan alanları sürün ya da portu '{opposite} {port} : {}' olarak bildirin",
                      origin.bundle),
        )
        .with_secondary(
            origin.port.span,
            lstr!(en: "bundle port declared here"; tr: "bundle portu burada bildirildi"),
        );
        diag = if origin.flipped {
            diag.with_note(
                NoteKind::Reason,
                lstr!(en: "field '{field}' is declared '{declared}' in '{}', and an 'in' bundle port flips every field direction (ADR-0039)",
                          origin.bundle;
                      tr: "'{field}' alanı '{}' içinde '{declared}' bildirilmiş; 'in' bundle portu her alanın yönünü tersler (ADR-0039)",
                          origin.bundle),
            )
        } else {
            diag.with_note(
                NoteKind::Reason,
                lstr!(en: "field '{field}' is declared '{declared}' in '{}' (ADR-0039)", origin.bundle;
                      tr: "'{field}' alanı '{}' içinde '{declared}' bildirilmiş (ADR-0039)", origin.bundle),
            )
        };
        self.diagnostics.push(diag);
    }

    /// E4008 (ADR-0051) — çift yönlü porta doğrudan atama. Beş parça:
    /// kod, konum, açıklama, öneri (yön başına yöntem listesi), gerekçe
    /// notu + ikincil etiket (port bildirimi).
    fn check_bidir_assign(&mut self, def: DefId, base: &Name) {
        let Some(&dir) = self.bidir_ports.get(&def) else {
            return;
        };
        let name = base.text.clone();
        let decl_span = self.def(def).span;
        let (drive, mode) = match dir {
            PortDir::OpenDrain => ("drive_low()", "pulled low"),
            _ => ("drive(value)", "driven"),
        };
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E4008,
                lstr!(en: "cannot assign to {} port '{name}' directly", dir.keyword();
                      tr: "{} portu '{name}' doğrudan atanamaz", dir.keyword()),
                LabeledSpan::primary(
                    base.span,
                    lstr!(en: "this would drive the line push-pull"; tr: "bu hattı push-pull sürerdi"),
                ),
                lstr!(en: "inside on clk {{ ... }} write {name}.{drive} when the line must be {mode} and {name}.release() otherwise; read it with {name}.read()";
                      tr: "on clk {{ ... }} içinde hat {mode} olacaksa {name}.{drive}, değilse {name}.release() yazın; {name}.read() ile okuyun"),
            )
            .with_secondary(
                decl_span,
                lstr!(en: "bidirectional port declared here"; tr: "çift yönlü port burada bildirildi"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "a bidirectional pad is shared with other devices: the module either drives it or leaves it high-impedance, so the drive state is a register and the tri-state buffer is generated by the compiler (ADR-0051)";
                      tr: "çift yönlü pad başka aygıtlarla paylaşılır: modül ya sürer ya da yüksek empedansta bırakır; sürücü durumu register'dır, üç durumlu tamponu derleyici üretir (ADR-0051)"),
            ),
        );
    }

    pub(super) fn resolve_lvalue(&mut self, lv: &LValue, scope: ScopeId) {
        let def = self.resolve_simple(&lv.base.clone(), scope, false);
        self.check_bundle_direction(def, &lv.base.clone());
        self.check_bidir_assign(def, &lv.base.clone());
        let mut current = Some(def);
        for suffix in &lv.suffixes {
            match suffix {
                LValueSuffix::Index(e) => {
                    self.resolve_expr(*e, scope);
                    current = None;
                }
                LValueSuffix::Range { hi, lo } => {
                    self.resolve_expr(*hi, scope);
                    self.resolve_expr(*lo, scope);
                    current = None;
                }
                LValueSuffix::PartSelect { start, width, .. } => {
                    self.resolve_expr(*start, scope);
                    self.resolve_expr(*width, scope);
                    current = None;
                }
                LValueSuffix::Field(field) => {
                    // uart.busy hedefi: instance portu doğrulanabilir.
                    if let Some(base) = current {
                        if let Some(&target) = self.instance_module.get(&base) {
                            self.check_port_exists(target, &field.clone());
                        } else if let Some(&prim) = self.instance_builtin.get(&base) {
                            self.check_builtin_field(prim, &field.clone());
                        }
                    }
                    current = None;
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
    fn declaration_statements_bind_their_kinds() {
        let r = resolved(
            "module M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    \
             reg r : u8 = 0\n    wire w : u8\n    on clk { r <= a }\n    w = r\n    y = w\n}\n",
        );
        assert!(r.error_codes().is_empty(), "{:?}", r.error_codes());
        assert_eq!(r.def_by_name("r").expect("r").1.kind, DefKind::Register);
        assert_eq!(r.def_by_name("w").expect("w").1.kind, DefKind::Wire);
    }

    #[test]
    fn direct_assignment_to_a_bidirectional_port_is_e4008() {
        let c =
            codes("module M {\n    in clk : clock\n    inout dq : u8\n    on clk { dq <= 0 }\n}\n");
        assert!(c.contains(&"E4008"), "{c:?}");
    }

    #[test]
    fn instance_name_is_not_visible_in_its_own_bindings() {
        let c = codes(
            "module Alt { in a : u8 out s : u8 s = a }
             module Top { in x : u8 out y : u8 let u = Alt { a: u } y = u.s }",
        );
        assert!(c.contains(&"E1002"), "{c:?}");
    }
}
