//! K10 (ADR-0039) — bundle portunun alanları tek saat alanında olmalı.

use volt_ast::{ModuleDecl, Port};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::{DomainId, Inferencer};

impl Inferencer<'_> {
    /// K10 (ADR-0039) — bir bundle portunun düzleştirilmiş alanları
    /// farklı saat alanlarına düştüyse E3013. Beş parça: kod, konum
    /// (bundle portu), açıklama, öneri, ADR referansı (not).
    pub(super) fn check_bundle_domains(&mut self, m: &ModuleDecl) {
        let mut groups: Vec<(String, Vec<(&Port, DomainId)>)> = Vec::new();
        for p in &m.ports {
            let Some(origin) = &p.bundle else { continue };
            let Some(def) = self.decl_def(&p.name) else {
                continue;
            };
            let Some(&dom) = self.signal_domains.get(&def) else {
                continue;
            };
            if matches!(dom, DomainId::Error) {
                continue;
            }
            match groups.iter_mut().find(|(k, _)| *k == origin.port.text) {
                Some((_, v)) => v.push((p, dom)),
                None => groups.push((origin.port.text.clone(), vec![(p, dom)])),
            }
        }
        for (port_name, fields) in groups {
            let Some(&(first, first_dom)) = fields.first() else {
                continue;
            };
            let Some(&(other, other_dom)) = fields.iter().find(|(_, d)| *d != first_dom) else {
                continue;
            };
            self.err_bundle_split(&port_name, (first, first_dom), (other, other_dom));
        }
    }

    /// E3013 — `first` ve `other` aynı bundle portunun farklı alanlara
    /// düşen iki alanı.
    fn err_bundle_split(
        &mut self,
        port_name: &str,
        (first, first_dom): (&Port, DomainId),
        (other, other_dom): (&Port, DomainId),
    ) {
        let (origin, first_o, other_o) = (
            first.bundle.as_ref().expect("bundle kaynağı"),
            first.bundle.as_ref().expect("bundle kaynağı"),
            other.bundle.as_ref().expect("bundle kaynağı"),
        );
        let (d1, d2) = (self.display(first_dom), self.display(other_dom));
        let diag = Diagnostic::error(
            ErrorCode::E3013,
            lstr!(en: "bundle port '{port_name}' spans two clock domains: '{}' is {d1} but '{}' is {d2}",
                      first_o.path, other_o.path;
                  tr: "'{port_name}' bundle portu iki saat alanına yayılıyor: '{}' {d1} ama '{}' {d2}",
                      first_o.path, other_o.path),
            LabeledSpan::primary(
                origin.port.span,
                lstr!(en: "fields of this bundle disagree on their domain";
                      tr: "bu bundle'ın alanları alanlarında uyuşmuyor"),
            ),
            lstr!(en: "annotate the whole port with one domain ({port_name} : {} {d1}) or split the interface into two bundles",
                      origin.bundle;
                  tr: "portun tamamını tek alanla anotasyonlayın ({port_name} : {} {d1}) ya da arayüzü iki bundle'a ayırın",
                      origin.bundle),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "a bundle is one interface: all of its fields cross the module boundary together (ADR-0039)";
                  tr: "bundle tek bir arayüzdür: bütün alanları modül sınırını birlikte geçer (ADR-0039)"),
        );
        self.diagnostics.push(diag);
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};

    #[test]
    fn bundle_split_across_two_domains_is_e3013() {
        let r = inferred(&format!(
            "struct port Bus {{\n    out data : u8 @Fast\n    in ready : bool @Slow\n}}\n{}",
            two_clock("    out bus : Bus")
        ));
        assert_eq!(r.count("E3013"), 1, "{:?}", r.codes());
        let d = r
            .dom
            .diagnostics
            .iter()
            .find(|d| d.code.as_str() == "E3013")
            .unwrap();
        assert!(d.message.contains("'bus'"), "{}", d.message);
        assert!(d.message.contains("@Fast") && d.message.contains("@Slow"));
    }

    #[test]
    fn bundle_in_one_domain_is_clean() {
        let r = inferred(&format!(
            "struct port Bus {{\n    out data : u8 @Fast\n    in ready : bool @Fast\n}}\n{}",
            two_clock("    out bus : Bus")
        ));
        assert_eq!(r.count("E3013"), 0, "{:?}", r.codes());
    }
}
