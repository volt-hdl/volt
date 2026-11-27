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

/// #458: Trit, tanımlı işlemleriyle (`-t`, `t * u`, `t ± u`, `t as iN`,
/// karşılaştırma) üretilir; işlem ve işlenen tipi sayacı raporda.
#[test]
fn trit_and_its_operations_are_generated() {
    let cov = ci_seed_coverage();
    assert!(cov.count("Trit") > 0);
    let text = cov.render();
    for k in [
        "-a on Trit",
        "a * b on Trit",
        "a + b on Trit",
        "a - b on Trit",
        "a as T on Trit",
        "a == b on Trit",
    ] {
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
