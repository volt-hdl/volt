//! Docker köprüsü — eksik Verilator/sby'yi sabitlenmiş imajda koşturmak
//! (ADR-0094).
//!
//! Bu modül dil bağımsız mekanizmadır: imaj sabitleri, seçim tercihi
//! (`VOLT_TOOL_BACKEND`), daemon ve imaj yoklaması, `docker run` komut
//! satırı. Kullanıcıya giden iletiler sürücüdedir (`lstr!`).

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::docker_paths::Mounts;
use crate::{probe_version, Probe, Tool};

/// Sabitlenmiş, genel erişimli bir araç imajı.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Image {
    /// Kullanıcıya gösterilen ad (`depo:etiket`).
    pub name: &'static str,
    /// İçerik özeti: etiket taşınsa da aynı imaj çekilir.
    pub digest: &'static str,
    /// Ölçülen indirme boyutu (sıkıştırılmış katmanlar, amd64), MB.
    pub download_mb: u32,
    /// İmajdaki araç sürümleri (ölçüm, ADR-0094 §2) — `volt doctor`.
    pub contents: &'static str,
}

impl Image {
    /// `docker run`/`pull`'a verilen başvuru: `ad@özet`.
    pub fn reference(&self) -> String {
        format!("{}@{}", self.name, self.digest)
    }
}

/// `volt test` / `volt run`: Verilator + g++ + make (Ubuntu 24.04).
pub const SIMULATION_IMAGE: Image = Image {
    name: "verilator/verilator:v5.052",
    digest: "sha256:a5b73e2fce0b2c483396f3800940f33b6faa802331a061c21694dd27b7352120",
    download_mb: 250,
    contents: "Verilator 5.052, g++ 13.3",
};

/// `volt verify`: Yosys + sby + boolector/yices/z3 (bitwuzla YOK).
pub const FORMAL_IMAGE: Image = Image {
    name: "hdlc/formal:all",
    digest: "sha256:d0852c894d13e634c8dd82eaafbd727b450103a12d92c38c763b4f80f75c1bb3",
    download_mb: 404,
    contents: "Yosys 0.66, SBY 0.69, boolector 3.2.4, yices 2.7.0, z3 4.15.0",
};

/// `FORMAL_IMAGE`'daki çözücüler (ölçüm); varsayılan boolector önde.
pub const FORMAL_IMAGE_SOLVERS: [Tool; 3] = [Tool::Boolector, Tool::Yices, Tool::Z3];

/// Aracın Docker'da koştuğu imaj; köprüsü olmayan araç `None`.
pub fn image_for(tool: Tool) -> Option<&'static Image> {
    match tool {
        Tool::Verilator => Some(&SIMULATION_IMAGE),
        Tool::Sby => Some(&FORMAL_IMAGE),
        _ => None,
    }
}

/// Araç nerede koşar (`VOLT_TOOL_BACKEND`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    /// Yerel araç varsa o, yoksa Docker (varsayılan).
    Auto,
    /// Yalnız yerel araç; Docker'a hiç geçilmez.
    Local,
    /// Yerel araç kurulu olsa da Docker.
    Docker,
}

impl Preference {
    pub const ENV: &'static str = "VOLT_TOOL_BACKEND";

    /// Değeri ayrıştırır; boş değer `Auto`dur.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Preference::Auto),
            "local" => Ok(Preference::Local),
            "docker" => Ok(Preference::Docker),
            other => Err(format!(
                "{}: unknown value '{other}' (expected auto, local or docker)",
                Self::ENV
            )),
        }
    }

    /// Süreç ortamından.
    pub fn from_process() -> Result<Self, String> {
        Self::parse(&std::env::var(Self::ENV).unwrap_or_default())
    }
}

/// Docker daemon'unun durumu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Daemon {
    /// Sunucu sürümüyle.
    Running(Option<String>),
    NotRunning,
    /// Zaman sınırında yanıt vermedi (Docker Desktop açılıyor olabilir).
    Unresponsive,
}

/// `docker info` daemon'a bağlanır; daemon kapalıysa hata verir.
pub fn daemon(docker: &Path, timeout: Duration) -> Daemon {
    match probe_version(docker, &["info", "--format", "{{.ServerVersion}}"], timeout) {
        Probe::Ran {
            success: true,
            output,
            ..
        } => Daemon::Running(
            output
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(str::to_string),
        ),
        Probe::Ran { .. } | Probe::FailedToStart(_) => Daemon::NotRunning,
        Probe::Unresponsive => Daemon::Unresponsive,
    }
}

/// İmaj yerelde var mı (`docker image inspect`)?
pub fn image_present(docker: &Path, image: &Image, timeout: Duration) -> bool {
    matches!(
        probe_version(
            docker,
            &[
                "image",
                "inspect",
                "--format",
                "{{.Id}}",
                &image.reference()
            ],
            timeout
        ),
        Probe::Ran { success: true, .. }
    )
}

/// İmajı indirir; süreyi ya da Docker'ın son hata satırını döndürür.
pub fn pull(docker: &Path, image: &Image) -> Result<Duration, String> {
    let start = Instant::now();
    let output = Command::new(docker)
        .args(["pull", "-q", &image.reference()])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(start.elapsed());
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Err(text
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("docker pull failed")
        .to_string())
}

/// Konteyner süreci belleği aşınca çekirdeğin öldürdüğü kod (128 + 9).
pub const OOM_EXIT_CODE: i32 = 137;

/// `docker run`'ın kendi hatası (konteyner başlamadı).
pub const DOCKER_RUN_ERROR: i32 = 125;

/// Tek `docker run` çağrısı.
#[derive(Clone, Debug)]
pub struct Run<'a> {
    pub image: &'a Image,
    pub mounts: &'a Mounts,
    /// Konteynerdeki çalışma dizini.
    pub workdir: String,
    /// İmajın giriş noktası yerine koşan program.
    pub entrypoint: Option<String>,
    /// Üretilen dosyaların sahibi (Linux): `--user uid:gid`.
    pub user: Option<(u32, u32)>,
    /// Konteyner adı — erken sonlandırmada `docker rm -f` için.
    pub name: Option<String>,
    pub args: Vec<OsString>,
}

impl Run<'_> {
    /// `docker` programına verilecek argümanlar (sıra sabittir).
    pub fn docker_args(&self) -> Vec<OsString> {
        let mut out: Vec<OsString> = ["run", "--rm", "--init", "--network", "none"]
            .iter()
            .map(OsString::from)
            .collect();
        // Kök olmayan kullanıcının yazılabilir bir HOME'u olsun.
        out.extend(["-e", "HOME=/tmp"].map(OsString::from));
        if let Some((uid, gid)) = self.user {
            out.push("--user".into());
            out.push(format!("{uid}:{gid}").into());
        }
        if let Some(name) = &self.name {
            out.push("--name".into());
            out.push(name.into());
        }
        out.extend(self.mounts.volume_args().into_iter().map(OsString::from));
        out.push("-w".into());
        out.push(self.workdir.clone().into());
        if let Some(entry) = &self.entrypoint {
            out.push("--entrypoint".into());
            out.push(entry.into());
        }
        out.push(self.image.reference().into());
        out.extend(self.args.iter().cloned());
        out
    }

    pub fn command(&self, docker: &Path) -> Command {
        let mut cmd = Command::new(docker);
        cmd.args(self.docker_args());
        cmd
    }
}

/// Dizinin sahibi (`uid`, `gid`) — Volt'un kendi oluşturduğu çıktı
/// dizini, yani çağıranın kimliği. Konteyner bu kimlikle koşunca
/// ürettiği dosyalar kök'e değil kullanıcıya ait olur.
#[cfg(unix)]
pub fn owner_of(dir: &Path) -> Option<(u32, u32)> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(dir).ok()?;
    Some((meta.uid(), meta.gid()))
}

/// Windows/macOS Docker Desktop bağlanan dosyaları kullanıcıya yazar.
#[cfg(not(unix))]
pub fn owner_of(_dir: &Path) -> Option<(u32, u32)> {
    None
}

/// `docker rm -f <ad>` — öldürülen `docker run` istemcisinin geride
/// bıraktığı konteyneri durdurur.
pub fn remove_container(docker: &Path, name: &str) {
    let _ = Command::new(docker)
        .args(["rm", "-f", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_strings(run: &Run<'_>) -> Vec<String> {
        run.docker_args()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn images_are_pinned_by_digest() {
        for image in [SIMULATION_IMAGE, FORMAL_IMAGE] {
            assert!(image.digest.starts_with("sha256:"));
            assert_eq!(image.digest.len(), "sha256:".len() + 64);
            assert_eq!(
                image.reference(),
                format!("{}@{}", image.name, image.digest)
            );
        }
        assert_eq!(image_for(Tool::Verilator), Some(&SIMULATION_IMAGE));
        assert_eq!(image_for(Tool::Sby), Some(&FORMAL_IMAGE));
        assert_eq!(image_for(Tool::Yosys), None);
    }

    #[test]
    fn preference_parses_known_values_and_rejects_others() {
        assert_eq!(Preference::parse(""), Ok(Preference::Auto));
        assert_eq!(Preference::parse(" Auto "), Ok(Preference::Auto));
        assert_eq!(Preference::parse("local"), Ok(Preference::Local));
        assert_eq!(Preference::parse("DOCKER"), Ok(Preference::Docker));
        assert!(Preference::parse("podman").is_err());
    }

    #[test]
    fn run_arguments_are_stable_and_carry_user_name_and_entrypoint() {
        let mut mounts = Mounts::for_platform(true);
        mounts.add_str(r"C:\p\build\sim\x");
        let run = Run {
            image: &SIMULATION_IMAGE,
            mounts: &mounts,
            workdir: "/volt/c/p/build/sim/x".into(),
            entrypoint: Some("/volt/c/p/build/sim/x/obj/VX".into()),
            user: Some((1001, 121)),
            name: Some("volt-1".into()),
            args: vec!["--flag".into()],
        };
        assert_eq!(
            arg_strings(&run),
            [
                "run",
                "--rm",
                "--init",
                "--network",
                "none",
                "-e",
                "HOME=/tmp",
                "--user",
                "1001:121",
                "--name",
                "volt-1",
                "-v",
                r"C:\p\build\sim\x:/volt/c/p/build/sim/x",
                "-w",
                "/volt/c/p/build/sim/x",
                "--entrypoint",
                "/volt/c/p/build/sim/x/obj/VX",
                &SIMULATION_IMAGE.reference(),
                "--flag",
            ]
        );
    }

    #[test]
    fn run_without_user_or_name_omits_the_flags() {
        let mounts = Mounts::for_platform(false);
        let run = Run {
            image: &FORMAL_IMAGE,
            mounts: &mounts,
            workdir: "/w".into(),
            entrypoint: None,
            user: None,
            name: None,
            args: vec!["sby".into()],
        };
        let args = arg_strings(&run);
        assert!(!args
            .iter()
            .any(|a| a == "--user" || a == "--name" || a == "--entrypoint"));
        assert_eq!(args.last().map(String::as_str), Some("sby"));
    }

    #[cfg(unix)]
    #[test]
    fn owner_of_a_fresh_directory_is_its_creator() {
        use std::os::unix::fs::MetadataExt;
        let dir = std::env::temp_dir().join(format!("volt-owner-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dizin");
        let file = dir.join("f");
        std::fs::write(&file, "").expect("yaz");
        let meta = std::fs::metadata(&file).expect("meta");
        assert_eq!(owner_of(&dir), Some((meta.uid(), meta.gid())));
        assert!(owner_of(&dir.join("volt-no-such-dir")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
