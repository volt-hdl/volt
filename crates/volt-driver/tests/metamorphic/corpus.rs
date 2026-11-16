//! The designs of the metamorphic test and of the real-bug test (#254,
//! ADR-0117 §5). PR tier: `tests/real_bugs/` and `tests/ui/`; nightly tier:
//! also `examples/` and the ```volt blocks of the book.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Pr,
    Nightly,
}

impl Tier {
    /// `VOLT_METAMORPHIC_TIER`: `pr` (default) or `nightly`.
    pub fn from_env() -> Tier {
        match std::env::var("VOLT_METAMORPHIC_TIER").as_deref() {
            Err(_) | Ok("") | Ok("pr") => Tier::Pr,
            Ok("nightly") => Tier::Nightly,
            Ok(other) => panic!("VOLT_METAMORPHIC_TIER: unknown tier `{other}` (pr, nightly)"),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Tier::Pr => "pr",
            Tier::Nightly => "nightly",
        }
    }
}

/// One design: its files, copied alone into a working directory, and the
/// entry file that `volt check`/`volt build` read and the rewrites change.
#[derive(Debug, Clone)]
pub struct Design {
    /// `tests/ui/pass/38_match_sequential.volt`,
    /// `book/src/tutorial/x.md#a.volt@2`.
    pub id: String,
    /// (path inside the working directory, text); LF line ends.
    pub files: Vec<(PathBuf, String)>,
    /// Index into `files`.
    pub entry: usize,
}

impl Design {
    pub fn text(&self) -> &str {
        &self.files[self.entry].1
    }
    pub fn entry_path(&self) -> &Path {
        &self.files[self.entry].0
    }
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .replace("\r\n", "\n")
}

fn sorted_dir(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    v.sort();
    v
}

fn rel_id(repo: &Path, p: &Path) -> String {
    p.strip_prefix(repo)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// A directory unit: every `.volt` file of `dir` (and its subdirectories),
/// the entry `entry_name`.
fn unit(repo: &Path, dir: &Path, entry_name: &str) -> Option<Design> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for p in sorted_dir(&d) {
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| {
                matches!(
                    e.to_str(),
                    Some("volt" | "sv" | "v" | "toml" | "hex" | "mem")
                )
            }) {
                files.push((p.strip_prefix(dir).unwrap().to_path_buf(), read(&p)));
            }
        }
    }
    let entry = files.iter().position(|(p, _)| p == Path::new(entry_name))?;
    Some(Design {
        id: rel_id(repo, &dir.join(entry_name)),
        files,
        entry,
    })
}

/// `tests/real_bugs/`: files and directory units (entry `main.volt`).
pub fn real_bugs(repo: &Path) -> Vec<Design> {
    let dir = repo.join("tests/real_bugs");
    let mut out = Vec::new();
    for p in sorted_dir(&dir) {
        if p.is_dir() {
            out.push(
                unit(repo, &p, "main.volt")
                    .unwrap_or_else(|| panic!("{}: a directory unit needs main.volt", p.display())),
            );
        } else if p.extension().is_some_and(|e| e == "volt") {
            out.push(single(repo, &p));
        }
    }
    out
}

fn single(repo: &Path, p: &Path) -> Design {
    Design {
        id: rel_id(repo, p),
        files: vec![(PathBuf::from(p.file_name().unwrap()), read(p))],
        entry: 0,
    }
}

/// Flat directories of standalone designs (`tests/ui/pass`, `tests/ui/fail`).
fn flat(repo: &Path, rel: &str, out: &mut Vec<Design>) {
    for p in sorted_dir(&repo.join(rel)) {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_file() && name.ends_with(".volt") && !name.ends_with("_test.volt") {
            out.push(single(repo, &p));
        }
    }
}

/// `tests/ui/multifile/<unit>/`: entry `main.volt` or `a.volt`.
fn multifile(repo: &Path, out: &mut Vec<Design>) {
    for p in sorted_dir(&repo.join("tests/ui/multifile")) {
        if p.is_dir() {
            if let Some(d) = unit(repo, &p, "main.volt").or_else(|| unit(repo, &p, "a.volt")) {
                out.push(d);
            }
        }
    }
}

/// `examples/<name>/`: every `.volt` file that is not a test file is a
/// design; its unit is the whole example directory.
fn examples(repo: &Path, out: &mut Vec<Design>) {
    for ex in sorted_dir(&repo.join("examples")) {
        if !ex.is_dir() {
            continue;
        }
        let mut entries = Vec::new();
        let mut stack = vec![ex.clone()];
        while let Some(d) = stack.pop() {
            for p in sorted_dir(&d) {
                let n = p.file_name().unwrap().to_string_lossy().to_string();
                if p.is_dir() && n != "build" && n != "target" {
                    stack.push(p);
                } else if n.ends_with(".volt") && !n.ends_with("_test.volt") {
                    entries.push(
                        p.strip_prefix(&ex)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        entries.sort();
        for e in entries {
            if let Some(d) = unit(repo, &ex, &e) {
                out.push(d);
            }
        }
    }
}

/// The ```volt blocks of the book that are designs: not `should_fail`,
/// not `output`, not a copy of a repository file (`from=`), not a test
/// file. Each block alone, next to the page's SystemVerilog `file=` blocks.
fn book(repo: &Path, out: &mut Vec<Design>) {
    let mut pages = Vec::new();
    let mut stack = vec![repo.join("book/src")];
    while let Some(d) = stack.pop() {
        for p in sorted_dir(&d) {
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "md") {
                pages.push(p);
            }
        }
    }
    pages.sort();
    let attr = |info: &str, key: &str| {
        info.split(',')
            .map(str::trim)
            .find_map(|kv| kv.strip_prefix(&format!("{key}=")).map(str::to_string))
    };
    for page in pages {
        let text = read(&page);
        let page_id = rel_id(repo, &page);
        let mut blocks: Vec<(String, String)> = Vec::new();
        let mut lines = text.lines();
        while let Some(l) = lines.next() {
            if let Some(info) = l.strip_prefix("```") {
                let mut body = String::new();
                for b in lines.by_ref() {
                    if b.starts_with("```") {
                        break;
                    }
                    body.push_str(b);
                    body.push('\n');
                }
                blocks.push((info.trim().to_string(), body));
            }
        }
        let sv: Vec<(PathBuf, String)> = blocks
            .iter()
            .filter(|(i, _)| i.starts_with("systemverilog"))
            .filter_map(|(i, b)| attr(i, "file").map(|f| (PathBuf::from(f), b.clone())))
            .collect();
        let mut n = 0;
        for (info, body) in &blocks {
            if !(info == "volt" || info.starts_with("volt,")) {
                continue;
            }
            n += 1;
            let flags: Vec<&str> = info.split(',').map(str::trim).collect();
            let file = attr(info, "file").unwrap_or_else(|| "main.volt".to_string());
            if file.ends_with("_test.volt")
                || flags.contains(&"output")
                || attr(info, "should_fail").is_some()
                || attr(info, "from").is_some()
                || body.contains("\ntest \"")
                || body.starts_with("test \"")
            {
                continue;
            }
            let mut files = sv.clone();
            files.push((PathBuf::from(&file), body.clone()));
            out.push(Design {
                id: format!("{page_id}#{file}@{n}"),
                entry: files.len() - 1,
                files,
            });
        }
    }
}

pub fn load(repo: &Path, tier: Tier) -> Vec<Design> {
    let mut out = real_bugs(repo);
    flat(repo, "tests/ui/pass", &mut out);
    flat(repo, "tests/ui/fail", &mut out);
    multifile(repo, &mut out);
    if tier == Tier::Nightly {
        examples(repo, &mut out);
        book(repo, &mut out);
    }
    out
}
