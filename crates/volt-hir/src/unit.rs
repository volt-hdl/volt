//! Çoklu dosya derleme birimi — `use` çözümlemesi dosya sınırını aşar
//! (ADR-0042, name-resolution.md §3.2 genişletmesi).
//!
//! Sürücü dosyaları bulur ve tek arena'ya ayrıştırır (`parse_unit`);
//! burada yalnız import'lar denetlenir:
//!   - hedef paketteki öğe yoksa            → E1011
//!   - öğe `pub` değilse                    → E1004
//!   - aynı yerel ad iki kez getiriliyorsa  → E1010
//!
//! Sonuç, dosya başına görünür ad kümesi + takma ad tablosudur; çözücü
//! kök kapsamdaki aramaları bununla süzer (`resolve_unit`).

use std::collections::{HashMap, HashSet};

use volt_ast::{ItemKind, Name, Path, SourceFile, UseTree, Visibility};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::{FileId, Span};

/// Paket yolu — `soc::gpio` → `["soc", "gpio"]`.
pub type PackagePath = Vec<String>;

/// Yerleşik kütüphane ön eki: `use std::...` dosya yüklemez.
pub const STD_PACKAGE: &str = "std";

/// Sürücünün kurduğu dosya ↔ paket eşlemesi.
#[derive(Debug, Default, Clone)]
pub struct UnitInfo {
    /// Paket yolu → o paketi tanımlayan dosya.
    pub files: HashMap<PackagePath, FileId>,
    /// Dosya → paket yolu.
    pub packages: HashMap<FileId, PackagePath>,
}

impl UnitInfo {
    pub fn add(&mut self, file: FileId, package: PackagePath) {
        self.files.insert(package.clone(), file);
        self.packages.insert(file, package);
    }
}

/// Bir dosyanın import'lardan gelen görünürlüğü.
#[derive(Debug, Default, Clone)]
pub struct FileScope {
    /// Bu dosyada yalın adıyla kullanılabilen dış öğeler (yerel ad).
    pub visible: HashSet<String>,
    /// Yerel ad → kök kapsamdaki gerçek ad (`use a::B as C` → C→B).
    pub aliases: HashMap<String, String>,
    /// Yerel ad → (paket, öğe adı, ilk import konumu) — E1010 için.
    pub origin: HashMap<String, (PackagePath, String, Name)>,
}

/// Import denetiminin sonucu.
#[derive(Debug, Default)]
pub struct ImportResult {
    pub scopes: HashMap<FileId, FileScope>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Tek bir `use` girdisi: hedef paket + öğe adı + yerel ad.
struct Entry {
    package: PackagePath,
    item: Name,
    local: Name,
}

fn segs(path: &Path) -> PackagePath {
    path.segments.iter().map(|s| s.text.clone()).collect()
}

pub fn package_display(p: &[String]) -> String {
    p.join("::")
}

/// AST öğesinin adı (test/hata öğeleri adsızdır).
pub fn item_name(kind: &ItemKind) -> Option<&Name> {
    match kind {
        ItemKind::Module(m) => Some(&m.name),
        ItemKind::Domain(d) => Some(&d.name),
        ItemKind::Fn(f) => Some(&f.name),
        ItemKind::Struct(s) => Some(&s.name),
        ItemKind::Enum(e) => Some(&e.name),
        ItemKind::Const(c) => Some(&c.name),
        ItemKind::TypeAlias(t) => Some(&t.name),
        ItemKind::Extern(x) => Some(&x.name),
        ItemKind::Test(_) | ItemKind::Error => None,
    }
}

/// Dosyanın üst düzey öğeleri: ad → görünürlük.
fn items_of(ast: &SourceFile, file: FileId) -> Vec<(&Name, Visibility)> {
    ast.items
        .iter()
        .map(|&i| &ast.items_arena[i])
        .filter(|item| item.span.file == file)
        .filter_map(|item| item_name(&item.kind).map(|n| (n, item.visibility)))
        .collect()
}

/// `use` ağacını düz girdilere açar. Glob için hedef dosyanın tüm `pub`
/// öğeleri; `use paket;` (öğesiz) hiçbir ad getirmez.
fn expand(ast: &SourceFile, info: &UnitInfo, use_decl: &volt_ast::UseDecl) -> Vec<Entry> {
    let base = segs(&use_decl.path);
    let mut out = Vec::new();
    let mut push = |package: PackagePath, item: &Name, local: &Name| {
        out.push(Entry {
            package,
            item: item.clone(),
            local: local.clone(),
        });
    };
    match &use_decl.tree {
        None | Some(UseTree::Alias(_)) => {
            let Some((last, prefix)) = use_decl.path.segments.split_last() else {
                return out;
            };
            if prefix.is_empty() {
                return out;
            }
            let local = match &use_decl.tree {
                Some(UseTree::Alias(a)) => a,
                _ => last,
            };
            push(prefix.iter().map(|s| s.text.clone()).collect(), last, local);
        }
        Some(UseTree::List(paths)) => {
            for p in paths {
                let Some((last, prefix)) = p.segments.split_last() else {
                    continue;
                };
                let mut package = base.clone();
                package.extend(prefix.iter().map(|s| s.text.clone()));
                push(package, last, last);
            }
        }
        Some(UseTree::Glob) => {
            if let Some(&target) = info.files.get(&base) {
                for (name, vis) in items_of(ast, target) {
                    if vis == Visibility::Public {
                        push(base.clone(), name, name);
                    }
                }
            } else {
                // Paket yok: tek E1011 üretmek için sahte girdi.
                let marker = Name {
                    text: "*".into(),
                    span: use_decl.span,
                };
                push(base.clone(), &marker, &marker);
            }
        }
    }
    out
}

/// Tüm `use` bildirimlerini denetler ve dosya kapsamlarını kurar.
pub fn check_imports(ast: &SourceFile, info: &UnitInfo) -> ImportResult {
    let mut result = ImportResult::default();
    for use_decl in &ast.uses {
        let file = use_decl.span.file;
        if use_decl
            .path
            .segments
            .first()
            .is_some_and(|s| s.text == STD_PACKAGE)
        {
            continue;
        }
        for entry in expand(ast, info, use_decl) {
            let scope = result.scopes.entry(file).or_default();
            check_entry(ast, info, &entry, scope, &mut result.diagnostics);
        }
    }
    result
}

fn check_entry(
    ast: &SourceFile,
    info: &UnitInfo,
    entry: &Entry,
    scope: &mut FileScope,
    diags: &mut Vec<Diagnostic>,
) {
    let pkg = package_display(&entry.package);
    let Some(&target) = info.files.get(&entry.package) else {
        diags.push(module_not_found(&pkg, entry.item.span));
        return;
    };
    if entry.item.text == "*" {
        return;
    }
    let items = items_of(ast, target);
    // Generic şablon (`Delay<N, W>`) monomorfizasyondan sonra yalnız
    // örnekleri (`Delay_4_8`) olarak yaşar — şablon adı onlardan görünür.
    let found = items
        .iter()
        .find(|(n, _)| n.text == entry.item.text)
        .or_else(|| {
            items
                .iter()
                .find(|(n, _)| mono_base(&n.text) == entry.item.text)
        });
    let Some((_, vis)) = found else {
        let public: Vec<&str> = items
            .iter()
            .filter(|(_, v)| *v == Visibility::Public)
            .map(|(n, _)| n.text.as_str())
            .collect();
        diags.push(item_not_found(&pkg, &entry.item, &public));
        return;
    };
    if *vis == Visibility::Private {
        diags.push(private_item(&pkg, &entry.item));
        return;
    }
    let local = &entry.local;
    if let Some((prev_pkg, prev_item, prev_name)) = scope.origin.get(&local.text) {
        if *prev_pkg != entry.package || *prev_item != entry.item.text {
            diags.push(ambiguous(local, prev_name));
            return;
        }
    }
    scope.visible.insert(local.text.clone());
    if local.text != entry.item.text {
        scope
            .aliases
            .insert(local.text.clone(), entry.item.text.clone());
    }
    scope.origin.insert(
        local.text.clone(),
        (
            entry.package.clone(),
            entry.item.text.clone(),
            local.clone(),
        ),
    );
}

/// `Fifo_8_16` → `Fifo`: monomorfizasyon son ekini (`_<sayı>`*) atar.
pub(crate) fn mono_base(name: &str) -> &str {
    let mut base = name;
    while let Some((head, tail)) = base.rsplit_once('_') {
        if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit()) {
            break;
        }
        base = head;
    }
    base
}

/// E1011 — paket bulunamadı (sürücü aranan yolları `note` ile ekler).
pub fn module_not_found(pkg: &str, span: Span) -> Diagnostic {
    let rel = pkg.replace("::", "/");
    Diagnostic::error(
        ErrorCode::E1011,
        lstr!(en: "module not found: '{pkg}'"; tr: "modül bulunamadı: '{pkg}'"),
        LabeledSpan::primary(
            span,
            lstr!(en: "no file provides this package"; tr: "bu paketi sağlayan dosya yok"),
        ),
        lstr!(en: "create '{rel}.volt' next to this file or under <root>/src/, and declare 'package {pkg};' in it";
              tr: "bu dosyanın yanında ya da <kök>/src/ altında '{rel}.volt' oluşturun ve içinde 'package {pkg};' bildirin"),
    )
}

fn item_not_found(pkg: &str, item: &Name, public: &[&str]) -> Diagnostic {
    let help = if public.is_empty() {
        lstr!(en: "package '{pkg}' exports nothing — mark items 'pub'";
              tr: "'{pkg}' paketi hiçbir şey dışa açmıyor — öğeleri 'pub' yapın")
    } else {
        lstr!(en: "public items of '{pkg}': {}", public.join(", ");
              tr: "'{pkg}' paketinin açık öğeleri: {}", public.join(", "))
    };
    Diagnostic::error(
        ErrorCode::E1011,
        lstr!(en: "'{}' not found in module '{pkg}'", item.text;
              tr: "'{}' '{pkg}' modülünde bulunamadı", item.text),
        LabeledSpan::primary(
            item.span,
            lstr!(en: "not defined in that package"; tr: "o pakette tanımlı değil"),
        ),
        help,
    )
}

fn private_item(pkg: &str, item: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E1004,
        lstr!(en: "'{}' is private to module '{pkg}'", item.text;
              tr: "'{}' '{pkg}' modülüne özeldir", item.text),
        LabeledSpan::primary(
            item.span,
            lstr!(en: "not visible from here"; tr: "buradan görünmüyor"),
        ),
        lstr!(en: "add 'pub' in front of the declaration of '{}' in package '{pkg}'", item.text;
              tr: "'{pkg}' paketindeki '{}' bildiriminin önüne 'pub' ekleyin", item.text),
    )
}

fn ambiguous(local: &Name, prev: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E1010,
        lstr!(en: "ambiguous import: '{}' is brought in twice", local.text;
              tr: "belirsiz import: '{}' iki kez getiriliyor", local.text),
        LabeledSpan::primary(
            local.span,
            lstr!(en: "second import here"; tr: "ikinci import burada"),
        ),
        lstr!(en: "give one of them an alias with 'as': use path::item as NewName";
              tr: "birine 'as' ile takma ad verin: use yol::öğe as YeniAd"),
    )
    .with_secondary(
        prev.span,
        lstr!(en: "first import here"; tr: "ilk import burada"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(files: &[(&str, &str)]) -> (SourceFile, UnitInfo) {
        let sources: Vec<(FileId, &str)> = files
            .iter()
            .enumerate()
            .map(|(i, (_, src))| (FileId(i as u32), *src))
            .collect();
        let parsed = volt_syntax::parse_unit(&sources);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
        let mut info = UnitInfo::default();
        for (i, (pkg, _)) in files.iter().enumerate() {
            info.add(
                FileId(i as u32),
                pkg.split("::").map(str::to_string).collect(),
            );
        }
        (parsed.ast, info)
    }

    const LIB: &str = "pub module Open { in clk : clock in a : bool out b : bool b = a }\nmodule Hidden { in clk : clock in a : bool out b : bool b = a }\n";

    fn codes(r: &ImportResult) -> Vec<&'static str> {
        r.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn plain_import_of_public_item_is_visible() {
        let (ast, info) = unit(&[("lib", LIB), ("main", "use lib::Open;\n")]);
        let r = check_imports(&ast, &info);
        assert!(r.diagnostics.is_empty(), "{:?}", codes(&r));
        assert!(r.scopes[&FileId(1)].visible.contains("Open"));
        assert!(r.scopes[&FileId(1)].aliases.is_empty());
    }

    #[test]
    fn private_item_import_is_e1004() {
        let (ast, info) = unit(&[("lib", LIB), ("main", "use lib::Hidden;\n")]);
        let r = check_imports(&ast, &info);
        assert_eq!(codes(&r), ["E1004"]);
        assert!(r.scopes[&FileId(1)].visible.is_empty());
    }

    #[test]
    fn missing_item_is_e1011_listing_public_items() {
        let (ast, info) = unit(&[("lib", LIB), ("main", "use lib::Nope;\n")]);
        let r = check_imports(&ast, &info);
        assert_eq!(codes(&r), ["E1011"]);
        let help = format!("{:?}", r.diagnostics[0]);
        assert!(help.contains("Open"), "{help}");
        assert!(!help.contains("Hidden"), "{help}");
    }

    #[test]
    fn missing_package_is_e1011() {
        let (ast, info) = unit(&[("main", "use ghost::Thing;\n")]);
        let r = check_imports(&ast, &info);
        assert_eq!(codes(&r), ["E1011"]);
    }

    #[test]
    fn alias_import_maps_local_to_root_name() {
        let (ast, info) = unit(&[("lib", LIB), ("main", "use lib::Open as Door;\n")]);
        let r = check_imports(&ast, &info);
        assert!(r.diagnostics.is_empty(), "{:?}", codes(&r));
        let scope = &r.scopes[&FileId(1)];
        assert!(scope.visible.contains("Door"));
        assert_eq!(scope.aliases["Door"], "Open");
    }

    #[test]
    fn glob_import_brings_only_public_items() {
        let (ast, info) = unit(&[("lib", LIB), ("main", "use lib::*;\n")]);
        let r = check_imports(&ast, &info);
        assert!(r.diagnostics.is_empty(), "{:?}", codes(&r));
        let scope = &r.scopes[&FileId(1)];
        assert!(scope.visible.contains("Open"));
        assert!(!scope.visible.contains("Hidden"));
    }

    #[test]
    fn list_import_with_nested_path_targets_sub_package() {
        let (ast, info) = unit(&[("soc::gpio", LIB), ("main", "use soc::{gpio::Open};\n")]);
        let r = check_imports(&ast, &info);
        assert!(r.diagnostics.is_empty(), "{:?}", codes(&r));
        assert!(r.scopes[&FileId(1)].visible.contains("Open"));
    }

    #[test]
    fn same_local_name_from_two_packages_is_e1010() {
        let (ast, info) = unit(&[
            ("a", LIB),
            ("b", LIB),
            ("main", "use a::Open;\nuse b::Open;\n"),
        ]);
        let r = check_imports(&ast, &info);
        assert_eq!(codes(&r), ["E1010"]);
    }

    #[test]
    fn repeated_identical_import_is_not_ambiguous() {
        let (ast, info) = unit(&[("a", LIB), ("main", "use a::Open;\nuse a::Open;\n")]);
        let r = check_imports(&ast, &info);
        assert!(r.diagnostics.is_empty(), "{:?}", codes(&r));
    }

    #[test]
    fn std_prefix_is_builtin_and_never_e1011() {
        let (ast, info) = unit(&[("main", "use std::fifo::SyncFifo;\n")]);
        let r = check_imports(&ast, &info);
        assert!(r.diagnostics.is_empty(), "{:?}", codes(&r));
    }

    #[test]
    fn item_name_covers_named_items_only() {
        let parsed = volt_syntax::parse(FileId(0), "const K : u8 = 1\nstruct S { a : u8 }\n");
        let names: Vec<_> = parsed
            .ast
            .items
            .iter()
            .filter_map(|&i| item_name(&parsed.ast.items_arena[i].kind))
            .map(|n| n.text.as_str())
            .collect();
        assert_eq!(names, ["K", "S"]);
    }
}
