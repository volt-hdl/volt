//! Verilator/sby nerede koşar: yerel araç ya da Docker köprüsü (ADR-0094).
//!
//! Sıra: `VOLT_TOOL_BACKEND` (auto | local | docker) → yerel araç
//! (`volt-tools` araması, değişmedi) → yoksa çalışan bir Docker daemon'u ve
//! sabitlenmiş imaj. Yerel araç bulunduğunda bu modül HİÇBİR ŞEY basmaz —
//! kurulu araçla çıktı bayt bayt eskisidir. Docker'a geçildiğinde tek
//! satırlık bilgi iletisi basılır (hangi imaj, neden); sessiz geçiş yok.
//! İmaj yoksa boyutu önceden, indirme süresi sonra bildirilir.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use volt_diagnostics::lstr;
use volt_tools::docker::{self, Daemon, Image, Preference, Run};
use volt_tools::docker_paths::Mounts;
use volt_tools::Tool;

/// `docker info` ve `docker image inspect` için zaman sınırı: Docker
/// Desktop açılırken daemon birkaç saniye yanıtsız kalabilir.
const DOCKER_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// Aracın çalışacağı yer.
#[derive(Debug)]
pub(crate) enum Runner {
    Local(PathBuf),
    Docker(DockerTool),
}

/// Docker'da koşan araç: `docker` yürütülebiliri ve imaj.
#[derive(Debug)]
pub(crate) struct DockerTool {
    pub docker: PathBuf,
    pub image: &'static Image,
}

impl DockerTool {
    /// `docker run` komutu. `owner_dir` Volt'un oluşturduğu çıktı
    /// dizinidir; Linux'ta konteyner onun sahibiyle koşar.
    pub(crate) fn command(
        &self,
        mounts: &Mounts,
        workdir: String,
        entrypoint: Option<String>,
        name: Option<String>,
        owner_dir: &Path,
        args: Vec<std::ffi::OsString>,
    ) -> Command {
        Run {
            image: self.image,
            mounts,
            workdir,
            entrypoint,
            user: docker::owner_of(owner_dir),
            name,
            args,
        }
        .command(&self.docker)
    }
}

/// Aracı çözer. `not_found` yerel aracın eski kurulum yardımını basar
/// (Docker de kullanılamıyorsa gösterilir). Hata çıkış kodu 3 (araç yok),
/// geçersiz `VOLT_TOOL_BACKEND` 2 (kullanım hatası).
pub(crate) fn resolve(tool: Tool, not_found: &dyn Fn()) -> Result<Runner, ExitCode> {
    let pref = Preference::from_process().map_err(|msg| {
        eprintln!("{}", lstr!(en: "error: {msg}"; tr: "hata: {msg}"));
        ExitCode::from(2)
    })?;
    if pref != Preference::Docker {
        if let Some(path) = volt_tools::find(tool) {
            return Ok(Runner::Local(path));
        }
        if pref == Preference::Local {
            not_found();
            return Err(ExitCode::from(3));
        }
    }
    let image = volt_tools::docker::image_for(tool).expect("köprülü araç");
    let label = tool_label(tool);
    let Some(docker) = volt_tools::find(Tool::Docker) else {
        if pref == Preference::Docker {
            print_docker_required(label);
        } else {
            not_found();
        }
        return Err(ExitCode::from(3));
    };
    match docker::daemon(&docker, DOCKER_PROBE_TIMEOUT) {
        Daemon::Running(_) => {}
        state => {
            print_daemon_down(label, &state, pref);
            return Err(ExitCode::from(3));
        }
    }
    print_using_docker(label, image, pref);
    if !docker::image_present(&docker, image, DOCKER_PROBE_TIMEOUT) {
        download(&docker, image, tool)?;
    }
    Ok(Runner::Docker(DockerTool { docker, image }))
}

fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Sby => "SymbiYosys",
        _ => "Verilator",
    }
}

fn setup_topic(tool: Tool) -> &'static str {
    match tool {
        Tool::Sby => "verify-setup",
        _ => "simulation-setup",
    }
}

/// Tek satırlık bilgi iletisi: hangi imaj ve neden.
fn print_using_docker(label: &str, image: &Image, pref: Preference) {
    let name = image.name;
    let env = Preference::ENV;
    let line = if pref == Preference::Docker {
        lstr!(
            en: "note: {env}=docker; running {label} in Docker ({name})";
            tr: "not: {env}=docker; {label} Docker'da çalıştırılıyor ({name})"
        )
    } else {
        lstr!(
            en: "note: {label} not found locally; running it in Docker ({name})";
            tr: "not: {label} yerelde bulunamadı; Docker'da çalıştırılıyor ({name})"
        )
    };
    eprintln!("{line}");
}

/// İlk kullanım: boyut önce, süre sonra.
fn download(docker: &Path, image: &Image, tool: Tool) -> Result<(), ExitCode> {
    let (name, mb) = (image.name, image.download_mb);
    eprintln!(
        "{}",
        lstr!(
            en: "note: downloading {name} (~{mb} MB, first use only; this can take several minutes)";
            tr: "not: {name} indiriliyor (~{mb} MB, yalnız ilk kullanımda; birkaç dakika sürebilir)"
        )
    );
    match docker::pull(docker, image) {
        Ok(elapsed) => {
            let took = human_duration(elapsed);
            eprintln!(
                "{}",
                lstr!(en: "note: downloaded {name} in {took}"; tr: "not: {name} {took} içinde indirildi")
            );
            Ok(())
        }
        Err(reason) => {
            let reference = image.reference();
            let topic = setup_topic(tool);
            eprintln!(
                "{}",
                lstr!(
                    en: "error: could not download the Docker image {name}\n  \
                         = reason: {reason}\n  \
                         = help: check the network connection and retry, or run 'docker pull {reference}'\n  \
                         = for more: volt explain {topic}";
                    tr: "hata: {name} Docker imajı indirilemedi\n  \
                         = neden: {reason}\n  \
                         = çözüm: ağ bağlantısını denetleyip yeniden deneyin ya da 'docker pull {reference}' çalıştırın\n  \
                         = daha fazla: volt explain {topic}"
                )
            );
            Err(ExitCode::from(3))
        }
    }
}

/// `1m 05s` ya da `8.4s`.
fn human_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

fn print_daemon_down(label: &str, state: &Daemon, pref: Preference) {
    let why = match state {
        Daemon::Unresponsive => {
            let secs = DOCKER_PROBE_TIMEOUT.as_secs();
            lstr!(
                en: "the Docker daemon did not answer within {secs} s";
                tr: "Docker daemon'u {secs} sn içinde yanıt vermedi"
            )
        }
        _ => lstr!(
            en: "Docker is installed but its daemon is not running";
            tr: "Docker kurulu ama daemon'u çalışmıyor"
        ),
    };
    let head = if pref == Preference::Docker {
        lstr!(en: "error: {label} cannot run in Docker: {why}"; tr: "hata: {label} Docker'da çalıştırılamıyor: {why}")
    } else {
        lstr!(en: "error: {label} not found locally, and {why}"; tr: "hata: {label} yerelde bulunamadı ve {why}")
    };
    eprintln!(
        "{head}\n  {}",
        lstr!(
            en: "= help: start Docker Desktop (or the docker service) and run the command again; \
                 Volt then runs {label} in a container";
            tr: "= çözüm: Docker Desktop'ı (ya da docker servisini) başlatıp komutu yeniden \
                 çalıştırın; Volt {label}'u konteynerde çalıştırır"
        )
    );
}

fn print_docker_required(label: &str) {
    let env = Preference::ENV;
    eprintln!(
        "{}",
        lstr!(
            en: "error: {env}=docker, but docker was not found\n  \
                 = help: install Docker Desktop, point VOLT_DOCKER at the docker executable, \
                 or unset {env} to use a local {label}";
            tr: "hata: {env}=docker ama docker bulunamadı\n  \
                 = çözüm: Docker Desktop'ı kurun, VOLT_DOCKER'ı docker yürütülebilirine \
                 yöneltin ya da yerel {label} için {env} değişkenini kaldırın"
        )
    );
}

/// Konteynerin kendi arızası (araç değil): bellek yetmedi (137) ya da
/// `docker run` konteyneri başlatamadı (125). Basıldıysa `true`.
pub(crate) fn report_container_failure(code: Option<i32>) -> bool {
    match code {
        Some(docker::OOM_EXIT_CODE) => {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: the Docker container ran out of memory (exit code 137)\n  \
                         = help: give Docker more memory (Docker Desktop: Settings > Resources) \
                         or lower -j";
                    tr: "hata: Docker konteynerinin belleği yetmedi (çıkış kodu 137)\n  \
                         = çözüm: Docker'a daha çok bellek verin (Docker Desktop: Settings > \
                         Resources) ya da -j değerini düşürün"
                )
            );
            true
        }
        Some(docker::DOCKER_RUN_ERROR) => {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: docker could not start the container (exit code 125; see the message above)";
                    tr: "hata: docker konteyneri başlatamadı (çıkış kodu 125; yukarıdaki iletiye bakın)"
                )
            );
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_as_minutes_or_seconds() {
        assert_eq!(human_duration(Duration::from_millis(8_400)), "8.4s");
        assert_eq!(human_duration(Duration::from_secs(711)), "11m 51s");
        assert_eq!(human_duration(Duration::from_secs(60)), "1m 00s");
    }

    #[test]
    fn container_failures_are_recognised_by_exit_code() {
        assert!(report_container_failure(Some(137)));
        assert!(report_container_failure(Some(125)));
        assert!(!report_container_failure(Some(1)));
        assert!(!report_container_failure(None));
    }
}
