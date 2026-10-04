//! Proje kipi (ADR-0095 §2): dosya argümanı verilmeyen `check`, `build`,
//! `run` ve `verify` Volt.toml'un projesinde çalışır.
//!
//! * Proje: çalışma dizininden yukarı ilk Volt.toml (ADR-0061 araması ve
//!   tavanı). Kaynaklar `[package] src` altındaki `*.volt` dosyalarıdır;
//!   `*_test.volt` hariç, atlama kuralları test keşfiyle aynı (ADR-0089).
//! * Üst modül: `[package] top` yazılmışsa o (tek ad ya da liste); yoksa
//!   projede HİÇ örneklenmemiş, generic olmayan modül. Birden fazla aday
//!   kullanım hatasıdır ve adaylar listelenir — sessiz seçim yok.
//! * Dosya argümanı verilen her çağrı eskisi gibidir; bu modül yalnız
//!   argümansız çağrıda devreye girer.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

use volt_ast::{ItemKind, SourceFile, StmtKind};
use volt_diagnostics::lstr;
use volt_hir::unit_load::Manifest;
use volt_span::FileId;

/// Argümansız çağrı mı? "Next:" satırları dosya adı yazmaz.
static PROJECT_MODE: AtomicBool = AtomicBool::new(false);

pub(crate) fn is_project_mode() -> bool {
    PROJECT_MODE.load(Ordering::Relaxed)
}

/// Yüklenmiş proje.
pub(crate) struct Project {
    pub manifest: Manifest,
    /// Tasarım kaynakları (test dosyaları hariç), çalışma dizinine göreli.
    pub sources: Vec<PathBuf>,
}

/// Üst modül ve onu tanımlayan dosya.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Top {
    pub module: String,
    pub file: PathBuf,
}

/// Argümansız `command` için projeyi yükler. Volt.toml yoksa kullanım
/// hatası (çıkış kodu 2) ve iki yol: dosya adı ya da `volt new`.
pub(crate) fn load(command: &str) -> Result<Project, ExitCode> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let Some(manifest) = Manifest::discover(&cwd) else {
        print_no_project(command);
        return Err(ExitCode::from(2));
    };
    PROJECT_MODE.store(true, Ordering::Relaxed);
    let src = manifest.src.clone();
    let sources: Vec<PathBuf> = crate::sim::project_files(&manifest, &cwd, &[src], &|p| {
        p.extension().is_some_and(|e| e == "volt") && !crate::sim::is_test_file(p)
    });
    if sources.is_empty() {
        // Yanlış yazılmış `src` (ör. `scr`) burada düşer: Volt.toml uyarıları
        // birim yüklenmeden önce basılır ki neden görünsün (ADR-0099).
        let mut map = volt_span::SourceMap::new();
        for d in volt_hir::unit_load::manifest_warnings_into(&manifest.root, &mut map) {
            eprintln!("{}", volt_diagnostics::render_human(&d, &map));
        }
        let dir = manifest.src_dir();
        usage_error(
            &lstr!(
                en: "no .volt source files in '{}' (the `src` of {})", dir.display(), manifest_path(&manifest).display();
                tr: "'{}' içinde .volt kaynak dosyası yok ({} dosyasının `src` dizini)", dir.display(), manifest_path(&manifest).display()
            ),
            &[lstr!(
                en: "set `src` in [package] of Volt.toml, or give a file: volt {command} design.volt";
                tr: "Volt.toml [package] bölümünde `src`'yi ayarlayın ya da dosya verin: volt {command} tasarim.volt"
            )],
        );
        return Err(ExitCode::from(2));
    }
    Ok(Project { manifest, sources })
}

fn manifest_path(m: &Manifest) -> PathBuf {
    m.root.join(volt_hir::MANIFEST_FILE)
}

fn print_no_project(command: &str) {
    usage_error(
        &lstr!(
            en: "'volt {command}' needs a file or a project, and no Volt.toml was found here or above";
            tr: "'volt {command}' bir dosya ya da proje ister; burada ve üst dizinlerde Volt.toml yok"
        ),
        &[
            lstr!(
                en: "give a file: volt {command} design.volt";
                tr: "dosya verin: volt {command} tasarim.volt"
            ),
            lstr!(
                en: "or create a project: volt new <name> (then run 'volt {command}' inside it)";
                tr: "ya da proje oluşturun: volt new <ad> (sonra içinde 'volt {command}')"
            ),
        ],
    );
}

/// UX Anayasası biçiminde kullanım hatası: `error:` satırı + `= help:`.
fn usage_error(message: &str, helps: &[String]) {
    eprintln!("{}", lstr!(en: "error: {message}"; tr: "hata: {message}"));
    for help in helps {
        eprintln!("{}", lstr!(en: "  = help: {help}"; tr: "  = çözüm: {help}"));
    }
}

/// Bir kaynağın modülleri (generic olmayanlar) ve örneklediği modül adları.
struct FileModules {
    file: PathBuf,
    modules: Vec<String>,
    instantiated: Vec<String>,
}

fn scan(file: &Path) -> FileModules {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let ast = volt_syntax::parse(FileId(0), &text).ast;
    let (modules, instantiated) = modules_and_instances(&ast);
    FileModules {
        file: file.to_path_buf(),
        modules,
        instantiated,
    }
}

/// Emitter'ın örnekleme kuralı (`reach.rs`): modül gövdesinin üst düzey
/// `Instance` deyimleri; hedef, yolun son parçası.
fn modules_and_instances(ast: &SourceFile) -> (Vec<String>, Vec<String>) {
    let mut modules = Vec::new();
    let mut instantiated = Vec::new();
    for &i in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[i].kind else {
            continue;
        };
        if m.generics.is_empty() {
            modules.push(m.name.text.clone());
        }
        for &s in &m.body {
            if let StmtKind::Instance(inst) = &ast.stmts[s].kind {
                if let Some(last) = inst.module_path.segments.last() {
                    instantiated.push(last.text.clone());
                }
            }
        }
    }
    (modules, instantiated)
}

/// Projenin üst modül(ler)i, `[package] top` sırasıyla ya da çıkarımla.
/// Aday yoksa ya da birden fazlaysa (çıkarımda) kullanım hatası.
pub(crate) fn tops(project: &Project, command: &str) -> Result<Vec<Top>, ExitCode> {
    let scanned: Vec<FileModules> = project.sources.iter().map(|f| scan(f)).collect();
    let declared = |name: &str| -> Vec<&Path> {
        scanned
            .iter()
            .filter(|f| f.modules.iter().any(|m| m == name))
            .map(|f| f.file.as_path())
            .collect()
    };
    let names: Vec<String> = match &project.manifest.top {
        Some(list) if !list.is_empty() => list.clone(),
        _ => infer(&scanned, project, command)?,
    };
    let mut tops = Vec::new();
    for name in names {
        match declared(&name).as_slice() {
            [file] => tops.push(Top {
                module: name,
                file: file.to_path_buf(),
            }),
            [] => {
                let all: BTreeSet<&str> = scanned
                    .iter()
                    .flat_map(|f| f.modules.iter().map(String::as_str))
                    .collect();
                let all: Vec<&str> = all.into_iter().collect();
                usage_error(
                    &lstr!(
                        en: "top module '{name}' (Volt.toml `top`) is not declared in the project's sources";
                        tr: "üst modül '{name}' (Volt.toml `top`) projenin kaynaklarında tanımlı değil"
                    ),
                    &[lstr!(
                        en: "modules in the project: {}", all.join(", ");
                        tr: "projedeki modüller: {}", all.join(", ")
                    )],
                );
                return Err(ExitCode::from(2));
            }
            files => {
                let files: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
                usage_error(
                    &lstr!(
                        en: "top module '{name}' is declared in more than one file: {}", files.join(", ");
                        tr: "üst modül '{name}' birden fazla dosyada tanımlı: {}", files.join(", ")
                    ),
                    &[lstr!(
                        en: "rename one of them, or give a file: volt {command} {}", files[0];
                        tr: "birini yeniden adlandırın ya da dosya verin: volt {command} {}", files[0]
                    )],
                );
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(tops)
}

/// Çıkarım: projede hiç örneklenmemiş modüller. Tam bir aday olmalı.
fn infer(
    scanned: &[FileModules],
    project: &Project,
    command: &str,
) -> Result<Vec<String>, ExitCode> {
    let used: BTreeSet<&str> = scanned
        .iter()
        .flat_map(|f| f.instantiated.iter().map(String::as_str))
        .collect();
    let mut candidates: Vec<(&str, &Path)> = Vec::new();
    for f in scanned {
        for m in &f.modules {
            if !used.contains(m.as_str()) {
                candidates.push((m.as_str(), f.file.as_path()));
            }
        }
    }
    let manifest = manifest_path(&project.manifest);
    match candidates.as_slice() {
        [(only, _)] => Ok(vec![(*only).to_string()]),
        [] => {
            usage_error(
                &lstr!(
                    en: "no top module in the project: every module is instantiated by another one, or there is none";
                    tr: "projede üst modül yok: her modül başka biri tarafından örneklenmiş ya da hiç modül yok"
                ),
                &[lstr!(
                    en: "name it in [package] of {}: top = \"MyTop\"", manifest.display();
                    tr: "{} dosyasının [package] bölümünde adlandırın: top = \"UstModul\"", manifest.display()
                )],
            );
            Err(ExitCode::from(2))
        }
        many => {
            let list: Vec<String> = many
                .iter()
                .map(|(m, f)| format!("{m} ({})", f.display()))
                .collect();
            usage_error(
                &lstr!(
                    en: "the project has {} top-module candidates (modules no other module instantiates): {}", many.len(), list.join(", ");
                    tr: "projede {} üst modül adayı var (başka modülün örneklemediği modüller): {}", many.len(), list.join(", ")
                ),
                &[
                    lstr!(
                        en: "choose in [package] of {}: top = \"{}\" (or a list: top = [\"A\", \"B\"])", manifest.display(), many[0].0;
                        tr: "{} dosyasının [package] bölümünde seçin: top = \"{}\" (ya da liste: top = [\"A\", \"B\"])", manifest.display(), many[0].0
                    ),
                    lstr!(
                        en: "or give a file: volt {command} {}", many[0].1.display();
                        tr: "ya da dosya verin: volt {command} {}", many[0].1.display()
                    ),
                ],
            );
            Err(ExitCode::from(2))
        }
    }
}

/// `run`/`verify` tek üst modül ister. `--top` (yalnız `run`) verilmişse
/// projenin kaynaklarında aranır.
pub(crate) fn single_top(
    project: &Project,
    command: &str,
    top_flag: Option<&str>,
) -> Result<Top, ExitCode> {
    if let Some(name) = top_flag {
        let found = project
            .sources
            .iter()
            .find(|f| scan(f).modules.iter().any(|m| m == name));
        return match found {
            Some(file) => Ok(Top {
                module: name.to_string(),
                file: file.clone(),
            }),
            None => {
                usage_error(
                    &lstr!(
                        en: "no module named '{name}' in the project's sources";
                        tr: "projenin kaynaklarında '{name}' adlı modül yok"
                    ),
                    &[],
                );
                Err(ExitCode::from(2))
            }
        };
    }
    let mut tops = tops(project, command)?;
    if tops.len() == 1 {
        return Ok(tops.remove(0));
    }
    let names: Vec<&str> = tops.iter().map(|t| t.module.as_str()).collect();
    let pick = if command == "run" {
        lstr!(
            en: "pick one: volt run --top {}", names[0];
            tr: "birini seçin: volt run --top {}", names[0]
        )
    } else {
        lstr!(
            en: "give the file of one: volt {command} {}", tops[0].file.display();
            tr: "birinin dosyasını verin: volt {command} {}", tops[0].file.display()
        )
    };
    usage_error(
        &lstr!(
            en: "Volt.toml names {} top modules ({}); 'volt {command}' works on one", names.len(), names.join(", ");
            tr: "Volt.toml {} üst modül adlandırıyor ({}); 'volt {command}' birinde çalışır", names.len(), names.join(", ")
        ),
        &[pick],
    );
    Err(ExitCode::from(2))
}

/// `volt check`'in denetleyeceği kök dosyalar: başka bir kaynağın `use`
/// ile yüklemediği kaynaklar (yüklenen dosya o birimde denetlenir). Hiçbir
/// kökün birimine girmeyen kalan dosyalar (yalnız birbirini yükleyen
/// döngü) de eklenir — her kaynak en az bir kez denetlenir.
pub(crate) fn check_roots(project: &Project) -> Vec<PathBuf> {
    let key = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let units: Vec<(PathBuf, Vec<PathBuf>)> = project
        .sources
        .iter()
        .map(|f| {
            let deps = volt_hir::unit_load::load_unit(f)
                .map(|u| u.files.iter().map(|(_, p)| key(p)).collect())
                .unwrap_or_default();
            (key(f), deps)
        })
        .collect();
    let imported: BTreeSet<&PathBuf> = units
        .iter()
        .flat_map(|(own, deps)| deps.iter().filter(move |d| *d != own))
        .collect();
    let mut roots: Vec<usize> = (0..units.len())
        .filter(|&i| !imported.contains(&units[i].0))
        .collect();
    let mut covered: BTreeSet<&PathBuf> = roots
        .iter()
        .flat_map(|&i| units[i].1.iter().chain(std::iter::once(&units[i].0)))
        .collect();
    for (i, (own, deps)) in units.iter().enumerate() {
        if !covered.contains(own) {
            roots.push(i);
            covered.extend(deps.iter().chain(std::iter::once(own)));
        }
    }
    roots.sort_unstable();
    roots
        .into_iter()
        .map(|i| project.sources[i].clone())
        .collect()
}

/// Üst modüllerin dosyaları, ilk görülme sırasıyla, tekil.
pub(crate) fn top_files(tops: &[Top]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    for t in tops {
        if !files.contains(&t.file) {
            files.push(t.file.clone());
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanned(files: &[(&str, &str)]) -> Vec<FileModules> {
        files
            .iter()
            .map(|(name, text)| {
                let ast = volt_syntax::parse(FileId(0), text).ast;
                let (modules, instantiated) = modules_and_instances(&ast);
                FileModules {
                    file: PathBuf::from(name),
                    modules,
                    instantiated,
                }
            })
            .collect()
    }

    const LEAF: &str = "module Leaf {\n    in clk : clock\n    out q : bool\n    q = true\n}\n";
    const TOP: &str = "module Top {\n    in clk : clock\n    out q : bool\n    let l = Leaf { clk: clk }\n    q = l.q\n}\n";

    #[test]
    fn instances_are_collected_from_module_bodies() {
        let s = scanned(&[("leaf.volt", LEAF), ("top.volt", TOP)]);
        assert_eq!(s[0].modules, ["Leaf"]);
        assert!(s[0].instantiated.is_empty());
        assert_eq!(s[1].modules, ["Top"]);
        assert_eq!(s[1].instantiated, ["Leaf"]);
    }

    #[test]
    fn generic_modules_are_never_candidates() {
        let src =
            "module Delay<const N: u32> {\n    in clk : clock\n    out q : bool\n    q = true\n}\n";
        assert!(scanned(&[("d.volt", src)])[0].modules.is_empty());
    }

    #[test]
    fn top_files_are_unique_in_first_seen_order() {
        let t = |m: &str, f: &str| Top {
            module: m.into(),
            file: PathBuf::from(f),
        };
        let files = top_files(&[t("A", "a.volt"), t("B", "b.volt"), t("C", "a.volt")]);
        assert_eq!(files, [PathBuf::from("a.volt"), PathBuf::from("b.volt")]);
    }
}
