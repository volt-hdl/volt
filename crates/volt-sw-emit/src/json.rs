//! `regmap.json` — `volt-regmap/1` şeması (ADR-0053 §JSON; ADR-0063 eklemeli
//! `regmap_hash` ve register `reset` anahtarları).
//!
//! Anahtarlar sabittir; sayılar ondalıktır (JSON hex bilmez), `address =
//! base + offset` hazır verilir. `doc` yoksa `null`. Register ve alan
//! sırası bildirim sırasıdır.

use serde_json::{json, Value};
use volt_ast::mmio::RegMap;

use crate::EmitOpts;

/// Şema kimliği — uyumsuz değişiklikte sayı artar.
pub const SCHEMA: &str = "volt-regmap/1";

pub fn emit(map: &RegMap, opts: &EmitOpts) -> String {
    let mut text = serde_json::to_string_pretty(&value(map, opts)).expect("regmap JSON");
    text.push('\n');
    text
}

pub fn value(map: &RegMap, opts: &EmitOpts) -> Value {
    json!({
        "schema": SCHEMA,
        "generator": format!("volt {}", opts.version),
        "source": opts.source,
        "regmap_hash": crate::check::regmap_hash(map),
        "name": map.module,
        "base": map.base,
        "bus": map.bus,
        "doc": map.doc,
        "registers": map.registers.iter().map(|r| json!({
            "name": r.name,
            "offset": r.offset,
            "address": r.address(map.base),
            "access": r.access.short(),
            "reset": crate::check::reset_value(r),
            "volatile": r.volatile,
            "doc": r.doc,
            "fields": r.fields.iter().map(|f| json!({
                "name": f.name,
                "lsb": f.lsb,
                "width": f.width,
                "type": f.kind.json_name(),
                "reserved": f.reserved,
                "self_clearing": f.self_clearing,
                "w1c": f.w1c,
                "doc": f.doc,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}
