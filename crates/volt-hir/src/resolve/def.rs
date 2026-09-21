//! Tanım kimlikleri ve türleri (name-resolution.md §1).

use volt_ast::PortDir;
use volt_span::{FileId, Span};

use super::scope::ScopeId;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DefId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefKind {
    // ── Öğe seviyesi ──
    Module,
    Domain,
    Function,
    Struct,
    Enum,
    EnumVariant {
        parent: DefId,
    },
    Const,
    TypeAlias,
    ExternModule,

    // ── Modül içi ──
    Port {
        dir: PortDir,
    },
    Register,
    Wire,
    Instance,

    // ── Yerel ──
    LocalBinding,
    LoopVar,
    PatternBinding,
    GenericParam,
    /// `extern module` içinde tanımsız `@Ad` — sembolik saat alanı
    /// parametresi (ADR-0047). Örneklemede saat bağlantısıyla gerçek
    /// alana bağlanır; extern kapsamı dışında görünmez.
    DomainParam,

    // ── Yerleşik ──
    Builtin(BuiltinKind),

    /// `use` ile getirilen dış isim — F1b tek dosya derlediği için
    /// gövdesi çözülmez; kullanımlar hatasız kabul edilir (W1005 izler).
    Import,

    /// Hata kurtarma — her kullanımla uyumlu.
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinKind {
    Sync,
    Sync3,
    Zext,
    Sext,
    Trunc,
    Concat,
    Replicate,
    PopCount,
    Clog2,
    /// prev(x[, N]) — kontratlarda N döngü önceki değer (ADR-0040).
    Prev,
}

#[derive(Debug)]
pub struct DefData {
    pub kind: DefKind,
    pub name: String,
    /// Bildirim konumu — hata mesajlarında "burada tanımlı".
    pub span: Span,
    pub scope: ScopeId,
    pub is_public: bool,
}

pub(super) fn synthetic_span() -> Span {
    Span::new(FileId(u32::MAX), 0, 0)
}

#[cfg(test)]
mod tests {
    use volt_span::FileId;

    use super::{synthetic_span, DefId};

    #[test]
    fn def_ids_order_by_declaration_index() {
        let mut ids = vec![DefId(3), DefId(0), DefId(2)];
        ids.sort();
        assert_eq!(ids, [DefId(0), DefId(2), DefId(3)]);
    }

    #[test]
    fn synthetic_span_belongs_to_no_real_file() {
        assert_eq!(synthetic_span().file, FileId(u32::MAX));
    }
}
