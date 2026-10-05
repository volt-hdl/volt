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

#[cfg(test)]
mod tests {
    use super::*;

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
