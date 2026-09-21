//! Extern modül bildirimi (ADR-0047): gövdesiz sınırın domain sözleşmesi.

use std::collections::HashSet;

use volt_ast::{ExternDecl, Name, Port};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::Inferencer;
use crate::resolve::{DefId, DefKind};

impl Inferencer<'_> {
    /// Extern sınırının domain sözleşmesi: sembolik `@Ad` en az bir clock
    /// portunda taşınmalı (E3002), çok saatli extern'de saat dışı her
    /// port anotasyonlu olmalı (E3010). Tek saatli extern K2 gibi
    /// davranır, saatsiz extern Timeless'tır — ikisi de anotasyonsuz.
    pub(super) fn check_extern_decl(&mut self, x: &ExternDecl) {
        let clocks: Vec<&Port> = x.ports.iter().filter(|p| self.is_clock_port(p)).collect();
        let anchored: HashSet<DefId> = clocks
            .iter()
            .filter_map(|c| c.domain.as_ref())
            .filter_map(|a| self.use_def(a.span))
            .collect();

        self.clock_candidates.clear();
        for c in &clocks {
            let shown = match &c.domain {
                Some(a) => format!("@{}", a.text),
                None => format!("@{}", c.name.text),
            };
            self.clock_candidates.push((c.name.span, shown));
        }

        let mut reported: HashSet<DefId> = HashSet::new();
        for p in &x.ports {
            let Some(def) = self.decl_def(&p.name) else {
                continue;
            };
            if self.is_clock_def(def) {
                continue;
            }
            match &p.domain {
                Some(ann) => self.check_extern_annotation(x, ann, &anchored, &mut reported),
                None if clocks.len() > 1 => self.err_ambiguous(&p.name, true),
                None => {}
            }
        }
    }

    /// Saat dışı extern portunun `@Ad` anotasyonu: sembolik alan bir
    /// clock portunda taşınmalı (alan başına bir kez raporlanır).
    fn check_extern_annotation(
        &mut self,
        x: &ExternDecl,
        ann: &Name,
        anchored: &HashSet<DefId>,
        reported: &mut HashSet<DefId>,
    ) {
        let Some(dom_def) = self.use_def(ann.span) else {
            return;
        };
        match self.res.def_kind(dom_def) {
            DefKind::DomainParam if !anchored.contains(&dom_def) && reported.insert(dom_def) => {
                self.err_symbolic_without_clock(&x.name, ann);
            }
            // `@port` clock tipinde değil — modülle aynı E3002.
            DefKind::Port { .. } if !self.is_clock_def(dom_def) => {
                self.err_annotation_not_clock(ann);
            }
            _ => {}
        }
    }

    /// E3002 (extern biçimi) — sembolik alanı taşıyan clock portu yok:
    /// örneklemede hiç bağlanamaz, dolayısıyla hiç denetlenemez.
    fn err_symbolic_without_clock(&mut self, module: &Name, ann: &Name) {
        let lower = ann.text.to_lowercase();
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E3002,
                lstr!(en: "symbolic domain '@{}' has no clock port in extern module '{}'",
                          ann.text, module.text;
                      tr: "'@{}' sembolik saat alanının '{}' extern modülünde clock portu yok",
                          ann.text, module.text),
                LabeledSpan::primary(
                    ann.span,
                    lstr!(en: "no clock port carries this domain";
                          tr: "bu alanı taşıyan clock portu yok"),
                ),
                lstr!(en: "add a clock port for it: in {lower}_clk : clock @{}", ann.text;
                      tr: "onun için bir clock portu ekleyin: in {lower}_clk : clock @{}", ann.text),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "a symbolic domain is bound to a real clock domain through a clock \
                           connection at instantiation; without a clock port it can never be \
                           bound (ADR-0047)";
                      tr: "sembolik alan örneklemede saat bağlantısıyla gerçek alana bağlanır; \
                           clock portu yoksa hiç bağlanamaz (ADR-0047)"),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::inferred;

    #[test]
    fn symbolic_domain_without_a_clock_port_is_reported_once() {
        let r = inferred(
            "extern module X {\n    in clk : clock @Src\n    in a : u8 @Src\n    \
             out q : u8 @Dst\n    out v : bool @Dst\n}\n",
        );
        assert_eq!(r.codes(), ["E3002"]);
        assert!(r.dom.diagnostics[0].message.contains("@Dst"));
    }

    #[test]
    fn multi_clock_extern_requires_annotations() {
        let r = inferred(
            "extern module X {\n    in c1 : clock @A\n    in c2 : clock @B\n    in a : u8\n    out q : u8 @B\n}\n",
        );
        assert_eq!(r.codes(), ["E3010"]);
        assert!(r.dom.diagnostics[0]
            .help
            .as_deref()
            .is_some_and(|h| h.contains("@A")));
    }

    #[test]
    fn single_clock_and_clockless_externs_need_no_annotation() {
        let r = inferred(
            "extern module X {\n    in clk : clock\n    in a : u8\n    out q : u8\n}\n\
             extern module Y {\n    in a : u8\n    out q : u8\n}\n",
        );
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn port_annotation_must_name_a_clock_port() {
        let r = inferred(
            "extern module X {\n    in clk : clock\n    in a : u8\n    out q : u8 @a\n}\n",
        );
        assert_eq!(r.codes(), ["E3002"]);
    }
}
