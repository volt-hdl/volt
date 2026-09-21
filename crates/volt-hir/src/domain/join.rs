//! K5 — `join_domains`: çıkarımın kalbi. Düz kafes: Error yutar,
//! Timeless birim elemandır, iki farklı belirli alan CDC ihlalidir
//! (E3001, kombinasyonel biçim). Çözülmemiş değişkenler kısıt olarak
//! bağlanır.

use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::{DomainId, InferVar, Inferencer};

impl Inferencer<'_> {
    /// Unresolved değişkenleri bağlarına kadar izler.
    pub(super) fn resolve_dom(&self, d: DomainId) -> DomainId {
        let mut cur = d;
        // Bağlama zinciri kısadır; döngü koruması için sınırlı adım.
        for _ in 0..64 {
            match cur {
                DomainId::Unresolved(v) => match self.vars[v.0 as usize] {
                    Some(next) => cur = next,
                    None => return cur,
                },
                other => return other,
            }
        }
        cur
    }

    pub(super) fn fresh_var(&mut self) -> DomainId {
        let v = InferVar(self.vars.len() as u32);
        self.vars.push(None);
        DomainId::Unresolved(v)
    }

    /// Unresolved tarafı varsa kısıt olarak bağla.
    pub(super) fn bind_if_var(&mut self, a: DomainId, b: DomainId) {
        match (self.resolve_dom(a), self.resolve_dom(b)) {
            (DomainId::Unresolved(v), x) | (x, DomainId::Unresolved(v)) if !matches!(x, DomainId::Unresolved(w) if w == v) =>
            {
                self.vars[v.0 as usize] = Some(x);
            }
            _ => {}
        }
    }

    /// join_domains — çıkarımın kalbi (K5).
    pub(super) fn join(&mut self, a: DomainId, b: DomainId, sa: Span, sb: Span) -> DomainId {
        let (ra, rb) = (self.resolve_dom(a), self.resolve_dom(b));
        match (ra, rb) {
            // Hata yayılımı.
            (DomainId::Error, _) | (_, DomainId::Error) => DomainId::Error,
            // Timeless her şeyle birleşir (sabitler).
            (DomainId::Timeless, x) | (x, DomainId::Timeless) => x,
            // Aynı domain → sorun yok.
            (DomainId::Explicit(x), DomainId::Explicit(y)) if x == y => ra,
            // FARKLI DOMAIN → CDC İHLALİ (glitch riski).
            (DomainId::Explicit(x), DomainId::Explicit(y)) => {
                self.err_cdc_combinational(x, y, sa, sb);
                DomainId::Error
            }
            // Çözülmemiş → kısıt biriktir.
            (DomainId::Unresolved(v), x) | (x, DomainId::Unresolved(v)) => {
                if !matches!(x, DomainId::Unresolved(w) if w == v) {
                    self.vars[v.0 as usize] = Some(x);
                }
                x
            }
        }
    }

    /// E3001 (kombinasyonel biçim, §4) — iki operand span'i + domain
    /// tanım satırlarına ikincil etiketler.
    fn err_cdc_combinational(&mut self, x: u32, y: u32, sa: Span, sb: Span) {
        let diag = Diagnostic::error(
            ErrorCode::E3001,
            lstr!(
                en: "different clock domains cannot be combined combinationally";
                tr: "farklı saat alanları kombinasyonel olarak birleşemez"
            ),
            LabeledSpan::primary(sa, format!("@{}", self.domain_name(x))),
            lstr!(
                en: "synchronize first: let s = sync(<signal>, <clock of {}>); \
                     then combine",
                    self.domain_name(y);
                tr: "önce senkronize edin: let s = sync(<sinyal>, <{} saati>); \
                     sonra birleştirin",
                    self.domain_name(y)
            ),
        )
        .with_secondary(sb, format!("@{}", self.domain_name(y)))
        .with_secondary(self.domain_span(x), self.defined_here(x))
        .with_secondary(self.domain_span(y), self.defined_here(y))
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "when signals from two clock domains meet at a gate they \
                     produce a transient pulse (glitch); the next \
                     register captures this pulse incorrectly";
                tr: "iki saat alanından gelen sinyaller kapıda birleşince \
                     geçici darbe (glitch) üretir; bu darbe sonraki \
                     register'da yanlış yakalanır"
            ),
        );
        self.diagnostics.push(diag);
    }
}

#[cfg(test)]
mod tests {
    use volt_span::{FileId, Span};

    use super::super::testutil::{with_inferencer, TWO_DOMAINS};
    use super::super::DomainId::{self, Error, Explicit, Timeless};

    fn sp(n: u32) -> Span {
        Span::new(FileId(0), n, n + 1)
    }

    /// `join(a, b)` sonucu ve ürettiği tanı sayısı.
    fn join(a: DomainId, b: DomainId) -> (DomainId, usize) {
        with_inferencer(TWO_DOMAINS, |inf| {
            let d = inf.join(a, b, sp(0), sp(2));
            (d, inf.diagnostics.len())
        })
    }

    #[test]
    fn timeless_is_the_identity_on_both_sides() {
        assert_eq!(join(Timeless, Explicit(0)), (Explicit(0), 0));
        assert_eq!(join(Explicit(1), Timeless), (Explicit(1), 0));
        assert_eq!(join(Timeless, Timeless), (Timeless, 0));
    }

    #[test]
    fn error_absorbs_everything_silently() {
        assert_eq!(join(Error, Explicit(0)), (Error, 0));
        assert_eq!(join(Explicit(0), Error), (Error, 0));
        assert_eq!(join(Timeless, Error), (Error, 0));
    }

    #[test]
    fn same_domain_joins_to_itself() {
        assert_eq!(join(Explicit(1), Explicit(1)), (Explicit(1), 0));
    }

    #[test]
    fn different_domains_are_a_cdc_violation() {
        with_inferencer(TWO_DOMAINS, |inf| {
            let d = inf.join(Explicit(0), Explicit(1), sp(0), sp(2));
            assert_eq!(d, Error);
            assert_eq!(inf.diagnostics.len(), 1);
            let diag = &inf.diagnostics[0];
            assert_eq!(diag.code.as_str(), "E3001");
            // Birincil sol operand, sonra sağ operand ve iki tanım satırı.
            assert_eq!(diag.spans.len(), 4);
            assert_eq!(diag.spans[0].span, sp(0));
            assert_eq!(diag.spans[0].label, "@Fast");
            assert_eq!(diag.spans[1].span, sp(2));
            assert_eq!(diag.spans[1].label, "@Slow");
        });
    }

    #[test]
    fn unresolved_variable_is_bound_by_join() {
        with_inferencer(TWO_DOMAINS, |inf| {
            let v = inf.fresh_var();
            assert_eq!(inf.join(v, Explicit(1), sp(0), sp(2)), Explicit(1));
            assert_eq!(inf.resolve_dom(v), Explicit(1));
            // Bağlandıktan sonra belirli alan gibi davranır.
            assert_eq!(inf.join(v, Explicit(0), sp(0), sp(2)), Error);
            assert_eq!(inf.diagnostics.len(), 1);
        });
    }

    #[test]
    fn timeless_does_not_bind_a_variable() {
        with_inferencer(TWO_DOMAINS, |inf| {
            let v = inf.fresh_var();
            assert_eq!(inf.join(Timeless, v, sp(0), sp(2)), v);
            assert_eq!(inf.resolve_dom(v), v);
        });
    }

    #[test]
    fn variable_is_never_bound_to_itself() {
        with_inferencer(TWO_DOMAINS, |inf| {
            let v = inf.fresh_var();
            inf.bind_if_var(v, v);
            assert_eq!(inf.join(v, v, sp(0), sp(2)), v);
            assert_eq!(inf.resolve_dom(v), v);
        });
    }

    #[test]
    fn resolve_dom_follows_binding_chains() {
        with_inferencer(TWO_DOMAINS, |inf| {
            let (a, b) = (inf.fresh_var(), inf.fresh_var());
            inf.bind_if_var(a, b);
            inf.bind_if_var(b, Explicit(0));
            assert_eq!(inf.resolve_dom(a), Explicit(0));
        });
    }
}
