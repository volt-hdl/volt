//! Desenler (name-resolution.md §5.1, ADR-0085): `match` kolu desenindeki
//! adlar.
//!
//! Volt'ta bağlama deseni yoktur: desendeki çıplak ad bir DEĞERDİR ve
//! sınanan onunla karşılaştırılır (`LIMIT => ...` ≡ `x == LIMIT`). Ad bir
//! derleme zamanı sabitine (`const`, generic parametre) çözülmelidir:
//!
//! - sinyal/örnek/yerel/döngü değişkeni → E1015 (desen sabit ister;
//!   eşitlik `if x == ad` ile yazılır),
//! - tanımsız ad → E1001; ad bir enum'un varyantıysa öneri `Enum::Ad`
//!   (varyantlar kök kapsama bağlanmaz, ADR-0074).
//!
//! Böylece yazım hatası (`LIMT`) ya da gölgeleyen ad sessizce "her şeyi
//! yakalayan" kola dönüşemez.

use volt_ast::{ExprKind, Idx, Name, Pattern, PatternArgs, PatternKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::def::{DefId, DefKind};
use super::scope::ScopeId;
use super::suggest::{closest_match, did_you_mean, with_rename};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn resolve_pattern(&mut self, pat_idx: Idx<Pattern>, scope: ScopeId) {
        let pat = &self.ast.patterns[pat_idx];
        match &pat.kind {
            PatternKind::Wildcard | PatternKind::Error => {}
            PatternKind::Literal(e) => self.resolve_pattern_value(*e, scope),
            PatternKind::Path { path, args } => {
                let def = self.resolve_path(&path.clone(), scope);
                self.pattern_resolutions.insert(pat_idx, def);
                match args {
                    Some(PatternArgs::Tuple(pats)) => {
                        for &p in pats {
                            self.resolve_pattern(p, scope);
                        }
                    }
                    Some(PatternArgs::Struct(fields)) => {
                        for f in fields {
                            match f.pattern {
                                Some(p) => self.resolve_pattern(p, scope),
                                // `Foo { x }` kısayolu x'i bağlar.
                                None => {
                                    self.declare_checked(
                                        &f.name.clone(),
                                        DefKind::PatternBinding,
                                        scope,
                                        false,
                                    );
                                }
                            }
                        }
                    }
                    None => {}
                }
            }
            PatternKind::Tuple(pats) | PatternKind::Or(pats) => {
                for &p in pats.clone().iter() {
                    self.resolve_pattern(p, scope);
                }
            }
        }
    }

    /// Değer deseni: literal ya da çıplak ad. Çıplak ad sabite
    /// çözülmelidir (modül başı).
    fn resolve_pattern_value(&mut self, e: Idx<volt_ast::Expr>, scope: ScopeId) {
        let name = match &self.ast.exprs[e].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => p.segments[0].clone(),
            _ => return self.resolve_expr(e, scope),
        };
        let Some(def) = self.lookup_visible(&name.text, scope) else {
            self.error_unresolved_pattern(&name, scope);
            // Kol gövdesindeki aynı ad ikinci E1001 üretmesin (kaskad yok):
            // ad kol kapsamında hata tanımına bağlanır.
            let err = self.declare(&name, DefKind::Error, scope, false);
            self.resolutions.insert(e, err);
            return;
        };
        self.reads.insert(def);
        self.use_spans.insert(name.span, def);
        self.resolutions.insert(e, def);
        let kind = self.def(def).kind;
        if !is_constant_kind(kind) {
            self.error_pattern_not_constant(&name, def, kind);
        }
    }

    /// E1015 — desendeki ad bir sinyali (ya da başka sabit olmayanı)
    /// adlandırıyor.
    fn error_pattern_not_constant(&mut self, name: &Name, def: DefId, kind: DefKind) {
        let what = describe_kind(kind);
        let decl = self.def(def).span;
        let text = &name.text;
        let mut diag = Diagnostic::error(
            ErrorCode::E1015,
            lstr!(en: "'{text}' in a pattern must be a constant, but it is {what}";
                  tr: "desendeki '{text}' bir sabit olmalı, ama {what}"),
            LabeledSpan::primary(
                name.span,
                lstr!(en: "a pattern compares with a constant value";
                      tr: "desen sabit bir değerle karşılaştırır"),
            ),
            lstr!(en: "compare explicitly: 'if <value> == {text} {{ ... }}'; or match a 'const' / a literal";
                  tr: "açıkça karşılaştırın: 'if <değer> == {text} {{ ... }}'; ya da bir 'const' / literal ile eşleyin"),
        )
        .with_note(
            NoteKind::Note,
            lstr!(en: "Volt has no binding patterns: a name in a pattern is a value, never a new variable (ADR-0085)";
                  tr: "Volt'ta bağlama deseni yok: desendeki ad bir değerdir, asla yeni bir değişken değil (ADR-0085)"),
        );
        if decl.file.0 != u32::MAX {
            diag = diag.with_secondary(
                decl,
                lstr!(en: "'{text}' is declared here"; tr: "'{text}' burada bildiriliyor"),
            );
        }
        self.diagnostics.push(diag);
    }

    /// E1001 (ya da E1002) — desendeki ad hiçbir kapsamda yok. Ad bir
    /// enum varyantıysa öneri tam yoldur; değilse en yakın ad.
    fn error_unresolved_pattern(&mut self, name: &Name, scope: ScopeId) {
        if self.report_used_before_decl(name) {
            return;
        }
        let text = &name.text;
        let variants = self.variant_paths_named(text);
        let (help, suggestion) = if let Some(first) = variants.first() {
            (
                lstr!(en: "enum variants are named with their enum: write '{first}'";
                      tr: "enum varyantları enum'larıyla adlandırılır: '{first}' yazın"),
                Some(first.clone()),
            )
        } else {
            let suggestion = closest_match(text, &self.visible_names(scope));
            let help = did_you_mean(
                suggestion.as_ref(),
                lstr!(en: "a name in a pattern must name a 'const'; to match every other value use '_'";
                      tr: "desendeki ad bir 'const' adlandırmalı; diğer her değer için '_' kullanın"),
            );
            (help, suggestion)
        };
        let diag = Diagnostic::error(
            ErrorCode::E1001,
            lstr!(en: "undefined name: '{text}'"; tr: "tanımsız isim: '{text}'"),
            LabeledSpan::primary(
                name.span,
                lstr!(en: "this name could not be resolved"; tr: "bu isim çözülemedi"),
            ),
            help,
        )
        .with_note(
            NoteKind::Note,
            lstr!(en: "Volt has no binding patterns: a name in a pattern is a value, never a new variable (ADR-0085)";
                  tr: "Volt'ta bağlama deseni yok: desendeki ad bir değerdir, asla yeni bir değişken değil (ADR-0085)"),
        );
        self.diagnostics
            .push(with_rename(diag, name.span, suggestion));
    }

    /// `text` adlı varyantı olan enum'lar için `Enum::text` yolları (enum
    /// adına göre sıralı — HashMap sırası tanıya sızmaz).
    fn variant_paths_named(&self, text: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .enum_variants
            .iter()
            .filter(|(_, vs)| vs.iter().any(|(n, _)| n == text))
            .map(|(&e, _)| format!("{}::{text}", self.def(e).name))
            .collect();
        out.sort();
        out
    }
}

/// Desende değer olarak kullanılabilen tanımlar: derleme zamanı sabitleri.
/// Hata/dış ad kaskad üretmez.
fn is_constant_kind(kind: DefKind) -> bool {
    matches!(
        kind,
        DefKind::Const | DefKind::GenericParam | DefKind::Error | DefKind::Import
    )
}

fn describe_kind(kind: DefKind) -> String {
    match kind {
        DefKind::Port { .. } => lstr!(en: "a port"; tr: "bir port"),
        DefKind::Register => lstr!(en: "a register"; tr: "bir register"),
        DefKind::Wire => lstr!(en: "a wire"; tr: "bir wire"),
        DefKind::LocalBinding => lstr!(en: "a 'let' binding"; tr: "bir 'let' bağlaması"),
        DefKind::LoopVar => lstr!(en: "a loop variable"; tr: "bir döngü değişkeni"),
        DefKind::Instance => lstr!(en: "a module instance"; tr: "bir modül örneği"),
        DefKind::Module | DefKind::ExternModule => lstr!(en: "a module"; tr: "bir modül"),
        DefKind::Function => lstr!(en: "a function"; tr: "bir fonksiyon"),
        DefKind::Builtin(_) => lstr!(en: "a builtin function"; tr: "bir yerleşik fonksiyon"),
        DefKind::Struct | DefKind::Enum | DefKind::TypeAlias => {
            lstr!(en: "a type"; tr: "bir tip")
        }
        DefKind::Domain | DefKind::DomainParam => {
            lstr!(en: "a clock domain"; tr: "bir saat alanı")
        }
        _ => lstr!(en: "not a constant"; tr: "sabit değil"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::codes;

    const HEAD: &str = "module M {\n    in clk : clock\n    in x : u8\n    out y : u8\n";

    #[test]
    fn const_name_in_pattern_is_a_value_not_a_binding() {
        let src =
            format!("const LIMIT : u8 = 10\n{HEAD}    y = match x {{ LIMIT => 1, _ => 2 }}\n}}\n");
        let c = codes(&src);
        assert!(!c.contains(&"W1002") && !c.contains(&"E1015"), "{c:?}");
    }

    #[test]
    fn port_name_in_pattern_is_e1015() {
        let src = HEAD.replace("in x : u8", "in x : u8\n    in lim : u8")
            + "    y = match x { lim => 1, _ => 2 }\n}\n";
        let c = codes(&src);
        assert!(c.contains(&"E1015") && !c.contains(&"W1002"), "{c:?}");
    }

    #[test]
    fn undefined_pattern_name_is_one_e1001_without_cascade() {
        let src = format!("{HEAD}    y = match x {{ n => n, _ => 2 }}\n}}\n");
        let c = codes(&src);
        assert_eq!(c.iter().filter(|c| **c == "E1001").count(), 1, "{c:?}");
    }
}
