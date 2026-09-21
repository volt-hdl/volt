//! Volt'un ürettiği `no_std` Rust sürücüsünü (ADR-0053 §Rust) okur.
//!
//! Olgular: `pub struct <Modül>`, `pub const BASE`, register başına
//! `<REG>_OFFSET`/`_MASK`/`_RESET`, alan başına `<REG>_<ALAN>_SHIFT`/`_MASK`
//! ve `pub fn` erişimci adları (`<reg>_raw` okunur, `set_<reg>_raw` yazılır,
//! `trigger_*`/`clear_*`). Satır düzenine bağlı değildir (`#[must_use] pub
//! fn` tek satırda, çok satırlı `pub const` aynı okunur). Erişimci
//! GÖVDELERİ ayrıştırılmaz; aynı sürümün dosyasında gövdeleri belirteç
//! karşılaştırması (`code.rs`) denetler.

use std::collections::HashSet;

use super::code::strip_comments;
use super::decls::{self, accessor_name, Accessor, Decls};
use super::{header, norm, parse_int, DriverView, Parsed, Unsupported};
use crate::names::upper_snake;

pub fn parse(text: &str, hint: Option<&DriverView>) -> Result<Parsed, Unsupported> {
    let header = header::read(text)?;
    // Boşluk koşuları tek boşluğa iner: satır kırımı ve girinti önemsiz.
    let code = strip_comments(text, true)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let module = after(&code, "pub struct ")
        .next()
        .map(|rest| upper_snake(&ident(rest)))
        .ok_or_else(|| Unsupported::Malformed("no `pub struct <Module>`".to_string()))?;
    let mut consts: Vec<(String, u64)> = Vec::new();
    let mut fns = HashSet::new();
    for rest in after(&code, "pub const ") {
        let stmt = rest.split(';').next().unwrap_or_default();
        if let (Some((name, _)), Some((_, value))) = (stmt.split_once(':'), stmt.split_once('=')) {
            if let Some(v) = parse_int(value) {
                consts.push((name.trim().to_string(), v));
            }
        }
    }
    // Yalnız `pub` erişimciler sürücünün sözüdür (özel `read`/`write` değil).
    for pat in [
        "pub fn ",
        "pub unsafe fn ",
        "pub const fn ",
        "pub const unsafe fn ",
    ] {
        for rest in after(&code, pat) {
            fns.insert(norm(&[&ident(rest)]));
        }
    }
    let base = consts
        .iter()
        .find(|(n, _)| n == "BASE")
        .map(|(_, v)| *v)
        .ok_or_else(|| Unsupported::Malformed("no `pub const BASE`".to_string()))?;
    let decls = Decls {
        consts,
        fns,
        addresses: Vec::new(),
        unparsed: Vec::new(),
    };
    let name_of = |a: Accessor<'_>| match a {
        Accessor::Read(r) => accessor_name(&[], &[r, "RAW"]),
        Accessor::Write(r) => accessor_name(&["SET"], &[r, "RAW"]),
        Accessor::Trigger(r, f, single) => pulse("TRIGGER", r, f, single),
        Accessor::Clear(r, f, single) => pulse("CLEAR", r, f, single),
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

/// `pat`'ın sözcük başındaki her geçişinden sonraki metin.
fn after<'a>(code: &'a str, pat: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    code.match_indices(pat).filter_map(move |(i, _)| {
        let boundary = code[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
        boundary.then(|| &code[i + pat.len()..])
    })
}

fn ident(rest: &str) -> String {
    rest.trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// `names::accessor` kuralı: tek alanlı register'da alan adı düşer.
fn pulse(verb: &str, reg: &str, field: &str, single: bool) -> String {
    if single {
        accessor_name(&[verb], &[reg])
    } else {
        accessor_name(&[verb], &[reg, field])
    }
}
