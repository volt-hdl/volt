//! `volt run` — tek modülü serbest koşan testbench ile simüle eder.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use volt_ast::ModuleDecl;
use volt_diagnostics::lstr;
use volt_sv_emit::{sim as tbgen, SimPort, SvaMode};

use super::verilator::{c_path, require_verilator, run_simulation, verilate, VerilateJob};
use super::{create_sim_dir, modules_of, write_file};
use crate::{compile, render_diagnostics, Compiled, OutputFormat};

pub(crate) fn run(
    file: &Path,
    cycles: u64,
    vcd: Option<&Path>,
    top: Option<&str>,
    target_dir: &Path,
) -> ExitCode {
    match run_inner(file, cycles, vcd, top, target_dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run_inner(
    file: &Path,
    cycles: u64,
    vcd: Option<&Path>,
    top: Option<&str>,
    target_dir: &Path,
) -> Result<(), ExitCode> {
    let start = Instant::now();
    let (compiled, sv) = compile_for_run(file)?;

    // Üst modül seçimi: --top > dosyadaki tek modül.
    let module = select_top(&modules_of(&compiled.ast), top, file)?;
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
    let tb = run_testbench(&module_name, &ports, cycles, vcd);
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
        inputs: &[sv_name],
        tb_file: "tb.cpp",
        module: &module_name,
        trace: vcd.is_some(),
        mdir: "obj_dir",
    };
    let exe = verilate(&verilator, &job)?;
    simulate(&exe, &sim_dir)?;
    print_finished(file, vcd, start);
    Ok(())
}

/// Simülasyonu koşturur, stdout'unu aktarır; sıfır dışı çıkış araç hatasıdır.
fn simulate(exe: &Path, sim_dir: &Path) -> Result<(), ExitCode> {
    let output = run_simulation(exe, sim_dir)?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
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
    Ok(())
}

/// Dosyayı derler; SV üretilemediyse (hata) çıkış kodu 1.
fn compile_for_run(file: &Path) -> Result<(Compiled, String), ExitCode> {
    eprintln!(
        "{}",
        lstr!(
            en: "   Compiling {}", file.display();
            tr: "   Derleniyor {}", file.display()
        )
    );
    let compiled = compile(file, true, SvaMode::None)?;
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
fn run_testbench(module: &str, ports: &[SimPort], cycles: u64, vcd: Option<&Path>) -> String {
    let vcd_str = vcd.map(absolute_vcd).as_deref().map(c_path);
    tbgen::run_testbench_cpp(module, ports, cycles, vcd_str.as_deref())
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
