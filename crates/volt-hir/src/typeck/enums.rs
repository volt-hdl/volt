//! Enum bildirim denetimi ve kodlama tablosu (ADR-0074 Karar 2).
//!
//! Kural `volt_ast::enum_layout`'tadır (sv-emit ve otomatik kontratlarla
//! ortak); burada ihlaller tanıya çevrilir: E2030 (geçersiz kodlama),
//! E2010 (açık değer negatif ya da taban tipine sığmıyor), E2021 (açık
//! değer sabit değil), E1003 (yinelenen varyant adı). Geçerli tablonun
//! genişliği `TypeArena`'ya yazılır — `signal_width` (W3003) ve tip
//! dönüşümü kuralları (`enum as uN`) oradan okur.
//!
//! Payload'lı ve generic enum'larda kodlama denetlenmez: bildirimleri
//! yasaldır, sinyal tipi olarak kullanımları E0003'tür (Karar 1).

use volt_ast::enum_layout::{self, LayoutIssue, Repr};
use volt_ast::{EnumDecl, ItemKind};
use volt_diagnostics::{lstr, ErrorCode};

use super::TypeChecker;
use crate::resolve::{DefId, DefKind};
use crate::ty::EnumId;

impl TypeChecker<'_, '_> {
    /// Birimdeki her enum bildirimini denetler, geçerli kodlamaları kaydeder.
    pub(super) fn check_enum_decls(&mut self) {
        let ast = self.ast;
        for &item_idx in &ast.items {
            let ItemKind::Enum(decl) = &ast.items_arena[item_idx].kind else {
                continue;
            };
            self.check_duplicate_variants(decl);
            if !decl.generics.is_empty() || enum_layout::payload_variant(decl).is_some() {
                continue;
            }
            let Some(&def) = self.res.decl_spans.get(&decl.name.span) else {
                continue;
            };
            let repr = enum_layout::repr_of(ast, decl, &mut |e| self.try_const_eval(e));
            match enum_layout::layout(decl, repr, &mut |e| self.try_const_eval(e)) {
                Ok(layout) => {
                    let width = u16::try_from(layout.width).unwrap_or(u16::MAX);
                    self.types.set_enum_width(EnumId(def.0), width);
                }
                Err(issues) => {
                    for issue in issues {
                        self.report_layout_issue(decl, repr, issue);
                    }
                }
            }
        }
    }

    /// E1003 — aynı adlı iki varyant (`enum E { A, A }`).
    fn check_duplicate_variants(&mut self, decl: &EnumDecl) {
        for (j, v) in decl.variants.iter().enumerate() {
            if decl.variants[..j]
                .iter()
                .any(|u| u.name.text == v.name.text)
            {
                let (enum_name, name) = (&decl.name.text, &v.name.text);
                self.error(
                    ErrorCode::E1003,
                    v.name.span,
                    lstr!(en: "variant '{name}' is declared twice in enum '{enum_name}'"; tr: "'{name}' varyantı '{enum_name}' enum'unda iki kez bildirilmiş"),
                    lstr!(en: "duplicate variant"; tr: "yinelenen varyant"),
                    lstr!(en: "rename or remove one of the two variants"; tr: "iki varyanttan birini yeniden adlandırın ya da kaldırın"),
                );
            }
        }
    }

    fn report_layout_issue(&mut self, decl: &EnumDecl, repr: Repr, issue: LayoutIssue) {
        let ast = self.ast;
        let enum_name = decl.name.text.clone();
        let variant = |i: usize| &decl.variants[i];
        let repr_span = decl.repr.map_or(decl.name.span, |t| ast.types[t].span);
        match issue {
            LayoutIssue::Empty => self.error(
                ErrorCode::E2030,
                decl.name.span,
                lstr!(en: "enum '{enum_name}' has no variants"; tr: "'{enum_name}' enum'unun varyantı yok"),
                lstr!(en: "empty enum"; tr: "boş enum"),
                lstr!(en: "add at least one variant: enum {enum_name} {{ Idle }}"; tr: "en az bir varyant ekleyin: enum {enum_name} {{ Idle }}"),
            ),
            LayoutIssue::Mixed { first_implicit } => {
                let v = variant(first_implicit);
                let name = v.name.text.clone();
                self.error(
                    ErrorCode::E2030,
                    v.span,
                    lstr!(en: "enum '{enum_name}' mixes explicit and implicit values: variant '{name}' has no value"; tr: "'{enum_name}' enum'u açık ve örtük değerleri karıştırıyor: '{name}' varyantının değeri yok"),
                    lstr!(en: "value missing"; tr: "değer eksik"),
                    lstr!(en: "give every variant a value ({name} = ...) or remove all values"; tr: "her varyanta değer verin ({name} = ...) ya da bütün değerleri kaldırın"),
                );
            }
            LayoutIssue::NonConst(i) => {
                let v = variant(i);
                let name = v.name.text.clone();
                self.error(
                    ErrorCode::E2021,
                    v.span,
                    lstr!(en: "the value of variant '{enum_name}::{name}' is not a compile-time constant"; tr: "'{enum_name}::{name}' varyantının değeri derleme zamanı sabiti değil"),
                    lstr!(en: "not a constant"; tr: "sabit değil"),
                    lstr!(en: "write an integer constant (e.g. {name} = 3)"; tr: "tamsayı sabiti yazın (ör. {name} = 3)"),
                );
            }
            LayoutIssue::Negative(i, value) => {
                let v = variant(i);
                let name = v.name.text.clone();
                self.error(
                    ErrorCode::E2010,
                    v.span,
                    lstr!(en: "the value {value} of variant '{enum_name}::{name}' is negative"; tr: "'{enum_name}::{name}' varyantının değeri {value} negatif"),
                    lstr!(en: "enum codes are unsigned"; tr: "enum kodları işaretsizdir"),
                    lstr!(en: "use a value >= 0"; tr: ">= 0 bir değer kullanın"),
                );
            }
            LayoutIssue::Duplicate(first, second, value) => {
                let (a, b) = (variant(first).name.text.clone(), variant(second).name.text.clone());
                self.error(
                    ErrorCode::E2030,
                    variant(second).span,
                    lstr!(en: "variants '{a}' and '{b}' of enum '{enum_name}' have the same value {value}"; tr: "'{enum_name}' enum'unun '{a}' ve '{b}' varyantları aynı değeri ({value}) taşıyor"),
                    lstr!(en: "duplicate code"; tr: "yinelenen kod"),
                    lstr!(en: "every variant needs its own code"; tr: "her varyantın kendi kodu olmalı"),
                );
            }
            LayoutIssue::InvalidRepr => {
                let shown = decl.repr.map_or_else(String::new, |t| {
                    let ty = self.resolve_type_ref(t);
                    self.show(ty)
                });
                self.error(
                    ErrorCode::E2030,
                    repr_span,
                    lstr!(en: "the base type of enum '{enum_name}' must be an unsigned uN, uint<N> or bits<N>, found '{shown}'"; tr: "'{enum_name}' enum'unun taban tipi işaretsiz uN, uint<N> ya da bits<N> olmalı, '{shown}' bulundu"),
                    lstr!(en: "invalid base type"; tr: "geçersiz taban tipi"),
                    lstr!(en: "use an unsigned base type: enum {enum_name} : u4 {{ ... }}"; tr: "işaretsiz bir taban tipi kullanın: enum {enum_name} : u4 {{ ... }}"),
                );
            }
            LayoutIssue::ReprTooNarrow { needed, width } => {
                let n = decl.variants.len();
                self.error(
                    ErrorCode::E2030,
                    repr_span,
                    lstr!(en: "the {width}-bit base type of enum '{enum_name}' is too narrow for its {n} variants (needs {needed} bits)"; tr: "'{enum_name}' enum'unun {width} bitlik taban tipi {n} varyant için dar ({needed} bit gerekli)"),
                    lstr!(en: "base type too narrow"; tr: "taban tipi dar"),
                    lstr!(en: "use a base type of at least {needed} bits, or remove it"; tr: "en az {needed} bitlik bir taban tipi kullanın ya da kaldırın"),
                );
            }
            LayoutIssue::ValueTooWide(i, value, width) => {
                let v = variant(i);
                let name = v.name.text.clone();
                let max = if width >= 128 {
                    u128::MAX
                } else {
                    (1u128 << width) - 1
                };
                debug_assert!(matches!(repr, Repr::Width(_)));
                self.error(
                    ErrorCode::E2010,
                    v.span,
                    lstr!(en: "the value {value} of variant '{enum_name}::{name}' does not fit in the {width}-bit base type (maximum {max})"; tr: "'{enum_name}::{name}' varyantının değeri {value}, {width} bitlik taban tipine sığmıyor (maksimum {max})"),
                    lstr!(en: "value is outside the base type's range"; tr: "değer taban tipinin aralığı dışında"),
                    lstr!(en: "use a wider base type or a smaller value"; tr: "daha geniş bir taban tipi ya da daha küçük bir değer kullanın"),
                );
            }
        }
    }

    /// Enum'un varyantları, bildirim sırasıyla: (ad, tanım).
    pub(super) fn enum_variants(&self, e: EnumId) -> Vec<(String, DefId)> {
        let parent = DefId(e.0);
        let mut out: Vec<(usize, String, DefId)> = self
            .res
            .defs
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == DefKind::EnumVariant { parent })
            .map(|(i, d)| {
                let def = DefId(i as u32);
                let order = self.res.variant_info.get(&def).map_or(i, |(o, _)| *o);
                (order, d.name.clone(), def)
            })
            .collect();
        out.sort_by_key(|(o, _, _)| *o);
        out.into_iter().map(|(_, n, d)| (n, d)).collect()
    }

    /// Enum tipinin adı (tanılar için).
    pub(super) fn enum_name(&self, e: EnumId) -> String {
        self.res
            .defs
            .get(e.0 as usize)
            .map_or_else(|| "enum".to_string(), |d| d.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, diagnostics};

    fn with_enum(decl: &str) -> String {
        format!("{decl}\n\nmodule M {{\n    in  a : u8\n    out y : u8\n\n    y = a\n}}\n")
    }

    #[test]
    fn default_and_explicit_encodings_are_silent() {
        assert!(codes(&with_enum("enum S { A, B, C }")).is_empty());
        assert!(codes(&with_enum("enum Op : u4 { Add = 0, Sub = 1, Jal = 8 }")).is_empty());
        assert!(codes(&with_enum("enum B : bits<3> { X = 1, Y = 4 }")).is_empty());
    }

    #[test]
    fn mixed_duplicate_and_empty_encodings_are_e2030() {
        for decl in [
            "enum Op : u4 { Add = 0, Sub, Jal = 8 }",
            "enum D { A = 1, B = 1 }",
            "enum E { }",
            "enum N : i4 { A = 0, B = 1 }",
            "enum T : u1 { A, B, C }",
        ] {
            let c = codes(&with_enum(decl));
            assert_eq!(c, ["E2030"], "{decl}: {c:?}");
        }
    }

    #[test]
    fn value_outside_base_type_is_e2010() {
        let c = codes(&with_enum("enum Op : u3 { A = 0, B = 8 }"));
        assert_eq!(c, ["E2010"], "{c:?}");
    }

    #[test]
    fn duplicate_variant_name_is_e1003() {
        let c = codes(&with_enum("enum S { A, A }"));
        assert!(c.contains(&"E1003"), "{c:?}");
    }

    #[test]
    fn mixed_message_names_the_variant_without_value() {
        let d = diagnostics(&with_enum("enum Op : u4 { Add = 0, Sub, Jal = 8 }"));
        assert!(d[0].message.contains("'Sub'"), "{}", d[0].message);
    }

    #[test]
    fn payload_enum_declaration_is_not_checked() {
        assert!(codes(&with_enum("enum Cmd { Idle, Load(u8) }")).is_empty());
    }
}

#[cfg(test)]
mod rule_tests {
    use super::super::testutil::{codes, def_ty, diagnostics};

    fn module(body: &str) -> String {
        format!(
            "enum State {{ Idle, Run, Done }}\nenum Mode {{ A, B }}\nmodule M {{\n    in  a : State\n    in  b : State\n    in  m : Mode\n    in  raw : u2\n    out y : bool\n\n{body}\n    y = a == b\n}}\n"
        )
    }

    #[test]
    fn same_enum_equality_is_bool() {
        let src = module("    let _e = a == State::Run\n    let _n = a != b");
        assert!(codes(&src).is_empty(), "{:?}", codes(&src));
        assert_eq!(def_ty(&src, "_e"), "bool");
    }

    #[test]
    fn ordering_arithmetic_and_bit_ops_are_e2003() {
        for body in [
            "    let _x = a < b",
            "    let _x = a >= State::Run",
            "    let _x = a + 1",
            "    let _x = a & b",
            "    let _x = ~a",
            "    let _x = a[0]",
            "    let _x = a << 1",
        ] {
            assert_eq!(codes(&module(body)), ["E2003"], "{body}");
        }
    }

    #[test]
    fn mismatch_messages_name_the_enums() {
        let d = diagnostics(&module("    let _x = a == m"));
        assert!(
            d[0].message.contains("'State' and 'Mode'"),
            "{}",
            d[0].message
        );
        let d = diagnostics(&module("    let _x = a == 1"));
        assert!(d[0].message.contains("'State'"), "{}", d[0].message);
    }

    #[test]
    fn enum_to_wide_enough_uint_or_bits_is_legal() {
        let src = module("    let _u = a as u2\n    let _w = a as u8\n    let _b = a as bits<2>");
        assert!(codes(&src).is_empty(), "{:?}", codes(&src));
        assert_eq!(def_ty(&src, "_w"), "u8");
    }

    #[test]
    fn narrowing_signed_bool_and_uint_to_enum_casts_are_e2009() {
        for body in [
            "    let _x = a as u1",
            "    let _x = a as i8",
            "    let _x = a as bool",
            "    let _x = a as Mode",
            "    let _x = raw as State",
            "    let _x = 1 as State",
        ] {
            assert_eq!(codes(&module(body)), ["E2009"], "{body}");
        }
    }

    #[test]
    fn uint_to_enum_help_shows_the_decoding_match() {
        let d = diagnostics(&module("    let _x = raw as State"));
        assert!(
            d[0].help.as_deref().unwrap_or("").contains("match raw"),
            "{:?}",
            d[0].help
        );
    }

    #[test]
    fn enum_register_needs_an_enum_initializer() {
        let src = "enum State { Idle, Run }\nmodule M {\n    in  clk : clock\n    out y : bool\n    reg s : State = 0\n    on clk { s <= State::Run }\n    y = s == State::Run\n}\n";
        assert_eq!(codes(src), ["E2003"]);
    }

    #[test]
    fn if_condition_must_be_bool_not_enum() {
        let src = module("    let _x : bool = if a { true } else { false }");
        assert!(codes(&src).contains(&"E2003"), "{:?}", codes(&src));
    }
}
