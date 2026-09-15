//! `@mmio` register haritası desugar'ı (ADR-0044).
//!
//! `@mmio(base, bus)` modülündeki `@reg(...) ad : { alanlar }`
//! bildirimleri parser sonunda tüketilir: AXI4-Lite slave arayüzü
//! (bundle portlar, ADR-0039), register saklayıcıları, adres çözümleme,
//! okuma çoklayıcısı, yazma mantığı ve otomatik kontratlar VOLT KAYNAK
//! METNİ olarak üretilir, kendi sentetik `FileId`'siyle ayrıştırılıp
//! modüle eklenir. Gövde ve kontratlardaki `regs.reg.alan` erişimleri
//! saklayıcıya yeniden yazılır. İsim çözümleme, tip denetimi ve SV
//! üretimi `@reg`'i HİÇ görmez (ADR-0038'in silme ilkesi).
//!
//! Sahiplik ve saklama:
//! - `volatile` olmayan, yazılabilir register BUS'a aittir: tam 32 bitlik
//!   `reg mmio_<reg> : u32` (rezerve bitler dahil — böylece `w_data` ve
//!   `w_strb`'nin her biti kullanılır, Verilator -Wall temiz kalır);
//!   okuma görünümü rezerve bitleri maskeler. RTL alanı `mmio_<reg>[hi:lo]`
//!   dilimiyle OKUR, yazması E4006.
//! - `volatile` register DONANIMA aittir: alan başına saklayıcı
//!   (`reg mmio_<reg>_<alan> : T`), RTL `<=` ile yazar, bus okur; `@w1c`
//!   alanı yazılım 1 yazınca temizlenir.
//! - ReadOnly + volatile olmayan register SABİTTİR: kimse yazmaz,
//!   sıfırlama değerini (0) okur; alanları `let` sabitidir.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use volt_ast::mmio::{FieldDesc, FieldKind, RegAccess, RegDesc, RegMap};
use volt_ast::{
    AttrArg, Block, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind, LValue,
    LValueSuffix, MatchArmBody, MmioRegDecl, Name, NumBase, Path, Stmt, StmtKind, TypeRef,
    TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::{FileId, Span};

use super::desugar::{collect_arm_idxs, expr_children, lvalue_suffix_exprs};
use super::{GeneratedSource, Parser};

/// Register sözcüğü genişliği (AXI4-Lite veri yolu).
const WORD_BITS: u32 = 32;
/// Sentetik modülün adı — gövdesi kullanıcı modülüne aktarılır, kendisi
/// arenadan çıkarılır.
const SYNTH_MODULE: &str = "__VoltMmio";
/// Kullanıcı tarafındaki register tutamacı: `regs.control.enable`.
const REGS_HANDLE: &str = "regs";
/// Üretilen tüm isimlerin öneki.
const PREFIX: &str = "mmio_";
/// Desteklenen tek bus.
const BUS_AXI4LITE: &str = "AXI4Lite";
/// Tanınan `@reg` erişim türleri.
const ACCESS_NAMES: &[&str] = &["ReadWrite", "ReadOnly", "WriteOnly"];
/// AXI4-Lite kanal bundle'ları (examples/axi4lite_slave.volt ile aynı
/// tanım; birimde zaten varsa üretilmez).
const AXI_BUNDLES: &[(&str, &str)] = &[
    (
        "AxiWriteAddr",
        "    out addr  : u32\n    out prot  : u3\n    out valid : bool\n    in  ready : bool\n",
    ),
    (
        "AxiWriteData",
        "    out data  : u32\n    out strb  : u4\n    out valid : bool\n    in  ready : bool\n",
    ),
    (
        "AxiWriteResp",
        "    in  resp  : u2\n    in  valid : bool\n    out ready : bool\n",
    ),
    (
        "AxiReadAddr",
        "    out addr  : u32\n    out prot  : u3\n    out valid : bool\n    in  ready : bool\n",
    ),
    (
        "AxiReadData",
        "    in  data  : u32\n    in  resp  : u2\n    in  valid : bool\n    out ready : bool\n",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    ReadWrite,
    ReadOnly,
    WriteOnly,
}

impl Access {
    fn readable(self) -> bool {
        self != Access::WriteOnly
    }
    fn writable(self) -> bool {
        self != Access::ReadOnly
    }
}

/// Alan tipi (genişliğiyle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldTy {
    Bool,
    Bits(u32),
    UInt(u32),
}

impl FieldTy {
    fn width(self) -> u32 {
        match self {
            FieldTy::Bool => 1,
            FieldTy::Bits(w) | FieldTy::UInt(w) => w,
        }
    }

    /// Volt tip yazımı (`bool`, `bits<8>`, `u8`).
    fn volt(self) -> String {
        match self {
            FieldTy::Bool => "bool".to_string(),
            FieldTy::Bits(w) => format!("bits<{w}>"),
            FieldTy::UInt(w) => format!("u{w}"),
        }
    }

    /// Sıfır sabiti: `bits<N>` literal kabul etmez, genişliği bilinen
    /// bir kaynaktan açık dönüşüm gerekir (uyarısız: yalnız genişleme).
    fn zero(self) -> String {
        match self {
            FieldTy::Bool => "false".to_string(),
            FieldTy::UInt(_) => "0".to_string(),
            FieldTy::Bits(8) => "0u8 as bits<8>".to_string(),
            FieldTy::Bits(w) if w > 8 => format!("(0u8 as u{w}) as bits<{w}>"),
            FieldTy::Bits(w) => format!("((false as u1) as u{w}) as bits<{w}>"),
        }
    }
}

#[derive(Debug)]
struct FieldInfo {
    /// `@reserved` alanında None.
    name: Option<String>,
    lo: u32,
    ty: FieldTy,
    self_clearing: bool,
    w1c: bool,
    /// `///` doc yorumu — yalnız yazılım tarafına (ADR-0053) aktarılır.
    doc: Option<String>,
}

impl FieldInfo {
    fn mask(&self) -> u64 {
        ((1u64 << self.ty.width()) - 1) << self.lo
    }
}

/// Register sahipliği — saklama biçimini belirler (modül belgesi).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// Tam sözcük `reg mmio_<reg> : u32`; alanlar dilim görünümü.
    Bus,
    /// Alan başına `reg`; RTL yazar.
    Hardware,
    /// Alan başına `let` sabiti (0).
    Const,
}

#[derive(Debug)]
struct RegInfo {
    name: String,
    /// `base + offset` — tam 32 bit adres karşılaştırması.
    addr: u64,
    /// `@reg(offset = ...)` — yazılım haritası taban göreli offset ister.
    offset: u64,
    access: Access,
    fields: Vec<FieldInfo>,
    owner: Owner,
    span: Span,
    /// `///` doc yorumu — yazılım tarafına (ADR-0053) aktarılır.
    doc: Option<String>,
}

impl RegInfo {
    fn named(&self) -> impl Iterator<Item = &FieldInfo> {
        self.fields.iter().filter(|f| f.name.is_some())
    }
    fn field(&self, name: &str) -> Option<&FieldInfo> {
        self.named().find(|f| f.name.as_deref() == Some(name))
    }
    fn has_w1c(&self) -> bool {
        self.named().any(|f| f.w1c)
    }
    /// Adlandırılmış alan bitlerinin maskesi (rezerve bitler 0 okur).
    fn read_mask(&self) -> u64 {
        self.named().fold(0, |m, f| m | f.mask())
    }
    fn word(&self) -> String {
        format!("{PREFIX}{}", self.name)
    }
    fn rd(&self) -> String {
        format!("{PREFIX}{}_rd", self.name)
    }
}

/// RTL'e açılan haritanın yazılım tarafı kopyası (ADR-0053): aynı
/// `RegInfo` listesinden üretilir, bu yüzden sürücü ile RTL ayrışamaz.
fn regmap(module: &str, base: u64, doc: Option<String>, regs: &[RegInfo]) -> RegMap {
    let registers = regs
        .iter()
        .map(|r| RegDesc {
            name: r.name.clone(),
            offset: r.offset,
            access: match r.access {
                Access::ReadWrite => RegAccess::ReadWrite,
                Access::ReadOnly => RegAccess::ReadOnly,
                Access::WriteOnly => RegAccess::WriteOnly,
            },
            volatile: r.owner == Owner::Hardware,
            doc: r.doc.clone(),
            fields: r
                .fields
                .iter()
                .map(|f| FieldDesc {
                    name: f.name.clone().unwrap_or_else(|| "_reserved".to_string()),
                    lsb: f.lo,
                    width: f.ty.width(),
                    kind: match f.ty {
                        FieldTy::Bool => FieldKind::Bool,
                        FieldTy::Bits(_) => FieldKind::Bits,
                        FieldTy::UInt(_) => FieldKind::UInt,
                    },
                    reserved: f.name.is_none(),
                    self_clearing: f.self_clearing,
                    w1c: f.w1c,
                    doc: f.doc.clone(),
                })
                .collect(),
        })
        .collect();
    RegMap {
        module: module.to_string(),
        base,
        bus: BUS_AXI4LITE.to_string(),
        doc,
        registers,
    }
}

fn flat_name(reg: &str, field: &str) -> String {
    format!("{PREFIX}{reg}_{field}")
}

/// `regs.<reg>.<alan>` için yeniden yazım hedefi.
#[derive(Debug, Clone)]
enum Rewrite {
    /// Düz isim (donanım saklayıcısı ya da sabit).
    Name(String),
    /// Bus sözcüğünün dilimi: `mmio_<reg>[hi:lo]` (+ `as uN`).
    View { word: String, lo: u32, ty: FieldTy },
}

/// Üretilen ve ayrıştırılmış parçalar.
struct Generated {
    ports: Vec<volt_ast::Port>,
    contracts: Vec<volt_ast::Contract>,
    /// Bildirimler (reg/let) — kullanıcı gövdesinin ÖNÜNE girer.
    head: Vec<Idx<Stmt>>,
    /// on bloğu + çıkış atamaları — kullanıcı gövdesinin ARKASINA.
    tail: Vec<Idx<Stmt>>,
    /// `@w1c` temizleme blokları (düz alan adı, on deyimi).
    w1c: Vec<(String, Idx<Stmt>)>,
}

/// Beş parçalı tanı kısayolu: kod, konum, açıklama, etiket, öneri.
fn err(code: ErrorCode, span: Span, msg: String, label: String, help: String) -> Diagnostic {
    Diagnostic::error(code, msg, LabeledSpan::primary(span, label), help)
}

impl Parser<'_> {
    /// Birimdeki tüm `@mmio` modüllerini açar; `@mmio` olmayan modüldeki
    /// `@reg` bildirimlerini E0015 ile düşürür.
    pub(crate) fn desugar_mmio(&mut self) {
        let items: Vec<_> = self.ast.items.clone();
        for item in items {
            let regs = match &mut self.ast.items_arena[item].kind {
                ItemKind::Module(m) => std::mem::take(&mut m.mmio_regs),
                _ => continue,
            };
            let mmio = self.ast.items_arena[item]
                .attrs
                .iter()
                .position(|a| a.name.text == "mmio");
            let Some(attr_idx) = mmio else {
                for r in &regs {
                    self.push_error(err(
                        ErrorCode::E0015,
                        r.span,
                        lstr!(en: "'@reg' register '{}' declared in a module without '@mmio'", r.name.text; tr: "'@reg' register'ı '{}' '@mmio' olmayan bir modülde bildirildi", r.name.text),
                        lstr!(en: "no '@mmio' on this module"; tr: "bu modülde '@mmio' yok"),
                        lstr!(en: "write @mmio(base = 0x4000_0000, bus = AXI4Lite) before 'module' (ADR-0044)"; tr: "'module' önüne @mmio(base = 0x4000_0000, bus = AXI4Lite) yazın (ADR-0044)"),
                    ));
                }
                continue;
            };
            let Some((base, module_name)) = self.mmio_header(item, attr_idx) else {
                continue;
            };
            let infos = self.collect_regs(regs, base);
            self.check_overlap(&infos);
            let clock = self.module_clock(item);
            let text = self.render(&module_name, base, &infos, clock.as_deref());
            let file = FileId(self.next_synthetic);
            self.next_synthetic += 1;
            let Some(gen) = self.parse_generated(file, &module_name, text) else {
                continue;
            };
            let doc = self.ast.items_arena[item].doc.clone();
            self.regmaps.push(regmap(&module_name, base, doc, &infos));
            self.rewrite_user_side(item, &infos);
            self.splice(item, gen);
        }
    }

    /// `@mmio(base = ..., bus = ...)` argümanları; hata varsa E0009 ve None.
    fn mmio_header(&mut self, item: Idx<volt_ast::Item>, attr_idx: usize) -> Option<(u64, String)> {
        let name = match &self.ast.items_arena[item].kind {
            ItemKind::Module(m) => m.name.text.clone(),
            _ => return None,
        };
        let args = attr_args(&self.ast.items_arena[item].attrs[attr_idx].args);
        let mut base = 0u64;
        let mut ok = true;
        for (arg_name, value) in args {
            let vspan = self.ast.exprs[value].span;
            match arg_name.as_deref() {
                Some("base") => match self.int_lit(value) {
                    Some(v) if v <= u64::from(u32::MAX) => base = v,
                    _ => {
                        ok = false;
                        self.push_error(err(
                            ErrorCode::E0009,
                            vspan,
                            lstr!(en: "'base' must be a 32-bit integer literal"; tr: "'base' 32 bitlik bir tam sayı literali olmalı"),
                            lstr!(en: "not a 32-bit literal"; tr: "32 bitlik literal değil"),
                            lstr!(en: "write it as @mmio(base = 0x4000_0000, bus = AXI4Lite)"; tr: "@mmio(base = 0x4000_0000, bus = AXI4Lite) biçiminde yazın"),
                        ));
                    }
                },
                Some("bus") => {
                    let bus = self.path_ident(value);
                    if bus.as_deref() != Some(BUS_AXI4LITE) {
                        ok = false;
                        let shown = bus.unwrap_or_else(|| "?".to_string());
                        self.push_error(err(
                            ErrorCode::E0009,
                            vspan,
                            lstr!(en: "unsupported bus '{shown}'"; tr: "desteklenmeyen bus '{shown}'"),
                            lstr!(en: "only AXI4Lite is supported"; tr: "yalnız AXI4Lite destekleniyor"),
                            lstr!(en: "write bus = AXI4Lite (ADR-0044 supports one bus adapter)"; tr: "bus = AXI4Lite yazın (ADR-0044 tek bus adaptörü destekler)"),
                        ));
                    }
                }
                other => {
                    ok = false;
                    let shown = other.unwrap_or("<positional>");
                    self.push_error(err(
                        ErrorCode::E0009,
                        vspan,
                        lstr!(en: "unknown '@mmio' argument '{shown}'"; tr: "bilinmeyen '@mmio' argümanı '{shown}'"),
                        lstr!(en: "expected base = ... or bus = ..."; tr: "base = ... veya bus = ... bekleniyor"),
                        lstr!(en: "@mmio takes only 'base' and 'bus'"; tr: "@mmio yalnız 'base' ve 'bus' alır"),
                    ));
                }
            }
        }
        ok.then_some((base, name))
    }

    fn int_lit(&self, e: Idx<Expr>) -> Option<u64> {
        match &self.ast.exprs[e].kind {
            ExprKind::IntLit { value, .. } => u64::try_from(*value).ok(),
            _ => None,
        }
    }

    fn path_ident(&self, e: Idx<Expr>) -> Option<String> {
        match &self.ast.exprs[e].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => Some(p.segments[0].text.clone()),
            _ => None,
        }
    }

    /// Alan tipini sınıflandırır (bool / bits<N> / uN; N literal).
    fn field_ty(&self, ty: Idx<TypeRef>) -> Option<FieldTy> {
        let lit_width = |e: Idx<Expr>| {
            self.int_lit(e)
                .and_then(|w| u32::try_from(w).ok())
                .filter(|w| (1..=WORD_BITS).contains(w))
        };
        match &self.ast.types[ty].kind {
            TypeRefKind::Bool => Some(FieldTy::Bool),
            TypeRefKind::UInt(w) if u32::from(*w) <= WORD_BITS => {
                Some(FieldTy::UInt(u32::from(*w)))
            }
            TypeRefKind::UIntN(e) => lit_width(*e).map(FieldTy::UInt),
            TypeRefKind::Bits(e) => lit_width(*e).map(FieldTy::Bits),
            _ => None,
        }
    }

    /// `@reg` bildirimlerini doğrular ve RegInfo'ya çevirir; hatalı
    /// register düşer (E0009 argüman, E0015 yerleşim).
    fn collect_regs(&mut self, regs: Vec<MmioRegDecl>, base: u64) -> Vec<RegInfo> {
        let mut out: Vec<RegInfo> = Vec::new();
        let mut names: HashMap<String, Span> = HashMap::new();
        for r in regs {
            let Some(info) = self.collect_reg(&r, base) else {
                continue;
            };
            if let Some(prev) = names.get(&info.name) {
                self.push_error(
                    err(
                        ErrorCode::E0015,
                        r.name.span,
                        lstr!(en: "register '{}' is declared twice", info.name; tr: "register '{}' iki kez bildirildi", info.name),
                        lstr!(en: "second declaration"; tr: "ikinci bildirim"),
                        lstr!(en: "give each register a unique name"; tr: "her register'a benzersiz bir ad verin"),
                    )
                    .with_secondary(*prev, lstr!(en: "first declaration"; tr: "ilk bildirim")),
                );
                continue;
            }
            names.insert(info.name.clone(), r.name.span);
            out.push(info);
        }
        out
    }

    fn collect_reg(&mut self, r: &MmioRegDecl, base: u64) -> Option<RegInfo> {
        let reg_attr = r.attrs.iter().find(|a| a.name.text == "reg")?;
        let attr_span = reg_attr.span;
        let args = attr_args(&reg_attr.args);
        let mut offset: Option<u64> = None;
        let mut access = Access::ReadWrite;
        let mut volatile = false;
        let mut ok = true;
        for (arg_name, value) in args {
            let vspan = self.ast.exprs[value].span;
            match arg_name.as_deref() {
                Some("offset") => match self.int_lit(value) {
                    Some(v) => offset = Some(v),
                    None => {
                        ok = false;
                        self.push_error(err(
                            ErrorCode::E0009,
                            vspan,
                            lstr!(en: "'offset' must be an integer literal"; tr: "'offset' bir tam sayı literali olmalı"),
                            lstr!(en: "not a literal"; tr: "literal değil"),
                            lstr!(en: "write it as @reg(offset = 0x04, access = ReadWrite)"; tr: "@reg(offset = 0x04, access = ReadWrite) biçiminde yazın"),
                        ));
                    }
                },
                Some("access") => match self.path_ident(value).as_deref() {
                    Some("ReadWrite") => access = Access::ReadWrite,
                    Some("ReadOnly") => access = Access::ReadOnly,
                    Some("WriteOnly") => access = Access::WriteOnly,
                    other => {
                        ok = false;
                        let shown = other.unwrap_or("?").to_string();
                        let known = ACCESS_NAMES.join(", ");
                        self.push_error(err(
                            ErrorCode::E0009,
                            vspan,
                            lstr!(en: "unknown access '{shown}'"; tr: "bilinmeyen erişim '{shown}'"),
                            lstr!(en: "expected {known}"; tr: "{known} bekleniyor"),
                            lstr!(en: "write access = ReadWrite, ReadOnly or WriteOnly"; tr: "access = ReadWrite, ReadOnly veya WriteOnly yazın"),
                        ));
                    }
                },
                None if self.path_ident(value).as_deref() == Some("volatile") => volatile = true,
                other => {
                    ok = false;
                    let shown = other.unwrap_or("<positional>").to_string();
                    self.push_error(err(
                        ErrorCode::E0009,
                        vspan,
                        lstr!(en: "unknown '@reg' argument '{shown}'"; tr: "bilinmeyen '@reg' argümanı '{shown}'"),
                        lstr!(en: "expected offset = ..., access = ... or volatile"; tr: "offset = ..., access = ... veya volatile bekleniyor"),
                        lstr!(en: "@reg takes 'offset', 'access' and the 'volatile' flag"; tr: "@reg yalnız 'offset', 'access' ve 'volatile' bayrağını alır"),
                    ));
                }
            }
        }
        let Some(offset) = offset else {
            self.push_error(err(
                ErrorCode::E0009,
                attr_span,
                lstr!(en: "'@reg' on '{}' has no 'offset'", r.name.text; tr: "'{}' üzerindeki '@reg' 'offset' içermiyor", r.name.text),
                lstr!(en: "offset is required"; tr: "offset zorunlu"),
                lstr!(en: "write it as @reg(offset = 0x00, access = ReadWrite)"; tr: "@reg(offset = 0x00, access = ReadWrite) biçiminde yazın"),
            ));
            return None;
        };
        if !ok {
            return None;
        }
        if volatile && access != Access::ReadOnly {
            self.push_error(err(
                ErrorCode::E0009,
                attr_span,
                lstr!(en: "volatile register '{}' must be ReadOnly", r.name.text; tr: "volatile register '{}' ReadOnly olmalı", r.name.text),
                lstr!(en: "bus writes would race the hardware writes"; tr: "bus yazmaları donanım yazmalarıyla yarışırdı"),
                lstr!(en: "write access = ReadOnly, volatile; use a @w1c field for software-cleared flags"; tr: "access = ReadOnly, volatile yazın; yazılımın temizlediği bayraklar için @w1c alanı kullanın"),
            ));
            return None;
        }
        if offset % 4 != 0 || base + offset > u64::from(u32::MAX) {
            self.push_error(err(
                ErrorCode::E0015,
                attr_span,
                lstr!(en: "offset {offset:#x} of register '{}' is not a 4-byte aligned 32-bit address", r.name.text; tr: "'{}' register'ının offset'i {offset:#x} 4 bayta hizalı 32 bitlik bir adres değil", r.name.text),
                lstr!(en: "misaligned or out-of-range offset"; tr: "hizasız ya da aralık dışı offset"),
                lstr!(en: "registers are 32-bit words: use offsets 0x00, 0x04, 0x08, ..."; tr: "register'lar 32 bitlik sözcüktür: 0x00, 0x04, 0x08, ... offset'lerini kullanın"),
            ));
            return None;
        }
        let owner = if volatile {
            Owner::Hardware
        } else if access.writable() {
            Owner::Bus
        } else {
            Owner::Const
        };
        let fields = self.collect_fields(r, owner)?;
        Some(RegInfo {
            name: r.name.text.clone(),
            addr: base + offset,
            offset,
            access,
            fields,
            owner,
            span: r.span,
            doc: r.doc.clone(),
        })
    }

    fn collect_fields(&mut self, r: &MmioRegDecl, owner: Owner) -> Option<Vec<FieldInfo>> {
        let mut fields = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut lo = 0u32;
        let mut ok = true;
        for f in &r.fields {
            let Some(ty) = self.field_ty(f.ty) else {
                ok = false;
                self.push_error(err(
                    ErrorCode::E0015,
                    self.ast.types[f.ty].span,
                    lstr!(en: "register field type must be bool, bits<N> or uN (N <= 32)"; tr: "register alanı tipi bool, bits<N> veya uN olmalı (N <= 32)"),
                    lstr!(en: "unsupported field type"; tr: "desteklenmeyen alan tipi"),
                    lstr!(en: "write the field as pins : bits<8> or enable : bool"; tr: "alanı pins : bits<8> ya da enable : bool olarak yazın"),
                ));
                continue;
            };
            let mut self_clearing = false;
            let mut w1c = false;
            for a in &f.attrs {
                match a.name.text.as_str() {
                    "reserved" => {}
                    "self_clearing" => self_clearing = true,
                    "w1c" => w1c = true,
                    other => {
                        ok = false;
                        self.push_error(err(
                            ErrorCode::E0009,
                            a.span,
                            lstr!(en: "'@{other}' is not a register field attribute"; tr: "'@{other}' bir register alanı niteliği değil"),
                            lstr!(en: "expected @reserved, @self_clearing or @w1c"; tr: "@reserved, @self_clearing veya @w1c bekleniyor"),
                            lstr!(en: "remove the attribute (ADR-0044 lists the field attributes)"; tr: "niteliği kaldırın (alan nitelikleri ADR-0044'te listelenir)"),
                        ));
                    }
                }
            }
            let name = f.name.as_ref().map(|n| n.text.clone());
            if let Some(n) = &name {
                if !seen.insert(n.clone()) {
                    ok = false;
                    self.push_error(err(
                        ErrorCode::E0015,
                        f.span,
                        lstr!(en: "field '{n}' is declared twice in register '{}'", r.name.text; tr: "'{n}' alanı '{}' register'ında iki kez bildirildi", r.name.text),
                        lstr!(en: "duplicate field"; tr: "yinelenen alan"),
                        lstr!(en: "give each field a unique name"; tr: "her alana benzersiz bir ad verin"),
                    ));
                }
            }
            if (self_clearing || w1c) && (name.is_none() || ty != FieldTy::Bool) {
                ok = false;
                self.push_error(err(
                    ErrorCode::E0009,
                    f.span,
                    lstr!(en: "@self_clearing / @w1c apply only to named bool fields"; tr: "@self_clearing / @w1c yalnız adlandırılmış bool alanlara uygulanır"),
                    lstr!(en: "not a named bool field"; tr: "adlandırılmış bool alan değil"),
                    lstr!(en: "declare the flag as name : bool @w1c"; tr: "bayrağı ad : bool @w1c olarak bildirin"),
                ));
            }
            if w1c && owner != Owner::Hardware {
                ok = false;
                self.push_error(err(
                    ErrorCode::E0009,
                    f.span,
                    lstr!(en: "@w1c field in a non-volatile register"; tr: "volatile olmayan register'da @w1c alanı"),
                    lstr!(en: "hardware must own the flag"; tr: "bayrağın sahibi donanım olmalı"),
                    lstr!(en: "declare the register as access = ReadOnly, volatile"; tr: "register'ı access = ReadOnly, volatile olarak bildirin"),
                ));
            }
            if self_clearing && owner != Owner::Bus {
                ok = false;
                self.push_error(err(
                    ErrorCode::E0009,
                    f.span,
                    lstr!(en: "@self_clearing field in a register the bus cannot write"; tr: "bus'ın yazamadığı register'da @self_clearing alanı"),
                    lstr!(en: "nothing would ever set it"; tr: "onu hiçbir şey kuramazdı"),
                    lstr!(en: "self-clearing fields live in ReadWrite or WriteOnly, non-volatile registers"; tr: "self-clearing alanlar ReadWrite ya da WriteOnly, volatile olmayan register'larda yaşar"),
                ));
            }
            fields.push(FieldInfo {
                name,
                lo,
                ty,
                self_clearing,
                w1c,
                doc: f.doc.clone(),
            });
            lo += ty.width();
        }
        if lo > WORD_BITS {
            ok = false;
            self.push_error(err(
                ErrorCode::E0015,
                r.span,
                lstr!(en: "fields of register '{}' total {lo} bits, more than the 32-bit word", r.name.text; tr: "'{}' register'ının alanları toplam {lo} bit, 32 bitlik sözcükten fazla", r.name.text),
                lstr!(en: "register overflows the word"; tr: "register sözcüğü taşırıyor"),
                lstr!(en: "shrink the fields or split the register into two words"; tr: "alanları küçültün ya da register'ı iki sözcüğe bölün"),
            ));
        }
        if ok && fields.iter().all(|f| f.name.is_none()) {
            ok = false;
            self.push_error(err(
                ErrorCode::E0015,
                r.span,
                lstr!(en: "register '{}' has no named field", r.name.text; tr: "'{}' register'ının adlandırılmış alanı yok", r.name.text),
                lstr!(en: "only @reserved fields"; tr: "yalnız @reserved alanlar"),
                lstr!(en: "give the register at least one named field, or drop it"; tr: "register'a en az bir adlandırılmış alan verin ya da onu kaldırın"),
            ));
        }
        ok.then_some(fields)
    }

    /// Aynı adresteki iki register → E0015.
    fn check_overlap(&mut self, infos: &[RegInfo]) {
        for (i, a) in infos.iter().enumerate() {
            if let Some(b) = infos[..i].iter().find(|b| b.addr == a.addr) {
                self.push_error(
                    err(
                        ErrorCode::E0015,
                        a.span,
                        lstr!(en: "registers '{}' and '{}' overlap at address {:#x}", b.name, a.name, a.addr; tr: "'{}' ve '{}' register'ları {:#x} adresinde çakışıyor", b.name, a.name, a.addr),
                        lstr!(en: "same offset as '{}'", b.name; tr: "'{}' ile aynı offset", b.name),
                        lstr!(en: "give each register its own 4-byte offset"; tr: "her register'a kendi 4 baytlık offset'ini verin"),
                    )
                    .with_secondary(b.span, lstr!(en: "'{}' declared here", b.name; tr: "'{}' burada bildirildi", b.name)),
                );
            }
        }
    }

    /// Modülün ilk `clock` portunun adı; yoksa None (üretim `clk` ekler).
    fn module_clock(&self, item: Idx<volt_ast::Item>) -> Option<String> {
        let ItemKind::Module(m) = &self.ast.items_arena[item].kind else {
            return None;
        };
        m.ports
            .iter()
            .find(|p| matches!(self.ast.types[p.ty].kind, TypeRefKind::Clock))
            .map(|p| p.name.text.clone())
    }

    fn unit_has_item(&self, name: &str) -> bool {
        self.ast
            .items
            .iter()
            .any(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Struct(s) => s.name.text == name,
                ItemKind::Module(m) => m.name.text == name,
                ItemKind::Enum(e) => e.name.text == name,
                ItemKind::TypeAlias(t) => t.name.text == name,
                _ => false,
            })
    }

    // ═══ Metin üretimi ════════════════════════════════════════════

    /// Sentetik Volt kaynağı: eksik AXI bundle'ları + `__VoltMmio` modülü.
    fn render(&self, module: &str, base: u64, regs: &[RegInfo], clock: Option<&str>) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "// Generated by the @mmio desugar (ADR-0044) for module {module}."
        );
        let _ = writeln!(
            s,
            "// base = {base:#010x}, bus = AXI4-Lite. Derived from the @reg declarations;"
        );
        let _ = writeln!(
            s,
            "// the text is not a file on disk, it is shown only in diagnostics."
        );
        for (name, body) in AXI_BUNDLES {
            if !self.unit_has_item(name) {
                let _ = write!(s, "\npub struct port {name} {{\n{body}}}\n");
            }
        }
        let clk = clock.unwrap_or("clk");
        let _ = write!(s, "\nmodule {SYNTH_MODULE} {{\n");
        if clock.is_none() {
            let _ = writeln!(s, "    in  clk : clock");
        }
        s.push_str(
            "    in  aw : AxiWriteAddr\n    in  w  : AxiWriteData\n    in  b  : AxiWriteResp\n",
        );
        s.push_str("    in  ar : AxiReadAddr\n    in  r  : AxiReadData\n\n");
        render_contracts(&mut s, regs);
        render_decls(&mut s, regs);
        render_logic(&mut s, regs, clk);
        s.push_str("}\n");
        s
    }

    // ═══ Sentetik metnin ayrıştırılması ═══════════════════════════

    fn parse_generated(&mut self, file: FileId, module: &str, text: String) -> Option<Generated> {
        let mut sub = Parser::new(file, &text);
        sub.ast = std::mem::take(&mut self.ast);
        sub.parse_items_only();
        let result = sub.finish();
        self.ast = result.ast;
        let failed = !result.diagnostics.is_empty();
        self.diagnostics.extend(result.diagnostics);
        self.generated.push(GeneratedSource {
            file,
            name: format!("<mmio:{module}>"),
            text,
        });
        // Sentetik modül listede son öğedir; arenada Error olarak kalır.
        let idx = self.ast.items.pop()?;
        let kind = std::mem::replace(&mut self.ast.items_arena[idx].kind, ItemKind::Error);
        let ItemKind::Module(m) = kind else {
            return None;
        };
        if failed {
            return None;
        }
        let mut head = Vec::new();
        let mut tail = Vec::new();
        let mut w1c = Vec::new();
        for si in m.body {
            match &self.ast.stmts[si].kind {
                StmtKind::Reg(_) | StmtKind::Let(_) if tail.is_empty() => head.push(si),
                StmtKind::On(on) => match self.w1c_target(on.body) {
                    Some(name) => w1c.push((name, si)),
                    None => tail.push(si),
                },
                _ => tail.push(si),
            }
        }
        Some(Generated {
            ports: m.ports,
            contracts: m.contracts,
            head,
            tail,
            w1c,
        })
    }

    /// Üretilen `on` bloğu bir w1c temizleyicisi mi? Yapısal tanıma: tek
    /// `if` (else'siz), gövdesinde tek `mmio_… <= false`. Ana blok her
    /// zaman birden çok deyim taşır, karışmaz.
    fn w1c_target(&self, block: Idx<Block>) -> Option<String> {
        let [BlockStmt::If(ifs)] = self.ast.blocks[block].stmts.as_slice() else {
            return None;
        };
        if ifs.else_branch.is_some() {
            return None;
        }
        let [BlockStmt::NonBlockAssign { lhs, rhs, .. }] =
            self.ast.blocks[ifs.then_block].stmts.as_slice()
        else {
            return None;
        };
        matches!(self.ast.exprs[*rhs].kind, ExprKind::BoolLit(false))
            .then(|| lhs.base.text.clone())
            .filter(|n| n.starts_with(PREFIX))
    }

    // ═══ Kullanıcı tarafı: E4006 + regs.reg.alan yeniden yazımı ═══

    fn rewrite_user_side(&mut self, item: Idx<volt_ast::Item>, regs: &[RegInfo]) {
        let (body, contracts) = match &self.ast.items_arena[item].kind {
            ItemKind::Module(m) => (
                m.body.clone(),
                m.contracts.iter().map(|c| c.expr).collect::<Vec<_>>(),
            ),
            _ => return,
        };
        let mut map: HashMap<String, Rewrite> = HashMap::new();
        for r in regs {
            for f in r.named() {
                let field = f.name.as_deref().unwrap_or_default();
                let target = match r.owner {
                    Owner::Bus => Rewrite::View {
                        word: r.word(),
                        lo: f.lo,
                        ty: f.ty,
                    },
                    Owner::Hardware | Owner::Const => Rewrite::Name(flat_name(&r.name, field)),
                };
                map.insert(format!("{REGS_HANDLE}.{}.{field}", r.name), target);
            }
        }
        // E4006: bus'a ait ya da sabit alana RTL yazması (kaynak adlarıyla).
        let mut writes = Vec::new();
        for &si in &body {
            self.collect_lvalues_stmt(si, &mut writes);
        }
        for (span, reg_name, field_name) in writes {
            let Some(r) = regs.iter().find(|r| r.name == reg_name) else {
                continue;
            };
            if r.owner == Owner::Hardware || r.field(&field_name).is_none() {
                continue;
            }
            self.push_error(
                err(
                    ErrorCode::E4006,
                    span,
                    lstr!(en: "register field 'regs.{reg_name}.{field_name}' is owned by the bus; RTL cannot write it"; tr: "'regs.{reg_name}.{field_name}' register alanı bus'a ait; RTL ona yazamaz"),
                    lstr!(en: "written from RTL"; tr: "RTL'den yazılıyor"),
                    lstr!(en: "declare '{reg_name}' as @reg(..., access = ReadOnly, volatile) if the hardware drives it"; tr: "donanım sürüyorsa '{reg_name}' register'ını @reg(..., access = ReadOnly, volatile) olarak bildirin"),
                )
                .with_secondary(r.span, lstr!(en: "'{reg_name}' declared here without 'volatile'"; tr: "'{reg_name}' burada 'volatile' olmadan bildirildi"))
                .with_note(
                    NoteKind::Reason,
                    lstr!(en: "a non-volatile register is written by the generated bus logic; a second writer would be a double driver (ADR-0044)"; tr: "volatile olmayan register'ı üretilen bus mantığı yazar; ikinci yazar çift sürücü olurdu (ADR-0044)"),
                ),
            );
        }
        for e in contracts {
            self.mmio_rw_expr(e, &map);
        }
        for si in body {
            self.mmio_rw_stmt(si, &map);
        }
    }

    fn collect_lvalues_stmt(&self, si: Idx<Stmt>, out: &mut Vec<(Span, String, String)>) {
        match &self.ast.stmts[si].kind {
            StmtKind::Assign(a) => push_regs_lvalue(&a.lhs, out),
            StmtKind::On(on) => self.collect_lvalues_block(on.body, out),
            StmtKind::Comb(b) => self.collect_lvalues_block(*b, out),
            StmtKind::For(f) => self.collect_lvalues_block(f.body, out),
            _ => {}
        }
    }

    fn collect_lvalues_block(&self, bi: Idx<Block>, out: &mut Vec<(Span, String, String)>) {
        for bs in &self.ast.blocks[bi].stmts {
            match bs {
                BlockStmt::NonBlockAssign { lhs, .. } | BlockStmt::BlockAssign { lhs, .. } => {
                    push_regs_lvalue(lhs, out);
                }
                BlockStmt::If(ifs) => self.collect_lvalues_if(ifs, out),
                BlockStmt::Match(m) => {
                    for arm in &m.arms {
                        if let MatchArmBody::Block(b) = &arm.body {
                            self.collect_lvalues_block(*b, out);
                        }
                    }
                }
                BlockStmt::For(f) => self.collect_lvalues_block(f.body, out),
                BlockStmt::Let(_) | BlockStmt::Error => {}
            }
        }
    }

    fn collect_lvalues_if(&self, ifs: &IfStmt, out: &mut Vec<(Span, String, String)>) {
        self.collect_lvalues_block(ifs.then_block, out);
        match &ifs.else_branch {
            Some(ElseBranch::Block(b)) => self.collect_lvalues_block(*b, out),
            Some(ElseBranch::If(inner)) => self.collect_lvalues_if(inner, out),
            None => {}
        }
    }

    // ─── Yeniden yazım yürüyüşü (bundle.rs ile aynı iskelet) ───

    fn mmio_rw_stmt(&mut self, si: Idx<Stmt>, map: &HashMap<String, Rewrite>) {
        let mut exprs: Vec<Idx<Expr>> = Vec::new();
        let mut blocks: Vec<Idx<Block>> = Vec::new();
        match &mut self.ast.stmts[si].kind {
            StmtKind::Reg(r) => exprs.push(r.init),
            StmtKind::Let(l) => exprs.push(l.value),
            StmtKind::Assign(a) => {
                rw_lvalue(&mut a.lhs, map);
                exprs.push(a.rhs);
                exprs.extend(a.lhs.suffixes.iter().flat_map(lvalue_suffix_exprs));
            }
            StmtKind::On(on) => blocks.push(on.body),
            StmtKind::Comb(b) => blocks.push(*b),
            StmtKind::For(f) => {
                exprs.extend([f.start, f.end]);
                blocks.push(f.body);
            }
            StmtKind::Expr(e) => exprs.push(*e),
            StmtKind::Instance(inst) => {
                exprs.extend(inst.bindings.iter().filter_map(|b| b.value));
            }
            StmtKind::Wire(_) | StmtKind::Error => {}
        }
        for e in exprs {
            self.mmio_rw_expr(e, map);
        }
        for b in blocks {
            self.mmio_rw_block(b, map);
        }
    }

    fn mmio_rw_block(&mut self, bi: Idx<Block>, map: &HashMap<String, Rewrite>) {
        let mut stmts = std::mem::take(&mut self.ast.blocks[bi].stmts);
        for bs in &mut stmts {
            self.mmio_rw_block_stmt(bs, map);
        }
        self.ast.blocks[bi].stmts = stmts;
        if let Some(tail) = self.ast.blocks[bi].tail {
            self.mmio_rw_expr(tail, map);
        }
    }

    fn mmio_rw_block_stmt(&mut self, bs: &mut BlockStmt, map: &HashMap<String, Rewrite>) {
        match bs {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                rw_lvalue(lhs, map);
                self.mmio_rw_expr(*rhs, map);
                let idxs: Vec<Idx<Expr>> =
                    lhs.suffixes.iter().flat_map(lvalue_suffix_exprs).collect();
                for e in idxs {
                    self.mmio_rw_expr(e, map);
                }
            }
            BlockStmt::If(ifstmt) => self.mmio_rw_if(ifstmt, map),
            BlockStmt::Match(m) => {
                self.mmio_rw_expr(m.scrutinee, map);
                let (mut exprs, mut blocks) = (Vec::new(), Vec::new());
                collect_arm_idxs(&m.arms, &mut exprs, &mut blocks);
                for e in exprs {
                    self.mmio_rw_expr(e, map);
                }
                for b in blocks {
                    self.mmio_rw_block(b, map);
                }
            }
            BlockStmt::Let(l) => self.mmio_rw_expr(l.value, map),
            BlockStmt::For(f) => {
                self.mmio_rw_expr(f.start, map);
                self.mmio_rw_expr(f.end, map);
                self.mmio_rw_block(f.body, map);
            }
            BlockStmt::Error => {}
        }
    }

    fn mmio_rw_if(&mut self, ifstmt: &IfStmt, map: &HashMap<String, Rewrite>) {
        self.mmio_rw_expr(ifstmt.cond, map);
        self.mmio_rw_block(ifstmt.then_block, map);
        match &ifstmt.else_branch {
            Some(ElseBranch::Block(b)) => self.mmio_rw_block(*b, map),
            Some(ElseBranch::If(inner)) => self.mmio_rw_if(inner, map),
            None => {}
        }
    }

    /// `regs.reg.alan` zincirini hedefe çevirir; eşleşmeyen düğümlerde
    /// çocuklara iner.
    fn mmio_rw_expr(&mut self, e: Idx<Expr>, map: &HashMap<String, Rewrite>) {
        if let Some(target) = self.mmio_field_chain(e).and_then(|k| map.get(&k)).cloned() {
            let span = self.ast.exprs[e].span;
            self.ast.exprs[e].kind = match target {
                Rewrite::Name(name) => ExprKind::Path(path(&name, span)),
                Rewrite::View { word, lo, ty } => self.view_kind(&word, lo, ty, span),
            };
            return;
        }
        let (exprs, blocks) = expr_children(&self.ast.exprs[e].kind);
        for c in exprs {
            self.mmio_rw_expr(c, map);
        }
        for b in blocks {
            self.mmio_rw_block(b, map);
        }
    }

    /// `mmio_<reg>[lo]` / `mmio_<reg>[hi:lo]` / `mmio_<reg>[hi:lo] as uN`.
    fn view_kind(&mut self, word: &str, lo: u32, ty: FieldTy, span: Span) -> ExprKind {
        let base = self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::Path(path(word, span)),
        });
        let lit = |me: &mut Self, v: u32| {
            me.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::IntLit {
                    value: u128::from(v),
                    suffix: None,
                    base: NumBase::Dec,
                },
            })
        };
        match ty {
            FieldTy::Bool => ExprKind::Index {
                base,
                index: lit(self, lo),
            },
            FieldTy::Bits(w) => ExprKind::Range {
                base,
                hi: lit(self, lo + w - 1),
                lo: lit(self, lo),
            },
            FieldTy::UInt(w) => {
                let hi = lit(self, lo + w - 1);
                let lo_e = lit(self, lo);
                let range = self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::Range { base, hi, lo: lo_e },
                });
                let uty = self.ast.types.alloc(TypeRef {
                    span,
                    kind: TypeRefKind::UInt(w as u8),
                });
                ExprKind::Cast {
                    expr: range,
                    ty: uty,
                }
            }
        }
    }

    /// `a.b.c` biçimindeki zinciri `"a.b.c"` anahtarına çevirir; taban
    /// tek segmentli bir yol değilse None.
    fn mmio_field_chain(&self, e: Idx<Expr>) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        let mut cur = e;
        loop {
            match &self.ast.exprs[cur].kind {
                ExprKind::Field { base, field } => {
                    parts.push(field.text.as_str());
                    cur = *base;
                }
                ExprKind::Path(p) if p.segments.len() == 1 && !parts.is_empty() => {
                    parts.push(p.segments[0].text.as_str());
                    parts.reverse();
                    return Some(parts.join("."));
                }
                _ => return None,
            }
        }
    }

    /// Bir `on` bloğu (özyineli) verilen saklayıcıya `<=` ile yazıyor mu?
    fn block_assigns(&self, bi: Idx<Block>, name: &str) -> bool {
        self.ast.blocks[bi].stmts.iter().any(|bs| match bs {
            BlockStmt::NonBlockAssign { lhs, .. } => lhs.base.text == name,
            BlockStmt::If(ifs) => self.if_assigns(ifs, name),
            BlockStmt::Match(m) => m.arms.iter().any(|arm| match &arm.body {
                MatchArmBody::Block(b) => self.block_assigns(*b, name),
                MatchArmBody::Expr(_) => false,
            }),
            BlockStmt::For(f) => self.block_assigns(f.body, name),
            _ => false,
        })
    }

    fn if_assigns(&self, ifs: &IfStmt, name: &str) -> bool {
        self.block_assigns(ifs.then_block, name)
            || match &ifs.else_branch {
                Some(ElseBranch::Block(b)) => self.block_assigns(*b, name),
                Some(ElseBranch::If(inner)) => self.if_assigns(inner, name),
                None => false,
            }
    }

    // ═══ Birleştirme ══════════════════════════════════════════════

    fn splice(&mut self, item: Idx<volt_ast::Item>, gen: Generated) {
        let user_body = match &self.ast.items_arena[item].kind {
            ItemKind::Module(m) => m.body.clone(),
            _ => return,
        };
        // @w1c temizleyicileri: alanı yazan kullanıcı `on` bloğunun BAŞINA
        // girer (aynı döngüde donanım kurması yazılım temizlemesini yener);
        // böyle bir blok yoksa kendi bloğu olarak kalır.
        let mut extra_tail = Vec::new();
        for (field, on_stmt) in gen.w1c {
            let target = user_body
                .iter()
                .copied()
                .find(|&si| match &self.ast.stmts[si].kind {
                    StmtKind::On(on) => self.block_assigns(on.body, &field),
                    _ => false,
                });
            let Some(user_on) = target else {
                extra_tail.push(on_stmt);
                continue;
            };
            let (StmtKind::On(gen_on), StmtKind::On(user)) =
                (&self.ast.stmts[on_stmt].kind, &self.ast.stmts[user_on].kind)
            else {
                continue;
            };
            let (gen_block, user_block) = (gen_on.body, user.body);
            let clears = std::mem::take(&mut self.ast.blocks[gen_block].stmts);
            let mut stmts = std::mem::take(&mut self.ast.blocks[user_block].stmts);
            stmts.splice(0..0, clears);
            self.ast.blocks[user_block].stmts = stmts;
        }
        let ItemKind::Module(m) = &mut self.ast.items_arena[item].kind else {
            return;
        };
        m.ports.extend(gen.ports);
        m.contracts.extend(gen.contracts);
        let mut body = gen.head;
        body.extend(user_body);
        body.extend(gen.tail);
        body.extend(extra_tail);
        m.body = body;
    }
}

/// Nitelik argümanları: (ad, değer) — konumsal argümanda ad None.
fn attr_args(args: &[AttrArg]) -> Vec<(Option<String>, Idx<Expr>)> {
    args.iter()
        .map(|a| match a {
            AttrArg::Named { name, value } => (Some(name.text.clone()), *value),
            AttrArg::Positional(e) => (None, *e),
        })
        .collect()
}

fn path(name: &str, span: Span) -> Path {
    Path {
        span,
        segments: vec![Name {
            text: name.to_string(),
            span,
        }],
    }
}

/// `regs.<reg>.<alan>[...]` sol tarafı → (span, reg, alan).
fn push_regs_lvalue(lv: &LValue, out: &mut Vec<(Span, String, String)>) {
    if lv.base.text != REGS_HANDLE {
        return;
    }
    let (Some(LValueSuffix::Field(r)), Some(LValueSuffix::Field(f))) =
        (lv.suffixes.first(), lv.suffixes.get(1))
    else {
        return;
    };
    out.push((lv.span, r.text.clone(), f.text.clone()));
}

/// Sol taraf: `regs.status.count <= x` → `mmio_status_count <= x`. Bus
/// sözcüğü görünümüne yazma zaten E4006 aldı; kaskad E1001 olmasın diye
/// yine de `mmio_<reg>` sözcüğüne çevrilir.
fn rw_lvalue(lv: &mut LValue, map: &HashMap<String, Rewrite>) {
    let (Some(LValueSuffix::Field(r)), Some(LValueSuffix::Field(f))) =
        (lv.suffixes.first(), lv.suffixes.get(1))
    else {
        return;
    };
    let key = format!("{}.{}.{}", lv.base.text, r.text, f.text);
    match map.get(&key) {
        Some(Rewrite::Name(name)) => {
            lv.base.text = name.clone();
            lv.suffixes.drain(..2);
        }
        Some(Rewrite::View { word, .. }) => {
            lv.base.text = word.clone();
            lv.suffixes.drain(..2);
        }
        None => {}
    }
}

fn addr_lit(addr: u64) -> String {
    format!("{addr:#x}")
}

/// `x == A0 || x == A1 ...`; register yoksa `false`.
fn hit_expr(regs: &[RegInfo], signal: &str) -> String {
    if regs.is_empty() {
        return "false".to_string();
    }
    regs.iter()
        .map(|r| format!("{signal} == {}", addr_lit(r.addr)))
        .collect::<Vec<_>>()
        .join(" || ")
}

fn render_contracts(s: &mut String, regs: &[RegInfo]) {
    s.push_str("    // ADR-0044 S5: an access outside the map answers SLVERR.\n");
    let whit = hit_expr(regs, "prev(aw_addr)");
    let rhit = hit_expr(regs, "prev(ar_addr)");
    let _ = writeln!(
        s,
        "    invariant: prev(aw_valid) && prev(w_valid) && !prev({PREFIX}bvalid) && !({whit}) -> b_valid && b_resp == 2"
    );
    let _ = writeln!(
        s,
        "    invariant: prev(ar_valid) && !prev({PREFIX}rvalid) && !({rhit}) -> r_valid && r_resp == 2"
    );
    for r in regs.iter().filter(|r| r.owner == Owner::Const) {
        s.push_str("    // ADR-0044 S5: a ReadOnly register is not changed by a bus write:\n");
        s.push_str("    // it always reads back its reset value.\n");
        let _ = writeln!(
            s,
            "    invariant: prev(ar_valid) && !prev({PREFIX}rvalid) && prev(ar_addr) == {} -> r_data == 0",
            addr_lit(r.addr)
        );
    }
    if !regs.is_empty() {
        s.push_str("    // ADR-0044 S5: every register is reachable at least once.\n");
    }
    for r in regs {
        let a = addr_lit(r.addr);
        if r.access.readable() {
            let _ = writeln!(s, "    cover: ar_valid && ar_ready && ar_addr == {a}");
        } else {
            let _ = writeln!(s, "    cover: aw_valid && aw_ready && aw_addr == {a}");
        }
    }
    s.push('\n');
}

fn render_decls(s: &mut String, regs: &[RegInfo]) {
    s.push_str("    // AXI4-Lite response state and the register storage.\n");
    let _ = writeln!(s, "    reg {PREFIX}bvalid : bool = false");
    let _ = writeln!(s, "    reg {PREFIX}berr   : bool = false");
    let _ = writeln!(s, "    reg {PREFIX}rvalid : bool = false");
    let _ = writeln!(s, "    reg {PREFIX}rerr   : bool = false");
    let _ = writeln!(s, "    reg {PREFIX}rdata  : u32  = 0");
    for r in regs {
        match r.owner {
            // Tam sözcük: rezerve bitler de saklanır (yazma yolu her biti
            // kullanır), okumada maskelenir.
            Owner::Bus => {
                let _ = writeln!(s, "    reg {} : u32 = 0", r.word());
            }
            Owner::Hardware | Owner::Const => {
                let kw = if r.owner == Owner::Const {
                    "let"
                } else {
                    "reg"
                };
                for f in r.named() {
                    let n = flat_name(&r.name, f.name.as_deref().unwrap_or_default());
                    let _ = writeln!(s, "    {kw} {n} : {} = {}", f.ty.volt(), f.ty.zero());
                }
            }
        }
    }
    s.push_str("\n    // Address decode.\n");
    let _ = writeln!(
        s,
        "    let {PREFIX}wr_fire : bool = aw_valid && w_valid && !{PREFIX}bvalid"
    );
    let _ = writeln!(
        s,
        "    let {PREFIX}rd_fire : bool = ar_valid && !{PREFIX}rvalid"
    );
    let _ = writeln!(
        s,
        "    let {PREFIX}whit : bool = {}",
        hit_expr(regs, "aw_addr")
    );
    let _ = writeln!(
        s,
        "    let {PREFIX}rhit : bool = {}",
        hit_expr(regs, "ar_addr")
    );
    let bus_written = regs.iter().any(|r| r.owner == Owner::Bus);
    if bus_written || regs.iter().any(RegInfo::has_w1c) {
        let _ = writeln!(
            s,
            "    let {PREFIX}we : bool = {PREFIX}wr_fire && aw_prot == 0 && {PREFIX}whit"
        );
    }
    if bus_written {
        let _ = writeln!(
            s,
            "    let {PREFIX}wmask : u32 = (if w_strb[0] {{ 0x000000FF }} else {{ 0 }}) | (if w_strb[1] {{ 0x0000FF00 }} else {{ 0 }})\n                        | (if w_strb[2] {{ 0x00FF0000 }} else {{ 0 }}) | (if w_strb[3] {{ 0xFF000000 }} else {{ 0 }})"
        );
    }
    s.push_str("\n    // Read view of every readable register (@reserved bits read 0).\n");
    for r in regs.iter().filter(|r| r.access.readable()) {
        let _ = writeln!(s, "    let {} : u32 = {}", r.rd(), compose_word(r));
    }
    s.push('\n');
}

/// Okuma sözcüğü: bus sözcüğü maskelenir, donanım/sabit alanlar bileşir.
fn compose_word(r: &RegInfo) -> String {
    if r.owner == Owner::Bus {
        let mask = r.read_mask();
        return if mask == u64::from(u32::MAX) {
            r.word()
        } else {
            format!("{} & {mask:#010x}", r.word())
        };
    }
    let terms: Vec<String> = r
        .named()
        .map(|f| {
            let n = flat_name(&r.name, f.name.as_deref().unwrap_or_default());
            match f.ty {
                FieldTy::Bool => format!("(if {n} {{ {:#x} }} else {{ 0 }})", 1u64 << f.lo),
                FieldTy::Bits(w) => shifted(&format!("(({n} as u{w}) as u32)"), f.lo),
                FieldTy::UInt(_) => shifted(&format!("({n} as u32)"), f.lo),
            }
        })
        .collect();
    terms.join(" | ")
}

fn shifted(term: &str, lo: u32) -> String {
    if lo == 0 {
        term.to_string()
    } else {
        format!("({term} << {lo})")
    }
}

fn render_logic(s: &mut String, regs: &[RegInfo], clk: &str) {
    let _ = writeln!(s, "    on {clk} {{");
    let _ = writeln!(s, "        if {PREFIX}wr_fire {{");
    let _ = writeln!(s, "            {PREFIX}bvalid <= true");
    let _ = writeln!(
        s,
        "            {PREFIX}berr   <= aw_prot != 0 || !{PREFIX}whit"
    );
    s.push_str("        }\n");
    let _ = writeln!(s, "        if {PREFIX}bvalid && b_ready {{");
    let _ = writeln!(s, "            {PREFIX}bvalid <= false");
    s.push_str("        }\n");
    let _ = writeln!(s, "        if {PREFIX}rd_fire {{");
    let _ = writeln!(s, "            {PREFIX}rvalid <= true");
    let _ = writeln!(
        s,
        "            {PREFIX}rerr   <= ar_prot != 0 || !{PREFIX}rhit"
    );
    let readable: Vec<&RegInfo> = regs.iter().filter(|r| r.access.readable()).collect();
    if readable.is_empty() {
        let _ = writeln!(s, "            {PREFIX}rdata <= 0");
    } else {
        s.push_str("            match ar_addr {\n");
        for r in &readable {
            let _ = writeln!(
                s,
                "                {} => {{ {PREFIX}rdata <= {} }}",
                addr_lit(r.addr),
                r.rd()
            );
        }
        let _ = writeln!(s, "                _ => {{ {PREFIX}rdata <= 0 }}");
        s.push_str("            }\n");
    }
    s.push_str("        }\n");
    let _ = writeln!(s, "        if {PREFIX}rvalid && r_ready {{");
    let _ = writeln!(s, "            {PREFIX}rvalid <= false");
    s.push_str("        }\n");
    // @self_clearing: bir döngülük darbe — aşağıdaki yazma bunu yener.
    for r in regs {
        for f in r.named().filter(|f| f.self_clearing) {
            let _ = writeln!(s, "        {}[{}] <= false", r.word(), f.lo);
        }
    }
    let written: Vec<&RegInfo> = regs.iter().filter(|r| r.owner == Owner::Bus).collect();
    if !written.is_empty() {
        let _ = writeln!(s, "        if {PREFIX}we {{");
        s.push_str("            match aw_addr {\n");
        for r in &written {
            let w = r.word();
            let _ = writeln!(
                s,
                "                {} => {{ {w} <= ({w} & ~{PREFIX}wmask) | (w_data & {PREFIX}wmask) }}",
                addr_lit(r.addr)
            );
        }
        s.push_str("                _ => { }\n");
        s.push_str("            }\n");
        s.push_str("        }\n");
    }
    s.push_str("    }\n");
    // @w1c: her alan için ayrı blok; splice kullanıcı bloğuna taşır.
    for r in regs {
        for f in r.named().filter(|f| f.w1c) {
            let n = flat_name(&r.name, f.name.as_deref().unwrap_or_default());
            let _ = writeln!(s, "    on {clk} {{");
            let _ = writeln!(
                s,
                "        if {PREFIX}we && aw_addr == {} && w_strb[{}] && w_data[{}] {{",
                addr_lit(r.addr),
                f.lo / 8,
                f.lo
            );
            let _ = writeln!(s, "            {n} <= false");
            s.push_str("        }\n    }\n");
        }
    }
    s.push_str("\n    // AXI4-Lite handshake outputs.\n");
    let _ = writeln!(s, "    aw_ready = {PREFIX}wr_fire");
    let _ = writeln!(s, "    w_ready  = {PREFIX}wr_fire");
    let _ = writeln!(s, "    b_valid  = {PREFIX}bvalid");
    let _ = writeln!(s, "    b_resp   = if {PREFIX}berr {{ 2 }} else {{ 0 }}");
    let _ = writeln!(s, "    ar_ready = {PREFIX}rd_fire");
    let _ = writeln!(s, "    r_valid  = {PREFIX}rvalid");
    let _ = writeln!(s, "    r_resp   = if {PREFIX}rerr {{ 2 }} else {{ 0 }}");
    let _ = writeln!(s, "    r_data   = {PREFIX}rdata");
}
