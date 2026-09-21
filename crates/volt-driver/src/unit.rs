//! Çoklu dosya derleme birimi yükleyicisi (ADR-0042).
//!
//! `volt build <dosya>` bağımlılıkları `use` bildirimlerinden kendisi
//! bulur. `use soc::gpio::Gpio;` için arama sırası:
//!   1. `./soc/gpio.volt`            (import eden dosyanın dizini)
//!   2. `<kök>/<src>/soc/gpio.volt`  (Volt.toml'un dizini; `src` varsayılan "src")
//!   3. `std` ön eki — yerleşik, dosya yüklenmez
//!
//! Bulunan dosyalar bağımlılık sırasıyla (önce bağımlılıklar) TEK
//! arena'ya ayrıştırılır (`parse_unit`); span'ler kendi dosyasını taşır.
//! Bulunamayan paket E1011 (denenen yollar not olarak), döngüsel import
//! E1006 üretir; keşif hatada durmaz, tüm hatalar toplanır.
//! Artımlı derleme yoktur — her build sıfırdan.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use volt_ast::{SourceFile, UseTree};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_hir::unit::{module_not_found, PackagePath, STD_PACKAGE};
use volt_hir::{SearchStop, UnenforcedLint, UnitInfo};
use volt_span::{FileId, SourceMap, Span};
use volt_syntax::ParseResult;

/// Yüklenmiş derleme birimi.
pub struct LoadedUnit {
    pub map: SourceMap,
    /// Bulunan Volt.toml (ADR-0048 `[lint]` politikası buradan okunur).
    pub manifest: Option<Manifest>,
    /// Birimdeki dosyalar, bağımlılık sırasıyla (ana dosya SONDA).
    pub files: Vec<(FileId, PathBuf)>,
    pub info: UnitInfo,
    /// Keşif tanıları: E1011 (bulunamadı), E1006 (döngü).
    pub diagnostics: Vec<Diagnostic>,
    /// Tüm dosyaların birleşik AST'si.
    pub parsed: ParseResult,
}

/// Volt.toml `[package]` bölümü. Tek anahtarlar için tam TOML
/// bağımlılığı almamak adına satır tarayıcı (bkz. `manifest_lang`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub root: PathBuf,
    pub name: Option<String>,
    /// Kaynak kök dizini, `root`'a göre (varsayılan `src`).
    pub src: PathBuf,
    /// `[lint] unenforced_attributes` (ADR-0048); varsayılan `warn`.
    pub lint_unenforced: UnenforcedLint,
}

impl Manifest {
    /// `dir`'den yukarı doğru ilk Volt.toml (tavan ve `VOLT_MANIFEST_DIR`:
    /// ADR-0061).
    pub fn discover(dir: &Path) -> Option<Manifest> {
        Self::lookup(dir).ok()
    }

    /// `discover`; bulunamazsa aramanın nerede durduğunu döndürür.
    /// Okunamayan manifest yok sayılır (`Exhausted`).
    pub fn lookup(dir: &Path) -> Result<Manifest, SearchStop> {
        let root = volt_hir::find_manifest_dir(dir)?;
        let text = std::fs::read_to_string(root.join(volt_hir::MANIFEST_FILE))
            .map_err(|_| SearchStop::Exhausted)?;
        Ok(Manifest::parse(root, &text))
    }

    pub fn parse(root: PathBuf, text: &str) -> Manifest {
        let mut name = None;
        let mut src = PathBuf::from("src");
        // `[lint]` bölümü volt-hir'de okunur (ADR-0048): sürücü ve LSP
        // aynı tarayıcıyı paylaşır. Tanınmayan değer `warn`'a düşer —
        // güvenli yön: susturma kazara açılamaz, yalnız kapanır.
        let lint_unenforced = UnenforcedLint::from_manifest(text);
        let mut in_package = false;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.starts_with('[') {
                in_package = line == "[package]";
                continue;
            }
            if !in_package {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"').to_string();
            match key.trim() {
                "name" => name = Some(value),
                "src" => src = PathBuf::from(value),
                _ => {}
            }
        }
        Manifest {
            root,
            name,
            src,
            lint_unenforced,
        }
    }

    pub fn src_dir(&self) -> PathBuf {
        self.root.join(&self.src)
    }
}

/// Ön ayrıştırmadan çıkarılan bir `use` hedefi: paket yolu + span.
struct UseTarget {
    package: PackagePath,
    span: Span,
}

fn use_targets(ast: &SourceFile) -> Vec<UseTarget> {
    let mut out = Vec::new();
    for u in &ast.uses {
        let base: Vec<String> = u.path.segments.iter().map(|s| s.text.clone()).collect();
        if base.first().is_some_and(|s| s == STD_PACKAGE) {
            continue;
        }
        match &u.tree {
            None | Some(UseTree::Alias(_)) => {
                if base.len() >= 2 {
                    out.push(UseTarget {
                        package: base[..base.len() - 1].to_vec(),
                        span: u.path.span,
                    });
                }
            }
            Some(UseTree::Glob) => out.push(UseTarget {
                package: base,
                span: u.path.span,
            }),
            Some(UseTree::List(paths)) => {
                for p in paths {
                    let mut package = base.clone();
                    package.extend(
                        p.segments[..p.segments.len().saturating_sub(1)]
                            .iter()
                            .map(|s| s.text.clone()),
                    );
                    out.push(UseTarget {
                        package,
                        span: p.span,
                    });
                }
            }
        }
    }
    out
}

struct Loader {
    map: SourceMap,
    manifest: Option<Manifest>,
    /// Manifest yoksa aramanın durduğu yer (E1011 notu).
    search_stop: Option<SearchStop>,
    /// Kanonik yol → dosya kimliği.
    seen: HashMap<PathBuf, FileId>,
    /// DFS yığını (E1006).
    stack: Vec<FileId>,
    /// Bağımlılık sıralı çıktı.
    order: Vec<(FileId, PathBuf)>,
    /// Dosya → metin (birim ayrıştırması için).
    texts: HashMap<FileId, String>,
    info: UnitInfo,
    diagnostics: Vec<Diagnostic>,
    /// Aynı paket için E1011'i bir kez raporla.
    missing: HashSet<PackagePath>,
}

impl Loader {
    fn candidates(&self, from_dir: &Path, package: &[String]) -> Vec<PathBuf> {
        let rel: PathBuf = package.iter().collect();
        let rel = rel.with_extension("volt");
        let mut out = vec![from_dir.join(&rel)];
        if let Some(m) = &self.manifest {
            let under_src = m.src_dir().join(&rel);
            if !out.contains(&under_src) {
                out.push(under_src);
            }
        }
        out
    }

    fn canonical(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    /// Dosyayı yükler; `reached_as` bu dosyaya varılan paket yolu.
    fn visit(&mut self, path: &Path, reached_as: Option<PackagePath>) -> std::io::Result<FileId> {
        let key = Self::canonical(path);
        if let Some(&fid) = self.seen.get(&key) {
            if let Some(p) = reached_as {
                self.info.files.entry(p).or_insert(fid);
            }
            return Ok(fid);
        }
        let text = std::fs::read_to_string(path)?;
        let fid = self.map.add_file(path.display().to_string(), text.clone());
        self.seen.insert(key, fid);
        self.stack.push(fid);

        // Ön ayrıştırma: yalnız package/use okunur (tanılar birim
        // ayrıştırmasında yeniden üretilir).
        let pre = volt_syntax::parse(fid, &text);
        let declared: Option<PackagePath> = pre
            .ast
            .packages
            .first()
            .map(|p| p.path.segments.iter().map(|s| s.text.clone()).collect());
        let own_package = declared
            .clone()
            .or_else(|| reached_as.clone())
            .unwrap_or_else(|| vec![stem_of(path)]);
        self.info.add(fid, own_package);
        if let Some(p) = reached_as {
            self.info.files.entry(p).or_insert(fid);
        }
        if let Some(d) = declared {
            self.info.files.entry(d).or_insert(fid);
        }

        let dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        for target in use_targets(&pre.ast) {
            self.resolve_target(&dir, target)?;
        }

        self.stack.pop();
        self.texts.insert(fid, text);
        self.order.push((fid, path.to_path_buf()));
        Ok(fid)
    }

    fn resolve_target(&mut self, dir: &Path, target: UseTarget) -> std::io::Result<()> {
        if self.info.files.contains_key(&target.package) {
            let fid = self.info.files[&target.package];
            if self.stack.contains(&fid) {
                self.diagnostics
                    .push(cyclic_import(&target, &self.map, &self.stack, fid));
            }
            return Ok(());
        }
        let candidates = self.candidates(dir, &target.package);
        match candidates.iter().find(|c| c.is_file()) {
            Some(found) => {
                let found = found.clone();
                let key = Self::canonical(&found);
                if let Some(&fid) = self.seen.get(&key) {
                    if self.stack.contains(&fid) {
                        self.diagnostics
                            .push(cyclic_import(&target, &self.map, &self.stack, fid));
                        self.info.files.entry(target.package).or_insert(fid);
                        return Ok(());
                    }
                }
                self.visit(&found, Some(target.package))?;
            }
            None => {
                if self.missing.insert(target.package.clone()) {
                    let pkg = target.package.join("::");
                    let mut diag = module_not_found(&pkg, target.span);
                    let tried: Vec<String> =
                        candidates.iter().map(|c| c.display().to_string()).collect();
                    diag = diag.with_note(
                        NoteKind::Reason,
                        lstr!(en: "searched: {}", tried.join(", ");
                              tr: "aranan yollar: {}", tried.join(", ")),
                    );
                    if let Some(stop) = &self.search_stop {
                        diag = diag.with_note(NoteKind::Reason, no_manifest_note(stop));
                    }
                    self.diagnostics.push(diag);
                }
            }
        }
        Ok(())
    }
}

/// Manifest bulunamayınca E1011'e eklenen not: aramanın nerede durduğu
/// (ADR-0061). Tavan sessizdir; yalnız manifest'in fark yarattığı bu
/// tanıda söylenir.
fn no_manifest_note(stop: &SearchStop) -> String {
    match stop {
        SearchStop::Override(dir) => {
            let dir = dir.display();
            lstr!(en: "VOLT_MANIFEST_DIR points to '{dir}', which has no Volt.toml — only the file's own directory was searched";
                  tr: "VOLT_MANIFEST_DIR '{dir}' dizinini gösteriyor, orada Volt.toml yok — yalnız dosyanın kendi dizini arandı")
        }
        SearchStop::GitRoot(dir) => {
            let dir = dir.display();
            lstr!(en: "no Volt.toml found up to the git root '{dir}' (the search stops there; set VOLT_MANIFEST_DIR to use another manifest) — only the file's own directory was searched";
                  tr: "git kökü '{dir}' dizinine kadar Volt.toml yok (arama orada durur; başka bir manifest için VOLT_MANIFEST_DIR) — yalnız dosyanın kendi dizini arandı")
        }
        SearchStop::Home(dir) => {
            let dir = dir.display();
            lstr!(en: "no Volt.toml found below the home directory '{dir}' (a Volt.toml in the home directory itself is not a project root; set VOLT_MANIFEST_DIR to use it) — only the file's own directory was searched";
                  tr: "ev dizini '{dir}' altında Volt.toml yok (ev dizininin kendi Volt.toml'u proje kökü sayılmaz; kullanmak için VOLT_MANIFEST_DIR) — yalnız dosyanın kendi dizini arandı")
        }
        SearchStop::Exhausted => {
            lstr!(en: "no Volt.toml found above this file — only the file's own directory was searched";
                  tr: "bu dosyanın üstünde Volt.toml yok — yalnız dosyanın kendi dizini arandı")
        }
    }
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn cyclic_import(
    target: &UseTarget,
    map: &SourceMap,
    stack: &[FileId],
    back_to: FileId,
) -> Diagnostic {
    let start = stack.iter().position(|&f| f == back_to).unwrap_or(0);
    let chain: Vec<String> = stack[start..]
        .iter()
        .chain(std::iter::once(&back_to))
        .map(|&f| map.file(f).path.display().to_string())
        .collect();
    Diagnostic::error(
        ErrorCode::E1006,
        lstr!(en: "cyclic import: '{}'", target.package.join("::");
              tr: "döngüsel import: '{}'", target.package.join("::")),
        LabeledSpan::primary(
            target.span,
            lstr!(en: "this import closes the cycle"; tr: "bu import döngüyü kapatıyor"),
        ),
        lstr!(en: "move the shared items into a third file that both import";
              tr: "ortak öğeleri her ikisinin de import ettiği üçüncü bir dosyaya taşıyın"),
    )
    .with_note(
        NoteKind::Reason,
        lstr!(en: "import chain: {}", chain.join(" -> ");
              tr: "import zinciri: {}", chain.join(" -> ")),
    )
}

/// Parser'ın ürettiği sentetik kaynakları (ADR-0044 `@mmio`) haritaya
/// kaydeder. Parser kimlikleri birimdeki dosyaların ardından sırayla
/// verdiğinden `add_file` aynı kimliği döndürmelidir; döndürmezse
/// üretilen koda düşen tanılar yanlış metin gösterirdi — bu yüzden
/// sıra bozulursa panik yerine boş metinle doldurulur.
pub fn register_generated(map: &mut SourceMap, generated: &[volt_syntax::GeneratedSource]) {
    for g in generated {
        while map.len() < g.file.0 as usize {
            map.add_file("<volt-internal>", String::new());
        }
        if map.len() == g.file.0 as usize {
            map.add_file(&g.name, g.text.clone());
        }
    }
}

/// Ana dosyadan başlayarak birimi yükler. G/Ç hatası (ana dosya ya da
/// bulunan bir bağımlılık okunamadı) `Err` döner — çıkış kodu 3.
pub fn load_unit(main: &Path) -> std::io::Result<LoadedUnit> {
    let dir = main
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let (manifest, search_stop) = match Manifest::lookup(&dir) {
        Ok(m) => (Some(m), None),
        Err(stop) => (None, Some(stop)),
    };
    let mut loader = Loader {
        map: SourceMap::new(),
        manifest,
        search_stop,
        seen: HashMap::new(),
        stack: Vec::new(),
        order: Vec::new(),
        texts: HashMap::new(),
        info: UnitInfo::default(),
        diagnostics: Vec::new(),
        missing: HashSet::new(),
    };
    loader.visit(main, None)?;

    let sources: Vec<(FileId, &str)> = loader
        .order
        .iter()
        .map(|(fid, _)| (*fid, loader.texts[fid].as_str()))
        .collect();
    let parsed = volt_syntax::parse_unit(&sources);
    register_generated(&mut loader.map, &parsed.generated);
    Ok(LoadedUnit {
        map: loader.map,
        manifest: loader.manifest,
        files: loader.order,
        info: loader.info,
        diagnostics: loader.diagnostics,
        parsed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parse_reads_package_name_and_src() {
        let m = Manifest::parse(
            PathBuf::from("/p"),
            "[package]\nname = \"soc\"   # yorum\nsrc = \"rtl\"\n[ui]\nlang = \"tr\"\n",
        );
        assert_eq!(m.name.as_deref(), Some("soc"));
        assert_eq!(m.src, PathBuf::from("rtl"));
        assert_eq!(m.src_dir(), PathBuf::from("/p").join("rtl"));
    }

    #[test]
    fn manifest_parse_defaults_src_to_src() {
        let m = Manifest::parse(PathBuf::from("/p"), "[ui]\nlang = \"en\"\n");
        assert_eq!(m.name, None);
        assert_eq!(m.src, PathBuf::from("src"));
    }

    #[test]
    fn manifest_parse_reads_lint_unenforced_attributes() {
        let m = Manifest::parse(
            PathBuf::from("/p"),
            "[package]\nname = \"p\"\n\n[lint]\nunenforced_attributes = \"allow\"  # ADR-0048\n",
        );
        assert_eq!(m.lint_unenforced, UnenforcedLint::Allow);
    }

    #[test]
    fn manifest_lint_defaults_to_warn_and_ignores_unknown_value() {
        let none = Manifest::parse(PathBuf::from("/p"), "[package]\nname = \"p\"\n");
        assert_eq!(none.lint_unenforced, UnenforcedLint::Warn);
        let bogus = Manifest::parse(
            PathBuf::from("/p"),
            "[lint]\nunenforced_attributes = \"maybe\"\n",
        );
        assert_eq!(bogus.lint_unenforced, UnenforcedLint::Warn);
    }

    fn ui(dir: &str, entry: &str) -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/ui/multifile"
        ))
        .join(dir)
        .join(entry)
    }

    fn codes(unit: &LoadedUnit) -> Vec<&'static str> {
        unit.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn load_unit_finds_sibling_package_and_orders_dependencies_first() {
        let unit = load_unit(&ui("basic", "main.volt")).expect("okunmalı");
        assert!(codes(&unit).is_empty(), "{:?}", codes(&unit));
        let names: Vec<String> = unit
            .files
            .iter()
            .map(|(_, p)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["lib.volt", "main.volt"]);
        assert!(unit.info.files.contains_key(&vec!["lib".to_string()]));
        assert_eq!(unit.parsed.ast.items.len(), 3);
    }

    #[test]
    fn load_unit_missing_package_is_e1011_with_searched_paths() {
        let unit = load_unit(&ui("notfound", "main.volt")).expect("okunmalı");
        assert_eq!(codes(&unit), ["E1011"]);
        let text = format!("{:?}", unit.diagnostics[0]);
        assert!(text.contains("nowhere.volt"), "{text}");
        assert_eq!(unit.files.len(), 1);
    }

    #[test]
    fn load_unit_cycle_is_e1006_and_still_loads_both_files() {
        let unit = load_unit(&ui("cyclic", "a.volt")).expect("okunmalı");
        assert_eq!(codes(&unit), ["E1006"]);
        assert_eq!(unit.files.len(), 2);
        let text = format!("{:?}", unit.diagnostics[0]);
        assert!(text.contains("a.volt") && text.contains("b.volt"), "{text}");
    }

    #[test]
    fn load_unit_declared_package_is_registered_alongside_reached_path() {
        // cyclic/b.volt declares `package b;` and is reached as `b`.
        let unit = load_unit(&ui("cyclic", "a.volt")).expect("okunmalı");
        let b = unit.info.files[&vec!["b".to_string()]];
        assert_eq!(unit.info.packages[&b], vec!["b".to_string()]);
    }

    #[test]
    fn load_unit_missing_main_file_is_io_error() {
        assert!(load_unit(Path::new("definitely/not/here.volt")).is_err());
    }

    #[test]
    fn manifest_discover_walks_up_to_volt_toml() {
        let soc = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/soc"));
        let m = Manifest::discover(&soc).expect("examples/Volt.toml");
        assert_eq!(m.name.as_deref(), Some("examples"));
        assert_eq!(m.src, PathBuf::from("."));
    }

    #[test]
    fn use_targets_cover_plain_alias_glob_and_list() {
        let src = "use a::b::X;\nuse a::c::Y as Z;\nuse a::d::*;\nuse a::{e::P, Q};\nuse std::fifo::F;\nuse solo;\n";
        let parsed = volt_syntax::parse(FileId(0), src);
        let pkgs: Vec<String> = use_targets(&parsed.ast)
            .iter()
            .map(|t| t.package.join("::"))
            .collect();
        assert_eq!(pkgs, ["a::b", "a::c", "a::d", "a::e", "a"]);
    }
}
