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

/// HEAD ya da işaret ettiği dal ilerleyince build.rs yeniden koşar.
fn rerun_on_new_commit(root: &Path) {
    let Some(git_dir) = git(root, &["rev-parse", "--absolute-git-dir"]) else {
        return;
    };
    let head = Path::new(&git_dir).join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    let common = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .unwrap_or(git_dir);
    if let Some(reference) = std::fs::read_to_string(&head)
        .ok()
        .and_then(|h| h.strip_prefix("ref: ").map(|r| r.trim().to_string()))
    {
        println!(
            "cargo:rerun-if-changed={}",
            Path::new(&common).join(reference).display()
        );
    }
    println!(
        "cargo:rerun-if-changed={}",
        Path::new(&common).join("packed-refs").display()
    );
}
