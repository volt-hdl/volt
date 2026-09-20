//! Verilator köprüsü: aracın keşfi, `--cc --exe --build` çağrısı ve
//! üretilen yürütülebilirin koşturulması.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use volt_diagnostics::lstr;

/// Günlüğün başarısızlıkta basılan kuyruğu (satır).
const LOG_TAIL_LINES: usize = 30;

// ═══ Verilator keşfi ══════════════════════════════════════════════

/// Verilator'u bulur; yoksa kurulum yardımını basar (çıkış kodu 3).
pub(super) fn require_verilator(command: &str) -> Result<PathBuf, ExitCode> {
    find_verilator().ok_or_else(|| {
        print_verilator_not_found(command);
        ExitCode::from(3)
    })
}

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
    /// Verilator komut satırı (argüman sırası sabittir).
    fn command(&self, verilator: &Path) -> Command {
        let mut cmd = Command::new(verilator);
        cmd.arg("--cc");
        for input in self.inputs {
            cmd.arg(input);
        }
        cmd.arg("--exe")
            .arg(self.tb_file)
            .arg("--build")
            .arg("--top-module")
            .arg(self.module)
            .arg("-Mdir")
            .arg(self.mdir)
            .current_dir(self.sim_dir);
        if self.trace {
            cmd.arg("--trace");
        }
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
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let tail: Vec<&str> = log.lines().rev().take(LOG_TAIL_LINES).collect();
        for line in tail.iter().rev() {
            eprintln!("{line}");
        }
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
/// Başarıda üretilen yürütülebilirin mutlak yolunu döndürür.
pub(super) fn verilate(verilator: &Path, job: &VerilateJob<'_>) -> Result<PathBuf, ExitCode> {
    let output = run_tool(job.command(verilator), verilator)?;
    if !output.status.success() {
        job.print_failure(&output);
        return Err(ExitCode::from(3));
    }
    Ok(job.exe_path())
}

// ═══ Koşturma ═════════════════════════════════════════════════════

/// Üretilen simülasyon yürütülebilirini sim dizininde koşturur.
pub(super) fn run_simulation(exe: &Path, sim_dir: &Path) -> Result<Output, ExitCode> {
    let mut cmd = Command::new(exe);
    cmd.current_dir(sim_dir);
    run_tool(cmd, exe)
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
        assert!(verilate(Path::new("volt-no-such-verilator-binary"), &j).is_err());
    }
}
