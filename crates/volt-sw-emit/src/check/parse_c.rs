//! Volt'un ürettiği C başlığını (ADR-0053 §C) okur.
//!
//! Yorumlar önce silinir (yorum içindeki bir `#define` sayılmaz). Olgular:
//! `#define <MOD>_BASE`, `<MOD>_<REG> (<MOD>_BASE + off)` adres makrosu —
//! erişimciler bunu kullandığı için operatif offset budur —,
//! `_OFFSET`/`_MASK`/`_RESET`, alan başına `_SHIFT`/`_MASK` ve
//! `static inline` erişimci adları. Satır düzenine bağlı değildir:
//! `#  define` ve dönüş tipinden sonra satır kıran biçimleyici çıktısı
//! aynı okunur.

use std::collections::HashSet;

use super::code::strip_comments;
use super::decls::{self, accessor_name, Accessor, Decls};
use super::{header, norm, parse_int, DriverView, Parsed, Unsupported};

pub fn parse(text: &str, hint: Option<&DriverView>) -> Result<Parsed, Unsupported> {
    let header = header::read(text)?;
    let code = strip_comments(text, false);
    let mut defines: Vec<(String, String)> = Vec::new();
    for line in code.lines() {
        let Some(directive) = line.trim().strip_prefix('#') else {
            continue;
        };
        if let Some(rest) = directive.trim_start().strip_prefix("define") {
            let mut parts = rest.split_whitespace();
            if let Some(name) = parts.next() {
                defines.push((name.to_string(), parts.collect::<Vec<_>>().join(" ")));
            }
        }
    }
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
    let mut inconsistencies = Vec::new();
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
