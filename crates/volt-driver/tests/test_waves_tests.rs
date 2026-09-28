//! ADR-0095 §3-4: `volt test --waves / --no-waves` ve `volt test --watch`.
//!
//! Verilator gerekmez: sürücü testbench'i aracı çağırmadan önce yazar;
//! çalıştırılamayan bir `VOLT_VERILATOR` ile koşu çıkış kodu 3'le biter
//! ve yazılan metin denetlenir (sim_golden deseni). Düşen testin izli
//! yeniden koşusu gerçek Verilator ister: `docker_e2e_tests.rs`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-waves-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".git")).expect("temp dizini");
    dir
}

const COUNTER: &str = "pub module Counter {\n    in  clk    : clock\n    in  enable : bool\n    \
                       out count  : u8\n    reg c : u8 = 0\n    on clk {\n        if enable { c <= c + 1 }\n    }\n    \
                       count = c\n}\n";

const TESTS: &str = "test \"counts\" {\n    let dut = Counter { };\n    dut.enable = true;\n    \
                     step(3);\n    assert_eq(dut.count, 3);\n}\n";

fn project(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    std::fs::write(
        dir.join("Volt.toml"),
        "[package]\nname = \"w\"\nsrc = \".\"\ntop = \"Counter\"\n",
    )
    .expect("yaz");
    std::fs::write(dir.join("counter.volt"), COUNTER).expect("yaz");
    std::fs::write(dir.join("counter_test.volt"), TESTS).expect("yaz");
    std::fs::write(dir.join("no-verilator"), "not a program").expect("yaz");
    dir
}

fn volt(dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    cmd.current_dir(dir)
        .env("VOLT_LANG", "en")
        .env_remove("VOLT_MANIFEST_DIR")
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_VERILATOR", dir.join("no-verilator"));
    cmd
}

fn run(dir: &Path, args: &[&str]) -> Output {
    volt(dir).args(args).output().expect("volt")
}

fn tb(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("build/sim/counter_test/tb_Counter.cpp")).expect("tb")
}

#[test]
fn default_run_writes_the_untraced_testbench() {
    let dir = project("default");
    let out = run(&dir, &["test"]);
    assert_eq!(out.status.code(), Some(3));
    let text = tb(&dir);
    assert!(!text.contains("verilated_vcd_c.h"), "{text}");
    assert!(!text.contains("volt_tfp"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn waves_flag_traces_every_test_into_the_waves_dir() {
    let dir = project("all");
    let out = run(&dir, &["test", "--waves"]);
    assert_eq!(out.status.code(), Some(3));
    let text = tb(&dir);
    assert!(text.contains("#include \"verilated_vcd_c.h\""), "{text}");
    assert!(
        text.contains("volt_trace.vcd.open(\"waves/Counter-counts.vcd\");"),
        "{text}"
    );
    assert!(dir.join("build/sim/counter_test/waves").is_dir());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_waves_writes_nothing_and_conflicts_with_waves() {
    let dir = project("off");
    let out = run(&dir, &["test", "--no-waves"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(!dir.join("build/sim/counter_test/waves").exists());
    let both = run(&dir, &["test", "--waves", "--no-waves"]);
    assert_eq!(both.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Alt sürecin stderr'ini satır satır toplar.
fn collect_stderr(child: &mut Child) -> Arc<Mutex<Vec<String>>> {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    let stderr = child.stderr.take().expect("stderr");
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            sink.lock().expect("kilit").push(line);
        }
    });
    lines
}

fn wait_until(lines: &Arc<Mutex<Vec<String>>>, what: &str, count: usize) -> bool {
    let end = Instant::now() + Duration::from_secs(60);
    while Instant::now() < end {
        let n = lines
            .lock()
            .expect("kilit")
            .iter()
            .filter(|l| l.contains(what))
            .count();
        if n >= count {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Test paniklese de `--watch` süreci öldürülür: Windows'ta yetim süreç
/// test borusunu açık tutup koşuyu asılı bırakırdı.
struct Watcher(Child);

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn watch_reruns_once_per_burst_of_saves_and_ignores_build_output() {
    // Arrange: araç çalıştırılamaz — her koşu hızla "could not run" biter.
    let dir = project("watch");
    let mut watcher = Watcher(
        volt(&dir)
            .args(["test", "--watch"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("volt"),
    );
    let child = &mut watcher.0;
    let lines = collect_stderr(child);
    assert!(wait_until(&lines, "waiting for changes", 1), "{:?}", lines);

    // Act 1: derleme çıktısı yazmak tetiklemez.
    std::fs::write(dir.join("build/sim/counter_test/extra.txt"), "x").expect("yaz");
    std::thread::sleep(Duration::from_millis(1500));
    let runs_after_build = lines
        .lock()
        .expect("kilit")
        .iter()
        .filter(|l| l.contains("volt test --watch"))
        .count();

    // Act 2: art arda iki kayıt → tek yeniden koşu.
    std::fs::write(dir.join("counter_test.volt"), format!("{TESTS}\n")).expect("yaz");
    std::fs::write(dir.join("counter_test.volt"), format!("{TESTS}\n\n")).expect("yaz");
    assert!(wait_until(&lines, "waiting for changes", 2), "{:?}", lines);
    std::thread::sleep(Duration::from_millis(1500));

    // Assert
    let all = lines.lock().expect("kilit").clone();
    let runs = all
        .iter()
        .filter(|l| l.contains("volt test --watch"))
        .count();
    assert_eq!(runs_after_build, 1, "{all:?}");
    assert_eq!(runs, 2, "{all:?}");
    assert!(all.iter().any(|l| l.contains("run 2")), "{all:?}");
    assert!(
        all.iter().any(|l| l.contains("could not run the tests")),
        "{all:?}"
    );
    stop(child, &lines);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Unix: gerçek Ctrl-C (SIGINT) → temiz çıkış, kod 130.
#[cfg(unix)]
fn stop(child: &mut Child, lines: &Arc<Mutex<Vec<String>>>) {
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("kill");
    assert!(status.success());
    let code = child.wait().expect("bekle").code();
    assert_eq!(code, Some(130));
    assert!(wait_until(lines, "Stopped watching", 1), "{lines:?}");
}

/// Windows: güvenli kodla konsol kesmesi gönderilemez; süreç öldürülür
/// (Ctrl-C yolu el ile doğrulandı, ADR-0095 §6).
#[cfg(not(unix))]
fn stop(child: &mut Child, _lines: &Arc<Mutex<Vec<String>>>) {
    let _ = child.kill();
    let _ = child.wait();
}
