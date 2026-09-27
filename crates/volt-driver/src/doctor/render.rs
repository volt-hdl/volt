//! `volt doctor` çıktısı: insan biçimi (stdout, iki dil) ve
//! `--format=json` (`volt-doctor/1`, CI ve LSP için; metin içermez,
//! yalnız yapı — dil bağımsız).

use std::fmt::Write as _;

use volt_diagnostics::explain::topics::install_steps;
use volt_diagnostics::lstr;
use volt_hir::SearchStop;
use volt_tools::Tool;

use super::{CapId, CapStatus, DockerDaemon, Report, SolverState, ToolReport, ToolStatus, SOLVERS};

/// İnsan raporundaki araç adı.
fn label(tool: Tool) -> String {
    match tool {
        Tool::Cc => return lstr!(en: "C compiler"; tr: "C derleyicisi"),
        Tool::Cxx => return lstr!(en: "C++ compiler"; tr: "C++ derleyicisi"),
        _ => {}
    }
    match tool {
        Tool::Verilator => "Verilator",
        Tool::Sby => "sby",
        Tool::Yosys => "Yosys",
        Tool::Boolector => "boolector",
        Tool::Bitwuzla => "bitwuzla",
        Tool::Yices => "yices",
        Tool::Z3 => "z3",
        Tool::OpenSta => "OpenSTA",
        Tool::Rustc => "rustc",
        Tool::Make => "make",
        Tool::Docker => "Docker",
        Tool::Cc | Tool::Cxx => unreachable!("yukarıda döndü"),
    }
    .to_string()
}

/// Kurulum ipucunun paragraf başlığı (`explain` konusundaki "Linux:" ...).
fn os_label() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "macOS",
        _ => "Linux",
    }
}

fn mark(status: CapStatus, optional: bool) -> &'static str {
    match status {
        CapStatus::Ok => "✓",
        CapStatus::Degraded => "!",
        CapStatus::Missing if optional => "-",
        CapStatus::Missing => "✗",
    }
}

/// "Verilator 5.050" ya da sürümsüz "Verilator".
fn found(r: &ToolReport) -> String {
    match &r.version {
        Some(v) => format!("{} {v}", label(r.tool)),
        None => label(r.tool),
    }
}

/// Eksik/sorunlu aracın tek cümlelik açıklaması.
fn problem(r: &ToolReport, secs: u64) -> String {
    let name = label(r.tool);
    match &r.status {
        ToolStatus::Ok => found(r),
        ToolStatus::Missing => lstr!(en: "{name} not found"; tr: "{name} bulunamadı"),
        ToolStatus::Unresponsive => lstr!(
            en: "{name} did not answer within {secs} s (unresponsive)";
            tr: "{name} {secs} sn içinde yanıt vermedi"
        ),
        ToolStatus::Broken(reason) => lstr!(
            en: "{name} cannot start: {reason}";
            tr: "{name} başlatılamıyor: {reason}"
        ),
        ToolStatus::TooOld(min) => {
            let v = r.version.as_deref().unwrap_or("?");
            lstr!(
                en: "{name} {v} is older than {min} (recommended minimum)";
                tr: "{name} {v}, {min} sürümünden eski (önerilen en düşük)"
            )
        }
    }
}

fn cap_title(cap: CapId) -> String {
    match cap {
        CapId::Timing => lstr!(en: "timing (optional)"; tr: "zamanlama (isteğe bağlı)"),
        CapId::Drivers => lstr!(
            en: "driver checks (optional)";
            tr: "sürücü denetimi (isteğe bağlı)"
        ),
        _ => cap.commands().join(", "),
    }
}

pub(super) fn human(report: &Report) -> String {
    let mut out = format!(
        "volt {} ({}-{})\n\n",
        volt_sv_emit::VOLT_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for cap in CapId::ALL {
        capability(&mut out, report, cap);
    }
    docker(&mut out, report);
    project(&mut out, report);
    out
}

fn capability(out: &mut String, report: &Report, cap: CapId) {
    let status = report.cap_status(cap);
    let secs = report.timeout.as_secs();
    let tools: Vec<&ToolReport> = cap.tools().iter().map(|t| report.tool(*t)).collect();
    let detail = match cap {
        CapId::Core => lstr!(en: "no external tools needed"; tr: "dış araç gerekmez"),
        _ if status == CapStatus::Missing => missing_detail(report, cap, &tools),
        _ => {
            let mut parts: Vec<String> = tools
                .iter()
                .map(|r| if r.ok() { found(r) } else { problem(r, secs) })
                .collect();
            if cap == CapId::Verify {
                parts.push(match report.solver_state() {
                    SolverState::Alternative(alt) => {
                        let alt = alt.name();
                        lstr!(
                            en: "default solver boolector not found, use --engine {alt}";
                            tr: "varsayılan çözücü boolector bulunamadı, --engine {alt} kullanın"
                        )
                    }
                    _ => found(report.tool(Tool::Boolector)),
                });
            }
            parts.join(", ")
        }
    };
    let _ = writeln!(
        out,
        "{} {} — {detail}",
        mark(status, cap.optional()),
        cap_title(cap)
    );
    if cap == CapId::Verify && status != CapStatus::Missing {
        optional_solvers(out, report);
    }
    if status != CapStatus::Ok {
        if let Some(topic) = cap.setup_topic() {
            install_hint(out, topic);
        }
    }
}

/// Eksik yeteneğin açıklaması: bulunamayanlar tek ifadede, diğer
/// sorunlar ayrı, bulunanlar parantezde; isteğe bağlı yeteneğin amacı.
fn missing_detail(report: &Report, cap: CapId, tools: &[&ToolReport]) -> String {
    let secs = report.timeout.as_secs();
    let mut parts = Vec::new();
    let absent: Vec<String> = tools
        .iter()
        .filter(|r| r.status == ToolStatus::Missing)
        .map(|r| label(r.tool))
        .collect();
    if !absent.is_empty() {
        let names = absent.join(", ");
        parts.push(lstr!(en: "{names} not found"; tr: "{names} bulunamadı"));
    }
    parts.extend(
        tools
            .iter()
            .filter(|r| !matches!(r.status, ToolStatus::Ok | ToolStatus::Missing))
            .map(|r| problem(r, secs)),
    );
    if cap == CapId::Verify && report.solver_state() == SolverState::None {
        parts.push(lstr!(
            en: "no SMT solver found (boolector, bitwuzla, yices, z3)";
            tr: "SMT çözücü bulunamadı (boolector, bitwuzla, yices, z3)"
        ));
    }
    let mut text = parts.join("; ");
    let present: Vec<String> = tools.iter().filter(|r| r.ok()).map(|r| found(r)).collect();
    if !present.is_empty() {
        let list = present.join(", ");
        text.push_str(&lstr!(en: " (found: {list})"; tr: " (bulunan: {list})"));
    }
    let purpose = match cap {
        CapId::Timing => Some(lstr!(
            en: "checks generated .sdc files (ADR-0065)";
            tr: "üretilen .sdc dosyalarını denetler (ADR-0065)"
        )),
        CapId::Drivers => Some(lstr!(
            en: "compiles drivers from --emit=c,rust (ADR-0053)";
            tr: "--emit=c,rust sürücülerini derler (ADR-0053)"
        )),
        _ => None,
    };
    if let Some(purpose) = purpose {
        text.push_str(&format!("; {purpose}"));
    }
    text
}

/// Varsayılanın yanında bulunan ya da önerilen çözücüler (ADR-0082).
fn optional_solvers(out: &mut String, report: &Report) {
    let also: Vec<String> = SOLVERS[1..]
        .iter()
        .map(|t| report.tool(*t))
        .filter(|r| r.ok())
        .map(found)
        .collect();
    if !also.is_empty() {
        let list = also.join(", ");
        let _ = writeln!(
            out,
            "    {}",
            lstr!(en: "also: {list}"; tr: "ayrıca: {list}")
        );
    }
    if !report.tool(Tool::Bitwuzla).ok() {
        let _ = writeln!(
            out,
            "    {}",
            lstr!(
                en: "(optional bitwuzla not found — often fastest on large designs, ADR-0082)";
                tr: "(isteğe bağlı bitwuzla bulunamadı — büyük tasarımlarda çoğu zaman en hızlısı, ADR-0082)"
            )
        );
    }
}

/// `volt explain <konu>` INSTALL bölümünün bu işletim sistemine düşen
/// satırları — metin konuda yaşar, burada kopyası yok (ADR-0084 §2).
fn install_hint(out: &mut String, topic: &str) {
    let os = os_label();
    let steps = install_steps(topic, volt_diagnostics::lang(), os);
    let head = lstr!(en: "install ({os}):"; tr: "kurulum ({os}):");
    for (i, step) in steps.iter().enumerate() {
        let lead = if i == 0 { head.as_str() } else { "" };
        let _ = writeln!(out, "    {lead:<w$} {step}", w = head.chars().count());
    }
    let _ = writeln!(
        out,
        "    {} volt explain {topic}",
        lstr!(en: "see:"; tr: "bkz.:")
    );
}

fn docker(out: &mut String, report: &Report) {
    let r = report.tool(Tool::Docker);
    let version = r.version.clone().unwrap_or_default();
    let secs = report.timeout.as_secs();
    let (mark, text) = match (&r.status, &report.docker_daemon) {
        (ToolStatus::Missing, _) => (
            "-",
            lstr!(
                en: "not found (optional: runs Verilator/sby in a container on Windows and macOS)";
                tr: "bulunamadı (isteğe bağlı: Windows ve macOS'ta Verilator/sby'yi konteynerde çalıştırır)"
            ),
        ),
        (ToolStatus::Ok, Some(DockerDaemon::Running(_))) => (
            "✓",
            lstr!(en: "{version}, daemon running"; tr: "{version}, daemon çalışıyor"),
        ),
        (ToolStatus::Ok, Some(DockerDaemon::Unresponsive)) => (
            "!",
            lstr!(
                en: "{version} installed, daemon did not answer within {secs} s";
                tr: "{version} kurulu, daemon {secs} sn içinde yanıt vermedi"
            ),
        ),
        (ToolStatus::Ok, _) => (
            "!",
            lstr!(
                en: "{version} installed, daemon not running (start Docker Desktop or the docker service)";
                tr: "{version} kurulu, daemon çalışmıyor (Docker Desktop'ı ya da docker servisini başlatın)"
            ),
        ),
        _ => ("!", problem(r, secs)),
    };
    let _ = writeln!(out, "{mark} docker — {text}");
}

fn project(out: &mut String, report: &Report) {
    match &report.project.manifest {
        Ok(dir) => {
            let dir = dir.display();
            let _ = writeln!(
                out,
                "✓ {}",
                lstr!(en: "project — Volt.toml in {dir}"; tr: "proje — Volt.toml: {dir}")
            );
        }
        Err(stop) => {
            let at = match stop {
                SearchStop::Override(d) => lstr!(
                    en: "VOLT_MANIFEST_DIR points to {}", d.display();
                    tr: "VOLT_MANIFEST_DIR {} dizinini gösteriyor", d.display()
                ),
                SearchStop::GitRoot(d) => lstr!(
                    en: "search stopped at the git root {}", d.display();
                    tr: "arama git kökünde durdu: {}", d.display()
                ),
                SearchStop::Home(d) => lstr!(
                    en: "search stopped at the home directory {}", d.display();
                    tr: "arama ev dizininde durdu: {}", d.display()
                ),
                SearchStop::Exhausted => lstr!(
                    en: "searched up to the filesystem root";
                    tr: "dosya sistemi köküne kadar arandı"
                ),
            };
            let _ = writeln!(
                out,
                "- {}",
                lstr!(
                    en: "project — no Volt.toml ({at}); single .volt files still work";
                    tr: "proje — Volt.toml yok ({at}); tek .volt dosyaları yine çalışır"
                )
            );
        }
    }
    if report.project.wsl_mount {
        let _ = writeln!(
            out,
            "! {}",
            lstr!(
                en: "project — running under /mnt/<drive> in WSL; build I/O is slow (move the project into the Linux file system, e.g. ~/)";
                tr: "proje — WSL'de /mnt/<sürücü> altında; derleme G/Ç'si yavaş (projeyi Linux dosya sistemine, ör. ~/ altına taşıyın)"
            )
        );
    }
}

// ═══ JSON ══════════════════════════════════════════════════════════

fn status_name(status: &ToolStatus) -> &'static str {
    match status {
        ToolStatus::Ok => "ok",
        ToolStatus::Missing => "missing",
        ToolStatus::Unresponsive => "unresponsive",
        ToolStatus::Broken(_) => "broken",
        ToolStatus::TooOld(_) => "too_old",
    }
}

fn cap_status_name(status: CapStatus) -> &'static str {
    match status {
        CapStatus::Ok => "ok",
        CapStatus::Degraded => "degraded",
        CapStatus::Missing => "missing",
    }
}

pub(super) fn json(report: &Report) -> String {
    use serde_json::{json, Value};
    let path = |p: &std::path::Path| Value::String(p.display().to_string());
    let tools: Vec<Value> = report
        .tools
        .iter()
        .map(|r| {
            json!({
                "name": r.tool.name(),
                "status": status_name(&r.status),
                "path": r.path.as_deref().map(path),
                "version": r.version,
                "min_version": super::min_version(r.tool),
            })
        })
        .collect();
    let caps: Vec<Value> = CapId::ALL
        .iter()
        .map(|c| {
            json!({
                "id": c.name(),
                "commands": c.commands(),
                "optional": c.optional(),
                "status": cap_status_name(report.cap_status(*c)),
                "tools": c.tools().iter().map(|t| t.name()).collect::<Vec<_>>(),
                "setup_topic": c.setup_topic(),
            })
        })
        .collect();
    let solver = match report.solver_state() {
        SolverState::Default => Some(Tool::Boolector.name()),
        SolverState::Alternative(t) => Some(t.name()),
        SolverState::None => None,
    };
    let daemon = match &report.docker_daemon {
        None => json!(null),
        Some(DockerDaemon::Running(v)) => json!({"status": "running", "version": v}),
        Some(DockerDaemon::NotRunning) => json!({"status": "not_running", "version": null}),
        Some(DockerDaemon::Unresponsive) => json!({"status": "unresponsive", "version": null}),
    };
    let (manifest_dir, stop, stop_dir) = match &report.project.manifest {
        Ok(d) => (Some(path(d)), None, None),
        Err(SearchStop::Override(d)) => (None, Some("override"), Some(path(d))),
        Err(SearchStop::GitRoot(d)) => (None, Some("git_root"), Some(path(d))),
        Err(SearchStop::Home(d)) => (None, Some("home"), Some(path(d))),
        Err(SearchStop::Exhausted) => (None, Some("exhausted"), None),
    };
    let doc = json!({
        "schema": "volt-doctor/1",
        "volt_version": volt_sv_emit::VOLT_VERSION,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "timeout_secs": report.timeout.as_secs(),
        "required_ok": report.all_required_ok(),
        "capabilities": caps,
        "solver": solver,
        "tools": tools,
        "docker_daemon": daemon,
        "project": {
            "manifest_dir": manifest_dir,
            "search_stop": stop,
            "stop_dir": stop_dir,
            "wsl_mount": report.project.wsl_mount,
        },
    });
    serde_json::to_string_pretty(&doc).expect("doctor JSON")
}
