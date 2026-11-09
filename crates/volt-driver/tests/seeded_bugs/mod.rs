//! Seeded-bug corpus (ADR-0113): clean designs, single-point hazards and
//! near misses, and a runner that checks them with the `volt` binary in
//! temporary copies of the corpus.

pub mod kinds;
pub mod scan;
pub mod specials;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Codes that guard a safety property (the ADR-0113 inventory). A near
/// miss that adds one of them, or any error, is a false alarm.
pub const SAFETY_CODES: &[&str] = &[
    "E0016", "E3001", "E3002", "E3003", "E3004", "E3005", "E3006", "E3007", "E3008", "E3009",
    "E3010", "E3011", "E3012", "E3013", "E3014", "E3015", "E3016", "E4001", "E4003", "E4004",
    "E4006", "E4007", "E4011", "E5010", "W3003", "W3005", "W3006", "W3007", "W3008", "W3009",
    "W3010", "W3011",
];

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One design of the corpus: a file inside a copied directory tree.
#[derive(Debug, Clone)]
pub struct Design {
    /// Repository path (`examples/vga/vga_top.volt`) or book block
    /// (`book/src/tutorial/two-clocks.md#sample_bridge.volt@3`).
    pub id: String,
    /// Path of the file inside a workspace copy.
    pub rel: PathBuf,
    pub text: String,
}

/// A design left out of the corpus, with the reason.
pub struct Excluded {
    pub id: String,
    pub reason: String,
}

fn keep_file(p: &Path) -> bool {
    let name = p.file_name().unwrap().to_string_lossy();
    if name.ends_with("_test.volt") {
        // Test files are not designs; `volt check X.volt` would load a
        // sibling test file and report test errors for a mutated port.
        return false;
    }
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("volt" | "sv" | "svh" | "v" | "toml" | "hex" | "mem")
    )
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            if n != "build" && n != "target" {
                walk(&p, out);
            }
        } else if keep_file(&p) {
            out.push(p);
        }
    }
}

/// Repository trees copied into every workspace.
pub const TREES: &[&str] = &["examples", "demo", "tests/ui/pass", "tests/fixtures"];

/// Files to copy: (path inside the workspace, contents).
pub struct Workspace {
    pub files: Vec<(PathBuf, String)>,
}

pub struct Corpus {
    pub designs: Vec<Design>,
    pub excluded: Vec<Excluded>,
    pub workspace: Workspace,
}

pub fn load_corpus() -> Corpus {
    let root = repo();
    let mut files = Vec::new();
    let mut designs = Vec::new();
    let mut excluded = Vec::new();
    for tree in TREES {
        let mut paths = Vec::new();
        walk(&root.join(tree), &mut paths);
        for p in paths {
            let rel = p.strip_prefix(&root).unwrap().to_path_buf();
            let text = std::fs::read_to_string(&p)
                .expect("read corpus file")
                .replace("\r\n", "\n");
            if rel.extension().is_some_and(|e| e == "volt") {
                designs.push(Design {
                    id: rel.to_string_lossy().replace('\\', "/"),
                    rel: rel.clone(),
                    text: text.clone(),
                });
            }
            files.push((rel, text));
        }
    }
    load_book(&root, &mut designs, &mut excluded, &mut files);
    Corpus {
        designs,
        excluded,
        workspace: Workspace { files },
    }
}

/// ```volt blocks of the book: each one alone in its own directory,
/// next to the page's SystemVerilog `file=` blocks.
fn load_book(
    root: &Path,
    designs: &mut Vec<Design>,
    excluded: &mut Vec<Excluded>,
    files: &mut Vec<(PathBuf, String)>,
) {
    let mut pages = Vec::new();
    walk_md(&root.join("book/src"), &mut pages);
    for page in pages {
        let text = std::fs::read_to_string(&page)
            .unwrap()
            .replace("\r\n", "\n");
        let page_id = page
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let mut blocks: Vec<(String, String)> = Vec::new(); // (info, body)
        let mut lines = text.lines();
        while let Some(l) = lines.next() {
            if let Some(info) = l.strip_prefix("```") {
                let info = info.trim().to_string();
                let mut body = String::new();
                for b in lines.by_ref() {
                    if b.starts_with("```") {
                        break;
                    }
                    body.push_str(b);
                    body.push('\n');
                }
                blocks.push((info, body));
            }
        }
        let sv: Vec<(String, String)> = blocks
            .iter()
            .filter(|(info, _)| info.starts_with("systemverilog"))
            .filter_map(|(info, body)| attr(info, "file").map(|f| (f, body.clone())))
            .collect();
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let volt: Vec<&(String, String)> = blocks
            .iter()
            .filter(|(info, _)| info == "volt" || info.starts_with("volt,"))
            .collect();
        for (n, (info, body)) in volt.into_iter().enumerate() {
            let file = attr(info, "file").unwrap_or_else(|| format!("block{}.volt", n + 1));
            let k = {
                let e = seen.entry(file.clone()).or_insert(0);
                *e += 1;
                *e
            };
            let total = blocks
                .iter()
                .filter(|(i, _)| {
                    (i == "volt" || i.starts_with("volt,"))
                        && attr(i, "file").as_deref() == Some(file.as_str())
                })
                .count();
            let id = if total > 1 {
                format!("{page_id}#{file}@{k}")
            } else {
                format!("{page_id}#{file}")
            };
            let flags: Vec<&str> = info.split(',').map(str::trim).collect();
            let reason = if file.ends_with("_test.volt") {
                Some("a test file, not a design".to_string())
            } else if flags.contains(&"output") {
                Some("shown output, not a design".to_string())
            } else if let Some(code) = attr(info, "should_fail") {
                Some(format!("written to fail (should_fail={code})"))
            } else {
                attr(info, "from").map(|from| format!("copy of {from}, which is in the corpus"))
            };
            if let Some(reason) = reason {
                excluded.push(Excluded { id, reason });
                continue;
            }
            let dir = PathBuf::from("book").join(id.replace(['/', '#', '@', '.'], "_"));
            for (f, body) in &sv {
                files.push((dir.join(f), body.clone()));
            }
            let rel = dir.join(&file);
            files.push((rel.clone(), body.clone()));
            designs.push(Design {
                id,
                rel,
                text: body.clone(),
            });
        }
    }
}

fn walk_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk_md(&p, out);
        } else if p.extension().is_some_and(|e| e == "md") {
            out.push(p);
        }
    }
}

fn attr(info: &str, key: &str) -> Option<String> {
    info.split(',')
        .map(str::trim)
        .find_map(|kv| kv.strip_prefix(&format!("{key}=")).map(str::to_string))
}

/// A workspace copy owned by one worker thread.
pub struct Worker {
    pub dir: PathBuf,
}

impl Worker {
    pub fn new(base: &Path, n: usize, ws: &Workspace) -> Self {
        let dir = base.join(format!("w{n}"));
        for (rel, text) in &ws.files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).expect("create workspace dir");
            std::fs::write(&p, text).expect("write workspace file");
        }
        Worker { dir }
    }

    /// `volt check` of `design` with its text replaced by `text`; returns
    /// the codes of the diagnostics. The original text is restored.
    pub fn check(&self, design: &Design, text: &str) -> Vec<String> {
        let path = self.dir.join(&design.rel);
        std::fs::write(&path, text).expect("write mutant");
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .args(["--lang", "en", "check", "--format", "json"])
            .arg(path.file_name().unwrap())
            .current_dir(path.parent().unwrap())
            .env_remove("VOLT_MANIFEST_DIR")
            .output()
            .expect("run volt");
        std::fs::write(&path, &design.text).expect("restore design");
        match serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            Ok(json) => json["diagnostics"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|d| d["code"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            Err(_) => vec![format!(
                "CRASH(exit {:?}): {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .last()
                    .unwrap_or("")
            )],
        }
    }
}

/// Codes whose count grew from `base` to `now`.
pub fn new_codes(base: &[String], now: &[String]) -> Vec<String> {
    let count = |v: &[String]| {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for c in v {
            *m.entry(c.clone()).or_default() += 1;
        }
        m
    };
    let (b, n) = (count(base), count(now));
    n.into_iter()
        .filter(|(c, k)| *k > b.get(c).copied().unwrap_or(0))
        .map(|(c, _)| c)
        .collect()
}

pub fn is_error(code: &str) -> bool {
    code.starts_with('E') || code.starts_with("CRASH")
}

pub fn is_safety(code: &str) -> bool {
    SAFETY_CODES.contains(&code)
}
