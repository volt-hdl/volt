//! Fark ve metamorfik test (ADR-0108): rastgele Volt ifadeleri derleyicinin
//! ürettiği SV ile Verilator'da koşar ve derleyiciden bağımsız referans
//! modelle (`volt-difftest`) karşılaştırılır. Her ifade satır içi, tipsiz
//! `let`'te, alt ifadesi `let`'e çıkarılmış, ek parantezli, işlenenleri
//! yer değiştirmiş, karşılaştırmada, daha geniş hedefte ve sözleşmede
//! sınanır.
//!
//! Tohum ve sayı: `VOLT_DIFFTEST_SEED` (varsayılan 20261104),
//! `VOLT_DIFFTEST_COUNT` (kabul edilmiş ifade hedefi, varsayılan 2000).
//! Özet `VOLT_DIFFTEST_SUMMARY` dosyasına da yazılır (CI günlüğü için).
//! Verilator yoksa test atlar; araçlı CI işinde (`VOLT_REQUIRE_TOOLS=all`)
//! yokluk testi düşürür (ADR-0107).

mod tools;

use std::io::Write as _;

#[test]
fn volt_expressions_match_the_reference_model_in_verilator() {
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    let cfg = volt_difftest::Config::from_env();
    let work =
        std::env::temp_dir().join(format!("volt-difftest-{}-{}", std::process::id(), cfg.seed));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("çalışma dizini");
    eprintln!(
        "difftest: seed={} count={} batch={} vectors={}",
        cfg.seed, cfg.count, cfg.batch, cfg.vectors
    );
    let tools = volt_difftest::Tools {
        volt: env!("CARGO_BIN_EXE_volt").into(),
        verilator,
        // #148: hakem `volt test`'in Verilator seçenekleriyle derler.
        verilator_flags: volt_tools::VERILATOR_BUILD_FLAGS
            .iter()
            .map(|f| (*f).to_string())
            .collect(),
    };
    let report = volt_difftest::run(&cfg, &tools, &work);
    let text = report.render();
    eprintln!("{text}");
    if let Ok(path) = std::env::var("VOLT_DIFFTEST_SUMMARY") {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|e| panic!("VOLT_DIFFTEST_SUMMARY ({path}): {e}"));
        f.write_all(text.as_bytes()).expect("özet yazılamadı");
    }
    if report.ok() {
        let _ = std::fs::remove_dir_all(&work);
    }
    assert!(report.ok(), "{text}");
}
