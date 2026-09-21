//! Seviye 1: sürücü görünümü ↔ üretilen SV'nin adres çözümlemesi.
//!
//! ADR-0053'te `volt-driver/tests/sw_emit_tests.rs` içinde test yardımcısı
//! olarak doğdu (`assert_regmap_matches_rtl`); ADR-0063 ile kütüphaneye
//! taşındı ve panik yerine kalem listesi döndürür. SV metni okunur —
//! HIR değil — çünkü denetlenen şey tam olarak sentezlenecek metindir.
//!
//! Karşılaştırılanlar (parser `desugar_mmio` çıktısının biçimi):
//! - `wire mmio_whit = ...` / `wire mmio_rhit = ...` adres kümeleri = sürücünün
//!   `base + offset` kümesi;
//! - `case (ar_addr)` kolları = okunabilir register'lar, `case (aw_addr)`
//!   kolları = bus'ın yazdığı register'lar (→ erişim);
//! - ReadWrite register'ın `mmio_<reg>_rd` okuma maskesi = sürücü maskesi;
//! - `@w1c` alan biti okuma görünümünde aynı bitte;
//! - sıfırlama dalındaki `mmio_<reg>[_<alan>] <= N'dV` atamaları = reset.

use std::collections::BTreeSet;

use crate::names::upper_snake;

use super::{access_name, hex_offset, hex_word, Drift, DriftKind, DriverView, FieldBehavior};

/// Görünüm ile SV arasındaki kalemler (boş = uyumlu). `sv` tek modülün
/// metni olmalıdır (ADR-0024 modül başına dosya).
pub fn check_rtl(view: &DriverView, sv: &str) -> Vec<Drift> {
    let module = &view.module;
    let mut out = Vec::new();
    let (Some(whit), Some(rhit)) = (
        line_addresses(sv, "wire mmio_whit ="),
        line_addresses(sv, "wire mmio_rhit ="),
    ) else {
        out.push(Drift {
            subject: module.clone(),
            kind: DriftKind::MissingRegister,
            file: Some("register map".to_string()),
            rtl: Some("no mmio address decode in the SystemVerilog".to_string()),
        });
        return out;
    };
    let ar = case_arms(sv, "ar_addr");
    let aw = case_arms(sv, "aw_addr");
    let resets = reset_values(sv, view);

    let mut seen = BTreeSet::new();
    for reg in &view.registers {
        let subject = format!("{module}_{}", reg.name);
        let addr = view.base + reg.offset;
        if !seen.insert(addr) {
            out.push(pair(
                &subject,
                DriftKind::Inconsistent,
                format!("address {} used twice", hex_word(addr)),
                "unique addresses".to_string(),
            ));
        }
        if !whit.contains(&addr) || !rhit.contains(&addr) {
            out.push(pair(
                &subject,
                DriftKind::Offset,
                hex_offset(reg.offset),
                "not decoded".to_string(),
            ));
            continue;
        }
        let rtl_access = (ar.contains(&addr), aw.contains(&addr));
        if rtl_access != (reg.readable, reg.writable) {
            out.push(pair(
                &subject,
                DriftKind::Access,
                reg.access_name().to_string(),
                access_name(rtl_access.0, rtl_access.1).to_string(),
            ));
        }
        let rd = rd_line(sv, &reg.name);
        if reg.readable && reg.writable {
            let rtl_mask = rd.map(read_mask);
            if rtl_mask != Some(Some(reg.mask)) {
                out.push(pair(
                    &subject,
                    DriftKind::Mask,
                    hex_word(u64::from(reg.mask)),
                    match rtl_mask {
                        Some(Some(m)) => hex_word(u64::from(m)),
                        _ => "unrecognised read view".to_string(),
                    },
                ));
            }
        }
        for f in reg
            .fields
            .iter()
            .filter(|f| f.behavior == FieldBehavior::W1c)
        {
            let bit = 1u64.checked_shl(f.lsb).unwrap_or(0);
            if !rd.is_some_and(|l| hex_literals(l).contains(&bit)) {
                out.push(pair(
                    &format!("{subject}.{}", f.name),
                    DriftKind::FieldAccess,
                    FieldBehavior::W1c.name().to_string(),
                    "no W1C bit in the read view".to_string(),
                ));
            }
        }
        if let Some(&(_, rtl_reset)) = resets.iter().find(|(n, _)| *n == reg.name) {
            if rtl_reset != reg.reset {
                out.push(pair(
                    &subject,
                    DriftKind::Reset,
                    hex_word(u64::from(reg.reset)),
                    hex_word(u64::from(rtl_reset)),
                ));
            }
        }
    }
    for addr in whit.union(&rhit) {
        if !seen.contains(addr) {
            out.push(Drift {
                subject: format!("{module}@{}", hex_word(*addr)),
                kind: DriftKind::MissingRegister,
                file: None,
                rtl: Some("RTL address decode".to_string()),
            });
        }
    }
    out
}

fn pair(subject: &str, kind: DriftKind, file: String, rtl: String) -> Drift {
    Drift {
        subject: subject.to_string(),
        kind,
        file: Some(file),
        rtl: Some(rtl),
    }
}

/// Satırdaki tüm `32'h<hex>` literalleri.
fn hex_literals(line: &str) -> BTreeSet<u64> {
    let mut out = BTreeSet::new();
    let mut rest = line;
    while let Some(pos) = rest.find("32'h") {
        let digits: String = rest[pos + 4..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if let Ok(v) = u64::from_str_radix(&digits, 16) {
            out.insert(v);
        }
        rest = &rest[pos + 4 + digits.len()..];
    }
    out
}

/// `wire mmio_whit = ...` gibi tek satırdaki adres kümesi.
fn line_addresses(sv: &str, marker: &str) -> Option<BTreeSet<u64>> {
    sv.lines().find(|l| l.contains(marker)).map(hex_literals)
}

/// `case (<sel>)` ... `endcase` arasındaki `32'h...:` kollarının adresleri.
fn case_arms(sv: &str, sel: &str) -> BTreeSet<u64> {
    let Some(start) = sv.find(&format!("case ({sel})")) else {
        return BTreeSet::new();
    };
    let body = &sv[start..];
    let end = body.find("endcase").unwrap_or(body.len());
    body[..end]
        .lines()
        .filter(|l| l.trim_start().starts_with("32'h") && l.contains(':'))
        .flat_map(|l| hex_literals(l.split(':').next().unwrap_or_default()))
        .collect()
}

/// `wire [31:0] mmio_<ham ad>_rd = ...` satırı. SV ham Volt adını taşır
/// (`dataOut`); görünüm `UPPER_SNAKE` (`DATA_OUT`) — eşleme `upper_snake`.
fn rd_line<'a>(sv: &'a str, reg: &str) -> Option<&'a str> {
    sv.lines().find(|l| {
        l.trim_start()
            .strip_prefix("wire [31:0] mmio_")
            .and_then(|rest| rest.split_once("_rd = "))
            .is_some_and(|(raw, _)| upper_snake(raw) == reg)
    })
}

/// Okuma görünümünün maskesi: literal yoksa tam sözcük, tek literal ise
/// o; başka biçim tanınmaz (`None`).
fn read_mask(line: &str) -> Option<u32> {
    let lits = hex_literals(line);
    match lits.len() {
        0 => Some(u32::MAX),
        1 => lits.first().map(|v| *v as u32),
        _ => None,
    }
}

/// Sıfırlama dallarındaki atamalardan register başına reset değeri.
/// Yalnız en az bir ataması olan register listelenir (sabit `wire`
/// register'ların — ör. ReadOnly kimlik sözcüğü — sıfırlaması yoktur).
fn reset_values(sv: &str, view: &DriverView) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    let mut in_reset = false;
    for line in sv.lines() {
        let t = line.trim();
        if t.starts_with("if (") && t.ends_with("begin") && mentions_reset(t) {
            in_reset = true;
            continue;
        }
        if in_reset && t.starts_with("end") {
            in_reset = false;
            continue;
        }
        if !in_reset {
            continue;
        }
        let Some((lhs, rhs)) = t.split_once("<=") else {
            continue;
        };
        let Some(target) = lhs.trim().strip_prefix("mmio_") else {
            continue;
        };
        let Some(value) = sv_literal(rhs.trim().trim_end_matches(';')) else {
            continue;
        };
        let target = upper_snake(target);
        for reg in &view.registers {
            let shift = if target == reg.name {
                Some(0)
            } else {
                reg.fields
                    .iter()
                    .find(|f| target == format!("{}_{}", reg.name, f.name))
                    .map(|f| f.lsb)
            };
            if let Some(shift) = shift {
                let bits = (value as u32).checked_shl(shift).unwrap_or(0);
                match out.iter_mut().find(|(n, _)| *n == reg.name) {
                    Some((_, v)) => *v |= bits,
                    None => out.push((reg.name.clone(), bits)),
                }
            }
        }
    }
    out
}

/// Koşul bir sıfırlama sinyali adı içeriyor mu (`rst`, `rst_n`, `sys_rst`)?
fn mentions_reset(cond: &str) -> bool {
    cond.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|w| w == "rst" || w.starts_with("rst_") || w.ends_with("_rst"))
}

/// `32'd0`, `1'b0`, `8'hFF` → değer.
fn sv_literal(text: &str) -> Option<u64> {
    let (_, rest) = text.split_once('\'')?;
    let (radix, digits) = rest.split_at(1);
    let digits = digits.replace('_', "");
    match radix {
        "d" => digits.parse().ok(),
        "h" => u64::from_str_radix(&digits, 16).ok(),
        "b" => u64::from_str_radix(&digits, 2).ok(),
        _ => None,
    }
}
