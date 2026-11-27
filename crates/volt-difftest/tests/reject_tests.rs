//! Beklenen ret ölçütü (#458), derleyicisiz: tablonun her satırı örnek
//! üretir, modelin kodu satırın kodudur; sınıflandırma kabul edilen ve
//! başka kodla reddedilen ifadeyi fark sayar.

use volt_difftest::ast::{BinOp, UnOp};
use volt_difftest::reject::{classify, errors, generate, module, Op, Operand, UNDEFINED};

#[test]
fn every_row_generates_cases_the_model_rejects_with_the_rows_code() {
    let (cases, ports) = generate(458, 5);
    assert_eq!(cases.len(), UNDEFINED.len() * 5);
    for (ri, u) in UNDEFINED.iter().enumerate() {
        let mine: Vec<_> = cases.iter().filter(|c| c.row == ri).collect();
        assert_eq!(mine.len(), 5, "{}", u.text());
        for c in mine {
            assert_eq!(
                c.model_code(&ports),
                Some(u.code),
                "{}: {}",
                u.text(),
                c.line(0)
            );
        }
    }
}

/// #458'in istediği en az satırlar: Trit'te `~`, `&`, `|`, `^`, `<<`, `>>`
/// ve bit seçme, E2003 (ADR-0142).
#[test]
fn the_table_has_the_trit_rows_of_adr_0142_with_e2003() {
    let want = [
        Op::Un(UnOp::BitNot),
        Op::Bin(BinOp::And),
        Op::Bin(BinOp::Or),
        Op::Bin(BinOp::Xor),
        Op::Bin(BinOp::Shl),
        Op::Bin(BinOp::Shr),
        Op::Index,
    ];
    for op in want {
        let row = UNDEFINED
            .iter()
            .find(|u| u.on == Operand::Trit && u.op == op)
            .unwrap_or_else(|| panic!("Trit {} yok", op.text()));
        assert_eq!(row.code, "E2003", "{}", row.text());
    }
}

/// Satırların metni: tip ve işlem; her satır ayrı.
#[test]
fn every_row_has_its_own_text() {
    assert_eq!(UNDEFINED[0].text(), "Trit: ~a");
    let texts: std::collections::BTreeSet<String> = UNDEFINED.iter().map(|u| u.text()).collect();
    assert_eq!(texts.len(), UNDEFINED.len(), "{texts:?}");
    for t in [
        "Trit: a << b",
        "Trit: a[i]",
        "Trit: a as i1",
        "Trit: a as bits<N>",
        "uN: a as Trit",
        "integer literal: let r : Trit = K (K not in {-1, 0, +1})",
        "bits<N>: a % b",
        "uN: -a",
    ] {
        assert!(texts.contains(t), "{t}: {texts:?}");
    }
}

#[test]
fn the_module_owns_one_line_per_case() {
    let (cases, ports) = generate(1, 2);
    let (text, owner) = module(&cases, &ports);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), owner.len());
    assert_eq!(lines[0], "module Rj {");
    assert_eq!(*lines.last().unwrap(), "}");
    for (k, c) in cases.iter().enumerate() {
        let at = owner.iter().position(|o| *o == Some(k)).expect("satır");
        assert_eq!(lines[at], c.line(k));
        assert!(
            lines[at].starts_with(&format!("    let r{k}")),
            "{}",
            lines[at]
        );
    }
    // `let r : Trit = K` satırı tiplidir.
    let typed = cases
        .iter()
        .position(|c| c.target.is_some())
        .expect("tipli");
    assert!(cases[typed].line(typed).contains(" : Trit = "));
}

/// Kabul edilen ve başka kodla reddedilen ifade fark; beklenen kod fark
/// değil; bir satırın ilk kodu sayılır.
#[test]
fn accepted_and_other_code_are_differences() {
    let (cases, ports) = generate(7, 1);
    let (text, owner) = module(&cases, &ports);
    let line_of = |k: usize| owner.iter().position(|o| *o == Some(k)).unwrap() + 1;
    let mut out = String::from("rj.volt:1:1: warning[W1001]: unused\n");
    for (k, c) in cases.iter().enumerate() {
        let code = UNDEFINED[c.row].code;
        match k {
            0 => {}
            1 => out.push_str(&format!("rj.volt:{}:9: error[E9999]: other\n", line_of(k))),
            _ => {
                out.push_str(&format!(
                    "rj.volt:{}:9: error[{code}]: expected\n",
                    line_of(k)
                ));
                out.push_str(&format!("rj.volt:{}:9: error[E0001]: second\n", line_of(k)));
            }
        }
    }
    let rep = classify(7, 1, &cases, &owner, &text, &out).expect("sınıflandırma");
    assert_eq!(rep.cases(), cases.len());
    assert_eq!(rep.diffs(), 2);
    assert!(!rep.ok());
    assert_eq!(rep.rows[cases[0].row].accepted, 1);
    assert_eq!(rep.rows[cases[1].row].other.get("E9999"), Some(&1));
    let r = rep.render();
    assert!(
        r.starts_with(&format!(
            "difftest-reject: seed=7 rows={} cases={} rejected-as-expected={} accepted=1 other-code=1 diffs=2\n",
            UNDEFINED.len(),
            cases.len(),
            cases.len() - 2
        )),
        "{r}"
    );
    let u0 = &UNDEFINED[cases[0].row];
    assert!(
        r.contains(&format!(
            "difftest-reject: diff: {} — expected {}, compiler accepted: {}\n",
            u0.text(),
            u0.code,
            cases[0].line(0).trim()
        )),
        "{r}"
    );
    assert!(r.contains(", ACCEPTED 1"), "{r}");
    assert!(r.contains(", E9999 1"), "{r}");
    // Hepsi beklenen kodla: fark yok.
    let mut good = String::new();
    for (k, c) in cases.iter().enumerate() {
        let code = UNDEFINED[c.row].code;
        good.push_str(&format!("rj.volt:{}:9: error[{code}]: x\n", line_of(k)));
    }
    let rep = classify(7, 1, &cases, &owner, &text, &good).unwrap();
    assert!(rep.ok(), "{}", rep.render());
    assert!(rep.findings.is_empty());
    assert!(!rep.render().contains("ACCEPTED"), "{}", rep.render());
    // Hiç örneği olmayan satır da koşuyu düşürür.
    let rest: Vec<_> = cases.iter().filter(|c| c.row != 0).cloned().collect();
    let (text, owner) = module(&rest, &ports);
    let mut good = String::new();
    for (k, c) in rest.iter().enumerate() {
        let at = owner.iter().position(|o| *o == Some(k)).unwrap() + 1;
        good.push_str(&format!(
            "rj.volt:{at}:9: error[{}]: x\n",
            UNDEFINED[c.row].code
        ));
    }
    let rep = classify(7, 1, &rest, &owner, &text, &good).unwrap();
    assert_eq!(rep.diffs(), 0);
    assert_eq!(rep.rows[0].cases, 0);
    assert!(!rep.ok());
}

#[test]
fn an_error_no_case_owns_breaks_the_run() {
    let (cases, ports) = generate(3, 1);
    let (text, owner) = module(&cases, &ports);
    let err = classify(
        3,
        1,
        &cases,
        &owner,
        &text,
        "rj.volt:1:1: error[E0001]: x\n",
    );
    assert!(err.is_err());
}

#[test]
fn short_diagnostics_parse_to_line_and_code() {
    let t = "rj.volt:12:9: error[E2003]: operator '~' is not defined for 'Trit'\n\
             rj.volt:3:1: warning[W1001]: unused\n\
             other.volt:4:1: error[E2001]: elsewhere\n\
             \x20 rj.volt:20:7: error[E2009]: cast\n";
    assert_eq!(
        errors(t, "rj.volt"),
        vec![(12, "E2003".to_string()), (20, "E2009".to_string())]
    );
}
