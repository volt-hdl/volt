//! `volt check` and `volt build` of a design, through the binary the user
//! runs (#258: a check the user sees is tested through the user's command).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::metamorphic::corpus::Design;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub code: String,
    pub error: bool,
    /// The primary span: in the entry file (`Some(byte)`), or elsewhere
    /// (`None`, with the file and line in `place`).
    pub byte: Option<usize>,
    pub place: String,
}

/// A working copy of one design.
pub struct Work {
    pub dir: PathBuf,
    entry: PathBuf,
    text: String,
}

impl Work {
    /// Writes the design into `dir` (created empty), the entry file's text
    /// replaced by `entry_text`.
    pub fn new(dir: &Path, d: &Design, entry_text: &str) -> Work {
        let _ = std::fs::remove_dir_all(dir);
        for (i, (rel, text)) in d.files.iter().enumerate() {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).expect("work dir");
            std::fs::write(&p, if i == d.entry { entry_text } else { text }).expect("write design");
        }
        Work {
            dir: dir.to_path_buf(),
            entry: dir.join(d.entry_path()),
            text: entry_text.to_string(),
        }
    }

    fn volt(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_volt"));
        c.current_dir(self.entry.parent().unwrap())
            .env_remove("VOLT_MANIFEST_DIR")
            .env("VOLT_LANG", "en")
            .env("NO_COLOR", "1")
            .env("VOLT_TOOL_BACKEND", "local");
        c
    }

    fn entry_name(&self) -> String {
        self.entry
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
    }

    /// The diagnostics of `volt check --format json`; a run without JSON
    /// (a crash) is one diagnostic `CRASH`.
    pub fn check(&self) -> Vec<Diag> {
        let out = self
            .volt()
            .args(["--lang", "en", "check", "--format", "json"])
            .arg(self.entry_name())
            .output()
            .expect("run volt check");
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
            return vec![Diag {
                code: "CRASH".into(),
                error: true,
                byte: None,
                place: format!(
                    "exit {:?}: {}",
                    out.status.code(),
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .last()
                        .unwrap_or("")
                ),
            }];
        };
        let entry = self.entry_name();
        json["diagnostics"]
            .as_array()
            .map(|a| a.iter().map(|d| diag(d, &entry, &self.text)).collect())
            .unwrap_or_default()
    }

    /// `volt build` into `<dir>/../<tag>-build`; the generated SV files
    /// (`build/rtl/*.sv`), or the error text.
    pub fn build(&self, target: &Path) -> Result<Vec<PathBuf>, String> {
        let _ = std::fs::remove_dir_all(target);
        let out = self
            .volt()
            .args(["--lang", "en", "build"])
            .arg(self.entry_name())
            .arg("--target-dir")
            .arg(target)
            .output()
            .expect("run volt build");
        if !out.status.success() {
            return Err(format!(
                "volt build failed (exit {:?}):\n{}{}",
                out.status.code(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let rtl = target.join("rtl");
        let mut svs: Vec<PathBuf> = std::fs::read_dir(&rtl)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        svs.retain(|p| p.extension().is_some_and(|e| e == "sv" || e == "v"));
        svs.sort();
        Ok(svs)
    }

    /// SystemVerilog files of the design itself (extern `@source` bodies).
    pub fn own_sv(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![self.dir.clone()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else {
                continue;
            };
            for p in rd.filter_map(|e| e.ok().map(|e| e.path())) {
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "sv" || e == "v") {
                    out.push(p);
                }
            }
        }
        out.sort();
        out
    }
}

/// The byte offset of 1-based `line` and `col` (characters) in `text`.
pub fn offset(text: &str, line: usize, col: usize) -> Option<usize> {
    let start = if line <= 1 {
        0
    } else {
        text.match_indices('\n').nth(line - 2)?.0 + 1
    };
    let rest = &text[start..];
    let in_line = rest
        .char_indices()
        .nth(col.saturating_sub(1))
        .map_or(rest.len(), |(b, _)| b);
    Some(start + in_line)
}

fn diag(d: &serde_json::Value, entry: &str, text: &str) -> Diag {
    let code = d["code"].as_str().unwrap_or("?").to_string();
    let error = d["severity"].as_str() == Some("error");
    let primary = d["spans"].as_array().and_then(|s| {
        s.iter()
            .find(|s| s["primary"].as_bool() == Some(true))
            .or(s.first())
    });
    let (mut byte, mut place) = (None, "no span".to_string());
    if let Some(s) = primary {
        let file = s["file"].as_str().unwrap_or("").replace('\\', "/");
        let line = s["start"]["line"].as_u64().unwrap_or(0);
        let col = s["start"]["col"].as_u64().unwrap_or(0);
        place = format!("{file}:{line}:{col}");
        if file == entry || file.ends_with(&format!("/{entry}")) {
            byte = offset(text, line as usize, col as usize);
        }
    }
    Diag {
        code,
        error,
        byte,
        place,
    }
}
