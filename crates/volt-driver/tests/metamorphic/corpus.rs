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

/// The environment variable that sets the run's seed (ADR-0117 §5): the
/// nightly report and its issue print it, so the same run can be repeated.
pub const SEED_ENV: &str = "VOLT_METAMORPHIC_SEED";

/// The seed of a run: `VOLT_METAMORPHIC_SEED` when set; otherwise fixed in
/// the PR tier (`pr`) and the UTC date of the run (`2026-11-16`) in the
/// nightly tier, so that every night tries other stimuli.
pub fn run_seed(tier: Tier, env: Option<&str>, today: &str) -> String {
    match (env.map(str::trim).filter(|s| !s.is_empty()), tier) {
        (Some(s), _) => s.to_string(),
        (None, Tier::Pr) => "pr".to_string(),
        (None, Tier::Nightly) => today.to_string(),
    }
}

/// The stimulus seed of one design: the run's seed and the design's id.
pub fn design_seed(run: &str, design: &str) -> String {
    format!("{run}:{design}")
}

/// Today's date in UTC, `YYYY-MM-DD`.
pub fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before 1970")
        .as_secs();
    civil_date(secs / 86_400)
}

/// The date `days` after 1970-01-01 (proleptic Gregorian; H. Hinnant's
/// `civil_from_days`).
pub fn civil_date(days: u64) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
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
                // Generated output is not part of a design.
                if !(p.ends_with("build") || p.ends_with("target")) {
                    stack.push(p);
                }
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
    // The files directly in `examples/` (with its `Volt.toml`): one unit.
    let root = repo.join("examples");
    let top: Vec<(PathBuf, String)> = sorted_dir(&root)
        .into_iter()
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|e| matches!(e.to_str(), Some("volt" | "sv" | "v" | "toml")))
        })
        .map(|p| (PathBuf::from(p.file_name().unwrap()), read(&p)))
        .collect();
    for (i, (name, _)) in top.iter().enumerate() {
        let n = name.to_string_lossy();
        if n.ends_with(".volt") && !n.ends_with("_test.volt") {
            out.push(Design {
                id: format!("examples/{n}"),
                files: top.clone(),
                entry: i,
            });
        }
    }
    for ex in sorted_dir(&root) {
        if !ex.is_dir() || ex.ends_with("build") || ex.ends_with("target") {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metamorphic::sim;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(11_016), "2000-02-29");
        assert_eq!(civil_date(19_723), "2024-01-01");
        assert_eq!(civil_date(20_773), "2026-11-16");
    }

    #[test]
    fn seed_is_fixed_in_pr_and_dated_at_night() {
        // PR tier: fixed, whatever the date.
        assert_eq!(run_seed(Tier::Pr, None, "2026-11-16"), "pr");
        assert_eq!(run_seed(Tier::Pr, None, "2026-11-17"), "pr");
        // Nightly tier: the date of the run.
        assert_eq!(run_seed(Tier::Nightly, None, "2026-11-16"), "2026-11-16");
        // VOLT_METAMORPHIC_SEED repeats a run in either tier; empty is unset.
        assert_eq!(
            run_seed(Tier::Nightly, Some("2026-11-02"), "2026-11-16"),
            "2026-11-02"
        );
        assert_eq!(run_seed(Tier::Pr, Some(" x "), "2026-11-16"), "x");
        assert_eq!(
            run_seed(Tier::Nightly, Some(""), "2026-11-16"),
            "2026-11-16"
        );
        assert_eq!(
            design_seed("2026-11-16", "tests/ui/pass/a.volt"),
            "2026-11-16:tests/ui/pass/a.volt"
        );
        assert_eq!(today_utc().len(), 10);
    }

    /// The nightly stimulus depends on the date and the design, and the
    /// seed printed in the report gives the same stimulus again.
    #[test]
    fn the_printed_seed_reproduces_the_stimulus() {
        let mods = sim::modules(
            "module M (\n    input  logic        clk,\n    input  logic [15:0] a,\n    output logic [7:0]  y\n);\nendmodule\n",
        )
        .unwrap();
        let roots = sim::roots(&mods);
        let clocks = ["clk".to_string()];
        let stim = |run: &str, design: &str| {
            sim::bench(&roots, &clocks, &[], &design_seed(run, design)).stim
        };
        let night = run_seed(Tier::Nightly, None, "2026-11-16");
        let again = run_seed(Tier::Nightly, Some(&night), "2026-12-01");
        assert_eq!(stim(&night, "d.volt"), stim(&again, "d.volt"));
        assert_ne!(stim(&night, "d.volt"), stim("2026-11-17", "d.volt"));
        assert_ne!(stim(&night, "d.volt"), stim(&night, "e.volt"));
        let pr = run_seed(Tier::Pr, None, "2026-11-16");
        assert_eq!(
            stim(&pr, "d.volt"),
            stim(&run_seed(Tier::Pr, None, "2027-01-01"), "d.volt")
        );
    }
}
