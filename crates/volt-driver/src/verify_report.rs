//! `volt verify` rapor katmanı (ADR-0055): ilerleme satırı, özet,
//! başarısızlık listesi ve JSON zarfının `verify` nesnesi.
//!
//! Rapor her zaman KAYNAK SIRASINDADIR; yalnız ilerleme satırları
//! tamamlanma sırasında akar (sayaç `[ k/N]` tamamlanan sayısıdır).
//! Süreler yalnız insan çıktısında ve `duration_ms` alanlarında görünür;
//! determinizm testleri bunları maskeler.

use std::time::Duration;

use volt_diagnostics::lstr;
use volt_sv_emit::SbyOptions;

use crate::verify::SbyOutcome;
use crate::verify_jobs::TaskStatus;

/// Bir kontratın rapordaki kimliği.
#[derive(Debug, Clone)]
pub(crate) struct PropInfo {
    pub(crate) name: String,
    pub(crate) keyword: &'static str,
}

/// Bir modül görevinin raporu (kaynak sırasında toplanır).
#[derive(Debug, Clone)]
pub(crate) struct ModuleOutcome {
    pub(crate) module: String,
    pub(crate) task: String,
    pub(crate) props: Vec<PropInfo>,
    pub(crate) status: TaskStatus,
    /// FAIL'de karşı örneğe eşlenen kontrat adı (`inv_0`).
    pub(crate) failed_prop: Option<String>,
}

impl ModuleOutcome {
    fn is_fail(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Done {
                outcome: SbyOutcome::Fail(_),
                ..
            }
        )
    }

    fn is_error(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Done {
                outcome: SbyOutcome::Error,
                ..
            } | TaskStatus::Missing
        )
    }

    /// Kontratın JSON durumu: `pass | fail | unproven | skipped | error`.
    fn prop_status(&self, prop: &PropInfo) -> &'static str {
        match &self.status {
            TaskStatus::Done {
                outcome: SbyOutcome::Pass,
                ..
            } => "pass",
            TaskStatus::Done {
                outcome: SbyOutcome::Fail(_),
                ..
            } => {
                if self.failed_prop.as_deref() == Some(prop.name.as_str()) {
                    "fail"
                } else {
                    // BMC ilk ihlalde durur: aynı modülün diğer kontratları
                    // o döngüden sonra denetlenmedi.
                    "unproven"
                }
            }
            TaskStatus::Done {
                outcome: SbyOutcome::Error,
                ..
            }
            | TaskStatus::Missing => "error",
            TaskStatus::Skipped => "skipped",
        }
    }

    fn module_status(&self) -> &'static str {
        match &self.status {
            TaskStatus::Done {
                outcome: SbyOutcome::Pass,
                ..
            } => "pass",
            TaskStatus::Done {
                outcome: SbyOutcome::Fail(_),
                ..
            } => "fail",
            TaskStatus::Done {
                outcome: SbyOutcome::Error,
                ..
            }
            | TaskStatus::Missing => "error",
            TaskStatus::Skipped => "skipped",
        }
    }

    fn duration_ms(&self) -> Option<u64> {
        match &self.status {
            TaskStatus::Done { elapsed, .. } => Some(elapsed.as_millis() as u64),
            _ => None,
        }
    }
}

fn secs(d: Duration) -> String {
    format!("{:.2}s", d.as_secs_f64())
}

/// `[ 3/8] SocTop (20 properties) ... ok (9.12s)` — tamamlanma anında
/// basılır; sayaç bitmiş görev sayısıdır.
pub(crate) fn progress_line(
    done: usize,
    total: usize,
    module: &str,
    props: usize,
    status: &TaskStatus,
) -> String {
    let width = total.to_string().len();
    let verdict = match status {
        TaskStatus::Done {
            outcome: SbyOutcome::Pass,
            elapsed,
        } => lstr!(en: "ok ({})", secs(*elapsed); tr: "tamam ({})", secs(*elapsed)),
        TaskStatus::Done {
            outcome: SbyOutcome::Fail(_),
            elapsed,
        } => lstr!(en: "FAIL ({})", secs(*elapsed); tr: "İHLAL ({})", secs(*elapsed)),
        TaskStatus::Done {
            outcome: SbyOutcome::Error,
            elapsed,
        } => lstr!(en: "error ({})", secs(*elapsed); tr: "hata ({})", secs(*elapsed)),
        TaskStatus::Missing => lstr!(en: "error"; tr: "hata"),
        TaskStatus::Skipped => lstr!(en: "skipped"; tr: "atlandı"),
    };
    lstr!(
        en: "     [{done:>width$}/{total}] {module} ({props} propert{}) ... {verdict}",
            if props == 1 { "y" } else { "ies" };
        tr: "     [{done:>width$}/{total}] {module} ({props} özellik) ... {verdict}"
    )
}

/// Koşu sonu özeti (insan çıktısı). Başarıda tek satır, aksi hâlde
/// kaynak sırasında başarısızlık listesi + atlanan modül sayısı.
pub(crate) fn summary_block(
    outcomes: &[ModuleOutcome],
    opts: &SbyOptions,
    jobs: usize,
    total: Duration,
) -> String {
    let props: usize = outcomes.iter().map(|m| m.props.len()).sum();
    let fails: Vec<&ModuleOutcome> = outcomes.iter().filter(|m| m.is_fail()).collect();
    let errors: Vec<&ModuleOutcome> = outcomes.iter().filter(|m| m.is_error()).collect();
    let skipped = outcomes
        .iter()
        .filter(|m| m.status == TaskStatus::Skipped)
        .count();
    let elapsed = format!("{:.1}s", total.as_secs_f64());

    if fails.is_empty() && errors.is_empty() && skipped == 0 {
        return lstr!(
            en: "      Result {props} propert{} verified in {elapsed} ({jobs} job{}; {}, depth {})",
                if props == 1 { "y" } else { "ies" },
                if jobs == 1 { "" } else { "s" },
                opts.mode.as_str(), opts.depth;
            tr: "       Sonuç {props} özellik {elapsed} içinde doğrulandı ({jobs} iş; {}, derinlik {})",
                opts.mode.as_str(), opts.depth
        );
    }

    let mut out = String::new();
    if !fails.is_empty() || !errors.is_empty() {
        out.push_str(&lstr!(en: "    Failures:\n"; tr: "  Başarısız:\n"));
    }
    for m in outcomes {
        if m.is_fail() {
            let prop = m.failed_prop.clone().unwrap_or_else(|| "?".into());
            let step = match &m.status {
                TaskStatus::Done {
                    outcome: SbyOutcome::Fail(f),
                    ..
                } => f.step,
                _ => None,
            };
            let at = match step {
                Some(s) => lstr!(en: " at cycle {s}"; tr: " {s}. döngüde"),
                None => String::new(),
            };
            out.push_str(&lstr!(
                en: "      {}.{prop}  E5001 contract violated{at}\n", m.module;
                tr: "      {}.{prop}  E5001 kontrat ihlal edildi{at}\n", m.module
            ));
        } else if m.is_error() {
            out.push_str(&lstr!(
                en: "      {}  tool error (see 'sby -f' hint above)\n", m.module;
                tr: "      {}  araç hatası (yukarıdaki 'sby -f' ipucuna bakın)\n", m.module
            ));
        }
    }
    if skipped > 0 {
        out.push_str(&lstr!(
            en: "      {skipped} module task(s) skipped (--fail-fast)\n";
            tr: "      {skipped} modül görevi atlandı (--fail-fast)\n"
        ));
    }
    let failed_props = fails.len();
    out.push_str(&lstr!(
        en: "      Result {failed_props} of {props} properties failed in {elapsed} ({jobs} job{}; {}, depth {})",
            if jobs == 1 { "" } else { "s" }, opts.mode.as_str(), opts.depth;
        tr: "       Sonuç {props} özellikten {failed_props} tanesi başarısız, {elapsed} ({jobs} iş; {}, derinlik {})",
            opts.mode.as_str(), opts.depth
    ));
    out
}

/// JSON zarfının `verify` nesnesi (cli-contract.md §8a). `properties`
/// düz liste, `modules` görev başına; her ikisi kaynak sırasında.
/// Kontratın `duration_ms`'i ait olduğu modül görevinin süresidir —
/// tüm kontratlar tek BMC koşusunda birlikte denetlenir.
pub(crate) fn verify_json(
    outcomes: &[ModuleOutcome],
    opts: &SbyOptions,
    jobs: usize,
    fail_fast: bool,
) -> serde_json::Value {
    let modules: Vec<serde_json::Value> = outcomes
        .iter()
        .map(|m| {
            serde_json::json!({
                "module": m.module,
                "task": m.task,
                "status": m.module_status(),
                "properties": m.props.len(),
                "duration_ms": m.duration_ms(),
            })
        })
        .collect();
    let properties: Vec<serde_json::Value> = outcomes
        .iter()
        .flat_map(|m| {
            m.props.iter().map(move |p| {
                serde_json::json!({
                    "module": m.module,
                    "name": p.name,
                    "keyword": p.keyword,
                    "status": m.prop_status(p),
                    "duration_ms": m.duration_ms(),
                })
            })
        })
        .collect();
    serde_json::json!({
        "mode": opts.mode.as_str(),
        "depth": opts.depth,
        "engine": opts.engine.as_str(),
        "jobs": jobs,
        "fail_fast": fail_fast,
        "modules": modules,
        "properties": properties,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::SbyFailure;

    fn outcome(
        module: &str,
        props: &[&str],
        status: TaskStatus,
        failed: Option<&str>,
    ) -> ModuleOutcome {
        ModuleOutcome {
            module: module.to_string(),
            task: module.to_lowercase(),
            props: props
                .iter()
                .map(|n| PropInfo {
                    name: n.to_string(),
                    keyword: "invariant",
                })
                .collect(),
            status,
            failed_prop: failed.map(str::to_string),
        }
    }

    fn pass() -> TaskStatus {
        TaskStatus::Done {
            outcome: SbyOutcome::Pass,
            elapsed: Duration::from_millis(1234),
        }
    }

    fn fail(step: Option<u32>) -> TaskStatus {
        TaskStatus::Done {
            outcome: SbyOutcome::Fail(SbyFailure {
                sv_line: None,
                step,
            }),
            elapsed: Duration::from_millis(500),
        }
    }

    #[test]
    fn progress_line_pads_counter_to_total_width() {
        let line = progress_line(3, 137, "SocTop", 20, &pass());
        assert!(
            line.contains("[  3/137] SocTop (20 properties) ... ok (1.23s)"),
            "{line}"
        );
        let one = progress_line(1, 1, "Counter", 1, &pass());
        assert!(one.contains("[1/1] Counter (1 property) ... ok"), "{one}");
    }

    #[test]
    fn progress_line_marks_fail_skip_and_error() {
        assert!(progress_line(1, 2, "M", 1, &fail(None)).contains("... FAIL (0.50s)"));
        assert!(progress_line(1, 2, "M", 1, &TaskStatus::Skipped).ends_with("... skipped"));
        assert!(progress_line(1, 2, "M", 1, &TaskStatus::Missing).ends_with("... error"));
    }

    #[test]
    fn json_property_statuses_follow_module_outcome() {
        let outcomes = [
            outcome("Alpha", &["inv_0"], pass(), None),
            outcome("Beta", &["inv_0", "inv_1"], fail(Some(7)), Some("inv_1")),
            outcome("Gamma", &["inv_0"], TaskStatus::Skipped, None),
            outcome("Delta", &["inv_0"], TaskStatus::Missing, None),
        ];
        let v = verify_json(&outcomes, &SbyOptions::default(), 4, true);
        assert_eq!(v["jobs"], 4);
        assert_eq!(v["fail_fast"], true);
        assert_eq!(v["mode"], "bmc");
        let props = v["properties"].as_array().unwrap();
        let statuses: Vec<&str> = props
            .iter()
            .map(|p| p["status"].as_str().unwrap())
            .collect();
        assert_eq!(statuses, ["pass", "unproven", "fail", "skipped", "error"]);
        assert_eq!(props[0]["duration_ms"], 1234);
        assert_eq!(props[2]["duration_ms"], 500);
        assert!(props[3]["duration_ms"].is_null());
        let mods = v["modules"].as_array().unwrap();
        assert_eq!(mods.len(), 4);
        assert_eq!(mods[1]["status"], "fail");
        assert_eq!(mods[1]["properties"], 2);
        assert_eq!(mods[2]["status"], "skipped");
        assert_eq!(mods[3]["status"], "error");
    }

    #[test]
    fn summary_success_counts_properties_and_jobs() {
        let outcomes = [
            outcome("Alpha", &["inv_0", "cov_0"], pass(), None),
            outcome("Beta", &["inv_0"], pass(), None),
        ];
        let text = summary_block(
            &outcomes,
            &SbyOptions::default(),
            8,
            Duration::from_millis(14_200),
        );
        assert!(
            text.contains("3 properties verified in 14.2s (8 jobs; bmc, depth 20)"),
            "{text}"
        );
        let one = summary_block(
            &outcomes[1..],
            &SbyOptions::default(),
            1,
            Duration::from_secs(1),
        );
        assert!(
            one.contains("1 property verified in 1.0s (1 job; bmc, depth 20)"),
            "{one}"
        );
    }

    #[test]
    fn summary_failure_lists_source_order_and_skipped_count() {
        let outcomes = [
            outcome("Alpha", &["inv_0"], pass(), None),
            outcome("Beta", &["inv_0"], fail(Some(7)), Some("inv_0")),
            outcome("Gamma", &["inv_0"], TaskStatus::Skipped, None),
            outcome("Delta", &["inv_0"], TaskStatus::Missing, None),
        ];
        let text = summary_block(&outcomes, &SbyOptions::default(), 4, Duration::from_secs(2));
        let beta = text
            .find("Beta.inv_0  E5001 contract violated at cycle 7")
            .expect("Beta");
        let delta = text.find("Delta  tool error").expect("Delta");
        assert!(beta < delta, "{text}");
        assert!(
            text.contains("1 module task(s) skipped (--fail-fast)"),
            "{text}"
        );
        assert!(
            text.contains("Result 1 of 4 properties failed in 2.0s (4 jobs; bmc, depth 20)"),
            "{text}"
        );
    }
}
