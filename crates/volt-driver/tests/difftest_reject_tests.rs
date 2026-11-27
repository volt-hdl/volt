//! Beklenen ret ölçütü (#458): her tipin tanımsız işlemleri
//! (`volt_difftest::reject::UNDEFINED`; Trit'te `~`, `& | ^`, `<< >>`, bit
//! seçme, tanımsız dönüşümler, `bits<N>` aritmetiği, ...) bu derleyicide
//! tam beklenen tanı koduyla reddedilir. Yalnız `volt check`: araç
//! gerekmez, her işte koşar.
//!
//! Yayımlanmış bir sürüm aynı ölçütle `volt-difftest-reject` örneğiyle
//! sınanır (`cargo run -p volt-difftest --example volt-difftest-reject --
//! --volt <yol>`; v0.1.3 `~t`'yi kabul eder, #434); o ikili depoya girmez.

#[test]
fn every_undefined_operation_is_rejected_with_its_code() {
    let seed = 458;
    let work = std::env::temp_dir().join(format!(
        "volt-difftest-reject-{}-{seed}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&work);
    let report = volt_difftest::reject::run_reject(
        std::path::Path::new(env!("CARGO_BIN_EXE_volt")),
        seed,
        8,
        &work,
    );
    let text = report.render();
    eprintln!("{text}");
    let _ = std::fs::remove_dir_all(&work);
    assert_eq!(
        report.cases(),
        volt_difftest::reject::UNDEFINED.len() * 8,
        "{text}"
    );
    assert!(report.ok(), "{text}");
}
