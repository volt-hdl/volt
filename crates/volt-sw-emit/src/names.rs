//! Ad dönüşümleri ve sayı biçimleri — dört üretici de bunları paylaşır.

use volt_ast::mmio::{FieldDesc, FieldKind, RegDesc};

/// `GpioRegs` → `gpio_regs`, `UART2Ctrl` → `uart2_ctrl`, `gpio` → `gpio`.
pub fn snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower =
                i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            let prev_upper = i > 0 && chars[i - 1].is_ascii_uppercase();
            if i > 0 && (prev_lower || (prev_upper && next_lower)) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// `GpioRegs` → `GPIO_REGS`.
pub fn upper_snake(name: &str) -> String {
    snake(name).to_ascii_uppercase()
}

/// Rust/C dosya kökü: modül adının snake_case hali (`gpio_regs`).
pub fn file_stem(module: &str) -> String {
    snake(module)
}

/// Erişimci temel adı: tek adlandırılmış alanı olan register'da alan adı
/// düşer (`direction`), birden çok alanda `reg_field` (`control_enable`).
pub fn accessor(reg: &RegDesc, field: &FieldDesc) -> String {
    if reg.named().count() == 1 {
        reg.name.clone()
    } else {
        format!("{}_{}", reg.name, field.name)
    }
}

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
