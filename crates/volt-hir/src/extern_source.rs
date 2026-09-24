//! `extern module`'ün SystemVerilog kaynağı (ADR-0076).
//!
//! `@source("rtl/foo.sv")` niteliği bir `extern module`'ün gövdesinin
//! hangi SV dosyalarında olduğunu söyler; `volt run`/`test`/`verify`
//! bu dosyaları Verilator'a ve sby'ye üretilen SV ile birlikte verir.
//! Yol, niteliği taşıyan `.volt` dosyasına göre göreli çözülür ve proje
//! kökünün (en yakın Volt.toml — ADR-0061 tavanıyla — yoksa o dosyanın
//! dizini) dışına çıkamaz: `read_hex`'in yol kuralının aynısı (ADR-0058,
//! [`normalize_data_path`]).
//!
//! Üç denetim katmanı:
//! - biçim (her ortam, LSP dahil): nitelik yalnız `extern module`'de,
//!   argümanları bir ya da daha çok string literal → E0009;
//! - dosya (yalnız dosya sistemine erişen sürücü): yol projeden
//!   çıkıyor ya da dosya yok → E1012;
//! - kullanım (yalnız `run`/`test`/`verify`): örneklenen extern'ün
//!   `@source`'u yoksa → E1012. `build`/`check` extern'ü örnekleme
//!   olarak üretir, gövdesini istemez.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use volt_ast::{AttrArg, Attribute, ExprKind, ItemKind, SourceFile, StmtKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::{FileId, Span};

use crate::testdata::{normalize_data_path, TestFileError};
use crate::unit_load::Manifest;

/// Nitelik adı: `@source("...")`.
pub const SOURCE_ATTRIBUTE: &str = "source";

/// Bir `extern module`'ün bildirdiği SV kaynakları (yazıldığı gibi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternSourceDecl {
    pub module: String,
    /// Extern adının span'i (kaynaksız kullanım tanısı buraya işaret eder).
    pub name_span: Span,
    /// (göreli yol, string literal span'i), yazım sırasıyla.
    pub paths: Vec<(String, Span)>,
}

/// Çözülmüş kaynak: gerçek (kanonik) dosya yolu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternSourceFile {
    pub module: String,
    pub path: PathBuf,
}

/// Yolu, niteliği taşıyan dosyaya göre dosya sisteminde çözer.
pub trait SourceLocator {
    fn locate(&self, file: FileId, rel_path: &str) -> Result<PathBuf, TestFileError>;
}

// ═══ Biçim denetimi (E0009) ════════════════════════════════════════

/// `@source` biçim denetimi: yalnız `extern module` öğesinde, en az bir
/// string literal argüman. LSP ve sürücü aynı tanıyı verir.
pub fn check_source_attributes(ast: &SourceFile) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for &item_idx in &ast.items {
        let item = &ast.items_arena[item_idx];
        let is_extern = matches!(item.kind, ItemKind::Extern(_));
        for attr in item
            .attrs
            .iter()
            .filter(|a| a.name.text == SOURCE_ATTRIBUTE)
        {
            if is_extern {
                if string_args(ast, attr).is_none() {
                    out.push(bad_arguments(attr));
                }
            } else {
                out.push(misplaced(attr));
            }
        }
        let nested: Vec<&[Attribute]> = match &item.kind {
            ItemKind::Module(m) => m
                .ports
                .iter()
                .map(|p| p.attrs.as_slice())
                .chain(m.body.iter().map(|&s| ast.stmts[s].attrs.as_slice()))
                .collect(),
            ItemKind::Extern(x) => x.ports.iter().map(|p| p.attrs.as_slice()).collect(),
            ItemKind::Struct(s) => s.fields.iter().map(|f| f.attrs.as_slice()).collect(),
            _ => Vec::new(),
        };
        for attrs in nested {
            for attr in attrs.iter().filter(|a| a.name.text == SOURCE_ATTRIBUTE) {
                out.push(misplaced(attr));
            }
        }
    }
    out
}

/// Argümanlar yalnız string literal ise (en az bir) (değer, span) listesi.
fn string_args(ast: &SourceFile, attr: &Attribute) -> Option<Vec<(String, Span)>> {
    if attr.args.is_empty() {
        return None;
    }
    attr.args
        .iter()
        .map(|arg| match arg {
            AttrArg::Positional(e) => match &ast.exprs[*e].kind {
                ExprKind::StringLit(s) => Some((s.clone(), ast.exprs[*e].span)),
                _ => None,
            },
            AttrArg::Named { .. } => None,
        })
        .collect()
}

fn bad_arguments(attr: &Attribute) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E0009,
        lstr!(en: "'@source' takes one or more string literal paths"; tr: "'@source' bir ya da daha çok string literal yol alır"),
        LabeledSpan::primary(attr.span, lstr!(en: "expected '@source(\"rtl/foo.sv\")'"; tr: "'@source(\"rtl/foo.sv\")' bekleniyor")),
        lstr!(en: "write the SystemVerilog file path relative to this .volt file: @source(\"rtl/foo.sv\")"; tr: "SystemVerilog dosya yolunu bu .volt dosyasına göre yazın: @source(\"rtl/foo.sv\")"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "several files are allowed: @source(\"rtl/fifo.sv\", \"rtl/fifo_mem.sv\") (ADR-0076)"; tr: "birden çok dosya olabilir: @source(\"rtl/fifo.sv\", \"rtl/fifo_mem.sv\") (ADR-0076)"),
    )
}

fn misplaced(attr: &Attribute) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E0009,
        lstr!(en: "'@source' applies only to an 'extern module'"; tr: "'@source' yalnız 'extern module' için geçerlidir"),
        LabeledSpan::primary(attr.span, lstr!(en: "not an extern module"; tr: "extern module değil"),),
        lstr!(en: "remove it — a Volt module's SystemVerilog is generated by the compiler"; tr: "kaldırın — Volt modülünün SystemVerilog'u derleyici tarafından üretilir"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "'@source' tells 'volt run', 'test' and 'verify' where an extern module's SystemVerilog lives (ADR-0076)"; tr: "'@source', 'volt run', 'test' ve 'verify'a bir extern modülün SystemVerilog'unun nerede olduğunu söyler (ADR-0076)"),
    )
}

// ═══ Bildirimler ve kullanım ═══════════════════════════════════════

/// Birimdeki `@source`'lu ve `@source`'suz bütün extern'ler (biçimi
/// bozuk nitelik yok sayılır — E0009 zaten verildi).
pub fn extern_source_decls(ast: &SourceFile) -> Vec<ExternSourceDecl> {
    ast.items
        .iter()
        .filter_map(|&i| {
            let item = &ast.items_arena[i];
            let ItemKind::Extern(x) = &item.kind else {
                return None;
            };
            let paths = item
                .attrs
                .iter()
                .filter(|a| a.name.text == SOURCE_ATTRIBUTE)
                .filter_map(|a| string_args(ast, a))
                .flatten()
                .collect();
            Some(ExternSourceDecl {
                module: x.name.text.clone(),
                name_span: x.name.span,
                paths,
            })
        })
        .collect()
}

/// Modül gövdelerinde örneklenen extern adları (bildirim sırasında).
pub fn instantiated_externs(ast: &SourceFile) -> Vec<String> {
    let externs: Vec<String> = extern_source_decls(ast)
        .into_iter()
        .map(|d| d.module)
        .collect();
    let mut used: Vec<String> = Vec::new();
    for &i in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[i].kind else {
            continue;
        };
        for &s in &m.body {
            let StmtKind::Instance(inst) = &ast.stmts[s].kind else {
                continue;
            };
            let Some(last) = inst.module_path.segments.last() else {
                continue;
            };
            if externs.contains(&last.text) && !used.contains(&last.text) {
                used.push(last.text.clone());
            }
        }
    }
    externs.into_iter().filter(|e| used.contains(e)).collect()
}

// ═══ Dosya denetimi (E1012) ════════════════════════════════════════

/// Her `@source` yolunu çözer; projeden çıkan ya da bulunamayan yol E1012.
/// Aynı dosyayı gösteren yollar tek kez döner (ilk geçiş sırası).
pub fn resolve_extern_sources(
    ast: &SourceFile,
    locator: &dyn SourceLocator,
) -> (Vec<ExternSourceFile>, Vec<Diagnostic>) {
    let mut files: Vec<ExternSourceFile> = Vec::new();
    let mut diags = Vec::new();
    for decl in extern_source_decls(ast) {
        for (rel, span) in &decl.paths {
            match locator.locate(span.file, rel) {
                Ok(path) => {
                    if !files.iter().any(|f| f.path == path) {
                        files.push(ExternSourceFile {
                            module: decl.module.clone(),
                            path,
                        });
                    }
                }
                Err(err) => diags.push(source_error(&decl.module, rel, *span, &err)),
            }
        }
    }
    (files, diags)
}

fn source_error(module: &str, rel: &str, span: Span, err: &TestFileError) -> Diagnostic {
    match err {
        TestFileError::OutsideProject => Diagnostic::error(
            ErrorCode::E1012,
            lstr!(en: "SystemVerilog source '{rel}' of extern module '{module}' leaves the project"; tr: "'{module}' extern modülünün SystemVerilog kaynağı '{rel}' projenin dışına çıkıyor"),
            LabeledSpan::primary(span, lstr!(en: "outside the project directory"; tr: "proje dizininin dışında")),
            lstr!(en: "use a path relative to this .volt file that stays inside the project (no absolute paths, no '..' past the root); copy vendor files into the project"; tr: "bu .volt dosyasına göre göreli ve proje içinde kalan bir yol kullanın (mutlak yol yok, kökü aşan '..' yok); üretici dosyalarını projeye kopyalayın"),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "the project root is the directory of the nearest Volt.toml, or this file's directory when there is none (ADR-0061, ADR-0076)"; tr: "proje kökü en yakın Volt.toml'un dizinidir, yoksa bu dosyanın dizini (ADR-0061, ADR-0076)"),
        ),
        TestFileError::NotFound(os) => Diagnostic::error(
            ErrorCode::E1012,
            lstr!(en: "cannot read SystemVerilog source '{rel}' of extern module '{module}': {os}"; tr: "'{module}' extern modülünün SystemVerilog kaynağı '{rel}' okunamadı: {os}"),
            LabeledSpan::primary(span, lstr!(en: "no such file"; tr: "böyle bir dosya yok")),
            lstr!(en: "the path is resolved relative to the directory of this .volt file"; tr: "yol, bu .volt dosyasının dizinine göre çözülür"),
        ),
    }
}

/// `run`/`test`/`verify`: örneklenen ama `@source`'u olmayan extern'ler
/// için E1012 — simülatör ve çözücü modülün gövdesini bilmeden koşamaz.
pub fn missing_sources(ast: &SourceFile, command: &str) -> Vec<Diagnostic> {
    let used = instantiated_externs(ast);
    extern_source_decls(ast)
        .into_iter()
        .filter(|d| d.paths.is_empty() && used.contains(&d.module))
        .map(|d| {
            let module = &d.module;
            Diagnostic::error(
                ErrorCode::E1012,
                lstr!(en: "extern module '{module}' has no SystemVerilog source; 'volt {command}' needs its body"; tr: "'{module}' extern modülünün SystemVerilog kaynağı yok; 'volt {command}' gövdesine ihtiyaç duyar"),
                LabeledSpan::primary(d.name_span, lstr!(en: "instantiated, but its SystemVerilog is unknown"; tr: "örnekleniyor ama SystemVerilog'u bilinmiyor")),
                lstr!(en: "name the file that defines it: @source(\"rtl/{module}.sv\") extern module {module} {{ ... }}"; tr: "onu tanımlayan dosyayı yazın: @source(\"rtl/{module}.sv\") extern module {module} {{ ... }}"),
            )
            .with_note(
                NoteKind::Note,
                lstr!(en: "'volt build' and 'volt check' do not need it: they emit the instantiation only (ADR-0071, ADR-0076)"; tr: "'volt build' ve 'volt check' buna ihtiyaç duymaz: yalnız örneklemeyi üretirler (ADR-0071, ADR-0076)"),
            )
        })
        .collect()
}

// ═══ Dosya sistemi yer bulucusu ════════════════════════════════════

/// Proje içi yol çözümü: `base_dir`'e göre göreli yol, kök `root`.
/// `read_hex` (sürücü `FsTestFiles`) ve `@source` ortak kuralı.
pub fn resolve_in_project(
    base_dir: &Path,
    root: &Path,
    rel_path: &str,
) -> Result<PathBuf, TestFileError> {
    let depth = base_dir
        .strip_prefix(root)
        .map_or(0, |rel| rel.components().count());
    let parts = normalize_data_path(rel_path, depth).ok_or(TestFileError::OutsideProject)?;
    let path = parts
        .iter()
        .fold(base_dir.to_path_buf(), |acc, part| acc.join(part));
    // Sembolik bağ sözcüksel kuralı dolanmasın: gerçek yol da kökün
    // altında kalmalı.
    let real = path
        .canonicalize()
        .map_err(|err| TestFileError::NotFound(err.to_string()))?;
    if !real.starts_with(root) {
        return Err(TestFileError::OutsideProject);
    }
    if !real.is_file() {
        return Err(TestFileError::NotFound(
            lstr!(en: "not a file"; tr: "dosya değil"),
        ));
    }
    Ok(real)
}

/// `base_dir` için proje kökü: en yakın Volt.toml (base_dir onun altında
/// ise), yoksa `base_dir`'in kendisi. İkisi de kanonik.
pub fn project_root(base_dir: &Path) -> (PathBuf, PathBuf) {
    let base = base_dir
        .canonicalize()
        .unwrap_or_else(|_| base_dir.to_path_buf());
    let root = Manifest::discover(&base)
        .map(|m| m.root.canonicalize().unwrap_or(m.root))
        .filter(|root| base.starts_with(root))
        .unwrap_or_else(|| base.clone());
    (base, root)
}

/// Birim dosyalarına göre çözen yer bulucu (sürücü ve LSP).
pub struct FsSourceLocator {
    dirs: HashMap<FileId, PathBuf>,
}

impl FsSourceLocator {
    /// `files`: birim dosyaları (`LoadedUnit::files`).
    pub fn new(files: &[(FileId, PathBuf)]) -> Self {
        let dirs = files
            .iter()
            .map(|(fid, path)| {
                let dir = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
                (*fid, dir)
            })
            .collect();
        Self { dirs }
    }
}

impl SourceLocator for FsSourceLocator {
    fn locate(&self, file: FileId, rel_path: &str) -> Result<PathBuf, TestFileError> {
        let dir = self
            .dirs
            .get(&file)
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        let (base, root) = project_root(&dir);
        resolve_in_project(&base, &root, rel_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> SourceFile {
        volt_syntax::parse(FileId(0), src).ast
    }

    const EXT: &str =
        "extern module Foo {\n    in  clk : clock\n    in  d   : u8\n    out q   : u8\n}\n";

    #[test]
    fn source_on_extern_with_string_args_is_clean() {
        let ast = parse(&format!(
            "@source(\"rtl/foo.sv\", \"rtl/foo_pkg.sv\")\n{EXT}"
        ));
        assert!(check_source_attributes(&ast).is_empty());
        let decls = extern_source_decls(&ast);
        assert_eq!(decls.len(), 1);
        let paths: Vec<&str> = decls[0].paths.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths, ["rtl/foo.sv", "rtl/foo_pkg.sv"]);
    }

    #[test]
    fn source_without_string_args_is_e0009() {
        for attr in [
            "@source",
            "@source()",
            "@source(foo)",
            "@source(path = \"x.sv\")",
        ] {
            let ast = parse(&format!("{attr}\n{EXT}"));
            let d = check_source_attributes(&ast);
            assert_eq!(d.len(), 1, "{attr}: {d:?}");
            assert_eq!(d[0].code.as_str(), "E0009", "{attr}");
        }
    }

    #[test]
    fn source_on_a_volt_module_is_e0009() {
        let ast = parse(
            "@source(\"m.sv\")\nmodule M {\n    in a : bool\n    out y : bool\n    y = a\n}\n",
        );
        let d = check_source_attributes(&ast);
        assert_eq!(d.len(), 1);
        assert!(
            d[0].message.contains("only to an 'extern module'"),
            "{}",
            d[0].message
        );
    }

    #[test]
    fn source_on_a_port_or_statement_is_e0009() {
        let port = parse(
            "module M {\n    @source(\"p.sv\")\n    in a : bool\n    out y : bool\n    y = a\n}\n",
        );
        let d = check_source_attributes(&port);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code.as_str(), "E0009");
        let ext_port = parse("extern module X {\n    @source(\"p.sv\")\n    in a : bool\n}\n");
        assert_eq!(check_source_attributes(&ext_port).len(), 1);
    }

    #[test]
    fn missing_source_only_for_instantiated_externs() {
        let used = format!("{EXT}module Top {{\n    in  clk : clock\n    out y : u8\n    let f = Foo {{ clk: clk, d: 1 }}\n    y = f.q\n}}\n");
        let ast = parse(&used);
        let d = missing_sources(&ast, "test");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code.as_str(), "E1012");
        assert!(d[0].message.contains("'volt test'"), "{}", d[0].message);
        // Örneklenmeyen extern kaynaksız da sorun değil.
        assert!(missing_sources(&parse(EXT), "test").is_empty());
        // Kaynağı olan örneklenmiş extern sorun değil.
        let with = parse(&format!("@source(\"foo.sv\")\n{used}"));
        assert!(missing_sources(&with, "verify").is_empty());
    }

    #[test]
    fn resolve_in_project_rejects_escapes_and_missing_files() {
        let dir = std::env::temp_dir().join(format!("volt-extern-src-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let inner = dir.join("proj").join("hw");
        std::fs::create_dir_all(inner.join("rtl")).unwrap();
        std::fs::write(inner.join("rtl").join("foo.sv"), "module Foo; endmodule\n").unwrap();
        std::fs::write(dir.join("outside.sv"), "").unwrap();
        let root = dir.join("proj").canonicalize().unwrap();
        let base = inner.canonicalize().unwrap();
        assert!(resolve_in_project(&base, &root, "rtl/foo.sv").is_ok());
        assert!(resolve_in_project(&base, &root, "../hw/rtl/foo.sv").is_ok());
        assert_eq!(
            resolve_in_project(&base, &root, "../../outside.sv"),
            Err(TestFileError::OutsideProject)
        );
        assert!(matches!(
            resolve_in_project(&base, &root, "rtl/nope.sv"),
            Err(TestFileError::NotFound(_))
        ));
        assert!(matches!(
            resolve_in_project(&base, &root, "rtl"),
            Err(TestFileError::NotFound(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
