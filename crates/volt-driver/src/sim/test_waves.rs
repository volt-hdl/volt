//! `volt test` dalga formu (ADR-0095 §3).
//!
//! Varsayılan (`OnFailure`): testler izsiz derlenip koşar — geçen yolda
//! maliyet ve çıktı değişmez. Bir grupta test düşerse YALNIZ düşen testler
//! `--trace` ile ayrı bir yürütülebilirde yeniden koşar (simülasyon
//! deterministiktir; ölçüm: soğuk konteynerde `--trace` derlemesi grup
//! başına +1,3–2,1 s, koşu farkı ihmal edilebilir). `--waves` bütün
//! testleri ilk koşuda izler, `--no-waves` kaydı kapatır.
//!
//! VCD'ler `<sim dizini>/waves/<Modül>-<test>.vcd`; testbench yolu
//! çalışma dizinine (sim dizini) göre açar — yerelde ve Docker'da aynı.
//! Kullanıcıya ana makine yolu basılır.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use volt_diagnostics::lstr;
use volt_sv_emit::{sim as tbgen, SimPort, TbTest};

use super::tb_output::{parse_tb_output, TestOutcome};
use super::verilator::{run_simulation, verilate, VerilateJob};
use super::write_file;
use crate::tool_backend::Runner;
use crate::waves::{self, PlainTrace, WaveScope};
use crate::Compiled;

/// Dalga formu kaydı (cli-contract.md §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaveMode {
    /// Düşen test izli yeniden koşar (varsayılan).
    OnFailure,
    /// `--waves`: her test ilk koşuda izlenir.
    All,
    /// `--no-waves`: kayıt yok.
    Off,
}

/// Grup dizinindeki VCD alt dizini.
pub(super) const WAVES_DIR: &str = "waves";

/// Testin VCD dosya adı: `<Modül>-<test>.vcd`; adın harf/rakam/`_`
/// dışındaki karakterleri `_` olur.
pub(super) fn vcd_name(module: &str, test: &str) -> String {
    let slug: String = test
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{module}-{slug}.vcd")
}

/// Testbench'e gömülen, sim dizinine göre yol (`waves/<ad>.vcd`).
pub(super) fn vcd_rel(module: &str, test: &str) -> String {
    format!("{WAVES_DIR}/{}", vcd_name(module, test))
}

/// Grubun eski kayıtlarını siler ve dizini hazırlar: bugün geçen testin
/// dünkü VCD'si yanıltmasın. Docker dizini kök sahipli oluşturmasın diye
/// dizin ana makinede kurulur.
pub(super) fn prepare_dir(sim_dir: &Path, module: &str) {
    let dir = sim_dir.join(WAVES_DIR);
    let prefix = format!("{module}-");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) {
                let path = entry.path();
                let _ = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
            }
        }
    }
    let _ = std::fs::create_dir_all(&dir);
}

/// Bir grubun dalga formu bağlamı.
pub(super) struct GroupWaves<'a> {
    pub sim_dir: &'a Path,
    pub module: &'a str,
    pub ports: &'a [SimPort],
    pub compiled: &'a Compiled,
}

impl GroupWaves<'_> {
    /// Testin ana makinedeki VCD yolu.
    fn host_vcd(&self, test: &str) -> PathBuf {
        self.sim_dir
            .join(WAVES_DIR)
            .join(vcd_name(self.module, test))
    }

    /// VCD'nin yanına GTKWave oturumunu yazar (DUT portları + enum/Trit
    /// çevirileri) ve `gtkwave <vcd> <gtkw>` satırını döndürür. VCD yoksa
    /// (testbench yazamadıysa) `None`.
    pub fn session_hint(&self, test: &str) -> Option<String> {
        let vcd = self.host_vcd(test);
        if !vcd.is_file() {
            return None;
        }
        let plain: Vec<PlainTrace> = self
            .ports
            .iter()
            .filter_map(|p| {
                Some(PlainTrace {
                    name: p.name.clone(),
                    width: p.bits?,
                })
            })
            .collect();
        let session = match waves::write_session_with(
            &vcd,
            &self.compiled.waves,
            self.module,
            WaveScope::Simulation,
            &plain,
        ) {
            Ok(s) => s,
            Err(e) => {
                warn(&lstr!(
                    en: "could not write the waveform session for {}: {e}", vcd.display();
                    tr: "{} için dalga formu oturumu yazılamadı: {e}", vcd.display()
                ));
                waves::Session::None
            }
        };
        Some(waves::open_hint(&vcd, &session))
    }

    /// Düşen testleri izli ayrı yürütülebilirde yeniden koşar; test adı →
    /// açma satırı. Yeniden koşu başarısız olursa (derleme, araç) uyarı
    /// basılır ve kayıt atlanır — test sonucu değişmez.
    pub fn rerun_failed(
        &self,
        runner: &Runner,
        inputs: &[String],
        failed: &[TbTest],
        contracts: bool,
    ) -> Vec<(String, String)> {
        match self.rerun(runner, inputs, failed, contracts) {
            Ok(hints) => hints,
            Err(_) => {
                warn(&lstr!(
                    en: "the waveform of the failed test(s) was not recorded (see the error above)";
                    tr: "düşen test(ler)in dalga formu kaydedilemedi (yukarıdaki hataya bakın)"
                ));
                Vec::new()
            }
        }
    }

    fn rerun(
        &self,
        runner: &Runner,
        inputs: &[String],
        failed: &[TbTest],
        contracts: bool,
    ) -> Result<Vec<(String, String)>, ExitCode> {
        let module = self.module;
        let vcds: Vec<String> = failed.iter().map(|t| vcd_rel(module, &t.name)).collect();
        let tb = tbgen::test_testbench_cpp_traced(module, self.ports, failed, contracts, &vcds);
        let tb_name = format!("tb_{module}_waves.cpp");
        write_file(&self.sim_dir.join(&tb_name), &tb)?;
        let job = VerilateJob {
            sim_dir: self.sim_dir,
            inputs,
            tb_file: &tb_name,
            module,
            trace: true,
            mdir: &format!("obj_{}_waves", module.to_lowercase()),
        };
        let exe = verilate(runner, &job, &[])?;
        let output = run_simulation(runner, &exe, self.sim_dir, &[])?;
        let rerun = parse_tb_output(&String::from_utf8_lossy(&output.stdout));
        Ok(self.hints_for(failed, &rerun))
    }

    /// Yeniden koşuda da düşen testlerin açma satırları. Geçen test
    /// (belirlenimci olmayan tasarım) için uyarı basılır, satır yok.
    fn hints_for(&self, tests: &[TbTest], rerun: &[TestOutcome]) -> Vec<(String, String)> {
        let mut hints = Vec::new();
        for t in tests {
            let still_failed = rerun.iter().any(|o| o.name == t.name && !o.passed);
            if !still_failed {
                warn(&lstr!(
                    en: "test '{}' passed when re-run with tracing; no waveform (is the design deterministic?)", t.name;
                    tr: "'{}' testi izle yeniden koşunca geçti; dalga formu yok (tasarım belirlenimci mi?)", t.name
                ));
                continue;
            }
            if let Some(hint) = self.session_hint(&t.name) {
                hints.push((t.name.clone(), hint));
            }
        }
        hints
    }
}

fn warn(msg: &str) {
    eprintln!("{}", lstr!(en: "warning: {msg}"; tr: "uyarı: {msg}"));
}

/// Düşen testlerin adları, gruptaki sırasıyla.
pub(super) fn failed_tests(tests: &[TbTest], outcomes: &[TestOutcome]) -> Vec<TbTest> {
    tests
        .iter()
        .filter(|t| outcomes.iter().any(|o| o.name == t.name && !o.passed))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(name: &str) -> TbTest {
        TbTest {
            name: name.into(),
            steps: Vec::new(),
        }
    }

    fn outcome(name: &str, passed: bool) -> TestOutcome {
        TestOutcome {
            name: name.into(),
            passed,
            failure: None,
            contract: None,
            waveform: None,
        }
    }

    #[test]
    fn vcd_name_is_module_and_sanitized_test_name() {
        assert_eq!(
            vcd_name("Counter", "wraps_to_zero"),
            "Counter-wraps_to_zero.vcd"
        );
        assert_eq!(vcd_name("C", "a b/c:d"), "C-a_b_c_d.vcd");
        assert_eq!(vcd_rel("C", "x"), "waves/C-x.vcd");
    }

    #[test]
    fn failed_tests_keeps_group_order_and_skips_passed() {
        let tests = [test("a"), test("b"), test("c")];
        let outcomes = [outcome("c", false), outcome("a", false), outcome("b", true)];
        let names: Vec<String> = failed_tests(&tests, &outcomes)
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, ["a", "c"]);
    }

    #[test]
    fn session_hint_lists_ports_and_enum_traces_only_when_the_vcd_exists() {
        // Arrange: enum'lu port (çeviri izi) + düz portlar.
        let dir = std::env::temp_dir().join(format!("volt-waves-hint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(WAVES_DIR)).expect("dizin");
        let src = dir.join("fsm.volt");
        std::fs::write(
            &src,
            "enum Mode { Idle, Run }\n\npub module Fsm {\n    in  clk  : clock\n    in  go   : bool\n    \
             out mode : Mode\n    out n    : u4\n    reg m : Mode = Mode::Idle\n    reg c : u4 = 0\n    \
             on clk {\n        if go { m <= Mode::Run }\n        c <= c + 1\n    }\n    mode = m\n    n = c\n}\n",
        )
        .expect("yaz");
        let compiled = crate::compile(&src, true, volt_sv_emit::SvaMode::None)
            .unwrap_or_else(|_| panic!("derlenmeli"));
        assert_eq!(compiled.errors(), 0);
        let module = super::super::modules_of(&compiled.ast)[0];
        let ports = tbgen::collect_sim_ports(&compiled.ast, module);
        let waves = GroupWaves {
            sim_dir: &dir,
            module: "Fsm",
            ports: &ports,
            compiled: &compiled,
        };

        // Act + Assert: VCD yokken satır yok.
        assert_eq!(waves.session_hint("t"), None);
        std::fs::write(
            dir.join(WAVES_DIR).join("Fsm-t.vcd"),
            "$enddefinitions $end\n",
        )
        .expect("yaz");
        let hint = waves.session_hint("t").expect("satır");

        // Assert
        assert!(hint.starts_with("gtkwave "), "{hint}");
        assert!(hint.ends_with("Fsm-t.gtkw"), "{hint}");
        let gtkw = std::fs::read_to_string(dir.join(WAVES_DIR).join("Fsm-t.gtkw")).expect("gtkw");
        for line in [
            "@28\nTOP.Fsm.clk\n",
            "@28\nTOP.Fsm.go\n",
            "@24\nTOP.Fsm.n[3:0]\n",
            "@2028\nTOP.Fsm.mode",
        ] {
            assert!(gtkw.contains(line), "{line:?}: {gtkw}");
        }
        // enum portu çevirisiz ikinci kez listelenmez.
        assert!(!gtkw.contains("@24\nTOP.Fsm.mode"), "{gtkw}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prepare_dir_removes_only_this_modules_recordings() {
        // Arrange
        let dir = std::env::temp_dir().join(format!("volt-waves-prep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let waves = dir.join(WAVES_DIR);
        std::fs::create_dir_all(waves.join("A-t.filters")).expect("dizin");
        for f in ["A-t.vcd", "A-t.gtkw", "B-t.vcd"] {
            std::fs::write(waves.join(f), "x").expect("yaz");
        }

        // Act
        prepare_dir(&dir, "A");

        // Assert
        assert!(!waves.join("A-t.vcd").exists());
        assert!(!waves.join("A-t.gtkw").exists());
        assert!(!waves.join("A-t.filters").exists());
        assert!(waves.join("B-t.vcd").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
