//! K6/K7 — atama uyumu ve 'on' bloğu içi: hedef ile kaynak aynı
//! alanda olmalı (E3001, atama biçimi); bloğun koşulu/seçicisi yabancı
//! alandan okunamaz (E3012).

use volt_ast::{Expr, Idx, LValue, LValueSuffix};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::{DomainId, Inferencer};

impl Inferencer<'_> {
    /// K7 — 'on' bloğu koşulunda/seçicisinde yabancı domain (E3012).
    pub(super) fn check_foreign_read(&mut self, dom: DomainId, span: Span, ctx: (DomainId, Span)) {
        let (block_dom, trigger_span) = ctx;
        let (d, b) = (self.resolve_dom(dom), self.resolve_dom(block_dom));
        let (DomainId::Explicit(x), DomainId::Explicit(y)) = (d, b) else {
            self.bind_if_var(dom, block_dom);
            return;
        };
        if x != y {
            self.err_foreign_read(d, (x, y), span, trigger_span);
        }
    }

    /// E3012, 5 parça: `x` okunan yabancı alan, `y` bloğun alanı.
    fn err_foreign_read(
        &mut self,
        d: DomainId,
        (x, y): (u32, u32),
        span: Span,
        trigger_span: Span,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E3012,
                lstr!(
                    en: "a signal from a foreign clock domain is read in an 'on' block";
                    tr: "'on' bloğunda yabancı saat alanından sinyal okunuyor"
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(
                        en: "comes from the {} domain", self.display(d);
                        tr: "{} alanından geliyor", self.display(d)
                    ),
                ),
                lstr!(
                    en: "synchronize it first: sync(<signal>, <clock of {}>)",
                        self.domain_name(y);
                    tr: "önce senkronize edin: sync(<sinyal>, <{} saati>)",
                        self.domain_name(y)
                ),
            )
            .with_secondary(
                trigger_span,
                lstr!(
                    en: "the block is in the @{} domain", self.domain_name(y);
                    tr: "blok @{} alanında", self.domain_name(y)
                ),
            )
            .with_secondary(self.domain_span(x), self.defined_here(x))
            .with_note(
                NoteKind::Reason,
                lstr!(
                    en: "a signal arriving from a foreign clock may be sampled during \
                         an unstable window by this block's registers (metastability)";
                    tr: "yabancı saatten gelen sinyal bu bloğun register'larında \
                         kararsız anda yakalanabilir (metastabilite)"
                ),
            ),
        );
    }

    pub(super) fn check_assign(
        &mut self,
        lhs: &LValue,
        rhs: Idx<Expr>,
        ctx: Option<(DomainId, Span)>,
    ) {
        let rhs_dom = self.expr_domain(rhs);
        let lhs_dom = self
            .use_def(lhs.base.span)
            .and_then(|def| self.signal_domains.get(&def).copied())
            .unwrap_or(DomainId::Timeless);

        // İndeks/aralık ifadeleri de okumadır — domain'leri hesaplanır.
        for suffix in &lhs.suffixes {
            match suffix {
                LValueSuffix::Index(e) => {
                    let d = self.expr_domain(*e);
                    self.check_compat(lhs_dom, d, lhs.span, self.ast.exprs[*e].span);
                }
                LValueSuffix::Range { hi, lo } => {
                    self.expr_domain(*hi);
                    self.expr_domain(*lo);
                }
                LValueSuffix::PartSelect { start, width, .. } => {
                    let d = self.expr_domain(*start);
                    self.check_compat(lhs_dom, d, lhs.span, self.ast.exprs[*start].span);
                    self.expr_domain(*width);
                }
                LValueSuffix::Field(_) => {}
            }
        }

        // 'on' bloğunda yazılan sinyal bloğun domain'inde olmalı (K7).
        if let Some((block_dom, trigger_span)) = ctx {
            let (l, b) = (self.resolve_dom(lhs_dom), self.resolve_dom(block_dom));
            if let (DomainId::Explicit(x), DomainId::Explicit(y)) = (l, b) {
                if x != y {
                    self.err_cdc_assign(lhs_dom, block_dom, lhs.span, trigger_span);
                    return;
                }
            }
        }

        self.check_compat(lhs_dom, rhs_dom, lhs.span, self.ast.exprs[rhs].span);
    }

    /// K6 — hedef ile kaynak aynı alanda mı? Timeless muaf.
    pub(super) fn check_compat(
        &mut self,
        dst: DomainId,
        src: DomainId,
        dst_span: Span,
        src_span: Span,
    ) {
        let (d, s) = (self.resolve_dom(dst), self.resolve_dom(src));
        match (d, s) {
            // Sabit her yere atanabilir; hata kaskadı bastırılır.
            (_, DomainId::Timeless) | (DomainId::Timeless, _) => {}
            (DomainId::Error, _) | (_, DomainId::Error) => {}
            (DomainId::Explicit(x), DomainId::Explicit(y)) if x == y => {}
            (DomainId::Explicit(_), DomainId::Explicit(_)) => {
                self.err_cdc_assign(d, s, dst_span, src_span);
            }
            _ => self.bind_if_var(d, s),
        }
    }

    /// E3001 (atama biçimi) — iki span: hedef ve kaynak alanları,
    /// domain tanım satırlarına ikincil etiketler (§4).
    fn err_cdc_assign(&mut self, dst: DomainId, src: DomainId, dst_span: Span, src_span: Span) {
        let (d, s) = (self.resolve_dom(dst), self.resolve_dom(src));
        let dst_name = match d {
            DomainId::Explicit(id) => Some(self.domain_name(id).to_string()),
            _ => None,
        };
        let diag = Diagnostic::error(
            ErrorCode::E3001,
            lstr!(
                en: "direct assignment between clock domains";
                tr: "saat alanları arasında doğrudan atama"
            ),
            LabeledSpan::primary(dst_span, self.display(d)),
            lstr!(
                en: "synchronize into the target domain with sync(): \
                     dest = sync(src, <clock of {}>)",
                    dst_name.clone().unwrap_or_else(|| "dest".to_string());
                tr: "sync() ile hedef alana senkronize edin: \
                     hedef = sync(kaynak, <{} saati>)",
                    dst_name.clone().unwrap_or_else(|| "hedef".to_string())
            ),
        )
        .with_secondary(src_span, self.display(s))
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "the destination register may sample the source signal \
                     during an unstable window (metastability)";
                tr: "hedef register kaynak sinyali kararsız anda \
                     yakalayabilir (metastabilite)"
            ),
        )
        .with_note(
            NoteKind::Note,
            lstr!(
                en: "for multi-bit data, AsyncFifo may be safer";
                tr: "çok bitli veri için AsyncFifo daha güvenli olabilir"
            ),
        );
        let diag = self.with_endpoint_defs(diag, d, s);
        self.diagnostics.push(diag);
    }

    /// Hedef ve kaynak alanlarının tanım satırlarına ikincil etiketler.
    fn with_endpoint_defs(&self, mut diag: Diagnostic, d: DomainId, s: DomainId) -> Diagnostic {
        if let DomainId::Explicit(id) = d {
            diag = diag.with_secondary(
                self.domain_span(id),
                lstr!(
                    en: "destination @{} defined here", self.domain_name(id);
                    tr: "hedef @{} burada tanımlı", self.domain_name(id)
                ),
            );
        }
        if let DomainId::Explicit(id) = s {
            diag = diag.with_secondary(
                self.domain_span(id),
                lstr!(
                    en: "source @{} defined here", self.domain_name(id);
                    tr: "kaynak @{} burada tanımlı", self.domain_name(id)
                ),
            );
        }
        diag
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};

    #[test]
    fn cross_domain_assignment_is_e3001_with_both_definitions() {
        let r = inferred(&two_clock(
            "    in a : u8 @Fast\n    out q : u8 @Slow\n    q = a",
        ));
        assert_eq!(r.codes(), ["E3001"]);
        let d = &r.dom.diagnostics[0];
        // Birincil hedef, kaynak, hedef tanımı, kaynak tanımı.
        assert_eq!(d.spans.len(), 4);
        assert_eq!(d.spans[0].label, "@Slow");
        assert_eq!(d.spans[1].label, "@Fast");
        assert!(d.spans[2].label.contains("@Slow") && d.spans[3].label.contains("@Fast"));
        assert!(d.help.as_deref().is_some_and(|h| h.contains("Slow")));
    }

    #[test]
    fn constants_are_assignable_to_any_domain() {
        let r = inferred(&two_clock(
            "    out p : u8 @Fast\n    out q : u8 @Slow\n    p = 1\n    q = 2",
        ));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn writing_a_foreign_register_in_an_on_block_is_e3001() {
        let r = inferred(&two_clock(
            "    reg(fast_clk) r : u8 = 0\n    on slow_clk {\n        r <= 1\n    }",
        ));
        assert_eq!(r.codes(), ["E3001"]);
    }

    #[test]
    fn foreign_condition_in_an_on_block_is_e3012() {
        let r = inferred(&two_clock(
            "    in ff : bool @Fast\n    in sd : u8 @Slow\n    reg r : u8 = 0\n    \
             on slow_clk {\n        if ff {\n            r <= sd\n        }\n    }",
        ));
        assert_eq!(r.codes(), ["E3012"]);
        let d = &r.dom.diagnostics[0];
        assert_eq!(d.spans.len(), 3);
        assert!(d.spans[0].label.contains("@Fast"));
        assert!(d.spans[1].label.contains("@Slow"));
    }

    #[test]
    fn foreign_match_selector_in_an_on_block_is_e3012() {
        let r = inferred(&two_clock(
            "    in fs : u2 @Fast\n    reg r : u8 = 0\n    on slow_clk {\n        \
             match fs {\n            0 => { r <= 1 }\n            _ => { r <= 2 }\n        }\n    }",
        ));
        assert_eq!(r.codes(), ["E3012"]);
    }

    #[test]
    fn unconstrained_wire_read_in_an_on_block_is_bound_to_the_block_domain() {
        // Koşuldaki `w` henüz kısıtsız: bloğun alanına (@Slow) bağlanır,
        // sonraki @Fast ataması bu yüzden CDC ihlalidir. Bağlama olmasa
        // `w = a` onu sessizce @Fast yapar ve ihlal kaçardı.
        let r = inferred(&two_clock(
            "    in a : bool @Fast\n    wire w : bool\n    reg r : u8 = 0\n    \
             on slow_clk {\n        if w {\n            r <= 1\n        }\n    }\n    w = a",
        ));
        assert_eq!(r.codes(), ["E3001"]);
        assert_eq!(r.dom.diagnostics[0].spans[0].label, "@Slow");
    }

    #[test]
    fn foreign_index_on_the_left_side_is_e3001() {
        let r = inferred(&two_clock(
            "    in i : u2 @Fast\n    out q : bits<4> @Slow\n    q[i] = true",
        ));
        assert_eq!(r.codes(), ["E3001"]);
    }
}
