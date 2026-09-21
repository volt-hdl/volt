//! Import görünürlüğü. Tek dosyada `use` adları kök kapsama
//! `DefKind::Import` olarak kaydedilir (E1010); derleme biriminde
//! (ADR-0042, §3.3) kayıt `unit::check_imports`'ta yapılmıştır, burada
//! yalnız kök kapsam araması dosya başına süzülür.

use volt_ast::{Name, UseTree};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use super::def::{synthetic_span, DefId, DefKind};
use super::scope::ScopeId;
use super::Resolver;
use crate::unit::mono_base;

impl Resolver<'_> {
    pub(super) fn collect_imports(&mut self) {
        let mut names: Vec<Name> = Vec::new();
        for use_decl in &self.ast.uses {
            match &use_decl.tree {
                Some(UseTree::Alias(alias)) => names.push(alias.clone()),
                Some(UseTree::List(paths)) => {
                    for p in paths {
                        if let Some(last) = p.segments.last() {
                            names.push(last.clone());
                        }
                    }
                }
                // Glob: hangi isimlerin geldiği bilinemez — F1b'de izlenmez.
                Some(UseTree::Glob) => {}
                None => {
                    if let Some(last) = use_decl.path.segments.last() {
                        names.push(last.clone());
                    }
                }
            }
        }
        for name in names {
            if let Some(&prev) = self.scope(self.root).bindings.get(&name.text) {
                let prev_span = self.def(prev).span;
                self.diagnostics.push(
                    Diagnostic::error(
                        ErrorCode::E1010,
                        lstr!(en: "ambiguous import: '{}' is brought in twice", name.text;
                              tr: "belirsiz import: '{}' iki kez getiriliyor", name.text),
                        LabeledSpan::primary(
                            name.span,
                            lstr!(en: "second import here"; tr: "ikinci import burada"),
                        ),
                        lstr!(en: "give one of them an alias with 'as': use path::item as NewName";
                              tr: "birine 'as' ile takma ad verin: use yol::öğe as YeniAd"),
                    )
                    .with_secondary(
                        prev_span,
                        lstr!(en: "first import here"; tr: "ilk import burada"),
                    ),
                );
                continue;
            }
            self.declare(&name, DefKind::Import, self.root, false);
        }
    }

    /// Kapsamda ad araması. Kök kapsamda derleme birimi süzgeci uygulanır
    /// (ADR-0042): başka dosyanın öğesi yalnız import edilmişse görünür;
    /// `use a::B as C` takma adı kök addan önce çözülür. Monomorfize
    /// örnekler (`Fifo_8_16`) şablon adının görünürlüğünü devralır.
    pub(super) fn scope_get(&self, scope: ScopeId, name: &str) -> Option<DefId> {
        let bindings = &self.scope(scope).bindings;
        let Some(scopes) = self.file_scopes.filter(|_| scope == self.root) else {
            return bindings.get(name).copied();
        };
        let file_scope = scopes.get(&self.current_file);
        let target = file_scope
            .and_then(|s| s.aliases.get(name))
            .map_or(name, String::as_str);
        let def = bindings.get(target).copied()?;
        let data = &self.def(def);
        if data.span.file == self.current_file || data.span == synthetic_span() {
            return Some(def);
        }
        let visible = file_scope
            .is_some_and(|s| s.visible.contains(name) || s.visible.contains(mono_base(name)));
        visible.then_some(def)
    }
}

#[cfg(test)]
mod unit_mode_tests {
    use crate::unit::mono_base;

    #[test]
    fn mono_base_strips_numeric_suffixes_only() {
        assert_eq!(mono_base("Fifo_8_16"), "Fifo");
        assert_eq!(mono_base("Fifo"), "Fifo");
        assert_eq!(mono_base("uart_tx"), "uart_tx");
        assert_eq!(mono_base("Shift_reg_4"), "Shift_reg");
    }

    #[test]
    fn mono_base_keeps_trailing_underscore_names() {
        assert_eq!(mono_base("x_"), "x_");
        assert_eq!(mono_base("_"), "_");
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, resolved};
    use super::super::DefKind;

    #[test]
    fn single_file_use_registers_an_import_in_the_root_scope() {
        let r = resolved("use paket::SABIT;\nmodule M { in a : u8 out y : u8 y = a + SABIT }");
        assert!(r.error_codes().is_empty(), "{:?}", r.error_codes());
        let (_, import) = r.def_by_name("SABIT").expect("SABIT");
        assert_eq!(import.kind, DefKind::Import);
    }

    #[test]
    fn importing_the_same_name_twice_is_e1010() {
        let c = codes("use a::Ortak;\nuse b::Ortak;\nmodule M { in x : u8 out y : u8 y = x }");
        assert!(c.contains(&"E1010"), "{c:?}");
    }
}
