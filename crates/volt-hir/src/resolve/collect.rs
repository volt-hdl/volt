//! Geçiş 1 — öğe toplama (name-resolution.md §4): kök kapsamdaki her
//! öğe gövdelerden ÖNCE bildirilir, böylece ileri referans serbesttir.

use volt_ast::{EnumDecl, Idx, Item, ItemKind, Visibility};

use super::def::{DefId, DefKind};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn collect_item(&mut self, item_idx: Idx<Item>) {
        let item = &self.ast.items_arena[item_idx];
        let is_public = item.visibility == Visibility::Public;
        let def = match &item.kind {
            ItemKind::Module(m) => {
                Some(self.declare_checked(&m.name.clone(), DefKind::Module, self.root, is_public))
            }
            ItemKind::Domain(d) => {
                Some(self.declare_checked(&d.name.clone(), DefKind::Domain, self.root, is_public))
            }
            ItemKind::Fn(f) => {
                Some(self.declare_checked(&f.name.clone(), DefKind::Function, self.root, is_public))
            }
            ItemKind::Struct(s) => {
                Some(self.declare_checked(&s.name.clone(), DefKind::Struct, self.root, is_public))
            }
            ItemKind::Enum(e) => Some(self.collect_enum(e, is_public)),
            ItemKind::Const(c) => {
                let def =
                    self.declare_checked(&c.name.clone(), DefKind::Const, self.root, is_public);
                self.const_inits.insert(def, c.value);
                Some(def)
            }
            ItemKind::TypeAlias(t) => Some(self.declare_checked(
                &t.name.clone(),
                DefKind::TypeAlias,
                self.root,
                is_public,
            )),
            ItemKind::Extern(x) => Some(self.declare_checked(
                &x.name.clone(),
                DefKind::ExternModule,
                self.root,
                is_public,
            )),
            // Test blokları isim alanına ad eklemez (ADR-0033);
            // doğrulamaları sim::check_tests yapar.
            ItemKind::Test(_) => None,
            ItemKind::Error => None,
        };
        if let Some(def) = def {
            self.item_of_def.insert(def, item_idx);
        }
    }

    /// Varyantlar kök kapsama BAĞLANMAZ — yalnız `Enum::Varyant` yoluyla
    /// (`enum_variants`) erişilir.
    fn collect_enum(&mut self, e: &EnumDecl, is_public: bool) -> DefId {
        let name = e.name.clone();
        let enum_def = self.declare_checked(&name, DefKind::Enum, self.root, is_public);
        let mut variants = Vec::new();
        for (i, v) in e.variants.iter().enumerate() {
            let v_def = self.add_def(
                DefKind::EnumVariant { parent: enum_def },
                &v.name.text,
                v.name.span,
                self.root,
                is_public,
            );
            self.variant_info.insert(v_def, (i, v.discriminant));
            variants.push((v.name.text.clone(), v_def));
        }
        self.enum_variants.insert(enum_def, variants);
        enum_def
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::resolved;
    use super::super::DefKind;

    #[test]
    fn items_are_declared_before_any_body_is_resolved() {
        let r = resolved(
            "module Top { in x : u8 out y : u9 let u = Alt { a: x } y = u.s }
             module Alt { in a : u8 out s : u9 s = a + 1 }",
        );
        assert!(r.error_codes().is_empty(), "{:?}", r.error_codes());
        let (alt, _) = r.def_by_name("Alt").expect("Alt");
        assert!(r.item_of_def.contains_key(&alt));
    }

    #[test]
    fn enum_variants_record_parent_order_and_discriminant() {
        let r = resolved(
            "enum Durum : bits<2> { Bekle = 0, Calis = 1 }
             module M { in a : u8 out y : u8 y = a }",
        );
        let (durum, _) = r.def_by_name("Durum").expect("Durum");
        let (calis, data) = r.def_by_name("Calis").expect("Calis");
        assert_eq!(data.kind, DefKind::EnumVariant { parent: durum });
        let (order, discriminant) = r.variant_info[&calis];
        assert_eq!(order, 1);
        assert!(discriminant.is_some());
    }
}
