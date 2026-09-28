//! Verilator köprüsü: aracın keşfi, `--cc --exe --build` çağrısı ve
//! üretilen yürütülebilirin koşturulması — yerelde ya da Docker'da
//! (ADR-0094; yerel yol değişmedi).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use volt_diagnostics::lstr;
use volt_tools::docker_paths::{container_path, host_absolute, Mounts};

use crate::tool_backend::{self, DockerTool, Runner};

/// Günlüğün başarısızlıkta basılan kuyruğu (satır).
const LOG_TAIL_LINES: usize = 30;

// ═══ Verilator keşfi ══════════════════════════════════════════════

/// Verilator'u bulur: önce VOLT_VERILATOR, sonra PATH (tek kaynak
/// `volt-tools`, ADR-0084), yoksa Docker (ADR-0094); hiçbiri yoksa
/// kurulum yardımını basar (çıkış kodu 3).
pub(super) fn require_verilator(command: &str) -> Result<Runner, ExitCode> {
    tool_backend::resolve(volt_tools::Tool::Verilator, &|| {
        print_verilator_not_found(command)
    })
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
                 Windows: install Docker Desktop (Volt then runs Verilator in a container) or use WSL\n  \
                 = note: 'volt build' and 'volt check' do not need it\n  \
                 = for more: volt explain simulation-setup";
            tr: "hata: Verilator bulunamadı\n\n  \
                 = neden: '{command}' simülasyon için Verilator kullanır\n  \
                 = çözüm: kurulum seçenekleri:\n      \
                 Linux:   apt install verilator\n      \
                 macOS:   brew install verilator\n      \
                 Windows: Docker Desktop kurun (Volt Verilator'u konteynerde çalıştırır) ya da WSL kullanın\n  \
                 = not: 'volt build' ve 'volt check' Verilator gerektirmez\n  \
                 = daha fazla: volt explain simulation-setup"
        )
    );
}

// ═══ Derleme ══════════════════════════════════════════════════════

/// Tek Verilator derlemesi. `inputs` (SV ve varsa önündeki `.vlt`) ile
/// `tb_file` sim dizinine göre görelidir.
pub(super) struct VerilateJob<'a> {
    pub sim_dir: &'a Path,
    pub inputs: &'a [String],
    pub tb_file: &'a str,
    pub module: &'a str,
    pub trace: bool,
    pub mdir: &'a str,
}

impl VerilateJob<'_> {
    /// Verilator argümanları (sıra sabittir); girdiler sim dizinine
    /// göreli olduğundan yerelde ve konteynerde aynıdır.
    fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec!["--cc".into()];
        args.extend(self.inputs.iter().map(OsString::from));
        args.extend(
            [
                "--exe",
                self.tb_file,
                "--build",
                "--top-module",
                self.module,
                "-Mdir",
                self.mdir,
            ]
            .map(OsString::from),
        );
        if self.trace {
            args.push("--trace".into());
        }
        args
    }

    /// Yerel Verilator komut satırı.
    fn command(&self, verilator: &Path) -> Command {
        let mut cmd = Command::new(verilator);
        cmd.args(self.args()).current_dir(self.sim_dir);
        cmd
    }

    /// Verilator yürütülebiliri obj dizinine V<modul> adıyla koyar.
    /// Yol MUTLAK yapılır: yürütme `current_dir(sim_dir)` ile yapılır
    /// ve göreli yol çocuğun cwd'sine göre çözülürdü.
    fn exe_path(&self) -> PathBuf {
        let sim_abs = self
            .sim_dir
            .canonicalize()
            .unwrap_or_else(|_| self.sim_dir.to_path_buf());
        let exe = sim_abs.join(self.mdir).join(format!("V{}", self.module));
        let exe_win = exe.with_extension("exe");
        if exe_win.is_file() {
            exe_win
        } else {
            exe
        }
    }

    /// Başarısız derlemenin günlük kuyruğu + hata iletisi.
    fn print_failure(&self, output: &Output) {
        print_log_tail(&output_text(output));
        let module = self.module;
        eprintln!(
            "{}",
            lstr!(
                en: "error: Verilator failed for module '{module}' (exit code {:?})\n  \
                     = help: re-run in '{}' to see the full log",
                    output.status.code(), self.sim_dir.display();
                tr: "hata: Verilator '{module}' modülünde başarısız (çıkış kodu {:?})\n  \
                     = çözüm: tam log için '{}' içinde yeniden çalıştırın",
                    output.status.code(), self.sim_dir.display()
            )
        );
    }
}

/// Verilator'u koşturur; başarısızlıkta günlüğün kuyruğunu basar.
/// Başarıda üretilen yürütülebilirin mutlak yolunu döndürür. `extra`
/// Docker'da sim dizinine ek olarak bağlanan dizinlerdir (VCD dizini).
pub(super) fn verilate(
    runner: &Runner,
    job: &VerilateJob<'_>,
    extra: &[&Path],
) -> Result<PathBuf, ExitCode> {
    let verilator = match runner {
        Runner::Local(path) => path,
        Runner::Docker(tool) => return verilate_in_docker(tool, job, extra),
    };
    let output = run_tool(job.command(verilator), verilator)?;
    if !output.status.success() {
        job.print_failure(&output);
        return Err(ExitCode::from(3));
    }
    Ok(job.exe_path())
}

/// Docker kolu: aynı argümanlar, sim dizini konteynere bağlı. Araç
/// çıktısındaki konteyner yolları ana makine yoluna çevrilir; tam günlük
/// sim dizinine yazılır (yeniden koşturma yerel araç gerektirirdi).
fn verilate_in_docker(
    tool: &DockerTool,
    job: &VerilateJob<'_>,
    extra: &[&Path],
) -> Result<PathBuf, ExitCode> {
    let (mounts, workdir) = sim_mounts(job.sim_dir, extra)?;
    // `--watch`'ta adlı: Ctrl-C konteyneri kaldırır (ADR-0095).
    let name = crate::interrupt::container_name_for(&tool.docker);
    let cmd = tool.command(&mounts, workdir, None, name, job.sim_dir, job.args());
    let output = translate(run_tool(cmd, &tool.docker)?, &mounts);
    crate::interrupt::park_if_stopping();
    if !output.status.success() {
        if tool_backend::report_container_failure(output.status.code()) {
            return Err(ExitCode::from(3));
        }
        let log = output_text(&output);
        print_log_tail(&log);
        let log_path = job.sim_dir.join("verilator.log");
        let _ = std::fs::write(&log_path, &log);
        let (module, image) = (job.module, tool.image.name);
        eprintln!(
            "{}",
            lstr!(
                en: "error: Verilator failed for module '{module}' (exit code {:?}, in Docker {image})\n  \
                     = help: the full log is in '{}'",
                    output.status.code(), log_path.display();
                tr: "hata: Verilator '{module}' modülünde başarısız (çıkış kodu {:?}, Docker {image} içinde)\n  \
                     = çözüm: tam log '{}' dosyasında",
                    output.status.code(), log_path.display()
            )
        );
        return Err(ExitCode::from(3));
    }
    // Linux yürütülebilirinin eki yoktur (yereldeki eski `.exe` değil).
    Ok(host_dir(job.sim_dir)?
        .join(job.mdir)
        .join(format!("V{}", job.module)))
}

// ═══ Koşturma ═════════════════════════════════════════════════════

/// Üretilen simülasyon yürütülebilirini sim dizininde koşturur.
pub(super) fn run_simulation(
    runner: &Runner,
    exe: &Path,
    sim_dir: &Path,
    extra: &[&Path],
) -> Result<Output, ExitCode> {
    let Runner::Docker(tool) = runner else {
        let mut cmd = Command::new(exe);
        cmd.current_dir(sim_dir);
        return run_tool(cmd, exe);
    };
    let (mounts, workdir) = sim_mounts(sim_dir, extra)?;
    let entry = container_path(exe).ok_or_else(|| unmappable(exe))?;
    let name = crate::interrupt::container_name_for(&tool.docker);
    let cmd = tool.command(&mounts, workdir, Some(entry), name, sim_dir, Vec::new());
    let output = translate(run_tool(cmd, &tool.docker)?, &mounts);
    crate::interrupt::park_if_stopping();
    if tool_backend::report_container_failure(output.status.code()) {
        return Err(ExitCode::from(3));
    }
    Ok(output)
}

/// Sim dizini (+ ek dizinler) bağlamaları ve konteynerdeki çalışma dizini.
fn sim_mounts(sim_dir: &Path, extra: &[&Path]) -> Result<(Mounts, String), ExitCode> {
    let mut mounts = Mounts::new();
    let workdir = mounts.add(sim_dir).map_err(|e| mount_error(sim_dir, &e))?;
    for dir in extra {
        mounts.add(dir).map_err(|e| mount_error(dir, &e))?;
    }
    Ok((mounts, workdir))
}

/// Kanonik ana makine dizini.
fn host_dir(dir: &Path) -> Result<PathBuf, ExitCode> {
    host_absolute(dir).map_err(|e| mount_error(dir, &e))
}

fn mount_error(dir: &Path, err: &std::io::Error) -> ExitCode {
    eprintln!(
        "{}",
        lstr!(
            en: "error: cannot share '{}' with the Docker container: {err}", dir.display();
            tr: "hata: '{}' Docker konteyneriyle paylaşılamıyor: {err}", dir.display()
        )
    );
    ExitCode::from(3)
}

fn unmappable(path: &Path) -> ExitCode {
    let err = std::io::Error::new(std::io::ErrorKind::Unsupported, "no container path");
    mount_error(path, &err)
}

/// Konteyner yolu → ana makine yolu (stdout ve stderr).
fn translate(output: Output, mounts: &Mounts) -> Output {
    let map = |bytes: &[u8]| {
        mounts
            .host_text(&String::from_utf8_lossy(bytes))
            .into_bytes()
    };
    Output {
        status: output.status,
        stdout: map(&output.stdout),
        stderr: map(&output.stderr),
    }
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn print_log_tail(log: &str) {
    let tail: Vec<&str> = log.lines().rev().take(LOG_TAIL_LINES).collect();
    for line in tail.iter().rev() {
        eprintln!("{line}");
    }
}

/// Testbench'e gömülecek VCD yolu (`/` ayraçlı): yerelde ana makine
/// yolu, Docker'da konteyner yolu — dizini `extra` ile bağlanmalı
/// (`vcd_mount_dir`).
pub(super) fn vcd_path_for(runner: &Runner, host_vcd: &Path) -> Result<String, ExitCode> {
    match runner {
        Runner::Local(_) => Ok(c_path(host_vcd)),
        Runner::Docker(_) => {
            let dir = vcd_mount_dir(host_vcd);
            let name = host_vcd.file_name().unwrap_or_default();
            let full = host_dir(&dir)?.join(name);
            container_path(&full).ok_or_else(|| unmappable(&full))
        }
    }
}

/// VCD'nin konteynere bağlanan dizini; yoksa oluşturulur (Docker onu
/// kök sahipli oluştururdu).
pub(super) fn vcd_mount_dir(host_vcd: &Path) -> PathBuf {
    let dir = match host_vcd.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Komutu koşturur; başlatılamazsa iletiyi basar (çıkış kodu 3).
fn run_tool(mut cmd: Command, program: &Path) -> Result<Output, ExitCode> {
    cmd.output().map_err(|err| {
        eprintln!(
            "{}",
            lstr!(
                en: "error: cannot run '{}': {}", program.display(), err;
                tr: "hata: '{}' çalıştırılamadı: {}", program.display(), err
            )
        );
        ExitCode::from(3)
    })
}

/// C string'ine gömülecek yol: ters bölüler öne çevrilir.
pub(super) fn c_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job<'a>(inputs: &'a [String], trace: bool) -> VerilateJob<'a> {
        VerilateJob {
            sim_dir: Path::new("build/sim/x_test"),
            inputs,
            tb_file: "tb_X.cpp",
            module: "X",
            trace,
            mdir: "obj_x",
        }
    }

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn c_path_uses_forward_slashes() {
        assert_eq!(c_path(Path::new("a\\b\\c.vcd")), "a/b/c.vcd");
    }

    #[test]
    fn command_puts_vlt_before_sv_and_keeps_argument_order() {
        // Arrange
        let inputs = ["load_X.vlt".to_string(), "X.sv".to_string()];

        // Act
        let cmd = job(&inputs, false).command(Path::new("verilator"));

        // Assert
        assert_eq!(
            args_of(&cmd),
            [
                "--cc",
                "load_X.vlt",
                "X.sv",
                "--exe",
                "tb_X.cpp",
                "--build",
                "--top-module",
                "X",
                "-Mdir",
                "obj_x"
            ]
        );
        assert_eq!(cmd.get_current_dir(), Some(Path::new("build/sim/x_test")));
    }

    #[test]
    fn command_appends_trace_flag_last() {
        let inputs = ["X.sv".to_string()];
        let cmd = job(&inputs, true).command(Path::new("verilator"));
        assert_eq!(args_of(&cmd).last().map(String::as_str), Some("--trace"));
    }

    #[test]
    fn exe_path_falls_back_to_extensionless_name() {
        // Dizin yok: canonicalize düşer, .exe de yok → V<modul>.
        let inputs = ["X.sv".to_string()];
        let exe = job(&inputs, false).exe_path();
        assert!(exe.ends_with(Path::new("obj_x").join("VX")));
    }

    #[test]
    fn verilate_fails_when_tool_cannot_start() {
        let inputs = ["X.sv".to_string()];
        let mut j = job(&inputs, false);
        let here = std::env::temp_dir();
        j.sim_dir = &here;
        let runner = Runner::Local(PathBuf::from("volt-no-such-verilator-binary"));
        assert!(verilate(&runner, &j, &[]).is_err());
    }
}
