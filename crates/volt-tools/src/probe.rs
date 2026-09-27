//! Zaman sınırlı sürüm sorgusu (`volt doctor`, ADR-0084 §3).
//!
//! Takılan bir araç (ör. yanıt vermeyen Docker daemon'u, stdin bekleyen
//! bir sarmalayıcı) raporu bekletmemeli: süre dolunca süreç öldürülür ve
//! araç "yanıt vermiyor" sayılır. Çıktı ayrı iş parçacığında okunur —
//! öldürülen bir `.bat`/kabuk betiğinin torun süreci boruyu açık
//! tutabilir; okuyucu beklenmez, süreç sonunda kendiliğinden biter.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Sürüm sorgusu için varsayılan zaman sınırı.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Bekleme döngüsünün yoklama aralığı.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Sürüm sorgusunun sonucu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe {
    /// Araç çalıştı; çıktıdaki ilk sürüm numarası (yoksa `None`).
    Ran {
        version: Option<String>,
        /// Çıkış kodu 0 mı?
        success: bool,
        /// stdout + stderr (ilk satırlar raporda gerekirse).
        output: String,
    },
    /// Başlatılamadı (izin, bozuk yorumlayıcı satırı ...).
    FailedToStart(String),
    /// Zaman sınırı doldu, süreç öldürüldü.
    Unresponsive,
}

/// `program args...` çalıştırır; `timeout` içinde bitmezse öldürür.
pub fn probe_version(program: &Path, args: &[&str], timeout: Duration) -> Probe {
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return Probe::FailedToStart(e.to_string()),
    };
    let (tx, rx) = mpsc::channel();
    for pipe in [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut pipe = pipe;
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
    }
    drop(tx);

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Probe::Unresponsive;
            }
        }
    };
    // Süreç bitti; borular çoğunlukla hemen kapanır. Torun süreç açık
    // tutuyorsa kalan süre kadar beklenir, sonra eldekiyle devam edilir.
    let mut output = String::new();
    for _ in 0..2 {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left.max(POLL_INTERVAL)) {
            Ok(buf) => output.push_str(&String::from_utf8_lossy(&buf)),
            Err(_) => break,
        }
    }
    Probe::Ran {
        version: parse_version(&output),
        success: status.success(),
        output,
    }
}

/// Metindeki ilk `sayı.sayı[.sayı...]` dizisi (ör. "Verilator 5.050
/// 2026-..." → "5.050", "Yosys 0.47+61" → "0.47").
pub(crate) fn parse_version(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let preceded_by_word = i > 0 && (bytes[i - 1].is_ascii_alphanumeric());
        if bytes[i].is_ascii_digit() && !preceded_by_word {
            let start = i;
            let mut dots = 0;
            while i < bytes.len()
                && (bytes[i].is_ascii_digit()
                    || (bytes[i] == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)))
            {
                if bytes[i] == b'.' {
                    dots += 1;
                }
                i += 1;
            }
            if dots > 0 {
                return Some(text[start..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_version_lines() {
        let cases = [
            ("Verilator 5.050 2026-08-01 rev v5.050", "5.050"),
            ("Yosys 0.47+61 (git sha1 81011ad92)", "0.47"),
            ("Z3 version 4.13.4 - 64 bit", "4.13.4"),
            ("3.2.3", "3.2.3"),
            ("Yices 2.6.5\nCopyright SRI International.", "2.6.5"),
            ("GNU Make 4.3", "4.3"),
            ("Docker version 27.3.1, build ce12230", "27.3.1"),
            ("rustc 1.90.0 (1159e78c4 2025-09-14)", "1.90.0"),
            ("gcc (Ubuntu 13.2.0-23ubuntu4) 13.2.0", "13.2.0"),
        ];
        for (text, want) in cases {
            assert_eq!(parse_version(text).as_deref(), Some(want), "{text}");
        }
    }

    #[test]
    fn no_version_without_a_dot_or_inside_a_word() {
        assert_eq!(parse_version("usage: sby [options]"), None);
        assert_eq!(parse_version("x86_64 build 42"), None);
        assert_eq!(parse_version("abc1.2"), None);
    }

    #[test]
    fn missing_program_fails_to_start() {
        let probe = probe_version(
            Path::new("volt-tools-no-such-program-xyz"),
            &["--version"],
            DEFAULT_TIMEOUT,
        );
        assert!(matches!(probe, Probe::FailedToStart(_)), "{probe:?}");
    }
}
