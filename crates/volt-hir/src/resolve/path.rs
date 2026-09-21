//! Yol çözümleme (name-resolution.md §3.1 yalın ad, §3.2 nitelikli yol)
//! ve çözülemeyen ad tanıları (§8: E1001 + öneri, E1002).

use volt_ast::{Name, Path};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::def::{DefId, DefKind};
use super::scope::ScopeId;
use super::suggest::{closest_match, did_you_mean, with_rename};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn resolve_path(&mut self, path: &Path, scope: ScopeId) -> DefId {
        let Some(first) = path.segments.first() else {
            return self.error_def;
        };
        if path.segments.len() == 1 {
            return self.resolve_simple(&first.clone(), scope, true);
        }

        // Çok segment: State::Idle vb.
        let mut current = self.resolve_simple(&first.clone(), scope, true);
        for seg in &path.segments[1..] {
            let kind = self.def(current).kind;
            current = match kind {
                DefKind::Enum => self.lookup_variant(current, seg),
                DefKind::Error | DefKind::Import => return self.error_def,
                _ => {
                    let name = self.def(current).name.clone();
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E1005,
                        lstr!(en: "'{}' is not a namespace", name;
                              tr: "'{}' bir ad alanı değil", name),
                        LabeledSpan::primary(
                            path.span,
                            lstr!(en: "'::' cannot be used here";
                                  tr: "'::' burada kullanılamaz"),
                        ),
                        lstr!(en: "'::' is only valid in enum and package paths; \
                                   use '.' for signal access";
                              tr: "'::' yalnız enum ve paket yollarında geçerlidir; \
                                   sinyal erişimi için '.' kullanın"),
                    ));
                    return self.error_def;
                }
            };
        }
        current
    }

    fn lookup_variant(&mut self, enum_def: DefId, seg: &Name) -> DefId {
        let variants = self
            .enum_variants
            .get(&enum_def)
            .cloned()
            .unwrap_or_default();
        if let Some((_, def)) = variants.iter().find(|(n, _)| *n == seg.text) {
            self.reads.insert(*def);
            return *def;
        }
        let enum_name = self.def(enum_def).name.clone();
        let names: Vec<String> = variants.iter().map(|(n, _)| n.clone()).collect();
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E1007,
            lstr!(en: "enum '{}' has no variant '{}'", enum_name, seg.text;
                  tr: "'{}' enum'ında '{}' varyantı yok", enum_name, seg.text),
            LabeledSpan::primary(
                seg.span,
                lstr!(en: "unknown variant"; tr: "bilinmeyen varyant"),
            ),
            did_you_mean(
                closest_match(&seg.text, &names).as_ref(),
                lstr!(en: "available variants: {}", names.join(", ");
                      tr: "mevcut varyantlar: {}", names.join(", ")),
            ),
        ));
        self.error_def
    }

    pub(super) fn resolve_simple(&mut self, name: &Name, scope: ScopeId, is_read: bool) -> DefId {
        let Some(def) = self.lookup_visible(&name.text, scope) else {
            self.error_unresolved(name, scope);
            return self.error_def;
        };
        if is_read {
            self.reads.insert(def);
        } else {
            self.writes.insert(def);
        }
        self.use_spans.insert(name.span, def);
        def
    }

    /// E1002 (ileride bildirilmiş) veya E1001 (hiç yok) üretir.
    fn error_unresolved(&mut self, name: &Name, scope: ScopeId) {
        let later_decl = self
            .pending
            .iter()
            .rev()
            .find_map(|later| later.get(&name.text).copied());
        if let Some(decl_span) = later_decl {
            self.err_used_before_decl(name, decl_span);
            return;
        }

        let candidates = self.visible_names(scope);
        let suggestion = closest_match(&name.text, &candidates);
        let diag = Diagnostic::error(
            ErrorCode::E1001,
            lstr!(en: "undefined name: '{}'", name.text;
                  tr: "tanımsız isim: '{}'", name.text),
            LabeledSpan::primary(
                name.span,
                lstr!(en: "this name could not be resolved"; tr: "bu isim çözülemedi"),
            ),
            did_you_mean(
                suggestion.as_ref(),
                lstr!(en: "this name is not defined in any scope";
                      tr: "bu isim hiçbir kapsamda tanımlı değil"),
            ),
        );
        self.diagnostics
            .push(with_rename(diag, name.span, suggestion));
    }

    /// E1002 — ad, içinde bulunulan sıralı gövdede daha AŞAĞIDA bildirilmiş.
    fn err_used_before_decl(&mut self, name: &Name, decl_span: Span) {
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E1002,
                lstr!(en: "'{}' is not yet defined at this point", name.text;
                      tr: "'{}' bu noktada henüz tanımlı değil", name.text),
                LabeledSpan::primary(name.span, lstr!(en: "used here"; tr: "burada kullanılıyor")),
                lstr!(en: "move the declaration of '{}' above this use", name.text;
                      tr: "'{}' bildirimini bu kullanımdan yukarı taşıyın", name.text),
            )
            .with_secondary(
                decl_span,
                lstr!(en: "but it is defined here"; tr: "ama burada tanımlanıyor"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "declarations inside a module must come before their uses";
                      tr: "modül içi bildirimler kullanımdan önce gelmeli"),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, resolved};

    #[test]
    fn unknown_name_suggests_the_closest_visible_name() {
        let r = resolved("module M { in data : u8 out y : u8 y = dta }");
        let diag = r
            .diagnostics
            .iter()
            .find(|d| d.code.as_str() == "E1001")
            .expect("E1001");
        assert_eq!(diag.suggestions[0].replacement, "data");
    }

    #[test]
    fn use_before_declaration_is_e1002_not_e1001() {
        assert_eq!(
            codes("module M { in a : u8 out y : u8 y = t let t = a }"),
            ["E1002", "W1001"]
        );
    }

    #[test]
    fn unknown_enum_variant_is_e1007_and_non_namespace_is_e1005() {
        let c = codes(
            "enum Durum : bits<2> { Bekle = 0, Calis = 1 }
             module M { in x : u8 out y : u8 y = match x { Durum::Kos => 0, _ => 1 } }",
        );
        assert!(c.contains(&"E1007"), "{c:?}");
        let c = codes("module M { in x : u8 out y : u8 y = x::alan }");
        assert!(c.contains(&"E1005"), "{c:?}");
    }
}
