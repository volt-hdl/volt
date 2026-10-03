//! Modül çıkarımı (§3 akışı): saat taraması (K2), port ataması
//! (K1, K2, K3), ardından bundle, register, gövde ve kontrat adımları.

use volt_ast::{ModuleDecl, Name, Port};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::edgeless::EdgeUse;
use super::{DomainId, Inferencer, ModuleClock};
use crate::resolve::DefId;

impl Inferencer<'_> {
    pub(super) fn infer_module(&mut self, m: &ModuleDecl) {
        self.module_name = m.name.text.clone();
        // 1. MODÜL TARAMASI — clock portlarını topla (K2).
        self.scan_clock_ports(m);

        // 2. PORT ATAMASI (K1, K2, K3).
        self.assign_port_domains(m);

        // 2b. BUNDLE TUTARLILIĞI (ADR-0039, E3013).
        self.check_bundle_domains(m);

        // 3. REGISTER ATAMASI (K4) — önce açık `reg(clk)`, sonra
        //    'on' bloğu yazıcılarından çıkarım.
        self.assign_reg_domains(m);

        // 4-6. YAYILIM + ATAMA + ÖRNEK KONTROLÜ (K5-K9).
        for &stmt in &m.body {
            self.walk_stmt(stmt);
        }

        // 7. KONTRATLAR (F4a) — kontrat ifadesindeki sinyaller aynı
        //    alanda olmalı; karışım join üzerinden E3001 üretir.
        //    Kontrat gözlemi metastabilite taşımaz: W3007 yok (ADR-0051).
        self.in_contract = true;
        for c in &m.contracts {
            self.expr_domain(c.expr);
        }
        self.in_contract = false;

        // 8. KONTRAT SAATİ (ADR-0098 eki 2): kontratlar modülün ilk saat
        //    portunun kenarında örneklenir; kenarsız alanda E3016.
        if let (Some(first), Some(clock)) = (m.contracts.first(), self.module_clocks.first()) {
            let (dom, clock) = (clock.dom, clock.name.clone());
            let use_ = EdgeUse::Contracts {
                module: &m.name.text,
                clock: &clock,
            };
            self.check_edgeless_use(dom, first.span, use_);
        }
    }

    /// Adım 1 — clock portları alanlarını alır (K1 anotasyon ya da K2
    /// örtük alan); modülün varsayılan alanı ve çoklu saat durumu kurulur.
    fn scan_clock_ports(&mut self, m: &ModuleDecl) {
        let mut clocks: Vec<(DefId, &Port)> = Vec::new();
        for p in &m.ports {
            if let Some(def) = self.decl_def(&p.name) {
                if self.is_clock_def(def) {
                    clocks.push((def, p));
                }
            }
        }

        self.clock_candidates.clear();
        self.anchored.clear();
        self.module_clocks.clear();
        self.annotated_ports.clear();
        for p in &m.ports {
            if let (Some(def), Some(ann)) = (self.decl_def(&p.name), &p.domain) {
                self.annotated_ports.insert(def, ann.text.clone());
            }
        }
        // Saat portu başka bir saat portuyla açıklanabilir (`in c1 : clock
        // @c0`): alanı zincirin sonundaki alandır — sırası önemli değil.
        // Döngü (`@c2` / `@c1`) hiçbir alana varmaz: E3002, üreticinin
        // flop denetimiyle aynı kural (ADR-0099).
        let target = |s: &Self, p: &Port| {
            p.domain
                .as_ref()
                .and_then(|ann| s.use_def(ann.span))
                .and_then(|t| clocks.iter().position(|(d, _)| *d == t))
        };
        let mut doms: Vec<Option<DomainId>> = vec![None; clocks.len()];
        for (i, &(def, p)) in clocks.iter().enumerate() {
            if let Some(ann) = &p.domain {
                if let Some(d) = self.use_def(ann.span) {
                    self.anchored.insert(d);
                }
                if target(self, p).is_none() {
                    doms[i] = Some(self.annotation_domain(&ann.clone()));
                }
            } else {
                let id = self.implicit_clock_domain(def, &p.name.text, p.name.span);
                doms[i] = Some(DomainId::Explicit(id));
            }
        }
        let mut cycle_reported = false;
        for i in 0..clocks.len() {
            let mut cur = i;
            let mut seen = Vec::new();
            let dom = loop {
                if let Some(d) = doms[cur] {
                    break d;
                }
                if seen.contains(&cur) {
                    if !cycle_reported {
                        cycle_reported = true;
                        self.err_clock_annotation_cycle(clocks[i].1);
                    }
                    break DomainId::Error;
                }
                seen.push(cur);
                match target(self, clocks[cur].1) {
                    Some(next) => cur = next,
                    None => break DomainId::Error,
                }
            };
            doms[i] = Some(dom);
        }
        for (i, &(def, p)) in clocks.iter().enumerate() {
            let dom = doms[i].unwrap_or(DomainId::Error);
            self.signal_domains.insert(def, dom);
            self.clock_candidates.push((p.name.span, self.display(dom)));
            self.module_clocks.push(ModuleClock {
                name: p.name.text.clone(),
                annotation: p.domain.as_ref().map(|a| a.text.clone()),
                dom,
            });
        }

        self.multi_clock = clocks.len() > 1;
        self.default_domain = match clocks.as_slice() {
            [] => DomainId::Timeless,
            [(def, _)] => self.signal_domains[def],
            _ => DomainId::Error, // çoklu saat: varsayılan yok (K3)
        };
    }

    /// E3002 — saat portu açıklamaları döngü kuruyor (`@c2` / `@c1`):
    /// hiçbir `domain` bildirimine ya da açıklamasız saate varılmaz.
    fn err_clock_annotation_cycle(&mut self, p: &Port) {
        let ann = p.domain.as_ref().map_or("", |a| a.text.as_str());
        let name = &p.name.text;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E3002,
                lstr!(
                    en: "cannot derive the clock domain of '{name}' from '@{ann}'";
                    tr: "'{name}' saatinin alanı '@{ann}' açıklamasından türetilemiyor"
                ),
                LabeledSpan::primary(
                    p.name.span,
                    lstr!(en: "the clock port annotations form a cycle"; tr: "saat portu açıklamaları döngü kuruyor"),
                ),
                lstr!(
                    en: "annotate the clock port with a 'domain' declaration, or remove the \
                         annotation (the port then has its own implicit domain)";
                    tr: "saat portunu bir 'domain' bildirimiyle açıklayın ya da açıklamayı \
                         kaldırın (port o zaman kendi örtük alanındadır)"
                ),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(
                    en: "a clock port annotated with another clock port shares its domain; \
                         following '@{ann}' leads back to '{name}', so no domain declaration \
                         and no clock edge is ever reached";
                    tr: "başka bir saat portuyla açıklanan saat portu onun alanını paylaşır; \
                         '@{ann}' izlenince '{name}' portuna geri dönülür, hiçbir domain \
                         bildirimine ve saat kenarına varılmaz"
                ),
            ),
        );
    }

    /// Adım 2 — saat dışı portlar: anotasyon (K1), tek saat (K2) ya da
    /// çoklu saatte belirsizlik (K3, E3010).
    fn assign_port_domains(&mut self, m: &ModuleDecl) {
        self.bidir_ports.clear();
        for p in &m.ports {
            let Some(def) = self.decl_def(&p.name) else {
                continue;
            };
            if self.is_clock_def(def) {
                continue;
            }
            if self.is_raw_reset_port(p, def) {
                self.assign_raw_reset_domain(p, def);
                continue;
            }
            if p.direction.is_bidirectional() {
                self.bidir_ports.insert(def);
            }
            let dom = match &p.domain {
                Some(ann) => self.signal_annotation_domain(&ann.clone()),
                None if self.multi_clock => {
                    self.err_ambiguous(&p.name.clone(), false);
                    DomainId::Error
                }
                None => self.default_domain,
            };
            self.signal_domains.insert(def, dom);
        }
    }

    /// K3 — çoklu saatte anotasyonsuz sinyal (E3010, 5 parça).
    pub(super) fn err_ambiguous(&mut self, name: &Name, in_extern: bool) {
        let candidate = self.clock_candidates.first().map(|(_, d)| d.clone());
        let reason = if in_extern {
            lstr!(en: "the extern module has more than one clock port, so it cannot be \
                       inferred which one the port belongs to";
                  tr: "extern modülde birden fazla clock portu var, portun hangisine \
                       ait olduğu çıkarılamıyor")
        } else {
            lstr!(en: "the module has more than one clock, so it cannot be inferred \
                       which one the signal belongs to";
                  tr: "modülde birden fazla saat var, sinyalin hangisine \
                       ait olduğu çıkarılamıyor")
        };
        let mut diag = Diagnostic::error(
            ErrorCode::E3010,
            lstr!(
                en: "cannot determine the signal's clock domain";
                tr: "sinyalin saat alanı belirlenemiyor"
            ),
            LabeledSpan::primary(
                name.span,
                lstr!(
                    en: "ambiguous which domain this belongs to";
                    tr: "hangi alana ait olduğu belirsiz"
                ),
            ),
            lstr!(
                en: "add an explicit annotation: {} : <type> {}",
                    name.text,
                    candidate.clone().unwrap_or_else(|| "@Domain".to_string());
                tr: "açık anotasyon ekleyin: {} : <tip> {}",
                    name.text,
                    candidate.clone().unwrap_or_else(|| "@Alan".to_string())
            ),
        )
        .with_note(NoteKind::Reason, reason);
        for (span, dom) in self.clock_candidates.clone() {
            diag = diag.with_secondary(span, lstr!(en: "candidate: {dom}"; tr: "aday: {dom}"));
        }
        self.diagnostics.push(diag);
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};
    use super::super::DomainId;

    #[test]
    fn clockless_module_is_timeless() {
        let r = inferred("module M {\n    in a : u8\n    out q : u8\n    q = a\n}\n");
        assert_eq!(r.domain_of("a"), DomainId::Timeless);
        assert!(r.dom.domains.is_empty());
    }

    #[test]
    fn single_clock_rule_assigns_every_port_silently() {
        let r = inferred(
            "module M {\n    in clk : clock\n    in a : u8\n    out q : u8\n    q = a\n}\n",
        );
        assert_eq!(r.domain_name_of("a"), Some("clk"));
        assert_eq!(r.domain_name_of("q"), Some("clk"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn unannotated_port_is_ambiguous_with_two_clocks() {
        let r = inferred(&two_clock("    in a : u8\n    in b : u8 @Fast"));
        assert_eq!(r.codes(), ["E3010"]);
        assert_eq!(r.domain_of("a"), DomainId::Error);
        // Birincil etiket + saat başına bir aday.
        assert_eq!(r.dom.diagnostics[0].spans.len(), 3);
        assert!(r.dom.diagnostics[0]
            .help
            .as_deref()
            .is_some_and(|h| h.contains("@Fast")));
    }

    #[test]
    fn module_state_does_not_leak_into_the_next_module() {
        let r = inferred(&format!(
            "{}module N {{\n    in clk : clock\n    in x : u8\n}}\n",
            two_clock("    in a : u8 @Fast")
        ));
        assert_eq!(r.domain_name_of("x"), Some("clk"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn contracts_mixing_domains_are_reported() {
        let r = inferred(&two_clock(
            "    in a : bool @Fast\n    in b : bool @Slow\n    invariant: a == b",
        ));
        assert_eq!(r.count("E3001"), 1, "{:?}", r.codes());
    }
}
