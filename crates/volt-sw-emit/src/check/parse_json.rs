//! Volt'un ürettiği `regmap.json`'u (`volt-regmap/1`, ADR-0053 §5) okur.
//!
//! İmza: `schema == "volt-regmap/1"`, `generator` `"volt "` ile başlar ve
//! `regmap_hash` anahtarı vardır (ADR-0063). `volatile`, `type`, `bus` ve
//! `doc` görünüme girmez (C/Rust'ta kod olarak yoklar); aynı sürümün
//! dosyasında bunları da `code.rs` değer karşılaştırması denetler.

use serde_json::Value;

use super::{
    field_mask, header::Header, Drift, DriftKind, DriverView, FieldBehavior, FieldView, Parsed,
    RegView, Unsupported,
};
use crate::json::SCHEMA;
use crate::names::upper_snake;

pub fn parse(text: &str) -> Result<Parsed, Unsupported> {
    let v: Value = serde_json::from_str(text).map_err(|_| Unsupported::NotVolt)?;
    let generator = v["generator"].as_str().unwrap_or_default();
    if v["schema"] != SCHEMA || !generator.starts_with("volt ") {
        return Err(Unsupported::NotVolt);
    }
    let Some(hash) = v["regmap_hash"].as_str() else {
        return Err(Unsupported::OldVolt);
    };
    let span = hash_span(text, hash);
    let header = Header {
        version: generator["volt ".len()..].to_string(),
        source: v["source"].as_str().unwrap_or_default().to_string(),
        hash: hash.to_string(),
        span,
    };
    let module = upper_snake(str_key(&v, "name")?);
    let base = u64_key(&v, "base")?;
    let mut inconsistencies = Vec::new();
    let regs = v["registers"]
        .as_array()
        .ok_or_else(|| malformed("registers"))?;
    let mut registers = Vec::new();
    for r in regs {
        let reg = register(r)?;
        let address = u64_key(r, "address")?;
        let expected = base
            .checked_add(reg.offset)
            .ok_or_else(|| Unsupported::Malformed("base + offset overflows".to_string()))?;
        if address != expected {
            let subject = format!("{module}_{}", reg.name);
            inconsistencies.push(Drift {
                subject,
                kind: DriftKind::Inconsistent,
                file: Some(format!(
                    "address 0x{address:08X} is not base + offset (0x{expected:08X})"
                )),
                rtl: None,
            });
        }
        registers.push(reg);
    }
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

fn register(r: &Value) -> Result<RegView, Unsupported> {
    let (readable, writable) = match str_key(r, "access")? {
        "rw" => (true, true),
        "ro" => (true, false),
        "wo" => (false, true),
        _ => return Err(malformed("access")),
    };
    let mut fields = Vec::new();
    for f in r["fields"].as_array().ok_or_else(|| malformed("fields"))? {
        if f["reserved"] == true {
            continue;
        }
        let behavior = if f["self_clearing"] == true {
            FieldBehavior::SelfClearing
        } else if f["w1c"] == true {
            FieldBehavior::W1c
        } else {
            FieldBehavior::Normal
        };
        fields.push(FieldView {
            name: upper_snake(str_key(f, "name")?),
            lsb: u32_key(f, "lsb")?,
            mask: field_mask(u32_key(f, "width")?),
            behavior,
        });
    }
    let mask = fields
        .iter()
        .fold(0u32, |m, f| m | f.mask.checked_shl(f.lsb).unwrap_or(0));
    Ok(RegView {
        name: upper_snake(str_key(r, "name")?),
        offset: u64_key(r, "offset")?,
        readable,
        writable,
        mask,
        reset: u32_key(r, "reset")?,
        fields,
    })
}

fn str_key<'a>(v: &'a Value, key: &str) -> Result<&'a str, Unsupported> {
    v[key].as_str().ok_or_else(|| malformed(key))
}

fn u64_key(v: &Value, key: &str) -> Result<u64, Unsupported> {
    v[key].as_u64().ok_or_else(|| malformed(key))
}

/// 32 bite sığmayan değer bozuk dosyadır (sessizce kesilmez).
fn u32_key(v: &Value, key: &str) -> Result<u32, Unsupported> {
    u32::try_from(u64_key(v, key)?)
        .map_err(|_| Unsupported::Malformed(format!("`{key}` does not fit 32 bits")))
}

fn malformed(key: &str) -> Unsupported {
    Unsupported::Malformed(format!("missing or mistyped `{key}`"))
}

/// `"regmap_hash": "<h>"` değerinin bayt aralığı (tanı konumu).
fn hash_span(text: &str, hash: &str) -> (usize, usize) {
    text.find("\"regmap_hash\"")
        .and_then(|k| text[k..].find(hash).map(|i| (k + i, k + i + hash.len())))
        .unwrap_or((0, 0))
}
