//! İsim çözümleme (docs/spec/name-resolution.md).
//!
//! İki geçiş: öğe seviyesinde ileri referans serbest (Geçiş 1),
//! modül/blok gövdeleri sıralı — bildirim kullanımdan önce (Geçiş 2,
//! ihlalde E1002). Prelude yerleşikleri §7'de; Trit prelude'de DEĞİL
//! (opt-in import, UX Anayasası).
//!
//! Modüller spec bölümlerini izler:
//!
//! | modül        | sorumluluk                                        | spec / ADR     |
//! |--------------|---------------------------------------------------|----------------|
//! | `mod`        | public API, `Resolver`, iki geçişin ana akışı     | §3.1, §4       |
//! | `def`        | `DefId`, `DefKind`, `BuiltinKind`, `DefData`      | §1             |
//! | `scope`      | kapsam ağacı, bildirim, çift tanım, gölgeleme     | §2, §6         |
//! | `prelude`    | yerleşik fonksiyonlar, genişletilmiş tam sayılar  | §7             |
//! | `imports`    | `use` kaydı (tek dosya), dosya süzgeci (birim)    | §3.3, ADR-0042 |
//! | `collect`    | Geçiş 1: öğe toplama                              | §4             |
//! | `body`       | Geçiş 2: öğe gövdeleri, modül gövdesi sırası      | §4, §5         |
//! | `domain_ref` | `@Ad` çözümü, extern gövdesi, sembolik alan       | ADR-0047       |
//! | `stmt`       | modül deyimleri, lvalue, atama yönü denetimleri   | §5, ADR-0039/51|
//! | `instance`   | örnekleme hedefi, port adı denetimi (E1009)       | ADR-0027       |
//! | `block`      | bloklar, if/match kolları, desenler               | §2, §5         |
//! | `expr`       | ifadeler, tip referansları, struct alanı, prev()  | §3, ADR-0040   |
//! | `path`       | yalın ve nitelikli yol, E1001/E1002               | §3.1, §3.2, §8 |
//! | `cycles`     | modül örnekleme döngüsü (E1006)                   | §10            |
//! | `usage`      | kullanılmayan tanım uyarıları                     | §9             |
//! | `suggest`    | Levenshtein önerisi, ortak öneri metni            | §8             |

mod block;
mod body;
mod collect;
mod cycles;
mod def;
mod domain_ref;
mod expr;
mod imports;
mod instance;
mod path;
mod prelude;
mod scope;
mod stmt;
mod suggest;
mod usage;

use std::collections::{HashMap, HashSet};

use volt_ast::{BundleOrigin, Expr, Idx, Item, Pattern, PortDir, SourceFile, TypeRef};
use volt_diagnostics::Diagnostic;
use volt_span::{FileId, Span};

use crate::builtin::BuiltinPrim;
use crate::unit::FileScope;

pub use def::{BuiltinKind, DefData, DefId, DefKind};
pub(crate) use prelude::is_widened_int_type;
pub use scope::{Scope, ScopeId, ScopeKind};
pub use suggest::closest_match;

use def::synthetic_span;

// ═══ Sonuç ════════════════════════════════════════════════════════

#[derive(Debug, Default)]
pub struct ResolveResult {
    pub defs: Vec<DefData>,
    pub diagnostics: Vec<Diagnostic>,
    /// Path ifadesi → çözülen tanım (const eval bu haritayı kullanır).
    pub resolutions: HashMap<Idx<Expr>, DefId>,
    /// Const tanımı → başlangıç ifadesi.
    pub const_inits: HashMap<DefId, Idx<Expr>>,
    /// Enum varyantı → (sıra, açık discriminant ifadesi).
    pub variant_info: HashMap<DefId, (usize, Option<Idx<Expr>>)>,
    /// Bildirim isminin span'ı → tanım (tip denetçisi bildirimleri bulur).
    pub decl_spans: HashMap<Span, DefId>,
    /// Kullanım isminin span'ı → tanım (lvalue tabanları için).
    pub use_spans: HashMap<Span, DefId>,
    /// Tip konumundaki Path → tanım (struct/enum/alias tipleri).
    pub type_resolutions: HashMap<Idx<TypeRef>, DefId>,
    /// Yol deseni (`State::Idle =>`) → çözülen tanım (ADR-0074 desen
    /// tiplemesi ve kapsayıcılık).
    pub pattern_resolutions: HashMap<Idx<Pattern>, DefId>,
    /// Okunan tanımlar (W4001/W4002 sürücü analizi için).
    pub reads: HashSet<DefId>,
    /// Instance tanımı → hedef modül tanımı.
    pub instance_module: HashMap<DefId, DefId>,
    /// Instance tanımı → yerleşik CDC primitifi (ADR-0027).
    pub instance_builtin: HashMap<DefId, BuiltinPrim>,
    /// Öğe tanımı → AST öğesi (port/alan tip araması için).
    pub item_of_def: HashMap<DefId, Idx<Item>>,
}

impl ResolveResult {
    pub fn error_codes(&self) -> Vec<&'static str> {
        self.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    pub fn def_kind(&self, def: DefId) -> DefKind {
        self.defs[def.0 as usize].kind
    }

    /// Test yardımcısı: isme göre ilk tanım.
    pub fn def_by_name(&self, name: &str) -> Option<(DefId, &DefData)> {
        self.defs
            .iter()
            .enumerate()
            .find(|(_, d)| d.name == name)
            .map(|(i, d)| (DefId(i as u32), d))
    }
}

/// Bir kaynak dosyanın tüm isimlerini çözer.
pub fn resolve_file(ast: &SourceFile) -> ResolveResult {
    let mut r = Resolver::new(ast);
    r.run();
    r.finish()
}

/// Çoklu dosya derleme birimini çözer (ADR-0042). `scopes` dosya başına
/// import görünürlüğüdür (`unit::check_imports`); kök kapsamdaki bir öğe
/// başka dosyadan yalnız import edilmişse görünür. `use` bildirimleri
/// burada YENİDEN kaydedilmez — birim denetimi (E1004/E1010/E1011)
/// `check_imports`'ta yapılmıştır.
pub fn resolve_unit(ast: &SourceFile, scopes: &HashMap<FileId, FileScope>) -> ResolveResult {
    let mut r = Resolver::new(ast);
    r.file_scopes = Some(scopes);
    r.run();
    r.finish()
}

struct Resolver<'a> {
    ast: &'a SourceFile,
    defs: Vec<DefData>,
    scopes: Vec<Scope>,
    diagnostics: Vec<Diagnostic>,
    resolutions: HashMap<Idx<Expr>, DefId>,
    const_inits: HashMap<DefId, Idx<Expr>>,
    variant_info: HashMap<DefId, (usize, Option<Idx<Expr>>)>,
    decl_spans: HashMap<Span, DefId>,
    use_spans: HashMap<Span, DefId>,
    type_resolutions: HashMap<Idx<TypeRef>, DefId>,
    pattern_resolutions: HashMap<Idx<Pattern>, DefId>,

    prelude: ScopeId,
    root: ScopeId,
    error_def: DefId,

    /// Okuma kullanımları (W1001/W1004/W1005/W3004 için).
    reads: HashSet<DefId>,
    /// Yazma kullanımları (W1004 yardım metni için).
    writes: HashSet<DefId>,
    /// "İleride bildirilecek" yığını — E1002 tespiti.
    pending: Vec<HashMap<String, Span>>,
    /// Öğe tanımı → AST öğesi (port/alan araması için).
    item_of_def: HashMap<DefId, Idx<Item>>,
    /// Enum tanımı → varyant isim/def listesi.
    enum_variants: HashMap<DefId, Vec<(String, DefId)>>,
    /// Instance tanımı → hedef modül tanımı.
    instance_module: HashMap<DefId, DefId>,
    /// Instance tanımı → yerleşik CDC primitifi (ADR-0027).
    instance_builtin: HashMap<DefId, BuiltinPrim>,
    /// Modül örnekleme kenarları (E1006 döngü tespiti).
    instance_edges: Vec<(DefId, DefId)>,
    /// Bundle'dan düzleştirilmiş GİRİŞ portları (ADR-0039) — atama
    /// hedefi olursa E4005.
    bundle_inputs: HashMap<DefId, BundleOrigin>,
    /// Çift yönlü portlar (ADR-0051) — doğrudan atama hedefi olursa E4008.
    bidir_ports: HashMap<DefId, PortDir>,
    /// Kontrat ifadesi çözümleniyor mu? prev() yalnız burada geçerli
    /// (E5017, ADR-0040).
    in_contract: bool,
    /// Derleme birimi modu (ADR-0042): dosya başına import görünürlüğü.
    /// `None` = tek dosya, kök kapsam süzülmez.
    file_scopes: Option<&'a HashMap<FileId, FileScope>>,
    /// Şu an çözümlenen öğenin dosyası (kök kapsam süzgeci için).
    current_file: FileId,
}

impl<'a> Resolver<'a> {
    fn new(ast: &'a SourceFile) -> Self {
        let mut r = Resolver {
            ast,
            defs: Vec::new(),
            scopes: Vec::new(),
            diagnostics: Vec::new(),
            resolutions: HashMap::new(),
            const_inits: HashMap::new(),
            variant_info: HashMap::new(),
            decl_spans: HashMap::new(),
            use_spans: HashMap::new(),
            type_resolutions: HashMap::new(),
            pattern_resolutions: HashMap::new(),
            prelude: ScopeId(0),
            root: ScopeId(0),
            error_def: DefId(0),
            reads: HashSet::new(),
            writes: HashSet::new(),
            pending: Vec::new(),
            item_of_def: HashMap::new(),
            enum_variants: HashMap::new(),
            instance_module: HashMap::new(),
            instance_builtin: HashMap::new(),
            instance_edges: Vec::new(),
            bundle_inputs: HashMap::new(),
            bidir_ports: HashMap::new(),
            in_contract: false,
            file_scopes: None,
            current_file: FileId(0),
        };
        r.prelude = r.new_scope(ScopeKind::Prelude, None);
        r.root = r.new_scope(ScopeKind::Root, Some(r.prelude));
        // Hata kurtarma tanımı — çözülemeyen her isim buna bağlanır.
        r.error_def = r.add_def(DefKind::Error, "<hata>", synthetic_span(), r.prelude, true);
        r.init_prelude();
        r
    }

    fn run(&mut self) {
        if self.file_scopes.is_none() {
            self.collect_imports();
        }
        // ── Geçiş 1: öğe toplama (ileri referans serbest) ──
        for &item in &self.ast.items {
            self.current_file = self.ast.items_arena[item].span.file;
            self.collect_item(item);
        }
        // ── Geçiş 2: gövde çözümleme ──
        for &item in &self.ast.items {
            self.current_file = self.ast.items_arena[item].span.file;
            self.resolve_item_body(item);
        }
        self.check_instance_cycles();
        self.report_unused();
    }

    fn finish(self) -> ResolveResult {
        ResolveResult {
            defs: self.defs,
            diagnostics: self.diagnostics,
            resolutions: self.resolutions,
            const_inits: self.const_inits,
            variant_info: self.variant_info,
            decl_spans: self.decl_spans,
            use_spans: self.use_spans,
            type_resolutions: self.type_resolutions,
            pattern_resolutions: self.pattern_resolutions,
            reads: self.reads,
            instance_module: self.instance_module,
            instance_builtin: self.instance_builtin,
            item_of_def: self.item_of_def,
        }
    }
}

#[cfg(test)]
mod testutil {
    use volt_syntax::{parse, FileId};

    use super::{resolve_file, ResolveResult};

    /// Kaynağı ayrıştırır (hatasız olmalı) ve YALNIZ isim çözümlemeyi koşar.
    pub(super) fn resolved(src: &str) -> ResolveResult {
        let parsed = parse(FileId(0), src);
        assert!(
            parsed.diagnostics.is_empty(),
            "kaynak ayrışmalı: {:?}",
            parsed.error_codes()
        );
        resolve_file(&parsed.ast)
    }

    /// İsim çözümleme tanı kodları, üretim sırasıyla.
    pub(super) fn codes(src: &str) -> Vec<&'static str> {
        resolved(src).error_codes()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use volt_syntax::{parse, FileId};

    use super::testutil::resolved;
    use super::{resolve_file, resolve_unit, DefKind};

    #[test]
    fn error_def_is_the_first_definition_and_lives_in_the_prelude() {
        let r = resolved("module M { in a : u8 out y : u8 y = a }");
        assert_eq!(r.defs[0].kind, DefKind::Error);
        assert_eq!(r.defs[0].name, "<hata>");
        assert_eq!(r.defs[0].scope.0, 0);
    }

    #[test]
    fn unit_mode_with_no_file_scopes_matches_single_file_definitions() {
        let parsed = parse(FileId(0), "module M { in a : u8 out y : u8 y = a }");
        let single = resolve_file(&parsed.ast);
        let unit = resolve_unit(&parsed.ast, &HashMap::new());
        assert_eq!(single.defs.len(), unit.defs.len());
        assert_eq!(single.error_codes(), unit.error_codes());
    }

    #[test]
    fn usage_warnings_come_after_body_diagnostics() {
        let r = resolved("module M { in a : u8 in b : u8 out y : u8 y = a + yok }");
        assert_eq!(r.error_codes(), ["E1001", "W1001"]);
    }
}
