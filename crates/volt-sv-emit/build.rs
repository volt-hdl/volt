//! Sürüm kimliği (ADR-0104): ikilinin derlendiği commit'in kısaltması
//! derleme zamanında `VOLT_VERSION_TEXT`'e yazılır. Depo bir git çalışma
//! ağacı değilse (sürüm arşivi, `git archive`, git kurulu değil) ya da
//! kaynak başka bir deponun içine açılmışsa yalnız sürüm yazılır. Derleme
//! tarihi eklenmez: aynı commit'ten her derleme aynı metni üretir.

use std::path::{Path, PathBuf};
use std::process::Command;

include!("src/version_text.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/version_text.rs");
    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let commit = git_commit();
    println!(
        "cargo:rustc-env=VOLT_VERSION_TEXT={}",
        version_text(&version, commit.as_deref())
    );
}

fn workspace_root() -> Option<PathBuf> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    Path::new(&manifest).join("../..").canonicalize().ok()
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_commit() -> Option<String> {
    let root = workspace_root()?;
    // Çalışma ağacının kökü bu depo olmalı: başka bir deponun içine açılmış
    // bir kaynak arşivi o deponun commit'ini almaz.
    let top = PathBuf::from(git(&root, &["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .ok()?;
    if top != root {
        return None;
    }
    rerun_on_new_commit(&root);
    short_commit(&git(&root, &["rev-parse", "HEAD"])?)
}

/// HEAD ya da işaret ettiği dal ilerleyince build.rs yeniden koşar. Yalnız
/// var olan dosyalar izlenir (`git_rerun_paths`, #366): CI'ın sığ
/// checkout'unda `packed-refs` yoktur ve var olmayan bir yol build
/// script'i her `cargo` çağrısında yeniden koşturuyordu.
fn rerun_on_new_commit(root: &Path) {
    let Some(git_dir) = git(root, &["rev-parse", "--absolute-git-dir"]) else {
        return;
    };
    let git_dir = PathBuf::from(git_dir);
    let common = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map(PathBuf::from)
    .unwrap_or_else(|| git_dir.clone());
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok();
    for path in git_rerun_paths(&git_dir, &common, head.as_deref(), &|p| p.exists()) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
