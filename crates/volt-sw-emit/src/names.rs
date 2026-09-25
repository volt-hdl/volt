//! Ad dönüşümleri ve sayı biçimleri — dört üretici de bunları paylaşır.

use volt_ast::mmio::{FieldDesc, FieldKind};

// Ad kuralları parser'ın çarpışma denetimiyle (E1014) tek kaynaktan gelir.
pub use volt_ast::mmio_names::{accessor, file_stem, rmw_local, upper_snake};

/// Alanı taşıyan en dar Rust tam sayı tipi.
pub fn rust_type(field: &FieldDesc) -> &'static str {
    match field.kind {
        FieldKind::Bool => "bool",
        _ if field.width <= 8 => "u8",
        _ if field.width <= 16 => "u16",
        _ => "u32",
    }
}

/// Alanı taşıyan en dar C tipi (`<stdint.h>` / `<stdbool.h>`).
pub fn c_type(field: &FieldDesc) -> &'static str {
    match field.kind {
        FieldKind::Bool => "bool",
        _ if field.width <= 8 => "uint8_t",
        _ if field.width <= 16 => "uint16_t",
        _ => "uint32_t",
    }
}

/// Volt tip yazımı (`bool`, `bits<8>`, `u16`) — belgeler için.
pub fn volt_type(field: &FieldDesc) -> String {
    match field.kind {
        FieldKind::Bool => "bool".to_string(),
        FieldKind::Bits => format!("bits<{}>", field.width),
        FieldKind::UInt => format!("u{}", field.width),
    }
}

/// `0x4000_0000` — Rust literal biçimi (8 hex hane, 4'lü gruplar).
pub fn hex32_rust(v: u64) -> String {
    let s = format!("{v:08X}");
    let (hi, lo) = s.split_at(4);
    format!("0x{hi}_{lo}")
}

/// `0x40000000U` — C literal biçimi.
pub fn hex32_c(v: u64) -> String {
    format!("0x{v:08X}U")
}

/// `0x0C` — kısa offset (en az iki hane).
pub fn hex_short(v: u64) -> String {
    format!("0x{v:02X}")
}

/// Doc metnini satır satır verilen önekle döker; boşsa hiçbir şey.
pub fn doc_lines(out: &mut String, indent: &str, prefix: &str, doc: Option<&str>) {
    if let Some(doc) = doc {
        for line in doc.lines() {
            out.push_str(indent);
            out.push_str(prefix);
            if !line.is_empty() {
                out.push(' ');
                out.push_str(line);
            }
            out.push('\n');
        }
    }
}

/// Doc'un ilk satırı (tablo hücreleri için); `|` kaçırılır.
pub fn doc_summary(doc: Option<&str>) -> String {
    doc.and_then(|d| d.lines().next())
        .map(|l| l.replace('|', "\\|"))
        .unwrap_or_default()
}
