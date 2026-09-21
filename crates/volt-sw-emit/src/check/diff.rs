//! İki [`DriverView`] arasındaki anlamsal fark.
//!
//! Register'lar ve alanlar ADLA eşlenir (sıra ayrışma değildir). Kalemler
//! beklenen (RTL) bildirim sırasında, dosyada fazla olanlar dosya
//! sırasında sona eklenir — aynı girdi → aynı liste.

use super::{hex_offset, hex_word, DriverView, FieldView, RegView};

/// Ayrışma türü — JSON çıktısındaki `kind` anahtarı [`DriftKind::key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftKind {
    /// Dosyanın modülü tasarımda yok.
    Module,
    Base,
    /// Register offset'i (Seviye 1'de: adres RTL'de çözülmüyor).
    Offset,
    /// Register erişimi (`ReadWrite`/`ReadOnly`/`WriteOnly`).
    Access,
    /// Register'ın adlandırılmış bit maskesi.
    Mask,
    Reset,
    /// RTL'de var, dosyada yok.
    MissingRegister,
    /// Dosyada var, RTL'de yok.
    ExtraRegister,
    MissingField,
    ExtraField,
    FieldLsb,
    FieldMask,
    /// Alan davranışı (`normal`/`W1C`/`self-clearing`).
    FieldAccess,
    /// Dosya kendi içinde çelişiyor (`file` ayrıntıyı taşır).
    Inconsistent,
    /// Görünüm aynı ama erişimci kodu aynı sürümün üretiminden farklı.
    Code,
}

impl DriftKind {
    pub fn key(self) -> &'static str {
        match self {
            DriftKind::Module => "module",
            DriftKind::Base => "base",
            DriftKind::Offset => "offset",
            DriftKind::Access => "access",
            DriftKind::Mask => "mask",
            DriftKind::Reset => "reset",
            DriftKind::MissingRegister => "missing_register",
            DriftKind::ExtraRegister => "extra_register",
            DriftKind::MissingField => "missing_field",
            DriftKind::ExtraField => "extra_field",
            DriftKind::FieldLsb => "field_lsb",
            DriftKind::FieldMask => "field_mask",
            DriftKind::FieldAccess => "field_access",
            DriftKind::Inconsistent => "inconsistent",
            DriftKind::Code => "code",
        }
    }
}

/// Tek ayrışma kalemi. Metin dile göre sürücüde kurulur; burada yalnız
/// veri durur. `subject` `MOD_REG` ya da `MOD_REG.FIELD` biçimindedir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub subject: String,
    pub kind: DriftKind,
    /// Dosyadaki değer (eksik kalemde `None`).
    pub file: Option<String>,
    /// RTL'deki değer (fazla kalemde `None`).
    pub rtl: Option<String>,
}

impl Drift {
    fn pair(subject: String, kind: DriftKind, file: String, rtl: String) -> Drift {
        Drift {
            subject,
            kind,
            file: Some(file),
            rtl: Some(rtl),
        }
    }
}

/// `expected` (tasarım) ile `actual` (dosya) arasındaki kalemler.
pub fn diff(expected: &DriverView, actual: &DriverView) -> Vec<Drift> {
    let module = &expected.module;
    let mut out = Vec::new();
    if expected.base != actual.base {
        out.push(Drift::pair(
            format!("{module}_BASE"),
            DriftKind::Base,
            hex_word(actual.base),
            hex_word(expected.base),
        ));
    }
    for want in &expected.registers {
        let subject = format!("{module}_{}", want.name);
        match actual.registers.iter().find(|r| r.name == want.name) {
            Some(got) => register(&mut out, &subject, want, got),
            None => out.push(Drift {
                subject,
                kind: DriftKind::MissingRegister,
                file: None,
                rtl: Some(hex_offset(want.offset)),
            }),
        }
    }
    for got in &actual.registers {
        if !expected.registers.iter().any(|r| r.name == got.name) {
            out.push(Drift {
                subject: format!("{module}_{}", got.name),
                kind: DriftKind::ExtraRegister,
                file: Some(hex_offset(got.offset)),
                rtl: None,
            });
        }
    }
    out
}

fn register(out: &mut Vec<Drift>, subject: &str, want: &RegView, got: &RegView) {
    let s = || subject.to_string();
    if want.offset != got.offset {
        out.push(Drift::pair(
            s(),
            DriftKind::Offset,
            hex_offset(got.offset),
            hex_offset(want.offset),
        ));
    }
    if (want.readable, want.writable) != (got.readable, got.writable) {
        out.push(Drift::pair(
            s(),
            DriftKind::Access,
            got.access_name().to_string(),
            want.access_name().to_string(),
        ));
    }
    if want.mask != got.mask {
        out.push(Drift::pair(
            s(),
            DriftKind::Mask,
            hex_word(u64::from(got.mask)),
            hex_word(u64::from(want.mask)),
        ));
    }
    if want.reset != got.reset {
        out.push(Drift::pair(
            s(),
            DriftKind::Reset,
            hex_word(u64::from(got.reset)),
            hex_word(u64::from(want.reset)),
        ));
    }
    for wf in &want.fields {
        let fs = format!("{subject}.{}", wf.name);
        match got.fields.iter().find(|f| f.name == wf.name) {
            Some(gf) => field(out, fs, wf, gf),
            None => out.push(Drift {
                subject: fs,
                kind: DriftKind::MissingField,
                file: None,
                rtl: Some(bits(wf)),
            }),
        }
    }
    for gf in &got.fields {
        if !want.fields.iter().any(|f| f.name == gf.name) {
            out.push(Drift {
                subject: format!("{subject}.{}", gf.name),
                kind: DriftKind::ExtraField,
                file: Some(bits(gf)),
                rtl: None,
            });
        }
    }
}

fn field(out: &mut Vec<Drift>, subject: String, want: &FieldView, got: &FieldView) {
    if want.lsb != got.lsb {
        out.push(Drift::pair(
            subject.clone(),
            DriftKind::FieldLsb,
            got.lsb.to_string(),
            want.lsb.to_string(),
        ));
    }
    if want.mask != got.mask {
        out.push(Drift::pair(
            subject.clone(),
            DriftKind::FieldMask,
            mask_text(got.mask),
            mask_text(want.mask),
        ));
    }
    if want.behavior != got.behavior {
        out.push(Drift::pair(
            subject,
            DriftKind::FieldAccess,
            got.behavior.name().to_string(),
            want.behavior.name().to_string(),
        ));
    }
}

/// `0x7 (3 bits)`; bitişik olmayan maske yalnız hex.
fn mask_text(mask: u32) -> String {
    let ones = mask.count_ones();
    if mask != 0 && mask.trailing_ones() == ones {
        format!(
            "0x{mask:X} ({ones} bit{})",
            if ones == 1 { "" } else { "s" }
        )
    } else {
        format!("0x{mask:X}")
    }
}

/// `bits 7:0` / `bit 3` — eksik/fazla alanın yeri.
fn bits(f: &FieldView) -> String {
    let width = f.mask.count_ones().max(1);
    if width == 1 {
        format!("bit {}", f.lsb)
    } else {
        format!("bits {}:{}", f.lsb.saturating_add(width - 1), f.lsb)
    }
}
