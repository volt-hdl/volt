//! K1 — açık anotasyon: `@Ad` ve `reg(clk)` bir saat alanına çevrilir.

use volt_ast::Name;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use super::{DomainId, Inferencer};
use crate::resolve::DefKind;

impl Inferencer<'_> {
    /// `@Ad` veya `reg(clk)` anotasyonunu domain'e çevirir (K1).
    /// Çözülemeyen isim E3002'yi isim çözümlemede almıştır → Error.
    pub(super) fn annotation_domain(&mut self, name: &Name) -> DomainId {
        let Some(def) = self.use_def(name.span) else {
            return DomainId::Error;
        };
        match self.res.def_kind(def) {
            DefKind::Domain => match self.by_decl.get(&def) {
                Some(&id) => DomainId::Explicit(id),
                None => DomainId::Error,
            },
            DefKind::Port { .. } if self.is_clock_def(def) => self
                .signal_domains
                .get(&def)
                .copied()
                .unwrap_or(DomainId::Error),
            // İsim çözümleme portları `reg(clk)` için kabul eder; clock
            // tipinde olmayan port burada yakalanır.
            DefKind::Port { .. } => {
                self.err_annotation_not_clock(name);
                DomainId::Error
            }
            // Diğer türler için E3002 isim çözümlemede üretildi.
            _ => DomainId::Error,
        }
    }

    /// E3002 — `@port` anotasyonu clock tipinde olmayan bir porta işaret
    /// ediyor (modül ve extern ortak).
    pub(super) fn err_annotation_not_clock(&mut self, name: &Name) {
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E3002,
            lstr!(
                en: "'{}' is not a clock domain", name.text;
                tr: "'{}' bir saat alanı değil", name.text
            ),
            LabeledSpan::primary(
                name.span,
                lstr!(en: "not of clock type"; tr: "clock tipinde değil"),
            ),
            lstr!(
                en: "use a port of clock type or a domain definition";
                tr: "clock tipinde bir port ya da domain tanımı kullanın"
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};
    use super::super::DomainId;

    #[test]
    fn domain_annotation_selects_the_declared_domain() {
        let r = inferred(&two_clock("    in a : u8 @Fast\n    in b : u8 @Slow"));
        assert_eq!(r.domain_name_of("a"), Some("Fast"));
        assert_eq!(r.domain_name_of("b"), Some("Slow"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn reg_clock_annotation_takes_the_domain_of_the_clock_port() {
        let r = inferred(&two_clock(
            "    in d : u8 @Slow\n    reg(slow_clk) r : u8 = 0\n    on slow_clk {\n        r <= d\n    }",
        ));
        assert_eq!(r.domain_name_of("r"), Some("Slow"));
    }

    #[test]
    fn annotation_pointing_at_a_non_clock_port_is_e3002() {
        let r = inferred(
            "module M {\n    in clk : clock\n    in d : u8\n    reg(d) r : u8 = 0\n    on clk {\n        r <= d\n    }\n}\n",
        );
        assert_eq!(r.count("E3002"), 1, "{:?}", r.codes());
        assert_eq!(r.domain_of("r"), DomainId::Error);
    }
}
