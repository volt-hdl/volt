//! `volt verify` — kontratları SymbiYosys ile kanıtlar (F4b, ADR-0055).
//!
//! Boru hattı: derle (SVA GÖMÜLÜ — bkz. volt-sv-emit/src/sby.rs
//! gerekçesi) → `build/formal/<iş>.sv` + tek `<iş>.sby` (kontratlı modül
//! başına bir sby GÖREVİ) üret → `sby -j N -f <iş>.sby` koştur →
//! görev akışını yorumla → raporu KAYNAK SIRASINDA yaz. Çıkış kodları
//! (cli-contract.md §2, ADR-0075): 0 tüm özellikler doğrulandı, 1 derleme
//! hatası, 3 sby yok / araç hatası (sby ERROR), 6 karşı örnek (FAIL),
//! 7 kanıtlanamadı (prove UNKNOWN: tümevarım tamamlanamadı), 8 zaman
//! aşımı (TIMEOUT). Birden çok durumda öncelik 6 > 3 > 8 > 7.
//!
//! Paralellik birimi MODÜLDÜR, kontrat değil: bir modülün tüm kontratları
//! tek BMC koşusunda birlikte denetlenir; kontrat başına ayrı koşu toplam
//! işi 3× büyütüp duvar süresini kısaltmadı (ADR-0055 ölçümü). `-j 1`
//! görevleri sırayla koşturur; sonuçlar her `-j` için aynıdır.
//!
//! sby bulunamadığında kurulum yardımı UX Anayasası biçiminde yazılır;
//! ayrıntılar `volt explain verify-setup` konusunda yaşar.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use volt_diagnostics::{
    lstr, render_human, render_short, Diagnostic, ErrorCode, LabeledSpan, NoteKind,
};
use volt_sv_emit::{sby_config_tasks, SbyOptions, SbyTask, SvaMode, SvaProp};

use crate::extern_stage::{compile_for_tool, stage_extern_sources};
use crate::verify_jobs::{run_sby_tasks, Jobs, RunConfig, TaskSpec, TaskStatus};
use crate::verify_report::{progress_line, summary_block, verify_json, ModuleOutcome, PropInfo};
use crate::{render_diagnostics, OutputFormat};

/// `sby` çıktısının tek görevdeki özeti (ADR-0075: sby'nin beş durumu
/// ayrı raporlanır, ayrı çıkış koduyla).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SbyOutcome {
    /// `DONE (PASS ...)` — tüm özellikler doğrulandı.
    Pass,
    /// `DONE (FAIL ...)` — karşı örnek bulundu.
    Fail(SbyFailure),
    /// `DONE (UNKNOWN ...)` — prove kipi: temel durum geçti, tümevarım
    /// adımı başarısız; özellik doğru olabilir ama tümevarımsal değil.
    /// Konum tümevarım izindeki başarısız iddiadır.
    Unknown(SbyFailure),
    /// `DONE (TIMEOUT ...)` — `--timeout` süresi doldu.
    Timeout,
    /// Statü satırı yok ya da `DONE (ERROR ...)` — gerçek araç hatası.
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

/// `volt verify` paralellik ayarları (cli-contract.md §8a).
#[derive(Debug, Clone, Copy)]
pub(crate) struct VerifyArgs {
    pub(crate) jobs: Jobs,
    pub(crate) fail_fast: bool,
}

pub(crate) fn verify(
    file: &Path,
    target_dir: &Path,
    format: OutputFormat,
    opts: SbyOptions,
    args: VerifyArgs,
) -> ExitCode {
    let start = Instant::now();
    let human = format == OutputFormat::Human;
    if human {
        eprintln!(
            "{}",
            lstr!(
                en: "   Verifying {}", file.display();
                tr: "  Doğrulanıyor {}", file.display()
            )
        );
    }

    let mut compiled = match compile_for_tool(file, SvaMode::Immediate, "verify") {
        Ok(c) => c,
        Err(code) => return code,
    };
    render_diagnostics(&compiled, format);

    let Some(sv) = compiled.sv.clone() else {
        if human {
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
    let modules = contract_modules(&compiled.sva_props);
    if modules.is_empty() {
        if human {
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

    // ── ADIM 1: build/formal/ altına tek .sv + görevli .sby üret ──
    let formal_dir = target_dir.join("formal");
    if let Err(err) = std::fs::create_dir_all(&formal_dir) {
        return io_error(&formal_dir, &err);
    }
    let job = job_name(file);
    let sv_name = format!("{job}.sv");
    let sby_name = format!("{job}.sby");
    let sv_path = formal_dir.join(&sv_name);
    if let Err(err) = std::fs::write(&sv_path, &sv) {
        return io_error(&sv_path, &err);
    }
    // Extern gövdeleri (ADR-0076) sby'nin [files]/[script] listesinde,
    // üretilen SV'den önce.
    let reserved = [sv_name.as_str(), sby_name.as_str()];
    let extern_files = match stage_extern_sources(&compiled.extern_sources, &formal_dir, &reserved)
    {
        Ok(names) => names,
        Err(code) => return code,
    };
    let tasks = sby_tasks(&modules, &compiled.multiclock_modules);
    let sby_path = formal_dir.join(&sby_name);
    let sby_text = sby_config_tasks(&tasks, &sv_name, &extern_files, &opts);
    if let Err(err) = std::fs::write(&sby_path, sby_text) {
        return io_error(&sby_path, &err);
    }
    let mut artifacts = vec![
        sv_path.display().to_string(),
        sby_path.display().to_string(),
    ];

    // ── ADIM 2: sby'yi bul (VOLT_SBY > PATH) ──
    let Some(sby) = find_sby() else {
        print_sby_not_found();
        return ExitCode::from(3);
    };

    // ── ADIM 3: tek sby süreci, modül başına görev, -j N ──
    let jobs = args.jobs.resolve();
    let specs: Vec<TaskSpec> = tasks
        .iter()
        .map(|t| TaskSpec {
            name: t.name.clone(),
            workdir: format!("{job}_{}", t.name),
        })
        .collect();
    let prop_counts: Vec<usize> = modules
        .iter()
        .map(|m| {
            compiled
                .sva_props
                .iter()
                .filter(|p| &p.module_name == m)
                .count()
        })
        .collect();
    let total = specs.len();
    let run = RunConfig {
        sby: &sby,
        sby_file: &sby_name,
        cwd: &formal_dir,
        jobs,
        fail_fast: args.fail_fast,
    };
    let report = match run_sby_tasks(&run, &specs, |done, idx, status| {
        if human {
            eprintln!(
                "{}",
                progress_line(done, total, &modules[idx], prop_counts[idx], status)
            );
        }
    }) {
        Ok(r) => r,
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

    // ── ADIM 4: rapor — KAYNAK SIRASINDA (tamamlanma sırası değil) ──
    let mut outcomes: Vec<ModuleOutcome> = Vec::with_capacity(total);
    let mut any_fail = false;
    let mut any_unknown = false;
    let mut any_timeout = false;
    let mut any_error = false;
    for (idx, module) in modules.iter().enumerate() {
        let result = &report.tasks[idx];
        let props: Vec<PropInfo> = compiled
            .sva_props
            .iter()
            .filter(|p| &p.module_name == module)
            .map(|p| PropInfo {
                name: p.name.clone(),
                keyword: p.keyword,
            })
            .collect();
        let mut failed_prop = None;
        match &result.status {
            TaskStatus::Done {
                outcome: SbyOutcome::Pass,
                ..
            }
            | TaskStatus::Skipped => {}
            TaskStatus::Done {
                outcome: SbyOutcome::Fail(failure),
                ..
            } => {
                any_fail = true;
                let cex = copy_trace(
                    &formal_dir,
                    &specs[idx].workdir,
                    &tasks[idx].name,
                    TraceKind::Counterexample,
                );
                if let Some(c) = &cex {
                    artifacts.push(c.display().to_string());
                }
                let (prop, diag) = counterexample_diagnostic(
                    module,
                    failure,
                    &sv,
                    &compiled.sva_props,
                    cex.as_deref(),
                );
                failed_prop = Some(prop);
                emit_diagnostic(&diag, &compiled, format);
                compiled.diagnostics.push(diag);
            }
            TaskStatus::Done {
                outcome: SbyOutcome::Unknown(failure),
                ..
            } => {
                any_unknown = true;
                let trace = copy_trace(
                    &formal_dir,
                    &specs[idx].workdir,
                    &tasks[idx].name,
                    TraceKind::Induction,
                );
                if let Some(t) = &trace {
                    artifacts.push(t.display().to_string());
                }
                let (prop, diag) = unproven_diagnostic(
                    module,
                    failure,
                    &sv,
                    &compiled.sva_props,
                    trace.as_deref(),
                    opts.depth,
                );
                failed_prop = Some(prop);
                emit_diagnostic(&diag, &compiled, format);
                compiled.diagnostics.push(diag);
            }
            TaskStatus::Done {
                outcome: SbyOutcome::Timeout,
                ..
            } => {
                any_timeout = true;
                print_timeout(module, opts.timeout);
            }
            TaskStatus::Done {
                outcome: SbyOutcome::Error,
                ..
            }
            | TaskStatus::Missing => {
                any_error = true;
                print_tool_error(
                    module,
                    &sby,
                    &sby_name,
                    &tasks[idx].name,
                    &formal_dir,
                    report.exit_code,
                );
            }
        }
        outcomes.push(ModuleOutcome {
            module: module.clone(),
            task: tasks[idx].name.clone(),
            props,
            status: result.status.clone(),
            failed_prop,
        });
    }
    if any_error && !report.global_log.trim().is_empty() {
        print_global_log(&report.global_log);
    }

    if format == OutputFormat::Json {
        let mut envelope = crate::json_envelope("verify", &compiled, &artifacts, start);
        envelope["verify"] = verify_json(&outcomes, &opts, jobs, args.fail_fast);
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).expect("JSON zarfı")
        );
    }
    if human {
        eprintln!(
            "{}",
            lstr!(
                en: "    Finished {:.2}s", start.elapsed().as_secs_f64();
                tr: "    Tamamlandı {:.2}s", start.elapsed().as_secs_f64()
            )
        );
        eprintln!("{}", summary_block(&outcomes, &opts, jobs, start.elapsed()));
        if any_fail {
            eprintln!(
                "{}",
                lstr!(
                    en: "        Next: volt explain E5001   (how to read a counterexample)";
                    tr: "    Sıradaki: volt explain E5001   (karşı örnek nasıl okunur)"
                )
            );
        } else if any_unknown {
            eprintln!(
                "{}",
                lstr!(
                    en: "        Next: volt explain E5002   (why a true property can fail induction)";
                    tr: "    Sıradaki: volt explain E5002   (doğru bir özellik tümevarımda neden kalır)"
                )
            );
        }
    }
    verify_exit_code(any_fail, any_error, any_timeout, any_unknown)
}

/// Çıkış kodu önceliği (ADR-0075, cli-contract.md §2): karşı örnek (6) >
/// araç hatası (3) > zaman aşımı (8) > kanıtlanamadı (7) > başarı (0).
/// En kesin ve en eyleme dönük sonuç baskındır.
fn verify_exit_code(fail: bool, error: bool, timeout: bool, unknown: bool) -> ExitCode {
    let code = if fail {
        6
    } else if error {
        3
    } else if timeout {
        8
    } else if unknown {
        7
    } else {
        0
    };
    ExitCode::from(code)
}

fn emit_diagnostic(diag: &Diagnostic, compiled: &crate::Compiled, format: OutputFormat) {
    match format {
        OutputFormat::Human => eprintln!("{}", render_human(diag, &compiled.map)),
        OutputFormat::Short => eprintln!("{}", render_short(diag, &compiled.map)),
        OutputFormat::Json => {}
    }
}

/// Kontratlı modüller, kaynak sırasında ve teklenmiş.
fn contract_modules(props: &[SvaProp]) -> Vec<String> {
    let mut modules: Vec<String> = Vec::new();
    for prop in props {
        if !modules.contains(&prop.module_name) {
            modules.push(prop.module_name.clone());
        }
    }
    modules
}

/// Modül → sby görevi; görev adı küçük harf modül adıdır, çakışırsa
/// (`Foo`/`foo`) sıra numarası eklenir — sby görev adları tekil olmalı.
fn sby_tasks(modules: &[String], multiclock_modules: &[String]) -> Vec<SbyTask> {
    let mut used: Vec<String> = Vec::new();
    modules
        .iter()
        .map(|module| {
            let base = sanitize(&module.to_lowercase());
            let mut name = base.clone();
            let mut n = 2;
            while used.contains(&name) {
                name = format!("{base}_{n}");
                n += 1;
            }
            used.push(name.clone());
            SbyTask {
                name,
                top: module.clone(),
                multiclock: multiclock_modules.iter().any(|m| m == module),
            }
        })
        .collect()
}

/// `.sby` iş adı: girdi dosyasının kök adı (`top.volt` → `top`); sby
/// çalışma dizinleri `<iş>_<görev>` olur.
fn job_name(file: &Path) -> String {
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = sanitize(&stem);
    if name.is_empty() {
        "verify".to_string()
    } else {
        name
    }
}

/// sby görev/iş adı alfabesi: `[A-Za-z0-9_]`, diğerleri `_`.
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Araç hatası mesajı: yeniden koşturma ipucu görevi tek başına seçer.
fn print_tool_error(
    module: &str,
    sby: &Path,
    sby_name: &str,
    task: &str,
    formal_dir: &Path,
    exit_code: Option<i32>,
) {
    let code = exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "-".into());
    eprintln!(
        "{}",
        lstr!(
            en: "error: SymbiYosys reported a tool error for module '{module}' \
                 (sby status ERROR, exit code {code})\n  \
                 = note: the tool itself failed; this says nothing about the contracts\n  \
                 = help: re-run '{} -f {sby_name} {task}' in '{}' to see the full log",
                sby.display(), formal_dir.display();
            tr: "hata: SymbiYosys '{module}' modülü için araç hatası bildirdi \
                 (sby durumu ERROR, çıkış kodu {code})\n  \
                 = not: aracın kendisi başarısız oldu; bu kontratlar hakkında bir şey söylemez\n  \
                 = çözüm: tam log için '{} -f {sby_name} {task}' komutunu '{}' içinde \
                 yeniden çalıştırın",
                sby.display(), formal_dir.display()
        )
    );
}

/// sby `DONE (TIMEOUT)`: süre doldu, sonuç yok (çıkış 8).
fn print_timeout(module: &str, timeout: Option<u32>) {
    let after = timeout
        .map(|s| lstr!(en: " after {s}s"; tr: " {s} sn sonra"))
        .unwrap_or_default();
    eprintln!(
        "{}",
        lstr!(
            en: "error: SymbiYosys timed out for module '{module}'{after} (sby status TIMEOUT)\n  \
                 = note: no result either way — the contracts were neither proven nor refuted\n  \
                 = help: raise --timeout, lower --depth, or try another --engine (boolector is often faster)";
            tr: "hata: SymbiYosys '{module}' modülü için{after} zaman aşımına uğradı (sby durumu TIMEOUT)\n  \
                 = not: iki yönde de sonuç yok — kontratlar ne kanıtlandı ne çürütüldü\n  \
                 = çözüm: --timeout değerini artırın, --depth değerini düşürün ya da başka bir --engine deneyin (boolector çoğu zaman daha hızlı)"
        )
    );
}

/// Göreve ait olmayan sby satırları (yapılandırma hatası gibi) — en
/// fazla 20 satır, araç hatası varsa bir kez.
fn print_global_log(log: &str) {
    let lines: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
    let shown = lines.len().min(20);
    eprintln!(
        "{}",
        lstr!(en: "  = note: sby output:"; tr: "  = not: sby çıktısı:")
    );
    for line in &lines[..shown] {
        eprintln!("      {line}");
    }
    if lines.len() > shown {
        eprintln!("      ...");
    }
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
    #[derive(Clone, Copy)]
    enum Done {
        Pass,
        Fail,
        Unknown,
        Timeout,
        Error,
    }
    let mut status: Option<Done> = None;
    let mut last_step: Option<u32> = None;
    let mut failure: Option<SbyFailure> = None;

    for line in log.lines() {
        if let Some(step) = parse_step(line) {
            last_step = Some(step);
        }
        // Cover kipinde başarısızlık "Unreached cover statement at dosya.sv:N"
        // satırıdır (ADR-0066: otomatik cover'lar bu kipi sık kullandırır).
        if (line.contains("Assert failed in")
            || line.contains("Assume failed in")
            || line.contains("Unreached cover statement at"))
            && failure.is_none()
        {
            failure = Some(SbyFailure {
                sv_line: parse_sv_line(line),
                step: last_step,
            });
        }
        let done = [
            ("DONE (PASS", Done::Pass),
            ("DONE (FAIL", Done::Fail),
            ("DONE (UNKNOWN", Done::Unknown),
            ("DONE (TIMEOUT", Done::Timeout),
            ("DONE (ERROR", Done::Error),
        ];
        if let Some(&(_, d)) = done.iter().find(|(tag, _)| line.contains(tag)) {
            status = Some(d);
        }
    }
    match status {
        Some(Done::Pass) => SbyOutcome::Pass,
        Some(Done::Fail) => SbyOutcome::Fail(failure.unwrap_or_default()),
        // Tümevarım adım numarası kullanıcı döngüsü değildir: yalnız konum.
        Some(Done::Unknown) => SbyOutcome::Unknown(SbyFailure {
            sv_line: failure.and_then(|f| f.sv_line),
            step: None,
        }),
        Some(Done::Timeout) => SbyOutcome::Timeout,
        Some(Done::Error) | None => SbyOutcome::Error,
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

/// FAIL → (kontrat adı, E5001 tanısı). Konum eşlenemezse modülün ilk
/// kontratına düşülür (tanı yine 5 parça taşır).
fn counterexample_diagnostic(
    module: &str,
    failure: &SbyFailure,
    sv: &str,
    props: &[SvaProp],
    cex: Option<&Path>,
) -> (String, Diagnostic) {
    let prop = failed_prop(module, failure, sv, props);

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
    diag = with_origin_and_trace(diag, prop, cex);
    (prop.name.clone(), diag)
}

/// UNKNOWN → (kontrat adı, E5002 tanısı, ADR-0075). Konum tümevarım
/// izindeki başarısız iddiadır; eşlenemezse modülün ilk kontratı.
fn unproven_diagnostic(
    module: &str,
    failure: &SbyFailure,
    sv: &str,
    props: &[SvaProp],
    trace: Option<&Path>,
    depth: u32,
) -> (String, Diagnostic) {
    let prop = failed_prop(module, failure, sv, props);
    let diag = Diagnostic::error(
        ErrorCode::E5002,
        lstr!(en: "contract not proven: the induction step failed"; tr: "kontrat kanıtlanamadı: tümevarım adımı başarısız"),
        LabeledSpan::primary(
            prop.span,
            lstr!(en: "not inductive at depth {depth}"; tr: "{depth} derinliğinde tümevarımsal değil"),
        ),
        lstr!(
            en: "try a larger --depth, or add an invariant that makes the property inductive";
            tr: "daha büyük bir --depth deneyin ya da özelliği tümevarımsal yapan bir invariant ekleyin"
        ),
    )
    .with_note(
        NoteKind::Reason,
        lstr!(
            en: "no counterexample exists within {depth} cycles from reset, but the induction step starts from an arbitrary state — possibly unreachable — and the '{}' contract of module '{module}' fails from there (sby status UNKNOWN)",
                prop.keyword;
            tr: "reset'ten itibaren {depth} döngüde karşı örnek yok; ama tümevarım adımı keyfi — belki erişilemez — bir durumdan başlar ve '{module}' modülünün '{}' kontratı oradan bozulur (sby durumu UNKNOWN)",
                prop.keyword
        ),
    );
    let diag = with_origin_and_trace(diag, prop, None);
    // Tümevarım izi karşı örnek DEĞİLDİR (erişilemez durumdan başlayabilir):
    // "= counterexample:" etiketi yanıltırdı.
    let diag = match trace {
        Some(t) => diag.with_note(
            NoteKind::Note,
            lstr!(
                en: "induction trace (may start from an unreachable state): {}", t.display();
                tr: "tümevarım izi (erişilemez bir durumdan başlayabilir): {}", t.display()
            ),
        ),
        None => diag,
    };
    (prop.name.clone(), diag)
}

/// sby'nin başarısız iddia satırını kontrata eşler; eşlenemezse modülün
/// ilk kontratı (tanı yine 5 parça taşır).
fn failed_prop<'p>(
    module: &str,
    failure: &SbyFailure,
    sv: &str,
    props: &'p [SvaProp],
) -> &'p SvaProp {
    let by_name = failure
        .sv_line
        .and_then(|line| prop_name_at(sv, line))
        .and_then(|name| {
            props
                .iter()
                .find(|p| p.module_name == module && p.name == name)
        });
    by_name
        .or_else(|| props.iter().find(|p| p.module_name == module))
        .expect("kontratlı modülün en az bir SvaProp'u olmalı")
}

/// Otomatik kontratın kökeni (ADR-0066 §4) ve iz dosyası notu.
fn with_origin_and_trace(mut diag: Diagnostic, prop: &SvaProp, trace: Option<&Path>) -> Diagnostic {
    // ADR-0066 §4: kullanıcı yazmadığı kontratın nereden geldiğini görür.
    if let Some(auto) = &prop.auto {
        if auto.from != prop.span {
            diag = diag.with_secondary(
                auto.from,
                lstr!(en: "generated from here"; tr: "buradan üretildi"),
            );
        }
        diag = diag.with_note(
            NoteKind::Note,
            lstr!(
                en: "auto-generated {} contract '{}', generated from {}", auto.rule, auto.text, auto.subject;
                tr: "otomatik üretilmiş {} kontratı '{}', kaynağı: {}", auto.rule, auto.text, auto.subject
            ),
        );
    }
    if let Some(trace) = trace {
        diag = diag.with_note(NoteKind::Counterexample, trace.display().to_string());
    }
    diag
}

/// Kopyalanacak sby izi.
#[derive(Clone, Copy)]
enum TraceKind {
    /// BMC/temel durum karşı örneği → `<görev>_cex.vcd`.
    Counterexample,
    /// prove UNKNOWN'da tümevarım adımının izi → `<görev>_induct.vcd`.
    Induction,
}

/// sby'nin `<iş>_<görev>/engine_0/` altındaki izini `build/formal/`
/// köküne kopyalar; iz üretilmediyse `None`.
fn copy_trace(formal_dir: &Path, workdir: &str, stem: &str, kind: TraceKind) -> Option<PathBuf> {
    let (names, suffix): (&[&str], &str) = match kind {
        TraceKind::Counterexample => (&["trace.vcd", "trace_tb.vcd"], "cex"),
        TraceKind::Induction => (&["trace_induct.vcd"], "induct"),
    };
    let engine_dir = formal_dir.join(workdir).join("engine_0");
    let trace = names
        .iter()
        .map(|n| engine_dir.join(n))
        .find(|p| p.is_file())?;
    let dest = formal_dir.join(format!("{stem}_{suffix}.vcd"));
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

    /// Cover kipi (ADR-0066): ulaşılamayan cover'ın satırı okunur — E5001
    /// modülün ilk kontratına değil, ulaşılamayan cover'a işaret eder.
    #[test]
    fn interpret_cover_log_extracts_the_unreached_cover_line() {
        let log = "\
SBY 14:36:54 [dead] engine_0: ##   0:00:00  Checking cover reachability in step 19..
SBY 14:36:54 [dead] engine_0: ##   0:00:00  Unreached cover statement at dead.sv:59.20-59.96 ($cover$dead.sv:59$26).
SBY 14:36:54 [dead] engine_0: ##   0:00:00  Unreached cover statement at dead.sv:55.20-55.70 ($cover$dead.sv:55$25).
SBY 14:36:54 [dead] DONE (FAIL, rc=2)
";
        let SbyOutcome::Fail(failure) = interpret_sby_output(log) else {
            panic!("FAIL bekleniyor");
        };
        assert_eq!(failure.sv_line, Some(59));
        let sv = "a\nb\n        if (!(rst)) cover (x); // volt:cov_1\n";
        assert_eq!(prop_name_at(sv, 3).as_deref(), Some("cov_1"));
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

    /// ADR-0075: gerçek prove UNKNOWN çıktısı (hdlc/formal, sby 0.36) —
    /// "tool error" değil; tümevarımda bozulan iddianın satırı okunur,
    /// tümevarım adım numarası döngü sayılmaz.
    #[test]
    fn interpret_prove_unknown_log_names_the_non_inductive_assert() {
        let log = "\
SBY 16:34:39 [n_shadow] engine_0.basecase: ##   0:00:00  Checking assertions in step 7..
SBY 16:34:39 [n_shadow] engine_0.induction: ##   0:00:00  Trying induction in step 8..
SBY 16:34:39 [n_shadow] engine_0.induction: ##   0:00:00  Temporal induction failed!
SBY 16:34:39 [n_shadow] engine_0.induction: ##   0:00:00  Assert failed in Shadow: n.sv:40.20-40.42 ($assert$n.sv:40$26)
SBY 16:34:39 [n_shadow] summary: engine_0 (smtbmc z3) returned pass for basecase
SBY 16:34:39 [n_shadow] summary: engine_0 (smtbmc z3) returned FAIL for induction
SBY 16:34:39 [n_shadow] DONE (UNKNOWN, rc=4)
";
        assert_eq!(
            interpret_sby_output(log),
            SbyOutcome::Unknown(SbyFailure {
                sv_line: Some(40),
                step: None
            })
        );
    }

    #[test]
    fn interpret_timeout_and_error_are_distinct() {
        let timeout = "\
SBY 16:34:51 [t_shadow] Reached TIMEOUT (2 seconds). Terminating all subprocesses.
SBY 16:34:51 [t_shadow] DONE (TIMEOUT, rc=8)
";
        assert_eq!(interpret_sby_output(timeout), SbyOutcome::Timeout);
        assert_eq!(
            interpret_sby_output("SBY 16:34:57 [e_shadow] DONE (ERROR, rc=16)\n"),
            SbyOutcome::Error
        );
    }

    #[test]
    fn exit_code_precedence() {
        let code = |f, e, t, u| format!("{:?}", verify_exit_code(f, e, t, u));
        assert_eq!(
            code(false, false, false, false),
            format!("{:?}", ExitCode::SUCCESS)
        );
        assert_eq!(
            code(true, true, true, true),
            format!("{:?}", ExitCode::from(6))
        );
        assert_eq!(
            code(false, true, true, true),
            format!("{:?}", ExitCode::from(3))
        );
        assert_eq!(
            code(false, false, true, true),
            format!("{:?}", ExitCode::from(8))
        );
        assert_eq!(
            code(false, false, false, true),
            format!("{:?}", ExitCode::from(7))
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

    #[test]
    fn job_name_uses_sanitized_file_stem() {
        assert_eq!(job_name(Path::new("examples/soc/top.volt")), "top");
        assert_eq!(
            job_name(Path::new("23_provable_invariant.volt")),
            "23_provable_invariant"
        );
        assert_eq!(job_name(Path::new("odd-name.v2.volt")), "odd_name_v2");
    }

    #[test]
    fn sby_tasks_keep_source_order_and_dedupe_case_collisions() {
        let modules = ["SocTop".to_string(), "Gpio".to_string(), "GPIO".to_string()];
        let tasks = sby_tasks(&modules, &["Gpio".to_string()]);
        let names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["soctop", "gpio", "gpio_2"]);
        assert_eq!(tasks[1].top, "Gpio");
        assert!(tasks[1].multiclock);
        assert!(!tasks[0].multiclock);
    }

    #[test]
    fn contract_modules_dedupes_in_first_seen_order() {
        let prop = |m: &str, n: &str| SvaProp {
            module_name: m.to_string(),
            name: n.to_string(),
            keyword: "invariant",
            span: volt_span::Span::new(volt_span::FileId(0), 0, 0),
            primitive: None,
            auto: None,
        };
        let props = [prop("B", "inv_0"), prop("A", "inv_0"), prop("B", "inv_1")];
        assert_eq!(contract_modules(&props), ["B", "A"]);
    }

    /// ADR-0066 §4: otomatik kontratın E5001'i kökenini söyler; köken
    /// ifadeden farklı yerdeyse (ör. `@mmio` niteliği) ikincil etiket alır.
    #[test]
    fn auto_contract_counterexample_names_its_origin() {
        let at = |s: u32| volt_span::Span::new(volt_span::FileId(0), s, s + 4);
        let auto = |from| volt_sv_emit::AutoProp {
            rule: "counter bound",
            text: "r <= 9".to_string(),
            subject: "wrap check on r".to_string(),
            from,
        };
        let prop = |from| SvaProp {
            module_name: "C".to_string(),
            name: "inv_0".to_string(),
            keyword: "invariant",
            span: at(10),
            primitive: None,
            auto: Some(auto(from)),
        };
        let failure = SbyFailure {
            sv_line: None,
            step: Some(3),
        };
        let (name, diag) = counterexample_diagnostic("C", &failure, "", &[prop(at(10))], None);
        assert_eq!(name, "inv_0");
        assert_eq!(diag.spans.len(), 1, "köken = ifade: ikincil etiket yok");
        let note = diag
            .notes
            .iter()
            .find(|n| n.kind == NoteKind::Note)
            .expect("köken notu");
        assert!(note.text.contains("counter bound"), "{}", note.text);
        assert!(note.text.contains("r <= 9"), "{}", note.text);
        assert!(note.text.contains("wrap check on r"), "{}", note.text);

        let (_, diag) = counterexample_diagnostic("C", &failure, "", &[prop(at(40))], None);
        assert_eq!(diag.spans.len(), 2);
        assert_eq!(diag.spans[1].span, at(40));
    }
}
