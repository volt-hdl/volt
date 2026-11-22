//! Volt'un ürettiği C başlığını (ADR-0053 §C) okur.
//!
//! Yorumlar önce silinir (yorum içindeki bir `#define` sayılmaz). Olgular:
//! `#define <MOD>_BASE`, `<MOD>_<REG> (<MOD>_BASE + off)` adres makrosu —
//! erişimciler bunu kullandığı için operatif offset budur —,
//! `_OFFSET`/`_MASK`/`_RESET`, alan başına `_SHIFT`/`_MASK` ve
//! `static inline` erişimci adları. Satır düzenine bağlı değildir:
//! `#  define` ve dönüş tipinden sonra satır kıran biçimleyici çıktısı
//! aynı okunur. `#define`/`#undef` önişlemci sırasıyla izlenir: görünüm
//! son tanımı okur; iki farklı tanım ya da `#undef` iç çelişkidir (#374).

use std::collections::HashSet;

use super::code::strip_comments;
use super::decls::{self, accessor_name, Accessor, Decls};
use super::{header, norm, parse_int, Drift, DriftKind, DriverView, Parsed, Unsupported};

pub fn parse(text: &str, hint: Option<&DriverView>) -> Result<Parsed, Unsupported> {
    let header = header::read(text)?;
    let code = strip_comments(text, false);
    let Macros { defines, history } = macros(&code);
    let fns = inline_fns(&code);
    let (module, base) = defines
        .iter()
        .find_map(|(n, v)| Some((n.strip_suffix("_BASE")?, parse_int(v)?)))
        .ok_or_else(|| Unsupported::Malformed("no `<MODULE>_BASE` define".to_string()))?;
    let module = module.to_string();
    let prefix = format!("{module}_");
    let base_ref = format!("{module}_BASE");
    let mut consts = Vec::new();
    let mut addresses = Vec::new();
    let mut unparsed = Vec::new();
    for (name, value) in &defines {
        let Some(short) = name.strip_prefix(&prefix) else {
            continue;
        };
        // `(GPIO_BASE + 0x0CU)` ve `((GPIO_BASE) + 0x0CU)` aynı biçimdir.
        let compact: String = value
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '(' && *c != ')')
            .collect();
        let address = compact
            .split_once('+')
            .filter(|(lhs, _)| *lhs == base_ref)
            .and_then(|(_, off)| parse_int(off));
        if let Some(off) = address {
            addresses.push((short.to_string(), off));
        } else if let Some(v) = parse_int(value) {
            consts.push((short.to_string(), v));
        } else {
            unparsed.push((short.to_string(), value.clone()));
        }
    }
    let decls = Decls {
        consts,
        fns,
        addresses,
        unparsed,
    };
    let m = module.clone();
    let name_of = move |a: Accessor<'_>| match a {
        Accessor::Read(r) => accessor_name(&[&m], &[r, "READ"]),
        Accessor::Write(r) => accessor_name(&[&m], &[r, "WRITE"]),
        Accessor::Trigger(r, f, _) => accessor_name(&[&m], &["TRIGGER", r, f]),
        Accessor::Clear(r, f, _) => accessor_name(&[&m], &["CLEAR", r, f]),
    };
    let mut inconsistencies = redefinitions(&prefix, &defines, &history);
    let registers = decls::registers(&module, &decls, &name_of, hint, &mut inconsistencies);
    Ok(Parsed {
        header,
        view: DriverView {
            module,
            base,
            registers,
        },
        inconsistencies,
    })
}

/// Önişlemcinin gördüğü makrolar: `defines` dosya sonunda yürürlükte
/// olan tanımlar (ilk tanım sırasıyla), `history` her adın sırayla
/// aldığı farklı değerler.
struct Macros {
    defines: Vec<(String, String)>,
    history: Vec<(String, Vec<String>)>,
}

/// `#define` ve `#undef` yönergeleri dosya sırasıyla, C önişlemcisi gibi:
/// yeniden tanım değeri değiştirir, `#undef` siler (#374). Görünüm, başlığı
/// içeren programın gördüğü değeri, yani son tanımı okur.
fn macros(code: &str) -> Macros {
    let mut defines: Vec<(String, String)> = Vec::new();
    let mut history: Vec<(String, Vec<String>)> = Vec::new();
    for line in code.lines() {
        let Some(directive) = line.trim().strip_prefix('#') else {
            continue;
        };
        let directive = directive.trim_start();
        if let Some(rest) = directive.strip_prefix("define") {
            let mut parts = rest.split_whitespace();
            let Some(name) = parts.next() else {
                continue;
            };
            let value = parts.collect::<Vec<_>>().join(" ");
            match history.iter_mut().find(|(n, _)| n == name) {
                Some((_, values)) => {
                    if !values.contains(&value) {
                        values.push(value.clone());
                    }
                }
                None => history.push((name.to_string(), vec![value.clone()])),
            }
            match defines.iter_mut().find(|(n, _)| n == name) {
                Some((_, v)) => *v = value,
                None => defines.push((name.to_string(), value)),
            }
        } else if let Some(rest) = directive.strip_prefix("undef") {
            if let Some(name) = rest.split_whitespace().next() {
                defines.retain(|(n, _)| n != name);
            }
        }
    }
    Macros { defines, history }
}

/// Modül önekli bir makronun iki farklı değeri ya da `#undef`'i: Volt
/// bir makroyu bir kez tanımlar. Tanımlar arasında kalan erişimciler
/// başka, başlığı içeren program başka değeri kullanır; görünüm yalnız
/// birini görebilir, dosya kendi içinde çelişir (#374).
fn redefinitions(
    prefix: &str,
    defines: &[(String, String)],
    history: &[(String, Vec<String>)],
) -> Vec<Drift> {
    let mut out = Vec::new();
    for (name, values) in history {
        if !name.starts_with(prefix) {
            continue;
        }
        let defined = defines.iter().any(|(n, _)| n == name);
        let detail = if values.len() > 1 {
            let all: Vec<String> = values.iter().map(|v| format!("`{v}`")).collect();
            format!(
                "{name} is defined with {} different values ({}); code before a redefinition uses the earlier value",
                values.len(),
                all.join(", then ")
            )
        } else if !defined {
            format!(
                "{name} is removed with #undef; code before the #undef uses `{}`",
                values[0]
            )
        } else {
            continue;
        };
        out.push(Drift {
            subject: name.clone(),
            kind: DriftKind::Inconsistent,
            file: Some(detail),
            rtl: None,
        });
    }
    out
}

/// Her `static inline ... <ad>(` bildiriminin adı (satır sonları önemsiz).
fn inline_fns(code: &str) -> HashSet<String> {
    let mut fns = HashSet::new();
    let mut rest = code;
    while let Some(pos) = rest.find("static inline") {
        rest = &rest[pos + "static inline".len()..];
        if let Some(name) = rest
            .split('(')
            .next()
            .and_then(|head| head.split_whitespace().last())
        {
            fns.insert(norm(&[name.trim_start_matches('*')]));
        }
    }
    fns
}
