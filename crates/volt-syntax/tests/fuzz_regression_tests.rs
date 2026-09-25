//! Fuzz bulguları için kalıcı regresyon (ADR-0067).
//!
//! `tests/fuzz_regressions/` altındaki her girdi bir zamanlar parser'ı
//! çökertti ya da kaynak tüketti; artık her biri süre sınırı içinde,
//! panik etmeden ayrıştırılmalıdır. Yeni bulgu → yeni dosya, açıklayıcı ad.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use volt_span::FileId;
use volt_syntax::parser::parse;

/// Bir girdi bu süreyi aşarsa test düşer (kaynak patlaması kabul edilmez).
const TIME_LIMIT: Duration = Duration::from_secs(5);

fn regression_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fuzz_regressions")
}

fn regression_inputs() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(regression_dir())
        .expect("tests/fuzz_regressions okunmalı")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("volt"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "fuzz regresyon dizini boş");
    files
}

#[test]
fn every_fuzz_regression_input_parses_within_time_limit() {
    for path in regression_inputs() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // Fuzz hedefi yalnız geçerli UTF-8'i besler (parse_never_panics).
        let src = String::from_utf8(std::fs::read(&path).expect("girdi okunmalı"))
            .unwrap_or_else(|_| panic!("{name}: geçerli UTF-8 olmalı"));
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let res = parse(FileId(0), &src);
            let _ = tx.send(res.diagnostics.len());
        });
        // Süre aşımında iş parçacığı arkada kalır; test süreci bitince ölür.
        assert!(
            rx.recv_timeout(TIME_LIMIT).is_ok(),
            "{name}: {TIME_LIMIT:?} içinde ayrışmadı (kaynak patlaması ya da panik)"
        );
    }
}

#[test]
fn recursive_bundle_fuzz_finding_reports_e4009_not_oom() {
    let src =
        std::fs::read_to_string(regression_dir().join("oom_recursive_struct_port_2087b.volt"))
            .expect("girdi okunmalı");
    let res = parse(FileId(0), &src);
    assert!(
        res.error_codes().contains(&"E4009"),
        "{:?}",
        res.error_codes()
    );
}

/// Fuzz bulgusu 2 (ADR-0068): iç içe `for` açılımında sabit olmayan iç
/// sınır her yineleme çiftinde aynı E2005'i (ADR-0072'den beri E2021) üretiyordu (283² → 65 303
/// tanı, 347 MB). Katlama sonrası: bir saniyenin altında, avuç içi tanı.
#[test]
fn nested_for_diagnostic_flood_fuzz_finding_parses_fast_with_few_diagnostics() {
    let src =
        std::fs::read_to_string(regression_dir().join("oom_nested_for_diag_flood_1157b.volt"))
            .expect("girdi okunmalı");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let res = parse(FileId(0), &src);
        let _ = tx.send((res.diagnostics.len(), res.error_codes()));
    });
    let (count, codes) = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("1 s içinde ayrışmalı (tanı seli)");
    assert!(count < 100, "{count} tanı: {codes:?}");
    assert!(codes.contains(&"E2021"), "{codes:?}");
}

/// Fuzz bulgusu 3 (ADR-0068 §6): üç iç içe süslüsüz `for`, gövdede hata
/// kurtarmanın bıraktığı n düğümlük desen ağacı. Bütçe yalnız deyim +
/// ifade sayıyordu; desen arenası 65 534 yinelemede kopyalandı (orijinal
/// 2,8 M desen düğümü, n = 128'de 1,47 GB). Şimdi her yazım sayılır ve
/// kurtarma düğümlü gövde bir kez açılır.
#[test]
fn pattern_clone_budget_fuzz_findings_parse_fast_with_a_bounded_ast() {
    for name in [
        "oom_for_pattern_clone_budget_min_84b.volt",
        "oom_for_pattern_clone_budget_2318b.volt",
    ] {
        let src = std::fs::read_to_string(regression_dir().join(name)).expect("girdi okunmalı");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let res = parse(FileId(0), &src);
            let a = &res.ast;
            let _ = tx.send(
                a.stmts.len() + a.exprs.len() + a.types.len() + a.patterns.len() + a.blocks.len(),
            );
        });
        let nodes = rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or_else(|_| panic!("{name}: 1 s içinde ayrışmalı (desen klonu)"));
        assert!(nodes < 300_000, "{name}: {nodes} AST düğümü");
    }
}

/// Yığın taşması sınıfı (ADR-0080): `stack_*` girdilerinin her biri
/// düzeltmeden önce derleyiciyi abort ettiriyordu (ADR-0068 §6 bulgusu:
/// 30 000 terimlik zincir generic şablonda Cloner'da, düz modülde
/// denetimde). Şimdi tek E0018, bir saniyenin altında.
#[test]
fn stack_depth_fuzz_findings_report_one_e0018_quickly() {
    let inputs: Vec<PathBuf> = regression_inputs()
        .into_iter()
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("stack_")
        })
        .collect();
    assert!(inputs.len() >= 14, "{} stack_ girdisi", inputs.len());
    for path in inputs {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(&path).expect("girdi okunmalı");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let res = parse(FileId(0), &src);
            let _ = tx.send(res.error_codes());
        });
        let codes = rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or_else(|_| panic!("{name}: 1 s içinde ayrışmalı"));
        assert_eq!(codes, ["E0018"], "{name}");
    }
}
