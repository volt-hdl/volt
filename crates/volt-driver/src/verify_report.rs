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
use crate::verify_depth::AutoUnreached;
use crate::verify_jobs::TaskStatus;

/// Bir kontratın rapordaki kimliği.
#[derive(Debug, Clone)]
pub(crate) struct PropInfo {
    /// Kontratın sahibi modül — görevin tepesi ya da (ADR-0097) altındaki
    /// bir örneğin modülü.
    pub(crate) module: String,
    pub(crate) name: String,
    pub(crate) keyword: &'static str,
    /// Bu görevde yalnız VARSAYILDI (görev tepesinin kendi requires/
    /// assume'u): doğrulanmış sayılmaz, durumu `assumed`.
    pub(crate) assumed: bool,
}

/// Bir modül görevinin raporu (kaynak sırasında toplanır).
#[derive(Debug, Clone)]
pub(crate) struct ModuleOutcome {
    pub(crate) module: String,
    pub(crate) task: String,
    pub(crate) props: Vec<PropInfo>,
    pub(crate) status: TaskStatus,
    /// FAIL'de karşı örneğe, UNKNOWN'da tümevarım izine eşlenen kontrat:
    /// (sahibi modül, ad) — `("Child", "req_0")`.
    pub(crate) failed_prop: Option<(String, String)>,
    /// Cover kipinde ulaşılmayan, ölü olduğu kanıtlanmamış otomatik
    /// cover'lar (ADR-0086) — başarısızlık değil.
    pub(crate) auto_unreached: Vec<AutoUnreached>,
}

impl ModuleOutcome {
    /// Bu görevde denetlenen özellik sayısı (varsayımlar hariç).
    fn checked(&self) -> usize {
        self.props.iter().filter(|p| !p.assumed).count()
    }

    fn assumed(&self) -> usize {
        self.props.iter().filter(|p| p.assumed).count()
    }

    fn is_fail(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Done {
                outcome: SbyOutcome::Fail(_),
                ..
            }
        )
    }

    fn is_unknown(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Done {
                outcome: SbyOutcome::Unknown(_),
                ..
            }
        )
    }

    fn is_timeout(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Done {
                outcome: SbyOutcome::Timeout,
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

    /// Kontratın JSON durumu: `pass | fail | unknown | unproven | timeout |
    /// skipped | error`.
    fn prop_status(&self, prop: &PropInfo) -> &'static str {
        if prop.assumed {
            return "assumed";
        }
        if let Some(n) = self.auto_unreached_of(prop) {
            return if n.min_depth.is_some() {
                "needs-depth"
            } else {
                "not-reached"
            };
        }
        let named = self
            .failed_prop
            .as_ref()
            .is_some_and(|(m, n)| *m == prop.module && *n == prop.name);
        match &self.status {
            TaskStatus::Done {
                outcome: SbyOutcome::Pass,
                ..
            } => "pass",
            // BMC ilk ihlalde durur: aynı modülün diğer kontratları o
            // döngüden sonra denetlenmedi. Tümevarım da bütün kontratları
            // birlikte kanıtlar: biri tümevarımsal değilse diğerleri de
            // kanıtlanmış sayılmaz.
            TaskStatus::Done {
                outcome: SbyOutcome::Fail(_),
                ..
            } => {
                if named {
                    "fail"
                } else {
                    "unproven"
                }
            }
            TaskStatus::Done {
                outcome: SbyOutcome::Unknown(_),
                ..
            } => {
                if named {
                    "unknown"
                } else {
                    "unproven"
                }
            }
            TaskStatus::Done {
                outcome: SbyOutcome::Timeout,
                ..
            } => "timeout",
            TaskStatus::Done {
                outcome: SbyOutcome::Error,
                ..
            }
            | TaskStatus::Missing => "error",
            TaskStatus::Skipped => "skipped",
        }
    }

    /// Kontrat ulaşılmayan (kanıtsız) otomatik cover mı?
    fn auto_unreached_of(&self, prop: &PropInfo) -> Option<&AutoUnreached> {
        if prop.module != self.module {
            return None;
        }
        self.auto_unreached.iter().find(|d| d.prop == prop.name)
    }

    /// Başarısız kontratın rapordaki adı (`Child.req_0`) ve bağlamı:
    /// sahibi görevin tepesi değilse görev eklenir (ADR-0097).
    fn failed_label(&self) -> (String, String) {
        match &self.failed_prop {
            Some((m, n)) if *m != self.module => (
                format!("{m}.{n}"),
                lstr!(en: " (in {})", self.module; tr: " ({} içinde)", self.module),
            ),
            Some((m, n)) => (format!("{m}.{n}"), String::new()),
            None => (format!("{}.?", self.module), String::new()),
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
                outcome: SbyOutcome::Unknown(_),
                ..
            } => "unknown",
            TaskStatus::Done {
                outcome: SbyOutcome::Timeout,
                ..
            } => "timeout",
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
/// basılır; sayaç bitmiş görev sayısıdır. `props` görevde DENETLENEN
/// özellikler; varsayımlar varsa ayrıca yazılır (ADR-0097).
pub(crate) fn progress_line(
    done: usize,
    total: usize,
    module: &str,
    (props, assumed): (usize, usize),
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
            outcome: SbyOutcome::Unknown(_),
            elapsed,
        } => lstr!(en: "unknown ({})", secs(*elapsed); tr: "kanıtlanamadı ({})", secs(*elapsed)),
        TaskStatus::Done {
            outcome: SbyOutcome::Timeout,
            elapsed,
        } => lstr!(en: "timeout ({})", secs(*elapsed); tr: "zaman aşımı ({})", secs(*elapsed)),
        TaskStatus::Done {
            outcome: SbyOutcome::Error,
            elapsed,
        } => lstr!(en: "error ({})", secs(*elapsed); tr: "hata ({})", secs(*elapsed)),
        TaskStatus::Missing => lstr!(en: "error"; tr: "hata"),
        TaskStatus::Skipped => lstr!(en: "skipped"; tr: "atlandı"),
    };
    let assumed = if assumed == 0 {
        String::new()
    } else {
        lstr!(en: ", {assumed} assumed"; tr: ", {assumed} varsayım")
    };
    lstr!(
        en: "     [{done:>width$}/{total}] {module} ({props} propert{}{assumed}) ... {verdict}",
            if props == 1 { "y" } else { "ies" };
        tr: "     [{done:>width$}/{total}] {module} ({props} özellik{assumed}) ... {verdict}"
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
    let props: usize = outcomes.iter().map(ModuleOutcome::checked).sum();
    let fails = outcomes.iter().filter(|m| m.is_fail()).count();
    let unknowns = outcomes.iter().filter(|m| m.is_unknown()).count();
    let timeouts = outcomes.iter().filter(|m| m.is_timeout()).count();
    let errors = outcomes.iter().filter(|m| m.is_error()).count();
    let skipped = outcomes
        .iter()
        .filter(|m| m.status == TaskStatus::Skipped)
        .count();
    let elapsed = format!("{:.1}s", total.as_secs_f64());
    // ADR-0086: ulaşılmayan (ölü olduğu kanıtlanmamış) otomatik cover'lar
    // doğrulanmış sayılmaz, başarısız da değildir.
    let deep: usize = outcomes.iter().map(|m| m.auto_unreached.len()).sum();
    let deep_note = if deep == 0 {
        String::new()
    } else {
        lstr!(en: ", {deep} auto cover(s) not reached at this depth"; tr: ", {deep} otomatik cover bu derinlikte ulaşılmadı")
    };

    if fails + unknowns + timeouts + errors + skipped == 0 {
        let verified = props - deep;
        if deep == 0 {
            return lstr!(
                en: "      Result {props} propert{} verified in {elapsed} ({jobs} job{}; {}, depth {})",
                    if props == 1 { "y" } else { "ies" },
                    if jobs == 1 { "" } else { "s" },
                    opts.mode.as_str(), opts.depth;
                tr: "       Sonuç {props} özellik {elapsed} içinde doğrulandı ({jobs} iş; {}, derinlik {})",
                    opts.mode.as_str(), opts.depth
            );
        }
        return lstr!(
            en: "      Result {verified} of {props} properties verified{deep_note} in {elapsed} ({jobs} job{}; {}, depth {})",
                if jobs == 1 { "" } else { "s" },
                opts.mode.as_str(), opts.depth;
            tr: "       Sonuç {props} özellikten {verified} tanesi doğrulandı{deep_note}, {elapsed} ({jobs} iş; {}, derinlik {})",
                opts.mode.as_str(), opts.depth
        );
    }

    let mut out = String::new();
    if fails + unknowns + timeouts + errors > 0 {
        out.push_str(&lstr!(en: "    Failures:\n"; tr: "  Başarısız:\n"));
    }
    for m in outcomes {
        if m.is_fail() {
            let (prop, ctx) = m.failed_label();
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
                en: "      {prop}  E5001 contract violated{at}{ctx}\n";
                tr: "      {prop}  E5001 kontrat ihlal edildi{at}{ctx}\n"
            ));
        } else if m.is_unknown() {
            let (prop, ctx) = m.failed_label();
            out.push_str(&lstr!(
                en: "      {prop}  E5002 not proven (induction step failed){ctx}\n";
                tr: "      {prop}  E5002 kanıtlanamadı (tümevarım adımı başarısız){ctx}\n"
            ));
        } else if m.is_timeout() {
            out.push_str(&lstr!(
                en: "      {}  timeout (no result; see --timeout)\n", m.module;
                tr: "      {}  zaman aşımı (sonuç yok; bkz. --timeout)\n", m.module
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
    let mut extra = deep_note;
    if unknowns > 0 {
        extra.push_str(
            &lstr!(en: ", {unknowns} not proven"; tr: ", {unknowns} tanesi kanıtlanamadı"),
        );
    }
    if timeouts > 0 {
        extra.push_str(&lstr!(
            en: ", {timeouts} module task(s) timed out";
            tr: ", {timeouts} modül görevi zaman aşımına uğradı"
        ));
    }
    out.push_str(&lstr!(
        en: "      Result {fails} of {props} properties failed{extra} in {elapsed} ({jobs} job{}; {}, depth {})",
            if jobs == 1 { "" } else { "s" }, opts.mode.as_str(), opts.depth;
        tr: "       Sonuç {props} özellikten {fails} tanesi başarısız{extra}, {elapsed} ({jobs} iş; {}, derinlik {})",
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
                "properties": m.checked(),
                "assumed": m.assumed(),
                "duration_ms": m.duration_ms(),
            })
        })
        .collect();
    let properties: Vec<serde_json::Value> = outcomes
        .iter()
        .flat_map(|m| {
            m.props.iter().map(move |p| {
                let mut prop = serde_json::json!({
                    "module": p.module,
                    "name": p.name,
                    "keyword": p.keyword,
                    "status": m.prop_status(p),
                    "duration_ms": m.duration_ms(),
                });
                if let Some(d) = m.auto_unreached_of(p).and_then(|n| n.min_depth) {
                    prop["min_depth"] = serde_json::json!(d);
                }
                // ADR-0097: örneğin yükümlülüğü üst modülün görevinde denetlenir.
                if p.module != m.module {
                    prop["context"] = serde_json::json!(m.module);
                }
                prop
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
                    module: module.to_string(),
                    name: n.to_string(),
                    keyword: "invariant",
                    assumed: false,
                })
                .collect(),
            status,
            failed_prop: failed.map(|n| (module.to_string(), n.to_string())),
            auto_unreached: Vec::new(),
        }
    }

    fn deep(module: &str, props: &[&str], status: TaskStatus, limited: &[&str]) -> ModuleOutcome {
        let mut m = outcome(module, props, status, None);
        m.auto_unreached = limited
            .iter()
            .map(|p| AutoUnreached {
                prop: p.to_string(),
                text: "c == 255".to_string(),
                subject: "wrap check on c".to_string(),
                min_depth: (*p != "cov_2").then_some(258),
            })
            .collect();
        m
    }

    #[test]
    fn depth_limited_cover_is_neither_verified_nor_failed() {
        // ADR-0086: 2 kontrat, biri derinliği yetmeyen otomatik cover.
        let outcomes = [deep(
            "C",
            &["inv_0", "cov_0", "cov_2"],
            pass(),
            &["cov_0", "cov_2"],
        )];
        let opts = SbyOptions {
            mode: volt_sv_emit::SbyMode::Cover,
            depth: 12,
            ..SbyOptions::default()
        };
        let text = summary_block(&outcomes, &opts, 1, Duration::from_millis(900));
        assert!(text.contains("1 of 3 properties verified"), "{text}");
        assert!(
            text.contains("2 auto cover(s) not reached at this depth"),
            "{text}"
        );
        assert!(!text.contains("Failures"), "{text}");
        let json = verify_json(&outcomes, &opts, 1, false);
        let props = json["properties"].as_array().expect("dizi");
        assert_eq!(props[0]["status"], "pass");
        assert_eq!(props[1]["status"], "needs-depth");
        assert_eq!(props[1]["min_depth"], 258);
        assert_eq!(props[2]["status"], "not-reached");
        assert!(props[0].get("min_depth").is_none() && props[2].get("min_depth").is_none());
    }

    #[test]
    fn depth_limited_cover_next_to_a_real_failure_is_counted_apart() {
        let outcomes = [deep("C", &["cov_0", "cov_1"], fail(Some(3)), &["cov_0"])];
        let opts = SbyOptions {
            mode: volt_sv_emit::SbyMode::Cover,
            depth: 12,
            ..SbyOptions::default()
        };
        let text = summary_block(&outcomes, &opts, 1, Duration::from_millis(900));
        assert!(text.contains("not reached at this depth"), "{text}");
        assert!(text.contains("Failures"), "{text}");
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
                unreached: Vec::new(),
            }),
            elapsed: Duration::from_millis(500),
        }
    }

    fn unknown() -> TaskStatus {
        TaskStatus::Done {
            outcome: SbyOutcome::Unknown(SbyFailure::default()),
            elapsed: Duration::from_millis(250),
        }
    }

    fn timeout() -> TaskStatus {
        TaskStatus::Done {
            outcome: SbyOutcome::Timeout,
            elapsed: Duration::from_millis(5000),
        }
    }

    /// ADR-0075: UNKNOWN ve TIMEOUT kendi sözcükleriyle — "error" değil.
    #[test]
    fn unknown_and_timeout_have_their_own_verdicts() {
        assert!(progress_line(1, 2, "M", (1, 0), &unknown()).ends_with("... unknown (0.25s)"));
        assert!(progress_line(1, 2, "M", (1, 0), &timeout()).ends_with("... timeout (5.00s)"));
        let outcomes = [
            outcome("Alpha", &["inv_0", "inv_1"], unknown(), Some("inv_1")),
            outcome("Beta", &["inv_0"], timeout(), None),
        ];
        let v = verify_json(&outcomes, &SbyOptions::default(), 2, false);
        let props: Vec<&str> = v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["status"].as_str().unwrap())
            .collect();
        assert_eq!(props, ["unproven", "unknown", "timeout"]);
        let text = summary_block(&outcomes, &SbyOptions::default(), 2, Duration::from_secs(6));
        assert!(text.contains("Alpha.inv_1  E5002 not proven"), "{text}");
        assert!(
            text.contains("Beta  timeout (no result; see --timeout)"),
            "{text}"
        );
        assert!(
            text.contains(
                "Result 0 of 3 properties failed, 1 not proven, 1 module task(s) timed out in 6.0s"
            ),
            "{text}"
        );
        assert!(!text.contains("tool error"), "{text}");
    }

    #[test]
    fn progress_line_pads_counter_to_total_width() {
        let line = progress_line(3, 137, "SocTop", (20, 0), &pass());
        assert!(
            line.contains("[  3/137] SocTop (20 properties) ... ok (1.23s)"),
            "{line}"
        );
        let one = progress_line(1, 1, "Counter", (1, 0), &pass());
        assert!(one.contains("[1/1] Counter (1 property) ... ok"), "{one}");
    }

    #[test]
    fn progress_line_marks_fail_skip_and_error() {
        assert!(progress_line(1, 2, "M", (1, 0), &fail(None)).contains("... FAIL (0.50s)"));
        assert!(progress_line(1, 2, "M", (1, 0), &TaskStatus::Skipped).ends_with("... skipped"));
        assert!(progress_line(1, 2, "M", (1, 0), &TaskStatus::Missing).ends_with("... error"));
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
