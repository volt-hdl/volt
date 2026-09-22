//! `volt test` ön yarısı: test dosyalarının keşfi, derlenmesi, test
//! bloklarının denetimi ve (birim, DUT modülü) gruplarına indirgenmesi.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use volt_ast::{ItemKind, SourceFile, TestDecl};
use volt_diagnostics::{lstr, Diagnostic};
use volt_sv_emit::{SvaMode, TbTest};

use super::test_files::sibling_path;
use crate::sim_lower::{lower_test, FsTestFiles, LowerCtx, LoweredTest};
use crate::{compile, render_diagnostics, Compiled, OutputFormat};

/// Aynı yürütülebilirde koşan testler: (birim, DUT modülü) başına bir grup.
pub(super) struct TestGroup {
    pub module: String,
    pub unit: usize,
    pub tests: Vec<TbTest>,
    /// `load` ile yazılan bellekler: (sahip modül, yazmaç adı).
    pub load_targets: Vec<(String, String)>,
}

/// Tek test dosyasının derlenmiş hâli (+ varsa kardeşi).
pub(super) struct TestUnit {
    pub file_label: String,
    /// `read_hex` yollarının çözüldüğü test dosyası.
    path: PathBuf,
    pub compiled: Compiled,
    pub sibling: Option<Compiled>,
}

impl TestUnit {
    /// Test dosyasının AST'si + varsa kardeşininki.
    fn sources(&self) -> Vec<&SourceFile> {
        let mut sources: Vec<&SourceFile> = vec![&self.compiled.ast];
        if let Some(sib) = &self.sibling {
            sources.push(&sib.ast);
        }
        sources
    }
}

// ═══ Derleme ve denetim ═══════════════════════════════════════════

/// Test dosyasını (+ kardeşini) derler ve test bloklarını denetler.
/// `sva_mode`: kontrat izleyicileri açıksa `Simulation` (ADR-0064),
/// `--no-contracts` ile `None`.
pub(super) fn compile_unit(file: &Path, sva_mode: SvaMode) -> Result<TestUnit, ExitCode> {
    eprintln!(
        "{}",
        lstr!(
            en: "   Compiling {}", file.display();
            tr: "   Derleniyor {}", file.display()
        )
    );
    let compiled = compile(file, true, sva_mode)?;
    render_diagnostics(&compiled, OutputFormat::Human);
    let sibling = match sibling_path(file) {
        Some(sib) => {
            let c = compile(&sib, true, sva_mode)?;
            render_diagnostics(&c, OutputFormat::Human);
            Some(c)
        }
        None => None,
    };
    let unit = TestUnit {
        path: file.to_path_buf(),
        file_label: file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display().to_string()),
        compiled,
        sibling,
    };
    let errors = unit.compiled.errors()
        + unit.sibling.as_ref().map_or(0, Compiled::errors)
        + check_unit_tests(&unit);
    if errors > 0 {
        eprintln!(
            "{}",
            lstr!(
                en: "     Error: test build failed due to {errors} error(s)";
                tr: "     Hata: {errors} hata nedeniyle test derlemesi başarısız"
            )
        );
        return Err(ExitCode::from(1));
    }
    Ok(unit)
}

/// Kardeş dosya dahil TAM test denetimi (E8501-E8512): tanıları basar,
/// hata sayısını döndürür.
///
/// `compile` aynı test bloklarını tek dosya olarak zaten denetlemiş ve
/// tanılarını `render_diagnostics` basmıştır; tam denetim onun üst
/// kümesidir. Yalnız YENİ tanılar basılır ve sayılır — yoksa her test
/// hatası iki kez görünür ve "2 error(s)" diye sayılırdı.
fn check_unit_tests(unit: &TestUnit) -> usize {
    let files = FsTestFiles::for_test_file(&unit.path);
    let test_diags =
        volt_hir::check_tests_with_files(&unit.sources(), &unit.compiled.ast, false, Some(&files));
    let fresh = unreported(test_diags, &unit.compiled.diagnostics);
    for diag in &fresh {
        eprintln!(
            "{}",
            volt_diagnostics::render_human(diag, &unit.compiled.map)
        );
    }
    fresh.iter().filter(|d| !d.code.is_warning()).count()
}

/// `candidates` içinden `reported`da birebir (kod, konum, ileti, notlar)
/// bulunmayanları sırayı koruyarak döndürür. Farklı konumdaki aynı hata
/// ayrı tanıdır: iki test bloğundaki aynı yanlış iki kez sayılır.
fn unreported(candidates: Vec<Diagnostic>, reported: &[Diagnostic]) -> Vec<Diagnostic> {
    candidates
        .into_iter()
        .filter(|diag| !reported.contains(diag))
        .collect()
}

// ═══ Gruplama ═════════════════════════════════════════════════════

/// AST'deki test bloklarını kaynak sırasıyla listeler.
fn tests_of(ast: &SourceFile) -> Vec<&TestDecl> {
    ast.items
        .iter()
        .filter_map(|i| match &ast.items_arena[*i].kind {
            ItemKind::Test(t) => Some(t),
            _ => None,
        })
        .collect()
}

/// Testleri toplar, ada göre süzer, (birim, modül) başına gruplar.
/// Dönen sayı, koşacak test sayısıdır.
pub(super) fn collect_groups(
    units: &[TestUnit],
    name_filter: Option<&str>,
) -> Result<(Vec<TestGroup>, usize), ExitCode> {
    let mut groups: Vec<TestGroup> = Vec::new();
    let mut total = 0usize;
    for (ui, unit) in units.iter().enumerate() {
        let files = FsTestFiles::for_test_file(&unit.path);
        let sources = unit.sources();
        let ctx = LowerCtx {
            map: &unit.compiled.map,
            file_label: &unit.file_label,
            sources: &sources,
            files: &files,
        };
        for decl in tests_of(&unit.compiled.ast) {
            let display = decl.display_name();
            if name_filter.is_some_and(|f| !display.contains(f)) {
                continue;
            }
            let Some(lowered) = lower_test(&ctx, decl) else {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: internal: test '{display}' passed checks but could not be lowered";
                        tr: "hata: içsel: '{display}' testi denetimden geçti ama indirgenemedi"
                    )
                );
                return Err(ExitCode::from(3));
            };
            total += 1;
            add_to_group(&mut groups, ui, lowered);
        }
    }
    Ok((groups, total))
}

/// İndirgenmiş testi (birim, modül) grubuna ekler; grup yoksa açar.
/// `load` hedefleri grupta tekilleştirilir (ilk görülme sırası korunur).
fn add_to_group(groups: &mut Vec<TestGroup>, unit: usize, lowered: LoweredTest) {
    let at = groups
        .iter()
        .position(|g| g.module == lowered.module && g.unit == unit)
        .unwrap_or_else(|| {
            groups.push(TestGroup {
                module: lowered.module.clone(),
                unit,
                tests: Vec::new(),
                load_targets: Vec::new(),
            });
            groups.len() - 1
        });
    let group = &mut groups[at];
    group.tests.push(lowered.tb);
    for target in lowered.load_targets {
        if !group.load_targets.contains(&target) {
            group.load_targets.push(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lowered(module: &str, name: &str, targets: &[(&str, &str)]) -> LoweredTest {
        LoweredTest {
            module: module.to_string(),
            tb: TbTest {
                name: name.to_string(),
                steps: Vec::new(),
            },
            load_targets: targets
                .iter()
                .map(|(m, r)| (m.to_string(), r.to_string()))
                .collect(),
        }
    }

    fn diag_at(code: volt_diagnostics::ErrorCode, start: u32) -> Diagnostic {
        let span = volt_span::Span::new(volt_span::FileId(0), start, start + 1);
        Diagnostic::error(
            code,
            "ileti".to_string(),
            volt_diagnostics::LabeledSpan::primary(span, "etiket".to_string()),
            "çözüm".to_string(),
        )
    }

    #[test]
    fn unreported_drops_exact_repeats_but_keeps_other_spans_and_codes() {
        // Arrange
        use volt_diagnostics::ErrorCode::{E8503, E8504};
        let reported = [diag_at(E8503, 10)];
        let candidates = vec![diag_at(E8503, 10), diag_at(E8503, 40), diag_at(E8504, 10)];

        // Act
        let fresh = unreported(candidates, &reported);

        // Assert: aynı kod başka konumda ve aynı konumda başka kod yenidir.
        assert_eq!(fresh, [diag_at(E8503, 40), diag_at(E8504, 10)]);
        assert!(unreported(vec![diag_at(E8503, 10)], &reported).is_empty());
        assert_eq!(unreported(vec![diag_at(E8503, 10)], &[]).len(), 1);
    }

    #[test]
    fn add_to_group_groups_by_unit_and_module_and_dedups_load_targets() {
        // Arrange
        let mut groups = Vec::new();

        // Act
        add_to_group(&mut groups, 0, lowered("Core", "a", &[("Soc", "mem")]));
        add_to_group(&mut groups, 0, lowered("Soc", "b", &[]));
        add_to_group(
            &mut groups,
            0,
            lowered("Core", "c", &[("Soc", "mem"), ("Soc", "ram")]),
        );
        add_to_group(&mut groups, 1, lowered("Core", "d", &[]));

        // Assert: ilk görülme sırası korunur, aynı modül başka birimde ayrı grup.
        let shape: Vec<(&str, usize, Vec<&str>)> = groups
            .iter()
            .map(|g| {
                let names = g.tests.iter().map(|t| t.name.as_str()).collect();
                (g.module.as_str(), g.unit, names)
            })
            .collect();
        assert_eq!(
            shape,
            [
                ("Core", 0, vec!["a", "c"]),
                ("Soc", 0, vec!["b"]),
                ("Core", 1, vec!["d"])
            ]
        );
        assert_eq!(
            groups[0].load_targets,
            [
                ("Soc".to_string(), "mem".to_string()),
                ("Soc".to_string(), "ram".to_string())
            ]
        );
    }
}
