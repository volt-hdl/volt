//! Docker köprüsü (ADR-0094): Verilator/sby yerelde yokken `volt test`,
//! `volt run`, `volt verify` ve `volt doctor` sabitlenmiş imaja geçer.
//!
//! `docker` burada PATH'teki sahte bir betiktir: çağrıları günlüğe yazar,
//! daemon/imaj/indirme/koşu sonucunu ortam değişkenlerinden alır. Böylece
//! bilgi iletisi, bağlanan dizinler, yol çevirisi, dosya sahipliği ve hata
//! iletileri gerçek Docker'sız her makinede sınanır. Gerçek Docker'la uçtan
//! uca koşu `docker_e2e_tests.rs`'tedir.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const SIM_IMAGE: &str = "verilator/verilator:v5.052";
const FORMAL_IMAGE: &str = "hdlc/formal:all";

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

/// ASCII yollu geçici dizin (sahte betiğin günlüğü kod sayfasından
/// etkilenmesin): hedef dizinin altı.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("volt-docker-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir.canonicalize()
        .map(|p| PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\")))
        .expect("kanonik")
}

#[cfg(windows)]
fn write_fake_docker(bin: &Path) {
    let script = "@echo off\r\n\
        echo %*>>\"%FAKE_DOCKER_LOG%\"\r\n\
        if \"%1\"==\"info\" goto info\r\n\
        if \"%1\"==\"image\" goto image\r\n\
        if \"%1\"==\"pull\" goto pull\r\n\
        if \"%1\"==\"run\" goto run\r\n\
        if \"%1\"==\"rm\" exit /b 0\r\n\
        exit /b 1\r\n\
        :info\r\n\
        if \"%FAKE_DOCKER_DAEMON%\"==\"down\" exit /b 1\r\n\
        echo 29.0.0\r\n\
        exit /b 0\r\n\
        :image\r\n\
        if \"%FAKE_DOCKER_IMAGE%\"==\"missing\" exit /b 1\r\n\
        echo sha256:0\r\n\
        exit /b 0\r\n\
        :pull\r\n\
        if \"%FAKE_DOCKER_PULL%\"==\"fail\" goto pullfail\r\n\
        exit /b 0\r\n\
        :pullfail\r\n\
        echo Error response from daemon: network unreachable 1>&2\r\n\
        exit /b 1\r\n\
        :run\r\n\
        if defined FAKE_DOCKER_RUN_OUT type \"%FAKE_DOCKER_RUN_OUT%\"\r\n\
        if not defined FAKE_DOCKER_RUN_RC exit /b 0\r\n\
        exit /b %FAKE_DOCKER_RUN_RC%\r\n";
    std::fs::write(bin.join("docker.bat"), script).expect("sahte docker");
}

#[cfg(unix)]
fn write_fake_docker(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let script = "#!/bin/sh\n\
        printf '%s\\n' \"$*\" >> \"$FAKE_DOCKER_LOG\"\n\
        case \"$1\" in\n\
        info) [ \"$FAKE_DOCKER_DAEMON\" = down ] && exit 1; echo 29.0.0; exit 0;;\n\
        image) [ \"$FAKE_DOCKER_IMAGE\" = missing ] && exit 1; echo sha256:0; exit 0;;\n\
        pull) if [ \"$FAKE_DOCKER_PULL\" = fail ]; then echo 'Error response from daemon: network unreachable' >&2; exit 1; fi; exit 0;;\n\
        run) if [ -n \"$FAKE_DOCKER_RUN_OUT\" ]; then while IFS= read -r l; do printf '%s\\n' \"$l\"; done < \"$FAKE_DOCKER_RUN_OUT\"; fi; exit \"${FAKE_DOCKER_RUN_RC:-0}\";;\n\
        rm) exit 0;;\n\
        esac\n\
        exit 1\n";
    let path = bin.join("docker");
    std::fs::write(&path, script).expect("sahte docker");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// Sahte docker'lı ortam: PATH yalnız sahte `bin/` dizini (Verilator,
/// sby yok); araç değişkenleri ve kullanıcının dili temizlenir.
struct Env {
    dir: PathBuf,
    log: PathBuf,
}

impl Env {
    fn new(tag: &str) -> Self {
        let dir = temp_dir(tag);
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        write_fake_docker(&bin);
        let log = dir.join("docker.log");
        Env { dir, log }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = volt();
        cmd.args(args)
            .current_dir(&self.dir)
            .env("PATH", self.dir.join("bin"))
            .env("FAKE_DOCKER_LOG", &self.log)
            .env("VOLT_MANIFEST_DIR", &self.dir)
            .env_remove("VOLT_LANG")
            .env_remove("VOLT_TARGET_DIR");
        for var in [
            "VOLT_VERILATOR",
            "VOLT_SBY",
            "VOLT_DOCKER",
            "VOLT_TOOL_BACKEND",
            "FAKE_DOCKER_DAEMON",
            "FAKE_DOCKER_IMAGE",
            "FAKE_DOCKER_PULL",
            "FAKE_DOCKER_RUN_OUT",
            "FAKE_DOCKER_RUN_RC",
        ] {
            cmd.env_remove(var);
        }
        cmd
    }

    /// Sahte docker'ın aldığı çağrılar, satır satır.
    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            // Rust `=` içeren argümanı .bat'a tırnaklı verir.
            .map(|l| l.trim_end().replace('"', ""))
            .collect()
    }

    fn runs(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter(|l| l.starts_with("run "))
            .collect()
    }

    /// `docker run` çıktısı olarak basılacak metin.
    fn run_output(&self, text: &str) -> PathBuf {
        let path = self.dir.join("run_out.txt");
        std::fs::write(&path, text).expect("run çıktısı");
        path
    }

    fn sim_dir(&self) -> PathBuf {
        self.dir.join("build").join("sim").join("echo_test")
    }
}

const ECHO_TEST: &str = "\
module Echo {
    in  clk : clock
    in  d   : u4
    out q   : u4

    reg hold : u4 = 0

    on clk {
        hold <= d
    }

    q = hold
}

test \"echo\" {
    let dut = Echo { };
    dut.d = 5;
    step(1);
    assert_eq(dut.q, 5);
}
";

/// Tek kontratlı modül; iş ve görev adı `ctr` → çalışma dizini `ctr_ctr`.
const CTR: &str = "\
@no_auto_contracts
module Ctr {
    in  clk   : clock
    out count : u4

    invariant: count_r <= 15

    reg count_r : u4 = 0

    on clk {
        count_r <= count_r + 1
    }

    count = count_r
}
";

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Bu platformda konteyner yolu (Windows `/volt/<sürücü>/...`, Unix aynı).
fn container_of(host: &Path) -> String {
    volt_tools::docker_paths::container_path(host).expect("eşlenebilir yol")
}

fn run_test(env: &Env) -> Output {
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let mut cmd = env.cmd(&["test", "echo_test.volt"]);
    cmd.output().expect("volt test")
}

#[test]
fn test_in_docker_prints_one_note_and_mounts_only_the_sim_dir() {
    let env = Env::new("note");
    let out = run_test(&env);
    let err = stderr(&out);
    let note = format!("note: Verilator not found locally; running it in Docker ({SIM_IMAGE})");
    assert_eq!(err.matches(&note).count(), 1, "tek bilgi satırı:\n{err}");
    assert!(
        !err.contains("downloading"),
        "imaj var, indirme yok:\n{err}"
    );

    let runs = env.runs();
    assert_eq!(runs.len(), 2, "verilate + simülasyon: {runs:#?}");
    let sim = env.sim_dir();
    let (host, container) = (sim.display().to_string(), container_of(&sim));
    let mount = format!("-v {host}:{container} -w {container}");
    for run in &runs {
        assert!(
            run.starts_with("run --rm --init --network none -e HOME=/tmp"),
            "{run}"
        );
        assert!(run.contains(&mount), "yalnız sim dizini bağlanır: {run}");
        assert_eq!(run.matches(" -v ").count(), 1, "{run}");
    }
    assert!(
        runs[0].contains(&format!("{SIM_IMAGE}@sha256:")),
        "{}",
        runs[0]
    );
    assert!(
        runs[0]
            .ends_with("--cc Echo.sv --exe tb_Echo.cpp --build --top-module Echo -Mdir obj_echo"),
        "{}",
        runs[0]
    );
    let exe = format!("--entrypoint {container}/obj_echo/VEcho ");
    assert!(runs[1].contains(&exe), "{}", runs[1]);
    if cfg!(windows) {
        assert!(container.starts_with("/volt/"), "{container}");
    }
}

/// Linux'ta konteyner, Volt'un oluşturduğu sim dizininin sahibiyle koşar
/// (üretilen dosyalar kök'e ait olmasın).
#[cfg(unix)]
#[test]
fn test_in_docker_runs_as_the_owner_of_the_output_dir() {
    use std::os::unix::fs::MetadataExt;
    let env = Env::new("owner");
    run_test(&env);
    let meta = std::fs::metadata(env.sim_dir()).expect("sim dizini");
    let user = format!("--user {}:{}", meta.uid(), meta.gid());
    let runs = env.runs();
    assert!(!runs.is_empty());
    for run in runs {
        assert!(run.contains(&user), "{run}");
    }
}

#[test]
fn verilator_errors_are_reported_with_host_paths() {
    let env = Env::new("paths");
    let sim = env.sim_dir();
    let line = format!("%Error: {}/Echo.sv:3:1: syntax error\n", container_of(&sim));
    let out_file = env.run_output(&line);
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let out = env
        .cmd(&["test", "echo_test.volt"])
        .env("FAKE_DOCKER_RUN_OUT", &out_file)
        .env("FAKE_DOCKER_RUN_RC", "1")
        .output()
        .expect("volt test");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    let host = format!(
        "%Error: {}:3:1: syntax error",
        sim.join("Echo.sv").display()
    );
    assert!(err.contains(&host), "ana makine yolu:\n{err}");
    if cfg!(windows) {
        assert!(!err.contains("/volt/"), "konteyner yolu sızdı:\n{err}");
    }
    let log = Path::new("build")
        .join("sim")
        .join("echo_test")
        .join("verilator.log");
    assert!(err.contains(&log.display().to_string()), "{err}");
    let saved = std::fs::read_to_string(env.dir.join(&log)).expect("tam günlük");
    assert!(saved.contains(&host), "{saved}");
}

#[test]
fn daemon_down_is_a_clear_error_with_exit_3() {
    let env = Env::new("daemon");
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let out = env
        .cmd(&["test", "echo_test.volt"])
        .env("FAKE_DOCKER_DAEMON", "down")
        .output()
        .expect("volt test");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    assert!(
        err.contains("error: Verilator not found locally, and Docker is installed but its daemon is not running"),
        "{err}"
    );
    assert!(err.contains("start Docker Desktop"), "{err}");
    assert!(env.runs().is_empty());
}

#[test]
fn local_backend_never_calls_docker() {
    let env = Env::new("local");
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let out = env
        .cmd(&["test", "echo_test.volt"])
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt test");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    assert!(err.contains("error: Verilator not found"), "{err}");
    assert!(env.calls().is_empty(), "{:?}", env.calls());
}

#[test]
fn unknown_backend_value_is_a_usage_error() {
    let env = Env::new("badpref");
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let out = env
        .cmd(&["test", "echo_test.volt"])
        .env("VOLT_TOOL_BACKEND", "podman")
        .output()
        .expect("volt test");
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("VOLT_TOOL_BACKEND: unknown value 'podman'"));
}

#[test]
fn forced_docker_wins_over_a_local_verilator_and_says_why() {
    let env = Env::new("forced");
    // Yerel "Verilator" başlatılırsa test düşer: hiç çağrılmamalı.
    let fake = env.dir.join("bin").join(if cfg!(windows) {
        "verilator.bat"
    } else {
        "verilator"
    });
    std::fs::write(&fake, "exit 1\n").expect("sahte verilator");
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let out = env
        .cmd(&["test", "echo_test.volt"])
        .env("VOLT_TOOL_BACKEND", "docker")
        .output()
        .expect("volt test");
    let err = stderr(&out);
    let note = format!("note: VOLT_TOOL_BACKEND=docker; running Verilator in Docker ({SIM_IMAGE})");
    assert_eq!(err.matches(&note).count(), 1, "{err}");
    assert_eq!(env.runs().len(), 2);
}

#[test]
fn first_use_prints_the_download_size_then_the_time() {
    let env = Env::new("pull");
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let out = env
        .cmd(&["test", "echo_test.volt"])
        .env("FAKE_DOCKER_IMAGE", "missing")
        .output()
        .expect("volt test");
    let err = stderr(&out);
    let size = format!("note: downloading {SIM_IMAGE} (~250 MB, first use only");
    let done = format!("note: downloaded {SIM_IMAGE} in ");
    let (a, b) = (err.find(&size), err.find(&done));
    assert!(a.is_some() && b.is_some() && a < b, "{err}");
    assert!(env
        .calls()
        .iter()
        .any(|c| c.starts_with(&format!("pull -q {SIM_IMAGE}@sha256:"))));
}

#[test]
fn failed_download_is_a_clear_error_with_exit_3() {
    let env = Env::new("pullfail");
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let out = env
        .cmd(&["test", "echo_test.volt"])
        .env("FAKE_DOCKER_IMAGE", "missing")
        .env("FAKE_DOCKER_PULL", "fail")
        .output()
        .expect("volt test");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    assert!(
        err.contains(&format!(
            "error: could not download the Docker image {SIM_IMAGE}"
        )),
        "{err}"
    );
    assert!(
        err.contains("= reason: Error response from daemon: network unreachable"),
        "{err}"
    );
    assert!(env.runs().is_empty());
}

#[test]
fn out_of_memory_in_the_container_is_named() {
    let env = Env::new("oom");
    std::fs::write(env.dir.join("echo_test.volt"), ECHO_TEST).expect("test dosyası");
    let out = env
        .cmd(&["test", "echo_test.volt"])
        .env("FAKE_DOCKER_RUN_RC", "137")
        .output()
        .expect("volt test");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    assert!(
        err.contains("error: the Docker container ran out of memory (exit code 137)"),
        "{err}"
    );
}

#[test]
fn run_mounts_the_vcd_dir_and_prints_the_host_waveform_path() {
    let env = Env::new("run");
    let design = env.dir.join("echo.volt");
    std::fs::write(&design, ECHO_TEST.split("test \"").next().unwrap()).expect("tasarım");
    let out = env
        .cmd(&[
            "run",
            "echo.volt",
            "--cycles",
            "3",
            "--vcd",
            "waves/echo.vcd",
        ])
        .output()
        .expect("volt run");
    let err = stderr(&out);
    assert!(
        err.contains(&format!("running it in Docker ({SIM_IMAGE})")),
        "{err}"
    );
    assert!(err.contains("Waveform waves/echo.vcd"), "{err}");
    assert!(!err.contains("/volt/"), "{err}");
    // Testbench VCD'yi konteyner yoluna yazar; dizin konteynere bağlıdır.
    let waves = env.dir.join("waves");
    let vcd_in_tb = format!("{}/echo.vcd", container_of(&waves));
    let tb = std::fs::read_to_string(env.dir.join("build/sim/echo/tb.cpp")).expect("tb");
    assert!(tb.contains(&format!("vcd.open(\"{vcd_in_tb}\")")), "{tb}");
    let mount = format!("-v {}:{}", waves.display(), container_of(&waves));
    for run in env.runs() {
        assert!(run.contains(&mount), "{run}");
    }
}

#[test]
fn verify_runs_sby_once_in_one_named_container() {
    let env = Env::new("verify");
    std::fs::write(env.dir.join("ctr.volt"), CTR).expect("tasarım");
    let formal = env.dir.join("build").join("formal");
    let out_file = env.run_output(&format!(
        "SBY 16:34:39 [ctr_ctr] engine_0: ## {}/ctr_ctr/src/ctr.sv ok\n\
         SBY 16:34:39 [ctr_ctr] DONE (PASS, rc=0)\n",
        container_of(&formal)
    ));
    let out = env
        .cmd(&["verify", "ctr.volt", "-j", "4"])
        .env("FAKE_DOCKER_RUN_OUT", &out_file)
        .output()
        .expect("volt verify");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    let note = format!("note: SymbiYosys not found locally; running it in Docker ({FORMAL_IMAGE})");
    assert_eq!(err.matches(&note).count(), 1, "{err}");
    let runs = env.runs();
    assert_eq!(runs.len(), 1, "-j konteynerin içinde: {runs:#?}");
    let run = &runs[0];
    assert!(run.contains("--name volt-sby-"), "{run}");
    let container = container_of(&formal);
    assert!(
        run.contains(&format!(
            "-v {}:{container} -w {container}",
            formal.display()
        )),
        "{run}"
    );
    assert!(run.ends_with("sby -j 4 -f ctr.sby"), "{run}");
}

#[test]
fn verify_tool_error_in_docker_points_at_the_host_log() {
    let env = Env::new("verifyerr");
    std::fs::write(env.dir.join("ctr.volt"), CTR).expect("tasarım");
    let out_file = env.run_output("SBY 16:34:39 [ctr_ctr] DONE (ERROR, rc=16)\n");
    let out = env
        .cmd(&["verify", "ctr.volt"])
        .env("FAKE_DOCKER_RUN_OUT", &out_file)
        .env("FAKE_DOCKER_RUN_RC", "16")
        .output()
        .expect("volt verify");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    let log = Path::new("build")
        .join("formal")
        .join("ctr_ctr")
        .join("logfile.txt");
    assert!(
        err.contains(&format!("= help: the full log is in '{}'", log.display())),
        "{err}"
    );
    assert!(err.contains(&format!("in Docker {FORMAL_IMAGE}")), "{err}");
}

fn doctor_json(env: &Env, image: Option<&str>) -> (String, Value) {
    let mut human = env.cmd(&["doctor"]);
    let mut json = env.cmd(&["doctor", "--format", "json"]);
    if let Some(state) = image {
        human.env("FAKE_DOCKER_IMAGE", state);
        json.env("FAKE_DOCKER_IMAGE", state);
    }
    let text = stdout(&human.output().expect("doctor"));
    let out = json.output().expect("doctor json");
    (
        text,
        serde_json::from_slice(&out.stdout).expect("doctor JSON"),
    )
}

fn cap<'a>(json: &'a Value, id: &str) -> &'a Value {
    json["capabilities"]
        .as_array()
        .expect("capabilities")
        .iter()
        .find(|c| c["id"] == id)
        .expect("yetenek")
}

#[test]
fn doctor_says_which_commands_run_via_docker() {
    let env = Env::new("doctor");
    let (text, json) = doctor_json(&env, None);
    assert!(
        text.contains(&format!(
            "✓ test, run — via Docker ({SIM_IMAGE}: Verilator 5.052"
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "✓ verify — via Docker ({FORMAL_IMAGE}: Yosys 0.66"
        )),
        "{text}"
    );
    assert!(!text.contains("not downloaded"), "{text}");
    let sim = cap(&json, "simulation");
    assert_eq!(sim["status"], "ok");
    assert_eq!(sim["backend"], "docker");
    assert_eq!(sim["image_present"], true);
    assert!(sim["image"]
        .as_str()
        .is_some_and(|r| r.starts_with(&format!("{SIM_IMAGE}@sha256:"))));
    assert_eq!(json["required_ok"], true);
}

#[test]
fn doctor_names_the_first_download_size() {
    let env = Env::new("doctorpull");
    let (text, json) = doctor_json(&env, Some("missing"));
    assert!(
        text.contains("image not downloaded yet: ~250 MB on first use"),
        "{text}"
    );
    assert!(
        text.contains("image not downloaded yet: ~404 MB on first use"),
        "{text}"
    );
    assert_eq!(cap(&json, "verify")["image_present"], false);
}

#[test]
fn doctor_with_local_backend_does_not_offer_docker() {
    let env = Env::new("doctorlocal");
    let out = env
        .cmd(&["doctor", "--format", "json"])
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("doctor");
    let json: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(cap(&json, "simulation")["status"], "missing");
    assert_eq!(cap(&json, "simulation")["backend"], Value::Null);
    assert!(!env.calls().iter().any(|c| c.starts_with("image")));
}
