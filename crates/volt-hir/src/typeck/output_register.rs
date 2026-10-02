//! Çıkış portuna `on` bloğunda `<=` (E0020, ADR-0098 eki 2).
//!
//! `out q : u8` ve `on clk { q <= d }` reset dalı boş bir flop
//! üretiyordu (`if (rst) begin end else q <= d`). Port bildiriminin
//! başlangıç değeri yoktur (grammar-full.ebnf:164-165 `Port`); reset
//! değeri yalnız `reg` bildiriminden gelir (sv-mapping.md:152-153) ve
//! çıkış portu bir atamayla sürülür (sv-mapping.md:189-193 §5.1,
//! type-inference.md:102 `port_out = expr`). Çıkış portu bu yüzden tel
//! gibidir: E0020'nin tel biçimiyle aynı kural.
//!
//! Denetim parser'da değil burada: bundle alanları (ADR-0039) ancak
//! düzleştirmeden sonra yönleriyle görünür (`in r : Bus` portunun `out`
//! alanı modülün çıkışıdır) ve birimin başka dosyasındaki tip metni
//! parser'da okunamaz.

use std::collections::HashMap;

use volt_ast::{Expr, Idx, LValue, ModuleDecl, PortDir};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::TypeChecker;
use crate::resolve::DefId;
use crate::ty::{Ty, TypeId};

/// Modülün bir çıkış portu (E0020 iletisi için).
#[derive(Debug, Clone)]
pub(super) struct OutPort {
    /// Kullanıcının yazdığı ad: `q`, bundle alanında `b.valid`.
    shown: String,
    /// Düz ad (SV adı): `q`, `b_valid`.
    flat: String,
    decl: Span,
    ty: TypeId,
}

/// Modülün çıkış portları, tanıma göre.
pub(super) type OutPorts = HashMap<DefId, OutPort>;

impl TypeChecker<'_, '_> {
    /// Modülün çıkış portlarını toplar (port tipleri kaydedildikten sonra).
    pub(super) fn collect_out_ports(&mut self, m: &ModuleDecl) {
        self.out_ports.clear();
        for p in m.ports.iter().filter(|p| p.direction == PortDir::Out) {
            let Some(&def) = self.res.decl_spans.get(&p.name.span) else {
                continue;
            };
            let shown = match &p.bundle {
                Some(b) => format!("{}.{}", b.port.text, b.path),
                None => p.name.text.clone(),
            };
            let ty = self.def_type(def);
            self.out_ports.insert(
                def,
                OutPort {
                    shown,
                    flat: p.name.text.clone(),
                    decl: p.span,
                    ty,
                },
            );
        }
    }

    /// `on` bloğundaki `lhs <= rhs`: hedef bir çıkış portuysa E0020.
    pub(super) fn check_output_nonblocking(&mut self, lhs: &LValue, rhs: Idx<Expr>) {
        let Some(port) = self
            .res
            .use_spans
            .get(&lhs.base.span)
            .and_then(|def| self.out_ports.get(def))
            .cloned()
        else {
            return;
        };
        let stmt = Span {
            start: lhs.span.start,
            end: self.ast.exprs[rhs].span.end,
            ..lhs.span
        };
        let diag = self.output_nonblocking_diag(&port, stmt);
        self.diagnostics.push(diag);
    }

    /// E0020 çıkış biçimi, 5 parça: kod, konum, açıklama, öneri, gerekçe
    /// (explain E0020 → ADR-0098).
    fn output_nonblocking_diag(&self, port: &OutPort, stmt: Span) -> Diagnostic {
        let OutPort {
            shown, flat, decl, ..
        } = port;
        let reg = format!("{flat}_r");
        let ty = self.types.display_named(port.ty, &self.res.defs);
        // Sıfırı tek sözcükle yazılabilen tipler; diğerlerinde yer tutucu.
        let kind = self.types.ty(port.ty);
        let value = if matches!(kind, Ty::Bool) {
            "false"
        } else if matches!(kind, Ty::UInt { .. } | Ty::SInt { .. } | Ty::Bits { .. }) {
            "0"
        } else {
            "<reset value>"
        };
        Diagnostic::error(
            ErrorCode::E0020,
            lstr!(
                en: "output port '{shown}' cannot be assigned with '<=' in an 'on' block";
                tr: "çıkış portu '{shown}' bir 'on' bloğunda '<=' ile atanamaz"
            ),
            LabeledSpan::primary(
                stmt,
                lstr!(
                    en: "this would make '{shown}' a flip-flop without a reset value";
                    tr: "bu, '{shown}' çıkışını reset değeri olmayan bir flop yapardı"
                ),
            ),
            lstr!(
                en: "keep the value in a register and drive the port from it: declare \
                     'reg {reg} : {ty} = {value}' (its value after reset), write '{reg} <= ...' \
                     in the 'on' block and add '{shown} = {reg}' at module level";
                tr: "değeri bir register'da tutun ve portu ondan sürün: 'reg {reg} : {ty} = \
                     {value}' bildirin (reset sonrası değeri), 'on' bloğunda '{reg} <= ...' \
                     yazın ve modül düzeyinde '{shown} = {reg}' ekleyin"
            ),
        )
        .with_secondary(
            *decl,
            lstr!(
                en: "'{shown}' is declared as an output port here";
                tr: "'{shown}' burada çıkış portu olarak bildirildi"
            ),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "a port declaration has no reset value, only a 'reg' declaration does; \
                     written in an 'on' block the output keeps, after reset, whatever value it \
                     had before (unknown in hardware)";
                tr: "port bildiriminin reset değeri yoktur, yalnız 'reg' bildiriminin vardır; \
                     'on' bloğunda yazılınca çıkış reset sonrasında önceki değerini korur \
                     (donanımda bilinmez)"
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::diagnostics;

    fn e0020s(src: &str) -> Vec<volt_diagnostics::Diagnostic> {
        diagnostics(src)
            .into_iter()
            .filter(|d| d.code.as_str() == "E0020")
            .collect()
    }

    #[test]
    fn output_written_with_nonblocking_in_on_block_is_e0020() {
        let d = e0020s(
            "module M {\n    in clk : clock\n    in d : u8\n    out q : u8\n    \
             on clk { q <= d }\n}\n",
        );
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].message.contains("output port 'q'"), "{}", d[0].message);
        let help = d[0].help.as_deref().unwrap_or("");
        assert!(help.contains("reg q_r : u8 = 0"), "{help}");
        assert!(help.contains("q = q_r"), "{help}");
        // Birincil etiket atamada, ikincil etiket port bildiriminde.
        assert_eq!(d[0].spans.len(), 2, "{:?}", d[0].spans);
    }

    #[test]
    fn nested_and_partial_output_writes_are_each_reported() {
        let d = e0020s(
            "module M {\n    in clk : clock\n    in en : bool\n    in d : bool\n    \
             out q : [bool; 2]\n    on clk {\n        if en { for i in 0..2 { q[i] <= d } }\n        \
             else { q[0] <= false }\n    }\n}\n",
        );
        assert_eq!(d.len(), 2, "{d:?}");
        // Dizi tipinin sıfırı tek sözcük değil: yer tutucu.
        let help = d[0].help.as_deref().unwrap_or("");
        assert!(help.contains("<reset value>"), "{help}");
    }

    #[test]
    fn bundle_output_fields_are_e0020_in_both_directions() {
        // `out b : Bus` alanı `valid` ve ters çevrilmiş `in r : Bus`
        // portunun `ready` alanı modülün çıkışlarıdır.
        let d = e0020s(
            "struct port Bus {\n    out valid : bool\n    in ready : bool\n}\n\
             module M {\n    in clk : clock\n    in x : bool\n    out b : Bus\n    in r : Bus\n    \
             on clk {\n        b.valid <= x\n        r.ready <= x\n    }\n}\n",
        );
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d[0].message.contains("'b.valid'"), "{}", d[0].message);
        assert!(d[1].message.contains("'r.ready'"), "{}", d[1].message);
        let help = d[1].help.as_deref().unwrap_or("");
        assert!(help.contains("reg r_ready_r : bool = false"), "{help}");
    }

    #[test]
    fn registered_outputs_and_combinational_outputs_are_valid() {
        let d = e0020s(
            "module M {\n    in clk : clock\n    in d : u8\n    out q : u8\n    out y : u8\n    \
             out z : u8\n    reg r : u8 = 0\n    on clk { r <= d }\n    q = r\n    \
             comb { y = d }\n    z = d + 1\n}\n",
        );
        assert!(d.is_empty(), "{d:?}");
    }
}
