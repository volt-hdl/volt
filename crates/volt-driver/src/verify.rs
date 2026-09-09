//! `volt verify` — kontratları SymbiYosys ile kanıtlar (F4b).
//!
//! Boru hattı: derle (SVA GÖMÜLÜ — bkz. volt-sv-emit/src/sby.rs
//! gerekçesi) → `build/formal/<modul>.sv` + `.sby` üret → `sby -f`
//! koştur → çıktıyı yorumla. Çıkış kodları (cli-contract.md §2):
//! 0 tüm özellikler doğrulandı, 1 derleme hatası, 3 sby yok / araç
//! hatası (IoError), 6 karşı örnek (VerifyFailure).
//!
//! sby bulunamadığında kurulum yardımı UX Anayasası biçiminde yazılır;
//! ayrıntılar `volt explain verify-setup` konusunda yaşar.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use volt_diagnostics::{
    lstr, render_human, render_short, Diagnostic, ErrorCode, LabeledSpan, NoteKind,
};
use volt_sv_emit::{sby_config, SbyOptions, SvaMode, SvaProp};

use crate::{compile, render_diagnostics, OutputFormat};

/// `sby` çıktısının tek koşudaki özeti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SbyOutcome {
    /// `DONE (PASS ...)` — tüm özellikler doğrulandı.
    Pass,
    /// `DONE (FAIL ...)` — karşı örnek bulundu.
    Fail(SbyFailure),
    /// Statü satırı yok ya da `DONE (ERROR ...)` — araç hatası.
    Error,
}

/// FAIL logundan çıkarılan konum bilgisi; alanlar log biçimine göre
/// eksik kalabilir (o durumda ilk kontrata düşülür).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SbyFailure {
    /// Üretilen .sv dosyasındaki 1-tabanlı satır (`bad.sv:23.5-...`).
    pub(crate) sv_line: Option<usize>,
    /// İhlalin görüldüğü BMC adımı (döngü).
    pub(crate) step: Option<u32>,
}

pub(crate) fn verify(
    file: &Path,
    target_dir: &Path,
    format: OutputFormat,
    opts: SbyOptions,
) -> ExitCode {
    let start = Instant::now();
    if format == OutputFormat::Human {
        eprintln!(
            "{}",
            lstr!(
                en: "   Verifying {}", file.display();
                tr: "  Doğrulanıyor {}", file.display()
            )
        );
    }

    let mut compiled = match compile(file, true, SvaMode::Immediate) {
        Ok(c) => c,
        Err(code) => return code,
    };
    render_diagnostics(&compiled, format);

    let Some(sv) = compiled.sv.clone() else {
        if format == OutputFormat::Human {
            eprintln!(
                "{}",
                lstr!(
                    en: "     Error: verify failed due to {} error(s), {} warning(s)",
                        compiled.errors(), compiled.warnings();
                    tr: "     Hata: {} hata, {} uyarı nedeniyle doğrulama başarısız",
                        compiled.errors(), compiled.warnings()
                )
            );
        }
        if format == OutputFormat::Json {
            crate::print_json_envelope("verify", &compiled, &[], start);
        }
        return ExitCode::from(1);
    };

    // Kontratlı modüller, kaynak sırası korunarak teklenir.
    let mut modules: Vec<String> = Vec::new();
    for prop in &compiled.sva_props {
        if !modules.contains(&prop.module_name) {
            modules.push(prop.module_name.clone());
        }
    }
    if modules.is_empty() {
        if format == OutputFormat::Human {
            eprintln!(
                "{}",
                lstr!(
                    en: "        Note no contracts found in '{}' — nothing to verify\n        \
                         Help add an 'invariant:', 'ensures:' or 'assert:' line to the module",
                        file.display();
                    tr: "           Not '{}' içinde kontrat yok — doğrulanacak bir şey yok\n        \
                         Çözüm modüle 'invariant:', 'ensures:' ya da 'assert:' satırı ekleyin",
                        file.display()
                )
            );
        }
        if format == OutputFormat::Json {
            crate::print_json_envelope("verify", &compiled, &[], start);
        }
        return ExitCode::SUCCESS;
    }

    // ── ADIM 1: build/formal/ altına .sv + .sby üret ──
    let formal_dir = target_dir.join("formal");
    if let Err(err) = std::fs::create_dir_all(&formal_dir) {
        return io_error(&formal_dir, &err);
    }
    let mut artifacts = Vec::new();
    for module in &modules {
        let stem = module.to_lowercase();
        let sv_path = formal_dir.join(format!("{stem}.sv"));
        if let Err(err) = std::fs::write(&sv_path, &sv) {
            return io_error(&sv_path, &err);
        }
        let sby_path = formal_dir.join(format!("{stem}.sby"));
        // İki+ saatli modül: Yosys clk2fflogic akışı için multiclock on.
        let mut mod_opts = opts;
        mod_opts.multiclock = compiled.multiclock_modules.iter().any(|m| m == module);
        let config = sby_config(module, &format!("{stem}.sv"), &mod_opts);
        if let Err(err) = std::fs::write(&sby_path, config) {
            return io_error(&sby_path, &err);
        }
        artifacts.push(sv_path.display().to_string());
        artifacts.push(sby_path.display().to_string());
    }

    // ── ADIM 2: sby'yi bul (VOLT_SBY > PATH) ──
    let Some(sby) = find_sby() else {
        print_sby_not_found();
        return ExitCode::from(3);
    };

    // ── ADIM 3: modül başına koştur ve yorumla ──
    let mut any_fail = false;
    let mut any_error = false;
    for module in &modules {
        let stem = module.to_lowercase();
        let output = match std::process::Command::new(&sby)
            .arg("-f")
            .arg(format!("{stem}.sby"))
            .current_dir(&formal_dir)
            .output()
        {
            Ok(o) => o,
            Err(err) => {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot run '{}': {}", sby.display(), err;
                        tr: "hata: '{}' çalıştırılamadı: {}", sby.display(), err
                    )
                );
                return ExitCode::from(3);
            }
        };
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        match interpret_sby_output(&log) {
            SbyOutcome::Pass => {}
            SbyOutcome::Fail(failure) => {
                any_fail = true;
                let cex = copy_counterexample(&formal_dir, &stem);
                if let Some(c) = &cex {
                    artifacts.push(c.display().to_string());
                }
                let diag = counterexample_diagnostic(
                    module,
                    &failure,
                    &sv,
                    &compiled.sva_props,
                    cex.as_deref(),
                );
                match format {
                    OutputFormat::Human => eprintln!("{}", render_human(&diag, &compiled.map)),
                    OutputFormat::Short => eprintln!("{}", render_short(&diag, &compiled.map)),
                    OutputFormat::Json => {}
                }
                compiled.diagnostics.push(diag);
            }
            SbyOutcome::Error => {
                any_error = true;
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: SymbiYosys reported a tool error for module '{module}' \
                             (exit code {:?})\n  = help: re-run '{} -f {stem}.sby' in '{}' \
                             to see the full log",
                            output.status.code(), sby.display(), formal_dir.display();
                        tr: "hata: SymbiYosys '{module}' modülü için araç hatası bildirdi \
                             (çıkış kodu {:?})\n  = çözüm: tam log için '{} -f {stem}.sby' \
                             komutunu '{}' içinde yeniden çalıştırın",
                            output.status.code(), sby.display(), formal_dir.display()
                    )
                );
            }
        }
    }

    if format == OutputFormat::Json {
        crate::print_json_envelope("verify", &compiled, &artifacts, start);
    }
    if any_fail {
        if format == OutputFormat::Human {
            eprintln!(
                "{}",
                lstr!(
                    en: "        Next: volt explain E5001   (how to read a counterexample)";
                    tr: "    Sıradaki: volt explain E5001   (karşı örnek nasıl okunur)"
                )
            );
        }
        return ExitCode::from(6);
    }
    if any_error {
        return ExitCode::from(3);
    }
    if format == OutputFormat::Human {
        let props = compiled.sva_props.len();
        eprintln!(
            "{}",
            lstr!(
                en: "    Finished {:.2}s\n      Result {props} propert{} verified \
                     ({}, depth {})",
                    start.elapsed().as_secs_f64(),
                    if props == 1 { "y" } else { "ies" },
                    opts.mode.as_str(), opts.depth;
                tr: "    Tamamlandı {:.2}s\n       Sonuç {props} özellik doğrulandı \
                     ({}, derinlik {})",
                    start.elapsed().as_secs_f64(), opts.mode.as_str(), opts.depth
            )
        );
    }
    ExitCode::SUCCESS
}

/// G/Ç hatası: mesaj + çıkış kodu 3 (cli-contract.md §2).
fn io_error(path: &Path, err: &std::io::Error) -> ExitCode {
    eprintln!(
        "{}",
        lstr!(
            en: "error: cannot write '{}': {}", path.display(), err;
            tr: "hata: '{}' yazılamadı: {}", path.display(), err
        )
    );
    ExitCode::from(3)
}

/// UX Anayasası biçiminde kurulum yardımı (goal ADIM 3).
fn print_sby_not_found() {
    eprintln!(
        "{}",
        lstr!(
            en: "error: SymbiYosys not found\n\n  \
                 = reason: 'volt verify' uses SymbiYosys for formal verification\n  \
                 = help: install options:\n      \
                 Linux:   apt install yosys z3, then pip install symbiyosys\n      \
                 Docker:  docker pull hdlc/formal\n      \
                 Windows: use WSL or Docker\n  \
                 = note: 'volt build' and 'volt check' do not need SymbiYosys\n  \
                 = for more: volt explain verify-setup";
            tr: "hata: SymbiYosys bulunamadı\n\n  \
                 = neden: 'volt verify' formal doğrulama için SymbiYosys kullanır\n  \
                 = çözüm: kurulum seçenekleri:\n      \
                 Linux:   apt install yosys z3, ardından pip install symbiyosys\n      \
                 Docker:  docker pull hdlc/formal\n      \
                 Windows: WSL ya da Docker kullanın\n  \
                 = not: 'volt build' ve 'volt check' SymbiYosys gerektirmez\n  \
                 = daha fazla: volt explain verify-setup"
        )
    );
}

/// `sby` çalıştırılabilir dosyası: önce VOLT_SBY, sonra PATH.
fn find_sby() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VOLT_SBY") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in ["sby", "sby.exe", "sby.bat", "sby.cmd"] {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// sby log metnini özetler. Statü `DONE (PASS/FAIL/...)` satırından,
/// konum `Assert/Assume failed in ...: dosya.sv:SATIR...` satırından,
/// döngü `... step N` izlerinden okunur.
pub(crate) fn interpret_sby_output(log: &str) -> SbyOutcome {
    let mut status: Option<bool> = None;
    let mut last_step: Option<u32> = None;
    let mut failure: Option<SbyFailure> = None;

    for line in log.lines() {
        if let Some(step) = parse_step(line) {
            last_step = Some(step);
        }
        if (line.contains("Assert failed in") || line.contains("Assume failed in"))
            && failure.is_none()
        {
            failure = Some(SbyFailure {
                sv_line: parse_sv_line(line),
                step: last_step,
            });
        }
        if line.contains("DONE (PASS") {
            status = Some(true);
        } else if line.contains("DONE (FAIL") {
            status = Some(false);
        }
    }
    match status {
        Some(true) => SbyOutcome::Pass,
        Some(false) => SbyOutcome::Fail(failure.unwrap_or_default()),
        None => SbyOutcome::Error,
    }
}

/// "Checking assertions in step 7.." → 7. Satırdaki son `step N`.
fn parse_step(line: &str) -> Option<u32> {
    let idx = line.rfind("step ")?;
    let digits: String = line[idx + 5..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// "... bad.sv:23.5-23.30" → 23. `.sv:` sonrası ilk sayı.
fn parse_sv_line(line: &str) -> Option<usize> {
    let idx = line.find(".sv:")?;
    let digits: String = line[idx + 4..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Üretilen SV'de `line_1based` konumundan yukarı tarayarak property
/// adını çıkarır — smtbmc konumu bazen `assert property (adı);`
/// satırını, bazen property gövdesindeki ifadeyi işaret eder.
pub(crate) fn prop_name_at(sv: &str, line_1based: usize) -> Option<String> {
    let lines: Vec<&str> = sv.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let idx = line_1based.saturating_sub(1).min(lines.len() - 1);
    lines[..=idx].iter().rev().find_map(|l| prop_in_line(l))
}

/// Tek satırdan property adı. Öncelik `// volt:<ad>` işaretidir
/// (SvaMode::Immediate); `assert property (inv_0);` ve
/// `property inv_0;` biçimleri de tanınır.
fn prop_in_line(line: &str) -> Option<String> {
    if let Some(idx) = line.rfind("// volt:") {
        let name: String = line[idx + 8..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            return Some(name);
        }
    }
    let t = line.trim();
    for verb in ["assert property (", "assume property (", "cover property ("] {
        if let Some(rest) = t.strip_prefix(verb) {
            return rest.split(')').next().map(str::to_string);
        }
    }
    let rest = t.strip_prefix("property ")?;
    let name = rest.split(';').next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// FAIL → E5001 tanısı. Konum eşlenemezse modülün ilk kontratına
/// düşülür (tanı yine 5 parça taşır).
fn counterexample_diagnostic(
    module: &str,
    failure: &SbyFailure,
    sv: &str,
    props: &[SvaProp],
    cex: Option<&Path>,
) -> Diagnostic {
    let by_name = failure
        .sv_line
        .and_then(|line| prop_name_at(sv, line))
        .and_then(|name| {
            props
                .iter()
                .find(|p| p.module_name == module && p.name == name)
        });
    let prop = by_name
        .or_else(|| props.iter().find(|p| p.module_name == module))
        .expect("kontratlı modülün en az bir SvaProp'u olmalı");

    let label = match failure.step {
        Some(step) => lstr!(
            en: "violated at cycle {step}";
            tr: "{step}. döngüde ihlal edildi"
        ),
        None => lstr!(en: "violated"; tr: "ihlal edildi"),
    };
    let mut diag = Diagnostic::error(
        ErrorCode::E5001,
        lstr!(en: "contract violated"; tr: "kontrat ihlal edildi"),
        LabeledSpan::primary(prop.span, label),
        lstr!(
            en: "open the counterexample with 'gtkwave' or 'surfer'";
            tr: "karşı örneği 'gtkwave' ya da 'surfer' ile açın"
        ),
    )
    .with_note(
        NoteKind::Reason,
        lstr!(
            en: "the '{}' contract of module '{module}' does not hold for every reachable state",
                prop.keyword;
            tr: "'{module}' modülünün '{}' kontratı erişilebilir her durumda sağlanmıyor",
                prop.keyword
        ),
    );
    if let Some(cex) = cex {
        diag = diag.with_note(NoteKind::Counterexample, cex.display().to_string());
    }
    diag
}

/// sby'nin `<modul>/engine_0/trace.vcd` izini `<modul>_cex.vcd` olarak
/// kopyalar; iz üretilmediyse `None`.
fn copy_counterexample(formal_dir: &Path, stem: &str) -> Option<PathBuf> {
    let workdir = formal_dir.join(stem);
    let engine_dir = workdir.join("engine_0");
    let trace = ["trace.vcd", "trace_tb.vcd"]
        .iter()
        .map(|n| engine_dir.join(n))
        .find(|p| p.is_file())?;
    let dest = formal_dir.join(format!("{stem}_cex.vcd"));
    std::fs::copy(&trace, &dest).ok()?;
    Some(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_FAIL_LOG: &str = "\
SBY 12:00:01 [counter] engine_0: ##   0:00:00  Checking assumptions in step 6..
SBY 12:00:01 [counter] engine_0: ##   0:00:00  Checking assertions in step 7..
SBY 12:00:01 [counter] engine_0: ##   0:00:00  BMC failed!
SBY 12:00:01 [counter] engine_0: ##   0:00:00  Assert failed in Counter: counter.sv:23.9-23.28
SBY 12:00:01 [counter] engine_0: ##   0:00:00  Writing trace to VCD file: engine_0/trace.vcd
SBY 12:00:01 [counter] DONE (FAIL, rc=2)
";

    const SAMPLE_PASS_LOG: &str = "\
SBY 12:00:01 [counter] engine_0: ##   0:00:00  Checking assertions in step 19..
SBY 12:00:01 [counter] engine_0: ##   0:00:00  Status: passed
SBY 12:00:01 [counter] DONE (PASS, rc=0)
";

    #[test]
    fn interpret_fail_log_extracts_line_and_step() {
        let SbyOutcome::Fail(failure) = interpret_sby_output(SAMPLE_FAIL_LOG) else {
            panic!("FAIL bekleniyor");
        };
        assert_eq!(failure.sv_line, Some(23));
        assert_eq!(failure.step, Some(7));
    }

    #[test]
    fn interpret_pass_log() {
        assert_eq!(interpret_sby_output(SAMPLE_PASS_LOG), SbyOutcome::Pass);
    }

    #[test]
    fn interpret_empty_log_is_tool_error() {
        assert_eq!(interpret_sby_output(""), SbyOutcome::Error);
        assert_eq!(
            interpret_sby_output("SBY 12:00:01 [x] DONE (ERROR, rc=16)\n"),
            SbyOutcome::Error
        );
    }

    #[test]
    fn fail_log_without_location_still_fails() {
        let log = "SBY 12:00:01 [m] DONE (FAIL, rc=2)\n";
        assert_eq!(
            interpret_sby_output(log),
            SbyOutcome::Fail(SbyFailure::default())
        );
    }

    #[test]
    fn prop_name_found_on_assert_line() {
        let sv = "module Counter (\n);\n    property inv_0;\n        @(posedge clk)\n        \
                  count_r <= 8'd10;\n    endproperty\n    assert property (inv_0);\nendmodule\n";
        // Satır 7: assert property satırının kendisi.
        assert_eq!(prop_name_at(sv, 7), Some("inv_0".to_string()));
    }

    #[test]
    fn prop_name_found_scanning_up_from_property_body() {
        let sv = "    property ens_1;\n        @(posedge clk)\n        a |-> b;\n    endproperty\n";
        // Satır 3: property gövdesindeki ifade — yukarı tarama bulmalı.
        assert_eq!(prop_name_at(sv, 3), Some("ens_1".to_string()));
    }

    #[test]
    fn prop_name_via_volt_marker_on_immediate_assert() {
        let sv = "    always @(posedge clk)\n        \
                  if (!(rst)) assert (count_r <= 8'd10); // volt:inv_0\n";
        assert_eq!(prop_name_at(sv, 2), Some("inv_0".to_string()));
    }

    #[test]
    fn prop_name_out_of_range_clamps_to_last_line() {
        let sv = "    assume property (req_0);\n";
        assert_eq!(prop_name_at(sv, 999), Some("req_0".to_string()));
    }

    #[test]
    fn parse_helpers_reject_garbage() {
        assert_eq!(parse_step("no numbers here"), None);
        assert_eq!(parse_sv_line("Assert failed somewhere else"), None);
        assert_eq!(prop_in_line("assign x = y;"), None);
    }
}
