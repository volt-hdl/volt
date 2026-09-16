//! `@timing` / `@false_path` / `@multicycle` biçimleri ve birimli
//! literal'ler (ADR-0054 §3). Desteklenen biçimlerin tek doğruluk
//! kaynağı bu dosyadır; tanınmayan her biçim E0017 üretir — yarım
//! anlaşılmış bir kısıt "kısıt var" izlenimi veremez.
//!
//! Frekans: `25175000` (Hz), `100.mhz`, `25_175.khz`, `1.ghz`, `50.hz`.
//! Süre:    `5.ns`, `250.ps`, `1.us` — birim ZORUNLU (çıplak sayı E0017).
//! Ondalık literal (`25.175.mhz`) lexer'da yoktur (E0001); kHz ya da Hz
//! ile yazılır.

use volt_ast::{AttrArg, Attribute, BinOp, ExprKind, Idx, SourceFile};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

/// Yol kısıtının türü — SDC komutuyla bire bir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// `set_false_path`
    FalsePath,
    /// `set_max_delay <ps>`
    MaxDelay(u64),
    /// `set_min_delay <ps>`
    MinDelay(u64),
    /// `set_multicycle_path N -setup` (+ `N-1 -hold`)
    Multicycle(u32),
}

/// `@timing(...)` içindeki tek bir argümanın çözümlenmiş biçimi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimingForm {
    /// `clk = F` — tam frekans (Hz).
    ClockExact { clock: String, hz: u64 },
    /// `clk >= F` — en düşük frekans (Hz).
    ClockAtLeast { clock: String, hz: u64 },
    /// `max_delay(a, b) <= T` — pikosaniye.
    MaxDelay { from: String, to: String, ps: u64 },
    /// `min_delay(a, b) >= T` — pikosaniye.
    MinDelay { from: String, to: String, ps: u64 },
}

/// `@false_path(from = a, to = b)` — en az biri yazılmalı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FalsePathForm {
    pub from: Option<String>,
    pub to: Option<String>,
}

/// `@multicycle(from = a, to = b, cycles = N)` / deyim üstünde
/// `@multicycle(N)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MulticycleForm {
    pub from: Option<String>,
    pub to: Option<String>,
    pub cycles: u32,
}

/// Pikosaniyeyi `ns` olarak üç ondalıkla yazar (`10.000`, `39.722`).
pub fn format_ps(ps: u64) -> String {
    format!("{}.{:03}", ps / 1000, ps % 1000)
}

pub(super) fn unsupported(span: Span, label: String, help: String) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E0017,
        lstr!(en: "unsupported timing constraint form"; tr: "desteklenmeyen zamanlama kısıtı biçimi"),
        LabeledSpan::primary(span, label),
        help,
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "supported: @timing(clk = 100.mhz), @timing(clk >= 100.mhz), @timing(max_delay(a, b) <= 5.ns), @timing(min_delay(a, b) >= 1.ns), @false_path(from = a, to = b), @multicycle(from = a, to = b, cycles = N); 'volt explain E0017' lists them";
              tr: "desteklenen: @timing(clk = 100.mhz), @timing(clk >= 100.mhz), @timing(max_delay(a, b) <= 5.ns), @timing(min_delay(a, b) >= 1.ns), @false_path(from = a, to = b), @multicycle(from = a, to = b, cycles = N); 'volt explain E0017' listeler"),
    )
}

/// Tek segmentli yol ifadesinin adı.
fn path_name(ast: &SourceFile, e: Idx<Expr>) -> Option<&str> {
    match &ast.exprs[e].kind {
        ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.as_str()),
        _ => None,
    }
}

type Expr = volt_ast::Expr;

/// `from_to` çıktısı: (from, to, kalan adlı/konumsal argümanlar).
type FromTo = (Option<String>, Option<String>, Vec<(String, Idx<Expr>)>);

/// `<int>` (Hz) ya da `<int>.hz|khz|mhz|ghz`. Hatada E0017.
pub(super) fn parse_frequency(ast: &SourceFile, e: Idx<Expr>) -> Result<u64, Box<Diagnostic>> {
    let span = ast.exprs[e].span;
    let help = lstr!(en: "write the frequency as 100.mhz, 25_175.khz or 25175000 (Hz); decimal literals like 25.175.mhz do not exist";
                     tr: "frekansı 100.mhz, 25_175.khz ya da 25175000 (Hz) biçiminde yazın; 25.175.mhz gibi ondalık literal yok");
    let (value, unit) = split_unit(ast, e);
    let Some(value) = value else {
        return Err(Box::new(unsupported(
            span,
            lstr!(en: "expected a frequency literal"; tr: "frekans literal'i bekleniyor"),
            help,
        )));
    };
    let scale: u64 = match unit {
        None | Some("hz") => 1,
        Some("khz") => 1_000,
        Some("mhz") => 1_000_000,
        Some("ghz") => 1_000_000_000,
        Some(other) => {
            return Err(Box::new(unsupported(
                span,
                lstr!(en: "unknown frequency unit '.{other}'"; tr: "bilinmeyen frekans birimi '.{other}'"),
                help,
            )))
        }
    };
    let hz = value
        .checked_mul(scale as u128)
        .filter(|&h| h > 0 && h <= u64::MAX as u128);
    hz.map(|h| h as u64).ok_or_else(|| {
        Box::new(unsupported(
            span,
            lstr!(en: "frequency must be a positive value that fits in 64 bits"; tr: "frekans 64 bite sığan pozitif bir değer olmalı"),
            help,
        ))
    })
}

/// `<int>.ps|ns|us` — birim zorunlu. Hatada E0017.
pub(super) fn parse_time(ast: &SourceFile, e: Idx<Expr>) -> Result<u64, Box<Diagnostic>> {
    let span = ast.exprs[e].span;
    let help = lstr!(en: "write the delay with a unit: 5.ns, 250.ps or 1.us";
                     tr: "gecikmeyi birimle yazın: 5.ns, 250.ps ya da 1.us");
    let (value, unit) = split_unit(ast, e);
    let Some(value) = value else {
        return Err(Box::new(unsupported(
            span,
            lstr!(en: "expected a delay literal"; tr: "gecikme literal'i bekleniyor"),
            help,
        )));
    };
    let scale: u64 = match unit {
        Some("ps") => 1,
        Some("ns") => 1_000,
        Some("us") => 1_000_000,
        None => {
            return Err(Box::new(unsupported(
                span,
                lstr!(en: "a delay needs a unit (ps, ns or us)"; tr: "gecikme bir birim ister (ps, ns ya da us)"),
                help,
            )))
        }
        Some(other) => {
            return Err(Box::new(unsupported(
                span,
                lstr!(en: "unknown time unit '.{other}'"; tr: "bilinmeyen zaman birimi '.{other}'"),
                help,
            )))
        }
    };
    let ps = value
        .checked_mul(scale as u128)
        .filter(|&p| p > 0 && p <= u64::MAX as u128);
    ps.map(|p| p as u64).ok_or_else(|| {
        Box::new(unsupported(
            span,
            lstr!(en: "delay must be a positive value that fits in 64 bits"; tr: "gecikme 64 bite sığan pozitif bir değer olmalı"),
            help,
        ))
    })
}

/// `100.mhz` → `(Some(100), Some("mhz"))`; `100` → `(Some(100), None)`;
/// başka ifade → `(None, _)`.
fn split_unit(ast: &SourceFile, e: Idx<Expr>) -> (Option<u128>, Option<&str>) {
    match &ast.exprs[e].kind {
        ExprKind::IntLit { value, .. } => (Some(*value), None),
        ExprKind::Field { base, field } => match &ast.exprs[*base].kind {
            ExprKind::IntLit { value, .. } => (Some(*value), Some(field.text.as_str())),
            _ => (None, None),
        },
        _ => (None, None),
    }
}

/// `@timing(...)`: her argüman bağımsız bir biçimdir; hatalı olanlar
/// tanı olarak döner, geçerliler forma çevrilir.
pub(super) fn parse_timing(
    ast: &SourceFile,
    attr: &Attribute,
) -> (Vec<(TimingForm, Span)>, Vec<Diagnostic>) {
    let mut forms = Vec::new();
    let mut diags = Vec::new();
    if attr.args.is_empty() {
        diags.push(unsupported(
            attr.span,
            lstr!(en: "@timing needs at least one constraint"; tr: "@timing en az bir kısıt ister"),
            lstr!(en: "write @timing(clk = 100.mhz) or @timing(max_delay(a, b) <= 5.ns)";
                  tr: "@timing(clk = 100.mhz) ya da @timing(max_delay(a, b) <= 5.ns) yazın"),
        ));
    }
    for arg in &attr.args {
        match parse_timing_arg(ast, arg) {
            Ok(form) => forms.push(form),
            Err(d) => diags.push(*d),
        }
    }
    (forms, diags)
}

fn parse_timing_arg(
    ast: &SourceFile,
    arg: &AttrArg,
) -> Result<(TimingForm, Span), Box<Diagnostic>> {
    match arg {
        // `clk = F`
        AttrArg::Named { name, value } => {
            let hz = parse_frequency(ast, *value)?;
            let span = Span {
                end: ast.exprs[*value].span.end,
                ..name.span
            };
            Ok((
                TimingForm::ClockExact {
                    clock: name.text.clone(),
                    hz,
                },
                span,
            ))
        }
        AttrArg::Positional(e) => {
            let span = ast.exprs[*e].span;
            let ExprKind::Binary { op, lhs, rhs } = &ast.exprs[*e].kind else {
                return Err(Box::new(unsupported(
                    span,
                    lstr!(en: "expected 'clk = F', 'clk >= F', 'max_delay(a, b) <= T' or 'min_delay(a, b) >= T'";
                          tr: "'clk = F', 'clk >= F', 'max_delay(a, b) <= T' ya da 'min_delay(a, b) >= T' bekleniyor"),
                    lstr!(en: "see 'volt explain E0017' for the supported forms"; tr: "desteklenen biçimler için 'volt explain E0017'"),
                )));
            };
            // `clk >= F`
            if let Some(clock) = path_name(ast, *lhs) {
                if *op != BinOp::Ge {
                    return Err(Box::new(unsupported(
                        span,
                        lstr!(en: "a clock requirement is written 'clk = F' or 'clk >= F'"; tr: "saat gereksinimi 'clk = F' ya da 'clk >= F' biçiminde yazılır"),
                        lstr!(en: "an upper bound on a clock is not a timing constraint; use '>=' for the minimum frequency"; tr: "saate üst sınır bir zamanlama kısıtı değildir; en düşük frekans için '>=' kullanın"),
                    )));
                }
                let hz = parse_frequency(ast, *rhs)?;
                return Ok((
                    TimingForm::ClockAtLeast {
                        clock: clock.to_string(),
                        hz,
                    },
                    span,
                ));
            }
            // `max_delay(a, b) <= T` / `min_delay(a, b) >= T`
            let ExprKind::Call { callee, args } = &ast.exprs[*lhs].kind else {
                return Err(Box::new(unsupported(
                    span,
                    lstr!(en: "left side must be a clock port or max_delay(a, b) / min_delay(a, b)"; tr: "sol taraf bir saat portu ya da max_delay(a, b) / min_delay(a, b) olmalı"),
                    lstr!(en: "see 'volt explain E0017' for the supported forms"; tr: "desteklenen biçimler için 'volt explain E0017'"),
                )));
            };
            let func = path_name(ast, *callee).unwrap_or("");
            let (from, to) = match args.as_slice() {
                [a, b] => match (path_name(ast, *a), path_name(ast, *b)) {
                    (Some(a), Some(b)) => (a.to_string(), b.to_string()),
                    _ => {
                        return Err(Box::new(unsupported(
                            span,
                            lstr!(en: "{func}(from, to) takes two signal names"; tr: "{func}(from, to) iki sinyal adı alır"),
                            lstr!(en: "name a port or register on each side: max_delay(a, b) <= 5.ns"; tr: "iki tarafa bir port ya da register adı yazın: max_delay(a, b) <= 5.ns"),
                        )))
                    }
                },
                _ => {
                    return Err(Box::new(unsupported(
                        span,
                        lstr!(en: "{func}() takes exactly two arguments (from, to)"; tr: "{func}() tam iki argüman alır (from, to)"),
                        lstr!(en: "write it as max_delay(a, b) <= 5.ns"; tr: "max_delay(a, b) <= 5.ns biçiminde yazın"),
                    )))
                }
            };
            match (func, op) {
                ("max_delay", BinOp::Le) => {
                    let ps = parse_time(ast, *rhs)?;
                    Ok((TimingForm::MaxDelay { from, to, ps }, span))
                }
                ("min_delay", BinOp::Ge) => {
                    let ps = parse_time(ast, *rhs)?;
                    Ok((TimingForm::MinDelay { from, to, ps }, span))
                }
                ("max_delay", _) | ("min_delay", _) => Err(Box::new(unsupported(
                    span,
                    lstr!(en: "max_delay is bounded with '<=', min_delay with '>='"; tr: "max_delay '<=' ile, min_delay '>=' ile sınırlanır"),
                    lstr!(en: "write max_delay(a, b) <= 5.ns or min_delay(a, b) >= 1.ns"; tr: "max_delay(a, b) <= 5.ns ya da min_delay(a, b) >= 1.ns yazın"),
                ))),
                _ => Err(Box::new(unsupported(
                    span,
                    lstr!(en: "unknown timing function '{func}'"; tr: "bilinmeyen zamanlama fonksiyonu '{func}'"),
                    lstr!(en: "only max_delay(a, b) and min_delay(a, b) exist"; tr: "yalnız max_delay(a, b) ve min_delay(a, b) var"),
                ))),
            }
        }
    }
}

/// Adlı argümanlardan `from` / `to` çeker; tanınmayan ad E0017.
fn from_to(ast: &SourceFile, attr: &Attribute, extra: &[&str]) -> Result<FromTo, Box<Diagnostic>> {
    let mut from = None;
    let mut to = None;
    let mut rest = Vec::new();
    for arg in &attr.args {
        match arg {
            AttrArg::Named { name, value } => {
                let key = name.text.as_str();
                if key == "from" || key == "to" {
                    let Some(sig) = path_name(ast, *value) else {
                        return Err(Box::new(unsupported(
                            ast.exprs[*value].span,
                            lstr!(en: "'{key}' must be a signal name"; tr: "'{key}' bir sinyal adı olmalı"),
                            lstr!(en: "write it as {key} = <port or register>"; tr: "{key} = <port ya da register> biçiminde yazın"),
                        )));
                    };
                    let slot = if key == "from" { &mut from } else { &mut to };
                    if slot.is_some() {
                        return Err(Box::new(unsupported(
                            name.span,
                            lstr!(en: "'{key}' given twice"; tr: "'{key}' iki kez verilmiş"),
                            lstr!(en: "keep a single {key} = ... argument"; tr: "tek bir {key} = ... argümanı bırakın"),
                        )));
                    }
                    *slot = Some(sig.to_string());
                } else if extra.contains(&key) {
                    rest.push((key.to_string(), *value));
                } else {
                    return Err(Box::new(unsupported(
                        name.span,
                        lstr!(en: "unknown argument '{key}'"; tr: "bilinmeyen argüman '{key}'"),
                        lstr!(en: "valid arguments: from, to{}", extra.iter().map(|e| format!(", {e}")).collect::<String>();
                              tr: "geçerli argümanlar: from, to{}", extra.iter().map(|e| format!(", {e}")).collect::<String>()),
                    )));
                }
            }
            AttrArg::Positional(e) => rest.push((String::new(), *e)),
        }
    }
    Ok((from, to, rest))
}

/// `@false_path(from = a, to = b)`; `on_stmt` deyim üstünde (`from`/`to`
/// örtük olabilir), modül üstünde en az biri zorunlu.
pub(super) fn parse_false_path(
    ast: &SourceFile,
    attr: &Attribute,
    on_stmt: bool,
) -> Result<FalsePathForm, Box<Diagnostic>> {
    let (from, to, rest) = from_to(ast, attr, &[])?;
    if let Some((_, e)) = rest.first() {
        return Err(Box::new(unsupported(
            ast.exprs[*e].span,
            lstr!(en: "@false_path takes only from = ... and to = ..."; tr: "@false_path yalnız from = ... ve to = ... alır"),
            lstr!(en: "write @false_path(from = a, to = b)"; tr: "@false_path(from = a, to = b) yazın"),
        )));
    }
    if !on_stmt && from.is_none() && to.is_none() {
        return Err(Box::new(unsupported(
            attr.span,
            lstr!(en: "@false_path on a module needs from = ... and/or to = ..."; tr: "modül üstündeki @false_path from = ... ve/veya to = ... ister"),
            lstr!(en: "write @false_path(from = a, to = b), or put the attribute on the register it targets"; tr: "@false_path(from = a, to = b) yazın ya da niteliği hedef register'ın üstüne koyun"),
        )));
    }
    Ok(FalsePathForm { from, to })
}

/// `@multicycle(from = a, to = b, cycles = N)`; deyim üstünde
/// `@multicycle(N)` / `@multicycle(cycles = N)`.
pub(super) fn parse_multicycle(
    ast: &SourceFile,
    attr: &Attribute,
    on_stmt: bool,
) -> Result<MulticycleForm, Box<Diagnostic>> {
    let (from, to, rest) = from_to(ast, attr, &["cycles"])?;
    let help = lstr!(en: "write @multicycle(from = a, to = b, cycles = 2), or @multicycle(2) on the register it targets";
                     tr: "@multicycle(from = a, to = b, cycles = 2) yazın ya da hedef register'ın üstüne @multicycle(2)");
    let cycles_expr = match rest.as_slice() {
        [(_, e)] => *e,
        [] => {
            return Err(Box::new(unsupported(
                attr.span,
                lstr!(en: "@multicycle needs the cycle count"; tr: "@multicycle çevrim sayısını ister"),
                help,
            )))
        }
        [_, (_, second), ..] => {
            return Err(Box::new(unsupported(
                ast.exprs[*second].span,
                lstr!(en: "cycle count given twice"; tr: "çevrim sayısı iki kez verilmiş"),
                help,
            )))
        }
    };
    let cycles = match &ast.exprs[cycles_expr].kind {
        ExprKind::IntLit { value, .. } if *value >= 1 && *value <= u32::MAX as u128 => {
            *value as u32
        }
        _ => {
            return Err(Box::new(unsupported(
                ast.exprs[cycles_expr].span,
                lstr!(en: "cycle count must be an integer literal >= 1"; tr: "çevrim sayısı 1'den büyük ya da eşit bir tamsayı literal'i olmalı"),
                help,
            )))
        }
    };
    if !on_stmt && from.is_none() && to.is_none() {
        return Err(Box::new(unsupported(
            attr.span,
            lstr!(en: "@multicycle on a module needs from = ... and/or to = ..."; tr: "modül üstündeki @multicycle from = ... ve/veya to = ... ister"),
            help,
        )));
    }
    Ok(MulticycleForm { from, to, cycles })
}
