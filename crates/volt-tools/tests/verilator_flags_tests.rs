//! Volt'un Verilator'ı derleme ya da simülasyon için çağırdığı her yol
//! `VERILATOR_BUILD_FLAGS`'ı (`-fno-dfg`) taşır (#148; ADR-0107, "Ek: DFG
//! kapalı").
//!
//! Bayrak tek bir yerde tanımlıdır (`volt_tools::VERILATOR_BUILD_FLAGS`).
//! Her yolun komut satırı kendi testinde denetlenir:
//!
//! - `volt test`, `volt run`, `volt test --waves`, yerel ve Docker:
//!   `volt-driver` `sim/verilator.rs`
//!   (`every_verilate_path_passes_volts_build_flags`) ve
//!   `docker_backend_tests` (sahte `docker`'ın aldığı komut);
//! - fark testinin koşucusu: `volt-difftest` `run.rs`
//!   (`verilator_command_carries_the_callers_flags`), listeyi
//!   `difftest_tests` verir;
//! - hakem kanaryası: `verilator_canary_tests` (listeyi doğrudan ekler).
//!
//! Bu test envanterdir: depoda Verilator'ı derleme için çağıran başka bir
//! yer (yeni bir test yardımcısı, bir betik, bir CI adımı) açılırsa düşer.
//! `--lint-only` çağrıları eniyileme yapmaz ve sayılmaz.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Depo dosyaları (`target`, `.git` ve kullanıcı derleme çıktıları hariç).
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                "target" | ".git" | "build" | "node_modules" | "book"
            ) {
                // `book/` yalnız `book/tools` için taranır (aşağıda); `book/src`
                // okurun komutlarıdır, Volt'un çağrısı değildir.
                continue;
            }
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn rel(path: &Path) -> String {
    path.strip_prefix(repo())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Verilator'ı derleme için çağıran Rust kaynakları ve her birinde
/// listenin geldiği ad.
const RUST_CALL_SITES: &[(&str, &str)] = &[
    (
        "crates/volt-driver/src/sim/verilator.rs",
        "args.extend(volt_tools::VERILATOR_BUILD_FLAGS",
    ),
    (
        "crates/volt-difftest/src/run.rs",
        "verilator_args(&self.tools.verilator_flags",
    ),
    (
        "crates/volt-driver/tests/verilator_canary_tests.rs",
        ".args(volt_tools::VERILATOR_BUILD_FLAGS)",
    ),
];

#[test]
fn the_flags_turn_dfg_off() {
    assert!(
        volt_tools::VERILATOR_BUILD_FLAGS.contains(&"-fno-dfg"),
        "#148: Verilator 5.052'nin DFG'si doğru SV'yi yanlış derler; bayrak \
         yalnız ADR-0107 ekindeki ölçütle kaldırılır: {:?}",
        volt_tools::VERILATOR_BUILD_FLAGS
    );
}

#[test]
fn every_verilator_build_in_the_repository_takes_the_flags_from_one_place() {
    let root = repo();
    let mut all = Vec::new();
    files(&root.join("crates"), &mut all);
    files(&root.join("scripts"), &mut all);
    files(&root.join(".github"), &mut all);
    files(&root.join("book").join("tools"), &mut all);
    files(&root.join("examples"), &mut all);
    files(&root.join("templates"), &mut all);
    all.push(root.join("justfile"));

    let mut found = Vec::new();
    let mut unknown = Vec::new();
    for path in &all {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let name = rel(path);
        if name.ends_with("/verilator_flags_tests.rs") {
            continue; // bu dosya aranan dizgeleri içerir
        }
        if name.ends_with(".rs") {
            // Verilator'ın derleme kipini seçen argüman bir Rust dizgesi.
            let builds = ["\"--binary\"", "\"--cc\"", "\"--build\""]
                .iter()
                .any(|a| text.contains(a));
            if !builds {
                continue;
            }
            match RUST_CALL_SITES.iter().find(|(f, _)| *f == name) {
                Some((_, via)) => {
                    assert!(
                        text.contains(via),
                        "{name} Verilator'ı derliyor ama seçenekleri `{via}`'dan almıyor"
                    );
                    found.push(name);
                }
                None => unknown.push(format!("{name}: Rust'ta Verilator derleme argümanı")),
            }
        } else {
            // Betik, iş akışı, justfile: `verilator --binary|--cc|--build|--exe`.
            for (i, line) in text.lines().enumerate() {
                let l = line.trim_start();
                if l.starts_with('#') || l.starts_with("//") {
                    continue;
                }
                let words: Vec<&str> = l.split_whitespace().collect();
                let call = words.windows(2).any(|w| {
                    (w[0] == "verilator" || w[0].ends_with("/verilator"))
                        && matches!(w[1], "--binary" | "--cc" | "--build" | "--exe" | "-b")
                });
                if call && !l.contains("-fno-dfg") {
                    unknown.push(format!("{name}:{}: {}", i + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "Verilator'ı derleme için çağıran, envanterde olmayan ya da -fno-dfg \
         almayan yerler (#148): bayrağı volt_tools::VERILATOR_BUILD_FLAGS'tan \
         alın ve RUST_CALL_SITES'a ekleyin.\n{}",
        unknown.join("\n")
    );
    found.sort();
    let mut expected: Vec<String> = RUST_CALL_SITES.iter().map(|(f, _)| f.to_string()).collect();
    expected.sort();
    assert_eq!(
        found, expected,
        "envanterdeki bir çağrı yeri artık Verilator derlemiyor: listeyi güncelleyin"
    );
}

/// Fark testinin koşucusu listeyi çağırandan alır (crate `volt-*`'a
/// bağlanmaz, ADR-0108 §1): her `volt_difftest::Tools` kurulumu
/// `VERILATOR_BUILD_FLAGS`'ı verir.
#[test]
fn every_difftest_runner_gets_the_flags() {
    let mut all = Vec::new();
    files(&repo().join("crates"), &mut all);
    let mut sites = 0;
    for path in all.iter().filter(|p| {
        p.extension().is_some_and(|e| e == "rs") && !p.ends_with("verilator_flags_tests.rs")
    }) {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for (at, _) in text.match_indices("volt_difftest::Tools {") {
            sites += 1;
            let body = &text[at..];
            let body = &body[..body.find("};").unwrap_or(body.len())];
            assert!(
                body.contains("verilator_flags: volt_tools::VERILATOR_BUILD_FLAGS"),
                "{}: volt_difftest::Tools VERILATOR_BUILD_FLAGS'sız kuruluyor:\n{body}",
                rel(path)
            );
        }
    }
    assert!(
        sites >= 1,
        "difftest_tests bir volt_difftest::Tools kurmalı"
    );
}
