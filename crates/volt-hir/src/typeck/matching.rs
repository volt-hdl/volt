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

use volt_ast::{MatchArm, MatchStmt, Pattern, PatternKind};
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
        self.check_match_patterns(m.span, scrut_ty, &m.arms, false);
        for arm in &m.arms {
            self.check_arm(arm);
        }
    }

    /// Desen tipleri, kapsayıcılık ve erişilemez kollar — deyim ve ifade
    /// `match`'inde aynı kural (ADR-0083 Karar 1, 3). `is_expr` yalnız
    /// tanı metnini seçer.
    pub(super) fn check_match_patterns(
        &mut self,
        span: Span,
        scrut_ty: TypeId,
        arms: &[MatchArm],
        is_expr: bool,
    ) {
        match *self.types.ty(scrut_ty) {
            Ty::Enum(e) => self.check_enum_match(span, arms, e, is_expr),
            _ => self.check_value_match(span, arms, scrut_ty, is_expr),
        }
    }

    /// Sayısal (enum olmayan) sınanan: yol desenleri E2003; parser'ın
    /// ertelediği E0014 aynen.
    fn check_value_match(
        &mut self,
        span: Span,
        arms: &[MatchArm],
        scrut_ty: TypeId,
        is_expr: bool,
    ) {
        let mut deferred = false;
        let mut has_wildcard = false;
        self.warn_unreachable_value_arms(arms);
        for arm in arms {
            let mut paths = Vec::new();
            let wild = self.collect_paths(arm.pattern, &mut paths);
            if arm.guard.is_none() {
                has_wildcard |= wild;
                deferred |= !paths.is_empty();
            }
            if self.types.is_error(scrut_ty) {
                continue;
            }
            self.check_value_pattern(arm.pattern, scrut_ty);
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
            self.diagnostics
                .push(numeric_missing_wildcard(span, is_expr));
        }
    }

    /// Sayısal sınananda değer deseni `x == P` gibi tiplenir (ADR-0085):
    /// literal sınananın tipine sığmalı (E2010 — `u8` üzerinde `300`
    /// SV'de `8'd300` = 44 olurdu), `const` adı sınanana atanabilir olmalı
    /// (E2001/E2002), enum tipli `const` adı yol deseniyle aynı E2003'ü
    /// alır. Tamsayı olmayan sınanan (bits, bool) eski kuralda kalır.
    fn check_value_pattern(&mut self, pat: volt_ast::Idx<Pattern>, scrut_ty: TypeId) {
        let ast = self.ast;
        match &ast.patterns[pat].kind {
            PatternKind::Or(alts) => {
                for &a in alts {
                    self.check_value_pattern(a, scrut_ty);
                }
            }
            PatternKind::Literal(lit) => {
                let lit = *lit;
                let numeric = self.types.int_range(scrut_ty).is_some();
                if !matches!(ast.exprs[lit].kind, volt_ast::ExprKind::Path(_)) {
                    if numeric {
                        self.check(lit, scrut_ty);
                    }
                    return;
                }
                let t = self.synth(lit);
                if let Ty::Enum(found) = *self.types.ty(t) {
                    let found = self.enum_name(found);
                    let expected = self.show(scrut_ty);
                    self.err_type_mismatch_msg(
                        ast.patterns[pat].span,
                        &lstr!(en: "pattern of enum '{found}' cannot match a value of type '{expected}'"; tr: "'{found}' enum'unun deseni '{expected}' tipinde bir değerle eşleşemez"),
                        &lstr!(en: "match on a value of type '{found}', or use integer literal patterns"; tr: "'{found}' tipinde bir değer üzerinde match yazın ya da tamsayı literal desenleri kullanın"),
                    );
                } else if numeric {
                    self.check_const_pattern_fits(lit, t, scrut_ty, ast.patterns[pat].span);
                }
            }
            _ => {}
        }
    }

    /// `const` adı deseninin değeri sınananın aralığında mı (E2010)?
    /// Sayı olmayan const (bool, bits) tip uyuşmazlığıdır (E2003).
    fn check_const_pattern_fits(
        &mut self,
        lit: volt_ast::Idx<volt_ast::Expr>,
        t: TypeId,
        scrut_ty: TypeId,
        span: Span,
    ) {
        if self.types.is_error(t) {
            return;
        }
        if self.types.int_range(t).is_none() {
            self.err_type_mismatch(scrut_ty, t, span);
            return;
        }
        let Some((signed, _, width)) = self.types.int_range(scrut_ty) else {
            return;
        };
        let Some(value) = self.try_const_eval(lit) else {
            return; // sabit değil: E1015 ad çözümlemede
        };
        if int_fits(value, signed, width) {
            return;
        }
        let name = match &self.ast.exprs[lit].kind {
            volt_ast::ExprKind::Path(p) => p.segments[0].text.clone(),
            _ => String::new(),
        };
        let shown = self.show(scrut_ty);
        self.error(
            ErrorCode::E2010,
            span,
            lstr!(en: "constant '{name}' = {value} does not fit in type {shown} of the matched value"; tr: "'{name}' sabiti = {value}, eşlenen değerin {shown} tipine sığmıyor"),
            lstr!(en: "this arm could never be taken"; tr: "bu kol hiç seçilemez"),
            lstr!(en: "match on a wider value, or remove the arm"; tr: "daha geniş bir değer üzerinde eşleyin ya da kolu kaldırın"),
        );
    }

    /// W2014: bütün literalleri önceki kollarda geçen sayısal kol (kural
    /// `volt_ast::match_cover`, sv-emit aynı kolu `case`'e yazmaz).
    fn warn_unreachable_value_arms(&mut self, arms: &[MatchArm]) {
        let ev = &mut *self.ev;
        let unreachable = volt_ast::match_cover::unreachable_value_arms(self.ast, arms, &mut |e| {
            let before = ev.diagnostics.len();
            let value = ev.const_eval(e);
            ev.diagnostics.truncate(before);
            match value {
                crate::ConstValue::Int(n) => Some(n),
                _ => None,
            }
        });
        for (arm, _) in arms.iter().zip(unreachable).filter(|(_, u)| *u) {
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
    fn check_enum_match(&mut self, span: Span, arms: &[MatchArm], e: EnumId, is_expr: bool) {
        let variants = self.enum_variants(e);
        let enum_name = self.enum_name(e);
        let mut covered: Vec<DefId> = Vec::new();
        let mut has_wildcard = false;
        for arm in arms {
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
            .push(enum_not_exhaustive(span, &enum_name, &missing, is_expr));
    }

    /// Enum sınananda bir desenin kapsadığı varyantlar; yanlış desenler
    /// E2003 alır. Aynı enum tipinde `const` adı (ADR-0085) değerinin
    /// varyantını kapsar; çıplak varyant adı (`Idle`) ad çözümlemede E1001
    /// + `State::Idle` önerisi alır.
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
            PatternKind::Wildcard => cover.wildcard = true,
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
            PatternKind::Literal(lit) => {
                let t = self.synth(*lit);
                let same_enum = matches!(*self.types.ty(t), Ty::Enum(pe) if pe == e);
                if let Some(variant) = self.const_variant_of(*lit, t, e) {
                    cover.variants.push(variant);
                } else if self.types.is_error(t) || same_enum {
                    // E1001/E1015 zaten var ya da değer sabit değil
                    // (E1015); kaskad yok.
                    cover.wildcard = true;
                } else {
                    let found = self.show(t);
                    self.err_pattern_type(span, &found, enum_name);
                    cover.wildcard = true;
                }
            }
            PatternKind::Tuple(_) => {
                let found = lstr!(en: "tuple"; tr: "tuple");
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

    /// `const` adı deseni sınananla aynı enum tipindeyse değerinin
    /// varyantı (ADR-0085). Değerlendirme sessizdir: sabit değilse E1015
    /// ad çözümlemede verildi.
    fn const_variant_of(
        &mut self,
        lit: volt_ast::Idx<volt_ast::Expr>,
        t: TypeId,
        e: EnumId,
    ) -> Option<DefId> {
        if !matches!(*self.types.ty(t), Ty::Enum(pe) if pe == e) {
            return None;
        }
        let before = self.ev.diagnostics.len();
        let value = self.ev.const_eval(lit);
        self.ev.diagnostics.truncate(before);
        match value {
            crate::ConstValue::EnumVariant { def, .. } => Some(def),
            _ => None,
        }
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
pub(crate) fn enum_not_exhaustive(
    span: Span,
    enum_name: &str,
    missing: &[String],
    is_expr: bool,
) -> Diagnostic {
    let list = missing.join(", ");
    let first = &missing[0];
    // İfadede her kol bir değer verir: öneri `=> <değer>` (ADR-0083 Karar 3).
    let help = if is_expr {
        lstr!(en: "add an arm for each missing variant ({first} => <value>) or a final '_ => <value>' arm"; tr: "her eksik varyant için kol ({first} => <değer>) ya da sona '_ => <değer>' kolu ekleyin")
    } else {
        lstr!(en: "add an arm for each missing variant ({first} => {{ }}) or a final '_ => {{ }}' arm"; tr: "her eksik varyant için kol ({first} => {{ }}) ya da sona '_ => {{ }}' kolu ekleyin")
    };
    Diagnostic::error(
        ErrorCode::E0014,
        lstr!(en: "'match' on enum '{enum_name}' does not cover every variant: missing {list}"; tr: "'{enum_name}' enum'u üzerindeki 'match' her varyantı kapsamıyor: eksik {list}"),
        LabeledSpan::primary(
            span,
            lstr!(en: "not every variant is covered"; tr: "her varyant kapsanmıyor"),
        ),
        help,
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "an enum match that names every variant needs no '_' arm; its last arm also takes the codes no variant uses (ADR-0074)"; tr: "her varyantı adlandıran enum match'i '_' kolu gerektirmez; son kolu hiçbir varyantın kullanmadığı kodları da alır (ADR-0074)"),
    )
}

/// Parser'ın sayısal `match` E0014'ü (ADR-0032) — ertelenen yol için
/// birebir aynı tanı (golden: sayısal match'in tanısı değişmez).
fn numeric_missing_wildcard(span: Span, is_expr: bool) -> Diagnostic {
    if is_expr {
        return Diagnostic::error(
            ErrorCode::E0014,
            lstr!(en: "'match' expression has no '_' arm"; tr: "'match' ifadesinde '_' kolu yok"),
            LabeledSpan::primary(
                span,
                lstr!(en: "every value must be covered"; tr: "her değer kapsanmalı"),
            ),
            lstr!(en: "add a final '_ => <value>' arm"; tr: "sona '_ => <değer>' kolu ekleyin"),
        )
        .with_note(
            NoteKind::Note,
            lstr!(en: "a match on a number covers every value only with a '_' arm, even if every value is written out (ADR-0083); an enum match is checked variant by variant instead (ADR-0074)"; tr: "sayı üzerindeki match her değeri yalnız '_' koluyla kapsar, bütün değerler yazılmış olsa da (ADR-0083); enum match'i bunun yerine varyant varyant denetlenir (ADR-0074)"),
        );
    }
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

/// `value`, `signed`/`width` tamsayı tipinin aralığında mı?
fn int_fits(value: i128, signed: bool, width: u16) -> bool {
    let w = u32::from(width);
    if signed {
        if w >= 128 {
            return true;
        }
        let half = 1i128 << (w - 1);
        (-half..half).contains(&value)
    } else if value < 0 {
        false
    } else if w >= 127 {
        true
    } else {
        value < (1i128 << w)
    }
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

    // ═══ ADR-0085: çıplak ad desen DEĞERİdir ═══

    fn value_match(head: &str, arms: &str) -> String {
        format!("{head}module M {{\n    in  x : u8\n    in  s : S\n    out y : u8\n    out z : u8\n    y = match x {{ {arms} }}\n    z = match s {{ S::A => 1, _ => 2 }}\n}}\n")
    }

    const CONSTS: &str = "enum S { A, B }\nconst LIMIT : u8 = 10\nconst BIG : u16 = 300\nconst SMALL : u16 = 7\nconst NEG : i8 = -1\nconst START : S = S::B\n";

    #[test]
    fn const_pattern_is_clean_and_wider_declared_const_fits_by_value() {
        let c = codes(&value_match(CONSTS, "LIMIT => 1, SMALL => 3, _ => 2"));
        assert!(c.is_empty(), "{c:?}");
    }

    #[test]
    fn const_pattern_value_must_fit_the_scrutinee() {
        assert_eq!(codes(&value_match(CONSTS, "BIG => 1, _ => 2")), ["E2010"]);
        assert_eq!(codes(&value_match(CONSTS, "NEG => 1, _ => 2")), ["E2010"]);
        assert_eq!(codes(&value_match(CONSTS, "300 => 1, _ => 2")), ["E2010"]);
    }

    #[test]
    fn enum_const_on_number_is_e2003() {
        assert_eq!(codes(&value_match(CONSTS, "START => 1, _ => 2")), ["E2003"]);
    }

    #[test]
    fn const_repeating_a_literal_is_unreachable_w2014() {
        let d = diagnostics(&value_match(CONSTS, "10 => 1, LIMIT => 2, _ => 3"));
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code.as_str(), "W2014");
    }

    #[test]
    fn enum_const_covers_its_variant() {
        let src = "enum S { A, B }\nconst START : S = S::B\nmodule M {\n    in  s : S\n    out y : u8\n    y = match s { START => 1, S::A => 2 }\n}\n";
        assert!(codes(src).is_empty(), "{:?}", codes(src));
        let missing = "enum S { A, B, C }\nconst START : S = S::B\nmodule M {\n    in  s : S\n    out y : u8\n    y = match s { START => 1, S::A => 2 }\n}\n";
        let d = diagnostics(missing);
        assert!(
            d.iter().any(|d| d.code.as_str() == "E0014"
                && d.message.contains("S::C")
                && !d.message.contains("S::B")),
            "{d:?}"
        );
    }

    #[test]
    fn int_fits_bounds() {
        use super::int_fits;
        assert!(int_fits(255, false, 8) && !int_fits(256, false, 8) && !int_fits(-1, false, 8));
        assert!(int_fits(-128, true, 8) && !int_fits(-129, true, 8) && !int_fits(128, true, 8));
        assert!(int_fits(i128::MAX, false, 128) && int_fits(i128::MIN, true, 128));
    }
}
