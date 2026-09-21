//! Domain tablosu (§1): `domain` bildirimleri, anotasyonsuz clock
//! portunun örtük alanı (K2) ve bütün kuralların paylaştığı sorgular.

use volt_ast::{
    ClockEdge, DomainKey, DomainValue, Expr, Idx, ItemKind, Name, Port, ResetPolarity, ResetSpec,
    ResetSync,
};
use volt_diagnostics::lstr;
use volt_span::Span;

use super::{ClockSpec, DomainId, DomainInfo, DomainSource, Inferencer};
use crate::resolve::{BuiltinKind, DefId, DefKind};
use crate::ty::Ty;

impl Inferencer<'_> {
    pub(super) fn collect_domain_decls(&mut self) {
        for &item_idx in &self.ast.items {
            let ItemKind::Domain(d) = &self.ast.items_arena[item_idx].kind else {
                continue;
            };
            let Some(&def) = self.res.decl_spans.get(&d.name.span) else {
                continue;
            };
            let mut clock = ClockSpec {
                edge: ClockEdge::Posedge,
            };
            let mut reset = ResetSpec {
                sync: ResetSync::None,
                polarity: ResetPolarity::ActiveHigh,
            };
            let mut trust = None;
            let mut trust_span = None;
            for field in &d.fields {
                match (&field.key, &field.value) {
                    (DomainKey::Clock, DomainValue::ClockEdge(edge)) => clock.edge = *edge,
                    (DomainKey::Reset, DomainValue::Reset(spec)) => reset = *spec,
                    (DomainKey::TrustLevel, DomainValue::Trust(level)) => {
                        trust = Some(*level);
                        trust_span = Some(field.span);
                    }
                    _ => {}
                }
            }
            let id = self.domains.len() as u32;
            self.domains.push(DomainInfo {
                name: d.name.text.clone(),
                clock,
                reset,
                span: d.name.span,
                source: DomainSource::Decl(def),
                trust,
                trust_span,
            });
            self.by_decl.insert(def, id);
        }
    }

    /// Anotasyonsuz clock portuna örtük domain açar (K2).
    pub(super) fn implicit_clock_domain(&mut self, port_def: DefId, name: &str, span: Span) -> u32 {
        if let Some(&id) = self.by_clock_port.get(&port_def) {
            return id;
        }
        let id = self.domains.len() as u32;
        self.domains.push(DomainInfo {
            name: name.to_string(),
            clock: ClockSpec {
                edge: ClockEdge::Posedge,
            },
            reset: ResetSpec {
                sync: ResetSync::None,
                polarity: ResetPolarity::ActiveHigh,
            },
            span,
            source: DomainSource::ClockPort(port_def),
            trust: None,
            trust_span: None,
        });
        self.by_clock_port.insert(port_def, id);
        id
    }

    pub(super) fn domain_name(&self, id: u32) -> &str {
        &self.domains[id as usize].name
    }

    pub(super) fn domain_span(&self, id: u32) -> Span {
        self.domains[id as usize].span
    }

    /// Domain tanım satırına bağlanan ikincil etiket (E3001/E3011/E3012).
    pub(super) fn defined_here(&self, id: u32) -> String {
        lstr!(
            en: "@{} defined here", self.domain_name(id);
            tr: "@{} burada tanımlı", self.domain_name(id)
        )
    }

    pub(super) fn display(&self, d: DomainId) -> String {
        match self.resolve_dom(d) {
            DomainId::Explicit(id) => format!("@{}", self.domain_name(id)),
            DomainId::Timeless => lstr!(en: "clockless (constant)"; tr: "saatsiz (sabit)"),
            DomainId::Unresolved(_) => lstr!(en: "<unresolved>"; tr: "<belirsiz>"),
            DomainId::Error => lstr!(en: "<error>"; tr: "<hata>"),
        }
    }

    pub(super) fn is_clock_def(&self, def: DefId) -> bool {
        self.tyck
            .def_types
            .get(&def)
            .is_some_and(|&t| matches!(self.tyck.types.ty(t), Ty::Clock))
    }

    /// Port clock tipinde mi (modül, extern ve örnekleme hedefi ortak).
    pub(super) fn is_clock_port(&self, port: &Port) -> bool {
        self.decl_def(&port.name)
            .is_some_and(|def| self.is_clock_def(def))
    }

    pub(super) fn decl_def(&self, name: &Name) -> Option<DefId> {
        self.res.decl_spans.get(&name.span).copied()
    }

    pub(super) fn use_def(&self, span: Span) -> Option<DefId> {
        self.res.use_spans.get(&span).copied()
    }

    pub(super) fn def_domain(&mut self, def: DefId) -> DomainId {
        if let Some(&d) = self.signal_domains.get(&def) {
            return d;
        }
        // Sabitler, enum varyantları, generic'ler, döngü değişkenleri:
        // saatten bağımsız.
        DomainId::Timeless
    }

    pub(super) fn builtin_of(&self, callee: Idx<Expr>) -> Option<BuiltinKind> {
        let def = self.res.resolutions.get(&callee)?;
        match self.res.def_kind(*def) {
            DefKind::Builtin(kind) => Some(kind),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use volt_ast::{ClockEdge, ResetPolarity, ResetSync, TrustLevel};

    use super::super::testutil::{inferred, with_inferencer, TWO_DOMAINS};
    use super::super::{DomainId, DomainSource};

    #[test]
    fn domain_declaration_fields_are_collected() {
        let r = inferred(
            "domain D { clock = negedge, reset = async active_low, trust_level = secret }\n\
             module M {\n    in clk : clock @D\n}\n",
        );
        let d = &r.dom.domains[0];
        assert_eq!(d.name, "D");
        assert_eq!(d.clock.edge, ClockEdge::Negedge);
        assert_eq!(d.reset.sync, ResetSync::Async);
        assert_eq!(d.reset.polarity, ResetPolarity::ActiveLow);
        assert_eq!(d.trust, Some(TrustLevel::Secret));
        assert!(d.trust_span.is_some());
    }

    #[test]
    fn unannotated_clock_port_opens_one_implicit_domain() {
        let r = inferred("module M {\n    in clk : clock\n    in a : u8\n    in b : u8\n}\n");
        assert_eq!(r.dom.domains.len(), 1);
        let d = &r.dom.domains[0];
        assert_eq!(d.name, "clk");
        assert!(matches!(d.source, DomainSource::ClockPort(_)));
        assert_eq!(d.trust, None);
        assert_eq!(r.domain_of("a"), r.domain_of("b"));
    }

    #[test]
    fn display_names_every_kind_of_domain() {
        with_inferencer(TWO_DOMAINS, |inf| {
            assert_eq!(inf.display(DomainId::Explicit(1)), "@Slow");
            assert!(inf.defined_here(0).contains("@Fast"));
            let timeless = inf.display(DomainId::Timeless);
            let error = inf.display(DomainId::Error);
            let var = inf.fresh_var();
            let unresolved = inf.display(var);
            assert!(!timeless.starts_with('@') && !timeless.is_empty());
            assert_ne!(timeless, error);
            assert_ne!(error, unresolved);
        });
    }

    #[test]
    fn display_follows_variable_bindings() {
        with_inferencer(TWO_DOMAINS, |inf| {
            let v = inf.fresh_var();
            inf.bind_if_var(v, DomainId::Explicit(0));
            assert_eq!(inf.display(v), "@Fast");
        });
    }
}
