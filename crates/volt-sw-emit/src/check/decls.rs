//! C ve Rust ayrıştırıcılarının ortak ikinci yarısı: sabitler + erişimci
//! adlarından [`RegView`] listesi kurmak.
//!
//! Register'lar `<REG>_OFFSET` sabitlerinden (dosya sırası), alanlar
//! `<REG>_<ALAN>_SHIFT` sabitlerinden gelir. Bir `_SHIFT` adı tek başına
//! belirsiz olabilir (`irq` + `status_rx` ile `irq_status` + `rx` aynı adı
//! verir): önce beklenen görünümdeki (tasarım) bölünme aranır, yoksa en
//! uzun register öneki seçilir. Erişim ve alan davranışı erişimci
//! VARLIĞINDAN okunur — yorumdan değil: okuma erişimcisi yoksa register
//! okunamaz.

use std::collections::HashSet;

use super::{norm, Drift, DriftKind, DriverView, FieldBehavior, FieldView, RegView};

/// Biçime özgü erişimci adı sorgusu.
pub enum Accessor<'a> {
    /// Register okuma (`gpio_ctrl_read` / `ctrl_raw`).
    Read(&'a str),
    /// Register yazma (`gpio_ctrl_write` / `set_ctrl_raw`).
    Write(&'a str),
    /// `@self_clearing`: (register, alan, register'ın tek alanı mı).
    Trigger(&'a str, &'a str, bool),
    /// `@w1c`: (register, alan, tek alan mı).
    Clear(&'a str, &'a str, bool),
}

/// Modül öneki soyulmuş sabitler ve normal biçimde fonksiyon adları.
pub struct Decls {
    /// (ad, değer) — dosya sırası.
    pub consts: Vec<(String, u64)>,
    /// [`norm`] biçiminde fonksiyon adları.
    pub fns: HashSet<String>,
    /// Register başına operatif offset (C: adres makrosu). Yoksa `_OFFSET`.
    pub addresses: Vec<(String, u64)>,
    /// Sayı ya da `<MOD>_BASE + off` biçiminde olmayan tanımlar (C) —
    /// bir register'ın adres makrosu buradaysa Volt biçiminden çıkmıştır.
    pub unparsed: Vec<(String, String)>,
}

impl Decls {
    fn get(&self, name: &str) -> Option<u64> {
        self.consts.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
    }

    fn has_fn(&self, name: &str) -> bool {
        self.fns.contains(name)
    }
}

/// Register listesini kurar; eksik ya da 32 biti aşan sabitler `out`'a
/// çelişki olarak düşer. `hint` belirsiz alan adlarını çözer.
pub fn registers(
    module: &str,
    decls: &Decls,
    name_of: &dyn Fn(Accessor<'_>) -> String,
    hint: Option<&DriverView>,
    out: &mut Vec<Drift>,
) -> Vec<RegView> {
    let names: Vec<(String, u64)> = decls
        .consts
        .iter()
        .filter_map(|(n, v)| n.strip_suffix("_OFFSET").map(|r| (r.to_string(), *v)))
        .collect();
    let mut regs: Vec<RegView> = names
        .iter()
        .map(|(r, offset_const)| {
            let subject = format!("{module}_{r}");
            let offset = match decls.addresses.iter().find(|(n, _)| n == r) {
                Some((_, addr)) => {
                    if addr != offset_const {
                        out.push(inconsistent(
                            &subject,
                            format!(
                                "{subject}_OFFSET is 0x{offset_const:02X} but the address macro {subject} uses 0x{addr:02X}"
                            ),
                        ));
                    }
                    *addr
                }
                None => {
                    let odd = decls
                        .unparsed
                        .iter()
                        .find(|(n, _)| n == r)
                        .map(|(_, v)| v.clone())
                        .or_else(|| decls.get(r).map(|v| format!("0x{v:X}")));
                    if let Some(value) = odd {
                        out.push(inconsistent(
                            &subject,
                            format!(
                                "address macro {subject} is `{value}`, not `{module}_BASE + offset`"
                            ),
                        ));
                    }
                    *offset_const
                }
            };
            let mut need = |suffix: &str| {
                let name = format!("{subject}_{suffix}");
                match decls.get(&format!("{r}_{suffix}")) {
                    Some(v) => word(v, &subject, &name, out),
                    None => {
                        out.push(inconsistent(&subject, format!("{name} is missing")));
                        0
                    }
                }
            };
            let mask = need("MASK");
            let reset = need("RESET");
            RegView {
                name: r.clone(),
                offset,
                readable: decls.has_fn(&name_of(Accessor::Read(r))),
                writable: decls.has_fn(&name_of(Accessor::Write(r))),
                mask,
                reset,
                fields: Vec::new(),
            }
        })
        .collect();
    attach_fields(module, decls, &mut regs, hint, out);
    for reg in &mut regs {
        let single = reg.fields.len() == 1;
        let name = reg.name.clone();
        for f in &mut reg.fields {
            f.behavior = if decls.has_fn(&name_of(Accessor::Trigger(&name, &f.name, single))) {
                FieldBehavior::SelfClearing
            } else if decls.has_fn(&name_of(Accessor::Clear(&name, &f.name, single))) {
                FieldBehavior::W1c
            } else {
                FieldBehavior::Normal
            };
        }
    }
    regs
}

/// 32 bite sığmayan değer çelişkidir (sessizce kesilmez).
fn word(v: u64, subject: &str, name: &str, out: &mut Vec<Drift>) -> u32 {
    u32::try_from(v).unwrap_or_else(|_| {
        out.push(inconsistent(
            subject,
            format!("{name} = 0x{v:X} does not fit a 32-bit register"),
        ));
        u32::MAX
    })
}

/// `body`'nin (`<REG>_<ALAN>`) olası (register indeksi, alan) bölünmeleri.
fn splits<'a>(body: &'a str, regs: &[RegView]) -> Vec<(usize, &'a str)> {
    regs.iter()
        .enumerate()
        .filter_map(|(i, r)| {
            body.strip_prefix(r.name.as_str())
                .and_then(|rest| rest.strip_prefix('_'))
                .filter(|f| !f.is_empty())
                .map(|f| (i, f))
        })
        .collect()
}

/// `<REG>_<ALAN>_SHIFT` sabitlerini register'lara bağlar.
fn attach_fields(
    module: &str,
    decls: &Decls,
    regs: &mut [RegView],
    hint: Option<&DriverView>,
    out: &mut Vec<Drift>,
) {
    for (name, lsb) in &decls.consts {
        let Some(body) = name.strip_suffix("_SHIFT") else {
            continue;
        };
        let candidates = splits(body, regs);
        let expected = |i: usize, f: &str| {
            hint.is_some_and(|h| {
                h.registers
                    .iter()
                    .any(|r| r.name == regs[i].name && r.fields.iter().any(|x| x.name == f))
            })
        };
        let chosen = candidates
            .iter()
            .find(|(i, f)| expected(*i, f))
            .or_else(|| candidates.iter().max_by_key(|(i, _)| regs[*i].name.len()))
            .map(|(i, f)| (*i, f.to_string()));
        let Some((i, field)) = chosen else {
            out.push(inconsistent(
                &format!("{module}_{body}"),
                format!("{module}_{name} belongs to no register"),
            ));
            continue;
        };
        let subject = format!("{module}_{}.{field}", regs[i].name);
        let mask = match decls.get(&format!("{body}_MASK")) {
            Some(m) => word(m, &subject, &format!("{module}_{body}_MASK"), out),
            None => {
                out.push(inconsistent(
                    &subject,
                    format!("{module}_{body}_MASK is missing"),
                ));
                0
            }
        };
        let lsb = word(*lsb, &subject, &format!("{module}_{name}"), out);
        regs[i].fields.push(FieldView {
            name: field,
            lsb,
            mask,
            behavior: FieldBehavior::Normal,
        });
    }
}

fn inconsistent(subject: &str, detail: String) -> Drift {
    Drift {
        subject: subject.to_string(),
        kind: DriftKind::Inconsistent,
        file: Some(detail),
        rtl: None,
    }
}

/// Normal biçimde erişimci adı (modül önekli ya da öneksiz).
pub fn accessor_name(prefix: &[&str], parts: &[&str]) -> String {
    let all: Vec<&str> = prefix.iter().chain(parts.iter()).copied().collect();
    norm(&all)
}
