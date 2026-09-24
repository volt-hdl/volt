//! Cargo biçimli test raporu. BİLEREK İngilizcedir (makine-okur).

use std::process::ExitCode;

use super::contracts::{cover_summary_lines, CoverCount};
use super::tb_output::{AssertFailure, TestOutcome};

/// Bir grubun test satırları: `test <ad> ... ok|FAILED`.
pub(super) fn print_test_lines(outcomes: &[TestOutcome]) {
    for o in outcomes {
        println!(
            "test {} ... {}",
            o.name,
            if o.passed { "ok" } else { "FAILED" }
        );
    }
}

/// Cargo biçimli özet; en az bir test kaldıysa çıkış kodu 5. Cover
/// özeti (ADR-0064) bilgidir, çıkış kodunu etkilemez.
pub(super) fn print_summary(outcomes: &[TestOutcome], covers: &[CoverCount]) -> ExitCode {
    let failed: Vec<&TestOutcome> = outcomes.iter().filter(|o| !o.passed).collect();
    let passed = outcomes.len() - failed.len();
    if !failed.is_empty() {
        println!("\nfailures:");
    }
    for o in &failed {
        println!("---- {} ----", o.name);
        for line in outcome_lines(o) {
            println!("{line}");
        }
    }
    let summary = cover_summary_lines(covers);
    if !summary.is_empty() {
        println!();
        for line in summary {
            println!("{line}");
        }
    }
    if failed.is_empty() {
        println!("\ntest result: ok. {passed} passed; 0 failed");
        return ExitCode::SUCCESS;
    }
    println!(
        "\ntest result: FAILED. {passed} passed; {} failed",
        failed.len()
    );
    ExitCode::from(5)
}

/// Kalan testin ayrıntısı: kontrat ihlali (ADR-0064) test iddiasından
/// ÖNCE gelir — ihlalin döngüsünde test durur.
fn outcome_lines(o: &TestOutcome) -> Vec<String> {
    match (&o.contract, &o.failure) {
        (Some(v), _) => v.report_lines(),
        (None, Some(f)) => failure_lines(f),
        (None, None) => vec!["  test failed (no assertion detail)".to_string()],
    }
}

fn failure_lines(f: &AssertFailure) -> Vec<String> {
    let mut lines = match f.kind.as_str() {
        "assert_eq" | "assert_ne" => {
            let named = |v: u64| match &f.labels {
                Some(l) => format!("{v} ({})", l.label(v)),
                None => v.to_string(),
            };
            vec![
                format!("  {} failed at {}", f.kind, f.loc),
                format!("    left:  {}", named(f.left)),
                format!("    right: {}", named(f.right)),
            ]
        }
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
        f.labels = Some(crate::sim_lower::EnumLabels {
            enum_name: "State".to_string(),
            variants: vec![("Idle".into(), 0), ("Run".into(), 1), ("Done".into(), 2)],
        });
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

    #[test]
    fn print_summary_exit_code_follows_failures() {
        let outcome = |passed| TestOutcome {
            name: "t".to_string(),
            passed,
            failure: None,
            contract: None,
        };
        let code = |o: &[TestOutcome]| format!("{:?}", print_summary(o, &[]));
        assert_eq!(code(&[outcome(true)]), format!("{:?}", ExitCode::SUCCESS));
        assert_eq!(
            code(&[outcome(true), outcome(false)]),
            format!("{:?}", ExitCode::from(5))
        );
    }
}
