//! Çıktı doğrulama ağı (ADR-0079): Volt'un ürettiği her çıktı, "Volt
//! tamam dedi" yerine GERÇEK tüketicisine verilir. Korpus: `tests/ui/pass`,
//! `examples/` (`*_test.volt` hariç) ve `tests/fixtures/` (parite
//! negatifleri ve testler hariç).
//!
//! | Çıktı | Tüketici | Test |
//! |---|---|---|
//! | SV (build) | Verilator `--lint-only -Wall` | `verilator_lints_sv_and_sva` |
//! | SVA (.sva + bind, satır içi) | Verilator `--assert` | aynı |
//! | SV (build) | Yosys `read_verilog -sv; hierarchy -check; proc; check -assert` | `yosys_elaborates_sv` |
//! | XDC | Tcl (Yosys `-c`, güvenli alt yorumlayıcıda Vivado komut kalıpları) | `yosys_tcl_evaluates_xdc` |
//! | C başlığı | `cc -std=c99` / `c++ -std=c++17`, `-Wall -Wextra -Werror -pedantic` | `cc_…`, `cxx_…` |
//! | Rust sürücüsü | `rustc --crate-type lib -D warnings` | `rustc_compiles_rust_drivers` |
//! | regmap JSON | `volt-regmap/1` şeması + `volt check-regmap` | `regmap_json_…` |
//!
//! SDC → OpenSTA `read_sdc` ayrı betiktir (`scripts/sta/sweep.py`, timing
//! işi): tasarımı Yosys'le hücrelere indirmek ve OpenSTA imajı gerekir.
//!
//! Uyarı politikası: araç HATASI her zaman düşürür. Verilator `-Wall` ve
//! Yosys uyarıları da düşürür; istisna yalnız gerekçeli işaretle ve işaret
//! bayatlarsa (uyarı artık çıkmıyorsa) test yine düşer:
//!
//! - `//~ LINT-ALLOW: <VERILATOR_KODU>: <gerekçe>` — fixture'ın bilinçli
//!   uyarısı (ör. yalnız kontratta okunan register).
//! - `//~ YOSYS-ALLOW: <ileti parçası>: <gerekçe>` — Yosys uyarısı.
//! - `//~ SYNTH-SKIP: <gerekçe>` — Yosys'e verilmez (ör. `tri1` çekme
//!   ağı); Yosys temiz geçerse işaret bayattır.
//! - `//~ NET-SKIP: <gerekçe>` — hiçbir tüketiciye verilmez (ör. gövdesi
//!   olmayan extern); Verilator temiz geçerse işaret bayattır.
//!
//! Araç yoksa test atlanır; `VOLT_REQUIRE_TOOLS` ile zorunlu kılınan araç
//! yoksa düşer (`tools` modülü, ADR-0079 §3).

mod tools;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use serde_json::Value;
use tools::Tool;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("depo kökü")
}

// ═══ Korpus ═════════════════════════════════════════════════════════

/// Fixture'daki gerekçeli ağ işaretleri.
#[derive(Debug, Default, PartialEq)]
struct Markers {
    lint_allow: Vec<String>,
    yosys_allow: Vec<String>,
    synth_skip: bool,
    net_skip: bool,
}

const MARKERS: [&str; 4] = [
    "//~ LINT-ALLOW",
    "//~ YOSYS-ALLOW",
    "//~ SYNTH-SKIP",
    "//~ NET-SKIP",
];

/// `//~ <İŞARET>: [<anahtar>:] <gerekçe>` satırlarını ayrıştırır.
fn parse_markers(text: &str) -> Result<Markers, String> {
    let mut m = Markers::default();
    for line in text.lines().map(str::trim) {
        let Some(marker) = MARKERS.iter().find(|p| line.starts_with(**p)) else {
            continue;
        };
        let rest = line[marker.len()..]
            .strip_prefix(':')
            .ok_or_else(|| format!("'{line}': biçim '{marker}: ...'"))?
            .trim();
        let keyed = matches!(*marker, "//~ LINT-ALLOW" | "//~ YOSYS-ALLOW");
        let (key, reason) = if keyed {
            let (k, r) = rest.split_once(':').unwrap_or((rest, ""));
            (k.trim().to_string(), r.trim())
        } else {
            (String::new(), rest)
        };
        if reason.is_empty() || (keyed && key.is_empty()) {
            return Err(format!("'{line}': anahtar ya da gerekçe eksik"));
        }
        match *marker {
            "//~ LINT-ALLOW" => m.lint_allow.push(key),
            "//~ YOSYS-ALLOW" => m.yosys_allow.push(key),
            "//~ SYNTH-SKIP" => m.synth_skip = true,
            _ => m.net_skip = true,
        }
    }
    Ok(m)
}

struct Design {
    /// Depo köküne göreli kaynak (`tests/ui/pass/12_x.volt`).
    rel: String,
    id: String,
    markers: Markers,
    /// `@source(...)` ile adlandırılan extern gövdeleri.
    externs: Vec<PathBuf>,
}

fn volt_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() && name != "parity" && name != "build" {
            volt_files(&p, out);
        } else if name.ends_with(".volt") && !name.ends_with("_test.volt") {
            out.push(p);
        }
    }
}

/// `@source("yol")` → volt dosyasının dizinine göreli yol (ADR-0076).
fn extern_sources(file: &Path, text: &str) -> Vec<PathBuf> {
    let dir = file.parent().unwrap();
    let mut out: Vec<PathBuf> = text
        .split("@source(\"")
        .skip(1)
        .filter_map(|s| s.split_once('"').map(|(p, _)| dir.join(p)))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn corpus() -> &'static [Design] {
    static CORPUS: OnceLock<Vec<Design>> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let root = root();
        let mut files = Vec::new();
        for dir in ["tests/ui/pass", "examples", "tests/fixtures"] {
            volt_files(&root.join(dir), &mut files);
        }
        files
            .into_iter()
            .map(|f| {
                let rel = f
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&f).expect("okunmalı");
                let markers = parse_markers(&text).unwrap_or_else(|e| panic!("{rel}: {e}"));
                Design {
                    id: rel.trim_end_matches(".volt").replace('/', "__"),
                    externs: extern_sources(&f, &text),
                    rel,
                    markers,
                }
            })
            .collect()
    })
}

/// İş parçacıklarıyla sıra korunarak eşleme.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let mut results: Vec<(usize, R)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                s.spawn(|| {
                    let mut local = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        local.push((i, f(item)));
                    }
                    local
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, r)| r).collect()
}

// ═══ Derleme (üç kip, tembel) ═══════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Kullanıcının sentezlediği RTL + kısıtlar + sürücüler.
    Rtl,
    /// Ayrı `.sva` + bind (`--emit=sva`).
    Sva,
    /// Satır içi SVA (`--sva=inline`).
    Inline,
}

impl Kind {
    fn dir(self) -> &'static str {
        match self {
            Kind::Rtl => "rtl",
            Kind::Sva => "sva",
            Kind::Inline => "inline",
        }
    }

    fn args(self) -> &'static [&'static str] {
        match self {
            Kind::Rtl => &["--emit=sdc,xdc,rust,c,regmap"],
            Kind::Sva => &["--emit=sva"],
            Kind::Inline => &["--emit=sva", "--sva=inline"],
        }
    }
}

fn work_dir() -> PathBuf {
    std::env::temp_dir().join(format!("volt-net-{}", std::process::id()))
}

fn out_dir(kind: Kind, d: &Design) -> PathBuf {
    work_dir().join(kind.dir()).join(&d.id)
}

fn build(kind: Kind) {
    static DONE: [OnceLock<()>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    DONE[kind as usize].get_or_init(|| {
        let root = root();
        let failures: Vec<String> = par_map(corpus(), |d| {
            let out = Command::new(env!("CARGO_BIN_EXE_volt"))
                .args(["--lang", "en", "build", "--format", "short"])
                .args(kind.args())
                .arg("--target-dir")
                .arg(out_dir(kind, d))
                .arg(&d.rel)
                .current_dir(&root)
                .output()
                .expect("volt çalışmalı");
            (!out.status.success())
                .then(|| format!("{}: {}", d.rel, String::from_utf8_lossy(&out.stdout)))
        })
        .into_iter()
        .flatten()
        .collect();
        assert!(
            failures.is_empty(),
            "build edilemeyen:\n{}",
            failures.join("\n")
        );
    });
}

fn files_with(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    files.sort();
    files
}

fn stem(p: &Path) -> String {
    p.file_stem().unwrap().to_string_lossy().into_owned()
}

fn text_of(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

// ═══ Verilator ══════════════════════════════════════════════════════

/// Bir Verilator koşusunun sonucu: hata satırları ve uyarı kodları.
#[derive(Default)]
struct Lint {
    errors: Vec<String>,
    warnings: Vec<(String, String)>,
}

fn parse_verilator(text: &str, status_ok: bool) -> Lint {
    let mut lint = Lint::default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("%Warning-") {
            let code = rest.split(':').next().unwrap_or_default().to_string();
            lint.warnings.push((code, line.to_string()));
        } else if line.starts_with("%Error") && !line.starts_with("%Error: Exiting due to") {
            lint.errors.push(line.to_string());
        }
    }
    if !status_ok && lint.errors.is_empty() && lint.warnings.is_empty() {
        lint.errors.push(format!("çıkış kodu sıfır değil:\n{text}"));
    }
    lint
}

fn verilator_lint(verilator: &Path, top: &str, files: &[PathBuf], assert: bool) -> Lint {
    let mut cmd = Command::new(verilator);
    cmd.args(["--lint-only", "-Wall", "--top-module", top]);
    if assert {
        cmd.arg("--assert");
    }
    let out = cmd.args(files).output().expect("verilator çalışmalı");
    let mut lint = parse_verilator(&text_of(&out), out.status.success());
    // Extern gövdeleri (`@source`) kullanıcının SV'sidir: onların stil
    // uyarıları ağın konusu değil; hataları öyle (tasarım derlenmez).
    let generated = work_dir().display().to_string();
    lint.warnings.retain(|(_, line)| line.contains(&generated));
    lint
}

/// Tasarımın tüm Verilator koşuları: RTL (her modül üst), .sva + bind,
/// satır içi SVA.
fn verilator_design(verilator: &Path, d: &Design) -> Lint {
    let mut all = Lint::default();
    let mut add = |l: Lint| {
        all.errors.extend(l.errors);
        all.warnings.extend(l.warnings);
    };
    let rtl_files = |kind: Kind| {
        let mut f = files_with(&out_dir(kind, d).join("rtl"), "sv");
        f.extend(d.externs.iter().cloned());
        f
    };
    let rtl = rtl_files(Kind::Rtl);
    for top in files_with(&out_dir(Kind::Rtl, d).join("rtl"), "sv") {
        add(verilator_lint(verilator, &stem(&top), &rtl, false));
    }
    let sva_rtl = rtl_files(Kind::Sva);
    for sva in files_with(&out_dir(Kind::Sva, d).join("formal"), "sva") {
        let text = std::fs::read_to_string(&sva).unwrap();
        let top = text
            .lines()
            .find_map(|l| l.strip_prefix("bind "))
            .and_then(|l| l.split_whitespace().next())
            .expect("bind satırı")
            .to_string();
        let mut files = sva_rtl.clone();
        files.push(sva);
        add(verilator_lint(verilator, &top, &files, true));
    }
    let inline = rtl_files(Kind::Inline);
    for top in files_with(&out_dir(Kind::Inline, d).join("rtl"), "sv") {
        add(verilator_lint(verilator, &stem(&top), &inline, true));
    }
    all
}

/// İşaretlere göre bulgular; boşsa tasarım temiz.
fn verilator_verdict(d: &Design, lint: &Lint) -> Vec<String> {
    let mut bad = Vec::new();
    if d.markers.net_skip {
        if lint.errors.is_empty() {
            bad.push(format!(
                "{}: NET-SKIP bayat — Verilator temiz geçiyor",
                d.rel
            ));
        }
        return bad;
    }
    bad.extend(lint.errors.iter().map(|e| format!("{}: {e}", d.rel)));
    let seen: BTreeSet<&str> = lint.warnings.iter().map(|(c, _)| c.as_str()).collect();
    for (code, line) in &lint.warnings {
        if !d.markers.lint_allow.contains(code) {
            bad.push(format!("{}: izinsiz uyarı: {line}", d.rel));
        }
    }
    for code in &d.markers.lint_allow {
        if !seen.contains(code.as_str()) {
            bad.push(format!(
                "{}: LINT-ALLOW {code} bayat — uyarı çıkmıyor",
                d.rel
            ));
        }
    }
    bad
}

#[test]
fn verilator_lints_sv_and_sva() {
    let Some(verilator) = tools::require(Tool::Verilator) else {
        return;
    };
    for kind in [Kind::Rtl, Kind::Sva, Kind::Inline] {
        build(kind);
    }
    let bad: Vec<String> = par_map(corpus(), |d| {
        verilator_verdict(d, &verilator_design(&verilator, d))
    })
    .into_iter()
    .flatten()
    .collect();
    let sva_count: usize = corpus()
        .iter()
        .map(|d| files_with(&out_dir(Kind::Sva, d).join("formal"), "sva").len())
        .sum();
    assert!(bad.is_empty(), "{} bulgu:\n{}", bad.len(), bad.join("\n"));
    // Korpus sessizce küçülürse ağ boşuna yeşil kalmasın.
    assert!(corpus().len() >= 120, "korpus {}", corpus().len());
    assert!(sva_count >= 70, ".sva sayısı {sva_count}");
}

/// Koşucu gerçekten hata görür: geçersiz SV sessizce geçmemeli.
#[test]
fn verilator_runner_rejects_invalid_sv() {
    let Some(verilator) = tools::require(Tool::Verilator) else {
        return;
    };
    let dir = work_dir().join("verilator-neg");
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("Bad.sv");
    std::fs::write(
        &bad,
        "module Bad(input logic a, output logic y);\n  assign y = a +;\nendmodule\n",
    )
    .unwrap();
    assert!(
        !verilator_lint(&verilator, "Bad", std::slice::from_ref(&bad), false)
            .errors
            .is_empty()
    );
    std::fs::write(
        &bad,
        "module Bad(input logic a, input logic b, output logic y);\n  assign y = a;\nendmodule\n",
    )
    .unwrap();
    let lint = verilator_lint(&verilator, "Bad", &[bad], false);
    assert!(
        lint.warnings.iter().any(|(c, _)| c == "UNUSEDSIGNAL"),
        "{:?}",
        lint.errors
    );
}

// ═══ Yosys ══════════════════════════════════════════════════════════

/// Tasarımdan bağımsız, gerekçeli Yosys bilgi uyarıları.
const YOSYS_GLOBAL_ALLOW: [(&str, &str); 2] = [
    (
        "Replacing memory",
        "dizi yazmaç listesine açılır; sentez notu (ADR-0035/0041 dizileri)",
    ),
    (
        "only limited support for tri-state logic",
        "inout/opendrain (ADR-0051) tri-state üretir; bilgi notu",
    ),
];

struct Elab {
    errors: Vec<String>,
    warnings: Vec<String>,
}

fn yosys_elaborate(yosys: &Path, top: &str, files: &[PathBuf]) -> Elab {
    let list: Vec<String> = files
        .iter()
        .map(|f| format!("\"{}\"", f.display()))
        .collect();
    let script = format!(
        "read_verilog -sv {}; hierarchy -check -top {top}; proc; check -assert",
        list.join(" ")
    );
    let out = Command::new(yosys)
        .args(["-q", "-p", &script])
        .output()
        .expect("yosys çalışmalı");
    let text = text_of(&out);
    let mut errors: Vec<String> = text
        .lines()
        .filter(|l| l.contains("ERROR"))
        .map(str::to_string)
        .collect();
    if !out.status.success() && errors.is_empty() {
        errors.push(format!("çıkış kodu sıfır değil:\n{text}"));
    }
    let warnings = text
        .lines()
        .filter(|l| l.contains("Warning:") && !l.starts_with("Warnings:"))
        .map(str::to_string)
        .collect();
    Elab { errors, warnings }
}

fn yosys_verdict(yosys: &Path, d: &Design) -> Vec<String> {
    if d.markers.net_skip {
        return Vec::new();
    }
    let dir = out_dir(Kind::Rtl, d).join("rtl");
    let mut files = files_with(&dir, "sv");
    files.extend(d.externs.iter().cloned());
    let (mut errors, mut warnings) = (Vec::new(), Vec::new());
    for top in files_with(&dir, "sv") {
        let e = yosys_elaborate(yosys, &stem(&top), &files);
        errors.extend(e.errors);
        warnings.extend(e.warnings);
    }
    if d.markers.synth_skip {
        return if errors.is_empty() && warnings.is_empty() {
            vec![format!("{}: SYNTH-SKIP bayat — Yosys temiz geçiyor", d.rel)]
        } else {
            Vec::new()
        };
    }
    let mut bad: Vec<String> = errors.iter().map(|e| format!("{}: {e}", d.rel)).collect();
    let allowed = |w: &str| {
        YOSYS_GLOBAL_ALLOW.iter().any(|(p, _)| w.contains(p))
            || d.markers.yosys_allow.iter().any(|p| w.contains(p.as_str()))
    };
    bad.extend(
        warnings
            .iter()
            .filter(|w| !allowed(w))
            .map(|w| format!("{}: izinsiz uyarı: {w}", d.rel)),
    );
    for p in &d.markers.yosys_allow {
        if !warnings.iter().any(|w| w.contains(p.as_str())) {
            bad.push(format!("{}: YOSYS-ALLOW '{p}' bayat", d.rel));
        }
    }
    bad
}

#[test]
fn yosys_elaborates_sv() {
    let Some(yosys) = tools::require(Tool::Yosys) else {
        return;
    };
    build(Kind::Rtl);
    let bad: Vec<String> = par_map(corpus(), |d| yosys_verdict(&yosys, d))
        .into_iter()
        .flatten()
        .collect();
    assert!(bad.is_empty(), "{} bulgu:\n{}", bad.len(), bad.join("\n"));
}

/// `-9'(a)` (ADR-0079 §1.3) Yosys'te gerçekten hatadır; koşucu görmeli.
#[test]
fn yosys_runner_rejects_invalid_sv() {
    let Some(yosys) = tools::require(Tool::Yosys) else {
        return;
    };
    let dir = work_dir().join("yosys-neg");
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("Bad.sv");
    std::fs::write(
        &bad,
        "module Bad(input logic signed [7:0] a, output logic signed [8:0] y);\n  assign y = -9'(a);\nendmodule\n",
    )
    .unwrap();
    assert!(!yosys_elaborate(&yosys, "Bad", &[bad]).errors.is_empty());
}

// ═══ XDC: Tcl söz dizimi kapısı ════════════════════════════════════

/// Güvenli alt yorumlayıcıda Volt'un ürettiği Vivado komut alt kümesi:
/// her komut seçeneklerini ve konumsal argüman sayısını denetler.
/// Bilinmeyen komut, bilinmeyen seçenek, dengesiz parantez → hata.
const XDC_GATE: &str = r#"
proc xdc_check {cmd flags nmin nmax argv} {
    set pos {}
    for {set i 0} {$i < [llength $argv]} {incr i} {
        set a [lindex $argv $i]
        if {[string match -* $a] && ![string is double -strict $a]} {
            if {![dict exists $flags $a]} { error "$cmd: unknown option $a" }
            if {[dict get $flags $a]} {
                incr i
                if {$i >= [llength $argv]} { error "$cmd: $a needs a value" }
            }
        } else { lappend pos $a }
    }
    if {[llength $pos] < $nmin || [llength $pos] > $nmax} {
        error "$cmd: [llength $pos] positional argument(s), expected $nmin..$nmax"
    }
    return $pos
}
proc num {cmd v} { if {![string is double -strict $v]} { error "$cmd: '$v' is not a number" } }
proc create_clock {args} {
    xdc_check create_clock {-name 1 -period 1 -waveform 1 -add 0} 0 1 $args
    num create_clock [lindex $args [expr {[lsearch -exact $args -period] + 1}]]
}
proc set_max_delay {args} {
    num set_max_delay [lindex [xdc_check set_max_delay {-datapath_only 0 -from 1 -to 1 -through 1} 1 1 $args] 0]
}
proc set_bus_skew {args} {
    num set_bus_skew [lindex [xdc_check set_bus_skew {-from 1 -to 1 -through 1} 1 1 $args] 0]
}
proc set_false_path {args} { xdc_check set_false_path {-from 1 -to 1 -through 1 -setup 0 -hold 0} 0 0 $args }
proc set_multicycle_path {args} {
    num set_multicycle_path [lindex [xdc_check set_multicycle_path {-from 1 -to 1 -through 1 -setup 0 -hold 0 -start 0 -end 0} 1 1 $args] 0]
}
proc set_clock_groups {args} {
    xdc_check set_clock_groups {-asynchronous 0 -logically_exclusive 0 -physically_exclusive 0 -group 1 -name 1} 0 0 $args
}
proc set_property {args} {
    set p [xdc_check set_property {} 3 3 $args]
    if {[lsearch -exact {ASYNC_REG} [lindex $p 0]] < 0} { error "set_property: unknown property [lindex $p 0]" }
}
proc get_ports {args} { lindex [xdc_check get_ports {-quiet 0} 1 1 $args] 0 }
proc get_cells {args} { lindex [xdc_check get_cells {-quiet 0 -hierarchical 0} 1 1 $args] 0 }
proc get_clocks {args} { lindex [xdc_check get_clocks {-quiet 0 -of_objects 1} 0 1 $args] 0 }
proc get_pins {args} { lindex [xdc_check get_pins {-quiet 0 -hierarchical 0} 1 1 $args] 0 }
set cmds {xdc_check num create_clock set_max_delay set_bus_skew set_false_path
          set_multicycle_path set_clock_groups set_property get_ports get_cells get_clocks get_pins}
set failures 0
foreach f $argv_files {
    set child [interp create -safe]
    foreach c $cmds { $child eval [list proc $c [info args $c] [info body $c]] }
    set fh [open $f r]; set text [read $fh]; close $fh
    if {[catch {$child eval $text} err]} {
        puts "XDC-FAIL $f: $err"
        incr failures
    }
    interp delete $child
}
puts "XDC-CHECKED [llength $argv_files] FAILED $failures"
"#;

/// XDC dosyalarını kapıdan geçirir; `(denetlenen, hatalar)`.
fn xdc_gate(yosys: &Path, files: &[PathBuf], dir: &Path) -> (usize, Vec<String>) {
    std::fs::create_dir_all(dir).unwrap();
    let list: Vec<String> = files
        .iter()
        .map(|f| format!("{{{}}}", f.display().to_string().replace('\\', "/")))
        .collect();
    let script = dir.join("xdc_gate.tcl");
    std::fs::write(
        &script,
        format!("set argv_files [list {}]\n{XDC_GATE}", list.join(" ")),
    )
    .unwrap();
    let out = Command::new(yosys)
        .arg("-q")
        .arg("-c")
        .arg(&script)
        .output()
        .expect("yosys çalışmalı");
    let text = text_of(&out);
    let checked = text
        .lines()
        .find_map(|l| l.strip_prefix("XDC-CHECKED "))
        .and_then(|l| l.split_whitespace().next()?.parse().ok());
    let Some(checked) = checked else {
        return (0, vec![format!("Tcl kapısı koşmadı:\n{text}")]);
    };
    let fails = text
        .lines()
        .filter(|l| l.starts_with("XDC-FAIL"))
        .map(str::to_string)
        .collect();
    (checked, fails)
}

#[test]
fn yosys_tcl_evaluates_xdc() {
    let Some(yosys) = tools::require(Tool::Yosys) else {
        return;
    };
    build(Kind::Rtl);
    let files: Vec<PathBuf> = corpus()
        .iter()
        .flat_map(|d| files_with(&out_dir(Kind::Rtl, d).join("constraints"), "xdc"))
        .collect();
    let (checked, fails) = xdc_gate(&yosys, &files, &work_dir().join("xdc"));
    assert!(fails.is_empty(), "{}", fails.join("\n"));
    assert_eq!(checked, files.len());
    assert!(checked >= 100, "XDC sayısı {checked}");
}

#[test]
fn yosys_tcl_gate_rejects_bad_xdc() {
    let Some(yosys) = tools::require(Tool::Yosys) else {
        return;
    };
    let dir = work_dir().join("xdc-neg");
    std::fs::create_dir_all(&dir).unwrap();
    let cases = [
        "create_clock -name c -period 10.0 [get_ports c]\nset_max_delay -datapath_only [get_cells {a*}]\n",
        "create_clock -nmae c -period 10.0 [get_ports c]\n",
        "set_input_jitter c 0.1\n",
        "set_false_path -from [get_cells {a*}\n",
    ];
    let files: Vec<PathBuf> = cases
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let f = dir.join(format!("bad{i}.xdc"));
            std::fs::write(&f, text).unwrap();
            f
        })
        .collect();
    let (checked, fails) = xdc_gate(&yosys, &files, &dir);
    assert_eq!(
        (checked, fails.len()),
        (cases.len(), cases.len()),
        "{fails:?}"
    );
}

// ═══ Yazılım sürücüleri (C, C++, Rust) ═════════════════════════════

fn sw_outputs(ext: &str) -> Vec<PathBuf> {
    build(Kind::Rtl);
    corpus()
        .iter()
        .flat_map(|d| files_with(&out_dir(Kind::Rtl, d).join("sw"), ext))
        .collect()
}

fn compile_header(cc: &Path, header: &Path, cxx: bool) -> Result<(), String> {
    let (std, lang) = if cxx {
        ("-std=c++17", "c++")
    } else {
        ("-std=c99", "c")
    };
    let out = Command::new(cc)
        .args([
            std,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pedantic",
            "-fsyntax-only",
            "-x",
            lang,
        ])
        .arg(header)
        .output()
        .expect("derleyici çalışmalı");
    out.status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{}: {}", header.display(), text_of(&out)))
}

fn compile_all_headers(tool: Tool, cxx: bool) {
    let Some(cc) = tools::require(tool) else {
        return;
    };
    let headers = sw_outputs("h");
    let bad: Vec<String> = par_map(&headers, |h| compile_header(&cc, h, cxx).err())
        .into_iter()
        .flatten()
        .collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert!(headers.len() >= 5, "başlık sayısı {}", headers.len());
    let dir = work_dir().join(if cxx { "cxx-neg" } else { "cc-neg" });
    std::fs::create_dir_all(&dir).unwrap();
    let neg = dir.join("bad.h");
    std::fs::write(
        &neg,
        "static inline int f(void) { int unused; return 0; }\n",
    )
    .unwrap();
    assert!(
        compile_header(&cc, &neg, cxx).is_err(),
        "koşucu -Werror'u görmüyor"
    );
}

#[test]
fn cc_compiles_c_headers() {
    compile_all_headers(Tool::Cc, false);
}

#[test]
fn cxx_compiles_c_headers() {
    compile_all_headers(Tool::Cxx, true);
}

fn compile_rust(rustc: &Path, file: &Path, out: &Path) -> Result<(), String> {
    let o = Command::new(rustc)
        .args(["--edition", "2021", "--crate-type", "lib", "-D", "warnings"])
        .args(["--emit=metadata", "-o"])
        .arg(out)
        .arg(file)
        .output()
        .expect("rustc çalışmalı");
    o.status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{}: {}", file.display(), text_of(&o)))
}

#[test]
fn rustc_compiles_rust_drivers() {
    let Some(rustc) = tools::require(Tool::Rustc) else {
        return;
    };
    let drivers = sw_outputs("rs");
    let dir = work_dir().join("rustc");
    std::fs::create_dir_all(&dir).unwrap();
    let bad: Vec<String> = par_map(&drivers, |f| {
        // <iş>/<kip>/<tasarım>/sw/<ad>.rs: tasarım kimliği adı tekilleştirir.
        let design = stem(f.parent().and_then(Path::parent).unwrap());
        let meta = dir.join(format!("{design}-{}.rmeta", stem(f)));
        compile_rust(&rustc, f, &meta).err()
    })
    .into_iter()
    .flatten()
    .collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert!(drivers.len() >= 5, "sürücü sayısı {}", drivers.len());
    let neg = dir.join("bad.rs");
    std::fs::write(&neg, "#![no_std]\npub fn f() { let unused = 1; }\n").unwrap();
    assert!(compile_rust(&rustc, &neg, &dir.join("bad.rmeta")).is_err());
}

// ═══ regmap JSON ════════════════════════════════════════════════════

/// `volt-regmap/1` (ADR-0053 §5, ADR-0063): anahtar tipleri, adres =
/// taban + offset, 4 bayt hizası, benzersiz ad/offset, alanlar ardışık
/// ve 32 biti aşmaz.
fn regmap_schema_errors(v: &Value) -> Vec<String> {
    let mut e = Vec::new();
    let is_str_or_null = |x: &Value| x.is_string() || x.is_null();
    if v["schema"] != "volt-regmap/1" {
        e.push(format!("schema {}", v["schema"]));
    }
    for k in ["generator", "source", "regmap_hash", "name", "bus"] {
        if !v[k].is_string() {
            e.push(format!("{k} string değil"));
        }
    }
    if !is_str_or_null(&v["doc"]) {
        e.push("doc".into());
    }
    let base = v["base"].as_u64();
    let Some(regs) = v["registers"].as_array().filter(|r| !r.is_empty()) else {
        e.push("registers boş ya da dizi değil".into());
        return e;
    };
    let (mut names, mut offsets) = (BTreeSet::new(), BTreeSet::new());
    for r in regs {
        let name = r["name"].as_str().unwrap_or("?");
        let off = r["offset"].as_u64();
        if !names.insert(name.to_string()) {
            e.push(format!("{name}: ad tekrar"));
        }
        match (base, off, r["address"].as_u64()) {
            (Some(b), Some(o), Some(a)) if a == b + o && o % 4 == 0 && offsets.insert(o) => {}
            _ => e.push(format!("{name}: base/offset/address tutarsız ya da tekrar")),
        }
        if !matches!(r["access"].as_str(), Some("rw" | "ro" | "wo")) {
            e.push(format!("{name}: access {}", r["access"]));
        }
        if !r["volatile"].is_boolean() || !r["reset"].is_u64() || !is_str_or_null(&r["doc"]) {
            e.push(format!("{name}: volatile/reset/doc tipi"));
        }
        e.extend(field_errors(name, &r["fields"]));
    }
    e
}

fn field_errors(reg: &str, fields: &Value) -> Vec<String> {
    let mut e = Vec::new();
    let Some(fields) = fields.as_array().filter(|f| !f.is_empty()) else {
        return vec![format!("{reg}: fields boş")];
    };
    let mut next = 0;
    for f in fields {
        let name = f["name"].as_str().unwrap_or("?");
        let (lsb, width) = (f["lsb"].as_u64(), f["width"].as_u64());
        match (lsb, width) {
            (Some(l), Some(w)) if l == next && w > 0 => next = l + w,
            _ => e.push(format!("{reg}.{name}: alanlar ardışık değil")),
        }
        if !matches!(f["type"].as_str(), Some("bool" | "bits" | "uint")) {
            e.push(format!("{reg}.{name}: type {}", f["type"]));
        }
        for k in ["reserved", "self_clearing", "w1c"] {
            if !f[k].is_boolean() {
                e.push(format!("{reg}.{name}: {k} bool değil"));
            }
        }
    }
    if next > 32 {
        e.push(format!("{reg}: alanlar {next} bit (> 32)"));
    }
    e
}

#[test]
fn regmap_json_matches_schema_and_check_regmap() {
    build(Kind::Rtl);
    let mut checked = 0;
    let mut bad = Vec::new();
    for d in corpus() {
        let sw = out_dir(Kind::Rtl, d).join("sw");
        let jsons = files_with(&sw, "json");
        for json in &jsons {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", json.display()));
            bad.extend(
                regmap_schema_errors(&v)
                    .into_iter()
                    .map(|e| format!("{}: {e}", json.display())),
            );
            checked += 1;
        }
        if jsons.is_empty() {
            continue;
        }
        // Gerçek okuyucu: `volt check-regmap` üç biçimi de ayrıştırır (ADR-0063).
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
        cmd.args(["--lang", "en", "check-regmap"])
            .arg(&d.rel)
            .current_dir(root());
        for ext in ["h", "rs", "json"] {
            for f in files_with(&sw, ext) {
                cmd.arg("--against").arg(f);
            }
        }
        let out = cmd.output().expect("volt çalışmalı");
        if !out.status.success() {
            bad.push(format!("{}: check-regmap: {}", d.rel, text_of(&out)));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert!(checked >= 5, "JSON sayısı {checked}");
}

#[test]
fn regmap_schema_rejects_overlapping_fields() {
    let v: Value = serde_json::json!({
        "schema": "volt-regmap/1", "generator": "volt", "source": "x.volt",
        "regmap_hash": "0", "name": "X", "base": 0, "bus": "AXI4Lite", "doc": null,
        "registers": [{
            "name": "ctrl", "offset": 0, "address": 0, "access": "rw", "reset": 0,
            "volatile": false, "doc": null,
            "fields": [
                {"name": "a", "lsb": 0, "width": 8, "type": "uint", "reserved": false, "self_clearing": false, "w1c": false, "doc": null},
                {"name": "b", "lsb": 4, "width": 8, "type": "uint", "reserved": false, "self_clearing": false, "w1c": false, "doc": null}
            ]
        }]
    });
    assert_eq!(
        regmap_schema_errors(&v).len(),
        1,
        "{:?}",
        regmap_schema_errors(&v)
    );
}

// ═══ İşaretler ══════════════════════════════════════════════════════

#[test]
fn net_markers_parse_and_require_reasons() {
    let m = parse_markers(
        "//~ LINT-ALLOW: UNUSEDSIGNAL: yalnız kontrat okur\n//~ YOSYS-ALLOW: is assigned: ayrık bitler\n//~ SYNTH-SKIP: tri1\n",
    )
    .unwrap();
    assert_eq!(m.lint_allow, ["UNUSEDSIGNAL"]);
    assert_eq!(m.yosys_allow, ["is assigned"]);
    assert!(m.synth_skip && !m.net_skip);
    for bad in [
        "//~ LINT-ALLOW: UNUSEDSIGNAL\n",
        "//~ LINT-ALLOW UNUSEDSIGNAL: neden\n",
        "//~ NET-SKIP:\n",
        "//~ YOSYS-ALLOW: : neden\n",
    ] {
        assert!(parse_markers(bad).is_err(), "{bad}");
    }
    // Korpustaki her işaret ayrıştırılır (bozuk işaret corpus()'ta düşer).
    assert!(corpus().iter().any(|d| !d.markers.lint_allow.is_empty()));
}

#[test]
fn require_tools_rejects_unknown_names() {
    assert_eq!(tools::parse_required("").unwrap(), []);
    assert_eq!(
        tools::parse_required("verilator, cc").unwrap(),
        [Tool::Verilator, Tool::Cc]
    );
    assert_eq!(tools::parse_required("all").unwrap().len(), Tool::ALL.len());
    assert!(tools::parse_required("verilater").is_err());
}
