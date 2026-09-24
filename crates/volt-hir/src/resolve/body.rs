//! Geçiş 2 — öğe gövdeleri (name-resolution.md §4, §5): fonksiyon,
//! struct, enum, const, alias ve modül gövdesi. Modül gövdesi SIRALIDIR;
//! ileride bildirilecek adlar E1002 için `pending` yığınında izlenir.

use std::collections::{HashMap, HashSet};

use volt_ast::{
    FnDecl, GenericParam, GenericParamKind, Idx, Item, ItemKind, ModuleDecl, Name, PortDir,
    StmtKind,
};
use volt_span::Span;

use super::def::{DefId, DefKind};
use super::scope::{ScopeId, ScopeKind};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn resolve_item_body(&mut self, item_idx: Idx<Item>) {
        let item = &self.ast.items_arena[item_idx];
        match &item.kind {
            ItemKind::Module(m) => self.resolve_module_body(m),
            ItemKind::Domain(d) => {
                for field in &d.fields {
                    if let volt_ast::DomainValue::Literal(expr) = &field.value {
                        self.resolve_expr(*expr, self.root);
                    }
                }
            }
            ItemKind::Fn(f) => self.resolve_fn_body(f),
            ItemKind::Struct(s) => {
                let scope = self.new_scope(ScopeKind::Block, Some(self.root));
                self.declare_generics(&s.generics, scope);
                for field in &s.fields {
                    self.resolve_type(field.ty, scope);
                }
            }
            ItemKind::Enum(e) => {
                if let Some(repr) = e.repr {
                    self.resolve_type(repr, self.root);
                }
                for v in &e.variants {
                    if let Some(disc) = v.discriminant {
                        self.resolve_expr(disc, self.root);
                    }
                }
            }
            ItemKind::Const(c) => {
                self.resolve_type(c.ty, self.root);
                self.resolve_expr(c.value, self.root);
            }
            ItemKind::TypeAlias(t) => {
                let scope = self.new_scope(ScopeKind::Block, Some(self.root));
                self.declare_generics(&t.generics, scope);
                self.resolve_type(t.target, scope);
            }
            ItemKind::Extern(x) => self.resolve_extern_body(x),
            // Gövdesi modül arenalarını kullanmaz (ADR-0033).
            ItemKind::Test(_) => {}
            ItemKind::Error => {}
        }
    }

    fn resolve_fn_body(&mut self, f: &FnDecl) {
        let fn_def = self.lookup_item_def(&f.name.text);
        let scope = self.new_scope(ScopeKind::Function(fn_def), Some(self.root));
        self.declare_generics(&f.generics, scope);
        for p in &f.params {
            self.declare_checked(&p.name.clone(), DefKind::LocalBinding, scope, false);
            self.resolve_type(p.ty, scope);
        }
        if let Some(ret) = f.return_ty {
            self.resolve_type(ret, scope);
        }
        self.in_contract = true;
        for c in &f.contracts {
            self.resolve_expr(c.expr, scope);
        }
        self.in_contract = false;
        self.resolve_block(f.body, scope);
    }

    pub(super) fn lookup_item_def(&self, name: &str) -> DefId {
        self.scope_get(self.root, name).unwrap_or(self.error_def)
    }

    /// Generic parametreler kapsamın İLK bildirimleridir (port ve gövde
    /// onları görür).
    pub(super) fn declare_generics(&mut self, generics: &[GenericParam], scope: ScopeId) {
        for g in generics {
            match &g.kind {
                GenericParamKind::Type { name, .. } => {
                    self.declare_checked(&name.clone(), DefKind::GenericParam, scope, false);
                }
                GenericParamKind::Const { name, ty } => {
                    let (name, ty) = (name.clone(), *ty);
                    self.declare_checked(&name, DefKind::GenericParam, scope, false);
                    self.resolve_type(ty, scope);
                }
            }
        }
    }

    fn resolve_module_body(&mut self, m: &ModuleDecl) {
        let module_def = self.lookup_item_def(&m.name.text);
        let scope = self.new_scope(ScopeKind::Module(module_def), Some(self.root));

        // 1. Generic parametreler (en önce).
        self.declare_generics(&m.generics, scope);

        // 2. Portlar — birbirini görebilir, sıra anlamsal bilgi taşımaz.
        self.declare_ports(m, scope);

        // 3. Gövde — SIRALI; ileride bildirilecekler E1002 için izlenir.
        self.resolve_module_stmts(m, scope, module_def);

        // 4. Kontratlar — bildirim sırasından bağımsızdır, gövdeden SONRA
        //    çözülür ki invariant/cover register ve let'leri görebilsin.
        //    Tür bazlı kapsam kısıtı (requires → yalnız port) typeck'te.
        self.in_contract = true;
        for c in &m.contracts {
            self.resolve_expr(c.expr, scope);
        }
        self.in_contract = false;
    }

    fn declare_ports(&mut self, m: &ModuleDecl, scope: ScopeId) {
        self.bundle_origins = m.ports.iter().filter_map(|p| p.bundle.clone()).collect();
        for p in &m.ports {
            let def = self.declare_checked(
                &p.name.clone(),
                DefKind::Port { dir: p.direction },
                scope,
                false,
            );
            if let (PortDir::In, Some(origin)) = (p.direction, &p.bundle) {
                self.bundle_inputs.insert(def, origin.clone());
            }
            if p.direction.is_bidirectional() {
                self.bidir_ports.insert(def, p.direction);
            }
        }
        for p in &m.ports {
            self.resolve_type(p.ty, scope);
            if let Some(domain) = &p.domain {
                self.resolve_domain_ref(&domain.clone(), scope);
            }
        }
    }

    fn resolve_module_stmts(&mut self, m: &ModuleDecl, scope: ScopeId, module_def: DefId) {
        // ADR-0051: parser'ın sentezlediği sürücü register'ları SV'de
        // üç durumlu tampon tarafından okunur — kaynakta okunmasa da
        // W1004/W4002 "yazılıp okunmayan" sayılmaz.
        let synth_reads: HashSet<String> = m
            .ports
            .iter()
            .filter_map(|p| p.direction.bidir_regs(&p.name.text))
            .flat_map(|r| std::iter::once(r.enable).chain(r.data))
            .collect();

        let mut later: HashMap<String, Span> = HashMap::new();
        for &stmt in &m.body {
            if let Some(name) = declared_name(&self.ast.stmts[stmt].kind) {
                later.entry(name.text.clone()).or_insert(name.span);
            }
        }
        self.pending.push(later);
        for &stmt in &m.body {
            self.resolve_stmt(stmt, scope, module_def);
            if let Some(name) = declared_name(&self.ast.stmts[stmt].kind) {
                if synth_reads.contains(&name.text) {
                    if let Some(&def) = self.decl_spans.get(&name.span) {
                        self.reads.insert(def);
                    }
                }
            }
        }
        self.pending.pop();
    }
}

/// Modül gövdesinde isim bildiren deyimler (E1002 ön taraması).
fn declared_name(kind: &StmtKind) -> Option<&Name> {
    match kind {
        StmtKind::Reg(r) => Some(&r.name),
        StmtKind::Let(l) => Some(&l.name),
        StmtKind::Wire(w) => Some(&w.name),
        StmtKind::Instance(i) => Some(&i.name),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::codes;

    #[test]
    fn generic_parameters_are_visible_to_ports_and_body() {
        let src = "module Delay<const N: u32> {\n    in clk : clock\n    in x : u8\n    out y : u8\n    \
                   reg line : [u8; N] = [0; N]\n    on clk { line[0] <= x }\n    y = line[N - 1]\n}\n";
        assert!(codes(src).is_empty(), "{:?}", codes(src));
    }

    #[test]
    fn ports_see_each_other_regardless_of_order() {
        let c = codes("module M { in d : u8 @clk in clk : clock out y : u8 y = d }");
        assert!(!c.contains(&"E3002") && !c.contains(&"E1002"), "{c:?}");
    }

    #[test]
    fn module_body_is_sequential() {
        let c = codes("module M { in a : u8 out y : u8 y = t let t = a }");
        assert_eq!(c, ["E1002", "W1001"]);
    }
}
