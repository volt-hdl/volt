//! CI'ın Verilator'ı ile Docker arka ucunun Verilator'ı aynı sürümdür
//! (ADR-0107, "Hakem sürümü"; #145).
//!
//! Sürüm tek bir yerde tanımlıdır: `scripts/ci/verilator.env`. CI'ın
//! `.github/actions/verilator` eylemi Verilator'ı oradaki imajdan çıkarır
//! ve `verilator --version` o sürümü vermezse işi düşürür. Docker arka ucu
//! (`docker::SIMULATION_IMAGE`) kullanıcının `volt test`'ini aynı imajda
//! koşturur. Biri değişip öteki değişmezse bu test düşer: CI ile kullanıcı
//! farklı hakemle ölçülmesin.

use std::collections::BTreeMap;
use std::path::Path;

use volt_tools::docker::SIMULATION_IMAGE;

/// `scripts/ci/verilator.env`'in `ANAHTAR=değer` satırları.
fn pin() -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/ci/verilator.env");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} okunamadı: {e}", path.display()));
    let mut map = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once('=')
            .unwrap_or_else(|| panic!("verilator.env: `ANAHTAR=değer` değil: {line}"));
        assert!(
            !v.contains(['"', '\'', '$', ' ']),
            "verilator.env: {k} düz bir değer olmalı (tırnak, boşluk, değişken yok): {v}"
        );
        assert!(
            map.insert(k.to_string(), v.to_string()).is_none(),
            "verilator.env: {k} iki kez tanımlı"
        );
    }
    map
}

fn get<'a>(pin: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    pin.get(key)
        .unwrap_or_else(|| panic!("verilator.env: {key} yok"))
}

#[test]
fn ci_verilator_pin_and_the_docker_backend_image_are_the_same_release() {
    let pin = pin();
    let version = get(&pin, "VERILATOR_VERSION");
    let image = get(&pin, "VERILATOR_IMAGE");
    let digest = get(&pin, "VERILATOR_DIGEST");

    // Bir sürüm: `5.052` biçimi; `5.053-devel` ya da bir commit değil.
    let parts: Vec<&str> = version.split('.').collect();
    assert!(
        parts.len() == 2 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
        "VERILATOR_VERSION bir Verilator sürümü olmalı (ör. 5.052), geliştirme derlemesi değil: {version}"
    );
    assert_eq!(
        image,
        format!("verilator/verilator:v{version}"),
        "VERILATOR_IMAGE, VERILATOR_VERSION'ın sürüm etiketi olmalı"
    );

    assert_eq!(
        SIMULATION_IMAGE.name, image,
        "Docker arka ucunun imajı (docker.rs SIMULATION_IMAGE) ile CI'ın Verilator'ı (scripts/ci/verilator.env) farklı"
    );
    assert_eq!(
        SIMULATION_IMAGE.digest, digest,
        "Docker arka ucunun imaj özeti ile CI'ınki farklı"
    );
    assert!(
        SIMULATION_IMAGE
            .contents
            .starts_with(&format!("Verilator {version},")),
        "SIMULATION_IMAGE.contents (volt doctor) Verilator {version} demeli: {}",
        SIMULATION_IMAGE.contents
    );
}
