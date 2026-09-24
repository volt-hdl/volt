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
    pub labels: Option<crate::sim_lower::EnumLabels>,
}

/// Testbench stdout'unu sonuçlara çevirir (sv-emit VOLT-* protokolü).
pub(super) fn parse_tb_output(out: &str) -> Vec<TestOutcome> {
    let mut results = Vec::new();
    let mut pending_fail: Option<AssertFailure> = None;
    let mut pending_contract: Option<ContractViolation> = None;
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("VOLT-ASSERT-FAIL ") {
            pending_fail = parse_assert_fail(rest);
        } else if let Some(rest) = line.strip_prefix("VOLT-CONTRACT-FAIL ") {
            pending_contract = parse_contract_fail(rest);
        } else if let Some(rest) = line.strip_prefix("VOLT-TEST-END ") {
            let (name, status) = match rest.rsplit_once(' ') {
                Some(pair) => pair,
                None => continue,
            };
            results.push(TestOutcome {
                name: name.to_string(),
                passed: status == "ok",
                failure: pending_fail.take(),
                contract: pending_contract.take(),
            });
        }
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

    #[test]
    fn parse_assert_fail_rejects_malformed() {
        assert!(parse_assert_fail("assert_eq file:1 left=x right=0").is_none());
        assert!(parse_assert_fail("assert_eq").is_none());
    }
}
