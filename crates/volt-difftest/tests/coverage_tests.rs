//! Kapsam raporu (#458, aşama 0): CI'nın küçük koşusunun tohumuyla
//! (`Config::DEFAULT_SEED`) üretilen toplu koşular, Verilator'suz. Üretecin
//! bugün ürettiği her tür ([`LANGUAGE`]'da `generated`) en az bir kez çıkar
//! ve üretmediği hiçbir tür çıkmaz; her biçim en az bir ifadede vardır.
//! Bir tür sıfıra düşerse (ya da tabloya yazılmadan üretilmeye başlarsa)
//! test kırmızıdır.

use volt_difftest::coverage::{item_of, Coverage, LANGUAGE};
use volt_difftest::harness::Variant;
use volt_difftest::rng::Rng;
use volt_difftest::run::{generate_batch, Config};

/// CI'daki küçük koşunun ilk 2000 ifadesi (8 toplu koşu × 250).
fn ci_seed_coverage() -> Coverage {
    let mut rng = Rng::new(Config::DEFAULT_SEED);
    let mut cov = Coverage::default();
    for _ in 0..8 {
        let b = generate_batch(&mut rng, 250, 16);
        cov.add_cases(&b.cases, &b.ports);
    }
    cov
}

#[test]
fn every_kind_the_generator_produces_appears_with_the_ci_seed() {
    let cov = ci_seed_coverage();
    eprintln!("{}", cov.render());
    assert_eq!(cov.cases, 2000);
    let mut wrong = Vec::new();
    for i in LANGUAGE {
        let n = cov.count(i.key);
        if i.generated && n == 0 {
            wrong.push(format!("{}: generated, but 0 with the CI seed", i.key));
        }
        if !i.generated && n > 0 {
            wrong.push(format!(
                "{}: {n} with the CI seed, but the table says never generated",
                i.key
            ));
        }
    }
    for v in Variant::ALL {
        if cov.forms.get(&v).copied().unwrap_or(0) == 0 {
            wrong.push(format!("form {} ({}): 0", v.letter(), v.what()));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Raporun biçimi: üretilmeyen satır kaynağı ve sonraki adımıyla, sayılan
/// satır sayısıyla; özet satırı tablo ile sayaçlardan.
#[test]
fn the_report_names_what_is_never_generated() {
    let cov = ci_seed_coverage();
    let text = cov.render();
    let never = LANGUAGE.iter().filter(|i| !i.generated).count();
    assert!(
        text.starts_with(&format!(
            "difftest: coverage: expressions=2000 language-items={} generated={} never-generated={never}\n",
            LANGUAGE.len(),
            LANGUAGE.len() - never
        )),
        "{text}"
    );
    let arr = item_of("[T; N] (array)");
    assert!(
        text.contains(&format!(
            "  type [T; N] (array): 0 never generated ({}; {})\n",
            arr.source, arr.next
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!("  operation a + b: {}\n", cov.count("a + b"))),
        "{text}"
    );
    assert!(text.contains("  form g (contract (ensures)): "), "{text}");
    assert!(
        text.contains("difftest: coverage by operand type: "),
        "{text}"
    );
    assert_eq!(cov.missing().len(), never);
}

/// #458: Trit, tanımlı işlemleriyle (`t ± u`, `t as iN`, karşılaştırma)
/// üretilir; `-t` ve Trit çarpımı yalnız `VOLT_DIFFTEST_TRIT_ARITH=1` ile
/// (`gen::trit_arith_enabled`). İşlem ve işlenen tipi sayacı raporda.
#[test]
fn trit_and_its_operations_are_generated() {
    let cov = ci_seed_coverage();
    assert!(cov.count("Trit") > 0);
    let text = cov.render();
    let mut want = vec![
        "a + b on Trit",
        "a - b on Trit",
        "a as T on Trit",
        "a == b on Trit",
    ];
    if volt_difftest::gen::trit_arith_enabled() {
        want.extend(["-a on Trit", "a * b on Trit"]);
    } else {
        // Geçersiz akışta `~t`, `t * y` (y : uN) gibi ifadeler kalır, ama
        // geçerli bir `-t` üretilmez.
        assert_eq!(cov.op_types.get("-a on Trit"), None, "{text}");
    }
    for k in want {
        let n = cov.op_types.get(k).copied().unwrap_or(0);
        assert!(n > 0, "{k}: 0\n{text}");
        assert!(text.contains(&format!("{k}={n}")), "{k}\n{text}");
    }
}

#[test]
#[should_panic(expected = "kapsam tablosunda yok")]
fn a_key_outside_the_table_is_a_programming_error() {
    item_of("a <=> b");
}

/// Geniş literalin (`K as bits<N>`, A14) işleneni literal sayılır: modelde
/// kendi başına tipi yoktur, "ill-typed" değildir.
#[test]
fn a_wide_literal_operand_counts_as_a_literal() {
    use volt_difftest::ast::{Expr, LitFmt, PortTy};
    use volt_difftest::harness::Case;
    let ports = [PortTy::Bits(128)];
    let e = Expr::cast(Expr::WideLit(1u128 << 127, LitFmt::Hex), PortTy::Bits(128));
    let mut rng = Rng::new(1);
    let c = Case::new(0, e, &ports, &mut rng);
    let mut cov = Coverage::default();
    cov.add_case(&c, &ports);
    assert_eq!(
        cov.op_types.get("a as T on integer literal (untyped)"),
        Some(&1),
        "{:?}",
        cov.op_types
    );
    assert_eq!(cov.op_types.get("a as T on ill-typed operand"), None);
    assert_eq!(cov.count("hexadecimal (0x)"), 1);
}

/// Kapsam her koşunun raporunda, en sonda (#458).
#[test]
fn the_run_report_ends_with_the_coverage() {
    let report = volt_difftest::Report {
        coverage: ci_seed_coverage(),
        ..volt_difftest::Report::default()
    };
    let text = report.render();
    let cov = report.coverage.render();
    assert!(text.ends_with(&cov), "{text}");
    assert!(
        text.contains("\ndifftest: coverage: expressions=2000 "),
        "{text}"
    );
    // Özet satırı ayrıştırıcısının (`difftest_summary.py`) satırı önde.
    assert!(text.starts_with("difftest: seed="), "{text}");
}
