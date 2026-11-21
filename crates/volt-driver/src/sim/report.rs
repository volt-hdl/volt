//! Cargo biçimli test raporu. BİLEREK İngilizcedir (makine-okur).

use std::process::ExitCode;

use super::contracts::{cover_summary_lines, CoverCount};
use super::tb_output::{AssertFailure, GroupRun, GroupStop, SimStop, TestOutcome};

/// Bir grubun test satırlarını basar (`test_lines`).
pub(super) fn print_test_lines(run: &GroupRun) {
    for line in test_lines(run) {
        println!("{line}");
    }
}

/// Bir grubun test satırları: `test <ad> ... ok|FAILED`, başlamayan
/// testler için `test <ad> ... not run` (ADR-0130).
fn test_lines(run: &GroupRun) -> Vec<String> {
    let mut lines: Vec<String> = run
        .outcomes
        .iter()
        .map(|o| {
            format!(
                "test {} ... {}",
                o.name,
                if o.passed { "ok" } else { "FAILED" }
            )
        })
        .collect();
    lines.extend(run.not_run.iter().map(|n| format!("test {n} ... not run")));
    lines
}

/// Bütün grupların toplanmış sonucu (`volt test` özeti).
#[derive(Debug, Default)]
pub(super) struct Totals {
    pub outcomes: Vec<TestOutcome>,
    pub not_run: Vec<String>,
    pub stops: Vec<GroupStop>,
}

impl Totals {
    pub(super) fn add(&mut self, run: GroupRun) {
        self.outcomes.extend(run.outcomes);
        self.not_run.extend(run.not_run);
        self.stops.extend(run.stop);
    }
}

/// Özeti basar ve çıkış kodunu döndürür (`summary`).
pub(super) fn print_summary(totals: &Totals, covers: &[CoverCount]) -> ExitCode {
    let (lines, code) = summary(totals, covers);
    for line in lines {
        println!("{line}");
    }
    code
}

/// Cargo biçimli özet; en az bir test kaldıysa, başlamayan test varsa
/// ya da bir benzetim beklenmedik bittiyse çıkış kodu 5 (ADR-0130).
/// Cover özeti (ADR-0064) bilgidir, çıkış kodunu etkilemez.
fn summary(totals: &Totals, covers: &[CoverCount]) -> (Vec<String>, ExitCode) {
    let mut out: Vec<String> = Vec::new();
    let failed: Vec<&TestOutcome> = totals.outcomes.iter().filter(|o| !o.passed).collect();
    let passed = totals.outcomes.len() - failed.len();
    if !failed.is_empty() || !totals.stops.is_empty() {
        out.extend([String::new(), "failures:".to_string()]);
    }
    for o in &failed {
        out.push(format!("---- {} ----", o.name));
        out.extend(outcome_lines(o));
        // ADR-0095: düşen testin kaydı (ADR-0092 açma satırı biçimi).
        if let Some(hint) = &o.waveform {
            out.push(format!("  Waveform {hint}"));
        }
    }
    for s in &totals.stops {
        out.push(format!("---- simulation of {} ----", s.module));
        out.extend(group_stop_lines(s));
    }
    if !totals.not_run.is_empty() {
        out.push(String::new());
        out.push("not run (the simulation stopped before these tests started):".to_string());
        out.extend(totals.not_run.iter().map(|n| format!("    {n}")));
    }
    let covered = cover_summary_lines(covers);
    if !covered.is_empty() {
        out.push(String::new());
        out.extend(covered);
    }
    out.push(String::new());
    if failed.is_empty() && totals.not_run.is_empty() && totals.stops.is_empty() {
        out.push(format!("test result: ok. {passed} passed; 0 failed"));
        return (out, ExitCode::SUCCESS);
    }
    let not_run = match totals.not_run.len() {
        0 => String::new(),
        n => format!("; {n} not run"),
    };
    out.push(format!(
        "test result: FAILED. {passed} passed; {} failed{not_run}",
        failed.len()
    ));
    (out, ExitCode::from(5))
}

/// Kalan testin ayrıntısı: benzetimin duruşu, sonra kontrat ihlali
/// (ADR-0064) test iddiasından ÖNCE gelir — ihlalin döngüsünde test
/// durur.
fn outcome_lines(o: &TestOutcome) -> Vec<String> {
    if let Some(stop) = &o.stop {
        return stop_lines("the simulation stopped during this test", stop);
    }
    match (&o.contract, &o.failure) {
        (Some(v), _) => v.report_lines(),
        (None, Some(f)) => failure_lines(f),
        (None, None) => vec!["  test failed (no assertion detail)".to_string()],
    }
}

/// Teste bağlanamayan duruş: son testten sonra ya da ilk testten önce.
fn group_stop_lines(s: &GroupStop) -> Vec<String> {
    let what = match &s.after {
        Some(name) => format!("the simulation stopped after test {name}"),
        None => "the simulation stopped before its first test".to_string(),
    };
    stop_lines(&what, &s.stop)
}

/// Duruşun satırları (ADR-0125 düzeni: ne oldu, neden, ne yapabilirsin).
fn stop_lines(what: &str, stop: &SimStop) -> Vec<String> {
    let how = match stop.code {
        Some(code) => format!("exit code {code}"),
        None => "ended by a signal".to_string(),
    };
    let mut lines = vec![format!("  {what} ({how})")];
    lines.extend(stop.messages.iter().map(|m| format!("    simulator: {m}")));
    lines.push(
        "  reason: the simulator ended the run before every test finished \
         ($fatal, $error, $stop, a failed assertion or a crash), so this run \
         has no trustworthy result"
            .to_string(),
    );
    lines.push(
        "  help: fix what the simulator reports above; \
         `volt test --nocapture` shows its full output"
            .to_string(),
    );
    lines
}

fn failure_lines(f: &AssertFailure) -> Vec<String> {
    let mut lines = match f.kind.as_str() {
        "assert_eq" | "assert_ne" => match &f.labels {
            // ADR-0077: struct değeri alan adlarıyla, farklı alanlar ayrıca.
            Some(crate::sim_lower::ValueLabels::Struct(s)) => {
                let mut lines = vec![
                    format!("  {} failed at {}", f.kind, f.loc),
                    format!("    left:  {}", s.render(f.left)),
                    format!("    right: {}", s.render(f.right)),
                ];
                let differs = s.differs(f.left, f.right);
                if !differs.is_empty() {
                    lines.push(format!("    differs: {}", differs.join(", ")));
                }
                lines
            }
            labels => {
                let named = |v: u64| match labels {
                    Some(l) => format!("{v} ({})", l.label(v)),
                    None => v.to_string(),
                };
                vec![
                    format!("  {} failed at {}", f.kind, f.loc),
                    format!("    left:  {}", named(f.left)),
                    format!("    right: {}", named(f.right)),
                ]
            }
        },
        "index_out_of_bounds" => vec![
            format!("  index out of bounds at {}", f.loc),
            format!("    index: {}", f.left),
            format!("    len:   {}", f.right),
        ],
        "division_by_zero" => vec![
            format!("  division by zero at {}", f.loc),
            format!("    dividend: {}", f.left),
        ],
        "port_overflow" => port_overflow_lines(f),
        "load_too_long" => vec![
            format!("  load() source does not fit the target at {}", f.loc),
            format!("    source elements: {}", f.left),
            format!("    target elements: {}", f.right),
        ],
        "load_value_too_wide" => vec![
            format!(
                "  load() value does not fit the target element at {}",
                f.loc
            ),
            format!("    value: {}", f.left),
            format!("    index: {}", f.right),
        ],
        _ => vec![
            format!("  {} failed at {}", f.kind, f.loc),
            format!("    value: {}", f.left),
        ],
    };
    if let Some(ctx) = &f.loop_ctx {
        lines.push(format!("    loop:  {ctx}"));
    }
    lines
}

/// Port genişlik hatası (ADR-0059): `right` port genişliğidir (0 =
/// derleyici çözemedi, denetim C++ depolama tipine göre yapıldı).
fn port_overflow_lines(f: &AssertFailure) -> Vec<String> {
    let value = volt_hir::describe_value(f.left);
    let Some((name, ty)) = &f.port else {
        return vec![format!("  port cannot hold value {value} at {}", f.loc)];
    };
    let Some(bits) = u32::try_from(f.right).ok().filter(|b| *b > 0) else {
        return vec![format!(
            "  port '{name}' cannot hold value {value} at {}",
            f.loc
        )];
    };
    let kind = if ty.starts_with('i') {
        volt_hir::ScalarKind::SInt
    } else {
        volt_hir::ScalarKind::UInt
    };
    let range = volt_hir::PortWidth { bits, kind }.write_range();
    vec![
        format!(
            "  port '{name}' ({ty}) cannot hold value {value} at {}",
            f.loc
        ),
        format!("    range: {range}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::tb_output::parse_assert_fail;
    use super::*;

    #[test]
    fn port_overflow_report_names_port_value_and_range() {
        let f = parse_assert_fail("port_overflow t_test.volt:14 left=8 right=3 port=addr:u3")
            .expect("ayrışmalı");
        assert_eq!(
            port_overflow_lines(&f),
            vec![
                "  port 'addr' (u3) cannot hold value 8 at t_test.volt:14".to_string(),
                "    range: 0..7".to_string(),
            ]
        );
    }

    #[test]
    fn port_overflow_report_shows_negative_numbers_and_signed_range() {
        let f = parse_assert_fail(
            "port_overflow t_test.volt:9 left=18446744073709551487 right=8 port=sv:i8",
        )
        .expect("ayrışmalı");
        let lines = port_overflow_lines(&f);
        assert!(lines[0].contains("(i8) cannot hold value 18446744073709551487 (-129)"));
        assert_eq!(lines[1], "    range: -128..255");
    }

    #[test]
    fn port_overflow_report_without_a_known_width_has_no_range() {
        let f = parse_assert_fail("port_overflow t_test.volt:9 left=70000 right=0 port=word:?")
            .expect("ayrışmalı");
        assert_eq!(
            port_overflow_lines(&f),
            vec!["  port 'word' cannot hold value 70000 at t_test.volt:9".to_string()]
        );
    }

    #[test]
    fn enum_assert_failure_shows_variant_names_and_invalid_codes() {
        // ADR-0074: testbench sayı basar, sürücü varyant adını ekler.
        let mut f = parse_assert_fail("assert_eq t.volt:9 left=3 right=0").expect("ayrışmalı");
        f.labels = Some(crate::sim_lower::ValueLabels::Enum(
            crate::sim_lower::EnumLabels {
                enum_name: "State".to_string(),
                variants: vec![("Idle".into(), 0), ("Run".into(), 1), ("Done".into(), 2)],
            },
        ));
        assert_eq!(
            failure_lines(&f),
            [
                "  assert_eq failed at t.volt:9",
                "    left:  3 (State: invalid code)",
                "    right: 0 (State::Idle)",
            ]
        );
    }

    #[test]
    fn failure_lines_cover_every_kind_and_append_loop_context() {
        let lines = |text: &str| failure_lines(&parse_assert_fail(text).expect("ayrışmalı"));
        assert_eq!(
            lines("assert_ne t.volt:3 left=4 right=4 loop=i=2"),
            [
                "  assert_ne failed at t.volt:3",
                "    left:  4",
                "    right: 4",
                "    loop:  i = 2"
            ]
        );
        assert_eq!(
            lines("index_out_of_bounds t.volt:5 left=9 right=4"),
            [
                "  index out of bounds at t.volt:5",
                "    index: 9",
                "    len:   4"
            ]
        );
        assert_eq!(
            lines("division_by_zero t.volt:6 left=7 right=0"),
            ["  division by zero at t.volt:6", "    dividend: 7"]
        );
        assert_eq!(
            lines("load_too_long t.volt:7 left=9 right=8"),
            [
                "  load() source does not fit the target at t.volt:7",
                "    source elements: 9",
                "    target elements: 8"
            ]
        );
        assert_eq!(
            lines("load_value_too_wide t.volt:8 left=256 right=3"),
            [
                "  load() value does not fit the target element at t.volt:8",
                "    value: 256",
                "    index: 3"
            ]
        );
        assert_eq!(
            lines("assert t.volt:9 left=0 right=0"),
            ["  assert failed at t.volt:9", "    value: 0"]
        );
    }

    fn outcome(passed: bool) -> TestOutcome {
        TestOutcome {
            name: "t".to_string(),
            passed,
            failure: None,
            contract: None,
            waveform: None,
            stop: None,
        }
    }

    fn code(totals: &Totals) -> String {
        format!("{:?}", summary(totals, &[]).1)
    }

    fn named(name: &str, passed: bool) -> TestOutcome {
        TestOutcome {
            name: name.to_string(),
            ..outcome(passed)
        }
    }

    #[test]
    fn a_normal_run_prints_the_same_report_as_before() {
        // ADR-0130: olağan koşunun raporu değişmez — "not run" ve duruş yok.
        let ok = Totals {
            outcomes: vec![named("a", true), named("b", true)],
            ..Totals::default()
        };
        assert_eq!(
            summary(&ok, &[]).0,
            ["", "test result: ok. 2 passed; 0 failed"]
        );
        let mut fails = named("b", false);
        fails.failure = parse_assert_fail("assert_eq t.volt:3 left=1 right=2");
        let failing = Totals {
            outcomes: vec![named("a", true), fails],
            ..Totals::default()
        };
        assert_eq!(
            summary(&failing, &[]).0,
            [
                "",
                "failures:",
                "---- b ----",
                "  assert_eq failed at t.volt:3",
                "    left:  1",
                "    right: 2",
                "",
                "test result: FAILED. 1 passed; 1 failed",
            ]
        );
    }

    #[test]
    fn a_stopped_run_lists_the_stop_and_the_tests_that_did_not_run() {
        let mut stopped = named("trips", false);
        stopped.stop = Some(stop(Some(1), &["%Fatal: x"]));
        let totals = Totals {
            outcomes: vec![named("first", true), stopped],
            not_run: vec!["later".to_string(), "last".to_string()],
            stops: Vec::new(),
        };
        let (lines, _) = summary(&totals, &[]);
        let at = |s: &str| lines.iter().position(|l| l == s).unwrap_or(usize::MAX);
        assert_eq!(&lines[..3], ["", "failures:", "---- trips ----"]);
        assert_eq!(
            lines[3],
            "  the simulation stopped during this test (exit code 1)"
        );
        let header = at("not run (the simulation stopped before these tests started):");
        assert_eq!(lines[header - 1], "");
        assert_eq!(lines[header + 1..header + 3], ["    later", "    last"]);
        assert_eq!(
            lines.last().map(String::as_str),
            Some("test result: FAILED. 1 passed; 1 failed; 2 not run")
        );
    }

    #[test]
    fn a_stop_outside_any_test_is_reported_under_failures_with_its_module() {
        let totals = Totals {
            outcomes: vec![named("a", true)],
            not_run: Vec::new(),
            stops: vec![GroupStop {
                module: "M".to_string(),
                after: Some("a".to_string()),
                stop: stop(Some(3), &[]),
            }],
        };
        let (lines, code) = summary(&totals, &[]);
        assert_eq!(&lines[..3], ["", "failures:", "---- simulation of M ----"]);
        assert_eq!(
            lines[3],
            "  the simulation stopped after test a (exit code 3)"
        );
        assert_eq!(
            lines.last().map(String::as_str),
            Some("test result: FAILED. 1 passed; 0 failed")
        );
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(5)));
    }

    #[test]
    fn test_lines_mark_results_and_tests_that_did_not_run() {
        let run = GroupRun {
            outcomes: vec![named("a", true), named("b", false)],
            not_run: vec!["c".to_string()],
            stop: None,
        };
        assert_eq!(
            test_lines(&run),
            ["test a ... ok", "test b ... FAILED", "test c ... not run"]
        );
    }

    #[test]
    fn totals_collect_every_group() {
        let mut totals = Totals::default();
        let group_stop = GroupStop {
            module: "M".to_string(),
            after: None,
            stop: stop(None, &[]),
        };
        totals.add(GroupRun {
            outcomes: vec![named("a", true)],
            not_run: vec!["b".to_string()],
            stop: Some(group_stop.clone()),
        });
        totals.add(GroupRun {
            outcomes: vec![named("c", false)],
            not_run: Vec::new(),
            stop: None,
        });
        assert_eq!(totals.outcomes, [named("a", true), named("c", false)]);
        assert_eq!(totals.not_run, ["b"]);
        assert_eq!(totals.stops, [group_stop]);
    }

    fn stop(code: Option<i32>, messages: &[&str]) -> SimStop {
        SimStop {
            code,
            messages: messages.iter().map(|m| m.to_string()).collect(),
        }
    }

    #[test]
    fn print_summary_exit_code_follows_failures() {
        let totals = |outcomes: Vec<TestOutcome>| Totals {
            outcomes,
            ..Totals::default()
        };
        assert_eq!(
            code(&totals(vec![outcome(true)])),
            format!("{:?}", ExitCode::SUCCESS)
        );
        assert_eq!(
            code(&totals(vec![outcome(true), outcome(false)])),
            format!("{:?}", ExitCode::from(5))
        );
    }

    #[test]
    fn a_test_that_did_not_run_or_a_stopped_simulation_fails_the_run() {
        // ADR-0130: every test passed, but one never started -> 5.
        let not_run = Totals {
            outcomes: vec![outcome(true)],
            not_run: vec!["later".to_string()],
            stops: Vec::new(),
        };
        assert_eq!(code(&not_run), format!("{:?}", ExitCode::from(5)));
        // Every test passed, but the simulation ended abnormally after them -> 5.
        let stopped = Totals {
            outcomes: vec![outcome(true)],
            not_run: Vec::new(),
            stops: vec![GroupStop {
                module: "M".to_string(),
                after: Some("t".to_string()),
                stop: stop(Some(3), &[]),
            }],
        };
        assert_eq!(code(&stopped), format!("{:?}", ExitCode::from(5)));
    }

    #[test]
    fn a_stopped_test_reports_the_exit_code_and_the_simulator_messages() {
        let mut o = outcome(false);
        o.stop = Some(stop(Some(1), &["%Fatal: chk.sv:2: out of range"]));
        // A contract violation or an assertion recorded before the stop
        // does not hide it: the stop is the first thing reported.
        o.failure = parse_assert_fail("assert_eq t.volt:3 left=1 right=2");
        let lines = outcome_lines(&o);
        assert_eq!(
            lines[..2],
            [
                "  the simulation stopped during this test (exit code 1)",
                "    simulator: %Fatal: chk.sv:2: out of range",
            ]
        );
        assert!(lines[2].starts_with("  reason: "), "{lines:?}");
        assert!(lines[3].starts_with("  help: "), "{lines:?}");
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn a_stop_by_a_signal_and_a_stop_outside_any_test_say_where_they_happened() {
        let after = GroupStop {
            module: "M".to_string(),
            after: Some("last".to_string()),
            stop: stop(None, &[]),
        };
        assert_eq!(
            group_stop_lines(&after)[0],
            "  the simulation stopped after test last (ended by a signal)"
        );
        let before = GroupStop {
            module: "M".to_string(),
            after: None,
            stop: stop(Some(0), &[]),
        };
        assert_eq!(
            group_stop_lines(&before)[0],
            "  the simulation stopped before its first test (exit code 0)"
        );
    }
}
