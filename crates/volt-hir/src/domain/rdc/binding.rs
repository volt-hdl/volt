//! Ham reset portu → alan bağlaması (ADR-0065 §1 bağlama tablosu):
//! birden çok ham portta anotasyon zorunluluğu (E3010, reset biçimi),
//! port/alan reset türü uyumu (E3003) ve otomatik port adı çakışması
//! (E3003).

use volt_ast::ResetSpec;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::facts::{auto_port_name, feeds_of, spec_text, ClockFact, ModuleFacts, RawFact};
use super::Rdc;

impl Rdc<'_> {
    /// Bağlama kurallarını denetler; saat başına besleyen ham portu döner.
    pub(super) fn check_raw_bindings(&mut self, m: usize) -> Vec<Option<usize>> {
        let facts = self.facts;
        let module = &facts.modules[m];
        let feeds = feeds_of(module);
        if module.raws.len() > 1 {
            for raw in module.raws.iter().filter(|r| r.ann.is_none()) {
                self.err_ambiguous_raw(module, raw);
            }
        }
        for (ri, raw) in module.raws.iter().enumerate() {
            self.check_raw_kind(module, &feeds, ri, raw);
        }
        self.check_name_clash(module, &feeds);
        feeds
    }

    /// Ham portun türü beslediği her alanın etkin reset'iyle aynı olmalı.
    /// Tür yazılmamışsa ilk beslenen alanınki geçerlidir; ikinci alan
    /// farklıysa yine E3003 (zincirin etkinleşme polaritesi tek olmalı).
    fn check_raw_kind(
        &mut self,
        module: &ModuleFacts,
        feeds: &[Option<usize>],
        ri: usize,
        raw: &RawFact,
    ) {
        let fed: Vec<&ClockFact> = module
            .clocks
            .iter()
            .zip(feeds)
            .filter(|(_, f)| **f == Some(ri))
            .map(|(c, _)| c)
            .collect();
        let Some(expected) = raw.spec.or_else(|| fed.first().and_then(|c| c.reset)) else {
            return;
        };
        let mut reported = Vec::new();
        for clock in fed {
            let Some(actual) = clock.reset else { continue };
            if actual != expected && !reported.contains(&clock.reset_span) {
                reported.push(clock.reset_span);
                self.err_kind_mismatch(raw, expected, clock, actual);
            }
        }
    }

    /// Beslenmeyen alanlar otomatik port alır; ham port o adı taşıyamaz.
    fn check_name_clash(&mut self, module: &ModuleFacts, feeds: &[Option<usize>]) {
        for raw in &module.raws {
            let clash = module
                .clocks
                .iter()
                .zip(feeds)
                .find(|(c, f)| {
                    f.is_none()
                        && c.reset
                            .is_some_and(|s| auto_port_name(s) == raw.port.name.text)
                })
                .map(|(c, _)| c);
            if let Some(clock) = clash {
                self.err_name_clash(raw, clock);
            }
        }
    }

    /// E3010 (reset biçimi) — birden çok ham portta anotasyonsuz olan.
    fn err_ambiguous_raw(&mut self, module: &ModuleFacts, raw: &RawFact) {
        let name = &raw.port.name.text;
        let mut diag = Diagnostic::error(
            ErrorCode::E3010,
            lstr!(en: "cannot determine which clock domains the raw reset '{name}' feeds";
                  tr: "'{name}' ham reset'inin hangi saat alanlarını beslediği belirlenemiyor"),
            LabeledSpan::primary(
                raw.port.name.span,
                lstr!(en: "one of several raw reset ports, without a domain annotation";
                      tr: "birden çok ham reset portundan biri, alan anotasyonu yok"),
            ),
            lstr!(en: "annotate every raw reset port with the domain it resets: \
                       in {name} : reset @Domain";
                  tr: "her ham reset portuna sıfırladığı alanı yazın: \
                       in {name} : reset @Alan"),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "a single raw reset port feeds every domain of the module; with more \
                       than one, each must name its domain (ADR-0065 §1)";
                  tr: "tek ham reset portu modülün bütün alanlarını besler; birden çok \
                       olduğunda her biri alanını belirtmelidir (ADR-0065 §1)"),
        );
        for other in module.raws.iter().filter(|r| r.def != raw.def) {
            diag = diag.with_secondary(
                other.port.name.span,
                lstr!(en: "another raw reset port"; tr: "diğer ham reset portu"),
            );
        }
        self.diagnostics.push(diag);
    }

    /// E3003 (tür biçimi) — ham port türü beslediği alanınkinden farklı.
    fn err_kind_mismatch(
        &mut self,
        raw: &RawFact,
        expected: ResetSpec,
        clock: &ClockFact,
        actual: ResetSpec,
    ) {
        let name = &raw.port.name.text;
        let (port_kind, dom_kind) = (spec_text(expected), spec_text(actual));
        let clk = &clock.port.name.text;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E3003,
                lstr!(en: "raw reset '{name}' is {port_kind} but the domain of '{clk}' expects {dom_kind}";
                      tr: "'{name}' ham reset'i {port_kind} ama '{clk}' alanı {dom_kind} bekliyor"),
                LabeledSpan::primary(
                    raw.port.span,
                    lstr!(en: "port reset kind: {port_kind}"; tr: "port reset türü: {port_kind}"),
                ),
                lstr!(en: "make the port match the domain: in {name} : {dom_kind}, \
                           or annotate it with the domain it is meant for";
                      tr: "portu alanla eşleştirin: in {name} : {dom_kind} ya da \
                           port için amaçlanan alanı anotasyonla yazın"),
            )
            .with_secondary(
                clock.reset_span,
                lstr!(en: "domain reset: {dom_kind}"; tr: "alan reset'i: {dom_kind}"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "the synchronizer asserts with the port's polarity and releases the \
                           domain's registers; the two must agree (ADR-0065 §1)";
                      tr: "senkronizör portun polaritesiyle etkinleşir ve alanın register'larını \
                           bırakır; ikisi uyuşmalıdır (ADR-0065 §1)"),
            ),
        );
    }

    /// E3003 (ad biçimi) — ham port, beslenmeyen bir alanın otomatik
    /// portuyla aynı adda (SV'de iki `input` aynı adla).
    fn err_name_clash(&mut self, raw: &RawFact, clock: &ClockFact) {
        let name = &raw.port.name.text;
        let clk = &clock.port.name.text;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E3003,
                lstr!(en: "raw reset '{name}' has the name of the automatic reset port of '{clk}'";
                      tr: "'{name}' ham reset'i '{clk}' saatinin otomatik reset portuyla aynı adda"),
                LabeledSpan::primary(
                    raw.port.name.span,
                    lstr!(en: "raw reset port"; tr: "ham reset portu"),
                ),
                lstr!(en: "remove the annotation so '{name}' feeds every domain, or rename it";
                      tr: "'{name}' bütün alanları beslesin diye anotasyonu kaldırın ya da yeniden adlandırın"),
            )
            .with_secondary(
                clock.port.name.span,
                lstr!(en: "this clock's domain is not fed by '{name}', so it gets an automatic '{name}' port";
                      tr: "bu saatin alanı '{name}' ile beslenmiyor, otomatik bir '{name}' portu alır"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "the generated module would have two inputs called '{name}' — one raw, \
                           one released synchronously (ADR-0065 §1)";
                      tr: "üretilen modülde '{name}' adlı iki giriş olurdu — biri ham, biri \
                           senkron bırakılan (ADR-0065 §1)"),
            ),
        );
    }
}
