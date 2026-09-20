//! `volt run` ve `volt test` — Verilator simülasyon köprüsü (ADR-0033).
//!
//! Akış: derle → SV + C++ testbench üret → `verilator --cc --exe
//! --build` → yürütülebiliri koştur → çıktıyı yorumla. Verilator
//! `VOLT_VERILATOR` > `PATH` sırasıyla aranır (F4b `verify` VOLT_SBY
//! deseni). Çıkış kodları (cli-contract.md §2): 0 başarı, 1 derleme
//! hatası, 2 kullanım hatası, 3 Verilator yok / araç hatası, 5 test
//! koştu ve en az biri kaldı.
//!
//! Cargo biçimli test raporu BİLEREK İngilizcedir (makine-okur veri
//! çıktısı, cargo ile birebir); çevresel iletiler `lstr!` ile yereldir.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use volt_ast::{ItemKind, ModuleDecl, SourceFile, TestDecl};
use volt_diagnostics::lstr;
use volt_sv_emit::{sim as tbgen, SvaMode, TbTest};

use crate::sim_lower::{lower_test, FsTestFiles, LowerCtx};
use crate::{compile, render_diagnostics, Compiled, OutputFormat};

// ═══ Verilator keşfi ══════════════════════════════════════════════

/// Verilator yürütülebiliri: önce VOLT_VERILATOR, sonra PATH.
fn find_verilator() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VOLT_VERILATOR") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in [
            "verilator",
            "verilator.exe",
            "verilator.bat",
            "verilator.cmd",
        ] {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// UX Anayasası biçiminde kurulum yardımı (goal ADIM 2).
fn print_verilator_not_found(command: &str) {
    eprintln!(
        "{}",
        lstr!(
            en: "error: Verilator not found\n\n  \
                 = reason: '{command}' uses Verilator for simulation\n  \
                 = help: install options:\n      \
                 Linux:   apt install verilator\n      \
                 macOS:   brew install verilator\n      \
                 Windows: use WSL or Docker\n  \
                 = note: 'volt build' and 'volt check' do not need it\n  \
                 = for more: volt explain simulation-setup";
            tr: "hata: Verilator bulunamadı\n\n  \
                 = neden: '{command}' simülasyon için Verilator kullanır\n  \
                 = çözüm: kurulum seçenekleri:\n      \
                 Linux:   apt install verilator\n      \
                 macOS:   brew install verilator\n      \
                 Windows: WSL ya da Docker kullanın\n  \
                 = not: 'volt build' ve 'volt check' Verilator gerektirmez\n  \
                 = daha fazla: volt explain simulation-setup"
        )
    );
}

// ═══ Ortak yardımcılar ════════════════════════════════════════════

fn io_error(path: &Path, err: &std::io::Error) -> ExitCode {
    eprintln!(
        "{}",
        lstr!(
            en: "error: cannot write '{}': {}", path.display(), err;
            tr: "hata: '{}' yazılamadı: {}", path.display(), err
        )
    );
    ExitCode::from(3)
}

/// AST'deki modülleri kaynak sırasıyla listeler.
fn modules_of(ast: &SourceFile) -> Vec<&ModuleDecl> {
    ast.items
        .iter()
        .filter_map(|i| match &ast.items_arena[*i].kind {
            ItemKind::Module(m) => Some(m),
            _ => None,
        })
        .collect()
}

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

/// Verilator'u koşturur; başarısızlıkta günlüğün kuyruğunu basar.
/// `sv_files` sim dizinine göre görelidir.
fn verilate(
    verilator: &Path,
    sim_dir: &Path,
    sv_files: &[String],
    tb_file: &str,
    module: &str,
    trace: bool,
    mdir: &str,
) -> Result<PathBuf, ExitCode> {
    let mut cmd = std::process::Command::new(verilator);
    cmd.arg("--cc");
    for sv in sv_files {
        cmd.arg(sv);
    }
    cmd.arg("--exe")
        .arg(tb_file)
        .arg("--build")
        .arg("--top-module")
        .arg(module)
        .arg("-Mdir")
        .arg(mdir)
        .current_dir(sim_dir);
    if trace {
        cmd.arg("--trace");
    }
    let output = match cmd.output() {
        Ok(o) => o,
        Err(err) => {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: cannot run '{}': {}", verilator.display(), err;
                    tr: "hata: '{}' çalıştırılamadı: {}", verilator.display(), err
                )
            );
            return Err(ExitCode::from(3));
        }
    };
    if !output.status.success() {
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let tail: Vec<&str> = log.lines().rev().take(30).collect();
        for line in tail.iter().rev() {
            eprintln!("{line}");
        }
        eprintln!(
            "{}",
            lstr!(
                en: "error: Verilator failed for module '{module}' (exit code {:?})\n  \
                     = help: re-run in '{}' to see the full log",
                    output.status.code(), sim_dir.display();
                tr: "hata: Verilator '{module}' modülünde başarısız (çıkış kodu {:?})\n  \
                     = çözüm: tam log için '{}' içinde yeniden çalıştırın",
                    output.status.code(), sim_dir.display()
            )
        );
        return Err(ExitCode::from(3));
    }
    // Verilator yürütülebiliri obj dizinine V<modul> adıyla koyar.
    // Yol MUTLAK yapılır: yürütme `current_dir(sim_dir)` ile yapılır
    // ve göreli yol çocuğun cwd'sine göre çözülürdü.
    let sim_abs = sim_dir
        .canonicalize()
        .unwrap_or_else(|_| sim_dir.to_path_buf());
    let exe = sim_abs.join(mdir).join(format!("V{module}"));
    let exe_win = exe.with_extension("exe");
    Ok(if exe_win.is_file() { exe_win } else { exe })
}

/// C string'ine gömülecek yol: ters bölüler öne çevrilir.
fn c_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

// ═══ volt run ═════════════════════════════════════════════════════

pub(crate) fn run(
    file: &Path,
    cycles: u64,
    vcd: Option<&Path>,
    top: Option<&str>,
    target_dir: &Path,
) -> ExitCode {
    let start = Instant::now();
    eprintln!(
        "{}",
        lstr!(
            en: "   Compiling {}", file.display();
            tr: "   Derleniyor {}", file.display()
        )
    );
    let compiled = match compile(file, true, SvaMode::None) {
        Ok(c) => c,
        Err(code) => return code,
    };
    render_diagnostics(&compiled, OutputFormat::Human);
    let Some(sv) = compiled.sv.clone() else {
        eprintln!(
            "{}",
            lstr!(
                en: "     Error: run failed due to {} error(s), {} warning(s)",
                    compiled.errors(), compiled.warnings();
                tr: "     Hata: {} hata, {} uyarı nedeniyle koşu başarısız",
                    compiled.errors(), compiled.warnings()
            )
        );
        return ExitCode::from(1);
    };

    // Üst modül seçimi: --top > dosyadaki tek modül.
    let modules = modules_of(&compiled.ast);
    let module = match top {
        Some(name) => match modules.iter().find(|m| m.name.text == name) {
            Some(m) => *m,
            None => {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: no module named '{name}' in '{}'", file.display();
                        tr: "hata: '{}' içinde '{name}' adlı modül yok", file.display()
                    )
                );
                return ExitCode::from(2);
            }
        },
        None => match modules.as_slice() {
            [] => {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: '{}' contains no module to simulate", file.display();
                        tr: "hata: '{}' simüle edilecek modül içermiyor", file.display()
                    )
                );
                return ExitCode::from(2);
            }
            [only] => *only,
            _ => {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: '{}' contains {} modules — pick one with --top <module>",
                            file.display(), modules.len();
                        tr: "hata: '{}' {} modül içeriyor — --top <modül> ile birini seçin",
                            file.display(), modules.len()
                    )
                );
                return ExitCode::from(2);
            }
        },
    };
    let module_name = module.name.text.clone();
    let ports = tbgen::collect_sim_ports(&compiled.ast, module);

    let Some(verilator) = find_verilator() else {
        print_verilator_not_found("volt run");
        return ExitCode::from(3);
    };

    // build/sim/<stem>/ altına SV + tb yaz.
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let sim_dir = target_dir.join("sim").join(&stem);
    if let Err(err) = std::fs::create_dir_all(&sim_dir) {
        return io_error(&sim_dir, &err);
    }
    let sv_name = format!("{module_name}.sv");
    if let Err(err) = std::fs::write(sim_dir.join(&sv_name), &sv) {
        return io_error(&sim_dir.join(&sv_name), &err);
    }
    // VCD yolu kullanıcı cwd'sine göre çözülür; tb'ye mutlak gömülür.
    let vcd_abs = vcd.map(|p| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    });
    let vcd_str = vcd_abs.as_deref().map(c_path);
    let tb = tbgen::run_testbench_cpp(&module_name, &ports, cycles, vcd_str.as_deref());
    if let Err(err) = std::fs::write(sim_dir.join("tb.cpp"), tb) {
        return io_error(&sim_dir.join("tb.cpp"), &err);
    }

    eprintln!(
        "{}",
        lstr!(
            en: "  Simulating {module_name} ({cycles} cycles)";
            tr: "  Simüle ediliyor {module_name} ({cycles} döngü)"
        )
    );
    let exe = match verilate(
        &verilator,
        &sim_dir,
        &[sv_name],
        "tb.cpp",
        &module_name,
        vcd.is_some(),
        "obj_dir",
    ) {
        Ok(e) => e,
        Err(code) => return code,
    };

    let output = match std::process::Command::new(&exe)
        .current_dir(&sim_dir)
        .output()
    {
        Ok(o) => o,
        Err(err) => {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: cannot run '{}': {}", exe.display(), err;
                    tr: "hata: '{}' çalıştırılamadı: {}", exe.display(), err
                )
            );
            return ExitCode::from(3);
        }
    };
    print!("{}", String::from_utf8_lossy(&output.stdout));
    if !output.status.success() {
        eprintln!(
            "{}",
            lstr!(
                en: "error: simulation exited with code {:?}", output.status.code();
                tr: "hata: simülasyon {:?} koduyla çıktı", output.status.code()
            )
        );
        return ExitCode::from(3);
    }
    if let Some(vcd_path) = vcd {
        eprintln!(
            "{}",
            lstr!(
                en: "    Waveform {}", vcd_path.display();
                tr: "  Dalga formu {}", vcd_path.display()
            )
        );
    }
    eprintln!(
        "{}",
        lstr!(
            en: "    Finished {:.2}s", start.elapsed().as_secs_f64();
            tr: "    Tamamlandı {:.2}s", start.elapsed().as_secs_f64()
        )
    );
    // Bağlama göre sonraki adım: dalga formu alınmadıysa --vcd öner.
    let name = file.display();
    if vcd.is_none() {
        eprintln!(
            "{}",
            lstr!(
                en: "        Next: volt test                        (run tests)\n              \
                     volt run --vcd waves.vcd {name}   (record a waveform)";
                tr: "    Sıradaki: volt test                        (testleri koştur)\n              \
                     volt run --vcd dalga.vcd {name}   (dalga formu kaydet)"
            )
        );
    } else {
        eprintln!(
            "{}",
            lstr!(
                en: "        Next: volt test   (run tests)";
                tr: "    Sıradaki: volt test   (testleri koştur)"
            )
        );
    }
    ExitCode::SUCCESS
}

// ═══ volt test ════════════════════════════════════════════════════

/// Bir test yürütülebilirinden ayrıştırılan tek test sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestOutcome {
    name: String,
    passed: bool,
    /// `VOLT-ASSERT-FAIL <kind> <loc> left=<l> right=<r>` ayrıntısı.
    failure: Option<AssertFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssertFailure {
    kind: String,
    loc: String,
    left: u64,
    right: u64,
    /// `loop=i=3,j=1` — hata `for` içindeyse sayaç değerleri (ADR-0058).
    loop_ctx: Option<String>,
    /// `port=addr:u3` — port genişlik hatasında (ad, tip) (ADR-0059).
    port: Option<(String, String)>,
}

/// Testbench stdout'unu sonuçlara çevirir (sv-emit VOLT-* protokolü).
fn parse_tb_output(out: &str) -> Vec<TestOutcome> {
    let mut results = Vec::new();
    let mut pending_fail: Option<AssertFailure> = None;
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("VOLT-ASSERT-FAIL ") {
            pending_fail = parse_assert_fail(rest);
        } else if let Some(rest) = line.strip_prefix("VOLT-TEST-END ") {
            let (name, status) = match rest.rsplit_once(' ') {
                Some(pair) => pair,
                None => continue,
            };
            results.push(TestOutcome {
                name: name.to_string(),
                passed: status == "ok",
                failure: pending_fail.take(),
            });
        }
    }
    results
}

/// `assert_eq dosya.volt:24 left=1 right=0` → AssertFailure.
fn parse_assert_fail(rest: &str) -> Option<AssertFailure> {
    let mut parts = rest.split_whitespace();
    let kind = parts.next()?.to_string();
    let loc = parts.next()?.to_string();
    let left = parts.next()?.strip_prefix("left=")?.parse().ok()?;
    let right = parts.next()?.strip_prefix("right=")?.parse().ok()?;
    let (mut loop_ctx, mut port) = (None, None);
    for extra in parts {
        if let Some(vars) = extra.strip_prefix("loop=") {
            loop_ctx = Some(vars.replace('=', " = ").replace(',', ", "));
        } else if let Some((name, ty)) = extra.strip_prefix("port=").and_then(|p| p.split_once(':'))
        {
            port = Some((name.to_string(), ty.to_string()));
        }
    }
    Some(AssertFailure {
        kind,
        loc,
        left,
        right,
        loop_ctx,
        port,
    })
}

/// `X_test.volt` için kardeş `X.volt` yolu (varsa).
fn sibling_path(file: &Path) -> Option<PathBuf> {
    let stem = file.file_stem()?.to_string_lossy();
    let base = stem.strip_suffix("_test")?;
    let sibling = file.with_file_name(format!("{base}.volt"));
    sibling.is_file().then_some(sibling)
}

/// Çalışma dizinindeki `*_test.volt` dosyaları (ad sırasıyla).
fn discover_test_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(".")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with("_test.volt"))
        })
        .collect();
    files.sort();
    files
}

/// Aynı yürütülebilirde koşan testler: (birim, DUT modülü) başına bir grup.
struct TestGroup {
    module: String,
    unit: usize,
    tests: Vec<TbTest>,
    /// `load` ile yazılan bellekler: (sahip modül, yazmaç adı).
    load_targets: Vec<(String, String)>,
}

/// Tek test dosyasının derlenmiş hâli (+ varsa kardeşi).
struct TestUnit {
    file_label: String,
    /// `read_hex` yollarının çözüldüğü test dosyası.
    path: PathBuf,
    compiled: Compiled,
    sibling: Option<Compiled>,
}

pub(crate) fn test(filter: Option<&str>, nocapture: bool, target_dir: &Path) -> ExitCode {
    // Filtre bir .volt dosyası mı, ad süzgeci mi?
    let (files, name_filter) = match filter {
        Some(f) if f.ends_with(".volt") => {
            let p = PathBuf::from(f);
            if !p.is_file() {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot read '{f}': file not found";
                        tr: "hata: '{f}' okunamadı: dosya yok"
                    )
                );
                return ExitCode::from(3);
            }
            (vec![p], None)
        }
        other => (discover_test_files(), other),
    };

    // ── Derle (test dosyası + kardeşi) ve testleri doğrula ──
    let mut units = Vec::new();
    for file in &files {
        eprintln!(
            "{}",
            lstr!(
                en: "   Compiling {}", file.display();
                tr: "   Derleniyor {}", file.display()
            )
        );
        let compiled = match compile(file, true, SvaMode::None) {
            Ok(c) => c,
            Err(code) => return code,
        };
        render_diagnostics(&compiled, OutputFormat::Human);
        let sibling = match sibling_path(file) {
            Some(sib) => {
                let c = match compile(&sib, true, SvaMode::None) {
                    Ok(c) => c,
                    Err(code) => return code,
                };
                render_diagnostics(&c, OutputFormat::Human);
                Some(c)
            }
            None => None,
        };
        let mut errors = compiled.errors() + sibling.as_ref().map_or(0, Compiled::errors);

        // Kardeş dosya dahil TAM test denetimi (E8501-E8506).
        let mut sources: Vec<&SourceFile> = vec![&compiled.ast];
        if let Some(sib) = &sibling {
            sources.push(&sib.ast);
        }
        let files = FsTestFiles::for_test_file(file);
        let test_diags =
            volt_hir::check_tests_with_files(&sources, &compiled.ast, false, Some(&files));
        for diag in &test_diags {
            eprintln!("{}", volt_diagnostics::render_human(diag, &compiled.map));
        }
        errors += test_diags.iter().filter(|d| !d.code.is_warning()).count();
        if errors > 0 {
            eprintln!(
                "{}",
                lstr!(
                    en: "     Error: test build failed due to {errors} error(s)";
                    tr: "     Hata: {errors} hata nedeniyle test derlemesi başarısız"
                )
            );
            return ExitCode::from(1);
        }
        units.push(TestUnit {
            path: file.clone(),
            file_label: file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| file.display().to_string()),
            compiled,
            sibling,
        });
    }

    // ── Testleri topla, süz, modüle göre grupla ──
    // (modül adı, ait olduğu birim indeksi) → tb testleri
    let mut groups: Vec<TestGroup> = Vec::new();
    let mut total = 0usize;
    for (ui, unit) in units.iter().enumerate() {
        let files = FsTestFiles::for_test_file(&unit.path);
        let mut sources: Vec<&SourceFile> = vec![&unit.compiled.ast];
        if let Some(sib) = &unit.sibling {
            sources.push(&sib.ast);
        }
        let ctx = LowerCtx {
            map: &unit.compiled.map,
            file_label: &unit.file_label,
            sources: &sources,
            files: &files,
        };
        for decl in tests_of(&unit.compiled.ast) {
            let display = decl.display_name();
            if let Some(f) = name_filter {
                if !display.contains(f) {
                    continue;
                }
            }
            let Some(lowered) = lower_test(&ctx, decl) else {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: internal: test '{display}' passed checks but could not be lowered";
                        tr: "hata: içsel: '{display}' testi denetimden geçti ama indirgenemedi"
                    )
                );
                return ExitCode::from(3);
            };
            total += 1;
            let at = groups
                .iter()
                .position(|g| g.module == lowered.module && g.unit == ui)
                .unwrap_or_else(|| {
                    groups.push(TestGroup {
                        module: lowered.module.clone(),
                        unit: ui,
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
    }

    println!("running {total} tests");
    if total == 0 {
        println!("\ntest result: ok. 0 passed; 0 failed");
        return ExitCode::SUCCESS;
    }

    let Some(verilator) = find_verilator() else {
        print_verilator_not_found("volt test");
        return ExitCode::from(3);
    };

    // ── Grup başına verilate + koştur ──
    let mut outcomes: Vec<TestOutcome> = Vec::new();
    for group in &groups {
        let (module, tests) = (&group.module, &group.tests);
        let unit = &units[group.unit];
        // Modülün portları ve SV'si test dosyasından ya da kardeşten gelir.
        let holder = [Some(&unit.compiled), unit.sibling.as_ref()]
            .into_iter()
            .flatten()
            .find_map(|c| {
                modules_of(&c.ast)
                    .into_iter()
                    .find(|m| m.name.text == *module)
                    .map(|m| (tbgen::collect_sim_ports(&c.ast, m), c.sv.clone()))
            });
        let Some((ports, Some(sv))) = holder else {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: internal: module '{module}' passed checks but has no generated SV";
                    tr: "hata: içsel: '{module}' modülü denetimden geçti ama üretilmiş SV'si yok"
                )
            );
            return ExitCode::from(3);
        };

        let stem = unit.file_label.trim_end_matches(".volt").to_string();
        let sim_dir = target_dir.join("sim").join(&stem);
        if let Err(err) = std::fs::create_dir_all(&sim_dir) {
            return io_error(&sim_dir, &err);
        }
        let sv_name = format!("{module}.sv");
        if let Err(err) = std::fs::write(sim_dir.join(&sv_name), &sv) {
            return io_error(&sim_dir.join(&sv_name), &err);
        }
        let tb_name = format!("tb_{module}.cpp");
        let tb = tbgen::test_testbench_cpp(module, &ports, tests);
        if let Err(err) = std::fs::write(sim_dir.join(&tb_name), tb) {
            return io_error(&sim_dir.join(&tb_name), &err);
        }
        // `load` hedefleri yalnız adlarıyla açılır (ADR-0058): .vlt
        // dosyası SV'den ÖNCE verilir.
        let mut inputs = Vec::new();
        if !group.load_targets.is_empty() {
            let vlt_name = format!("load_{module}.vlt");
            let vlt = tbgen::load_config_vlt(&group.load_targets);
            if let Err(err) = std::fs::write(sim_dir.join(&vlt_name), vlt) {
                return io_error(&sim_dir.join(&vlt_name), &err);
            }
            inputs.push(vlt_name);
        }
        inputs.push(sv_name);
        let exe = match verilate(
            &verilator,
            &sim_dir,
            &inputs,
            &tb_name,
            module,
            false,
            &format!("obj_{}", module.to_lowercase()),
        ) {
            Ok(e) => e,
            Err(code) => return code,
        };
        let output = match std::process::Command::new(&exe)
            .current_dir(&sim_dir)
            .output()
        {
            Ok(o) => o,
            Err(err) => {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot run '{}': {}", exe.display(), err;
                        tr: "hata: '{}' çalıştırılamadı: {}", exe.display(), err
                    )
                );
                return ExitCode::from(3);
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        if nocapture {
            print!("{stdout}");
        }
        let parsed = parse_tb_output(&stdout);
        for o in &parsed {
            println!(
                "test {} ... {}",
                o.name,
                if o.passed { "ok" } else { "FAILED" }
            );
        }
        outcomes.extend(parsed);
    }

    // ── Cargo biçimli özet ──
    let failed: Vec<&TestOutcome> = outcomes.iter().filter(|o| !o.passed).collect();
    let passed = outcomes.len() - failed.len();
    if !failed.is_empty() {
        println!("\nfailures:");
        for o in &failed {
            println!("---- {} ----", o.name);
            match &o.failure {
                Some(f) => print_failure(f),
                None => println!("  test failed (no assertion detail)"),
            }
        }
        println!(
            "\ntest result: FAILED. {passed} passed; {} failed",
            failed.len()
        );
        return ExitCode::from(5);
    }
    println!("\ntest result: ok. {passed} passed; 0 failed");
    ExitCode::SUCCESS
}

/// Tek hatanın cargo biçimli ayrıntısı (İngilizce — makine-okur rapor).
fn print_failure(f: &AssertFailure) {
    match f.kind.as_str() {
        "assert_eq" | "assert_ne" => {
            println!("  {} failed at {}", f.kind, f.loc);
            println!("    left:  {}", f.left);
            println!("    right: {}", f.right);
        }
        "index_out_of_bounds" => {
            println!("  index out of bounds at {}", f.loc);
            println!("    index: {}", f.left);
            println!("    len:   {}", f.right);
        }
        "division_by_zero" => {
            println!("  division by zero at {}", f.loc);
            println!("    dividend: {}", f.left);
        }
        "port_overflow" => print_port_overflow(f),
        "load_too_long" => {
            println!("  load() source does not fit the target at {}", f.loc);
            println!("    source elements: {}", f.left);
            println!("    target elements: {}", f.right);
        }
        "load_value_too_wide" => {
            println!(
                "  load() value does not fit the target element at {}",
                f.loc
            );
            println!("    value: {}", f.left);
            println!("    index: {}", f.right);
        }
        _ => {
            println!("  {} failed at {}", f.kind, f.loc);
            println!("    value: {}", f.left);
        }
    }
    if let Some(ctx) = &f.loop_ctx {
        println!("    loop:  {ctx}");
    }
}

/// Port genişlik hatası (ADR-0059): `right` port genişliğidir (0 =
/// derleyici çözemedi, denetim C++ depolama tipine göre yapıldı).
fn port_overflow_lines(f: &AssertFailure) -> Vec<String> {
    let value = volt_hir::describe_value(f.left);
    let Some((name, ty)) = &f.port else {
        return vec![format!("  port cannot hold value {value} at {}", f.loc)];
    };
    let Some(bits) = u32::try_from(f.right).ok().filter(|b| *b > 0) else {
        return vec![format!(
            "  port '{name}' cannot hold value {value} at {}",
            f.loc
        )];
    };
    let kind = if ty.starts_with('i') {
        volt_hir::ScalarKind::SInt
    } else {
        volt_hir::ScalarKind::UInt
    };
    let range = volt_hir::PortWidth { bits, kind }.write_range();
    vec![
        format!(
            "  port '{name}' ({ty}) cannot hold value {value} at {}",
            f.loc
        ),
        format!("    range: {range}"),
    ]
}

fn print_port_overflow(f: &AssertFailure) {
    for line in port_overflow_lines(f) {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tb_output_reads_ok_and_fail() {
        let out = "VOLT-TEST-BEGIN a\nVOLT-TEST-END a ok\n\
                   VOLT-TEST-BEGIN b\n\
                   VOLT-ASSERT-FAIL assert_eq uart_tx_test.volt:24 left=1 right=0\n\
                   VOLT-TEST-END b fail\n";
        let results = parse_tb_output(out);
        assert_eq!(results.len(), 2);
        assert!(results[0].passed);
        assert!(results[0].failure.is_none());
        assert!(!results[1].passed);
        let f = results[1].failure.as_ref().expect("hata ayrıntısı");
        assert_eq!(f.kind, "assert_eq");
        assert_eq!(f.loc, "uart_tx_test.volt:24");
        assert_eq!((f.left, f.right), (1, 0));
    }

    #[test]
    fn parse_assert_fail_reads_port_and_loop_context() {
        let f = parse_assert_fail(
            "port_overflow t_test.volt:14 left=8 right=3 loop=i=8,j=1 port=addr:u3",
        )
        .expect("ayrışmalı");
        assert_eq!(f.kind, "port_overflow");
        assert_eq!((f.left, f.right), (8, 3));
        assert_eq!(f.loop_ctx.as_deref(), Some("i = 8, j = 1"));
        assert_eq!(f.port, Some(("addr".to_string(), "u3".to_string())));
    }

    #[test]
    fn port_overflow_report_names_port_value_and_range() {
        let f = parse_assert_fail("port_overflow t_test.volt:14 left=8 right=3 port=addr:u3")
            .expect("ayrışmalı");
        assert_eq!(
            port_overflow_lines(&f),
            vec![
                "  port 'addr' (u3) cannot hold value 8 at t_test.volt:14".to_string(),
                "    range: 0..7".to_string(),
            ]
        );
    }

    #[test]
    fn port_overflow_report_shows_negative_numbers_and_signed_range() {
        let f = parse_assert_fail(
            "port_overflow t_test.volt:9 left=18446744073709551487 right=8 port=sv:i8",
        )
        .expect("ayrışmalı");
        let lines = port_overflow_lines(&f);
        assert!(lines[0].contains("(i8) cannot hold value 18446744073709551487 (-129)"));
        assert_eq!(lines[1], "    range: -128..255");
    }

    #[test]
    fn port_overflow_report_without_a_known_width_has_no_range() {
        let f = parse_assert_fail("port_overflow t_test.volt:9 left=70000 right=0 port=word:?")
            .expect("ayrışmalı");
        assert_eq!(
            port_overflow_lines(&f),
            vec!["  port 'word' cannot hold value 70000 at t_test.volt:9".to_string()]
        );
    }

    #[test]
    fn parse_tb_output_ignores_noise_lines() {
        let out = "some verilator chatter\nVOLT-TEST-END only ok\n";
        let results = parse_tb_output(out);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "only");
    }

    #[test]
    fn parse_assert_fail_rejects_malformed() {
        assert!(parse_assert_fail("assert_eq file:1 left=x right=0").is_none());
        assert!(parse_assert_fail("assert_eq").is_none());
    }

    #[test]
    fn sibling_path_only_for_test_suffix() {
        // Kardeşi olmayan adlar: _test soneki yoksa None.
        assert!(sibling_path(Path::new("counter.volt")).is_none());
        // _test soneki var ama kardeş dosya diskte yok → None.
        assert!(sibling_path(Path::new("nonexistent_test.volt")).is_none());
    }

    #[test]
    fn c_path_uses_forward_slashes() {
        assert_eq!(c_path(Path::new("a\\b\\c.vcd")), "a/b/c.vcd");
    }
}
