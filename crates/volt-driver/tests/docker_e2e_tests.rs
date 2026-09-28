//! Docker köprüsü gerçek Docker'la uçtan uca (ADR-0094 §5).
//!
//! Yalnız `VOLT_DOCKER_E2E=1` iken koşar: imajları indirir (ilk koşuda
//! ~650 MB) ve Verilator/sby'yi konteynerde çalıştırır. CI'da araçsız
//! Linux işi (`docker-backend`) açar; yerelde Windows doğrulaması aynı
//! testle yapılır. PATH yalnız `docker`'ın dizinidir — Verilator ve sby
//! yerelde bulunamaz, Volt kendiliğinden Docker'a geçer.
//!
//! Ağır koşu: aynı anda tek konteyner (CLAUDE.md bellek kuralı), adımlar
//! sırayla tek test fonksiyonunda.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn enabled() -> bool {
    std::env::var("VOLT_DOCKER_E2E").is_ok_and(|v| v == "1")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("volt-docker-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

struct Ctx {
    path: std::ffi::OsString,
}

impl Ctx {
    fn new() -> Self {
        let docker = volt_tools::find(volt_tools::Tool::Docker).expect("docker bulunmalı");
        let dir = docker.parent().expect("docker dizini").to_path_buf();
        assert!(
            volt_tools::find_in(
                volt_tools::Tool::Verilator,
                &volt_tools::ToolEnv {
                    path: Some(dir.clone().into_os_string()),
                    overrides: Vec::new(),
                }
            )
            .is_none(),
            "docker'ın dizininde Verilator var; köprü sınanamaz"
        );
        Ctx {
            path: dir.into_os_string(),
        }
    }

    fn volt(&self, cwd: &Path, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
        cmd.args(args).current_dir(cwd).env("PATH", &self.path);
        for var in [
            "VOLT_VERILATOR",
            "VOLT_SBY",
            "VOLT_DOCKER",
            "VOLT_TOOL_BACKEND",
            "VOLT_LANG",
            "VOLT_TARGET_DIR",
        ] {
            cmd.env_remove(var);
        }
        let out = cmd.output().expect("volt");
        eprintln!(
            "$ volt {}\n{}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Konteyner yolu kullanıcıya sızmadı (Windows'ta `/volt/...`).
fn assert_no_container_paths(text: &str) {
    if cfg!(windows) {
        assert!(!text.contains("/volt/"), "konteyner yolu sızdı:\n{text}");
    }
}

/// Linux'ta ağaçtaki her dosya ve dizin çağıranın (runner kullanıcısı).
#[cfg(unix)]
fn assert_owned_by_caller(root: &Path) {
    use std::os::unix::fs::MetadataExt;
    let me = std::fs::metadata(root).expect("kök").uid();
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("dizin").flatten() {
            let meta = entry.path().symlink_metadata().expect("meta");
            assert_eq!(meta.uid(), me, "{} kök'e ait", entry.path().display());
            seen += 1;
            if meta.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    assert!(seen > 0);
}

#[cfg(not(unix))]
fn assert_owned_by_caller(_root: &Path) {}

#[test]
fn new_test_verify_and_run_through_docker() {
    if !enabled() {
        eprintln!("SKIP: VOLT_DOCKER_E2E=1 değil (gerçek Docker koşusu)");
        return;
    }
    let ctx = Ctx::new();
    let root = temp_dir("demo");

    // indir → volt new → volt test → volt verify
    assert_eq!(
        ctx.volt(&root, &["new", "demo", "--template", "cdc"])
            .status
            .code(),
        Some(0)
    );
    let demo = root.join("demo");
    let test = ctx.volt(&demo, &["test"]);
    let err = stderr(&test);
    assert_eq!(test.status.code(), Some(0), "{err}");
    assert!(
        err.contains(
            "note: Verilator not found locally; running it in Docker (verilator/verilator:v5.052)"
        ),
        "{err}"
    );
    assert!(String::from_utf8_lossy(&test.stdout).contains("test result: ok."));
    assert_no_container_paths(&err);

    let verify = ctx.volt(&demo, &["verify", "event_counter.volt"]);
    let err = stderr(&verify);
    assert_eq!(verify.status.code(), Some(0), "{err}");
    assert!(
        err.contains("note: SymbiYosys not found locally; running it in Docker (hdlc/formal:all)"),
        "{err}"
    );
    assert_no_container_paths(&err);
    assert_owned_by_caller(&demo);

    // uart_tx: dalga formu + enum oturumu, basılan komut ana makine yollu
    let uart = root.join("uart");
    std::fs::create_dir_all(&uart).expect("uart");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/uart_tx.volt");
    std::fs::copy(&src, uart.join("uart_tx.volt")).expect("uart_tx kopyası");
    let run = ctx.volt(
        &uart,
        &[
            "run",
            "uart_tx.volt",
            "--cycles",
            "60",
            "--vcd",
            "waves/uart.vcd",
        ],
    );
    let err = stderr(&run);
    assert_eq!(run.status.code(), Some(0), "{err}");
    let line = err
        .lines()
        .find(|l| l.trim_start().starts_with("Waveform "))
        .unwrap_or_else(|| panic!("Waveform satırı yok:\n{err}"));
    let cmd = line.trim_start().trim_start_matches("Waveform ");
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    assert_eq!(parts.first(), Some(&"gtkwave"), "{line}");
    for file in &parts[1..] {
        let p = uart.join(file);
        assert!(p.is_file(), "basılan yol ana makinede yok: {file}");
        assert!(std::fs::metadata(&p).unwrap().len() > 0, "{file} boş");
    }
    assert_no_container_paths(&err);
    assert_owned_by_caller(&uart);

    project_mode_and_failed_test_waveform(&ctx, &root);
}

/// ADR-0095: argümansız komutlar ve düşen testin dalga formu — basılan
/// yollar ana makinede, `.gtkw` izleri VCD başlığında.
fn project_mode_and_failed_test_waveform(ctx: &Ctx, root: &Path) {
    assert_eq!(ctx.volt(root, &["new", "mini"]).status.code(), Some(0));
    let mini = root.join("mini");
    for cmd in ["check", "build"] {
        let out = ctx.volt(&mini, &[cmd]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }
    assert!(mini.join("build/rtl/Counter.sv").is_file());
    let test_file = mini.join("counter_test.volt");
    let text = std::fs::read_to_string(&test_file).expect("test");
    let broken = text.replacen("assert_eq(dut.count, 3);", "assert_eq(dut.count, 4);", 1);
    assert_ne!(text, broken);
    std::fs::write(&test_file, broken).expect("yaz");

    let out = ctx.volt(&mini, &["test"]);
    assert_eq!(out.status.code(), Some(5), "{}", stderr(&out));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with("Waveform gtkwave "))
        .unwrap_or_else(|| {
            panic!(
                "Waveform satırı yok:
{stdout}"
            )
        });
    let parts: Vec<&str> = line.split_whitespace().skip(2).collect();
    assert_eq!(parts.len(), 2, "{line}");
    let (vcd, gtkw) = (mini.join(parts[0]), mini.join(parts[1]));
    let vcd_text = std::fs::read_to_string(&vcd).expect("VCD ana makinede");
    let session = std::fs::read_to_string(&gtkw).expect("gtkw ana makinede");
    let header = vcd_names(&vcd_text);
    let traces: Vec<&str> = session
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with(['[', '@', '^', '*']))
        .collect();
    assert!(!traces.is_empty(), "{session}");
    for t in &traces {
        assert!(
            header.contains(&(*t).to_string()),
            "{t} VCD'de yok: {header:?}"
        );
    }
    // Yalnız düşen test kaydedilir.
    assert_eq!(stdout.matches("Waveform gtkwave").count(), 1, "{stdout}");
    assert_no_container_paths(&stdout);
    assert_owned_by_caller(&mini);
}

/// VCD başlığındaki sinyallerin GTKWave adları (`TOP.M.ad[W-1:0]`).
fn vcd_names(vcd: &str) -> Vec<String> {
    let mut scope: Vec<&str> = Vec::new();
    let mut names = Vec::new();
    for line in vcd.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.first().copied() {
            Some("$scope") => scope.push(t[2]),
            Some("$upscope") => {
                scope.pop();
            }
            Some("$var") => {
                let width: u32 = t[2].parse().expect("genişlik");
                let range = match t.get(5) {
                    Some(r) if r.starts_with('[') => (*r).to_string(),
                    _ if width > 1 => format!("[{}:0]", width - 1),
                    _ => String::new(),
                };
                names.push(format!("{}.{}{range}", scope.join("."), t[4]));
            }
            Some("$enddefinitions") => break,
            _ => {}
        }
    }
    names
}
