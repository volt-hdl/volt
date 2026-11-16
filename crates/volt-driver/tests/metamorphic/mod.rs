//! Metamorphic test and real-bug corpus (#254, ADR-0117).
//!
//! Every rewrite turns a design into another spelling of the same design
//! (the spec sentence that says so is the rewrite's `source`). Through the
//! commands the user runs, the two must give:
//!
//! - `codes`: the same diagnostic codes (a multiset; the rewrite's
//!   documented `extra_codes` aside); `safety` when a differing code
//!   guards a safety property;
//! - `location`: for errors, the same primary location, mapped back
//!   through the rewrite's edits;
//! - `hardware`: for a design without errors, the same outputs in Verilator
//!   under the same random stimulus.
//!
//! A failure that an open issue reproduces is listed in `known_failures.json`
//! with that issue; any other failure fails the run, and so does an entry
//! that no longer fails.

pub mod check;
pub mod corpus;
pub mod edit;
pub mod known;
pub mod lex;
pub mod rewrites;
pub mod sim;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use check::{Diag, Work};
use corpus::Design;
use known::Key;
use rewrites::Rewrite;

/// Codes that guard a safety property (the ADR-0113 inventory,
/// `seeded_bugs::SAFETY_CODES`).
pub const SAFETY_CODES: &[&str] = &[
    "E0016", "E3001", "E3002", "E3003", "E3004", "E3005", "E3006", "E3007", "E3008", "E3009",
    "E3010", "E3011", "E3012", "E3013", "E3014", "E3015", "E3016", "E4001", "E4003", "E4004",
    "E4006", "E4007", "E4011", "E5010", "W3003", "W3005", "W3006", "W3007", "W3008", "W3009",
    "W3010", "W3011",
];

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

pub fn temp_base(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-meta-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Verilator, found through `tools::require` (ADR-0107).
pub struct Sim {
    pub verilator: PathBuf,
}

#[derive(Default)]
pub struct Outcome {
    pub failures: Vec<(Key, String)>,
    pub evaluated: BTreeSet<Key>,
    /// rewrite id → (designs, sites, hardware compared, hardware not
    /// compared with the reason).
    pub per_rewrite: BTreeMap<String, RewriteCount>,
    pub designs: usize,
    pub originals_with_errors: usize,
}

#[derive(Default, Clone)]
pub struct RewriteCount {
    pub designs: usize,
    pub sites: usize,
    pub hw_compared: usize,
    pub hw_skipped: BTreeMap<String, usize>,
}

impl Outcome {
    fn merge(&mut self, o: Outcome) {
        self.failures.extend(o.failures);
        self.evaluated.extend(o.evaluated);
        for (k, v) in o.per_rewrite {
            let e = self.per_rewrite.entry(k).or_default();
            e.designs += v.designs;
            e.sites += v.sites;
            e.hw_compared += v.hw_compared;
            for (r, n) in v.hw_skipped {
                *e.hw_skipped.entry(r).or_default() += n;
            }
        }
        self.designs += o.designs;
        self.originals_with_errors += o.originals_with_errors;
    }
}

fn key(check: &str, design: &str, relation: &str) -> Key {
    Key {
        check: check.into(),
        design: design.into(),
        relation: relation.into(),
    }
}

fn multiset(diags: &[Diag], extra: &[(&str, &str)]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for d in diags {
        if !extra.iter().any(|(c, _)| *c == d.code) {
            *m.entry(d.code.clone()).or_default() += 1;
        }
    }
    m
}

fn show(m: &BTreeMap<String, usize>) -> String {
    if m.is_empty() {
        return "none".into();
    }
    m.iter()
        .map(|(c, n)| {
            if *n == 1 {
                c.clone()
            } else {
                format!("{c}x{n}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Error locations of the original that the variant's errors do not
/// match (mapped back through the rewrite), and the other way round.
fn location_mismatch(orig: &[Diag], var: &[Diag], rw: &edit::Rewritten) -> Option<String> {
    let mut left: Vec<&Diag> = orig.iter().filter(|d| d.error).collect();
    let mut unmatched = Vec::new();
    for v in var.iter().filter(|d| d.error) {
        let hit = left.iter().position(|o| {
            o.code == v.code
                && match (o.byte, v.byte) {
                    (Some(ob), Some(vb)) => {
                        let (s, e) = rw.to_orig(vb);
                        ob >= s && ob <= e
                    }
                    (None, None) => o.place == v.place,
                    _ => false,
                }
        });
        match hit {
            Some(i) => {
                left.remove(i);
            }
            None => unmatched.push(format!("{} at {} (rewritten)", v.code, v.place)),
        }
    }
    unmatched.extend(
        left.iter()
            .map(|o| format!("{} at {} (original)", o.code, o.place)),
    );
    (!unmatched.is_empty()).then(|| unmatched.join(", "))
}

/// Builds `work` and simulates its roots. `Err((relation_failure, text))`:
/// `false` when the hardware cannot be compared at all (not a finding of
/// the rewrite), `true` when it is a finding.
/// A simulation: the printed lines, the output names, the root ports.
type Simulated = (Vec<String>, Vec<String>, String);

fn simulate(
    sim: &Sim,
    work: &Work,
    base: &Path,
    tag: &str,
    volt_text: &str,
    seed: &str,
) -> Result<Simulated, String> {
    let build = base.join(format!("{tag}-build"));
    let svs = work.build(&build)?;
    let mut all = Vec::new();
    for p in &svs {
        all.extend(sim::modules(
            &std::fs::read_to_string(p).unwrap_or_default(),
        )?);
    }
    let roots = sim::roots(&all);
    let sig: String = roots
        .iter()
        .map(|m| {
            format!(
                "{}({})",
                m.name,
                m.ports
                    .iter()
                    .map(|p| format!("{}{}:{}", if p.input { "i" } else { "o" }, p.name, p.width))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    if roots.iter().all(|m| m.ports.iter().all(|p| p.input)) {
        return Err("no output to compare".into());
    }
    let (clocks, resets) = sim::clock_and_reset_names(volt_text);
    let bench = sim::bench(&roots, &clocks, &resets, seed);
    let mut files = svs.clone();
    files.extend(work.own_sv());
    let lines = sim::run(
        &sim.verilator,
        &base.join(format!("{tag}-sim")),
        &bench,
        &files,
    )?;
    Ok((lines, sim::output_names(&roots), sig))
}

/// The metamorphic relations on one design for every rewrite.
/// `seed` is the run's seed (`corpus::run_seed`); the stimulus of a design
/// comes from it and the design's id.
pub fn run_design(
    d: &Design,
    rewrites: &[Rewrite],
    sim: Option<&Sim>,
    base: &Path,
    seed: &str,
) -> Outcome {
    let mut out = Outcome {
        designs: 1,
        ..Default::default()
    };
    let text = d.text().to_string();
    let orig_work = Work::new(&base.join("orig"), d, &text);
    let orig = orig_work.check();
    let orig_errors = orig.iter().any(|x| x.error);
    if orig_errors {
        out.originals_with_errors = 1;
    }
    let volt_all: String = d
        .files
        .iter()
        .filter(|(p, _)| p.extension().is_some_and(|e| e == "volt"))
        .map(|(_, t)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    // The original's simulation, computed once, when a rewrite needs it.
    let mut orig_sim: Option<Result<Simulated, String>> = None;
    for r in rewrites {
        let edits = (r.sites)(&text);
        if edits.is_empty() {
            continue;
        }
        let count = out.per_rewrite.entry(r.id.to_string()).or_default();
        count.designs += 1;
        count.sites += edits.len();
        let rw = edit::apply(&text, edits);
        let var_work = Work::new(&base.join(r.id), d, &rw.text);
        let var = var_work.check();
        let (om, vm) = (multiset(&orig, &[]), multiset(&var, r.extra_codes));
        out.evaluated.insert(key(r.id, &d.id, "codes"));
        out.evaluated.insert(key(r.id, &d.id, "safety"));
        if om != vm {
            let differ: BTreeSet<&String> = om
                .keys()
                .chain(vm.keys())
                .filter(|c| om.get(*c) != vm.get(*c))
                .collect();
            let rel = if differ.iter().any(|c| SAFETY_CODES.contains(&c.as_str())) {
                "safety"
            } else {
                "codes"
            };
            out.failures.push((
                key(r.id, &d.id, rel),
                format!("original: {}; rewritten: {}", show(&om), show(&vm)),
            ));
            continue;
        }
        out.evaluated.insert(key(r.id, &d.id, "location"));
        if let Some(m) = location_mismatch(&orig, &var, &rw) {
            out.failures.push((key(r.id, &d.id, "location"), m));
        }
        if orig_errors {
            continue;
        }
        let Some(sim) = sim else {
            *count.hw_skipped.entry("no Verilator".into()).or_default() += 1;
            continue;
        };
        let seed = corpus::design_seed(seed, &d.id);
        let o = orig_sim
            .get_or_insert_with(|| simulate(sim, &orig_work, base, "orig", &volt_all, &seed));
        let (o_lines, names, o_sig) = match o {
            Ok(v) => v.clone(),
            Err(e) => {
                let reason = e
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(110)
                    .collect::<String>();
                *count
                    .hw_skipped
                    .entry(format!("original: {reason}"))
                    .or_default() += 1;
                continue;
            }
        };
        out.evaluated.insert(key(r.id, &d.id, "hardware"));
        let v = simulate(sim, &var_work, base, r.id, &volt_all, &seed);
        let count = out.per_rewrite.get_mut(r.id).unwrap();
        count.hw_compared += 1;
        let finding = match v {
            Err(e) => Some(format!(
                "the original simulates, the rewritten does not: {e}"
            )),
            Ok((_, _, v_sig)) if v_sig != o_sig => {
                Some(format!("ports differ: {o_sig} vs {v_sig}"))
            }
            Ok((v_lines, _, _)) => {
                o_lines
                    .iter()
                    .zip(&v_lines)
                    .find(|(a, b)| a != b)
                    .map(|(a, b)| {
                        format!(
                            "first difference (outputs {}): original `{a}`, rewritten `{b}`",
                            names.join(" ")
                        )
                    })
            }
        };
        if let Some(f) = finding {
            out.failures.push((key(r.id, &d.id, "hardware"), f));
        }
    }
    out
}

/// Runs `designs` on up to `workers` threads.
pub fn run_all(
    designs: &[Design],
    rewrites: &[Rewrite],
    sim: Option<&Sim>,
    base: &Path,
    seed: &str,
) -> Outcome {
    let workers = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .clamp(1, 4);
    let next = AtomicUsize::new(0);
    let total = Mutex::new(Outcome::default());
    std::thread::scope(|s| {
        for w in 0..workers {
            let (next, total) = (&next, &total);
            s.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(d) = designs.get(i) else { break };
                let dir = base.join(format!("w{w}"));
                let o = run_design(d, rewrites, sim, &dir, seed);
                let _ = std::fs::remove_dir_all(&dir);
                total.lock().unwrap().merge(o);
            });
        }
    });
    let mut o = total.into_inner().unwrap();
    o.failures.sort();
    o
}

/// ADR-0117 §6: the rewrites that found no site in the corpus of this run
/// (each tier judges its own corpus).
pub fn unused_rewrites<'a>(o: &Outcome, rewrites: &'a [Rewrite]) -> Vec<&'a str> {
    rewrites
        .iter()
        .filter(|r| o.per_rewrite.get(r.id).is_none_or(|c| c.sites == 0))
        .map(|r| r.id)
        .collect()
}

/// Writes the report to the file named by `var` when it is set.
pub fn write_summary(var: &str, text: &str) {
    if let Some(p) = std::env::var_os(var).filter(|p| !p.is_empty()) {
        std::fs::write(&p, text).unwrap_or_else(|e| panic!("{var}: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-0117 §6: the rule judges the outcome of one run, so each tier is
    /// judged on its own corpus: a rewrite with sites only in `examples/`
    /// passes the nightly tier and fails the PR tier.
    #[test]
    fn a_rewrite_without_a_site_in_this_run_is_reported() {
        let mut pr = Outcome::default();
        let mut night = Outcome::default();
        for r in rewrites::ALL {
            night.per_rewrite.insert(
                r.id.to_string(),
                RewriteCount {
                    designs: 1,
                    sites: 1,
                    ..Default::default()
                },
            );
        }
        for r in &rewrites::ALL[1..] {
            pr.per_rewrite
                .insert(r.id.to_string(), night.per_rewrite[r.id].clone());
        }
        assert!(unused_rewrites(&night, rewrites::ALL).is_empty());
        assert_eq!(unused_rewrites(&pr, rewrites::ALL), [rewrites::ALL[0].id]);
        // A rewrite that was tried on designs but found no site counts too.
        pr.per_rewrite
            .insert(rewrites::ALL[0].id.to_string(), RewriteCount::default());
        assert_eq!(unused_rewrites(&pr, rewrites::ALL), [rewrites::ALL[0].id]);
        assert_eq!(
            unused_rewrites(&Outcome::default(), rewrites::ALL).len(),
            rewrites::ALL.len()
        );
    }
}
