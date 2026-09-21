//! Yerleşikler (name-resolution.md §7): prelude fonksiyonları ve isim
//! olmayan genişletilmiş tam sayı tip ailesi.

use super::def::{synthetic_span, BuiltinKind, DefKind};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn init_prelude(&mut self) {
        // name-resolution.md §7 — Trit bilinçli olarak YOK (opt-in import).
        const BUILTINS: &[(&str, BuiltinKind)] = &[
            ("sync", BuiltinKind::Sync),
            ("sync3", BuiltinKind::Sync3),
            ("zext", BuiltinKind::Zext),
            ("sext", BuiltinKind::Sext),
            ("trunc", BuiltinKind::Trunc),
            ("concat", BuiltinKind::Concat),
            ("replicate", BuiltinKind::Replicate),
            ("popcount", BuiltinKind::PopCount),
            ("clog2", BuiltinKind::Clog2),
            ("prev", BuiltinKind::Prev),
        ];
        for &(name, kind) in BUILTINS {
            let def = self.add_def(
                DefKind::Builtin(kind),
                name,
                synthetic_span(),
                self.prelude,
                true,
            );
            self.bind(self.prelude, name, def);
        }
    }
}

/// `u9`, `i128` gibi genişlik-sonekli yerleşik tam sayı tipleri.
pub(crate) fn is_widened_int_type(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(['u', 'i']) else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::super::testutil::resolved;
    use super::super::DefKind;
    use super::is_widened_int_type;

    #[test]
    fn widened_int_types_are_a_letter_followed_by_digits_only() {
        assert!(is_widened_int_type("u9"));
        assert!(is_widened_int_type("i128"));
        assert!(!is_widened_int_type("u"));
        assert!(!is_widened_int_type("uart"));
        assert!(!is_widened_int_type("x8"));
        assert!(!is_widened_int_type("u8_t"));
    }

    #[test]
    fn prelude_holds_ten_builtins_and_no_trit() {
        let r = resolved("module M { in a : u8 out y : u8 y = a }");
        let builtins: Vec<&str> = r
            .defs
            .iter()
            .filter(|d| matches!(d.kind, DefKind::Builtin(_)))
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(builtins.len(), 10);
        assert_eq!(builtins[0], "sync");
        assert!(!builtins.contains(&"Trit"));
    }
}
