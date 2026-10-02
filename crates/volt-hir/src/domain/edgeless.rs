//! Kenarsız alanda flop (E3016, ADR-0098 eki ve eki 2).
//!
//! `domain D { clock = none }` saat kenarı olmayan bir alan bildirir
//! (grammar-full.ebnf §3 ClockEdge). Değerini saat kenarında örnekleyen
//! her yapı (flop) böyle bir alanın saatiyle zamanlanamaz: örneklenecek
//! kenar yoktur. sv-emit kenarsız alanı `posedge` yazıyordu: alanın var
//! olmadığını söylediği bir kenarla zamanlanan sessiz bir flop.
//!
//! Saatin kenar gerektiren kullanımları (sv-emit'in flop ürettiği her
//! yer, [`EdgeUse`]): `on` bloğu; `sync()` hedefi ve kaynağın yakalama
//! flop'u; yerleşik primitifin saat portu; alt modülün kenarlı alandaki
//! saat portu; kontratların örneklendiği saat (modülün ilk saat portu).
//! sv-emit aynı listeyi üretimden önce yeniden denetler (`flop_audit`).

use volt_ast::ClockEdge;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::{DomainId, Inferencer};

/// Saatin kenar gerektiren kullanımı: E3016 iletisi kullanım yerine göre
/// uyarlanır.
pub(super) enum EdgeUse<'u> {
    /// `on clk { ... }` register'ları.
    On,
    /// `sync(d, clk)`: senkronizör aşamaları hedef saatte.
    SyncDestination,
    /// `sync(d, ...)`: kaynak port kendi alanının saat portuyla
    /// (`clock`) önce yakalanır.
    SyncSource { source: &'u str, clock: &'u str },
    /// Yerleşik primitifin saat portu (`SyncFifo { clk: .. }`).
    Builtin(&'static str),
    /// Alt modülün kenarlı alandaki saat portu.
    ChildClock { module: &'u str, port: &'u str },
    /// Kontratlar modülün ilk saat portunda örneklenir.
    Contracts { module: &'u str, clock: &'u str },
}

/// E3016 metinleri: ileti, birincil etiket, öneri, gerekçe.
struct Texts {
    message: String,
    label: String,
    help: String,
    reason: String,
}

impl Inferencer<'_> {
    /// `dom` kenarsız bir alansa `use_` için E3016 verir; verildiyse true.
    /// `span`: kullanım yeri (saat argümanı, bağlama, kontrat).
    pub(super) fn check_edgeless_use(
        &mut self,
        dom: DomainId,
        span: Span,
        use_: EdgeUse<'_>,
    ) -> bool {
        let Some(id) = self.edgeless_domain(dom) else {
            return false;
        };
        let name = self.domain_name(id).to_string();
        let t = texts(&name, &use_);
        let diag = Diagnostic::error(
            ErrorCode::E3016,
            t.message,
            LabeledSpan::primary(span, t.label),
            t.help,
        )
        .with_secondary(self.domain_span(id), self.defined_here(id))
        .with_note(NoteKind::Reason, t.reason);
        self.diagnostics.push(diag);
        true
    }

    /// `dom` kenarsız (`clock = none`) bir alana çözülüyorsa indeksi.
    pub(super) fn edgeless_domain(&self, dom: DomainId) -> Option<u32> {
        let DomainId::Explicit(id) = self.resolve_dom(dom) else {
            return None;
        };
        matches!(self.domains[id as usize].clock.edge, ClockEdge::None).then_some(id)
    }
}

/// Kullanım yerine göre E3016 metinleri (5 parça: kod, konum, açıklama,
/// öneri, gerekçe notu; explain E3016 ADR-0098'e işaret eder).
fn texts(name: &str, use_: &EdgeUse<'_>) -> Texts {
    let give_edge = lstr!(
        en: "give the domain a clock edge: domain {name} {{ clock = posedge }}";
        tr: "alana bir saat kenarı verin: domain {name} {{ clock = posedge }}"
    );
    let owner = lstr!(
        en: "this clock belongs to @{name}, declared with 'clock = none'";
        tr: "bu saat 'clock = none' ile bildirilen @{name} alanına ait"
    );
    let no_edge = lstr!(
        en: "'clock = none' declares a domain without a clock edge, so there is no edge to sample on";
        tr: "'clock = none' saat kenarı olmayan bir alan bildirir, örneklenecek bir kenar yoktur"
    );
    match use_ {
        EdgeUse::On => Texts {
            message: lstr!(
                en: "register in clock domain '{name}', which has no clock edge";
                tr: "saat kenarı olmayan '{name}' alanında register"
            ),
            label: owner,
            help: lstr!(
                en: "{give_edge}; if the signal is combinational, write it with '=' in a 'comb' \
                     block or at module level instead of in an 'on' block";
                tr: "{give_edge}; sinyal kombinasyonelse 'on' bloğu yerine bir 'comb' bloğunda \
                     ya da modül düzeyinde '=' ile yazın"
            ),
            reason: lstr!(
                en: "an 'on' block samples its values at a clock edge; 'clock = none' declares \
                     a domain without one, so there is no edge to sample on";
                tr: "'on' bloğu değerlerini bir saat kenarında örnekler; 'clock = none' kenarı \
                     olmayan bir alan bildirir, örneklenecek bir kenar yoktur"
            ),
        },
        EdgeUse::SyncDestination => Texts {
            message: lstr!(
                en: "sync() into clock domain '{name}', which has no clock edge";
                tr: "saat kenarı olmayan '{name}' alanına sync()"
            ),
            label: owner,
            help: lstr!(
                en: "{give_edge}; sync() crosses into a domain whose clock has an edge";
                tr: "{give_edge}; sync() saati kenarlı bir alana geçirir"
            ),
            reason: lstr!(
                en: "the synchronizer stages are flip-flops on the destination clock; {no_edge}";
                tr: "senkronizör aşamaları hedef saatteki flop'lardır; {no_edge}"
            ),
        },
        EdgeUse::SyncSource { source, clock } => Texts {
            message: lstr!(
                en: "sync() source '{source}' is captured on clock '{clock}' of domain '{name}', \
                     which has no clock edge";
                tr: "sync() kaynağı '{source}', saat kenarı olmayan '{name}' alanının '{clock}' \
                     saatinde yakalanıyor"
            ),
            label: lstr!(
                en: "'{source}' is in @{name}, and '{clock}' is a clock port of @{name}";
                tr: "'{source}' @{name} alanında, '{clock}' @{name} alanının saat portu"
            ),
            help: lstr!(
                en: "{give_edge}; or, if '{source}' is asynchronous, do not declare '{clock}' in \
                     @{name}: the synchronizer then samples '{source}' directly on the \
                     destination clock";
                tr: "{give_edge}; ya da '{source}' asenkronsa '{clock}' saatini @{name} alanında \
                     bildirmeyin: senkronizör '{source}' sinyalini doğrudan hedef saatte örnekler"
            ),
            reason: lstr!(
                en: "when the source's domain has a clock port, sync() first captures the source \
                     in a flip-flop on that clock; {no_edge}";
                tr: "kaynağın alanının bir saat portu varsa sync() kaynağı önce o saatteki bir \
                     flop'ta yakalar; {no_edge}"
            ),
        },
        EdgeUse::Builtin(prim) => Texts {
            message: lstr!(
                en: "{prim} clocked by clock domain '{name}', which has no clock edge";
                tr: "{prim}, saat kenarı olmayan '{name}' alanının saatiyle zamanlanıyor"
            ),
            label: owner,
            help: lstr!(
                en: "{give_edge}; or connect a clock of a domain with an edge";
                tr: "{give_edge}; ya da kenarlı bir alanın saatini bağlayın"
            ),
            reason: lstr!(
                en: "{prim} is built from flip-flops that sample at a clock edge; {no_edge}";
                tr: "{prim} bir saat kenarında örnekleyen flop'lardan oluşur; {no_edge}"
            ),
        },
        EdgeUse::ChildClock { module, port } => Texts {
            message: lstr!(
                en: "clock of domain '{name}', which has no clock edge, drives clock port \
                     '{port}' of '{module}'";
                tr: "saat kenarı olmayan '{name}' alanının saati '{module}' modülünün '{port}' \
                     saat portunu sürüyor"
            ),
            label: owner,
            help: lstr!(
                en: "{give_edge}; or, if '{module}' uses '{port}' only combinationally, declare \
                     the port in a domain without an edge: in {port} : clock @{name}";
                tr: "{give_edge}; ya da '{module}' '{port}' portunu yalnız kombinasyonel \
                     kullanıyorsa portu kenarsız bir alanda bildirin: in {port} : clock @{name}"
            ),
            reason: lstr!(
                en: "'{module}' declares '{port}' in a domain with a clock edge and clocks its \
                     flip-flops on that edge; {no_edge}";
                tr: "'{module}' '{port}' portunu kenarlı bir alanda bildirir ve flop'larını o \
                     kenarda zamanlar; {no_edge}"
            ),
        },
        EdgeUse::Contracts { module, clock } => Texts {
            message: lstr!(
                en: "contracts of '{module}' are checked on clock '{clock}' of domain '{name}', \
                     which has no clock edge";
                tr: "'{module}' kontratları saat kenarı olmayan '{name}' alanının '{clock}' \
                     saatinde denetleniyor"
            ),
            label: lstr!(
                en: "contracts are sampled on the first clock port, '{clock}' (@{name})";
                tr: "kontratlar ilk saat portunda örneklenir: '{clock}' (@{name})"
            ),
            help: lstr!(
                en: "{give_edge}; or declare a clock port of a domain with an edge before \
                     '{clock}'";
                tr: "{give_edge}; ya da '{clock}' portundan önce kenarlı bir alanın saat portunu \
                     bildirin"
            ),
            reason: lstr!(
                en: "assertions, assumptions and covers are sampled at a clock edge; {no_edge}";
                tr: "assert, assume ve cover bir saat kenarında örneklenir; {no_edge}"
            ),
        },
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

    /// `aclk @Async` (kenarsız) + `fclk @Fast` (posedge) önsözü.
    fn mixed(body: &str) -> String {
        format!(
            "{ASYNC}domain Fast {{ clock = posedge }}\nmodule M {{\n    in aclk : clock @Async\n    \
             in fclk : clock @Fast\n{body}\n}}\n"
        )
    }

    fn e3016_text(r: &super::super::testutil::Inferred) -> String {
        let d = r
            .dom
            .diagnostics
            .iter()
            .find(|d| d.code.as_str() == "E3016")
            .expect("E3016 bekleniyor");
        format!("{} | {}", d.message, d.help.as_deref().unwrap_or(""))
    }

    #[test]
    fn sync_into_an_edgeless_domain_is_e3016() {
        let r = inferred(&mixed(
            "    in d : bool @Fast\n    out q : bool @Async\n    q = sync(d, aclk)",
        ));
        assert_eq!(r.count("E3016"), 1, "{:?}", r.codes());
        let text = e3016_text(&r);
        assert!(text.contains("sync()"), "{text}");
        assert!(text.contains("clock = posedge"), "{text}");
    }

    #[test]
    fn sync_source_captured_on_an_edgeless_clock_is_e3016() {
        // Kaynak port kenarsız alanda ve o alanın bir saat portu var:
        // köprü kaynağı önce o saatle yakalar (`sync_d_src` flop'u).
        let r = inferred(&mixed(
            "    in d : bool @Async\n    out q : bool @Fast\n    q = sync(d, fclk)",
        ));
        assert_eq!(r.count("E3016"), 1, "{:?}", r.codes());
        let text = e3016_text(&r);
        assert!(text.contains("aclk"), "{text}");
    }

    #[test]
    fn sync_from_an_edgeless_domain_without_its_clock_port_is_valid() {
        // Saat portu olmayan kenarsız alandan (asenkron giriş) sync():
        // yakalama flop'u yok, iki aşama hedef saatte. Geçerli.
        let r = inferred(&format!(
            "{ASYNC}domain Fast {{ clock = posedge }}\nmodule M {{\n    in fclk : clock @Fast\n    \
             in d : bool @Async\n    out q : bool @Fast\n    q = sync(d, fclk)\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 0, "{:?}", r.codes());
    }

    #[test]
    fn builtin_primitive_on_an_edgeless_clock_is_e3016() {
        let r = inferred(&format!(
            "{ASYNC}module M {{\n    in clk : clock @Async\n    in d : u8\n    in we : bool\n    \
             out q : u8\n    let f = SyncFifo<u8, 4> {{ clk: clk, wr_data: d, wr_en: we, rd_en: true }}\n    \
             q = f.rd_data\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 1, "{:?}", r.codes());
        let text = e3016_text(&r);
        assert!(text.contains("SyncFifo"), "{text}");
    }

    #[test]
    fn child_clock_port_driven_by_an_edgeless_clock_is_e3016() {
        let r = inferred(&format!(
            "{ASYNC}module Child {{\n    in clk : clock\n    in d : u8\n    out q : u8\n    \
             reg r : u8 = 0\n    on clk {{ r <= d }}\n    q = r\n}}\n\
             module M {{\n    in clk : clock @Async\n    in d : u8\n    out q : u8\n    \
             let c = Child {{ clk: clk, d: d }}\n    q = c.q\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 1, "{:?}", r.codes());
        let text = e3016_text(&r);
        assert!(text.contains("Child"), "{text}");
    }

    #[test]
    fn child_clock_port_in_an_edgeless_domain_accepts_an_edgeless_clock() {
        // Çocuk portu da kenarsız alanda: çocuk o saatle flop zamanlayamaz
        // (kendi içinde E3016), bağlama kendisi geçerli.
        let r = inferred(&format!(
            "{ASYNC}module Child {{\n    in clk : clock @Async\n    in d : u8 @Async\n    \
             out q : u8 @Async\n    q = d\n}}\n\
             module M {{\n    in clk : clock @Async\n    in d : u8\n    out q : u8\n    \
             let c = Child {{ clk: clk, d: d }}\n    q = c.q\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 0, "{:?}", r.codes());
    }

    #[test]
    fn contracts_clocked_by_an_edgeless_clock_are_e3016() {
        let r = inferred(&format!(
            "{ASYNC}module M {{\n    in clk : clock @Async\n    in a : u8\n    out y : u8\n    \
             y = a\n    invariant: y == a\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 1, "{:?}", r.codes());
        let text = e3016_text(&r);
        assert!(text.contains("contract"), "{text}");
    }

    #[test]
    fn contracts_follow_the_first_clock_port() {
        // Kontratlar ilk saat portunun kenarında örneklenir (sv-emit);
        // ilk saat kenarlıysa ikinci, kenarsız saat kontratı etkilemez.
        let r = inferred(&format!(
            "{ASYNC}domain Fast {{ clock = posedge }}\nmodule M {{\n    in fclk : clock @Fast\n    \
             in aclk : clock @Async\n    in a : u8 @Fast\n    out y : u8 @Fast\n    \
             y = a\n    invariant: y == a\n}}\n"
        ));
        assert_eq!(r.count("E3016"), 0, "{:?}", r.codes());
    }
}
