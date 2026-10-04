//! Cover kipinde ulaşılamayan OTOMATİK cover'lar (ADR-0086).
//!
//! Sınırlı BMC'de "Unreached cover" iki şey olabilir: gerçekten ölü kod
//! ya da derinliğin yetmediği bir yol. Kullanıcı bu kontratı yazmadı; E5001
//! ("contract violated") yalnız kanıt varsa doğrudur. Kural (yanlış alarm,
//! eksik uyarıdan kötü):
//!
//! * `CoverReach::Never` — tanıyıcı resetten hiçbir yol bulamadı (FSM
//!   durum grafiği): gerçek bulgu, E5001 kalır (ADR-0066 fail/71).
//! * `MinDepth(d)`, `d > derinlik` — yapısal alt sınır derinliği aşıyor:
//!   "--depth ≥ d gerekir" notu.
//! * Diğerleri (sınır derinlikten küçük ama başka koşullar geciktiriyor,
//!   ya da sınır bilinmiyor) — "bu derinlikte ulaşılmadı" notu.
//!
//! Notlar başarısızlık değildir (çıkış 0). Kullanıcı cover'ları her zaman
//! E5001 kalır.

use volt_diagnostics::lstr;
use volt_sv_emit::{CoverReach, SvaProp};

use crate::verify::{prop_name_at, SbyFailure, SbyOutcome};

/// Bu derinlikte ulaşılmayan, ölü olduğu kanıtlanmamış otomatik cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutoUnreached {
    /// Property adı (`cov_0`).
    pub(crate) prop: String,
    /// Kontrat metni (`c == 255`).
    pub(crate) text: String,
    /// Kökenin anlatımı (`wrap check on c`).
    pub(crate) subject: String,
    /// Yapısal alt sınır derinliği aşıyorsa ulaşılabilecek en küçük
    /// derinlik; `None`: sınır yok ya da derinlikten küçük.
    pub(crate) min_depth: Option<u32>,
}

/// Ulaşılamayan cover satırlarını ikiye ayırır: gerçek başarısızlıklar
/// (sırası korunur) ve ölü olduğu kanıtlanmamış otomatik cover'lar.
pub(crate) fn split_unreached(
    module: &str,
    unreached: &[usize],
    sv: &str,
    props: &[SvaProp],
    depth: u32,
) -> (Vec<usize>, Vec<AutoUnreached>) {
    let mut real = Vec::new();
    let mut notes: Vec<AutoUnreached> = Vec::new();
    for &line in unreached {
        match auto_unreached_at(module, line, sv, props, depth) {
            Some(n) if !notes.contains(&n) => notes.push(n),
            Some(_) => {}
            None => real.push(line),
        }
    }
    (real, notes)
}

fn auto_unreached_at(
    module: &str,
    line: usize,
    sv: &str,
    props: &[SvaProp],
    depth: u32,
) -> Option<AutoUnreached> {
    let name = prop_name_at(sv, line)?;
    let prop = props
        .iter()
        .find(|p| p.module_name == module && p.name == name)?;
    let auto = prop.auto.as_ref()?;
    let min_depth = match auto.reach {
        CoverReach::Never => return None,
        CoverReach::MinDepth(d) => Some(d).filter(|&d| d > depth),
        CoverReach::Unknown => None,
    };
    Some(AutoUnreached {
        prop: prop.name.clone(),
        text: auto.text.clone(),
        subject: auto.subject.clone(),
        min_depth,
    })
}

/// Görev sonucunu düzeltir: ulaşılamayan cover'ların hepsi ölü olduğu
/// kanıtlanmamış otomatik cover'sa sonuç geçer; değilse başarısızlık ilk
/// GERÇEK ulaşılamayan cover'a işaret eder.
pub(crate) fn settle(
    outcome: SbyOutcome,
    module: &str,
    sv: &str,
    props: &[SvaProp],
    depth: u32,
) -> SbyOutcome {
    let SbyOutcome::Fail(failure) = outcome else {
        return outcome;
    };
    if failure.unreached.is_empty() {
        return SbyOutcome::Fail(failure);
    }
    let (real, notes) = split_unreached(module, &failure.unreached, sv, props, depth);
    if notes.is_empty() {
        return SbyOutcome::Fail(failure);
    }
    match real.first() {
        None => SbyOutcome::Pass,
        Some(&first) => SbyOutcome::Fail(SbyFailure {
            sv_line: Some(first),
            step: failure.step,
            unreached: real,
        }),
    }
}

/// İnsan çıktısındaki not satırı.
pub(crate) fn note_line(module: &str, n: &AutoUnreached, depth: u32) -> String {
    match n.min_depth {
        Some(d) => lstr!(
            en: "        Note {module}.{}: auto cover '{}' ({}) needs --depth {d} or more; \
                 unreachable at depth {depth} by construction, not a design error",
                n.prop, n.text, n.subject;
            tr: "         Not {module}.{}: otomatik cover '{}' ({}) en az --depth {d} ister; \
                 derinlik {depth}'de yapısal olarak ulaşılamaz, tasarım hatası değil",
                n.prop, n.text, n.subject
        ),
        None => lstr!(
            en: "        Note {module}.{}: auto cover '{}' ({}) not reached within depth {depth}; \
                 not proven unreachable, so not an error — try a deeper run or 'volt test'",
                n.prop, n.text, n.subject;
            tr: "         Not {module}.{}: otomatik cover '{}' ({}) derinlik {depth} içinde ulaşılmadı; \
                 ulaşılamaz olduğu kanıtlanmadı, hata değil — daha derin koşu ya da 'volt test' deneyin",
                n.prop, n.text, n.subject
        ),
    }
}

#[cfg(test)]
mod tests {
    use volt_span::{FileId, Span};
    use volt_sv_emit::AutoProp;

    use super::*;

    const SV: &str = "module C;\n\
                      if (!(rst)) cover (c == 8'd255); // volt:cov_0\n\
                      if (!(rst)) cover (s == 2'd1); // volt:cov_1\n\
                      if (!(rst)) cover (d == 8'd3); // volt:cov_2\n\
                      if (!(rst)) cover (t == 2'd2); // volt:cov_3\n\
                      if (!(rst)) cover (c == 8'd5); // volt:cov_4\n\
                      endmodule\n";

    fn prop(name: &str, reach: Option<CoverReach>) -> SvaProp {
        SvaProp {
            module_name: "C".to_string(),
            name: name.to_string(),
            keyword: "cover",
            span: Span::new(FileId(0), 0, 1),
            primitive: None,
            auto: reach.map(|reach| AutoProp {
                rule: "counter wrap",
                text: format!("{name} text"),
                subject: "wrap check on c".to_string(),
                from: Span::new(FileId(0), 0, 1),
                reach,
            }),
        }
    }

    fn props() -> Vec<SvaProp> {
        vec![
            prop("cov_0", Some(CoverReach::MinDepth(258))), // sayaç, derin
            prop("cov_1", Some(CoverReach::Unknown)),       // sınır bilinmez
            prop("cov_2", None),                            // kullanıcı cover'ı
            prop("cov_3", Some(CoverReach::Never)),         // yapısal ölü
            prop("cov_4", Some(CoverReach::MinDepth(8))),   // sınır < derinlik
        ]
    }

    fn fail(lines: &[usize]) -> SbyOutcome {
        SbyOutcome::Fail(SbyFailure {
            sv_line: lines.first().copied(),
            step: None,
            unreached: lines.to_vec(),
        })
    }

    #[test]
    fn unproven_auto_covers_pass_with_notes() {
        assert_eq!(
            settle(fail(&[2, 3, 6]), "C", SV, &props(), 20),
            SbyOutcome::Pass
        );
        let (real, notes) = split_unreached("C", &[2, 3, 6], SV, &props(), 20);
        assert!(real.is_empty());
        let bounds: Vec<Option<u32>> = notes.iter().map(|n| n.min_depth).collect();
        assert_eq!(bounds, [Some(258), None, None]);
    }

    #[test]
    fn a_deep_enough_run_drops_the_bound_but_stays_a_note() {
        let (real, notes) = split_unreached("C", &[2], SV, &props(), 300);
        assert!(real.is_empty());
        assert_eq!(notes[0].min_depth, None);
    }

    #[test]
    fn user_covers_and_structurally_dead_auto_covers_stay_failures() {
        let settled = settle(fail(&[2, 4, 5]), "C", SV, &props(), 20);
        assert_eq!(settled, fail(&[4, 5]));
        assert_eq!(settle(fail(&[5]), "C", SV, &props(), 20), fail(&[5]));
    }

    #[test]
    fn other_outcomes_and_assert_failures_are_untouched() {
        let assert_fail = SbyOutcome::Fail(SbyFailure {
            sv_line: Some(2),
            step: Some(4),
            unreached: Vec::new(),
        });
        assert_eq!(
            settle(assert_fail.clone(), "C", SV, &props(), 20),
            assert_fail
        );
        assert_eq!(
            settle(SbyOutcome::Pass, "C", SV, &props(), 20),
            SbyOutcome::Pass
        );
    }

    #[test]
    fn another_module_with_the_same_prop_name_is_not_matched() {
        assert_eq!(settle(fail(&[2]), "D", SV, &props(), 20), fail(&[2]));
    }

    #[test]
    fn note_lines_name_the_bound_or_the_lack_of_proof() {
        let (_, notes) = split_unreached("C", &[2, 3], SV, &props(), 20);
        let deep = note_line("C", &notes[0], 20);
        assert!(deep.contains("needs --depth 258"), "{deep}");
        let unknown = note_line("C", &notes[1], 20);
        assert!(unknown.contains("not proven unreachable"), "{unknown}");
    }
}
