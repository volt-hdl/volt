//! Dalga formunda enum adları (ADR-0092) — `volt run --vcd` uçtan uca.
//!
//! Gerçek Verilator ister (`VOLT_VERILATOR` ya da `PATH`); yoksa atlanır,
//! `VOLT_REQUIRE_TOOLS=verilator` ile düşer (ADR-0079 §3). Oturumdaki her
//! iz yolu AYNI koşunun VCD başlığında aranır; GTKWave ad kuralı: kapsamlar
//! `.` ile birleşir, çok bitli değişken `ad[msb:lsb]` (VCD aralık yazmasa
//! da `[size-1:0]`), tek bit aralıksız.

mod tools;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn volt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    // Araç arka ucu açık: Verilator/sby yoksa Docker'a sessizce düşülmez.
    cmd.env("VOLT_TOOL_BACKEND", "local");
    cmd.env("VOLT_LANG", "en");
    cmd
}

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(rel)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-waves-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn run_with_vcd(design: &Path, top: Option<&str>, dir: &Path) -> Output {
    let mut cmd = volt();
    cmd.arg("run").arg("--cycles").arg("30");
    if let Some(t) = top {
        cmd.arg("--top").arg(t);
    }
    let out = cmd
        .arg("--vcd")
        .arg(dir.join("w.vcd"))
        .arg("--target-dir")
        .arg(dir.join("build"))
        .arg(design)
        .output()
        .expect("volt run");
    assert!(
        out.status.success(),
        "volt run başarısız:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// VCD başlığındaki değişkenlerin GTKWave adları.
fn vcd_names(vcd: &Path) -> HashSet<String> {
    let text = std::fs::read_to_string(vcd).expect("vcd");
    let mut names = HashSet::new();
    let mut scope: Vec<String> = Vec::new();
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.first().copied() {
            Some("$enddefinitions") => break,
            Some("$scope") => scope.push(t[2].to_string()),
            Some("$upscope") => {
                scope.pop();
            }
            Some("$var") => {
                let size: u32 = t[2].parse().expect("genişlik");
                let mut name = t[4].to_string();
                match t.get(5) {
                    Some(r) if r.starts_with('[') => name.push_str(r),
                    _ if size > 1 => name.push_str(&format!("[{}:0]", size - 1)),
                    _ => {}
                }
                let mut full = scope.join(".");
                full.push('.');
                full.push_str(&name);
                names.insert(full);
            }
            _ => {}
        }
    }
    names
}

/// Oturumun izleri ve her izin çeviri dosyası.
fn session_traces(gtkw: &Path) -> Vec<(String, PathBuf)> {
    let text = std::fs::read_to_string(gtkw).expect("gtkw");
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("[*] Volt waveform session (ADR-0092)"));
    let mut current: Option<PathBuf> = None;
    let mut out = Vec::new();
    for line in lines {
        if let Some(rest) = line.strip_prefix('^') {
            let (_, path) = rest.split_once(' ').expect("^N yol");
            current = Some(PathBuf::from(path));
        } else if line.starts_with('@') || line.starts_with('[') {
            continue;
        } else {
            out.push((line.to_string(), current.clone().expect("iz tablosuz")));
        }
    }
    out
}

fn assert_paths_exist(dir: &Path) -> Vec<(String, PathBuf)> {
    let names = vcd_names(&dir.join("w.vcd"));
    let traces = session_traces(&dir.join("w.gtkw"));
    assert!(!traces.is_empty());
    for (path, filter) in &traces {
        assert!(
            names.contains(path),
            "{path} VCD başlığında yok; başlık: {names:?}"
        );
        assert!(filter.is_file(), "{} yok", filter.display());
    }
    traces
}

#[test]
fn uart_tx_state_names_reach_the_simulation_waveform() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("uart");
    let out = run_with_vcd(&repo("examples/uart_tx.volt"), None, &dir);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let traces = assert_paths_exist(&dir);
    assert_eq!(traces[0].0, "TOP.UartTx.state_r[1:0]");
    let table = std::fs::read_to_string(&traces[0].1).expect("tablo");
    assert!(
        table.ends_with("00 Idle\n01 Start\n10 Data\n11 Stop\n"),
        "{table}"
    );
    // Tek satır: iki dosyayı birlikte açan komut.
    let hint = format!(
        "Waveform gtkwave {} {}",
        dir.join("w.vcd").display(),
        dir.join("w.gtkw").display()
    );
    assert!(stderr.contains(&hint), "{stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Alt örnek, struct alanı, örnek çıkış teli ve Trit — yolların hepsi
/// Verilator'ın VCD'sinde.
#[test]
fn instance_struct_and_trit_paths_exist_in_the_vcd() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("hier");
    let design = dir.join("hier.volt");
    std::fs::write(
        &design,
        "\
enum Phase { Idle, Run, Done }

struct Req {
    kind : Phase
    last : bool
}

module Sub {
    in  clk : clock
    in  go  : bool
    out ph  : Phase
    out tv  : Trit

    reg st : Phase = Phase::Idle
    reg t  : Trit  = 0

    on clk {
        match st {
            Phase::Idle => { if go { st <= Phase::Run } }
            Phase::Run  => { st <= Phase::Done }
            Phase::Done => { st <= Phase::Idle }
        }
        t <= -t
    }
    ph = st
    tv = t
}

pub module Top {
    in  clk  : clock
    in  go   : bool
    in  req  : Req
    out busy : bool

    let u = Sub { clk: clk, go: go }
    reg hold : Req = Req { kind: Phase::Idle, last: false }

    on clk {
        hold.kind <= req.kind
        hold.last <= req.last
    }
    busy = u.ph != Phase::Idle || hold.kind != Phase::Idle
}
",
    )
    .expect("yaz");
    run_with_vcd(&design, Some("Top"), &dir);
    let traces = assert_paths_exist(&dir);
    let paths: Vec<&str> = traces.iter().map(|(p, _)| p.as_str()).collect();
    for want in [
        "TOP.Top.req_kind[1:0]",
        "TOP.Top.hold_kind[1:0]",
        "TOP.Top.u_ph[1:0]",
        "TOP.Top.u_tv[1:0]",
        "TOP.Top.u.st[1:0]",
        "TOP.Top.u.t[1:0]",
    ] {
        assert!(paths.contains(&want), "{want}: {paths:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Enum kullanmayan tasarım: VCD'nin yanında yeni dosya yok, komut satırı
/// yalnız VCD'yi açar.
#[test]
fn design_without_enums_writes_only_the_vcd() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("plain");
    let out = run_with_vcd(&repo("tests/fixtures/counter.volt"), None, &dir);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .expect("dizin")
        .map(|e| e.expect("girdi").file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, ["build", "w.vcd"]);
    let hint = format!("Waveform {}\n", dir.join("w.vcd").display());
    assert!(stderr.contains(&hint), "{stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}
