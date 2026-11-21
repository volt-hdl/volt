// Sürüm kimliği metni (ADR-0104). `build.rs` bu dosyayı `include!` ile
// kullanır; kütüphane aynı fonksiyonları birim testleriyle sınar. Bu
// yüzden dosyada iç belge yorumu (`//!`) yok.

/// `volt --version`'ın ve üretilen dosya başlıklarının sürüm metni:
/// `0.1.0 (a1b2c3d)`; derlendiği commit bilinmiyorsa yalnız `0.1.0`.
pub fn version_text(version: &str, commit: Option<&str>) -> String {
    match commit {
        Some(c) => format!("{version} ({c})"),
        None => version.to_string(),
    }
}

/// Tam commit özetinin ilk 7 karakteri (küçük harf); onaltılık olmayan
/// ya da 7 karakterden kısa girdi `None`.
pub fn short_commit(full: &str) -> Option<String> {
    let full = full.trim();
    (full.len() >= 7 && full.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| full[..7].to_ascii_lowercase())
}

/// `build.rs`'in `cargo:rerun-if-changed` ile izlediği git dosyaları: HEAD,
/// HEAD bir dala işaret ediyorsa o dalın dosyası ve `packed-refs`; yalnız
/// `exists`'in var dediği yollar. Cargo var olmayan bir yolu her çağrıda
/// değişmiş sayar ve build script'i yeniden koşturur (#366). Dosya sonradan
/// oluşursa (dal `packed-refs`'e taşınır) izlenen gevşek dosya silinmiş
/// olur; cargo bunu da değişiklik sayar, build script yeniden koşar.
/// `git_dir` çalışma ağacının, `common_dir` deponun ortak git dizinidir;
/// `head` HEAD dosyasının içeriğidir.
// Kütüphane bu fonksiyonu yalnız testlerde çağırır; build.rs kullanır.
#[cfg_attr(not(test), allow(dead_code))]
pub fn git_rerun_paths(
    git_dir: &std::path::Path,
    common_dir: &std::path::Path,
    head: Option<&str>,
    exists: &dyn Fn(&std::path::Path) -> bool,
) -> Vec<std::path::PathBuf> {
    let mut paths = vec![git_dir.join("HEAD")];
    if let Some(reference) = head.and_then(|h| h.strip_prefix("ref: ")) {
        paths.push(common_dir.join(reference.trim()));
    }
    paths.push(common_dir.join("packed-refs"));
    paths.retain(|p| exists(p));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // #366: CI'ın sığ, ayrık checkout'unda `packed-refs` yoktur. Cargo var
    // olmayan bir `rerun-if-changed` yolunu her çağrıda "değişti" sayar;
    // build script her `cargo` çağrısında yeniden koşar ve volt-sv-emit ile
    // ona bağlı crate'ler yeniden derlenir (mutant başına ~14 s).
    #[test]
    fn a_git_file_that_does_not_exist_is_not_watched() {
        let git = Path::new("w").join(".git");
        let exists = |p: &Path| p == git.join("HEAD");
        assert_eq!(
            git_rerun_paths(&git, &git, Some("0123456789abcdef\n"), &exists),
            vec![git.join("HEAD")],
            "ayrık HEAD, packed-refs yok"
        );
    }

    #[test]
    fn head_its_branch_and_packed_refs_are_watched_when_they_exist() {
        let git = Path::new("w").join(".git");
        let all = |_: &Path| true;
        assert_eq!(
            git_rerun_paths(&git, &git, Some("ref: refs/heads/main\n"), &all),
            vec![
                git.join("HEAD"),
                git.join("refs/heads/main"),
                git.join("packed-refs"),
            ]
        );
        // Dal packed-refs'e taşınmışsa (gevşek dosyası yok) packed-refs izlenir.
        let packed = |p: &Path| p != git.join("refs/heads/main");
        assert_eq!(
            git_rerun_paths(&git, &git, Some("ref: refs/heads/main\n"), &packed),
            vec![git.join("HEAD"), git.join("packed-refs")]
        );
    }

    #[test]
    fn the_text_names_the_commit_when_known() {
        assert_eq!(version_text("0.1.0", Some("a1b2c3d")), "0.1.0 (a1b2c3d)");
        assert_eq!(version_text("0.1.0", None), "0.1.0");
    }

    #[test]
    fn the_short_commit_is_seven_lowercase_hex_digits() {
        assert_eq!(
            short_commit("E289D864ABCDEF0123456789\n").as_deref(),
            Some("e289d86")
        );
        assert_eq!(short_commit("e289d8"), None, "kısa");
        assert_eq!(short_commit("not-a-hash"), None, "onaltılık değil");
        assert_eq!(short_commit(""), None);
    }
}
