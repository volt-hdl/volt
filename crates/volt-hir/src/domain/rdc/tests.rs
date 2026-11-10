//! rdc olgu tablosu birim testleri; kural testleri `tests/rdc_tests.rs`.

use volt_ast::{ResetPolarity, ResetSpec, ResetSync};
use volt_syntax::{parse, FileId};

use super::facts::{feeds_of, UnitFacts, DEFAULT_RESET};
use crate::consteval::ConstEvaluator;
use crate::domain::testutil::inferred;
use crate::domain::{infer_domains, DomainId};
use crate::resolve::resolve_file;
use crate::typeck::typecheck;

/// Olgu tablosu üzerinde `check` çalıştırır.
fn with_facts(src: &str, check: impl FnOnce(&UnitFacts)) {
    let parsed = parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let res = resolve_file(&parsed.ast);
    let mut evaluator = ConstEvaluator::new(&parsed.ast, &res);
    evaluator.eval_all_consts();
    let tyck = typecheck(&parsed.ast, &res, &mut evaluator);
    let dom = infer_domains(&parsed.ast, &res, &tyck);
    check(&UnitFacts::collect(&parsed.ast, &res, &dom));
}

#[test]
fn implicit_clock_domain_has_the_generated_sync_reset() {
    with_facts("module M { in clk : clock }", |f| {
        let c = &f.modules[0].clocks[0];
        assert_eq!(c.reset, Some(DEFAULT_RESET));
        assert_eq!(c.reset_span, c.port.name.span, "etiket port adına düşer");
        assert_eq!(c.decl, None);
        assert!(!c.used);
    });
}

#[test]
fn declared_domain_without_reset_field_uses_the_default_at_its_name() {
    with_facts(
        "domain D { clock = negedge }\nmodule M { in clk : clock @D }",
        |f| {
            let c = &f.modules[0].clocks[0];
            assert_eq!(c.reset, Some(DEFAULT_RESET));
            assert_ne!(c.reset_span, c.port.name.span);
            assert!(c.decl.is_some());
        },
    );
}

#[test]
fn reset_none_is_never_fed_by_a_raw_port() {
    with_facts(
        "domain D { clock = posedge, reset = none }\nmodule M { in clk : clock @D in r : reset }",
        |f| {
            assert_eq!(f.modules[0].clocks[0].reset, None);
            assert_eq!(feeds_of(&f.modules[0]), vec![None]);
        },
    );
}

#[test]
fn the_last_reset_field_wins_like_in_sv_emit() {
    with_facts(
        "domain D { reset = sync active_high, reset = async active_low }\nmodule M { in clk : clock @D }",
        |f| {
            let expected = ResetSpec {
                sync: ResetSync::Async,
                polarity: ResetPolarity::ActiveLow,
            };
            assert_eq!(f.modules[0].clocks[0].reset, Some(expected));
        },
    );
}

#[test]
fn only_modules_nobody_instantiates_are_roots() {
    let src = "module C { in clk : clock out q : u8 q = 0 }\n\
               module T { in clk : clock out q : u8 let u = C { clk: clk } q = u.q }";
    with_facts(src, |f| {
        assert!(!f.modules[0].is_root, "C örnekleniyor");
        assert!(f.modules[1].is_root);
        assert!(
            f.modules[1].clocks[0].used,
            "örnek bağlaması da kullanımdır"
        );
    });
}

#[test]
fn ram_write_clock_and_extern_bindings_are_not_reset_uses() {
    let src = "domain A { clock = posedge }\ndomain B { clock = posedge }\n\
               extern module X { in c : clock @S }\n\
               module M { in a_clk : clock @A in b_clk : clock @B in x_clk : clock @A \
               in ra : bits<2> @B out q : u8 @B \
               let m = AsyncDualPortRam<u8, 4> { wr_clk: a_clk, wr_addr: 0, wr_data: 1, wr_en: true, \
               rd_clk: b_clk, rd_addr: ra } q = m.rd_data let e = X { c: x_clk } }";
    with_facts(src, |f| {
        let used: Vec<bool> = f.modules[0].clocks.iter().map(|c| c.used).collect();
        assert_eq!(
            used,
            [false, true, false],
            "a: yazma tarafı, b: okuma, x: extern"
        );
    });
}

#[test]
fn raw_port_spec_and_annotation_are_recorded() {
    let src = "domain D { clock = posedge, reset = async active_low }\n\
               module M { in clk : clock @D in r : reset(async, active_low) @D in s : reset }";
    with_facts(src, |f| {
        let raws = &f.modules[0].raws;
        assert_eq!(raws.len(), 2);
        assert!(raws[0].ann.is_some() && raws[0].spec.is_some());
        assert!(raws[1].ann.is_none() && raws[1].spec.is_none());
        // İki ham port: yalnız anotasyonu eşleşen besler.
        assert_eq!(feeds_of(&f.modules[0]), vec![Some(0)]);
    });
}

#[test]
fn raw_reset_port_belongs_to_no_clock_domain() {
    let r = inferred(
        "domain A { clock = posedge }\ndomain B { clock = posedge }\n\
         module M { in a_clk : clock @A in b_clk : clock @B in rst : reset }",
    );
    assert_eq!(r.domain_of("rst"), DomainId::Timeless);
    assert!(
        r.codes().is_empty(),
        "çoklu saatte E3010 değil: {:?}",
        r.codes()
    );
}
