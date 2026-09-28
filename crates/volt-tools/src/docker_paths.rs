//! Ana makine ↔ konteyner yol eşlemesi (ADR-0094 §3).
//!
//! Volt ana makinede derler ve dosya yazar; konteynerde yalnız araç
//! koşar. Aracın gördüğü her yol bu eşlemeyle kurulur ve aracın
//! çıktısındaki konteyner yolu kullanıcıya basılmadan önce geri çevrilir:
//! tanılarda ve çıktı satırlarında HER ZAMAN ana makine yolu görünür.
//!
//! Eşleme belirlenimcidir (durum tutmaz): aynı ana makine yolu her
//! çağrıda aynı konteyner yoluna gider, bu yüzden testbench'e gömülen
//! yol (VCD) ile bağlanan dizin ayrı ayrı hesaplansa da tutarlıdır.
//!
//! - Unix: konteyner yolu ana makine yoluyla AYNIDIR — çeviri gerekmez,
//!   araç çıktısındaki yol zaten ana makine yoludur.
//! - Windows: `C:\Dev\demo` → `/volt/c/Dev/demo`. Geri çeviri yalnız
//!   bağlanan dizinlerin önekini tanır (boşluklu yol da doğru döner).

use std::io;
use std::path::{Path, PathBuf};

/// Windows sürücülerinin konteynerdeki kökü.
pub const WINDOWS_ROOT: &str = "/volt";

/// Mutlak ana makine yolunun konteyner karşılığı (metin düzeyinde; iki
/// platform da her yerde sınanabilsin diye `windows` açıkça verilir).
/// Windows'ta sürücü harfi olmayan yol (UNC paylaşımı) `None`dır.
pub fn container_path_str(host: &str, windows: bool) -> Option<String> {
    if !windows {
        return host.starts_with('/').then(|| host.to_string());
    }
    let bytes = host.as_bytes();
    let has_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if !has_drive {
        return None;
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    let rest = host[2..].replace('\\', "/");
    let rest = rest.trim_end_matches('/');
    let rest = if rest.is_empty() || rest.starts_with('/') {
        rest.to_string()
    } else {
        format!("/{rest}")
    };
    Some(format!("{WINDOWS_ROOT}/{drive}{rest}"))
}

/// Mutlak ana makine yolunun bu platformdaki konteyner karşılığı.
pub fn container_path(host: &Path) -> Option<String> {
    container_path_str(&host.to_string_lossy(), cfg!(windows))
}

/// Konteynere bağlanacak ana makine yolu: mutlak, sembolik bağları ve
/// Windows kısa adlarını (`ALAR~1`) çözülmüş, `\\?\` öneki soyulmuş. Kısa
/// adlı yol Docker Desktop'ta bağlanamaz (ölçüm), bu yüzden kanonik yol.
pub fn host_absolute(path: &Path) -> io::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    Ok(strip_verbatim(&canonical))
}

/// `\\?\C:\x` → `C:\x`. `\\?\UNC\...` olduğu gibi kalır (eşlenemez).
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// Bir koşuda konteynere bağlanan dizinler.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mounts {
    /// (ana makine, konteyner) çiftleri, eklenme sırasıyla.
    dirs: Vec<(String, String)>,
    windows: bool,
}

impl Mounts {
    pub fn new() -> Self {
        Self::for_platform(cfg!(windows))
    }

    /// Platformu açıkça verilen eşleme (testler).
    pub fn for_platform(windows: bool) -> Self {
        Self {
            dirs: Vec::new(),
            windows,
        }
    }

    /// Mutlak ana makine dizinini ekler; konteyner yolunu döndürür.
    /// Eşlenemeyen yol (UNC) `None`dır.
    pub fn add_str(&mut self, host: &str) -> Option<String> {
        let container = container_path_str(host, self.windows)?;
        if !self.dirs.iter().any(|(h, _)| h == host) {
            self.dirs.push((host.to_string(), container.clone()));
        }
        Some(container)
    }

    /// Dizini kanonik yoluyla ekler.
    pub fn add(&mut self, dir: &Path) -> io::Result<String> {
        let host = host_absolute(dir)?;
        self.add_str(&host.to_string_lossy()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "'{}' cannot be mounted into a container (network share)",
                    host.display()
                ),
            )
        })
    }

    /// `docker run -v` argümanları (`<ana makine>:<konteyner>`).
    pub fn volume_args(&self) -> Vec<String> {
        self.dirs
            .iter()
            .flat_map(|(h, c)| ["-v".to_string(), format!("{h}:{c}")])
            .collect()
    }

    /// Araç çıktısındaki konteyner yollarını ana makine yoluna çevirir.
    /// Önek bir yol bileşeni sınırında eşleşmeli; önekten sonraki yol
    /// kuyruğu Windows'ta `\` ayraçlı olur.
    pub fn host_text(&self, text: &str) -> String {
        let mut pairs: Vec<&(String, String)> = self.dirs.iter().filter(|(h, c)| h != c).collect();
        if pairs.is_empty() {
            return text.to_string();
        }
        // En uzun önek önce: iç içe bağlamada daha özel olan kazanır.
        pairs.sort_by_key(|(_, c)| std::cmp::Reverse(c.len()));
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        'scan: while !rest.is_empty() {
            for (host, container) in &pairs {
                if let Some(after) = rest.strip_prefix(container.as_str()) {
                    if after.is_empty() || after.starts_with('/') || is_delimiter(after) {
                        out.push_str(host);
                        let tail_len = after.find(|c: char| is_path_end(c)).unwrap_or(after.len());
                        let tail = &after[..tail_len];
                        if self.windows {
                            out.push_str(&tail.replace('/', "\\"));
                        } else {
                            out.push_str(tail);
                        }
                        rest = &after[tail_len..];
                        continue 'scan;
                    }
                }
            }
            let ch = rest.chars().next().expect("boş değil");
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
        out
    }
}

/// Yol kuyruğunu bitiren karakter (araç iletilerindeki `dosya:satır`,
/// tırnak, parantez, boşluk).
fn is_path_end(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            ':' | '\'' | '"' | '`' | '(' | ')' | ',' | ';' | '<' | '>' | '|'
        )
}

fn is_delimiter(after: &str) -> bool {
    after.chars().next().is_some_and(is_path_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_drive_paths_map_under_the_volt_root() {
        assert_eq!(
            container_path_str(r"C:\Dev\demo\build\sim", true).as_deref(),
            Some("/volt/c/Dev/demo/build/sim")
        );
        assert_eq!(
            container_path_str(r"d:\x\", true).as_deref(),
            Some("/volt/d/x")
        );
        assert_eq!(container_path_str(r"C:\", true).as_deref(), Some("/volt/c"));
        assert_eq!(
            container_path_str(r"C:\Users\Çağlar\p q", true).as_deref(),
            Some("/volt/c/Users/Çağlar/p q")
        );
    }

    #[test]
    fn windows_paths_without_a_drive_are_not_mappable() {
        assert_eq!(container_path_str(r"\\server\share\x", true), None);
        assert_eq!(container_path_str(r"relative\x", true), None);
    }

    #[test]
    fn unix_paths_map_to_themselves() {
        assert_eq!(
            container_path_str("/home/u/p", false).as_deref(),
            Some("/home/u/p")
        );
        assert_eq!(container_path_str("rel/p", false), None);
    }

    #[test]
    fn mounts_dedupe_and_emit_volume_arguments() {
        let mut m = Mounts::for_platform(true);
        assert_eq!(m.add_str(r"C:\a\b").as_deref(), Some("/volt/c/a/b"));
        m.add_str(r"C:\a\b");
        m.add_str(r"D:\w");
        assert_eq!(
            m.volume_args(),
            ["-v", r"C:\a\b:/volt/c/a/b", "-v", r"D:\w:/volt/d/w"]
        );
    }

    #[test]
    fn host_text_translates_container_paths_back_on_windows() {
        let mut m = Mounts::for_platform(true);
        m.add_str(r"C:\Dev\demo\build\sim\x_test");
        let line = "%Error: /volt/c/Dev/demo/build/sim/x_test/obj/X.sv:12:3: bad";
        assert_eq!(
            m.host_text(line),
            r"%Error: C:\Dev\demo\build\sim\x_test\obj\X.sv:12:3: bad"
        );
        assert_eq!(
            m.host_text("in '/volt/c/Dev/demo/build/sim/x_test'"),
            r"in 'C:\Dev\demo\build\sim\x_test'"
        );
    }

    #[test]
    fn host_text_needs_a_component_boundary_and_keeps_other_text() {
        let mut m = Mounts::for_platform(true);
        m.add_str(r"C:\p\a");
        assert_eq!(m.host_text("/volt/c/p/ab/x"), "/volt/c/p/ab/x");
        assert_eq!(m.host_text("ok çç /usr/bin"), "ok çç /usr/bin");
    }

    #[test]
    fn host_text_prefers_the_longest_mount_and_handles_spaces() {
        let mut m = Mounts::for_platform(true);
        m.add_str(r"C:\My Proj");
        m.add_str(r"C:\My Proj\build");
        assert_eq!(
            m.host_text("/volt/c/My Proj/build/w.vcd and /volt/c/My Proj/x.sv:1"),
            r"C:\My Proj\build\w.vcd and C:\My Proj\x.sv:1"
        );
    }

    #[test]
    fn host_text_is_identity_on_unix() {
        let mut m = Mounts::for_platform(false);
        m.add_str("/home/u/p");
        assert_eq!(m.host_text("/home/u/p/x.sv:1"), "/home/u/p/x.sv:1");
    }

    #[test]
    fn strip_verbatim_keeps_unc_and_plain_paths() {
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\C:\x")),
            PathBuf::from(r"C:\x")
        );
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\UNC\s\x")),
            PathBuf::from(r"\\?\UNC\s\x")
        );
        assert_eq!(strip_verbatim(Path::new("/a/b")), PathBuf::from("/a/b"));
    }

    #[test]
    fn host_absolute_resolves_an_existing_directory() {
        let dir = std::env::temp_dir();
        let abs = host_absolute(&dir).expect("temp dizini var");
        assert!(abs.is_absolute());
        assert!(!abs.to_string_lossy().starts_with(r"\\?\"));
        assert!(container_path(&abs).is_some());
    }
}
