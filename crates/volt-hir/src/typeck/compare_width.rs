//! Farklı genişlikte aritmetik karşılaştırma (ADR-0106, E2031).
//!
//! Aritmetik sonuç esnek bir genişlik aralığı taşır (ADR-0025):
//! `b + 1` (b: u3) `[3, 4]`'tür. Karşılaştırmanın iki yanı farklı doğal
//! genişlikteyse (u3 ile `[3, 4]`) karşılaştırma ya taşma bitini atar
//! (3 bitte sarar) ya da tutar (4 bit); iki okuma `b = 7` için farklı
//! sonuç verir. Volt seçmez: yazan seçer. Aralıklar kesişmeyince
//! (u3 ile u8) eski E2003 geçerlidir; bu dosya yalnız kesişen ama doğal
//! genişliği farklı yanlara bakar — fark ancak aritmetik genişlemeden
//! gelebilir (somut tipler eşit genişlikte değilse kesişmez).
//!
//! Sabit istisnası: değeri derleme anında bilinen yan karşı yanın
//! genişliğine sığıyorsa karşılaştırma iki okumada da aynıdır, hata yok.
//! Geniş yan sabitse ve dar yana sığmıyorsa karşılaştırmanın sonucu hiç
//! değişmez: E2031 bunu ayrı bir notla söyler.
//!
//! Öneri her yerde `as` ile (ADR-0106 §3): modülde, blokta ve kontratta
//! aynı yazım. Sarma `(ifade)[N-1:0] as uN` (düz `as` daraltması W2010
//! verir, §3.6), taşmayı tutma `(dar as uM)` (genişletme uyarısız).

use volt_ast::{BinOp, Expr, Idx};
use volt_diagnostics::{a_an, lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::TypeChecker;
use crate::ty::TypeId;

/// `value`, `signed` işaretli `width` bitlik bir tipe sığar mı?
fn fits(value: i128, signed: bool, width: u16) -> bool {
    let width = u32::from(width);
    if width >= 127 {
        return true;
    }
    if signed {
        let half = 1i128 << (width - 1);
        (-half..half).contains(&value)
    } else {
        (0..(1i128 << width)).contains(&value)
    }
}

/// Yardım satırındaki iki yazımın etiketleri, aynı sütuna hizalı.
fn labels(nhi: u16) -> (String, String) {
    let wrap = lstr!(en: "wrap at {nhi} bits:"; tr: "{nhi} bitte sarsın:");
    let keep = lstr!(en: "keep the carry:"; tr: "taşma kalsın:");
    let width = wrap.chars().count().max(keep.chars().count()) + 2;
    (format!("{wrap:<width$}"), format!("{keep:<width$}"))
}

/// Karşılaştırmanın iki yanı ve genişlikleri.
struct Sides {
    op: BinOp,
    lhs: Idx<Expr>,
    wide: Idx<Expr>,
    narrow: Idx<Expr>,
    whi: u16,
    nhi: u16,
    signed: bool,
}

impl TypeChecker<'_, '_> {
    /// Kesişen aralıklı iki karşılaştırma operandının doğal genişliği
    /// farklıysa E2031 (sabit istisnasıyla).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_comparison_widths(
        &mut self,
        op: BinOp,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        lt: TypeId,
        rt: TypeId,
        signed: bool,
        _span: Span,
    ) {
        let (Some((_, _, lhi)), Some((_, _, rhi))) =
            (self.types.int_range(lt), self.types.int_range(rt))
        else {
            return;
        };
        if lhi == rhi {
            return;
        }
        let (wide, narrow, whi, nhi) = if lhi > rhi {
            (lhs, rhs, lhi, rhi)
        } else {
            (rhs, lhs, rhi, lhi)
        };
        let always_same = match self.try_const_eval(wide) {
            // Sabit geniş yan dar yana sığıyorsa iki okuma aynı.
            Some(value) if fits(value, signed, nhi) => return,
            Some(value) => Some(value),
            None => {
                // Dar yan sabitse geniş yanın doğal genişliğine zaten sığar;
                // karşılaştırma doğal genişlikte yapılır (ADR-0106 §2).
                if self.try_const_eval(narrow).is_some() {
                    return;
                }
                None
            }
        };
        let sides = Sides {
            op,
            lhs,
            wide,
            narrow,
            whi,
            nhi,
            signed,
        };
        self.err_comparison_width(&sides, always_same);
    }

    fn err_comparison_width(&mut self, s: &Sides, always_same: Option<i128>) {
        let (whi, nhi) = (s.whi, s.nhi);
        let p = if s.signed { "i" } else { "u" };
        let wide_span = self.ast.exprs[s.wide].span;
        let narrow_span = self.ast.exprs[s.narrow].span;
        let help = match always_same {
            Some(value) => lstr!(
                en: "check the constant: {value} is outside the range of {p}{nhi}";
                tr: "sabiti denetleyin: {value}, {p}{nhi} aralığının dışında"
            ),
            None => self.comparison_width_help(s, p),
        };
        let (art_n, art_w) = (a_an(u64::from(nhi)), a_an(u64::from(whi)));
        let mut diag = Diagnostic::error(
            ErrorCode::E2031,
            lstr!(
                en: "comparison of {art_n} {nhi}-bit value with {art_w} {whi}-bit arithmetic result";
                tr: "{nhi} bitlik değer {whi} bitlik aritmetik sonuçla karşılaştırılıyor"
            ),
            LabeledSpan::primary(
                wide_span,
                lstr!(en: "{p}{whi}: this arithmetic keeps its carry"; tr: "{p}{whi}: bu aritmetik taşma bitini tutar"),
            ),
            help,
        )
        .with_secondary(narrow_span, lstr!(en: "{p}{nhi}"; tr: "{p}{nhi}"))
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "the comparison could drop the carry (wrap at {nhi} bits) or keep it ({whi} bits), and the two give different results; Volt does not choose";
                tr: "karşılaştırma taşma bitini atabilir ({nhi} bitte sarar) ya da tutabilir ({whi} bit) ve ikisi farklı sonuç verir; Volt seçmez"
            ),
        );
        match always_same {
            Some(value) => {
                diag = diag.with_note(
                    NoteKind::Note,
                    lstr!(
                        en: "the {whi}-bit side is the constant {value}, which no {p}{nhi} value can hold: this comparison has the same result in every cycle";
                        tr: "{whi} bitlik yan {value} sabitidir ve hiçbir {p}{nhi} değer onu tutamaz: bu karşılaştırma her çevrimde aynı sonucu verir"
                    ),
                );
            }
            None => {
                let top = nhi - 1;
                // Sıra yardım satırıyla aynı: önce sarma, sonra taşmayı tutma.
                // suggestion: e2031_inline, e2031_let, e2031_contract
                let wrap = self.suffix_fix(wide_span, &format!("[{top}:0] as {p}{nhi}"));
                // suggestion: e2031_inline, e2031_let, e2031_contract
                let keep = self.cast_fix(narrow_span, &format!("{p}{whi}"));
                diag = diag.with_suggestion(wrap).with_suggestion(keep);
            }
        }
        self.diagnostics.push(diag);
    }

    /// İki hazır yazım, önerilerin uygulanmış hâli: taşmayı atan
    /// (`(b + 1)[2:0] as u3`) ve tutan (`(a as u4)`).
    fn comparison_width_help(&self, s: &Sides, p: &str) -> String {
        let text = |e: Idx<Expr>, fallback: &str| {
            volt_syntax::parser::expr_text(self.ast, e).unwrap_or_else(|| fallback.to_string())
        };
        let (whi, nhi) = (s.whi, s.nhi);
        let w = text(s.wide, "<expr>");
        let n = text(s.narrow, "<value>");
        let sym = s.op.symbol();
        let ordered = |narrow_text: &str, wide_text: &str| {
            if s.lhs == s.narrow {
                format!("{narrow_text} {sym} {wide_text}")
            } else {
                format!("{wide_text} {sym} {narrow_text}")
            }
        };
        let top = nhi - 1;
        let wide_base = if self.takes_suffix(self.ast.exprs[s.wide].span) {
            w.clone()
        } else {
            format!("({w})")
        };
        let modular = ordered(&n, &format!("{wide_base}[{top}:0] as {p}{nhi}"));
        let arith = ordered(&format!("({n} as {p}{whi})"), &w);
        let (wrap, keep) = labels(nhi);
        lstr!(
            en: "write the width you mean with 'as':\n  {wrap}{modular}\n  {keep}{arith}";
            tr: "istediğiniz genişliği 'as' ile yazın:\n  {wrap}{modular}\n  {keep}{arith}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, diagnostics};
    use super::fits;

    fn module(body: &str) -> String {
        format!(
            "const LIMIT : u8 = 6\nconst ZERO : u8 = 0\n\nmodule M {{\n    in  clk : clock\n    in  a : u3\n    in  b : u3\n    in  c : u8\n    in  d : u8\n    in  r : u8\n    out y : bool\n\n{body}\n}}\n"
        )
    }

    #[test]
    fn fits_respects_sign_and_width() {
        assert!(fits(7, false, 3) && !fits(8, false, 3) && !fits(-1, false, 3));
        assert!(fits(-4, true, 3) && fits(3, true, 3) && !fits(4, true, 3));
    }

    #[test]
    fn inline_and_let_forms_give_the_same_error() {
        let inline = module("    y = a == b + 1");
        let via_let = module("    let s = b + 1\n    y = a == s");
        assert_eq!(codes(&inline), vec!["E2031"]);
        assert_eq!(codes(&via_let), vec!["E2031"]);
    }

    #[test]
    fn every_comparison_operator_is_checked() {
        for op in ["==", "!=", "<", "<=", ">", ">="] {
            let src = module(&format!("    y = c {op} c + d"));
            assert_eq!(codes(&src), vec!["E2031"], "{op}");
        }
    }

    #[test]
    fn constant_side_that_fits_is_accepted() {
        // Geniş yan sabit, dar yana sığıyor: sayaç sınırı deseni.
        assert!(codes(&module("    y = r == LIMIT - 1")).is_empty());
        // Dar yan sabit / literal: karşılaştırma doğal genişlikte.
        assert!(codes(&module("    y = c + d == LIMIT")).is_empty());
        assert!(codes(&module("    y = c + d == 300")).is_empty());
    }

    #[test]
    fn constant_side_that_does_not_fit_is_an_error_with_a_note() {
        let src = module("    y = r == ZERO - 1");
        let diags = diagnostics(&src);
        let d = diags
            .iter()
            .find(|d| d.code.as_str() == "E2031")
            .expect("E2031");
        assert!(
            d.notes
                .iter()
                .any(|n| n.text.contains("same result in every cycle")),
            "{:?}",
            d.notes
        );
        assert!(d.suggestions.is_empty(), "sabit yanlışsa öneri yok");
    }

    #[test]
    fn equal_natural_widths_are_accepted() {
        assert!(codes(&module("    y = c + d == c + d")).is_empty());
    }

    #[test]
    fn help_offers_both_as_forms_in_a_module() {
        let diags = diagnostics(&module("    y = a == b + 1"));
        let help = diags[0].help.clone().unwrap_or_default();
        assert!(help.contains("a == (b + 1)[2:0] as u3"), "{help}");
        assert!(help.contains("(a as u4) == b + 1"), "{help}");
        assert_eq!(diags[0].suggestions.len(), 2);
    }

    #[test]
    fn contracts_follow_the_rule_with_the_same_forms() {
        let src = module(
            "    reg l : u3 = 0\n    on clk {\n        l <= l + 1\n    }\n    invariant: l == prev(l) + 1 || prev(l) == 7",
        );
        let diags = diagnostics(&src);
        let d = diags
            .iter()
            .find(|d| d.code.as_str() == "E2031")
            .expect("E2031");
        let help = d.help.clone().unwrap_or_default();
        assert!(help.contains("l == (prev(l) + 1)[2:0] as u3"), "{help}");
        assert!(help.contains("(l as u4) == prev(l) + 1"), "{help}");
    }
}
