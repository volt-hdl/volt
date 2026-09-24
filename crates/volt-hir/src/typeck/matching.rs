//! `match` deyimi desen tiplemesi ve enum kapsayıcılığı (ADR-0074 Karar 4).
//!
//! Sınanan enum ise desenler aynı enum'un varyantları olmalıdır (E2003);
//! muhafızsız kollar bütün varyantları adlandırıyorsa `_` isteğe bağlıdır,
//! eksik varyant `_`'sız E0014'tür (mesaj eksikleri listeler), aynı
//! varyantı ikinci kez adlandıran kol erişilemezdir (W2014).
//!
//! Sayısal sınananda kural değişmez (ADR-0032): E0014 parser'da tipsiz
//! verilir; önceki kolların literal değerlerini yineleyen kol enum'daki
//! gibi erişilemezdir (W2014, ADR-0075). Parser, muhafızsız kollarından
//! biri yol deseni olan `_`'sız `match`'lerde E0014'ü buraya erteler; sınanan enum değilse (yol deseni
//! + sayısal sınanan) desen E2003 alır ve parser'ın E0014'ü aynen verilir.

use volt_ast::{MatchStmt, Pattern, PatternKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::TypeChecker;
use crate::resolve::{DefId, DefKind};
use crate::ty::{EnumId, Ty, TypeId};

/// Bir kol deseninin kapsadıkları.
#[derive(Default)]
struct ArmCover {
    wildcard: bool,
    variants: Vec<DefId>,
}

impl TypeChecker<'_, '_> {
    pub(super) fn check_match_stmt(&mut self, m: &MatchStmt) {
        let scrut_ty = self.synth(m.scrutinee);
        match *self.types.ty(scrut_ty) {
            Ty::Enum(e) => self.check_enum_match(m, e),
            _ => self.check_value_match(m, scrut_ty),
        }
        for arm in &m.arms {
            self.check_arm(arm);
        }
    }

    /// Sayısal (enum olmayan) sınanan: yol desenleri E2003; parser'ın
    /// ertelediği E0014 aynen.
    fn check_value_match(&mut self, m: &MatchStmt, scrut_ty: TypeId) {
        let mut deferred = false;
        let mut has_wildcard = false;
        self.warn_unreachable_value_arms(m);
        for arm in &m.arms {
            let mut paths = Vec::new();
            let wild = self.collect_paths(arm.pattern, &mut paths);
            if arm.guard.is_none() {
                has_wildcard |= wild;
                deferred |= !paths.is_empty();
            }
            if self.types.is_error(scrut_ty) {
                continue;
            }
            for (span, def) in paths {
                let Some(def) = def else { continue };
                if let DefKind::EnumVariant { parent } = self.res.def_kind(def) {
                    let found = self.enum_name(EnumId(parent.0));
                    let expected = self.show(scrut_ty);
                    self.err_type_mismatch_msg(
                        span,
                        &lstr!(en: "pattern of enum '{found}' cannot match a value of type '{expected}'"; tr: "'{found}' enum'unun deseni '{expected}' tipinde bir değerle eşleşemez"),
                        &lstr!(en: "match on a value of type '{found}', or use integer literal patterns"; tr: "'{found}' tipinde bir değer üzerinde match yazın ya da tamsayı literal desenleri kullanın"),
                    );
                }
            }
        }
        if deferred && !has_wildcard {
            self.diagnostics.push(numeric_missing_wildcard(m.span));
        }
    }

    /// W2014: bütün literalleri önceki kollarda geçen sayısal kol (kural
    /// `volt_ast::match_cover`, sv-emit aynı kolu `case`'e yazmaz).
    fn warn_unreachable_value_arms(&mut self, m: &MatchStmt) {
        let unreachable = volt_ast::match_cover::unreachable_value_arms(self.ast, m);
        for (arm, _) in m.arms.iter().zip(unreachable).filter(|(_, u)| *u) {
            let span = self.ast.patterns[arm.pattern].span;
            self.warning(
                ErrorCode::W2014,
                span,
                lstr!(en: "unreachable arm: an earlier arm already covers this value"; tr: "erişilemez kol: bu değeri önceki bir kol zaten kapsıyor"),
                lstr!(en: "never taken"; tr: "hiç seçilmez"),
                lstr!(en: "merge the arm into the earlier one or write the value it was meant for"; tr: "kolu öncekiyle birleştirin ya da kastettiği değeri yazın"),
            );
        }
    }

    /// Enum sınanan: desen tipi, kapsayıcılık, erişilemez kol.
    fn check_enum_match(&mut self, m: &MatchStmt, e: EnumId) {
        let variants = self.enum_variants(e);
        let enum_name = self.enum_name(e);
        let mut covered: Vec<DefId> = Vec::new();
        let mut has_wildcard = false;
        for arm in &m.arms {
            let cover = self.enum_arm_cover(arm.pattern, e, &enum_name);
            if arm.guard.is_some() {
                continue; // muhafızlı kol kapsamaya sayılmaz (E0003, ADR-0032)
            }
            let fresh = cover.variants.iter().any(|v| !covered.contains(v));
            if !cover.variants.is_empty() && !fresh && !cover.wildcard && !has_wildcard {
                let span = self.ast.patterns[arm.pattern].span;
                self.warning(
                    ErrorCode::W2014,
                    span,
                    lstr!(en: "unreachable arm: an earlier arm already covers this variant of '{enum_name}'"; tr: "erişilemez kol: '{enum_name}' enum'unun bu varyantını önceki bir kol zaten kapsıyor"),
                    lstr!(en: "never taken"; tr: "hiç seçilmez"),
                    lstr!(en: "merge the arm into the earlier one or name the variant it was meant for"; tr: "kolu öncekiyle birleştirin ya da kastettiği varyantı yazın"),
                );
            }
            has_wildcard |= cover.wildcard;
            for v in cover.variants {
                if !covered.contains(&v) {
                    covered.push(v);
                }
            }
        }
        if has_wildcard {
            return;
        }
        let missing: Vec<String> = variants
            .iter()
            .filter(|(_, d)| !covered.contains(d))
            .map(|(n, _)| format!("{enum_name}::{n}"))
            .collect();
        if missing.is_empty() {
            return;
        }
        self.diagnostics
            .push(enum_not_exhaustive(m.span, &enum_name, &missing));
    }

    /// Enum sınananda bir desenin kapsadığı varyantlar; yanlış desenler
    /// E2003 alır. Bağlama deseni (çıplak `Idle`) her şeyi eşler — SV
    /// üretimi onu E0003 + `State::Idle` önerisiyle reddeder.
    fn enum_arm_cover(
        &mut self,
        pat: volt_ast::Idx<Pattern>,
        e: EnumId,
        enum_name: &str,
    ) -> ArmCover {
        let ast = self.ast;
        let mut cover = ArmCover::default();
        let span = ast.patterns[pat].span;
        match &ast.patterns[pat].kind {
            PatternKind::Wildcard | PatternKind::Binding(_) => cover.wildcard = true,
            PatternKind::Error => cover.wildcard = true, // parse tanısı zaten var
            PatternKind::Or(alts) => {
                for &a in alts {
                    let sub = self.enum_arm_cover(a, e, enum_name);
                    cover.wildcard |= sub.wildcard;
                    cover.variants.extend(sub.variants);
                }
            }
            PatternKind::Path { .. } => match self.res.pattern_resolutions.get(&pat) {
                Some(&def) => match self.res.def_kind(def) {
                    DefKind::EnumVariant { parent } if parent.0 == e.0 => cover.variants.push(def),
                    DefKind::EnumVariant { parent } => {
                        let found = self.enum_name(EnumId(parent.0));
                        self.err_pattern_type(span, &found, enum_name);
                        cover.wildcard = true;
                    }
                    // Çözülemeyen yol E1001/E1007'yi aldı; kaskad yok.
                    _ => cover.wildcard = true,
                },
                None => cover.wildcard = true,
            },
            PatternKind::Literal(_) | PatternKind::Tuple(_) => {
                let found = match &ast.patterns[pat].kind {
                    PatternKind::Literal(lit) => {
                        let t = self.synth(*lit);
                        self.show(t)
                    }
                    _ => lstr!(en: "tuple"; tr: "tuple"),
                };
                self.err_pattern_type(span, &found, enum_name);
                cover.wildcard = true;
            }
        }
        cover
    }

    fn err_pattern_type(&mut self, span: Span, found: &str, enum_name: &str) {
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "pattern of type '{found}' cannot match a value of enum '{enum_name}'"; tr: "'{found}' tipindeki desen '{enum_name}' enum'unun değeriyle eşleşemez"),
            &lstr!(en: "use the variants of '{enum_name}' as patterns ({enum_name}::...)"; tr: "desen olarak '{enum_name}' varyantlarını kullanın ({enum_name}::...)"),
        );
    }

    /// Desendeki yol desenleri (span, tanım) ve joker içerip içermediği.
    fn collect_paths(
        &self,
        pat: volt_ast::Idx<Pattern>,
        out: &mut Vec<(Span, Option<DefId>)>,
    ) -> bool {
        let p = &self.ast.patterns[pat];
        match &p.kind {
            PatternKind::Wildcard => true,
            PatternKind::Or(alts) => {
                let mut wild = false;
                for &a in alts {
                    wild |= self.collect_paths(a, out);
                }
                wild
            }
            PatternKind::Path { .. } => {
                let def = self.res.pattern_resolutions.get(&pat).copied();
                out.push((p.span, def));
                false
            }
            _ => false,
        }
    }
}

/// Enum `match`'i her varyantı kapsamıyor (E0014, ADR-0074 Karar 4).
/// `missing` boş olmamalı (`Enum::Varyant` biçiminde). Çözümleme hatalı
/// birimdeki yedek denetim de aynı tanıyı verir (ADR-0075).
pub(crate) fn enum_not_exhaustive(span: Span, enum_name: &str, missing: &[String]) -> Diagnostic {
    let list = missing.join(", ");
    let first = &missing[0];
    Diagnostic::error(
        ErrorCode::E0014,
        lstr!(en: "'match' on enum '{enum_name}' does not cover every variant: missing {list}"; tr: "'{enum_name}' enum'u üzerindeki 'match' her varyantı kapsamıyor: eksik {list}"),
        LabeledSpan::primary(
            span,
            lstr!(en: "not every variant is covered"; tr: "her varyant kapsanmıyor"),
        ),
        lstr!(en: "add an arm for each missing variant ({first} => {{ }}) or a final '_ => {{ }}' arm"; tr: "her eksik varyant için kol ({first} => {{ }}) ya da sona '_ => {{ }}' kolu ekleyin"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "an enum match that names every variant needs no '_' arm; its last arm also takes the codes no variant uses (ADR-0074)"; tr: "her varyantı adlandıran enum match'i '_' kolu gerektirmez; son kolu hiçbir varyantın kullanmadığı kodları da alır (ADR-0074)"),
    )
}

/// Parser'ın sayısal `match` E0014'ü (ADR-0032) — ertelenen yol için
/// birebir aynı tanı (golden: sayısal match'in tanısı değişmez).
fn numeric_missing_wildcard(span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E0014,
        lstr!(en: "'match' statement has no '_' arm"; tr: "'match' deyiminde '_' kolu yok"),
        LabeledSpan::primary(
            span,
            lstr!(en: "every value must be covered"; tr: "her değer kapsanmalı"),
        ),
        lstr!(en: "add a final '_ => {{ }}' arm"; tr: "sona '_ => {{ }}' kolu ekleyin"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "a match on a number covers every value only with a '_' arm (ADR-0032); an enum match is checked variant by variant instead (ADR-0074); in a sequential block an empty '_' arm keeps the registers' values"; tr: "sayı üzerindeki match her değeri yalnız '_' koluyla kapsar (ADR-0032); enum match'i bunun yerine varyant varyant denetlenir (ADR-0074); sıralı blokta boş '_' kolu register değerlerini korur"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, diagnostics};

    fn fsm(arms: &str) -> String {
        format!(
            "enum State {{ Idle, Run, Done }}\nenum Mode {{ A, B }}\nmodule M {{\n    in  clk : clock\n    in  n   : u2\n    out y   : bool\n    reg s : State = State::Idle\n    reg r : u2 = 0\n    on clk {{\n        match s {{\n{arms}\n        }}\n        r <= n\n    }}\n    y = s == State::Done && r == 0\n}}\n"
        )
    }

    const ALL: &str = "            State::Idle => { s <= State::Run }\n            State::Run => { s <= State::Done }\n            State::Done => { s <= State::Idle }";

    #[test]
    fn every_variant_named_needs_no_wildcard() {
        assert!(codes(&fsm(ALL)).is_empty(), "{:?}", codes(&fsm(ALL)));
    }

    #[test]
    fn missing_variant_without_wildcard_is_e0014_listing_it() {
        let src = fsm("            State::Idle => { s <= State::Run }\n            State::Run => { s <= State::Done }");
        let d = diagnostics(&src);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code.as_str(), "E0014");
        assert!(
            d[0].message.contains("missing State::Done"),
            "{}",
            d[0].message
        );
    }

    #[test]
    fn wildcard_covers_missing_variants() {
        let src = fsm(
            "            State::Idle => { s <= State::Run }\n            _ => { s <= State::Idle }",
        );
        assert!(codes(&src).is_empty());
    }

    #[test]
    fn or_pattern_covers_each_alternative() {
        let src = fsm("            State::Idle | State::Run => { s <= State::Done }\n            State::Done => { s <= State::Idle }");
        assert!(codes(&src).is_empty());
    }

    #[test]
    fn guarded_arm_does_not_cover() {
        let src = fsm("            State::Idle => { s <= State::Run }\n            State::Run => { s <= State::Done }\n            State::Done if r == 0 => { s <= State::Idle }");
        assert!(codes(&src).contains(&"E0014"), "{:?}", codes(&src));
    }

    #[test]
    fn second_arm_for_a_variant_is_w2014() {
        let src = fsm(&format!(
            "{ALL}\n            State::Run => {{ s <= State::Idle }}"
        ));
        assert_eq!(codes(&src), ["W2014"]);
    }

    #[test]
    fn foreign_enum_and_integer_patterns_are_e2003() {
        let other = fsm(
            "            Mode::A => { s <= State::Run }\n            _ => { s <= State::Idle }",
        );
        assert_eq!(codes(&other), ["E2003"]);
        let int =
            fsm("            0 => { s <= State::Run }\n            _ => { s <= State::Idle }");
        assert_eq!(codes(&int), ["E2003"]);
    }

    fn num(ty: &str, arms: &str) -> String {
        format!(
            "module M {{
    in  clk : clock
    in  x : {ty}
    out y : u8
    reg r : u8 = 0
    on clk {{
        match x {{
{arms}
        }}
    }}
    y = r
}}
"
        )
    }

    /// ADR-0075: sayısal yinelenen kol enum'daki gibi W2014 alır.
    #[test]
    fn repeated_numeric_value_is_w2014_whatever_its_spelling() {
        for dup in ["1", "0x1", "0b01"] {
            let src = num(
                "u2",
                &format!(
                    "            1 => {{ r <= 1 }}
            {dup} => {{ r <= 2 }}
            _ => {{ r <= 0 }}"
                ),
            );
            assert_eq!(codes(&src), ["W2014"], "{dup}");
        }
        let d = diagnostics(&num(
            "u2",
            "            1 => { r <= 1 }
            1 => { r <= 2 }
            _ => { r <= 0 }",
        ));
        assert!(
            d[0].message.contains("already covers this value"),
            "{}",
            d[0].message
        );
    }

    #[test]
    fn repeated_or_pattern_values_are_unreachable_but_partial_overlap_is_not() {
        let all = num(
            "u2",
            "            1 | 2 => { r <= 1 }
            2 | 1 => { r <= 2 }
            _ => { r <= 0 }",
        );
        assert_eq!(codes(&all), ["W2014"]);
        let partial = num(
            "u2",
            "            1 => { r <= 1 }
            0 | 1 => { r <= 2 }
            _ => { r <= 0 }",
        );
        assert!(codes(&partial).is_empty(), "{:?}", codes(&partial));
    }

    #[test]
    fn negative_zero_bool_and_signed_values_compare_by_value() {
        let neg = num(
            "i4",
            "            -1 => { r <= 1 }
            -1 => { r <= 2 }
            1 => { r <= 3 }
            _ => { r <= 0 }",
        );
        assert_eq!(codes(&neg), ["W2014"], "yalnız ikinci -1");
        let zero = num(
            "i4",
            "            0 => { r <= 1 }
            -0 => { r <= 2 }
            _ => { r <= 0 }",
        );
        assert_eq!(codes(&zero), ["W2014"]);
        let b = num(
            "bool",
            "            true => { r <= 1 }
            true => { r <= 2 }
            _ => { r <= 0 }",
        );
        assert_eq!(codes(&b), ["W2014"]);
    }

    #[test]
    fn guarded_arms_and_arms_after_wildcard_are_not_judged() {
        // Muhafızlı kol kapsamaya sayılmaz (enum kuralıyla aynı).
        let guard = num(
            "u2",
            "            1 if x == 1 => { r <= 1 }
            1 => { r <= 2 }
            _ => { r <= 0 }",
        );
        assert!(!codes(&guard).contains(&"W2014"), "{:?}", codes(&guard));
        let after = num(
            "u2",
            "            _ => { r <= 0 }
            1 => { r <= 1 }",
        );
        assert!(!codes(&after).contains(&"W2014"), "{:?}", codes(&after));
    }

    #[test]
    fn enum_pattern_on_a_number_is_e2003_and_keeps_numeric_e0014() {
        let src = "enum S { A, B }\nmodule M {\n    in  clk : clock\n    in  x : u2\n    out y : u8\n    reg r : u8 = 0\n    on clk {\n        match x {\n            S::A => { r <= 1 }\n        }\n    }\n    y = r\n}\n";
        let mut c = codes(src);
        c.sort_unstable();
        assert_eq!(c, ["E0014", "E2003"]);
        let d = diagnostics(src);
        let e = d.iter().find(|d| d.code.as_str() == "E0014").unwrap();
        assert!(e.message.contains("no '_' arm"), "{}", e.message);
    }
}
