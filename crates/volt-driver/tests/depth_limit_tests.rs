//! Yığın taşması sınıfı uçtan uca (ADR-0080): gerçek `volt` ikilisi AYRI
//! SÜREÇTE koşar — yığın taşması Rust'ta panik değil abort'tur, süreç
//! içinde yakalanamaz; burada çıkış kodu olarak görünür (Windows
//! 0xC00000FD, Unix SIGSEGV/SIGABRT) ve test düşer.
//!
//! * `tests/fuzz_regressions/stack_*.volt`: düzeltmeden önce abort; şimdi
//!   `check` ve `build` çıkış 1 + tek E0018.
//! * Sınırın hemen altındaki GEÇERLİ tasarımlar tüm geçitlerden (çözümleme,
//!   tip, saat alanı, SV/SVA/SDC üretimi) geçer: derleyici yığınının
//!   (`COMPILER_STACK_SIZE`) sınıra yettiğinin kanıtı. Windows ana iş
//!   parçacığı (1 MB) bu ağaçları debug derlemede taşıyamaz.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use volt_syntax::MAX_DEPTH;

const D: usize = MAX_DEPTH as usize - 8;
/// Süreç başına üst sınır; ölçülen ~0,1 s (debug). Aşım = kaynak patlaması.
const TIME_LIMIT: Duration = Duration::from_secs(5);

fn volt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    cmd.env("VOLT_LANG", "en");
    cmd
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-depth-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn regressions() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fuzz_regressions");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("tests/fuzz_regressions okunmalı")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("stack_")
        })
        .collect();
    files.sort();
    assert!(files.len() >= 14, "{} stack_ girdisi", files.len());
    files
}

/// Koşar; abort (çıkış kodu 0/1/2 dışı ya da sinyal) ve süre aşımı `Err`.
fn try_run(what: &str, cmd: &mut Command) -> Result<Output, String> {
    let start = Instant::now();
    let out = cmd.output().expect("volt çalışmalı");
    let elapsed = start.elapsed();
    if !matches!(out.status.code(), Some(0..=2)) {
        let text = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "{what}: ABORT — süreç çöktü (yığın taşması?), durum {:?}: {}",
            out.status,
            text.lines().last().unwrap_or_default()
        ));
    }
    if elapsed >= TIME_LIMIT {
        return Err(format!("{what}: {elapsed:?}"));
    }
    Ok(out)
}

fn run(what: &str, cmd: &mut Command) -> Output {
    try_run(what, cmd).unwrap_or_else(|e| panic!("{e}"))
}

fn error_codes(out: &Output) -> Vec<String> {
    let text = String::from_utf8_lossy(&out.stderr);
    text.lines()
        .filter_map(|l| {
            let i = l.find("error[")?;
            Some(l[i + 6..i + 11].to_string())
        })
        .collect()
}

/// Tüm girdiler dolaşılır, sonuç topluca raporlanır: bir koruma
/// kaldırılınca (mutasyon) hangi girdinin abort, hangisinin yanlış tanı
/// verdiği birlikte görünür.
#[test]
fn stack_regressions_are_one_e0018_in_check_and_build_not_an_abort() {
    let target = temp_dir("regress");
    let mut failures = Vec::new();
    for path in regressions() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut check = volt();
        check.args(["check", "--format", "short"]).arg(&path);
        let mut build = volt();
        build
            .args([
                "build",
                "--format",
                "short",
                "--emit",
                "sva,sdc",
                "--target-dir",
            ])
            .arg(&target)
            .arg(&path);
        for (what, cmd) in [("check", &mut check), ("build", &mut build)] {
            match try_run(&format!("{what} {name}"), cmd) {
                Err(e) => failures.push(e),
                Ok(out) if out.status.code() != Some(1) || error_codes(&out) != ["E0018"] => {
                    failures.push(format!(
                        "{what} {name}: çıkış {:?}, tanılar {:?}",
                        out.status.code(),
                        error_codes(&out)
                    ))
                }
                Ok(_) => {}
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

fn module(e: &str) -> String {
    format!("module Top {{\n    in a : u8\n    out y : u8\n    y = {e}\n}}\n")
}

fn seq(body: &str) -> String {
    format!(
        "module Top {{\n    in clk : clock\n    in a : u8\n    out y : u8\n    reg r : u8 = 0\n    on clk {{\n{body}\n    }}\n    y = r\n}}\n"
    )
}

/// Sınırın hemen altındaki geçerli tasarımlar (ad, kaynak).
fn near_limit_designs() -> Vec<(&'static str, String)> {
    let xor = vec!["a"; D].join(" ^ ");
    let else_if: String = (1..D)
        .map(|i| format!(" else if a == {} {{ r <= {} }}", i % 250, i % 250))
        .collect();
    vec![
        ("zincir", module(&xor)),
        (
            "generic zincir",
            format!(
                "module G<const N: u32> {{\n    in a : u8\n    out y : u8\n    y = {xor}\n}}\n\
                 module Top {{\n    in a : u8\n    out y : u8\n    let g = G<1> {{ a }}\n    y = g.y\n}}\n"
            ),
        ),
        ("parantez", module(&format!("{}a{}", "(".repeat(D), ")".repeat(D)))),
        ("tekli", module(&format!("{}a", "~".repeat(D)))),
        ("as zinciri", module(&format!("a{}", " as u8".repeat(D)))),
        (
            "if ifadesi",
            module(&format!("{}a{}", "if a == 0 { ".repeat(D), " } else { a }".repeat(D))),
        ),
        ("else if", seq(&format!("if a == 0 {{ r <= 0 }}{else_if}"))),
        (
            "iç içe if",
            seq(&format!("{}r <= a{}", "if a != 0 { ".repeat(D), " }".repeat(D))),
        ),
        (
            "iç içe match",
            seq(&format!(
                "{}r <= a{}",
                "match a { 0 => { ".repeat(D / 2),
                " } _ => { r <= 0 } }".repeat(D / 2)
            )),
        ),
    ]
}

#[test]
fn designs_just_below_the_limit_pass_every_stage_on_the_compiler_stack() {
    let dir = temp_dir("near");
    for (name, src) in near_limit_designs() {
        let file = dir.join(format!("{}.volt", name.replace(' ', "_")));
        std::fs::write(&file, src).expect("yazılmalı");
        let target = dir.join(format!("out_{}", name.replace(' ', "_")));
        let out = run(
            &format!("build {name}"),
            volt()
                .args(["build", "--emit", "sva,sdc", "--target-dir"])
                .arg(&target)
                .arg(&file),
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            target.join("rtl").join("Top.sv").is_file(),
            "{name}: Top.sv üretilmeli"
        );
    }
}
