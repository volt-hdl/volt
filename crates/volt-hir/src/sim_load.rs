//! `load(dut.mem, veri)` denetimi (ADR-0058, E8509/E8510).
//!
//! Hedef, DUT modülünde ya da alt örneklerinden birinde dizi tipli bir
//! `reg` olmalıdır. Yol `let ad = Modul { ... }` örnekleri izlenerek
//! çözülür; bu crate'in test denetimi isim çözümlemeden bağımsız
//! çalıştığından boyutlar yalnız düz literal ya da düz literal `const`
//! ise derleme zamanında karşılaştırılır — diğer durumda aynı denetim
//! testbench'te çalışma zamanında yapılır.

use std::collections::HashMap;

use volt_ast::{
    ModuleDecl, Name, RegDecl, SourceFile, StmtKind, TestExpr, TestExprKind, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use crate::sim::undefined_dut;
use crate::sim_expr::{type_mismatch, Scope};
use crate::sim_port::{literal_or_const, scalar_width, ScalarWidth};

/// Testbench'in doğrudan yazabildiği en geniş eleman (C++ `QData`).
const MAX_LOAD_ELEM_BITS: u32 = 64;

type ModuleMap<'a> = HashMap<String, (&'a SourceFile, &'a ModuleDecl)>;

/// Çözülmüş `load` hedefi — sürücü bundan Verilator adını kurar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadTarget {
    /// Yazmacın sahibi olan modülün adı (`.vlt` dosyası için).
    pub owner_module: String,
    /// DUT'tan yazmaca örnek adları + yazmaç adı: `["cpu", "imem"]`.
    pub path: Vec<String>,
    /// Eleman genişliği biliniyorsa: testbench değeri buna göre denetler
    /// (C++ tipi 12 bitlik elemanı 16 bit taşır, taşma görünmezdi).
    pub elem_bits: Option<u32>,
}

/// Çözümleme sonucu: bulunamadıysa tanı, dış modüle çıktıysa `External`.
enum Resolved<'a> {
    Reg(&'a SourceFile, &'a ModuleDecl, &'a RegDecl),
    /// Yol, kaynakları görünmeyen bir modüle girdi: denetlenemez.
    External,
    Failed,
}

pub(crate) fn check_load(
    scope: &Scope<'_>,
    modules: &ModuleMap<'_>,
    target: &TestExpr,
    source: &TestExpr,
    diags: &mut Vec<Diagnostic>,
) {
    let source_info = match &source.kind {
        TestExprKind::Var(name) => scope.expect_array(name, diags),
        _ => {
            diags.push(type_mismatch(
                source.span,
                lstr!(en: "the load() source must be the name of an array";
                      tr: "load() kaynağı bir dizi adı olmalıdır"),
            ));
            None
        }
    };

    let Some((dut, path)) = target_path(target) else {
        diags.push(not_a_memory(
            target.span,
            lstr!(en: "the load() target must be a memory of the instance, written as dut.memory";
                  tr: "load() hedefi örneğin bir belleği olmalı ve dut.bellek biçiminde yazılmalıdır"),
        ));
        return;
    };
    let Some(entry) = scope.duts.get(dut.text.as_str()) else {
        diags.push(undefined_dut(dut));
        return;
    };
    let Some((src, module)) = entry else {
        return; // dış modül varsayımı: hedef denetlenemez
    };
    let Resolved::Reg(reg_src, _, reg) = resolve(modules, src, module, &path, diags) else {
        return;
    };

    let Some(ty) = reg.ty else {
        return; // tipi çıkarımla gelen yazmaç: çalışma zamanı denetimi
    };
    let TypeRefKind::Array { elem, len } = &reg_src.types[ty].kind else {
        // Konum test dosyasındaki hedeftir: yazmaç kardeş dosyada
        // olabilir ve onun span'ı bu dosyanın haritasında anlamsızdır.
        diags.push(not_a_memory(
            target.span,
            lstr!(en: "register '{}' is not an array", reg.name.text;
                  tr: "'{}' yazmacı dizi değil", reg.name.text),
        ));
        return;
    };
    let elem_bits = match scalar_width(reg_src, *elem) {
        ScalarWidth::Known(w) if w.bits <= MAX_LOAD_ELEM_BITS => Some(w.bits),
        ScalarWidth::Known(_) => {
            diags.push(not_a_memory(
                target.span,
                lstr!(en: "elements of '{}' are wider than 64 bits", reg.name.text;
                      tr: "'{}' elemanları 64 bitten geniş", reg.name.text),
            ));
            return;
        }
        ScalarWidth::NotScalar => {
            diags.push(not_a_memory(
                target.span,
                lstr!(en: "elements of '{}' are not plain numbers", reg.name.text;
                      tr: "'{}' elemanları düz sayı değil", reg.name.text),
            ));
            return;
        }
        ScalarWidth::Unknown => None,
    };
    let Some(info) = source_info else {
        return;
    };
    if let Some(target_len) = literal_or_const(reg_src, *len) {
        if info.len as u128 > target_len {
            diags.push(does_not_fit(
                source.span,
                lstr!(en: "{} element(s) do not fit into '{}' ({target_len} element(s))", info.len, reg.name.text;
                      tr: "{} eleman '{}' dizisine sığmıyor ({target_len} eleman)", info.len, reg.name.text),
            ));
            return;
        }
    }
    if let Some(bits) = elem_bits.filter(|w| *w < MAX_LOAD_ELEM_BITS) {
        if info.max >> bits != 0 {
            diags.push(does_not_fit(
                source.span,
                lstr!(en: "value {:#x} does not fit into the {bits}-bit elements of '{}'", info.max, reg.name.text;
                      tr: "{:#x} değeri '{}' dizisinin {bits} bitlik elemanlarına sığmıyor", info.max, reg.name.text),
            ));
        }
    }
}

/// `dut.mem` ya da `dut.a.b.mem` → (dut, [a, b, mem]).
fn target_path(target: &TestExpr) -> Option<(&Name, Vec<&Name>)> {
    match &target.kind {
        TestExprKind::PortRead { dut, port } => Some((dut, vec![port])),
        TestExprKind::MemberPath { dut, path } => Some((dut, path.iter().collect())),
        _ => None,
    }
}

/// `load` hedefini çözer; sürücü de aynı yolu Verilator adını kurmak
/// için kullanır (tanılar atılır).
pub fn resolve_load_target(
    sources: &[&SourceFile],
    dut_module: &str,
    target: &TestExpr,
) -> Option<LoadTarget> {
    let modules = crate::sim::collect_modules(sources);
    let (src, module) = *modules.get(dut_module)?;
    let (_, path) = target_path(target)?;
    let mut scratch = Vec::new();
    let Resolved::Reg(reg_src, owner, reg) = resolve(&modules, src, module, &path, &mut scratch)
    else {
        return None;
    };
    let elem_bits = reg.ty.and_then(|ty| match &reg_src.types[ty].kind {
        TypeRefKind::Array { elem, .. } => match scalar_width(reg_src, *elem) {
            ScalarWidth::Known(w) => Some(w.bits),
            ScalarWidth::Unknown | ScalarWidth::NotScalar => None,
        },
        _ => None,
    });
    Some(LoadTarget {
        owner_module: owner.name.text.clone(),
        path: path.iter().map(|n| n.text.clone()).collect(),
        elem_bits,
    })
}

fn resolve<'a>(
    modules: &ModuleMap<'a>,
    src: &'a SourceFile,
    module: &'a ModuleDecl,
    path: &[&Name],
    diags: &mut Vec<Diagnostic>,
) -> Resolved<'a> {
    let (mut src, mut module) = (src, module);
    let Some((last, instances)) = path.split_last() else {
        return Resolved::Failed;
    };
    for seg in instances {
        let Some(target_module) = instance_module(src, module, &seg.text) else {
            diags.push(not_a_memory(
                seg.span,
                lstr!(en: "module '{}' has no instance '{}'", module.name.text, seg.text;
                      tr: "'{}' modülünde '{}' diye örnek yok", module.name.text, seg.text),
            ));
            return Resolved::Failed;
        };
        let Some(found) = modules.get(target_module) else {
            return Resolved::External;
        };
        (src, module) = *found;
    }
    match find_reg(src, module, &last.text) {
        Some(reg) => Resolved::Reg(src, module, reg),
        None => {
            diags.push(not_a_memory(
                last.span,
                lstr!(en: "module '{}' has no register '{}'", module.name.text, last.text;
                      tr: "'{}' modülünde '{}' diye yazmaç yok", module.name.text, last.text),
            ));
            Resolved::Failed
        }
    }
}

/// `let ad = Modul { ... }` örneğinin modül adı (yolun son parçası).
fn instance_module<'a>(src: &'a SourceFile, module: &ModuleDecl, name: &str) -> Option<&'a str> {
    module
        .body
        .iter()
        .find_map(|idx| match &src.stmts[*idx].kind {
            StmtKind::Instance(inst) if inst.name.text == name => inst
                .module_path
                .segments
                .last()
                .map(|seg| seg.text.as_str()),
            _ => None,
        })
}

fn find_reg<'a>(src: &'a SourceFile, module: &ModuleDecl, name: &str) -> Option<&'a RegDecl> {
    module
        .body
        .iter()
        .find_map(|idx| match &src.stmts[*idx].kind {
            StmtKind::Reg(reg) if reg.name.text == name => Some(reg),
            _ => None,
        })
}

fn not_a_memory(span: volt_span::Span, message: String) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8509,
        message,
        LabeledSpan::primary(
            span,
            lstr!(en: "not a memory array"; tr: "bellek dizisi değil"),
        ),
        lstr!(en: "load() writes into an array register: reg mem : [u32; N] = [0; N]";
              tr: "load() dizi tipli bir yazmaca yazar: reg mem : [u32; N] = [0; N]"),
    )
}

fn does_not_fit(span: volt_span::Span, message: String) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8510,
        message,
        LabeledSpan::primary(
            span,
            lstr!(en: "does not fit the target"; tr: "hedefe sığmıyor"),
        ),
        lstr!(en: "enlarge the memory or shorten the data; load() never truncates";
              tr: "belleği büyütün ya da veriyi kısaltın; load() asla kırpmaz"),
    )
}
