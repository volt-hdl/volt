//! Register haritası tutarlılık denetimi (ADR-0063; cli-contract.md §6a).
//!
//! - `volt check-regmap <tasarım> --against <dosya>...` (Seviye 2): elde
//!   duran sürücü dosyalarını tasarımın bugünkü haritasıyla karşılaştırır.
//! - `volt build --check-regmap` (Seviye 1): yeni üretilen sürücüleri
//!   üretilen SV'nin adres çözümlemesiyle karşılaştırır.
//!
//! Karşılaştırmanın kendisi `volt_sw_emit::check`'tedir; burada yalnız
//! dosya okuma, tanı kurma (E9003/E9004) ve çıktı biçimi durur.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use serde_json::{json, Value};
use volt_diagnostics::{
    lstr, render_human, render_short, Diagnostic, ErrorCode, LabeledSpan, NoteKind,
};
use volt_span::{SourceMap, Span};
use volt_sv_emit::SvaMode;
use volt_sw_emit::check::{self, Cause, Drift, DriftKind, Format, Report, Unsupported};
use volt_sw_emit::{EmitOpts, SwKind};

use crate::{compile, json_envelope, render_diagnostics, Compiled, OutputFormat};

/// `volt check-regmap`: çıkış 0 = hepsi uyumlu, 1 = ayrışma/E9004 ya da
/// tasarım hatası, 2 = tasarımda `@mmio` modülü yok, 3 = dosya okunamadı.
pub fn check_regmap(file: &Path, against: &[PathBuf], format: OutputFormat) -> ExitCode {
    let start = Instant::now();
    if format == OutputFormat::Human {
        eprintln!(
            "{}",
            lstr!(
                en: "    Checking {} against {} file(s)", file.display(), against.len();
                tr: "    Kontrol {} ↔ {} dosya", file.display(), against.len()
            )
        );
    }
    let mut compiled = match compile(file, true, SvaMode::None) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if compiled.errors() > 0 {
        return finish(&compiled, format, &[], start);
    }
    if compiled.regmaps.is_empty() {
        eprintln!(
            "{}",
            lstr!(
                en: "error: {} has no @mmio module; there is no register map to compare against", file.display();
                tr: "hata: {} içinde @mmio modülü yok; karşılaştırılacak register haritası yok", file.display()
            )
        );
        return ExitCode::from(2);
    }
    let opts = emit_opts(file);
    let mut outcomes = Vec::new();
    for path in against {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(err) => {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot read '{}': {}", path.display(), err;
                        tr: "hata: '{}' okunamadı: {}", path.display(), err
                    )
                );
                return ExitCode::from(3);
            }
        };
        let fid = compiled.map.add_file(path.clone(), text.clone());
        let outcome = match Format::from_path(path) {
            Some(fmt) => check::check_file(&compiled.regmaps, fmt, &text, &opts),
            None => Err(Unsupported::Extension(
                path.extension()
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )),
        };
        let first_line = text.lines().next().map_or(0, str::len);
        let diag = match &outcome {
            Ok(r) if r.is_match() => None,
            Ok(r) => {
                let (s, e) = r.parsed.header.span;
                Some(drift_diagnostic(
                    path,
                    Span::new(fid, s as u32, e as u32),
                    r,
                    file,
                ))
            }
            Err(u) => Some(unsupported_diagnostic(
                path,
                Span::new(fid, 0, first_line as u32),
                u,
                file,
            )),
        };
        compiled.diagnostics.extend(diag);
        outcomes.push((path.clone(), outcome));
    }
    finish(&compiled, format, &outcomes, start)
}

type Outcome = (PathBuf, Result<Report, Unsupported>);

/// Tanıları basar, özet/JSON yazar, çıkış kodunu seçer.
fn finish(
    compiled: &Compiled,
    format: OutputFormat,
    outcomes: &[Outcome],
    start: Instant,
) -> ExitCode {
    render_diagnostics(compiled, format);
    let matched = outcomes
        .iter()
        .filter(|(_, o)| o.as_ref().is_ok_and(Report::is_match))
        .count();
    if format == OutputFormat::Human {
        for (path, outcome) in outcomes {
            if let Some(r) = outcome.as_ref().ok().filter(|r| r.is_match()) {
                let how = if r.fast_path {
                    lstr!(en: "hash match"; tr: "hash eşit")
                } else {
                    lstr!(en: "compared"; tr: "karşılaştırıldı")
                };
                eprintln!(
                    "{}",
                    lstr!(
                        en: "       Match {} (regmap-hash {}, {})", path.display(), r.design_hash, how;
                        tr: "      Uyumlu {} (regmap-hash {}, {})", path.display(), r.design_hash, how
                    )
                );
            }
        }
        eprintln!(
            "{}",
            lstr!(
                en: "    Finished {:.2}s", start.elapsed().as_secs_f64();
                tr: "    Tamamlandı {:.2}s", start.elapsed().as_secs_f64()
            )
        );
        eprintln!(
            "{}",
            lstr!(
                en: "      Result {} of {} file(s) match the register map", matched, outcomes.len();
                tr: "       Sonuç {} / {} dosya register haritasıyla uyumlu", matched, outcomes.len()
            )
        );
    }
    if format == OutputFormat::Json {
        let mut envelope = json_envelope("check-regmap", compiled, &[], start);
        envelope["regmap_check"] = json!({
            "design_hashes": compiled
                .regmaps
                .iter()
                .map(|m| (m.module.clone(), Value::from(check::regmap_hash(m))))
                .collect::<serde_json::Map<_, _>>(),
            "files": outcomes.iter().map(outcome_json).collect::<Vec<_>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).expect("JSON zarfı")
        );
    }
    if compiled.errors() > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn outcome_json((path, outcome): &Outcome) -> Value {
    let path = path.display().to_string();
    match outcome {
        Ok(r) => json!({
            "path": path,
            "format": format_flag(r.format),
            "status": if r.is_match() { "match" } else { "drift" },
            "module": r.parsed.view.module,
            "declared_hash": r.parsed.header.hash,
            "content_hash": r.content_hash,
            "design_hash": r.design_hash,
            "generator_version": r.parsed.header.version,
            "fast_path": r.fast_path,
            "code_compared": r.code_compared,
            "cause": r.cause.map(cause_key),
            "drift": r.drift.iter().map(|d| json!({
                "kind": d.kind.key(),
                "subject": d.subject,
                "file": d.file,
                "rtl": d.rtl,
            })).collect::<Vec<_>>(),
        }),
        Err(u) => json!({
            "path": path,
            "status": "unsupported",
            "reason": match u {
                Unsupported::Extension(_) => "extension",
                Unsupported::NotVolt => "not_volt",
                Unsupported::OldVolt => "old_volt",
                Unsupported::Malformed(_) => "malformed",
            },
        }),
    }
}

fn cause_key(c: Cause) -> &'static str {
    match c {
        Cause::Stale => "stale",
        Cause::Edited => "edited",
        Cause::StaleAndEdited => "stale_and_edited",
    }
}

/// `--emit` değeri (`c` | `rust` | `regmap`).
fn format_flag(f: Format) -> &'static str {
    f.kind().flag()
}

fn emit_opts(file: &Path) -> EmitOpts {
    EmitOpts {
        source: file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display().to_string()),
        version: volt_sv_emit::VOLT_VERSION.to_string(),
    }
}

/// E9003 (Seviye 2): dosya ↔ tasarım.
fn drift_diagnostic(path: &Path, span: Span, r: &Report, design: &Path) -> Diagnostic {
    let flag = format_flag(r.format);
    let label = if r.parsed.header.hash == r.design_hash {
        lstr!(
            en: "matches the design, but the content no longer does";
            tr: "tasarımla eşit, ama içerik artık değil"
        )
    } else {
        lstr!(
            en: "the design's regmap-hash is {}", r.design_hash;
            tr: "tasarımın regmap-hash'i {}", r.design_hash
        )
    };
    let mut diag = Diagnostic::error(
        ErrorCode::E9003,
        lstr!(
            en: "register map drift in {}", path.display();
            tr: "{} içinde register haritası ayrışması", path.display()
        ),
        LabeledSpan::primary(span, label),
        lstr!(
            en: "regenerate with volt build --emit={} {}", flag, design.display();
            tr: "volt build --emit={} {} ile yeniden üretin", flag, design.display()
        ),
    );
    for d in &r.drift {
        diag = diag.with_note(
            NoteKind::Note,
            describe(d, Side::File, &r.parsed.header.version),
        );
    }
    if let Some(cause) = r.cause {
        diag = diag.with_note(NoteKind::Reason, cause_text(cause));
    }
    diag
}

fn cause_text(c: Cause) -> String {
    match c {
        Cause::Stale => lstr!(
            en: "the file is stale: it was generated from an older register map (its regmap-hash matches its own content)";
            tr: "dosya bayat: eski bir register haritasından üretilmiş (regmap-hash'i kendi içeriğine eşit)"
        ),
        Cause::Edited => lstr!(
            en: "the file was edited after generation (its regmap-hash is the design's, its content is not)";
            tr: "dosya üretildikten sonra düzenlenmiş (regmap-hash'i tasarımın, içeriği değil)"
        ),
        Cause::StaleAndEdited => lstr!(
            en: "the file was generated from another register map and edited afterwards";
            tr: "dosya başka bir register haritasından üretilmiş ve sonra düzenlenmiş"
        ),
    }
}

/// E9004: desteklenmeyen dosya — sessizlik yerine açık tanı.
fn unsupported_diagnostic(path: &Path, span: Span, u: &Unsupported, design: &Path) -> Diagnostic {
    let design = design.display();
    let (message, note, help) = match u {
        Unsupported::Extension(ext) => (
            lstr!(
                en: "{} is not a Volt driver file (extension '.{}')", path.display(), ext;
                tr: "{} bir Volt sürücü dosyası değil (uzantı '.{}')", path.display(), ext
            ),
            lstr!(
                en: "check-regmap reads .h (--emit=c), .rs (--emit=rust) and .json (--emit=regmap)";
                tr: "check-regmap .h (--emit=c), .rs (--emit=rust) ve .json (--emit=regmap) okur"
            ),
            lstr!(
                en: "pass a file generated by volt build --emit=c,rust,regmap {}", design;
                tr: "volt build --emit=c,rust,regmap {} ile üretilmiş bir dosya verin", design
            ),
        ),
        Unsupported::NotVolt => (
            lstr!(
                en: "{} is not a Volt-generated register map", path.display();
                tr: "{} Volt'un ürettiği bir register haritası değil", path.display()
            ),
            lstr!(
                en: "no '// Generated by Volt' + '// regmap-hash:' signature (JSON: schema volt-regmap/1 with regmap_hash); hand-written files are not parsed";
                tr: "'// Generated by Volt' + '// regmap-hash:' imzası yok (JSON: regmap_hash'li volt-regmap/1 şeması); elle yazılmış dosyalar ayrıştırılmaz"
            ),
            lstr!(
                en: "generate the driver with volt build --emit=c,rust,regmap {} and compare that file", design;
                tr: "sürücüyü volt build --emit=c,rust,regmap {} ile üretip o dosyayı karşılaştırın", design
            ),
        ),
        Unsupported::OldVolt => (
            lstr!(
                en: "{} was generated by a Volt version without check-regmap support", path.display();
                tr: "{} check-regmap desteği olmayan bir Volt sürümüyle üretilmiş", path.display()
            ),
            lstr!(
                en: "it has the ADR-0053 header but no regmap-hash signature";
                tr: "ADR-0053 başlığı var ama regmap-hash imzası yok"
            ),
            lstr!(
                en: "regenerate it once with volt build --emit=c,rust,regmap {}", design;
                tr: "bir kez volt build --emit=c,rust,regmap {} ile yeniden üretin", design
            ),
        ),
        Unsupported::Malformed(what) => (
            lstr!(
                en: "{} carries a Volt signature but cannot be read: {}", path.display(), what;
                tr: "{} Volt imzası taşıyor ama okunamıyor: {}", path.display(), what
            ),
            lstr!(
                en: "a required declaration was removed or renamed by hand";
                tr: "zorunlu bir bildirim elle silinmiş ya da yeniden adlandırılmış"
            ),
            lstr!(
                en: "regenerate it with volt build --emit=c,rust,regmap {}", design;
                tr: "volt build --emit=c,rust,regmap {} ile yeniden üretin", design
            ),
        ),
    };
    Diagnostic::error(
        ErrorCode::E9004,
        message,
        LabeledSpan::primary(
            span,
            match u {
                Unsupported::Extension(_) => {
                    lstr!(en: "unsupported file type"; tr: "desteklenmeyen dosya türü")
                }
                Unsupported::Malformed(_) => {
                    lstr!(en: "signed by Volt, but incomplete"; tr: "Volt imzalı ama eksik")
                }
                Unsupported::NotVolt | Unsupported::OldVolt => {
                    lstr!(en: "no regmap-hash signature"; tr: "regmap-hash imzası yok")
                }
            },
        ),
        help,
    )
    .with_note(NoteKind::Note, note)
}

/// Kalemin hangi tarafı "dosya" diye anılır.
#[derive(Clone, Copy)]
enum Side {
    /// Seviye 2: `--against` dosyası.
    File,
    /// Seviye 1: yeni üretilen sürücü.
    Driver,
}

/// Tek ayrışma kalemi → `= not:` satırı (`GPIO_CONTROL offset: file 0x0C, RTL 0x10`).
fn describe(d: &Drift, side: Side, version: &str) -> String {
    let (file, rtl) = (
        d.file.as_deref().unwrap_or("-"),
        d.rtl.as_deref().unwrap_or("-"),
    );
    let s = &d.subject;
    let side_name = match side {
        Side::File => lstr!(en: "file"; tr: "dosya"),
        Side::Driver => lstr!(en: "driver"; tr: "sürücü"),
    };
    let property = match d.kind {
        DriftKind::Module => Some(lstr!(en: "module"; tr: "modül")),
        DriftKind::Base => Some(String::new()),
        DriftKind::Offset => Some("offset".to_string()),
        DriftKind::Access | DriftKind::FieldAccess => Some(lstr!(en: "access"; tr: "erişim")),
        DriftKind::Mask | DriftKind::FieldMask => Some(lstr!(en: "mask"; tr: "maske")),
        DriftKind::Reset => Some("reset".to_string()),
        DriftKind::FieldLsb => Some(lstr!(en: "bit position"; tr: "bit konumu")),
        _ => None,
    };
    if let Some(p) = property {
        let head = if p.is_empty() {
            s.clone()
        } else {
            format!("{s} {p}")
        };
        return format!("{head}: {side_name} {file}, RTL {rtl}");
    }
    match d.kind {
        DriftKind::MissingRegister | DriftKind::MissingField => lstr!(
            en: "missing in {}: {} ({})", side_name, s, rtl;
            tr: "{} içinde eksik: {} ({})", side_name, s, rtl
        ),
        DriftKind::ExtraRegister | DriftKind::ExtraField => lstr!(
            en: "extra in {}: {} ({}), not in RTL", side_name, s, file;
            tr: "{} içinde fazla: {} ({}), RTL'de yok", side_name, s, file
        ),
        DriftKind::Inconsistent => lstr!(
            en: "{}: the {} contradicts itself: {}", s, side_name, file;
            tr: "{}: {} kendi içinde çelişiyor: {}", s, side_name, file
        ),
        _ => lstr!(
            en: "accessor code differs from what Volt {} generates: {} {}, generated {}", version, side_name, file, rtl;
            tr: "erişimci kodu Volt {} üretiminden farklı: {} {}, üretilen {}", version, side_name, file, rtl
        ),
    }
}

/// Seviye 1 (`volt build --check-regmap`): her `@mmio` modülünün
/// sürücüleri (istenen türler; hiçbiri istenmediyse üçü de bellekte)
/// yeniden okunur, haritayla ve modülün SV'siyle karşılaştırılır.
/// Dönen tanılar derleyici hatasıdır — ikisi de aynı haritadan gelir.
pub fn build_check(compiled: &Compiled, kinds: &[SwKind], opts: &EmitOpts) -> Vec<Diagnostic> {
    let mut formats: Vec<Format> = kinds.iter().filter_map(|k| Format::of_kind(*k)).collect();
    if formats.is_empty() {
        formats = vec![Format::C, Format::Rust, Format::Json];
    }
    let mut out = Vec::new();
    for map in &compiled.regmaps {
        let sv = compiled
            .modules
            .iter()
            .find(|m| m.name == map.module)
            .map(|m| m.sv.as_str())
            .or(compiled.sv.as_deref())
            .unwrap_or_default();
        let expected = check::view_of(map);
        for &fmt in &formats {
            let text = fmt.kind().render(map, opts);
            let drift = match check::parse_with(fmt, &text, Some(&expected)) {
                Ok(parsed) => {
                    let mut d = parsed.inconsistencies;
                    d.extend(check::diff(&expected, &parsed.view));
                    d.extend(check::check_rtl(&parsed.view, sv));
                    d
                }
                Err(u) => vec![Drift {
                    subject: expected.module.clone(),
                    kind: DriftKind::Inconsistent,
                    file: Some(format!("generated file is unreadable: {u:?}")),
                    rtl: None,
                }],
            };
            if drift.is_empty() {
                continue;
            }
            let mut diag = Diagnostic::error(
                ErrorCode::E9003,
                lstr!(
                    en: "generated {} driver for '{}' disagrees with the generated RTL", fmt.kind().flag(), map.module;
                    tr: "'{}' için üretilen {} sürücüsü üretilen RTL ile uyuşmuyor", map.module, fmt.kind().flag()
                ),
                LabeledSpan::primary(
                    module_span(compiled, &map.module),
                    lstr!(en: "@mmio register map declared here"; tr: "@mmio register haritası burada"),
                ),
                lstr!(
                    en: "this is a compiler bug: report it; do not ship the generated {} file", fmt.kind().flag();
                    tr: "bu bir derleyici hatası: bildirin; üretilen {} dosyasını kullanmayın", fmt.kind().flag()
                ),
            );
            for d in &drift {
                diag = diag.with_note(NoteKind::Note, describe(d, Side::Driver, &opts.version));
            }
            diag = diag.with_note(
                NoteKind::Reason,
                lstr!(
                    en: "the driver and the SystemVerilog are generated from the same @mmio map (ADR-0063)";
                    tr: "sürücü ve SystemVerilog aynı @mmio haritasından üretilir (ADR-0063)"
                ),
            );
            out.push(diag);
        }
    }
    out
}

/// `@mmio` modülünün ad konumu; bulunamazsa birimin ilk öğesi.
fn module_span(compiled: &Compiled, module: &str) -> Span {
    let ast = &compiled.ast;
    let items = || ast.items.iter().map(|i| &ast.items_arena[*i]);
    items()
        .find_map(|item| match &item.kind {
            volt_ast::ItemKind::Module(m) if m.name.text == module => Some(m.name.span),
            _ => None,
        })
        .or_else(|| items().next().map(|i| i.span))
        .unwrap_or(Span::new(volt_span::FileId(0), 0, 0))
}

/// Seviye 1 tanılarını seçilen biçimde basar (JSON zarfı çağıranda).
pub fn render(diags: &[Diagnostic], map: &SourceMap, format: OutputFormat) {
    for diag in diags {
        match format {
            OutputFormat::Human => eprintln!("{}", render_human(diag, map)),
            OutputFormat::Short => eprintln!("{}", render_short(diag, map)),
            OutputFormat::Json => {}
        }
    }
}
