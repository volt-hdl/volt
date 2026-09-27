//! `volt doctor` — kurulum teşhisi (ADR-0084 Bölüm 1).
//!
//! Kullanıcı eksik aracı `volt test`/`volt verify` hata verince değil,
//! kurulumdan hemen sonra öğrensin. Rapor araç listesi değil, YETENEK
//! listesidir: her satır hangi komutların çalışacağını söyler.
//!
//! Araçlar `volt-tools` ile bulunur — `volt test`, `volt verify` ve CI
//! testleri (`VOLT_REQUIRE_TOOLS`) ile AYNI arama; doctor'ın "bulundu"
//! dediği aracı komut da bulur. Sürüm sorguları paralel ve zaman
//! sınırlıdır; takılan araç "yanıt vermiyor" olur, rapor beklemez.
//!
//! Çıkış kodu (cli-contract.md §2, ADR-0084 §3): 0 — rapor üretildi,
//! eksik araç yalnız bildirilir (Volt tek başına build/check/explain
//! yapar). `--strict`: zorunlu bir yetenek tam değilse 3 — `volt test` ve
//! `volt verify`'ın eksik araç kodu; CI kurulum adımında kullanılır.

mod render;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use volt_tools::{Probe, Tool};

use crate::OutputFormat;

/// Asgari sürüm gerektiren araçlar (ADR-0084 §2). Yalnız kanıtı olanlar:
/// Volt'un testbench'i ve CI'ı Verilator 5.x kullanır, simulation-setup
/// konusu 5.x önerir. sby/Yosys için sürüm tabanı ölçülmedi, sürüm
/// yalnız raporlanır.
const MIN_VERSIONS: &[(Tool, &str)] = &[(Tool::Verilator, "5.0")];

/// Çözücüler, varsayılan önce (ADR-0082).
const SOLVERS: [Tool; 4] = [Tool::Boolector, Tool::Bitwuzla, Tool::Yices, Tool::Z3];

/// Tek aracın durumu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ToolStatus {
    Ok,
    Missing,
    /// Zaman sınırında yanıt vermedi.
    Unresponsive,
    /// Başlatılamadı (bozuk kurulum, izin).
    Broken(String),
    /// Asgari sürümün altında.
    TooOld(&'static str),
}

#[derive(Clone, Debug)]
pub(crate) struct ToolReport {
    pub tool: Tool,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub status: ToolStatus,
}

impl ToolReport {
    pub fn ok(&self) -> bool {
        self.status == ToolStatus::Ok
    }
}

/// Yetenek grupları (ADR-0084 §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CapId {
    /// build, check, check-regmap, explain, lsp — Volt tek başına.
    Core,
    /// test, run — Verilator + C++ derleyicisi + make.
    Simulation,
    /// verify — sby + Yosys + çözücü.
    Verify,
    /// Üretilen SDC'nin OpenSTA ile denetimi (isteğe bağlı).
    Timing,
    /// Üretilen C/Rust sürücülerinin derlenmesi (isteğe bağlı).
    Drivers,
}

impl CapId {
    pub const ALL: [CapId; 5] = [
        CapId::Core,
        CapId::Simulation,
        CapId::Verify,
        CapId::Timing,
        CapId::Drivers,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CapId::Core => "core",
            CapId::Simulation => "simulation",
            CapId::Verify => "verify",
            CapId::Timing => "timing",
            CapId::Drivers => "drivers",
        }
    }

    pub fn commands(self) -> &'static [&'static str] {
        match self {
            CapId::Core => &["build", "check", "explain"],
            CapId::Simulation => &["test", "run"],
            CapId::Verify => &["verify"],
            CapId::Timing | CapId::Drivers => &[],
        }
    }

    pub fn optional(self) -> bool {
        matches!(self, CapId::Timing | CapId::Drivers)
    }

    /// Zorunlu araçlar (çözücüler ayrıca, `solver_state`).
    pub fn tools(self) -> &'static [Tool] {
        match self {
            CapId::Core => &[],
            CapId::Simulation => &[Tool::Verilator, Tool::Cxx, Tool::Make],
            CapId::Verify => &[Tool::Sby, Tool::Yosys],
            CapId::Timing => &[Tool::OpenSta],
            CapId::Drivers => &[Tool::Cc, Tool::Rustc],
        }
    }

    /// Kurulum ipucunun geldiği `volt explain` konusu.
    pub fn setup_topic(self) -> Option<&'static str> {
        match self {
            CapId::Simulation => Some("simulation-setup"),
            CapId::Verify => Some("verify-setup"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CapStatus {
    Ok,
    /// Çalışır ama eksikle (ör. varsayılan çözücü yok, eski sürüm).
    Degraded,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DockerDaemon {
    Running(Option<String>),
    NotRunning,
    Unresponsive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectReport {
    /// `Volt.toml`'un dizini (ADR-0061 araması) ya da aramanın durduğu yer.
    pub manifest: Result<PathBuf, volt_hir::SearchStop>,
    /// WSL içinde `/mnt/<sürücü>` altında mı (yavaş 9P G/Ç)?
    pub wsl_mount: bool,
}

pub(crate) struct Report {
    pub tools: Vec<ToolReport>,
    pub docker_daemon: Option<DockerDaemon>,
    pub project: ProjectReport,
    pub timeout: Duration,
}

impl Report {
    pub fn tool(&self, tool: Tool) -> &ToolReport {
        self.tools
            .iter()
            .find(|r| r.tool == tool)
            .expect("her araç raporlanır")
    }

    pub fn cap_status(&self, cap: CapId) -> CapStatus {
        let tools: Vec<&ToolReport> = cap.tools().iter().map(|t| self.tool(*t)).collect();
        let usable = |r: &&ToolReport| matches!(r.status, ToolStatus::Ok | ToolStatus::TooOld(_));
        if !tools.iter().all(usable) {
            return CapStatus::Missing;
        }
        let mut degraded = tools.iter().any(|r| !r.ok());
        if cap == CapId::Verify {
            match self.solver_state() {
                SolverState::Default => {}
                SolverState::Alternative(_) => degraded = true,
                SolverState::None => return CapStatus::Missing,
            }
        }
        if degraded {
            CapStatus::Degraded
        } else {
            CapStatus::Ok
        }
    }

    /// Varsayılan çözücü var mı, yoksa hangi seçenek çalışır?
    pub fn solver_state(&self) -> SolverState {
        if self.tool(Tool::Boolector).ok() {
            return SolverState::Default;
        }
        match SOLVERS.iter().find(|t| self.tool(**t).ok()) {
            Some(t) => SolverState::Alternative(*t),
            None => SolverState::None,
        }
    }

    /// `--strict`: zorunlu yeteneklerin hepsi tam mı?
    pub fn all_required_ok(&self) -> bool {
        CapId::ALL
            .iter()
            .filter(|c| !c.optional())
            .all(|c| self.cap_status(*c) == CapStatus::Ok)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SolverState {
    Default,
    Alternative(Tool),
    None,
}

/// `volt doctor` seçenekleri.
pub(crate) struct DoctorOptions {
    pub format: OutputFormat,
    pub strict: bool,
    pub timeout: Duration,
}

pub(crate) fn doctor(opts: DoctorOptions) -> ExitCode {
    let report = collect(opts.timeout);
    match opts.format {
        OutputFormat::Json => println!("{}", render::json(&report)),
        OutputFormat::Human | OutputFormat::Short => print!("{}", render::human(&report)),
    }
    if opts.strict && !report.all_required_ok() {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    }
}

/// Araçları bulur ve sürümlerini paralel sorar.
fn collect(timeout: Duration) -> Report {
    let (tools, docker_daemon) = std::thread::scope(|scope| {
        let handles: Vec<_> = Tool::ALL
            .iter()
            .map(|tool| scope.spawn(move || tool_report(*tool, timeout)))
            .collect();
        let daemon = scope.spawn(move || {
            volt_tools::find(Tool::Docker).map(|docker| docker_daemon(&docker, timeout))
        });
        let tools: Vec<ToolReport> = handles
            .into_iter()
            .map(|h| h.join().expect("sonda iş parçacığı"))
            .collect();
        (tools, daemon.join().expect("docker sondası"))
    });
    Report {
        tools,
        docker_daemon,
        project: project_report(),
        timeout,
    }
}

fn tool_report(tool: Tool, timeout: Duration) -> ToolReport {
    let Some(path) = volt_tools::find(tool) else {
        return ToolReport {
            tool,
            path: None,
            version: None,
            status: ToolStatus::Missing,
        };
    };
    let (version, status) = match volt_tools::probe_version(&path, tool.version_args(), timeout) {
        Probe::Ran { version, .. } => {
            let status = match min_version(tool) {
                Some(min) if version.as_deref().is_some_and(|v| older_than(v, min)) => {
                    ToolStatus::TooOld(min)
                }
                _ => ToolStatus::Ok,
            };
            (version, status)
        }
        Probe::FailedToStart(reason) => (None, ToolStatus::Broken(reason)),
        Probe::Unresponsive => (None, ToolStatus::Unresponsive),
    };
    ToolReport {
        tool,
        path: Some(path),
        version,
        status,
    }
}

pub(crate) fn min_version(tool: Tool) -> Option<&'static str> {
    MIN_VERSIONS
        .iter()
        .find(|(t, _)| *t == tool)
        .map(|(_, v)| *v)
}

/// Noktalı sürüm karşılaştırması; sayı olmayan parça 0 sayılır.
pub(crate) fn older_than(version: &str, min: &str) -> bool {
    let parts = |s: &str| -> Vec<u64> { s.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (v, m) = (parts(version), parts(min));
    for i in 0..v.len().max(m.len()) {
        let (a, b) = (
            v.get(i).copied().unwrap_or(0),
            m.get(i).copied().unwrap_or(0),
        );
        if a != b {
            return a < b;
        }
    }
    false
}

/// `docker info` daemon'a bağlanır; daemon kapalıysa hata verir, Docker
/// Desktop açılırken takılabilir (zaman sınırı).
fn docker_daemon(docker: &Path, timeout: Duration) -> DockerDaemon {
    match volt_tools::probe_version(docker, &["info", "--format", "{{.ServerVersion}}"], timeout) {
        Probe::Ran {
            success: true,
            output,
            ..
        } => DockerDaemon::Running(
            output
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(str::to_string),
        ),
        Probe::Ran { .. } | Probe::FailedToStart(_) => DockerDaemon::NotRunning,
        Probe::Unresponsive => DockerDaemon::Unresponsive,
    }
}

fn project_report() -> ProjectReport {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let osrelease = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok();
    ProjectReport {
        manifest: volt_hir::find_manifest_dir(&cwd),
        wsl_mount: wsl_mount(
            &cwd,
            osrelease.as_deref(),
            std::env::var_os("WSL_DISTRO_NAME").is_some(),
        ),
    }
}

/// WSL'de Windows sürücüsü (`/mnt/c/...`) üzerinde mi? Oradaki G/Ç 9P
/// üzerinden gider; Verilator derlemesi ve `build/` yazımı Linux dosya
/// sistemindekinden kat kat yavaştır.
pub(crate) fn wsl_mount(cwd: &Path, osrelease: Option<&str>, distro_env: bool) -> bool {
    let in_wsl =
        distro_env || osrelease.is_some_and(|r| r.to_ascii_lowercase().contains("microsoft"));
    let text = cwd.to_string_lossy();
    let on_mount = text
        .strip_prefix("/mnt/")
        .is_some_and(|rest| rest.split('/').next().is_some_and(|d| d.len() == 1));
    in_wsl && on_mount
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(older_than("4.228", "5.0"));
        assert!(!older_than("5.0", "5.0"));
        assert!(!older_than("5.050", "5.0"));
        assert!(older_than("0.9", "0.10"));
        assert!(!older_than("10.1", "9.9"));
    }

    #[test]
    fn wsl_mount_needs_wsl_and_a_drive_mount() {
        let wsl = Some("5.15.153.1-microsoft-standard-WSL2");
        assert!(wsl_mount(Path::new("/mnt/c/Users/me/proj"), wsl, false));
        assert!(wsl_mount(Path::new("/mnt/d"), None, true));
        assert!(!wsl_mount(Path::new("/home/me/proj"), wsl, true));
        assert!(!wsl_mount(Path::new("/mnt/data/proj"), wsl, false));
        assert!(!wsl_mount(
            Path::new("/mnt/c/proj"),
            Some("6.8.0-45-generic"),
            false
        ));
    }
}
