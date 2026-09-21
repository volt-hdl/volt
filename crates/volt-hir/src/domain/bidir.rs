//! ADR-0051 — çift yönlü (inout/opendrain) port okumasının domain kuralı.

use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::{DomainId, Inferencer};
use crate::resolve::DefId;

impl Inferencer<'_> {
    /// ADR-0051 — çift yönlü port okuması. Hattın öbür ucu modülün saat
    /// alanı dışındaki bir aygıttır: `sync()` kaynağı değilse ve kontrat
    /// içinde değilse W3007 (uyarı; okuma modülün alanında sayılmaya
    /// devam eder ki E3012 kaskadı oluşmasın). `sync()` içinde kaynak
    /// alan `Timeless` döner: hedefle aynı alan sayılmaz, W3002 çıkmaz.
    pub(super) fn bidir_read(&mut self, def: DefId, span: Span) -> DomainId {
        if self.in_sync_source {
            return DomainId::Timeless;
        }
        let dom = self.def_domain(def);
        if self.in_contract {
            return dom;
        }
        let name = self.res.defs[def.0 as usize].name.clone();
        let decl_span = self.res.defs[def.0 as usize].span;
        self.diagnostics.push(
            Diagnostic::warning(
                ErrorCode::W3007,
                lstr!(en: "external bidirectional signal '{name}' read without synchronization";
                      tr: "harici çift yönlü sinyal '{name}' senkronizasyonsuz okunuyor"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "the other end of this line is a device outside the clock domain";
                          tr: "bu hattın öbür ucu saat alanı dışındaki bir aygıt"),
                ),
                lstr!(en: "synchronize it first: wire {name}_s : bool; {name}_s = sync({name}.read(), <clock>) and read {name}_s";
                      tr: "önce senkronize edin: wire {name}_s : bool; {name}_s = sync({name}.read(), <saat>) ve {name}_s okuyun"),
            )
            .with_secondary(
                decl_span,
                lstr!(en: "bidirectional port declared here"; tr: "çift yönlü port burada bildirildi"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "an 'in' port is trusted to be in the module's domain (K2); a bidirectional pad is by definition driven by another device with its own timing, so a direct sample can go metastable (ADR-0051)";
                      tr: "'in' portunun modülün alanında olduğuna güvenilir (K2); çift yönlü pad tanımı gereği kendi zamanlamasıyla başka bir aygıtça sürülür, doğrudan örnekleme yarı kararlı kalabilir (ADR-0051)"),
            ),
        );
        dom
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::inferred;

    fn pin_module(body: &str) -> String {
        format!(
            "module M {{\n    in clk : clock\n    inout pin : bool\n    out q : bool\n{body}\n}}\n"
        )
    }

    #[test]
    fn direct_read_of_a_bidirectional_port_warns_w3007() {
        let r = inferred(&pin_module("    q = pin.read()"));
        assert_eq!(r.codes(), ["W3007"]);
        // Okuma modülün alanında sayılmaya devam eder (kaskad yok).
        assert_eq!(r.domain_name_of("q"), Some("clk"));
    }

    #[test]
    fn read_as_sync_source_is_the_legitimate_bridge() {
        let r = inferred(&pin_module("    q = sync(pin.read(), clk)"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }
}
