//! Kullanım takibi (name-resolution.md §9): hiç okunmayan tanımlar için
//! W1001/W1004/W1005/W3004.

use volt_ast::PortDir;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use super::def::{DefData, DefId, DefKind};
use super::scope::ScopeKind;
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn report_unused(&mut self) {
        let mut warnings = Vec::new();
        for (i, data) in self.defs.iter().enumerate() {
            let def = DefId(i as u32);
            if self.is_usage_exempt(data) || self.reads.contains(&def) {
                continue;
            }
            let Some((code, msg)) = self.unused_message(def, data.kind) else {
                continue;
            };
            // Açılmış `for` gövdesinde üretilmiş ad (`x_0`) değil,
            // kullanıcının yazdığı ad: kopyaların mesajı özdeş olur ve
            // katlanır (ADR-0068), `_` önerisi kaynağa yazılabilir.
            let name = self.ast.generate.source_name(data.span, &data.name);
            // Bundle alanı (`hs.data`, ADR-0075): alan adı tek başına
            // yeniden adlandırılamaz; bundle portunun `_` öneki bütün
            // alanlarını susturur (düzleştirilmiş adlar `_hs_...` olur).
            let help = match name.split_once('.') {
                Some((port, _)) => lstr!(
                    en: "add a '_' prefix to the bundle port to silence all its fields: _{port}";
                    tr: "bundle portuna '_' öneki ekleyerek bütün alanlarını susturabilirsiniz: _{port}"
                ),
                None => lstr!(en: "add a '_' prefix to silence: _{}", name;
                              tr: "'_' öneki ekleyerek susturabilirsiniz: _{}", name),
            };
            warnings.push(Diagnostic::warning(
                code,
                format!("{}: '{}'", msg, name),
                LabeledSpan::primary(
                    data.span,
                    lstr!(en: "defined here, never read"; tr: "burada tanımlı, hiç okunmuyor"),
                ),
                help,
            ));
        }
        self.diagnostics.extend(warnings);
    }

    /// Kullanım raporundan muaf tanımlar.
    fn is_usage_exempt(&self, data: &DefData) -> bool {
        // '_' önekli isimler muaf (UX Anayasası).
        data.name.starts_with('_')
            // Public öğeler muaf (dışarıdan kullanılabilir).
            || data.is_public
            // Extern portları dış SV modülüne aittir (ADR-0047).
            || matches!(
                self.scope(data.scope).kind,
                ScopeKind::Extern(_)
            )
    }

    /// Okunmayan tanımın uyarı kodu ve başlığı; izlenmeyen türlerde `None`.
    fn unused_message(&self, def: DefId, kind: DefKind) -> Option<(ErrorCode, String)> {
        Some(match kind {
            DefKind::Port { dir: PortDir::In } => (
                ErrorCode::W1001,
                lstr!(en: "unused input port"; tr: "kullanılmayan giriş portu"),
            ),
            DefKind::Register => {
                if self.writes.contains(&def) {
                    (
                        ErrorCode::W1004,
                        lstr!(en: "register written but never read";
                              tr: "yazılıp hiç okunmayan register"),
                    )
                } else {
                    (
                        ErrorCode::W1004,
                        lstr!(en: "unused register"; tr: "kullanılmayan register"),
                    )
                }
            }
            DefKind::Wire | DefKind::LocalBinding => (
                ErrorCode::W1001,
                lstr!(en: "unused binding"; tr: "kullanılmayan bağlama"),
            ),
            DefKind::Domain => (
                ErrorCode::W3004,
                lstr!(en: "unused domain definition"; tr: "kullanılmayan domain tanımı"),
            ),
            DefKind::Import => (
                ErrorCode::W1005,
                lstr!(en: "unused import"; tr: "kullanılmayan import"),
            ),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::codes;

    #[test]
    fn unread_definitions_warn_by_kind() {
        assert_eq!(
            codes("module M { in a : u8 in b : u8 out y : u8 y = a }"),
            ["W1001"]
        );
        let c = codes(
            "module M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    \
             reg r : u8 = 0\n    on clk { r <= a }\n    y = a\n}\n",
        );
        assert_eq!(c, ["W1004"]);
        let c = codes("use paket::Yok;\nmodule M { in a : u8 out y : u8 y = a }");
        assert_eq!(c, ["W1005"]);
    }

    #[test]
    fn underscore_public_and_extern_definitions_are_exempt() {
        assert!(codes("module M { in _a : u8 in b : u8 out y : u8 y = b }").is_empty());
        let c = codes("extern module X {\n    in clk : clock\n    in d : u8\n    out q : u8\n}\n");
        assert!(c.is_empty(), "{c:?}");
    }
}
