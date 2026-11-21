//! Testbench çıktısının (sv-emit VOLT-* protokolü) ayrıştırılması.

use super::contracts::{parse_contract_fail, ContractViolation};

/// Bir test yürütülebilirinden ayrıştırılan tek test sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TestOutcome {
    pub name: String,
    pub passed: bool,
    /// `VOLT-ASSERT-FAIL <kind> <loc> left=<l> right=<r>` ayrıntısı.
    pub failure: Option<AssertFailure>,
    /// `VOLT-CONTRACT-FAIL` — testi düşüren kontrat ihlali (ADR-0064).
    pub contract: Option<ContractViolation>,
    /// Düşen testin dalga formu açma satırı (ADR-0095); testbench
    /// çıktısında yok, sürücü ekler.
    pub waveform: Option<String>,
    /// Benzetim bu testin ortasında bitti (ADR-0130): `VOLT-TEST-BEGIN`
    /// basıldı, `VOLT-TEST-END` basılmadı. Böyle test kalmış sayılır.
    /// `parse_tb_output` yalnız yarıda kaldığını bilir (kod `None`, ileti
    /// yok); çıkış kodunu ve simülatör iletilerini `reconcile` doldurur.
    pub stop: Option<SimStop>,
}

/// Benzetim sürecinin beklenmedik bitişi (ADR-0130).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SimStop {
    /// Sürecin çıkış kodu; sinyalle sonlandıysa `None`.
    pub code: Option<i32>,
    /// Simülatörün kendi iletileri (`%Fatal`, `%Error` …): son
    /// `VOLT-TEST-BEGIN`'den sonraki protokol dışı stdout satırları ve
    /// stderr, en çok `MAX_SIM_MESSAGES` satır.
    pub messages: Vec<String>,
}

/// Hiçbir teste bağlanamayan beklenmedik bitiş: son testten sonra, iki
/// test arasında ya da ilk testten önce (ADR-0130).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GroupStop {
    pub module: String,
    /// Bitişten önce sonucu yazılan son test.
    pub after: Option<String>,
    pub stop: SimStop,
}

/// Bir grubun testbench koşusu, gruptaki testlerle uzlaştırılmış.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GroupRun {
    /// Sonucu olan testler; yarıda kalan test kalmış (`stop` dolu).
    pub outcomes: Vec<TestOutcome>,
    /// Hiç başlamayan testler, gruptaki sırasıyla. Geçti sayılmaz.
    pub not_run: Vec<String>,
    pub stop: Option<GroupStop>,
}

/// Raporda bir duruştan gösterilen en çok simülatör satırı.
const MAX_SIM_MESSAGES: usize = 20;

/// Testbench'in olağan bitişi: her test sonucunu yazar ve süreç, kalan
/// test yoksa 0, varsa 1 ile çıkar (sv-emit `test_testbench`). Bunun
/// dışındaki her bitiş simülatörün koşuyu kestiğini gösterir (`$fatal`,
/// `$error`, `$stop`, başarısız `assert`, çökme): yarıda kalan test
/// kalır, başlamayanlar "not run" olur, teste bağlanamayan bitiş grubun
/// duruşudur (ADR-0130). Verilator koşuyu keserken de 1 ile çıkar; bu
/// yüzden çıkış kodu tek başına yetmez, eksik `VOLT-TEST-END` de bakılır.
pub(super) fn reconcile(
    module: &str,
    tests: &[String],
    (stdout, stderr): (&str, &str),
    code: Option<i32>,
) -> GroupRun {
    let mut outcomes = parse_tb_output(stdout);
    let not_run: Vec<String> = tests
        .iter()
        .filter(|t| !outcomes.iter().any(|o| o.name == **t))
        .cloned()
        .collect();
    let interrupted = outcomes.iter().any(|o| o.stop.is_some());
    let failed = outcomes.iter().any(|o| !o.passed);
    let expected = i32::from(failed);
    if !interrupted && not_run.is_empty() && code == Some(expected) {
        return GroupRun {
            outcomes,
            not_run,
            stop: None,
        };
    }
    let stop = SimStop {
        code,
        messages: sim_messages(stdout, stderr),
    };
    if let Some(o) = outcomes.iter_mut().find(|o| o.stop.is_some()) {
        o.stop = Some(stop);
        return GroupRun {
            outcomes,
            not_run,
            stop: None,
        };
    }
    let after = outcomes.last().map(|o| o.name.clone());
    GroupRun {
        outcomes,
        not_run,
        stop: Some(GroupStop {
            module: module.to_string(),
            after,
            stop,
        }),
    }
}

/// Simülatörün iletileri: son `VOLT-TEST-BEGIN`'den (yoksa baştan)
/// sonraki protokol dışı stdout satırları, ardından stderr.
fn sim_messages(stdout: &str, stderr: &str) -> Vec<String> {
    let lines: Vec<&str> = stdout.lines().collect();
    // Başlangıç satırının kendisi de alınır; protokol satırı olarak elenir.
    let start = lines
        .iter()
        .rposition(|l| l.starts_with("VOLT-TEST-BEGIN "))
        .unwrap_or(0);
    lines[start..]
        .iter()
        .copied()
        .chain(stderr.lines())
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty() && !l.starts_with("VOLT-"))
        .take(MAX_SIM_MESSAGES)
        .map(str::to_string)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AssertFailure {
    pub kind: String,
    pub loc: String,
    pub left: u64,
    pub right: u64,
    /// `loop=i=3,j=1` — hata `for` içindeyse sayaç değerleri (ADR-0058).
    pub loop_ctx: Option<String>,
    /// `port=addr:u3` — port genişlik hatasında (ad, tip) (ADR-0059).
    pub port: Option<(String, String)>,
    /// Enum değerli karşılaştırmada varyant adları (ADR-0074); testbench
    /// çıktısında yok, sürücü iddianın konumundan ekler.
    pub labels: Option<crate::sim_lower::ValueLabels>,
}

/// Testbench stdout'unu sonuçlara çevirir (sv-emit VOLT-* protokolü).
/// `VOLT-TEST-BEGIN`'i basılıp `VOLT-TEST-END`'i basılmayan test (süreç
/// testin ortasında bitti) kalmış bir sonuç olarak en sona eklenir.
pub(super) fn parse_tb_output(out: &str) -> Vec<TestOutcome> {
    let mut results = Vec::new();
    let mut pending_fail: Option<AssertFailure> = None;
    let mut pending_contract: Option<ContractViolation> = None;
    let mut begun: Option<&str> = None;
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("VOLT-ASSERT-FAIL ") {
            pending_fail = parse_assert_fail(rest);
        } else if let Some(rest) = line.strip_prefix("VOLT-CONTRACT-FAIL ") {
            pending_contract = parse_contract_fail(rest);
        } else if let Some(name) = line.strip_prefix("VOLT-TEST-BEGIN ") {
            begun = Some(name);
        } else if let Some(rest) = line.strip_prefix("VOLT-TEST-END ") {
            let (name, status) = match rest.rsplit_once(' ') {
                Some(pair) => pair,
                None => continue,
            };
            begun = None;
            results.push(TestOutcome {
                name: name.to_string(),
                passed: status == "ok",
                failure: pending_fail.take(),
                contract: pending_contract.take(),
                waveform: None,
                stop: None,
            });
        }
    }
    if let Some(name) = begun {
        results.push(TestOutcome {
            name: name.to_string(),
            passed: false,
            failure: pending_fail.take(),
            contract: pending_contract.take(),
            waveform: None,
            stop: Some(SimStop {
                code: None,
                messages: Vec::new(),
            }),
        });
    }
    results
}

/// `assert_eq dosya.volt:24 left=1 right=0` → AssertFailure.
pub(super) fn parse_assert_fail(rest: &str) -> Option<AssertFailure> {
    let mut parts = rest.split_whitespace();
    let kind = parts.next()?.to_string();
    let loc = parts.next()?.to_string();
    let left = parts.next()?.strip_prefix("left=")?.parse().ok()?;
    let right = parts.next()?.strip_prefix("right=")?.parse().ok()?;
    let (mut loop_ctx, mut port) = (None, None);
    for extra in parts {
        if let Some(vars) = extra.strip_prefix("loop=") {
            loop_ctx = Some(vars.replace('=', " = ").replace(',', ", "));
        } else if let Some((name, ty)) = extra.strip_prefix("port=").and_then(|p| p.split_once(':'))
        {
            port = Some((name.to_string(), ty.to_string()));
        }
    }
    Some(AssertFailure {
        kind,
        loc,
        left,
        right,
        loop_ctx,
        port,
        labels: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tb_output_reads_ok_and_fail() {
        let out = "VOLT-TEST-BEGIN a\nVOLT-TEST-END a ok\n\
                   VOLT-TEST-BEGIN b\n\
                   VOLT-ASSERT-FAIL assert_eq uart_tx_test.volt:24 left=1 right=0\n\
                   VOLT-TEST-END b fail\n";
        let results = parse_tb_output(out);
        assert_eq!(results.len(), 2);
        assert!(results[0].passed);
        assert!(results[0].failure.is_none());
        assert!(!results[1].passed);
        let f = results[1].failure.as_ref().expect("hata ayrıntısı");
        assert_eq!(f.kind, "assert_eq");
        assert_eq!(f.loc, "uart_tx_test.volt:24");
        assert_eq!((f.left, f.right), (1, 0));
    }

    #[test]
    fn parse_assert_fail_reads_port_and_loop_context() {
        let f = parse_assert_fail(
            "port_overflow t_test.volt:14 left=8 right=3 loop=i=8,j=1 port=addr:u3",
        )
        .expect("ayrışmalı");
        assert_eq!(f.kind, "port_overflow");
        assert_eq!((f.left, f.right), (8, 3));
        assert_eq!(f.loop_ctx.as_deref(), Some("i = 8, j = 1"));
        assert_eq!(f.port, Some(("addr".to_string(), "u3".to_string())));
    }

    #[test]
    fn parse_tb_output_attaches_contract_violation_to_its_test() {
        let out = "VOLT-TEST-BEGIN a\n\
                   VOLT-CONTRACT-FAIL Cnt.inv_0 cycle=6 inst=TOP.Cnt\n\
                   VOLT-TEST-END a fail\n\
                   VOLT-TEST-BEGIN b\nVOLT-TEST-END b ok\nVOLT-COVER Cnt.cov_0 3\n";
        let results = parse_tb_output(out);
        assert_eq!(results.len(), 2);
        let v = results[0].contract.as_ref().expect("ihlal");
        assert_eq!((v.id.as_str(), v.cycle), ("Cnt.inv_0", 6));
        assert!(results[0].failure.is_none());
        assert!(results[1].contract.is_none());
    }

    #[test]
    fn parse_tb_output_ignores_noise_lines() {
        let out = "some verilator chatter\nVOLT-TEST-END only ok\n";
        let results = parse_tb_output(out);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "only");
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| n.to_string()).collect()
    }

    /// Verilator 5.054'ün `$fatal`'da bastığı çıktı (#377): ikinci test
    /// ortada kesilir, süreç 1 ile çıkar, üçüncü test hiç başlamaz.
    const FATAL_RUN: &str = "VOLT-TEST-BEGIN first_ok\nVOLT-TEST-END first_ok ok\n\
                             VOLT-TEST-BEGIN trips_checker\n\
                             [4] %Fatal: extern_chk.sv:2: Assertion failed in TOP.M.c: value out of range\n\
                             %Error: extern_chk.sv:2: Verilog $stop\nAborting...\n";

    #[test]
    fn parse_tb_output_keeps_a_test_that_began_but_never_ended_as_failed() {
        let results = parse_tb_output(FATAL_RUN);
        assert_eq!(results.len(), 2);
        assert!(results[0].passed && results[0].stop.is_none());
        assert_eq!(results[1].name, "trips_checker");
        assert!(!results[1].passed);
        assert!(results[1].stop.is_some());
    }

    #[test]
    fn reconcile_fails_the_interrupted_test_and_lists_the_tests_that_never_ran() {
        let tests = names(&["first_ok", "trips_checker", "never_runs"]);
        let run = reconcile("M", &tests, (FATAL_RUN, ""), Some(1));
        assert_eq!(run.outcomes.len(), 2);
        assert!(run.outcomes[0].passed);
        let stopped = &run.outcomes[1];
        assert!(!stopped.passed);
        let stop = stopped.stop.as_ref().expect("duruş");
        assert_eq!(stop.code, Some(1));
        assert_eq!(
            stop.messages,
            [
                "[4] %Fatal: extern_chk.sv:2: Assertion failed in TOP.M.c: value out of range",
                "%Error: extern_chk.sv:2: Verilog $stop",
                "Aborting...",
            ]
        );
        assert_eq!(run.not_run, ["never_runs"]);
        assert!(run.stop.is_none(), "duruş teste bağlandı");
    }

    #[test]
    fn reconcile_leaves_a_normal_run_unchanged() {
        // Hepsi geçti, çıkış 0; biri kaldı, çıkış 1 (testbench'in sözleşmesi).
        let ok = "VOLT-TEST-BEGIN a\nVOLT-TEST-END a ok\nVOLT-TEST-BEGIN b\nVOLT-TEST-END b ok\n";
        let run = reconcile("M", &names(&["a", "b"]), (ok, ""), Some(0));
        assert_eq!(run.outcomes, parse_tb_output(ok));
        assert!(run.not_run.is_empty() && run.stop.is_none());
        let failing = "VOLT-TEST-BEGIN a\n\
                       VOLT-ASSERT-FAIL assert_eq t.volt:3 left=1 right=2\n\
                       VOLT-TEST-END a fail\nVOLT-TEST-BEGIN b\nVOLT-TEST-END b ok\n";
        let run = reconcile("M", &names(&["a", "b"]), (failing, ""), Some(1));
        assert_eq!(run.outcomes, parse_tb_output(failing));
        assert!(run.not_run.is_empty() && run.stop.is_none());
    }

    #[test]
    fn reconcile_reports_an_unexpected_exit_after_the_last_test_as_a_group_stop() {
        let ok = "VOLT-TEST-BEGIN a\nVOLT-TEST-END a ok\n";
        // Hepsi geçti ama süreç 1 ile (ya da sinyalle) çıktı.
        for code in [Some(1), Some(134), None] {
            let run = reconcile("M", &names(&["a"]), (ok, "boom\n"), code);
            assert!(run.outcomes[0].passed, "{code:?}");
            let s = run.stop.expect("grup duruşu");
            assert_eq!(s.module, "M");
            assert_eq!(s.after.as_deref(), Some("a"));
            assert_eq!(s.stop.code, code);
            assert_eq!(s.stop.messages, ["boom"]);
        }
        // Kalan test var ama süreç 0 ile çıktı: testbench 1 vermeliydi.
        let failing = "VOLT-TEST-BEGIN a\nVOLT-TEST-END a fail\n";
        let run = reconcile("M", &names(&["a"]), (failing, ""), Some(0));
        assert!(run.stop.is_some());
    }

    #[test]
    fn reconcile_reports_a_run_that_ended_between_tests_or_before_the_first() {
        // Süreç iki test arasında 0 ile bitti: kalan test "not run".
        let between = "VOLT-TEST-BEGIN a\nVOLT-TEST-END a ok\n";
        let run = reconcile("M", &names(&["a", "b"]), (between, ""), Some(0));
        assert_eq!(run.not_run, ["b"]);
        assert_eq!(run.stop.expect("duruş").after.as_deref(), Some("a"));
        // Hiçbir test başlamadı.
        let run = reconcile("M", &names(&["a"]), ("", "segfault\n"), None);
        assert_eq!(run.not_run, ["a"]);
        let s = run.stop.expect("duruş");
        assert_eq!(s.after, None);
        assert_eq!(s.stop.messages, ["segfault"]);
    }

    #[test]
    fn sim_messages_start_after_the_last_test_begin_and_are_capped() {
        let noisy: String = (0..30).map(|i| format!("line {i}\n")).collect();
        let out = format!("old chatter\nVOLT-TEST-BEGIN a\n{noisy}");
        let msgs = sim_messages(&out, "");
        assert_eq!(msgs.len(), MAX_SIM_MESSAGES);
        assert_eq!(msgs[0], "line 0");
        // Başlangıç satırı yoksa baştan; boş satırlar ve protokol satırları atlanır.
        assert_eq!(
            sim_messages("a\n\nVOLT-COVER x 1\n", "  \nb  \n"),
            ["a", "b"]
        );
    }

    #[test]
    fn parse_assert_fail_rejects_malformed() {
        assert!(parse_assert_fail("assert_eq file:1 left=x right=0").is_none());
        assert!(parse_assert_fail("assert_eq").is_none());
    }
}
