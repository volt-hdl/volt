//! `volt run` — tek modülü serbest koşan testbench ile simüle eder.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use volt_ast::ModuleDecl;
use volt_diagnostics::lstr;
use volt_sv_emit::{sim as tbgen, uses_sim_contracts, SimPort, SvaMode};

use super::contracts::{
    cover_summary_lines, covers_in, parse_contract_fail, ContractIndex, ContractViolation,
};
use super::verilator::{c_path, require_verilator, run_simulation, verilate, VerilateJob};
use super::{create_sim_dir, modules_of, write_file};
use crate::extern_stage::{compile_for_tool, stage_extern_sources};
use crate::{render_diagnostics, Compiled, OutputFormat};

/// `volt run` seçenekleri (cli-contract.md §7).
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunOptions<'a> {
    pub cycles: u64,
    pub vcd: Option<&'a Path>,
    pub top: Option<&'a str>,
    /// Kontrat izleyicileri (ADR-0064) — `volt run`'da isteğe bağlı:
    /// duman uyarıcısı (girişler 1) tasarımın varsayımlarını gözetmez.
    pub contracts: bool,
    pub target_dir: &'a Path,
}

pub(crate) fn run(file: &Path, opts: RunOptions<'_>) -> ExitCode {
    match run_inner(file, opts) {
        Ok(code) | Err(code) => code,
    }
}

fn run_inner(file: &Path, opts: RunOptions<'_>) -> Result<ExitCode, ExitCode> {
    let RunOptions {
        cycles,
        vcd,
        top,
        target_dir,
        ..
    } = opts;
    let start = Instant::now();
    let (compiled, sv) = compile_for_run(file, opts.contracts)?;

    // Üst modül seçimi: --top > dosyadaki tek modül.
    let module = select_top(&modules_of(&compiled.ast), top, file)?;
    // ADR-0078: Verilator model sınıfıyla çakışan üst port (E8513).
    let clashes = volt_hir::verilator_top_clashes(&compiled.ast, module);
    if !clashes.is_empty() {
        for diag in &clashes {
            eprintln!("{}", volt_diagnostics::render_human(diag, &compiled.map));
        }
        return Err(ExitCode::from(1));
    }
    let module_name = module.name.text.clone();
    let ports = tbgen::collect_sim_ports(&compiled.ast, module);

    let verilator = require_verilator("volt run")?;

    // build/sim/<stem>/ altına SV + tb yaz.
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let sim_dir = target_dir.join("sim").join(&stem);
    let sv_name = format!("{module_name}.sv");
    create_sim_dir(&sim_dir)?;
    write_file(&sim_dir.join(&sv_name), &sv)?;
    // Extern gövdeleri (ADR-0076) üretilen SV'den önce.
    let mut inputs = stage_extern_sources(&compiled.extern_sources, &sim_dir, &[sv_name.as_str()])?;
    inputs.push(sv_name);
    let contracts = uses_sim_contracts(&sv);
    let tb = run_testbench(&module_name, &ports, cycles, vcd, contracts);
    write_file(&sim_dir.join("tb.cpp"), &tb)?;

    eprintln!(
        "{}",
        lstr!(
            en: "  Simulating {module_name} ({cycles} cycles)";
            tr: "  Simüle ediliyor {module_name} ({cycles} döngü)"
        )
    );
    let job = VerilateJob {
        sim_dir: &sim_dir,
        inputs: &inputs,
        tb_file: "tb.cpp",
        module: &module_name,
        trace: vcd.is_some(),
        mdir: "obj_dir",
    };
    let exe = verilate(&verilator, &job)?;
    let stdout = simulate(&exe, &sim_dir)?;
    let code = if contracts {
        let index = ContractIndex::new(&compiled.sva_props, &compiled.map, &module_name);
        report_contracts(&stdout, &index)
    } else {
        ExitCode::SUCCESS
    };
    print_finished(file, vcd, start);
    Ok(code)
}

/// Koşu sonundaki ihlal ve cover satırlarının raporu (ADR-0064). Bir
/// kontrat ihlali çıkış kodu 5'tir; varsayım ihlali duman uyarıcısının
/// sonucudur (tasarım hatası değil), yalnız raporlanır.
fn report_contracts(stdout: &str, index: &ContractIndex) -> ExitCode {
    let violations: Vec<ContractViolation> = stdout
        .lines()
        .filter_map(|l| l.strip_prefix("VOLT-CONTRACT-FAIL "))
        .filter_map(parse_contract_fail)
        .map(|v| index.resolve(v))
        .collect();
    for v in &violations {
        println!();
        for line in v.report_lines() {
            println!("{line}");
        }
    }
    let summary = cover_summary_lines(&index.covers(&covers_in(stdout)));
    if !summary.is_empty() {
        println!();
        for line in summary {
            println!("{line}");
        }
    }
    if violations.iter().any(|v| !v.is_assumption()) {
        ExitCode::from(5)
    } else {
        ExitCode::SUCCESS
    }
}

/// Simülasyonu koşturur ve çevrim tablosunu basar; izleyici satırları
/// (`VOLT-*`) tablodan ayıklanır ve ham çıktıyla döner.
fn simulate(exe: &Path, sim_dir: &Path) -> Result<String, ExitCode> {
    let output = run_simulation(exe, sim_dir)?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    for line in stdout.lines().filter(|l| !l.starts_with("VOLT-")) {
        println!("{line}");
    }
    if !output.status.success() {
        eprintln!(
            "{}",
            lstr!(
                en: "error: simulation exited with code {:?}", output.status.code();
                tr: "hata: simülasyon {:?} koduyla çıktı", output.status.code()
            )
        );
        return Err(ExitCode::from(3));
    }
    Ok(stdout)
}

/// Dosyayı derler; SV üretilemediyse (hata) çıkış kodu 1.
fn compile_for_run(file: &Path, contracts: bool) -> Result<(Compiled, String), ExitCode> {
    eprintln!(
        "{}",
        lstr!(
            en: "   Compiling {}", file.display();
            tr: "   Derleniyor {}", file.display()
        )
    );
    let mode = if contracts {
        SvaMode::Simulation
    } else {
        SvaMode::None
    };
    let compiled = compile_for_tool(file, mode, "run")?;
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
        return Err(ExitCode::from(1));
    };
    Ok((compiled, sv))
}

/// Üst modül: `--top` verildiyse o ad, yoksa dosyadaki TEK modül.
/// Bulunamazsa kullanım hatası (çıkış kodu 2).
fn select_top<'a>(
    modules: &[&'a ModuleDecl],
    top: Option<&str>,
    file: &Path,
) -> Result<&'a ModuleDecl, ExitCode> {
    if let Some(name) = top {
        if let Some(m) = modules.iter().find(|m| m.name.text == name) {
            return Ok(*m);
        }
        eprintln!(
            "{}",
            lstr!(
                en: "error: no module named '{name}' in '{}'", file.display();
                tr: "hata: '{}' içinde '{name}' adlı modül yok", file.display()
            )
        );
        return Err(ExitCode::from(2));
    }
    match modules {
        [] => eprintln!(
            "{}",
            lstr!(
                en: "error: '{}' contains no module to simulate", file.display();
                tr: "hata: '{}' simüle edilecek modül içermiyor", file.display()
            )
        ),
        [only] => return Ok(*only),
        _ => eprintln!(
            "{}",
            lstr!(
                en: "error: '{}' contains {} modules — pick one with --top <module>",
                    file.display(), modules.len();
                tr: "hata: '{}' {} modül içeriyor — --top <modül> ile birini seçin",
                    file.display(), modules.len()
            )
        ),
    }
    Err(ExitCode::from(2))
}

/// Serbest koşan testbench; VCD yolu tb'ye mutlak ve `/` ile gömülür.
fn run_testbench(
    module: &str,
    ports: &[SimPort],
    cycles: u64,
    vcd: Option<&Path>,
    contracts: bool,
) -> String {
    let vcd_str = vcd.map(absolute_vcd).as_deref().map(c_path);
    tbgen::run_testbench_cpp_with(module, ports, cycles, vcd_str.as_deref(), contracts)
}

/// VCD yolu kullanıcı cwd'sine göre çözülür; tb'ye mutlak gömülür.
fn absolute_vcd(vcd: &Path) -> PathBuf {
    if vcd.is_absolute() {
        vcd.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(vcd)
    }
}

/// Kapanış satırları: dalga formu, süre ve bağlama göre sonraki adım.
fn print_finished(file: &Path, vcd: Option<&Path>, start: Instant) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_vcd_keeps_absolute_and_anchors_relative_to_cwd() {
        let cwd = std::env::current_dir().expect("cwd");
        assert_eq!(absolute_vcd(&cwd.join("w.vcd")), cwd.join("w.vcd"));
        assert_eq!(absolute_vcd(Path::new("w.vcd")), cwd.join("w.vcd"));
    }

    #[test]
    fn select_top_without_modules_is_a_usage_error() {
        let file = Path::new("empty.volt");
        assert!(select_top(&[], None, file).is_err());
        assert!(select_top(&[], Some("Top"), file).is_err());
    }
}
