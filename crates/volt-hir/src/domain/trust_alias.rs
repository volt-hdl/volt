//! K11 (ADR-0052) — güven takma adı. Saat alanı çıkarımının güven
//! seviyeleriyle TEK temas noktası: trust_level yazılmış, clock portu
//! taşımayan `@Ad` yeni saat alanı açmaz. Güven kafesinin kendisi
//! (kısmi sıralama, declassify, E3009) `crate::trust`'tadır; saat
//! `join`'i ile kod paylaşmaz — iki cebir farklıdır (saat alanında
//! farklı iki eleman hatadır, güvende üst sınır alınır).

use std::collections::HashSet;

use volt_ast::Name;

use super::{DomainId, Inferencer};
use crate::resolve::{DefId, DefKind};

impl Inferencer<'_> {
    /// K11 (ADR-0052) — saat dışı sinyalin `@Ad` anotasyonu. `Ad` bir
    /// `trust_level` taşıyan domain bildirimiyse ve bu modülün hiçbir
    /// clock portu onu taşımıyorsa yeni saat alanı AÇMAZ: sinyal saat
    /// boyutunda modülün tek alanında (K2) kalır, anotasyon yalnız güven
    /// boyutunu belirler. Çoklu saatte böyle bir anotasyon hangi saate
    /// ait olduğunu söylemez → E3010. trust_level'sız bildirimler için
    /// davranış değişmez (geriye uyumluluk).
    pub(super) fn signal_annotation_domain(&mut self, ann: &Name) -> DomainId {
        let Some(def) = self.use_def(ann.span) else {
            return DomainId::Error;
        };
        if self.is_trust_only_alias(def, &self.anchored.clone()) {
            if self.multi_clock {
                self.err_ambiguous(ann, false);
                return DomainId::Error;
            }
            return self.default_domain;
        }
        self.annotation_domain(ann)
    }

    /// `def`, trust_level yazılmış bir domain bildirimi mi ve `anchored`
    /// (bir modülün clock portlarının taşıdığı bildirimler) dışında mı?
    pub(super) fn is_trust_only_alias(&self, def: DefId, anchored: &HashSet<DefId>) -> bool {
        self.res.def_kind(def) == DefKind::Domain
            && self
                .by_decl
                .get(&def)
                .is_some_and(|&id| self.domains[id as usize].trust.is_some())
            && !anchored.contains(&def)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::inferred;
    use super::super::DomainId;

    const KEYS: &str =
        "domain Keys { clock = posedge, reset = sync active_high, trust_level = secret }\n";

    #[test]
    fn trust_only_alias_stays_in_the_single_clock_domain() {
        let r = inferred(&format!(
            "{KEYS}module M {{\n    in clk : clock\n    in key : u8 @Keys\n    in d : u8\n}}\n"
        ));
        assert_eq!(r.domain_name_of("key"), Some("clk"));
        assert_eq!(r.domain_of("key"), r.domain_of("d"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn trust_only_alias_is_ambiguous_with_several_clocks() {
        let r = inferred(&format!(
            "{KEYS}module M {{\n    in c1 : clock\n    in c2 : clock\n    in key : u8 @Keys\n}}\n"
        ));
        assert_eq!(r.codes(), ["E3010"]);
        assert_eq!(r.domain_of("key"), DomainId::Error);
    }

    #[test]
    fn anchored_trust_domain_is_a_real_clock_domain() {
        let r = inferred(&format!(
            "{KEYS}module M {{\n    in clk : clock @Keys\n    in c2 : clock\n    in key : u8 @Keys\n}}\n"
        ));
        assert_eq!(r.domain_name_of("key"), Some("Keys"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn domain_without_trust_level_keeps_opening_a_clock_domain() {
        let r = inferred(
            "domain Plain { clock = posedge, reset = sync active_high }\n\
             module M {\n    in clk : clock\n    in a : u8 @Plain\n}\n",
        );
        assert_eq!(r.domain_name_of("a"), Some("Plain"));
    }

    #[test]
    fn alias_on_an_instance_target_port_falls_back_to_its_single_clock() {
        let r = inferred(&format!(
            "{KEYS}module Inner {{\n    in clk : clock\n    in key : u8 @Keys\n}}\n\
             module Top {{\n    in clk : clock\n    in k : u8\n    let i = Inner {{ clk: clk, key: k }}\n}}\n"
        ));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }
}
