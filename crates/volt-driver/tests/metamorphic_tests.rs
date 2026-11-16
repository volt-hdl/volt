//! Metamorphic test and real-bug corpus (#254, ADR-0117).
//!
//! - `real_bugs`: every reproduction in `tests/real_bugs/` gives the codes
//!   its `//~` lines name, and its `test` blocks pass in Verilator.
//! - `metamorphic`: every rewrite of the catalogue on every design of the
//!   tier's corpus (`VOLT_METAMORPHIC_TIER`: `pr`, the default, is
//!   `tests/real_bugs/` and `tests/ui/`; `nightly` adds `examples/` and the
//!   book) keeps the codes, the error locations, the safety verdicts and,
//!   for a design without errors, the hardware (Verilator, random
//!   stimulus).
//! - `rewrite_examples_keep_their_meaning`: the relations on each
//!   rewrite's own examples, with no known failure allowed.
//!
//! Failures that an open issue reproduces are listed in
//! `metamorphic/known_failures.json`. Verilator is looked up through
//! `tools::require` (ADR-0107): the tool job requires it, so the hardware
//! relation and the `test` blocks never skip there.

mod metamorphic;
mod tools;

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::time::Instant;

use metamorphic::check::Work;
use metamorphic::corpus::{self, Tier};
use metamorphic::known::{self, Key};
use metamorphic::rewrites;
use metamorphic::Sim;

fn sim() -> Option<Sim> {
    tools::require(tools::Tool::Verilator).map(|verilator| Sim { verilator })
}

/// Fails with the report when there are unlisted failures or stale
/// entries.
fn verdict(report: &mut String, unlisted: &[String], stale: &[String]) {
    if !unlisted.is_empty() {
        report.push_str(&format!(
            "\nFailures not in the known-failures list ({}):\n",
            unlisted.len()
        ));
        for u in unlisted {
            report.push_str(&format!("  {u}\n"));
        }
    }
    if !stale.is_empty() {
        report.push_str(&format!(
            "\nStale known-failure entries ({}):\n",
            stale.len()
        ));
        for s in stale {
            report.push_str(&format!("  {s}\n"));
        }
    }
}

// ═══ Real-bug corpus ════════════════════════════════════════════════════

/// The expected outcome of a reproduction: `//~ CODE` lines (a set) or
/// `//~ PASS` (none), at the top of the file.
fn expected(text: &str) -> Result<BTreeSet<String>, String> {
    let mut codes = BTreeSet::new();
    let mut pass = false;
    for l in text.lines() {
        let Some(v) = l.strip_prefix("//~ ") else {
            break;
        };
        let v = v.trim();
        if v == "PASS" {
            pass = true;
        } else if v.len() == 5
            && (v.starts_with('E') || v.starts_with('W'))
            && v[1..].bytes().all(|b| b.is_ascii_digit())
        {
            codes.insert(v.to_string());
        } else {
            return Err(format!("`//~ {v}` is neither a code nor PASS"));
        }
    }
    match (pass, codes.is_empty()) {
        (true, true) | (false, false) => Ok(codes),
        (true, false) => Err("`//~ PASS` with codes".into()),
        (false, true) => Err("no `//~` line at the top".into()),
    }
}

/// The issue number in the corpus id: `tests/real_bugs/216_x.volt`.
fn issue_of(id: &str) -> Option<u64> {
    let name = id.strip_prefix("tests/real_bugs/")?;
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    if name[digits.len()..].starts_with('_') {
        digits.parse().ok()
    } else {
        None
    }
}

#[test]
fn real_bugs() {
    let started = Instant::now();
    let repo = metamorphic::repo();
    let entries = known::load(&repo);
    let designs = corpus::real_bugs(&repo);
    assert!(
        designs.len() >= 20,
        "tests/real_bugs/ has {} reproductions",
        designs.len()
    );
    let sim = sim();
    let base = metamorphic::temp_base("realbugs");
    let mut failures: Vec<(Key, String)> = Vec::new();
    let mut evaluated = BTreeSet::new();
    let mut seen_issues = BTreeSet::new();
    let mut report = String::from("Real-bug corpus (tests/real_bugs/, ADR-0117)\n");
    let (mut with_tests, mut tests_run) = (0, 0);
    for d in &designs {
        // The form is not a known failure: a malformed file fails at once.
        let issue = issue_of(&d.id)
            .unwrap_or_else(|| panic!("{}: the name must start with `<issue>_`", d.id));
        assert!(seen_issues.insert(issue), "#{issue} has two reproductions");
        let want = expected(d.text()).unwrap_or_else(|e| panic!("{}: {e}", d.id));
        assert!(
            d.text().contains(&format!("// #{issue}")),
            "{}: the comment after the `//~` lines names the issue: `// #{issue}: ...`",
            d.id
        );
        let dir = base.join(format!("i{issue}"));
        let work = Work::new(&dir.join("src"), d, d.text());
        let got: BTreeSet<String> = work.check().into_iter().map(|x| x.code).collect();
        evaluated.insert(Key {
            check: "real-bug".into(),
            design: d.id.clone(),
            relation: "expect".into(),
        });
        let show = |s: &BTreeSet<String>| {
            if s.is_empty() {
                "PASS".to_string()
            } else {
                s.iter().cloned().collect::<Vec<_>>().join(" ")
            }
        };
        report.push_str(&format!(
            "  #{issue:<4} {:<58} expect {:<12} got {}\n",
            d.id,
            show(&want),
            show(&got)
        ));
        if got != want {
            failures.push((
                Key {
                    check: "real-bug".into(),
                    design: d.id.clone(),
                    relation: "expect".into(),
                },
                format!("expected {}, volt check gave {}", show(&want), show(&got)),
            ));
        }
        let tests = d.text().matches("\ntest \"").count();
        if tests == 0 {
            continue;
        }
        with_tests += 1;
        let Some(_) = &sim else { continue };
        tests_run += 1;
        evaluated.insert(Key {
            check: "real-bug".into(),
            design: d.id.clone(),
            relation: "test".into(),
        });
        let entry_dir = dir
            .join("src")
            .join(d.entry_path())
            .parent()
            .unwrap()
            .to_path_buf();
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .current_dir(&entry_dir)
            .env("VOLT_TOOL_BACKEND", "local")
            .env("VOLT_LANG", "en")
            .env("NO_COLOR", "1")
            .args(["test"])
            .arg(d.entry_path().file_name().unwrap())
            .args(["--target-dir"])
            .arg(dir.join("build"))
            .output()
            .expect("volt test");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let ok = out.status.success() && text.contains(&format!("{tests} passed; 0 failed"));
        report.push_str(&format!(
            "         volt test: {}\n",
            text.lines()
                .find(|l| l.contains("passed;"))
                .unwrap_or("(no summary line)")
                .trim()
        ));
        if !ok {
            let why: Vec<&str> = text
                .lines()
                .filter(|l| l.contains("FAIL") || l.contains("assert") || l.contains("error"))
                .take(4)
                .collect();
            failures.push((
                Key {
                    check: "real-bug".into(),
                    design: d.id.clone(),
                    relation: "test".into(),
                },
                format!(
                    "volt test failed (exit {:?}): {}",
                    out.status.code(),
                    why.join(" | ")
                ),
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    failures.sort();
    // The real-bug relations (`expect`, `test`) do not depend on a stimulus.
    let (unlisted, stale, _) = known::compare(&entries, &failures, &evaluated, &["real-bug"], true);
    report.push_str(&format!(
        "\n{} reproductions, {} with test blocks ({} run in Verilator{}); {} failures, {} known; {:.1} s\n",
        designs.len(),
        with_tests,
        tests_run,
        if sim.is_some() { "" } else { ": SKIP, no Verilator" },
        failures.len(),
        failures.len() - unlisted.len(),
        started.elapsed().as_secs_f64()
    ));
    for (k, why) in &failures {
        if let Some(e) = entries.iter().find(|e| &e.key == k) {
            report.push_str(&format!(
                "  known (#{}, {}): {k}: {why}\n",
                e.issue, e.reason
            ));
        }
    }
    verdict(&mut report, &unlisted, &stale);
    eprintln!("{report}");
    metamorphic::write_summary("VOLT_REAL_BUGS_SUMMARY", &report);
    assert!(unlisted.is_empty() && stale.is_empty(), "{report}");
}

// ═══ Metamorphic relations ══════════════════════════════════════════════

fn render(tier: &str, o: &metamorphic::Outcome, secs: f64, sim: bool) -> String {
    let mut s = format!(
        "Metamorphic test (ADR-0117), tier {tier}: {} designs ({} with errors in the original), Verilator: {}\n",
        o.designs,
        o.originals_with_errors,
        if sim { "yes" } else { "SKIP" }
    );
    s.push_str("  rewrite              designs  sites  hardware compared  not compared\n");
    for r in rewrites::ALL {
        let c = o.per_rewrite.get(r.id).cloned().unwrap_or_default();
        let skipped: usize = c.hw_skipped.values().sum();
        s.push_str(&format!(
            "  {:<20} {:>7} {:>6} {:>18} {:>13}\n",
            r.id, c.designs, c.sites, c.hw_compared, skipped
        ));
        s.push_str(&format!("      ({}, #{})\n", r.construct, r.issue));
        for (why, n) in &c.hw_skipped {
            s.push_str(&format!("      not compared x{n}: {why}\n"));
        }
    }
    s.push_str(&format!(
        "  hardware: {} comparisons, {} Verilator builds and runs, {:.1} s in them (summed over the workers)\n",
        compared(o),
        o.sim_runs,
        o.sim_secs
    ));
    s.push_str(&format!(
        "  failures: {}; {:.1} s\n",
        o.failures.len(),
        secs
    ));
    s
}

/// Hardware comparisons of all rewrites.
fn compared(o: &metamorphic::Outcome) -> usize {
    o.per_rewrite.values().map(|c| c.hw_compared).sum()
}

#[test]
fn metamorphic() {
    let started = Instant::now();
    let tier = Tier::from_env();
    let repo = metamorphic::repo();
    let entries = known::load(&repo);
    let designs = corpus::load(&repo, tier);
    let sim = sim();
    let seed = corpus::run_seed(
        tier,
        std::env::var(corpus::SEED_ENV).ok().as_deref(),
        &corpus::today_utc(),
    );
    let base = metamorphic::temp_base("relations");
    let o = metamorphic::run_all(&designs, rewrites::ALL, sim.as_ref(), &base, &seed);
    let _ = std::fs::remove_dir_all(&base);
    let checks: Vec<&str> = rewrites::ALL.iter().map(|r| r.id).collect();
    let (unlisted, stale, unjudged) = known::compare(
        &entries,
        &o.failures,
        &o.evaluated,
        &checks,
        corpus::stimulus_is_fixed(&seed),
    );
    let mut report = render(
        tier.name(),
        &o,
        started.elapsed().as_secs_f64(),
        sim.is_some(),
    );
    report.push_str(&format!(
        "  seed: {seed} (reproduce: {}={} {}={seed} cargo test -p volt-driver --test metamorphic_tests -- metamorphic --exact)\n",
        "VOLT_METAMORPHIC_TIER",
        tier.name(),
        corpus::SEED_ENV
    ));
    for (k, why) in &o.failures {
        if let Some(e) = entries.iter().find(|e| &e.key == k) {
            report.push_str(&format!(
                "  known (#{}, {}): {k}: {why}\n",
                e.issue, e.reason
            ));
        }
    }
    // ADR-0117 §4: with a dated seed a listed hardware failure that did not
    // show is a note, not a red run.
    if !unjudged.is_empty() {
        report.push_str(&format!(
            "
Known hardware failures that did not show with seed {seed} ({}; not judged: only the fixed seed `{}` judges a hardware entry):
",
            unjudged.len(),
            corpus::FIXED_SEED
        ));
        for n in &unjudged {
            report.push_str(&format!(
                "  {n}
"
            ));
        }
    }
    // ADR-0117 §6: a rewrite that finds no site in this tier's corpus tests
    // nothing.
    let unused = metamorphic::unused_rewrites(&o, rewrites::ALL);
    if !unused.is_empty() {
        report.push_str(&format!(
            "\nRewrites with no site in the corpus: {}\n",
            unused.join(", ")
        ));
    }
    verdict(&mut report, &unlisted, &stale);
    eprintln!("{report}");
    metamorphic::write_summary("VOLT_METAMORPHIC_SUMMARY", &report);
    assert!(
        unlisted.is_empty() && stale.is_empty() && unused.is_empty(),
        "{report}"
    );
    // Every comparison simulated the rewritten design (and its original
    // once): a comparison without a Verilator run compared nothing.
    assert!(
        o.sim_runs >= compared(&o) && (compared(&o) == 0 || sim.is_some()),
        "{report}"
    );
}

#[test]
fn rewrite_examples_keep_their_meaning() {
    let sim = sim();
    let base = metamorphic::temp_base("examples");
    let mut failures = Vec::new();
    let mut compared = BTreeMap::new();
    for r in rewrites::ALL {
        for (n, ex) in r.examples.iter().enumerate() {
            let d = corpus::Design {
                id: format!("example:{}:{n}", r.id),
                files: vec![("example.volt".into(), ex.to_string())],
                entry: 0,
            };
            let o = metamorphic::run_design(
                &d,
                std::slice::from_ref(r),
                sim.as_ref(),
                &base.join(r.id),
                "example",
            );
            failures.extend(o.failures);
            let c = o.per_rewrite.get(r.id).cloned().unwrap_or_default();
            assert!(c.sites > 0, "{}: example {n} has no site", r.id);
            assert!(
                c.hw_skipped.keys().all(|k| k == "no Verilator"),
                "{}: example {n} cannot be simulated: {:?}",
                r.id,
                c.hw_skipped
            );
            *compared.entry(r.id).or_insert(0) += c.hw_compared;
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    assert!(failures.is_empty(), "{failures:#?}");
    if sim.is_some() {
        assert!(compared.values().all(|n| *n > 0), "{compared:?}");
    }
}
