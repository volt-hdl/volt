//! K9 — `sync()` köprüsü: tek meşru CDC geçiş yolu.

use volt_ast::{Expr, Idx};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::{DomainId, Inferencer};

impl Inferencer<'_> {
    /// Tek meşru CDC geçiş yolu: çıkış hedef saat alanındadır.
    pub(super) fn sync_domain(&mut self, args: &[Idx<Expr>], span: Span) -> DomainId {
        let Some(&data) = args.first() else {
            return DomainId::Error;
        };
        let src = self.expr_domain(data);
        let dst = match args.get(1) {
            Some(&clk) => self.expr_domain(clk),
            None => DomainId::Error,
        };

        // Aynı domain → gereksiz senkronizatör (W3002).
        if let (DomainId::Explicit(a), DomainId::Explicit(b)) =
            (self.resolve_dom(src), self.resolve_dom(dst))
        {
            if a == b {
                self.warn_same_domain_sync(a, span);
            }
        }

        // Çok bitli veri uyarısı (W3003).
        if let Some(&ty) = self.tyck.expr_types.get(&data) {
            if let Some(w) = self.tyck.types.width_of(ty) {
                if w > 1 {
                    self.warn_multibit_sync(w, span);
                }
            }
        }

        dst
    }

    /// W3002 — kaynak ve hedef aynı alanda: gereksiz senkronizatör.
    fn warn_same_domain_sync(&mut self, a: u32, span: Span) {
        self.diagnostics.push(
            Diagnostic::warning(
                ErrorCode::W3002,
                lstr!(
                    en: "sync() is unnecessary within the same clock domain";
                    tr: "sync() aynı saat alanı içinde gereksiz"
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(
                        en: "source and destination are both @{}", self.domain_name(a);
                        tr: "kaynak ve hedef @{}", self.domain_name(a)
                    ),
                ),
                lstr!(
                    en: "a direct assignment is enough — remove the sync() call";
                    tr: "doğrudan atama yeterli — sync() çağrısını kaldırın"
                ),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(
                    en: "a synchronizer is only needed when crossing between \
                         domains; within the same domain it adds 2 cycles of latency";
                    tr: "senkronizatör yalnız alanlar arası geçişte gerekir; \
                         aynı alanda 2 çevrim gecikme ekler"
                ),
            ),
        );
    }

    /// W3003 — çok bitli veri iki-flop ile bit tutarlı taşınamaz.
    fn warn_multibit_sync(&mut self, w: u16, span: Span) {
        self.diagnostics.push(
            Diagnostic::warning(
                ErrorCode::W3003,
                lstr!(
                    en: "two-flop synchronization does not guarantee \
                         bit coherence for {w}-bit signals";
                    tr: "{w}-bit sinyal için iki-flop senkronizasyonu \
                         bit tutarlılığı garanti etmez"
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(
                        en: "bits may be captured on different edges";
                        tr: "bitler farklı kenarlarda yakalanabilir"
                    ),
                ),
                lstr!(
                    en: "for multi-bit data use one of:\n           \
                         AsyncFifo<T, N>   — data streams\n           \
                         HandshakeSync<T>  — single transfers\n           \
                         gray coding       — counters\n           \
                         AsyncDualPortRam<T, N> — random-access data";
                    tr: "çok bitli veri için şunlardan birini kullanın:\n           \
                         AsyncFifo<T, N>   — veri akışları\n           \
                         HandshakeSync<T>  — tek transferler\n           \
                         gray kodlama      — sayaçlar\n           \
                         AsyncDualPortRam<T, N> — rastgele erişimli veri"
                ),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(
                    en: "a two-flop synchronizer synchronizes each bit \
                         independently; when bits are captured on different \
                         clock edges an invalid intermediate value appears";
                    tr: "iki-flop senkronizatör her biti bağımsız senkronize \
                         eder; bitler farklı saat kenarlarında yakalanınca \
                         geçersiz ara değer oluşur"
                ),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};

    #[test]
    fn sync_output_is_in_the_destination_clock_domain() {
        let r = inferred(&two_clock(
            "    in a : bool @Fast\n    out q : bool @Slow\n    let s = sync(a, slow_clk)\n    q = s",
        ));
        assert_eq!(r.domain_name_of("s"), Some("Slow"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn sync_within_one_domain_warns_w3002() {
        let r = inferred(&two_clock(
            "    in a : bool @Slow\n    let s = sync(a, slow_clk)",
        ));
        assert_eq!(r.codes(), ["W3002"]);
    }

    #[test]
    fn multi_bit_sync_warns_w3003() {
        let r = inferred(&two_clock(
            "    in a : u8 @Fast\n    let s = sync(a, slow_clk)",
        ));
        assert_eq!(r.codes(), ["W3003"]);
        assert!(r.dom.diagnostics[0].message.contains('8'));
    }

    #[test]
    fn same_domain_multi_bit_sync_warns_in_order() {
        let r = inferred(&two_clock(
            "    in a : u8 @Slow\n    let s = sync(a, slow_clk)",
        ));
        assert_eq!(r.codes(), ["W3002", "W3003"]);
    }
}
