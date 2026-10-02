//! Kenarsız alanda register (E3016, ADR-0098 eki).
//!
//! `domain D { clock = none }` saat kenarı olmayan bir alan bildirir
//! (grammar-full.ebnf §3 ClockEdge). `on` bloğu değerini saat kenarında
//! örnekleyen register tanımlar; böyle bir alanda örneklenecek kenar
//! yoktur. sv-emit kenarsız alanı da `posedge` yazıyordu: alanın var
//! olmadığını söylediği bir kenarla zamanlanan sessiz bir flop.

use volt_ast::ClockEdge;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::{DomainId, Inferencer};

impl Inferencer<'_> {
    /// `on` bloğunun saati kenarsız bir alandaysa E3016. `clock`: bloğun
    /// tetikleyici saatinin konumu.
    pub(super) fn check_edgeless_on(&mut self, dom: DomainId, clock: Span) {
        let DomainId::Explicit(id) = self.resolve_dom(dom) else {
            return;
        };
        if !matches!(self.domains[id as usize].clock.edge, ClockEdge::None) {
            return;
        }
        let name = self.domain_name(id).to_string();
        let diag = Diagnostic::error(
            ErrorCode::E3016,
            lstr!(
                en: "register in clock domain '{name}', which has no clock edge";
                tr: "saat kenarı olmayan '{name}' alanında register"
            ),
            LabeledSpan::primary(
                clock,
                lstr!(
                    en: "this clock belongs to @{name}, declared with 'clock = none'";
                    tr: "bu saat 'clock = none' ile bildirilen @{name} alanına ait"
                ),
            ),
            lstr!(
                en: "give the domain a clock edge: domain {name} {{ clock = posedge }}; \
                     if the signal is combinational, write it with '=' in a 'comb' block \
                     or at module level instead of in an 'on' block";
                tr: "alana bir saat kenarı verin: domain {name} {{ clock = posedge }}; \
                     sinyal kombinasyonelse 'on' bloğu yerine bir 'comb' bloğunda ya da \
                     modül düzeyinde '=' ile yazın"
            ),
        )
        .with_secondary(self.domain_span(id), self.defined_here(id))
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "an 'on' block samples its values at a clock edge; 'clock = none' declares \
                     a domain without one, so there is no edge to sample on";
                tr: "'on' bloğu değerlerini bir saat kenarında örnekler; 'clock = none' kenarı \
                     olmayan bir alan bildirir, örneklenecek bir kenar yoktur"
            ),
        );
        self.diagnostics.push(diag);
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::inferred;

    const ASYNC: &str = "domain Async { clock = none }\n";

    #[test]
    fn on_block_in_an_edgeless_domain_is_e3016() {
        let r = inferred(&format!(
            "{ASYNC}module M {{\n    in clk : clock @Async\n    in d : u8\n    out q : u8\n    \
             reg r : u8 = 0\n    on clk {{\n        r <= d\n    }}\n    q = r\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 1, "{:?}", r.codes());
        let d = r
            .dom
            .diagnostics
            .iter()
            .find(|d| d.code.as_str() == "E3016")
            .unwrap();
        // Birincil etiket `on clk`'teki saatte, ikincil alan tanımında.
        assert_eq!(d.spans.len(), 2, "{:?}", d.spans);
        assert!(d.spans[1].label.contains("@Async"), "{}", d.spans[1].label);
        let help = d.help.as_deref().unwrap_or("");
        assert!(help.contains("clock = posedge"), "{help}");
        assert!(help.contains("comb"), "{help}");
    }

    #[test]
    fn each_on_block_of_an_edgeless_clock_is_reported() {
        let r = inferred(&format!(
            "{ASYNC}module M {{\n    in clk : clock @Async\n    reg a : u8 = 0\n    reg b : u8 = 0\n    \
             on clk {{\n        a <= 1\n    }}\n    on clk {{\n        b <= 2\n    }}\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 2, "{:?}", r.codes());
    }

    #[test]
    fn edged_domains_and_clockless_signals_are_not_e3016() {
        // posedge/negedge alanları ve kenarsız alandaki kombinasyonel
        // sinyaller geçerlidir.
        let r = inferred(&format!(
            "{ASYNC}domain Neg {{ clock = negedge }}\nmodule M {{\n    in clk : clock @Neg\n    \
             in a : u8 @Async\n    in b : u8 @Async\n    out y : u8 @Async\n    out q : u8 @Neg\n    \
             reg r : u8 @Neg = 0\n    on clk {{\n        r <= r + 1\n    }}\n    y = a + b\n    q = r\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 0, "{:?}", r.codes());
    }
}
