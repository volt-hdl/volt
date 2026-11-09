//! Seeded-bug test (ADR-0113): do the safety checks miss unsafe designs?
//!
//! The corpus is every clean design of `examples/`, `demo/`,
//! `tests/ui/pass/`, `tests/fixtures/` and the book. Each mutation kind
//! (`seeded_bugs/kinds.rs`) makes one single-point change that creates a
//! real hazard (a synchronizer removed, a register moved to another
//! clock, a raw reset dropped, a declassify removed, ...) and is applied
//! to every design where it fits; design-specific mutations
//! (`seeded_bugs/specials.rs`) add the forms a pattern cannot write. The
//! code each mutant must produce comes from the specification, by hand,
//! never from the compiler's output. Near misses are changes that look
//! dangerous but are safe: they must add no error and no safety code.
//!
//! Every outcome is strict. A missed mutant must be in `KNOWN_MISSES`
//! with its issue, and a listed one must still be missed: when the
//! compiler is fixed the test fails until the entry is removed. The same
//! holds for `KNOWN_DIFFERENT` (rejected with another code) and
//! `KNOWN_FALSE_ALARMS`. Nothing is skipped silently: a design that does
//! not compile cleanly is left out only by a rule or a listed reason, and
//! the report prints it.
//!
//! The test needs no tool: it runs `volt check` only.

mod seeded_bugs;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use seeded_bugs::kinds::{kinds, Role};
use seeded_bugs::scan::Source;
use seeded_bugs::specials::SPECIALS;
use seeded_bugs::{is_error, is_safety, load_corpus, new_codes, Design, Worker};

/// Hand classification of a mutant.
enum Hand {
    /// Hazard: one of these codes must newly appear.
    Expect(&'static [&'static str]),
    /// Not a hazard under the specification: run as a near miss (no new
    /// error, no new safety code except the listed ones).
    Safe(&'static [&'static str]),
}

/// Mutants of kinds without a fixed answer (`expect: None`) and the
/// mutants whose answer differs from their kind's rule. The third field
/// is the reasoning from the specification.
const HAND: &[(&str, Hand, &str)] = &[
    // ── rdc.drop-raw-reset: the raw reset port is removed, so the
    //    domains it fed get the automatic port (ADR-0065 §1) ──
    ("rdc.drop-raw-reset | examples/hybrid_accel/hybrid_top.volt | HybridTop.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | examples/vga/vga_top.volt | VgaTop.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | demo/crossing_fixed.volt | Crossing.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | tests/ui/pass/129_prim_struct_elements.volt | PrimStructs.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | tests/ui/pass/130_domain_annotations.volt | Annotated.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | tests/ui/pass/27_async_fifo.volt | FifoBridge.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | tests/ui/pass/28_handshake_sync.volt | HsBridge.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | tests/ui/pass/29_pulse_sync.volt | PulseBridge.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | tests/ui/pass/74_sdc_multi_clock.volt | Bridge.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | book/src/tutorial/two-clocks.md#sample_bridge.volt@2 | SampleBridge.rst", Hand::Expect(&["W3010"]), R5S),
    ("rdc.drop-raw-reset | book/src/tutorial/two-clocks.md#sample_bridge.volt@3 | SampleBridge.rst", Hand::Expect(&["W3010"]), R5S),
    (
        "rdc.drop-raw-reset | tests/ui/pass/86_rdc_raw_reset_sync_domains.volt | Video.rst",
        Hand::Expect(&["W3010"]),
        "ADR-0065 R5' + ADR-0098 / sv-mapping.md §7: a domain without a `reset` key has the spec default (sync, active_high); Sys and Pix both have flops, so they share the automatic `rst`",
    ),
    (
        "rdc.drop-raw-reset | tests/ui/pass/88_rdc_ram_write_clock_no_reset.volt | Display.rst",
        Hand::Expect(&["W3010"]),
        "ADR-0065 R5' + ADR-0098 / sv-mapping.md §7: default sync reset; Display has flops on sys_clk (wa) and pix_clk (ra)",
    ),
    ("rdc.drop-raw-reset | tests/ui/pass/83_rdc_raw_reset_two_clocks.volt | Bridge.rst_n", Hand::Expect(&["E3003"]), R5),
    ("rdc.drop-raw-reset | book/src/cookbook/reset-sync.md#board.volt@2 | Board.rst_n", Hand::Expect(&["E3003"]), R5),
    ("rdc.drop-raw-reset | book/src/cookbook/reset-sync.md#board_led.volt | Board.rst_n", Hand::Expect(&["E3003"]), R5),
    ("rdc.drop-raw-reset | tests/ui/pass/84_rdc_raw_reset_per_domain.volt | Top.fast_rst_n", Hand::Expect(&["W3009"]), R1),
    ("rdc.drop-raw-reset | tests/ui/pass/85_rdc_raw_reset_hierarchy.volt | Top.rst_n", Hand::Expect(&["W3009"]), R1),
    ("rdc.drop-raw-reset | tests/ui/pass/87_sdc_targeted_all_bridges.volt | AllBridges.ext_rst_n", Hand::Expect(&["W3009"]), R1),
    ("rdc.drop-raw-reset | tests/ui/pass/84_rdc_raw_reset_per_domain.volt | Top.slow_rst", Hand::Safe(&[]), SYNC_ROOT),
    ("rdc.drop-raw-reset | tests/ui/pass/87_sdc_targeted_all_bridges.volt | AllBridges.ext_rst", Hand::Safe(&[]), SYNC_ROOT),
    ("rdc.drop-raw-reset | tests/fixtures/formal_obligations/multi_instance.volt | Parent.rst", Hand::Safe(&[]), SYNC_ROOT),
    ("rdc.drop-raw-reset | tests/fixtures/formal_obligations/multi_instance_satisfy.volt | Parent.rst", Hand::Safe(&[]), SYNC_ROOT),
    ("rdc.drop-raw-reset | tests/fixtures/formal_obligations/satisfy.volt | Parent.rst", Hand::Safe(&[]), SYNC_ROOT),
    ("rdc.drop-raw-reset | tests/fixtures/formal_obligations/three_level_satisfy.volt | Top.rst", Hand::Safe(&[]), SYNC_ROOT),
    ("rdc.drop-raw-reset | tests/fixtures/formal_obligations/three_level_violate.volt | Top.rst", Hand::Safe(&[]), SYNC_ROOT),
    ("rdc.drop-raw-reset | tests/fixtures/formal_obligations/violate.volt | Parent.rst", Hand::Safe(&[]), SYNC_ROOT),
    // ── trust.port-downgrade: the output moves to the public domain ──
    ("trust.port-downgrade | examples/crypto/key_store.volt | KeyStore.key: @SecureCore -> @Debug", Hand::Expect(&["E3009"]), "K11: key = key_r, a secret register (K2/K4: written in the SecureCore clock's block)"),
    ("trust.port-downgrade | tests/ui/pass/71_declassify.volt | KeyPresence.key: @SecureCore -> @Debug", Hand::Expect(&["E3009"]), "K11: key = key_r, a secret register"),
    ("trust.port-downgrade | tests/ui/pass/70_trust_levels.volt | TrustFlows.secret_out: @Secure -> @Debug", Hand::Expect(&["E3009"]), "K11: secret_out = acc_r & cfg; acc_r takes key's secret level (unclassified register)"),
    ("trust.port-downgrade | tests/ui/pass/70_trust_levels.volt | TrustFlows.cfg_out: @Internal -> @Debug", Hand::Expect(&["E3009"]), "K11: cfg_out = cfg + 1, confidential → public"),
    ("trust.port-downgrade | tests/ui/pass/70_trust_levels.volt | Upward.gate: @Secure -> @Debug", Hand::Safe(&[]), "K11: gate = !ctl and ctl is public: public → public, no hazard"),
    // ── other kinds without a fixed answer ──
    ("timing.drop-retiming | tests/ui/pass/44_delayed_aligned.volt | let asserted: Delayed annotation removed", Hand::Expect(&["E5010"]), "ADR-0037: the arms s2 (2) and s1 (1) are combined without the retiming assertion"),
    ("cdc.extern-domain-merge | tests/ui/pass/62_extern_domains.volt | ExtAsyncFifo: @Dst -> @Src", Hand::Expect(&["E3014"]), "K8a: the instance binds wr_clk to sys_clk and rd_clk to pix_clk, now the same symbolic domain"),
    // ── the sync result is read inside an `on` block: K7 names both codes ──
    ("cdc.drop-sync | demo/crossing_fixed.volt | sync(pressed_r, slow_clk)", Hand::Expect(CDC_READ), ON_READ),
    ("cdc.sync-to-other-clock | demo/crossing_fixed.volt | sync(pressed_r, slow_clk) -> fast_clk", Hand::Expect(CDC_READ), ON_READ),
    ("cdc.drop-sync | book/src/tutorial/two-clocks.md#sample_bridge.volt@2 | sync(last_r, sys_clk)", Hand::Expect(CDC_READ), ON_READ),
    ("cdc.sync-to-other-clock | book/src/tutorial/two-clocks.md#sample_bridge.volt@2 | sync(last_r, sys_clk) -> adc_clk", Hand::Expect(CDC_READ), ON_READ),
    // ── double writer of a register with an explicit domain: K7 (write
    //    outside its domain, E3001/E3012) applies as well as K4/ADR-0073 ──
    ("cdc.double-writer | tests/ui/pass/83_rdc_raw_reset_two_clocks.volt | Bridge: seen also written on slow_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    ("cdc.double-writer | tests/ui/pass/84_rdc_raw_reset_per_domain.volt | Top: ra also written on slow_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    ("cdc.double-writer | tests/ui/pass/84_rdc_raw_reset_per_domain.volt | Top: rb also written on fast_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    ("cdc.double-writer | tests/ui/pass/86_rdc_raw_reset_sync_domains.volt | Video: ra also written on pix_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    ("cdc.double-writer | tests/ui/pass/86_rdc_raw_reset_sync_domains.volt | Video: rc also written on sys_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    ("cdc.double-writer | tests/ui/pass/88_rdc_ram_write_clock_no_reset.volt | Display: wa also written on pix_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    ("cdc.double-writer | tests/ui/pass/88_rdc_ram_write_clock_no_reset.volt | Display: ra also written on sys_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    ("cdc.double-writer | tests/ui/pass/130_domain_annotations.volt | Annotated: last also written on slow_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    ("cdc.double-writer | tests/ui/pass/130_domain_annotations.volt | Annotated: count also written on fast_clk", Hand::Expect(DW_EXPLICIT), EXPLICIT),
    // ── an input used only as a sync() source: moved to the destination
    //    domain, the module holds no crossing; the spec's answer is the
    //    redundant-sync warning (K9) ──
    ("cdc.port-domain-swap | tests/ui/pass/13_cdc_correct_bridge.volt | CdcBridge.fast_flag: @Fast -> @Slow", Hand::Expect(&["W3002"]), SYNC_ONLY),
    ("cdc.port-domain-swap | examples/vga/vga_top.volt | VgaTop.invert: @SysDomain -> @PixDomain", Hand::Expect(&["W3002"]), SYNC_ONLY),
    ("cdc.port-domain-swap | tests/fixtures/parity/p24_sync_src_width.volt | M.x: @A -> @B", Hand::Expect(&["W3002"]), SYNC_ONLY),
];

const R5: &str =
    "ADR-0065 R5: two async domains of one polarity share the automatic `rst_n`: E3003";
const R5S: &str = "ADR-0065 R5': two sync domains with flops share the automatic `rst`: W3010";
const R1: &str = "ADR-0065 R1: an async domain of the unit root without a raw port: W3009";
const SYNC_ROOT: &str = "ADR-0065 §3: the root warning is for async domains only; a sync automatic port is the documented contract, so no code";
const ON_READ: &str = "domain-inference.md K7: the value is read in an `on` block of the other domain (the K7 example says E3001, §5 names E3012)";
const EXPLICIT: &str = "K4 explicit domain + K7 (write outside the register's domain: E3001/E3012); K4 (E3011) and ADR-0073 d03b (E4001) name the same case";
const SYNC_ONLY: &str = "K9: the port is read only as a sync() source; with it in the destination domain the sync is redundant (W3002) and no crossing is left in the module";
const DW_EXPLICIT: &[&str] = &["E3011", "E4001", "E3001", "E3012"];
const CDC_READ: &[&str] = seeded_bugs::kinds::CDC_READ;

/// Mutants the compiler accepts although the specification rejects
/// them, with the issue that tracks each root cause.
const KNOWN_MISSES: &[(&str, u32)] = &[(
    "cdc.leak-clockless-child | tests/ui/pass/13_cdc_correct_bridge.volt | slow_flag = p.y (Pass { a: fast_flag }, no clock)",
    189,
)];

/// Mutants rejected, but with another code than the specification
/// gives; the second field is the new codes, comma-separated.
const KNOWN_DIFFERENT: &[(&str, &str)] = &[];

/// Near misses that raise an error or a safety code, with the issue
/// and the new codes.
const KNOWN_FALSE_ALARMS: &[(&str, u32, &str)] = &[];

/// Designs left out of the corpus by name.
const EXCLUDED_DESIGNS: &[(&str, &str)] = &[(
    "demo/crossing.volt",
    "the failing half of the CDC tour (book tour/cdc-error.md, should_fail=E3001)",
)];

/// Directories whose files are written to fail: a file there with an
/// error in its baseline is left out (and printed).
const FAILING_BY_DESIGN: &[&str] = &["tests/fixtures/parity/"];

struct Mutant {
    id: String,
    kind: String,
    design: usize,
    text: String,
    role: Role,
    /// The specification rule behind `role`.
    spec: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    Caught,
    Missed,
    Different(String),
    Quiet,
    Alarm(String),
}

fn run_parallel<T: Send + Sync, R: Send>(
    items: &[T],
    workers: &[Worker],
    f: impl Fn(&Worker, &T) -> R + Sync,
) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for w in workers {
            let (next, out, f) = (&next, &out, &f);
            s.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= items.len() {
                    break;
                }
                let r = f(w, &items[i]);
                out.lock().unwrap()[i] = Some(r);
            });
        }
    });
    out.into_inner()
        .unwrap()
        .into_iter()
        .map(|r| r.expect("every job ran"))
        .collect()
}

fn apply_special(text: &str, edits: &[(&str, &str)]) -> Result<String, String> {
    let mut t = text.to_string();
    for (find, replace) in edits {
        let n = t.matches(find).count();
        if n != 1 {
            return Err(format!(
                "`{}` occurs {n} times (expected once)",
                find.escape_debug()
            ));
        }
        t = t.replacen(find, replace, 1);
    }
    Ok(t)
}

#[test]
fn seeded_bugs_are_caught_and_near_misses_stay_quiet() {
    let corpus = load_corpus();
    let designs: &[Design] = &corpus.designs;
    let mut problems: Vec<String> = Vec::new();

    let base = std::env::temp_dir().join(format!("volt-seeded-bugs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(1, 4);
    let workers: Vec<Worker> = (0..n)
        .map(|k| Worker::new(&base, k, &corpus.workspace))
        .collect();

    // ── Baseline: which designs compile without an error ──
    let baselines: Vec<Vec<String>> = run_parallel(designs, &workers, |w, d| w.check(d, &d.text));
    let mut excluded: Vec<(String, String)> = corpus
        .excluded
        .iter()
        .map(|e| (e.id.clone(), e.reason.clone()))
        .collect();
    let mut clean = vec![false; designs.len()];
    for (i, d) in designs.iter().enumerate() {
        let errors: Vec<&String> = baselines[i].iter().filter(|c| is_error(c)).collect();
        let listed = EXCLUDED_DESIGNS.iter().find(|(id, _)| *id == d.id);
        match (errors.is_empty(), listed) {
            (true, None) => clean[i] = true,
            (true, Some(_)) => problems.push(format!(
                "{}: listed in EXCLUDED_DESIGNS but compiles without an error now",
                d.id
            )),
            (false, Some((_, why))) => excluded.push((d.id.clone(), why.to_string())),
            (false, None) if FAILING_BY_DESIGN.iter().any(|p| d.id.starts_with(p)) => {
                excluded.push((d.id.clone(), "written to fail (parity fixture)".to_string()))
            }
            (false, None) => problems.push(format!(
                "{}: does not compile cleanly ({}); fix it or list it in EXCLUDED_DESIGNS",
                d.id,
                errors
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    // ── Mutants ──
    let hand: BTreeMap<&str, (&Hand, &str)> =
        HAND.iter().map(|(id, h, why)| (*id, (h, *why))).collect();
    let mut mutants: Vec<Mutant> = Vec::new();
    let kinds = kinds();
    for (i, d) in designs.iter().enumerate() {
        if !clean[i] {
            continue;
        }
        let src = Source::new(&d.text);
        for k in &kinds {
            for (site, text) in (k.sites)(&src) {
                let id = format!("{} | {} | {}", k.name, d.id, site);
                let spec = hand.get(id.as_str()).map_or(k.spec, |h| h.1).to_string();
                let role = match (hand.get(id.as_str()), k.expect) {
                    (Some((Hand::Expect(c), _)), _) => Role::Hazard(c.to_vec()),
                    (Some((Hand::Safe(c), _)), _) => Role::NearMiss(c.to_vec()),
                    (None, Some(c)) if k.near_miss => Role::NearMiss(c.to_vec()),
                    (None, Some(c)) => Role::Hazard(c.to_vec()),
                    (None, None) => {
                        problems.push(format!(
                            "{id}: not classified; add it to HAND with the spec rule"
                        ));
                        continue;
                    }
                };
                if text == d.text {
                    problems.push(format!("{id}: the mutation changes nothing"));
                    continue;
                }
                mutants.push(Mutant {
                    id,
                    kind: k.name.to_string(),
                    design: i,
                    text,
                    role,
                    spec,
                });
            }
        }
    }
    for s in SPECIALS {
        let id = format!("{} | {} | {}", s.kind, s.design, s.site);
        let Some(i) = designs.iter().position(|d| d.id == s.design) else {
            problems.push(format!("{id}: design not found in the corpus"));
            continue;
        };
        if !clean[i] {
            problems.push(format!("{id}: design is not clean"));
            continue;
        }
        match apply_special(&designs[i].text, s.edits) {
            Ok(text) => mutants.push(Mutant {
                id,
                kind: s.kind.to_string(),
                design: i,
                text,
                role: if s.near_miss {
                    Role::NearMiss(s.codes.to_vec())
                } else {
                    Role::Hazard(s.codes.to_vec())
                },
                spec: s.spec.to_string(),
            }),
            Err(e) => problems.push(format!("{id}: {e}")),
        }
    }
    let ids: BTreeSet<&str> = mutants.iter().map(|m| m.id.as_str()).collect();
    if ids.len() != mutants.len() {
        problems.push("two mutants share an id".to_string());
    }
    for id in HAND.iter().map(|h| h.0) {
        if !ids.contains(id) {
            problems.push(format!("HAND entry matches no mutant: {id}"));
        }
    }

    // ── Run ──
    let outcomes: Vec<(Outcome, Vec<String>)> = run_parallel(&mutants, &workers, |w, m| {
        let now = w.check(&designs[m.design], &m.text);
        let added = new_codes(&baselines[m.design], &now);
        let outcome = match &m.role {
            Role::Hazard(expect) => {
                if added.iter().any(|c| expect.contains(&c.as_str())) {
                    Outcome::Caught
                } else {
                    let other: Vec<&str> = added
                        .iter()
                        .filter(|c| is_error(c) || is_safety(c))
                        .map(String::as_str)
                        .collect();
                    if other.is_empty() {
                        Outcome::Missed
                    } else {
                        Outcome::Different(other.join(","))
                    }
                }
            }
            Role::NearMiss(allowed) => {
                let alarms: Vec<&str> = added
                    .iter()
                    .filter(|c| (is_error(c) || is_safety(c)) && !allowed.contains(&c.as_str()))
                    .map(String::as_str)
                    .collect();
                if alarms.is_empty() {
                    Outcome::Quiet
                } else {
                    Outcome::Alarm(alarms.join(","))
                }
            }
        };
        (outcome, added)
    });
    let _ = std::fs::remove_dir_all(&base);

    // ── Compare with the lists ──
    let misses: BTreeMap<&str, u32> = KNOWN_MISSES.iter().copied().collect();
    let different: BTreeMap<&str, &str> = KNOWN_DIFFERENT.iter().copied().collect();
    let alarms: BTreeMap<&str, (u32, &str)> = KNOWN_FALSE_ALARMS
        .iter()
        .map(|(id, n, c)| (*id, (*n, *c)))
        .collect();
    for (m, (o, _)) in mutants.iter().zip(&outcomes) {
        let id = m.id.as_str();
        let listed_miss = misses.contains_key(id);
        let listed_diff = different.get(id).copied();
        let listed_alarm = alarms.get(id).copied();
        match o {
            Outcome::Caught | Outcome::Quiet
                if listed_miss || listed_diff.is_some() || listed_alarm.is_some() =>
            {
                problems.push(format!(
                    "{id}: now {} as the spec says; remove it from its known list (and close the issue if it was the last one)",
                    if *o == Outcome::Caught { "caught" } else { "quiet" }
                ))
            }
            Outcome::Missed if !listed_miss => problems.push(format!(
                "{id}: MISSED (no error, no expected code); unlisted"
            )),
            Outcome::Different(c) if listed_diff != Some(c.as_str()) => problems.push(format!(
                "{id}: rejected with {c} instead of {:?}; listed: {listed_diff:?}",
                m.role
            )),
            Outcome::Alarm(c) if listed_alarm.map(|a| a.1) != Some(c.as_str()) => {
                problems.push(format!(
                    "{id}: near miss raises {c}; listed: {:?}",
                    listed_alarm.map(|a| a.1)
                ))
            }
            _ => {}
        }
    }
    for id in misses.keys().chain(different.keys()).chain(alarms.keys()) {
        if !ids.contains(id) {
            problems.push(format!("known-list entry matches no mutant: {id}"));
        }
    }

    // ── Report ──
    let mut by_kind: BTreeMap<&str, [usize; 5]> = BTreeMap::new();
    for (m, (o, _)) in mutants.iter().zip(&outcomes) {
        let slot = match o {
            Outcome::Caught => 0,
            Outcome::Missed => 1,
            Outcome::Different(_) => 2,
            Outcome::Quiet => 3,
            Outcome::Alarm(_) => 4,
        };
        by_kind.entry(m.kind.as_str()).or_default()[slot] += 1;
    }
    let used: BTreeSet<usize> = mutants.iter().map(|m| m.design).collect();
    println!(
        "seeded-bug corpus: {} designs ({} clean, {} with a mutation site), {} excluded",
        designs.len(),
        clean.iter().filter(|c| **c).count(),
        used.len(),
        excluded.len()
    );
    println!(
        "{} mutants: {} hazards, {} near misses",
        mutants.len(),
        mutants
            .iter()
            .filter(|m| matches!(m.role, Role::Hazard(_)))
            .count(),
        mutants
            .iter()
            .filter(|m| matches!(m.role, Role::NearMiss(_)))
            .count()
    );
    println!("{:<34} caught missed differ quiet alarm", "kind");
    for (k, c) in &by_kind {
        println!(
            "{k:<34} {:>6} {:>6} {:>6} {:>5} {:>5}",
            c[0], c[1], c[2], c[3], c[4]
        );
    }
    // Which expected codes caught each hazard kind (K4 and ADR-0073 name
    // two codes for a double writer, K7 two for an `on` block read).
    let mut caught_by: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
    for (m, (o, added)) in mutants.iter().zip(&outcomes) {
        if let (Outcome::Caught, Role::Hazard(expect)) = (o, &m.role) {
            let hit: Vec<&str> = added
                .iter()
                .filter(|c| expect.contains(&c.as_str()))
                .map(String::as_str)
                .collect();
            *caught_by
                .entry(m.kind.as_str())
                .or_default()
                .entry(hit.join("+"))
                .or_default() += 1;
        }
    }
    for (k, codes) in &caught_by {
        let list: Vec<String> = codes.iter().map(|(c, n)| format!("{c} x{n}")).collect();
        println!("caught by: {k:<34} {}", list.join(", "));
    }
    for (m, (o, added)) in mutants.iter().zip(&outcomes) {
        if !matches!(o, Outcome::Caught | Outcome::Quiet) {
            println!(
                "{o:?}: {} (expected {:?}; new codes {added:?})
    spec: {}",
                m.id, m.role, m.spec
            );
        }
    }
    for (id, why) in &excluded {
        println!("excluded: {id}: {why}");
    }
    assert!(
        problems.is_empty(),
        "{} problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}
