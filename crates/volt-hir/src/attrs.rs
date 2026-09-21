//! Uygulanmayan nitelik denetimi (ADR-0048): ayrıştırılan ama hiçbir
//! geçidin yorumlamadığı nitelikler W0021 üretir.
//!
//! UX Anayasası "sessizce yok sayma" yasağı: `@timing(...)` gramerde
//! olduğu için W0020 üretmez, ama hiçbir aşama okumaz ve SDC yazılmaz.
//! Kullanıcı bir kısıt yazdığını sanır. Bu geçit, `UNENFORCED_ATTRIBUTES`
//! listesindeki her kullanımı bir uyarıyla görünür kılar; bir nitelik
//! gerçekten uygulanmaya başlayınca listeden ÇIKARILIR (nitelik başına
//! gerekçe metni de onunla gider).
//!
//! Susturma:
//!   * aynı düğümde `@allow(unenforced)` — o düğümün tüm uygulanmayan
//!     nitelikleri;
//!   * öğe düzeyinde `@allow(unenforced)` — öğenin portları, alanları
//!     ve gövde deyimleri de dâhil (kapsamlı susturma);
//!   * Volt.toml `[lint] unenforced_attributes = "allow"` — sürücü ve
//!     LSP `UnenforcedLint::discover` ile aynı kararı verir, W0021 hiç
//!     üretilmez.
//!
//! `@allow` argümanı yalnız `unenforced` olabilir; başka ya da eksik
//! argüman E0009 (geçersiz nitelik argümanı) — politika ne olursa olsun.

use std::collections::HashSet;
use std::path::Path;

use volt_ast::{AttrArg, Attribute, ExprKind, ItemKind, SourceFile};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::{FileId, Span};

/// Gramerde tanınan (W0020 üretmeyen) ama hiçbir geçidin yorumlamadığı
/// nitelikler. Sıra grammar-full.ebnf §2 ile aynıdır.
pub const UNENFORCED_ATTRIBUTES: &[&str] = &[
    // [F2]
    "domain",
    // [F4] — @timing / @false_path / @multicycle ADR-0054 ile uygulanıyor
    // (constraints.rs), listeden çıktılar.
    "budget",
    // [F5]
    "version",
    "abi_version",
    // [V1]
    "dft",
    "debug_visible",
    "debug_trace",
    "synthesis_target",
];

/// `@allow(...)` niteliğinin adı ve W0021 için kabul ettiği tek argüman.
const ALLOW_ATTRIBUTE: &str = "allow";
const ALLOW_UNENFORCED: &str = "unenforced";

/// Volt.toml `[lint] unenforced_attributes` politikası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnenforcedLint {
    /// Varsayılan: her uygulanmayan nitelik W0021.
    #[default]
    Warn,
    /// W0021 üretilmez (E0009 yine üretilir).
    Allow,
}

impl UnenforcedLint {
    /// `"allow"` / `"warn"` metnini çözer; tanınmayan değer `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().trim_matches('"') {
            "allow" => Some(UnenforcedLint::Allow),
            "warn" => Some(UnenforcedLint::Warn),
            _ => None,
        }
    }

    /// Volt.toml metninden `[lint] unenforced_attributes` değerini okur.
    /// Tek anahtar için TOML bağımlılığı almamak adına satır tarayıcı
    /// (sürücüdeki `Manifest::parse` ile aynı disiplin). Tanınmayan ya
    /// da eksik değer `Warn`: susturma kazara açılamaz, yalnız kapanır.
    pub fn from_manifest(text: &str) -> Self {
        let mut in_lint = false;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.starts_with('[') {
                in_lint = line == "[lint]";
                continue;
            }
            if !in_lint {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "unenforced_attributes" {
                    return Self::parse(value).unwrap_or_default();
                }
            }
        }
        Self::default()
    }

    /// `dir`den yukarı doğru ilk Volt.toml dosyasının politikası (git
    /// kökü / ev dizini tavanı ve `VOLT_MANIFEST_DIR`, ADR-0061); manifest
    /// yoksa ya da okunamıyorsa `Warn`. Sürücü ve LSP aynı kararı verir.
    pub fn discover(dir: Option<&Path>) -> Self {
        dir.and_then(|d| crate::manifest_search::find_manifest_dir(d).ok())
            .and_then(|root| std::fs::read_to_string(root.join(crate::MANIFEST_FILE)).ok())
            .map(|t| Self::from_manifest(&t))
            .unwrap_or_default()
    }
}

/// Tüm nitelik taşıyan düğümleri (öğe, port, struct alanı, deyim) tarar.
pub fn check_attributes(ast: &SourceFile, lint: UnenforcedLint) -> Vec<Diagnostic> {
    let mut checker = AttrChecker {
        ast,
        lint,
        diags: Vec::new(),
        seen: HashSet::new(),
    };
    for &item_idx in &ast.items {
        let item = &ast.items_arena[item_idx];
        let item_allows = checker.check_node(&item.attrs, false);
        match &item.kind {
            ItemKind::Module(m) => {
                for port in &m.ports {
                    checker.check_node(&port.attrs, item_allows);
                }
                for &stmt_idx in &m.body {
                    checker.check_node(&ast.stmts[stmt_idx].attrs, item_allows);
                }
            }
            ItemKind::Extern(e) => {
                for port in &e.ports {
                    checker.check_node(&port.attrs, item_allows);
                }
            }
            ItemKind::Struct(s) => {
                for field in &s.fields {
                    checker.check_node(&field.attrs, item_allows);
                }
            }
            _ => {}
        }
    }
    checker.diags
}

struct AttrChecker<'a> {
    ast: &'a SourceFile,
    lint: UnenforcedLint,
    diags: Vec<Diagnostic>,
    /// Raporlanan nitelik konumları (`ctx` hariç): generic bir modülün
    /// monomorph klonları aynı kaynak satırını taşır; kullanıcı bir
    /// nitelik yazdıysa bir uyarı görür.
    seen: HashSet<(FileId, u32, u32)>,
}

impl AttrChecker<'_> {
    /// Bir düğümün nitelik listesini denetler; düğümün (ya da kapsayan
    /// öğenin) `@allow(unenforced)` taşıyıp taşımadığını döndürür.
    fn check_node(&mut self, attrs: &[Attribute], inherited_allow: bool) -> bool {
        let allowed = inherited_allow || attrs.iter().any(|a| self.is_allow_unenforced(a));
        for attr in attrs {
            if attr.name.text == ALLOW_ATTRIBUTE {
                self.check_allow_args(attr);
            } else if !allowed
                && self.lint == UnenforcedLint::Warn
                && UNENFORCED_ATTRIBUTES.contains(&attr.name.text.as_str())
                && self
                    .seen
                    .insert((attr.span.file, attr.span.start, attr.span.end))
            {
                self.diags.push(unenforced_warning(attr));
            }
        }
        allowed
    }

    fn is_allow_unenforced(&self, attr: &Attribute) -> bool {
        attr.name.text == ALLOW_ATTRIBUTE
            && attr.args.len() == 1
            && self.positional_ident(&attr.args[0]) == Some(ALLOW_UNENFORCED)
    }

    /// `@allow` yalnız `@allow(unenforced)` biçiminde geçerlidir.
    fn check_allow_args(&mut self, attr: &Attribute) {
        if self.is_allow_unenforced(attr) {
            return;
        }
        // Altı çizilen: ilk HATALI argüman (`@allow(unenforced, extra)`
        // için `extra`); argüman hiç yoksa niteliğin kendisi.
        let span = attr
            .args
            .iter()
            .find(|a| self.positional_ident(a) != Some(ALLOW_UNENFORCED))
            .map_or(attr.span, |a| match a {
                AttrArg::Named { name, .. } => name.span,
                AttrArg::Positional(e) => self.ast.exprs[*e].span,
            });
        self.diags.push(
            Diagnostic::error(
                ErrorCode::E0009,
                lstr!(en: "invalid argument to '@allow'"; tr: "'@allow' argümanı geçersiz"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "expected 'unenforced'"; tr: "'unenforced' bekleniyor"),
                ),
                lstr!(en: "write it as @allow(unenforced) — the only lint that can be allowed today";
                      tr: "@allow(unenforced) biçiminde yazın — bugün izin verilebilen tek lint bu"),
            )
            .with_note(
                NoteKind::Note,
                lstr!(en: "a misspelled @allow silences nothing, so the W0021 it was meant to hide stays visible";
                      tr: "yanlış yazılmış @allow hiçbir şeyi susturmaz; gizlemesi beklenen W0021 görünür kalır"),
            ),
        );
    }

    fn positional_ident(&self, arg: &AttrArg) -> Option<&str> {
        match arg {
            AttrArg::Positional(e) => match &self.ast.exprs[*e].kind {
                ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.as_str()),
                _ => None,
            },
            AttrArg::Named { .. } => None,
        }
    }
}

/// W0021: nitelik başına gerekçe (`= reason:`) ve ne yapmalı (`= help:`).
fn unenforced_warning(attr: &Attribute) -> Diagnostic {
    let name = attr.name.text.as_str();
    let (reason, help) = reason_and_help(name);
    Diagnostic::warning(
        ErrorCode::W0021,
        lstr!(en: "attribute '@{name}' is parsed but not yet enforced";
              tr: "'@{name}' niteliği ayrıştırılıyor ama henüz uygulanmıyor"),
        // Altı çizilen: `@ad` (argümanlar hariç) — kullanıcı neyin yok
        // sayıldığını bir bakışta görür.
        LabeledSpan::primary(
            Span {
                end: attr.name.span.end,
                ..attr.span
            },
            lstr!(en: "no constraint or check is generated from this";
                  tr: "bundan hiçbir kısıt ya da denetim üretilmiyor"),
        ),
        help,
    )
    .with_note(NoteKind::Reason, reason)
    .with_note(
        NoteKind::Note,
        lstr!(en: "the attribute is syntactically valid and will be honored in a future release; silence this with @allow(unenforced) on the same item or Volt.toml [lint] unenforced_attributes = \"allow\"";
              tr: "nitelik sözdizimsel olarak geçerli, ileriki bir sürümde uygulanacak; aynı öğede @allow(unenforced) ya da Volt.toml [lint] unenforced_attributes = \"allow\" ile susturulabilir"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "still unenforced: {}; @timing, @false_path and @multicycle are enforced since ADR-0054 (volt build --emit=sdc)", unenforced_list();
              tr: "hâlâ uygulanmayan: {}; @timing, @false_path ve @multicycle ADR-0054'ten beri uygulanıyor (volt build --emit=sdc)", unenforced_list()),
    )
}

/// `@ad, @ad, ...` — W0021 notunda hâlâ uygulanmayan niteliklerin listesi.
fn unenforced_list() -> String {
    UNENFORCED_ATTRIBUTES
        .iter()
        .map(|a| format!("@{a}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Nitelik ailesine göre gerekçe ve öneri metni (ADR-0048 tablosu).
fn reason_and_help(name: &str) -> (String, String) {
    match name {
        "budget" => (
            lstr!(en: "resource budget checking (E6001) needs synthesis estimates that are not implemented yet";
                  tr: "kaynak bütçesi denetimi (E6001) henüz gerçeklenmemiş sentez kestirimlerine dayanır"),
            lstr!(en: "check the vendor utilization report meanwhile, or silence with @allow(unenforced)";
                  tr: "şimdilik üretici kullanım raporuna bakın ya da @allow(unenforced) ile susturun"),
        ),
        "version" | "abi_version" => (
            lstr!(en: "interface versioning checks (E7001/E7002) are not implemented yet";
                  tr: "arayüz sürüm denetimleri (E7001/E7002) henüz gerçeklenmedi"),
            lstr!(en: "keep the version in review notes meanwhile, or silence with @allow(unenforced)";
                  tr: "şimdilik sürümü inceleme notlarında tutun ya da @allow(unenforced) ile susturun"),
        ),
        "domain" => (
            lstr!(en: "the @domain attribute has no interpreter; clock domains are assigned with the @Name annotation on ports";
                  tr: "@domain niteliğinin yorumlayıcısı yok; saat alanları port üzerindeki @Ad anotasyonuyla atanır"),
            lstr!(en: "annotate the port instead ('in clk : clock @Fast'), or silence with @allow(unenforced)";
                  tr: "bunun yerine portu anotasyonlayın ('in clk : clock @Fast') ya da @allow(unenforced) ile susturun"),
        ),
        "dft" | "debug_visible" | "debug_trace" | "synthesis_target" => (
            lstr!(en: "synthesis and debug hints (@dft, @debug_visible, @debug_trace, @synthesis_target) are V1 features not implemented yet";
                  tr: "sentez ve hata ayıklama ipuçları (@dft, @debug_visible, @debug_trace, @synthesis_target) henüz gerçeklenmemiş V1 özellikleridir"),
            lstr!(en: "apply the hint in the vendor flow meanwhile, or silence with @allow(unenforced)";
                  tr: "şimdilik ipucunu üretici akışında uygulayın ya da @allow(unenforced) ile susturun"),
        ),
        // Listeye eklenen yeni bir nitelik gerekçesini de getirmeli; bu
        // dal yalnız o unutulursa konuşur ve unutulduğunu söyler.
        _ => (
            lstr!(en: "no compiler pass interprets '@{name}' yet";
                  tr: "'@{name}' niteliğini henüz hiçbir derleyici geçidi yorumlamıyor"),
            lstr!(en: "remove the attribute or silence with @allow(unenforced)";
                  tr: "niteliği kaldırın ya da @allow(unenforced) ile susturun"),
        ),
    }
}
