//! Renk (cli-contract.md §3 "Genel Bayraklar" ve "Renk Davranışı", #95):
//! `--color auto|always|never`, `--no-color` ve `VOLT_COLOR` her komutta
//! geçerlidir ve hata/uyarı tanılarını da renklendirir. Öncelik: bayrak >
//! `VOLT_COLOR` > auto; auto'da terminal değilse, `NO_COLOR` ya da `CI`
//! ayarlıysa renk yok. Testler boru üzerinden koşar (terminal yok):
//! auto'nun terminal kolu `color.rs` birim testlerindedir.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ESC: &str = "\x1b[";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ui/fail/01_cdc_violation.volt")
}

/// Renk ortamı temizlenmiş komut: üst kabuğun NO_COLOR/CI/VOLT_COLOR'u
/// testi etkilemez.
fn volt(envs: &[(&str, &str)]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_volt"));
    c.env_remove("NO_COLOR")
        .env_remove("CI")
        .env_remove("VOLT_COLOR")
        .env("VOLT_LANG", "en");
    for (k, v) in envs {
        c.env(k, v);
    }
    c
}

fn run(mut c: Command, args: &[&str]) -> Output {
    c.args(args).output().expect("volt çalışmalı")
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn check_accepts_color_always_and_colors_the_error() {
    let f = fixture();
    let o = run(
        volt(&[]),
        &["check", "--color", "always", f.to_str().unwrap()],
    );
    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    let err = stderr(&o);
    assert!(err.contains(ESC), "renk kodu yok:\n{err}");
    assert!(err.contains("E3001"), "{err}");
}

#[test]
fn the_flag_works_before_the_command_too() {
    let f = fixture();
    let o = run(
        volt(&[]),
        &["--color", "always", "check", f.to_str().unwrap()],
    );
    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    assert!(stderr(&o).contains(ESC));
}

#[test]
fn volt_color_always_colors_the_diagnostics_of_check_and_build() {
    let f = fixture();
    let target = std::env::temp_dir().join(format!("volt-color-{}", std::process::id()));
    for args in [
        vec!["check", f.to_str().unwrap()],
        vec![
            "build",
            "--target-dir",
            target.to_str().unwrap(),
            f.to_str().unwrap(),
        ],
    ] {
        let o = run(volt(&[("VOLT_COLOR", "always")]), &args);
        assert_eq!(o.status.code(), Some(1), "{args:?}: {}", stderr(&o));
        assert!(stderr(&o).contains(ESC), "{args:?}: renk kodu yok");
    }
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn warnings_are_colored_too() {
    let dir = std::env::temp_dir().join(format!("volt-color-warn-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dizin");
    let file = dir.join("w.volt");
    std::fs::write(
        &file,
        "module W {\n    in  a : u8\n    out b : u8\n    in  c : u8\n    b = a\n}\n",
    )
    .expect("yazılmalı");
    let o = run(
        volt(&[]),
        &["check", "--color", "always", file.to_str().unwrap()],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let err = stderr(&o);
    assert!(err.contains("W1001") && err.contains(ESC), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_color_never_and_no_color_flag_leave_the_output_plain() {
    let f = fixture();
    let file = f.to_str().unwrap();
    for (envs, args) in [
        (vec![("NO_COLOR", "1")], vec!["check", file]),
        (
            vec![("VOLT_COLOR", "always")],
            vec!["check", "--color", "never", file],
        ),
        (
            vec![("VOLT_COLOR", "always")],
            vec!["check", "--no-color", file],
        ),
        (vec![], vec!["check", file]),
    ] {
        let o = run(volt(&envs), &args);
        assert_eq!(
            o.status.code(),
            Some(1),
            "{envs:?} {args:?}: {}",
            stderr(&o)
        );
        assert!(
            !stderr(&o).contains(ESC),
            "{envs:?} {args:?}: renk kodu var"
        );
    }
}

#[test]
fn explain_takes_the_same_flag() {
    for args in [
        vec!["explain", "E3001", "--color", "always"],
        vec!["--color", "always", "explain", "E3001"],
    ] {
        let o = run(volt(&[]), &args);
        assert_eq!(o.status.code(), Some(0), "{args:?}");
        assert!(String::from_utf8_lossy(&o.stdout).contains(ESC), "{args:?}");
    }
}

#[test]
fn an_unknown_color_mode_is_a_usage_error() {
    let o = run(volt(&[]), &["check", "--color", "sometimes", "x.volt"]);
    assert_eq!(o.status.code(), Some(2));
}
