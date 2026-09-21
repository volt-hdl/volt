//! Kapsam hiyerarşisi (name-resolution.md §2): kapsam ağacı, bildirim,
//! çift tanım (E1003), gölgeleme (§6, W1002/W1003) ve zincir araması.

use std::collections::HashMap;

use volt_ast::Name;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use super::def::{DefData, DefId, DefKind};
use super::Resolver;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScopeId(pub u32);

#[derive(Debug)]
pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub bindings: HashMap<String, DefId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScopeKind {
    Root,
    Prelude,
    Module(DefId),
    /// `extern module` port kapsamı (ADR-0047) — kullanım raporu
    /// (W1001) bu kapsamı atlar: portlar dış SV modülüne aittir.
    Extern(DefId),
    Function(DefId),
    Block,
    Loop,
    MatchArm,
}

impl Resolver<'_> {
    pub(super) fn def(&self, id: DefId) -> &DefData {
        &self.defs[id.0 as usize]
    }

    pub(super) fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.0 as usize]
    }

    /// Adı kapsama bağlar (aynı ad varsa üzerine yazar — çift tanım
    /// denetimi çağıranın işidir).
    pub(super) fn bind(&mut self, scope: ScopeId, name: &str, def: DefId) {
        self.scopes[scope.0 as usize]
            .bindings
            .insert(name.to_string(), def);
    }

    pub(super) fn new_scope(&mut self, kind: ScopeKind, parent: Option<ScopeId>) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(Scope {
            kind,
            parent,
            bindings: HashMap::new(),
        });
        id
    }

    pub(super) fn add_def(
        &mut self,
        kind: DefKind,
        name: &str,
        span: Span,
        scope: ScopeId,
        is_public: bool,
    ) -> DefId {
        let id = DefId(self.defs.len() as u32);
        self.defs.push(DefData {
            kind,
            name: name.to_string(),
            span,
            scope,
            is_public,
        });
        id
    }

    pub(super) fn declare(
        &mut self,
        name: &Name,
        kind: DefKind,
        scope: ScopeId,
        is_public: bool,
    ) -> DefId {
        let def = self.add_def(kind, &name.text, name.span, scope, is_public);
        self.bind(scope, &name.text, def);
        self.decl_spans.insert(name.span, def);
        def
    }

    /// Aynı kapsamda çift tanım E1003; dış kapsam gölgelemesi W1002/W1003.
    pub(super) fn declare_checked(
        &mut self,
        name: &Name,
        kind: DefKind,
        scope: ScopeId,
        is_public: bool,
    ) -> DefId {
        if self.report_duplicate(name, scope) {
            return self.error_def;
        }
        self.warn_shadowing(name, scope);
        self.declare(name, kind, scope, is_public)
    }

    /// Sıralı gövdede (modül/blok) bildirim: ad artık "ileride
    /// bildirilecek" değildir (E1002 listesinden düşer), sonra denetimli
    /// bildirim yapılır.
    pub(super) fn declare_local(&mut self, name: &Name, kind: DefKind, scope: ScopeId) -> DefId {
        if let Some(top) = self.pending.last_mut() {
            top.remove(&name.text);
        }
        self.declare_checked(name, kind, scope, false)
    }

    /// Aynı kapsamda çift tanım E1003 (true → çağıran bildirimden vazgeçer).
    pub(super) fn report_duplicate(&mut self, name: &Name, scope: ScopeId) -> bool {
        if let Some(&prev) = self.scope(scope).bindings.get(&name.text) {
            let prev_span = self.def(prev).span;
            self.diagnostics.push(
                Diagnostic::error(
                    ErrorCode::E1003,
                    lstr!(en: "'{}' is already defined in this scope", name.text;
                          tr: "'{}' bu kapsamda zaten tanımlı", name.text),
                    LabeledSpan::primary(
                        name.span,
                        lstr!(en: "second definition here"; tr: "ikinci tanım burada"),
                    ),
                    lstr!(en: "use a different name — two signals cannot share the same name in hardware";
                          tr: "farklı bir isim kullanın — donanımda iki sinyal aynı adı taşıyamaz"),
                )
                .with_secondary(
                    prev_span,
                    lstr!(en: "previous definition here"; tr: "önceki tanım burada"),
                ),
            );
            return true;
        }
        false
    }

    /// Dış kapsam gölgelemesi W1002 / yerleşik gölgeleme W1003.
    fn warn_shadowing(&mut self, name: &Name, scope: ScopeId) {
        if let Some(outer) = self.lookup_in_parents(&name.text, scope) {
            let outer_data = &self.def(outer);
            if let DefKind::Builtin(_) = outer_data.kind {
                self.diagnostics.push(Diagnostic::warning(
                    ErrorCode::W1003,
                    lstr!(en: "builtin '{}' is shadowed", name.text;
                          tr: "yerleşik '{}' gölgeleniyor", name.text),
                    LabeledSpan::primary(
                        name.span,
                        lstr!(en: "this definition hides the builtin";
                              tr: "bu tanım yerleşiği gizler"),
                    ),
                    lstr!(en: "choose a different name — the builtin function becomes inaccessible in this scope";
                          tr: "farklı bir isim seçin — yerleşik fonksiyon bu kapsamda erişilmez olur"),
                ));
            } else if !matches!(outer_data.kind, DefKind::Error | DefKind::Import) {
                let outer_span = outer_data.span;
                self.diagnostics.push(
                    Diagnostic::warning(
                        ErrorCode::W1002,
                        lstr!(en: "'{}' shadows a definition in an outer scope", name.text;
                              tr: "'{}' dış kapsamdaki tanımı gölgeliyor", name.text),
                        LabeledSpan::primary(
                            name.span,
                            lstr!(en: "inner definition here"; tr: "iç tanım burada"),
                        ),
                        lstr!(en: "use a different name to avoid confusion";
                              tr: "karışıklığı önlemek için farklı bir isim kullanın"),
                    )
                    .with_secondary(
                        outer_span,
                        lstr!(en: "shadowed definition here"; tr: "gölgelenen tanım burada"),
                    ),
                );
            }
        }
    }

    fn lookup_in_parents(&self, name: &str, scope: ScopeId) -> Option<DefId> {
        let parent = self.scope(scope).parent?;
        self.lookup_visible(name, parent)
    }

    /// Kapsam zincirinde (kendisi dahil) görünür ilk tanım — tanı
    /// üretmeyen sessiz arama.
    pub(super) fn lookup_visible(&self, name: &str, scope: ScopeId) -> Option<DefId> {
        let mut current = Some(scope);
        while let Some(s) = current {
            if let Some(def) = self.scope_get(s, name) {
                return Some(def);
            }
            current = self.scope(s).parent;
        }
        None
    }

    /// Görünür adlar — DETERMİNİSTİK sırada: içten dışa kapsam, kapsam
    /// içinde DefId (bildirim) sırası. `closest_match` eşit uzaklıkta ilk
    /// adayı seçtiğinden öneri `HashMap` sırasına sızmamalıdır.
    pub(super) fn visible_names(&self, scope: ScopeId) -> Vec<String> {
        let mut names = Vec::new();
        let mut current = Some(scope);
        while let Some(s) = current {
            let mut bound: Vec<(DefId, &String)> = self
                .scope(s)
                .bindings
                .iter()
                .map(|(name, &def)| (def, name))
                .collect();
            bound.sort_unstable();
            names.extend(bound.into_iter().map(|(_, name)| name.clone()));
            current = self.scope(s).parent;
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, resolved};
    use super::super::DefKind;

    #[test]
    fn duplicate_in_the_same_scope_is_e1003_and_keeps_the_first_definition() {
        let r = resolved("module M { in a : u8 in a : bool out y : u8 y = a }");
        assert_eq!(r.error_codes(), ["E1003"]);
        let ports = r.defs.iter().filter(|d| d.name == "a").count();
        assert_eq!(ports, 1);
    }

    #[test]
    fn inner_binding_shadowing_an_outer_definition_is_w1002() {
        let c = codes("module M { in x : u8 out y : u8 y = match x { x => x } }");
        assert!(c.contains(&"W1002"), "{c:?}");
    }

    #[test]
    fn shadowing_a_builtin_is_w1003_not_w1002() {
        let c = codes("module M { in a : u8 out y : u8 let sync = a y = sync }");
        assert!(c.contains(&"W1003") && !c.contains(&"W1002"), "{c:?}");
    }

    #[test]
    fn declared_names_are_bound_with_their_kind() {
        let r = resolved("module M { in a : u8 out y : u8 let t = a y = t }");
        let (_, t) = r.def_by_name("t").expect("t");
        assert_eq!(t.kind, DefKind::LocalBinding);
    }
}
