//! Üretilen yazılım sürücülerinin ad uzayı (ADR-0053 adlandırması,
//! ADR-0079 §2).
//!
//! Rust ve C üreticileri (`volt-sw-emit`) adları buradaki kurallarla kurar;
//! parser aynı kurallarla, iki `@mmio` adının (ya da bir ad ile üreticinin
//! kendi adının) aynı hedef dil tanımlayıcısına indiği durumu derleme
//! zamanında E1014 olarak reddeder — sessizce ad değiştirmez (ADR-0078
//! ilkesi). Kural ile üretici ayrışırsa `volt-sw-emit` testleri (üretilen
//! metinden çıkarılan tanımlayıcı kümesi = [`driver_names`]) düşer.
//!
//! Kapsamlar:
//! - Rust: sürücü `impl` bloğu — ilişkili sabitler ve metotlar tek ad
//!   uzayıdır (E0201).
//! - C: dosya kapsamı — `#define` makroları ve `static inline` işlevler;
//!   ayrıca setter parametresi bir makro ya da `<stdint.h>` tip adıyla
//!   aynıysa önişlemci/tip çakışması.
//! - Birim: iki modül aynı dosya köküne (`snake_case`) inerse biri
//!   ötekinin `build/sw/<kök>.{rs,h,json}` dosyasının üzerine yazar.

use crate::mmio::{FieldDesc, RegDesc, RegMap};

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

/// Rust/C/JSON dosya kökü: modül adının snake_case hali (`gpio_regs`).
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

/// Setter gövdesindeki okuma-değiştirme-yazma yereli. Parametre alanın
/// adını taşır; alan `word` ise yerel başka ad alır — Rust'ta gölgeleme
/// parametreyi sessizce eski sözcükle değiştirirdi, C'de yeniden bildirim
/// hatasıdır (ADR-0079 §2). Kullanıcının gördüğü hiçbir ad değişmez.
pub fn rmw_local(field: &FieldDesc) -> &'static str {
    if field.name == "word" {
        "current"
    } else {
        "word"
    }
}

/// Setter üretilir mi (tetikleyici ve w1c alanı yerine özel metot alır)?
pub fn has_setter(reg: &RegDesc, field: &FieldDesc) -> bool {
    reg.access.writable() && !field.self_clearing && !field.w1c
}

/// C başlığının kullandığı `<stdint.h>` tip adları: setter parametresi
/// bunlardan biri olamaz (`uint32_t` ve C/C++ anahtar sözcükleri E1013).
pub const C_TYPE_NAMES: [&str; 3] = ["uint8_t", "uint16_t", "uint32_t"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {
    Rust,
    C,
}

impl Lang {
    pub fn name(self) -> &'static str {
        match self {
            Lang::Rust => "Rust",
            Lang::C => "C",
        }
    }
}

/// Üretilen tanımlayıcının kaynağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Üreticinin kendi adı; metin ne olduğunu söyler (`the constructor`).
    Generator(&'static str),
    /// `registers[i]`.
    Register(usize),
    /// `registers[i].fields[j]`.
    Field(usize, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverName {
    pub lang: Lang,
    pub ident: String,
    pub origin: Origin,
}

/// İki kaynağın aynı tanımlayıcıya inmesi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    pub lang: Lang,
    pub ident: String,
    /// Önce üretilen (üretici adı ya da daha önce bildirilen ad).
    pub first: Origin,
    /// Çakışan, sonra bildirilen ad — tanı buraya bağlanır.
    pub second: Origin,
}

fn push(out: &mut Vec<DriverName>, lang: Lang, ident: String, origin: Origin) {
    out.push(DriverName {
        lang,
        ident,
        origin,
    });
}

/// Rust `impl` bloğunun tanımlayıcıları, üretim sırasıyla.
fn rust_names(map: &RegMap, out: &mut Vec<DriverName>) {
    let g = |what| Origin::Generator(what);
    push(
        out,
        Lang::Rust,
        "BASE".into(),
        g("the base address constant"),
    );
    push(out, Lang::Rust, "new".into(), g("the constructor"));
    push(
        out,
        Lang::Rust,
        "at_default_base".into(),
        g("the default-base constructor"),
    );
    push(out, Lang::Rust, "read".into(), g("the private word reader"));
    push(
        out,
        Lang::Rust,
        "write".into(),
        g("the private word writer"),
    );
    for (i, reg) in map.registers.iter().enumerate() {
        let up = upper_snake(&reg.name);
        let r = Origin::Register(i);
        for suffix in ["OFFSET", "MASK", "RESET"] {
            push(out, Lang::Rust, format!("{up}_{suffix}"), r);
        }
        for (j, f) in reg.fields.iter().enumerate().filter(|(_, f)| !f.reserved) {
            let fup = format!("{up}_{}", upper_snake(&f.name));
            for suffix in ["SHIFT", "MASK"] {
                push(
                    out,
                    Lang::Rust,
                    format!("{fup}_{suffix}"),
                    Origin::Field(i, j),
                );
            }
        }
    }
    for (i, reg) in map.registers.iter().enumerate() {
        let r = Origin::Register(i);
        if reg.access.readable() {
            push(out, Lang::Rust, format!("{}_raw", reg.name), r);
        }
        if reg.access.writable() {
            push(out, Lang::Rust, format!("set_{}_raw", reg.name), r);
        }
        for (j, f) in reg.fields.iter().enumerate().filter(|(_, f)| !f.reserved) {
            let acc = accessor(reg, f);
            let o = Origin::Field(i, j);
            if reg.access.readable() {
                push(out, Lang::Rust, acc.clone(), o);
            }
            if f.self_clearing {
                push(out, Lang::Rust, format!("trigger_{acc}"), o);
            } else if f.w1c {
                push(out, Lang::Rust, format!("clear_{acc}"), o);
            } else if reg.access.writable() {
                push(out, Lang::Rust, format!("set_{acc}"), o);
            }
        }
    }
}

/// C başlığının dosya kapsamındaki tanımlayıcıları, üretim sırasıyla.
fn c_names(map: &RegMap, out: &mut Vec<DriverName>) {
    let mod_up = upper_snake(&map.module);
    let lower = mod_up.to_ascii_lowercase();
    push(
        out,
        Lang::C,
        format!("{mod_up}_H"),
        Origin::Generator("the include guard"),
    );
    push(
        out,
        Lang::C,
        format!("{mod_up}_BASE"),
        Origin::Generator("the base address macro"),
    );
    for (i, reg) in map.registers.iter().enumerate() {
        let up = format!("{mod_up}_{}", upper_snake(&reg.name));
        let r = Origin::Register(i);
        push(out, Lang::C, up.clone(), r);
        for suffix in ["OFFSET", "MASK", "RESET"] {
            push(out, Lang::C, format!("{up}_{suffix}"), r);
        }
        for (j, f) in reg.fields.iter().enumerate().filter(|(_, f)| !f.reserved) {
            let fup = format!("{up}_{}", upper_snake(&f.name));
            for suffix in ["SHIFT", "MASK"] {
                push(out, Lang::C, format!("{fup}_{suffix}"), Origin::Field(i, j));
            }
        }
        if reg.access.readable() {
            push(out, Lang::C, format!("{lower}_{}_read", reg.name), r);
        }
        if reg.access.writable() {
            push(out, Lang::C, format!("{lower}_{}_write", reg.name), r);
        }
        for (j, f) in reg.fields.iter().enumerate().filter(|(_, f)| !f.reserved) {
            let name = format!("{}_{}", reg.name, f.name);
            let o = Origin::Field(i, j);
            if reg.access.readable() {
                push(out, Lang::C, format!("{lower}_get_{name}"), o);
            }
            if f.self_clearing {
                push(out, Lang::C, format!("{lower}_trigger_{name}"), o);
            } else if f.w1c {
                push(out, Lang::C, format!("{lower}_clear_{name}"), o);
            } else if reg.access.writable() {
                push(out, Lang::C, format!("{lower}_set_{name}"), o);
            }
        }
    }
}

/// Sürücülerin kapsam düzeyindeki tüm tanımlayıcıları (Rust `impl`
/// öğeleri, C makroları ve işlevleri), üretim sırasıyla.
pub fn driver_names(map: &RegMap) -> Vec<DriverName> {
    let mut out = Vec::new();
    rust_names(map, &mut out);
    c_names(map, &mut out);
    out
}

/// Haritanın bütün ad çarpışmaları, bildirim sırasıyla. Aynı çift bir
/// kez raporlanır (ilk ortak tanımlayıcıyla).
pub fn collisions(map: &RegMap) -> Vec<Collision> {
    let names = driver_names(map);
    let mut found: Vec<Collision> = Vec::new();
    let mut add = |lang, ident: &str, first: Origin, second: Origin| {
        if first == second || found.iter().any(|c| c.first == first && c.second == second) {
            return;
        }
        found.push(Collision {
            lang,
            ident: ident.to_string(),
            first,
            second,
        });
    };
    for (k, n) in names.iter().enumerate() {
        if let Some(prev) = names[..k]
            .iter()
            .find(|p| p.lang == n.lang && p.ident == n.ident && p.origin != n.origin)
        {
            add(n.lang, &n.ident, prev.origin, n.origin);
        }
    }
    // C setter parametresi: makro adıysa önişlemci onu açar; stdint tip
    // adıysa bildirim `uint32_t uint32_t` olur.
    for (i, reg) in map.registers.iter().enumerate() {
        for (j, f) in reg.fields.iter().enumerate().filter(|(_, f)| !f.reserved) {
            if !has_setter(reg, f) {
                continue;
            }
            let o = Origin::Field(i, j);
            if C_TYPE_NAMES.contains(&f.name.as_str()) {
                add(Lang::C, &f.name, Origin::Generator("a <stdint.h> type"), o);
            } else if let Some(m) = names.iter().find(|m| {
                m.lang == Lang::C
                    && m.ident == f.name
                    && m.ident.bytes().all(|b| !b.is_ascii_lowercase())
            }) {
                add(Lang::C, &f.name, m.origin, o);
            }
        }
    }
    found.sort_by_key(|c| (origin_key(c.second), origin_key(c.first)));
    found
}

fn origin_key(o: Origin) -> (usize, usize, usize) {
    match o {
        Origin::Generator(_) => (0, 0, 0),
        Origin::Register(i) => (1 + i, 0, 0),
        Origin::Field(i, j) => (1 + i, 1 + j, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::{FieldKind, RegAccess};

    fn field(name: &str, lsb: u32) -> FieldDesc {
        FieldDesc {
            name: name.into(),
            lsb,
            width: 1,
            kind: FieldKind::Bool,
            reserved: false,
            self_clearing: false,
            w1c: false,
            doc: None,
        }
    }

    fn reg(name: &str, offset: u64, fields: &[&str]) -> RegDesc {
        RegDesc {
            name: name.into(),
            offset,
            access: RegAccess::ReadWrite,
            volatile: false,
            doc: None,
            fields: fields.iter().zip(0..).map(|(f, i)| field(f, i)).collect(),
        }
    }

    fn map(regs: Vec<RegDesc>) -> RegMap {
        RegMap {
            module: "Regs".into(),
            base: 0,
            bus: "AXI4Lite".into(),
            doc: None,
            registers: regs,
        }
    }

    fn idents(m: &RegMap) -> Vec<(Lang, String)> {
        collisions(m)
            .into_iter()
            .map(|c| (c.lang, c.ident))
            .collect()
    }

    #[test]
    fn clean_map_has_no_collision() {
        let m = map(vec![
            reg("ctrl", 0, &["en", "go"]),
            reg("status", 4, &["busy"]),
        ]);
        assert_eq!(collisions(&m), []);
    }

    #[test]
    fn raw_field_hits_the_raw_word_accessor() {
        let m = map(vec![reg("ctrl", 0, &["raw", "en"])]);
        let c = collisions(&m);
        assert_eq!(c[0].ident, "ctrl_raw");
        assert_eq!(
            (c[0].first, c[0].second),
            (Origin::Register(0), Origin::Field(0, 0))
        );
        // set_ctrl_raw aynı çifttir: çift bir kez raporlanır.
        assert_eq!(c.len(), 1, "{c:?}");
    }

    #[test]
    fn register_and_field_paths_meet() {
        // irq.status_rx ve irq_status.rx → irq_status_rx (Rust + C).
        let m = map(vec![
            reg("irq", 0, &["status_rx", "x"]),
            reg("irq_status", 4, &["rx", "y"]),
        ]);
        let ids = idents(&m);
        assert!(
            ids.contains(&(Lang::Rust, "IRQ_STATUS_RX_SHIFT".into())),
            "{ids:?}"
        );
        assert!(collisions(&m)
            .iter()
            .all(|c| c.first == Origin::Field(0, 0) && c.second == Origin::Field(1, 0)));
    }

    #[test]
    fn generator_names_are_taken() {
        for (name, ident) in [
            ("new", "new"),
            ("read", "read"),
            ("h", "REGS_H"),
            ("base", "REGS_BASE"),
        ] {
            let m = map(vec![reg(name, 0, &["v"])]);
            let c = collisions(&m);
            assert!(
                c.iter()
                    .any(|c| c.ident == ident && matches!(c.first, Origin::Generator(_))),
                "{name}: {c:?}"
            );
        }
    }

    #[test]
    fn case_folding_collides_in_constants() {
        let m = map(vec![
            reg("ctrl", 0, &["a", "b"]),
            reg("Ctrl", 4, &["c", "d"]),
        ]);
        assert!(idents(&m).contains(&(Lang::Rust, "CTRL_OFFSET".into())));
    }

    #[test]
    fn c_parameter_names() {
        let m = map(vec![
            reg("ctrl", 0, &["uint32_t", "b"]),
            reg("x", 4, &["REGS_BASE", "c"]),
        ]);
        let ids = idents(&m);
        assert!(ids.contains(&(Lang::C, "uint32_t".into())), "{ids:?}");
        assert!(ids.contains(&(Lang::C, "REGS_BASE".into())), "{ids:?}");
    }

    #[test]
    fn word_field_gets_another_local() {
        assert_eq!(rmw_local(&field("word", 0)), "current");
        assert_eq!(rmw_local(&field("current", 0)), "word");
    }
}
