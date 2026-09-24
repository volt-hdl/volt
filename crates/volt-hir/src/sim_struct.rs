//! Test dilinde struct portları (ADR-0077 Karar 6).
//!
//! Struct tipli DUT portu SV'de yaprak başına bir porttur (`p_a`, …).
//! Test betiği alanları kaynak adıyla yazar: `dut.q.a` okuma, `dut.p.a =
//! 3` yazma, `dut.p = P { a: 1, b: true }` bütün yazma,
//! `assert_eq(dut.q, P { … })` bütün karşılaştırma. Bu modül:
//!
//! - yaprak yollarını noktalı port adına indirger (`q.a`): port araması
//!   ([`find_port`]) noktalı adı struct yaprağı olarak çözer; sürücü SV
//!   adını (`q_a`) noktayı `_` yaparak kurar;
//! - bütün yazmayı yaprak yazmalarına açar;
//! - bütün karşılaştırmayı denetler (aynı struct, eksiksiz literal, 64
//!   bit sınırı — betik değerleri 64 bittir).
//!
//! Tanı kodları mevcut aileden (yeni kod yok): bilinmeyen alan E8502,
//! yanlış türde değer E8511.

use std::collections::HashMap;

use volt_ast::struct_layout::{self, Leaf, StructLayout};
use volt_ast::{
    Idx, ModuleDecl, Name, PortDir, SourceFile, TestExpr, TestExprKind, TestStmt, TypeRef,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use crate::sim_expr::type_mismatch;

/// Betik değerinin genişliği: bütün struct karşılaştırması bunu aşamaz.
pub const MAX_WHOLE_STRUCT_BITS: u32 = 64;

/// Bir DUT portunun (ya da struct portu yaprağının) test görünümü.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PortView {
    pub direction: PortDir,
    pub ty: Idx<TypeRef>,
}

/// Struct tipli portun düzeni (geçerli düz struct, generic değil).
pub fn struct_port_layout(
    src: &SourceFile,
    module: &ModuleDecl,
    port: &str,
) -> Option<StructLayout> {
    let p = module.ports.iter().find(|p| p.name.text == port)?;
    let decl = struct_layout::struct_of_type(src, p.ty)?;
    struct_layout::layout(src, decl, &mut |e| crate::sim_port::const_value(src, e)).ok()
}

/// Port araması: düz ad ya da struct yaprağının noktalı adı (`q.a`).
pub(crate) fn find_port(src: &SourceFile, module: &ModuleDecl, name: &str) -> Option<PortView> {
    let Some((base, rest)) = name.split_once('.') else {
        let p = module.ports.iter().find(|p| p.name.text == name)?;
        return Some(PortView {
            direction: p.direction,
            ty: p.ty,
        });
    };
    let p = module.ports.iter().find(|p| p.name.text == base)?;
    let layout = struct_port_layout(src, module, base)?;
    let path: Vec<String> = rest.split('.').map(str::to_string).collect();
    let leaf = layout.leaves.iter().find(|l| l.path == path)?;
    Some(PortView {
        direction: p.direction,
        ty: leaf.ty,
    })
}

/// Noktalı test port adının SV adı: `q.a` → `q_a` (ADR-0077 Karar 5).
pub fn sv_port_name(name: &str) -> String {
    name.replace('.', "_")
}

type Modules<'a> = HashMap<String, (&'a SourceFile, &'a ModuleDecl)>;

/// Test deyimlerindeki struct port yollarını yaprak biçimine açar.
/// `diags` verilirse (denetim) hatalı yollar tanı alır; indirgemede
/// (`None`) denetimden geçmiş betik varsayılır.
pub fn expand_struct_tests(
    sources: &[&SourceFile],
    stmts: &[TestStmt],
    diags: Option<&mut Vec<Diagnostic>>,
) -> Vec<TestStmt> {
    let modules = crate::sim::collect_modules(sources);
    let mut sink = Vec::new();
    let mut ex = Expander {
        modules: &modules,
        duts: HashMap::new(),
        diags: &mut sink,
    };
    let out = ex.block(stmts);
    if let Some(d) = diags {
        d.extend(sink);
    }
    out
}

struct Expander<'a, 'm> {
    modules: &'m Modules<'a>,
    /// dut adı → modül adı.
    duts: HashMap<String, String>,
    diags: &'m mut Vec<Diagnostic>,
}

impl<'a> Expander<'a, '_> {
    fn dut(&self, dut: &Name) -> Option<(&'a SourceFile, &'a ModuleDecl)> {
        let module = self.duts.get(&dut.text)?;
        self.modules.get(module).copied()
    }

    fn block(&mut self, stmts: &[TestStmt]) -> Vec<TestStmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for s in stmts {
            self.stmt(s, &mut out);
        }
        out
    }

    fn stmt(&mut self, s: &TestStmt, out: &mut Vec<TestStmt>) {
        match s {
            TestStmt::LetDut { name, module, .. } => {
                self.duts
                    .entry(name.text.clone())
                    .or_insert_with(|| module.text.clone());
                out.push(s.clone());
            }
            TestStmt::SetPort {
                span,
                dut,
                port,
                fields,
                value,
            } => self.set_port(*span, dut, port, fields, value, out),
            TestStmt::Call { span, func, args } => out.push(TestStmt::Call {
                span: *span,
                func: func.clone(),
                args: args.iter().map(|a| self.expr(a)).collect(),
            }),
            TestStmt::LetVar { span, name, value } => out.push(TestStmt::LetVar {
                span: *span,
                name: name.clone(),
                value: self.expr(value),
            }),
            TestStmt::For {
                span,
                var,
                start,
                end,
                body,
            } => {
                let body = self.block(body);
                out.push(TestStmt::For {
                    span: *span,
                    var: var.clone(),
                    start: self.expr(start),
                    end: self.expr(end),
                    body,
                });
            }
        }
    }

    /// `dut.p.a = v` → `dut.p.a` (noktalı yaprak); `dut.p = P { .. }` →
    /// yaprak başına yazma.
    fn set_port(
        &mut self,
        span: Span,
        dut: &Name,
        port: &Name,
        fields: &[Name],
        value: &TestExpr,
        out: &mut Vec<TestStmt>,
    ) {
        let plain = |port: Name, value: TestExpr| TestStmt::SetPort {
            span,
            dut: dut.clone(),
            port,
            fields: Vec::new(),
            value,
        };
        let layout = self
            .dut(dut)
            .and_then(|(src, m)| struct_port_layout(src, m, &port.text));
        let Some(layout) = layout else {
            if let Some(f) = fields.first() {
                // Struct olmayan portta alan yolu (ya da bilinmeyen port —
                // E8502'yi port denetimi verir).
                if self
                    .dut(dut)
                    .is_some_and(|(_, m)| m.ports.iter().any(|p| p.name.text == port.text))
                {
                    self.diags.push(type_mismatch(
                        f.span,
                        lstr!(en: "port '{}' is not a struct; it has no field '{}'", port.text, f.text;
                              tr: "'{}' portu struct değil; '{}' alanı yok", port.text, f.text),
                    ));
                }
            }
            out.push(plain(port.clone(), self.expr(value)));
            return;
        };
        if !fields.is_empty() {
            let path: Vec<Name> = fields.to_vec();
            match self.leaf(&layout, port, &path) {
                Some(leaf_name) => out.push(plain(leaf_name, self.expr(value))),
                None => out.push(plain(port.clone(), self.expr(value))),
            }
            return;
        }
        // Bütün yazma: yalnız aynı struct'ın literali.
        let TestExprKind::StructLit { .. } = &value.kind else {
            self.diags.push(type_mismatch(
                value.span,
                lstr!(en: "port '{}' is struct '{}'; write a struct literal ({} {{ ... }}) or a field (dut.{}.<field> = ...)", port.text, layout.name, layout.name, port.text;
                      tr: "'{}' portu '{}' struct'ı; bir struct literali ({} {{ ... }}) ya da bir alan (dut.{}.<alan> = ...) yazın", port.text, layout.name, layout.name, port.text),
            ));
            return;
        };
        let Some(values) = self.literal_leaves(&layout, value) else {
            return;
        };
        for (leaf, v) in layout.leaves.iter().zip(values) {
            let name = Name {
                text: format!("{}.{}", port.text, leaf.dotted()),
                span: port.span,
            };
            out.push(plain(name, v));
        }
    }

    /// Alan yolunu yaprağa çözer; noktalı port adı döner.
    fn leaf(&mut self, layout: &StructLayout, port: &Name, path: &[Name]) -> Option<Name> {
        let texts: Vec<String> = path.iter().map(|n| n.text.clone()).collect();
        if layout.leaves.iter().any(|l| l.path == texts) {
            let end = path.last().map_or(port.span, |n| n.span);
            return Some(Name {
                text: format!("{}.{}", port.text, texts.join(".")),
                span: Span {
                    end: end.end,
                    ..port.span
                },
            });
        }
        // Hangi parça yanlış: bilinmeyen alan (E8502) ya da yaprak değil.
        for i in 1..=texts.len() {
            if layout.field_bits(&texts[..i]).is_none() {
                let bad = &path[i - 1];
                let known: Vec<String> = layout
                    .leaves
                    .iter()
                    .filter(|l| l.path.len() >= i && l.path[..i - 1] == texts[..i - 1])
                    .map(|l| l.path[i - 1].clone())
                    .fold(Vec::new(), |mut acc, n| {
                        if !acc.contains(&n) {
                            acc.push(n);
                        }
                        acc
                    });
                self.diags.push(Diagnostic::error(
                    ErrorCode::E8502,
                    lstr!(en: "struct '{}' of port '{}' has no field '{}'", layout.name, port.text, bad.text;
                          tr: "'{}' portunun '{}' struct'ında '{}' alanı yok", port.text, layout.name, bad.text),
                    LabeledSpan::primary(bad.span, lstr!(en: "unknown field"; tr: "bilinmeyen alan")),
                    lstr!(en: "available fields: {}", known.join(", "); tr: "mevcut alanlar: {}", known.join(", ")),
                ));
                return None;
            }
        }
        let span = path.last().map_or(port.span, |n| n.span);
        self.diags.push(type_mismatch(
            span,
            lstr!(en: "'{}.{}' is a nested struct, not a number; read one of its fields", port.text, texts.join(".");
                  tr: "'{}.{}' iç içe bir struct, sayı değil; alanlarından birini okuyun", port.text, texts.join(".")),
        ));
        None
    }

    /// Literalin yaprak değerleri (düzen sırasıyla); hatalı literalde `None`.
    fn literal_leaves(&mut self, layout: &StructLayout, lit: &TestExpr) -> Option<Vec<TestExpr>> {
        let mut out = Vec::with_capacity(layout.leaves.len());
        let mut ok = true;
        for leaf in &layout.leaves {
            match self.literal_value(lit, &leaf.path) {
                Some(v) => out.push(v),
                None => ok = false,
            }
        }
        ok.then_some(out)
    }

    /// Literalde `path` yaprağının değeri (iç içe literal izlenir).
    fn literal_value(&mut self, lit: &TestExpr, path: &[String]) -> Option<TestExpr> {
        let TestExprKind::StructLit { name, fields } = &lit.kind else {
            return Some(self.expr(lit));
        };
        if path.is_empty() {
            self.diags.push(type_mismatch(
                lit.span,
                lstr!(en: "a struct literal where a number is expected"; tr: "sayı beklenen yerde struct literali"),
            ));
            return None;
        }
        let Some((_, value)) = fields.iter().find(|(f, _)| f.text == path[0]) else {
            // Eksik alan yalnız bir kez raporlanır (aynı tanı katlanır).
            self.missing_field(name, &path[0], lit.span);
            return None;
        };
        self.literal_value(value, &path[1..])
    }

    fn missing_field(&mut self, name: &Name, field: &str, span: Span) {
        let diag = type_mismatch(
            span,
            lstr!(en: "the '{}' literal has no value for field '{field}' — every field must be given", name.text;
                  tr: "'{}' literalinde '{field}' alanının değeri yok — her alan verilmeli", name.text),
        );
        if !self.diags.contains(&diag) {
            self.diags.push(diag);
        }
    }

    /// İfadedeki `dut.q.a` okumaları noktalı porta iner.
    fn expr(&mut self, e: &TestExpr) -> TestExpr {
        let kind = match &e.kind {
            TestExprKind::MemberPath { dut, path } => {
                let layout = path.first().and_then(|p| {
                    self.dut(dut)
                        .and_then(|(src, m)| struct_port_layout(src, m, &p.text))
                });
                match layout {
                    Some(layout) => match self.leaf(&layout, &path[0], &path[1..]) {
                        Some(port) => TestExprKind::PortRead {
                            dut: dut.clone(),
                            port,
                        },
                        None => TestExprKind::Int(0),
                    },
                    None => e.kind.clone(),
                }
            }
            TestExprKind::Unary { op, operand } => TestExprKind::Unary {
                op: *op,
                operand: Box::new(self.expr(operand)),
            },
            TestExprKind::Binary { op, lhs, rhs } => TestExprKind::Binary {
                op: *op,
                lhs: Box::new(self.expr(lhs)),
                rhs: Box::new(self.expr(rhs)),
            },
            TestExprKind::Index { base, index } => TestExprKind::Index {
                base: base.clone(),
                index: Box::new(self.expr(index)),
            },
            TestExprKind::Call { func, args } => TestExprKind::Call {
                func: func.clone(),
                args: args.iter().map(|a| self.expr(a)).collect(),
            },
            TestExprKind::Array(items) => {
                TestExprKind::Array(items.iter().map(|a| self.expr(a)).collect())
            }
            TestExprKind::StructLit { name, fields } => TestExprKind::StructLit {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|(n, v)| (n.clone(), self.expr(v)))
                    .collect(),
            },
            other => other.clone(),
        };
        TestExpr { span: e.span, kind }
    }
}

/// Bütün-struct karşılaştırmasının tarafları: (struct portu okuması,
/// karşı taraf). `assert_eq(dut.q, P { … })` ya da ters sıra.
pub fn struct_compare<'e>(
    sources: &[&SourceFile],
    duts: &HashMap<String, String>,
    args: &'e [TestExpr],
) -> Option<(StructLayout, &'e Name, &'e TestExpr)> {
    let [a, b] = args else {
        return None;
    };
    let modules = crate::sim::collect_modules(sources);
    for (read, other) in [(a, b), (b, a)] {
        let TestExprKind::PortRead { dut, port } = &read.kind else {
            continue;
        };
        let (src, m) = modules.get(duts.get(&dut.text)?).copied()?;
        if let Some(layout) = struct_port_layout(src, m, &port.text) {
            return Some((layout, port, other));
        }
    }
    None
}

/// `assert_eq`/`assert_ne` argümanlarından biri bütün struct mı?
pub(crate) fn is_whole_struct_arg(duts: &crate::sim::DutMap<'_>, arg: &TestExpr) -> bool {
    match &arg.kind {
        TestExprKind::StructLit { .. } => true,
        TestExprKind::PortRead { dut, port } => duts
            .get(dut.text.as_str())
            .copied()
            .flatten()
            .and_then(|(src, m)| struct_port_layout(src, m, &port.text))
            .is_some(),
        _ => false,
    }
}

/// Bütün-struct karşılaştırması denetimi: bir taraf struct portu, diğeri
/// aynı struct'ın eksiksiz literali; genişlik ≤ 64 bit.
pub(crate) fn check_struct_compare(
    duts: &crate::sim::DutMap<'_>,
    args: &[TestExpr],
    check_scalar: &mut dyn FnMut(&TestExpr, &mut Vec<Diagnostic>),
    diags: &mut Vec<Diagnostic>,
) {
    let [a, b] = args else {
        return;
    };
    // Dış (kardeş dosyadaki) DUT: portları burada bilinmez.
    let external = |e: &TestExpr| match &e.kind {
        TestExprKind::PortRead { dut, .. } => {
            matches!(duts.get(dut.text.as_str()), Some(None))
        }
        _ => false,
    };
    if external(a) || external(b) {
        return;
    }
    let layout_of = |e: &TestExpr| match &e.kind {
        TestExprKind::PortRead { dut, port } => duts
            .get(dut.text.as_str())
            .copied()
            .flatten()
            .and_then(|(src, m)| struct_port_layout(src, m, &port.text).map(|l| (l, port.clone()))),
        _ => None,
    };
    let (layout, port, other) = match (layout_of(a), layout_of(b)) {
        (Some((l, p)), _) => (l, p, b),
        (None, Some((l, p))) => (l, p, a),
        (None, None) => {
            // İki literal ya da literal + sayı: karşılaştırılacak port yok.
            let lit = if matches!(a.kind, TestExprKind::StructLit { .. }) {
                a
            } else {
                b
            };
            diags.push(type_mismatch(
                lit.span,
                lstr!(en: "a struct literal can only be compared with a struct port of the DUT (assert_eq(dut.q, P {{ ... }}))";
                      tr: "struct literali yalnız DUT'un struct portuyla karşılaştırılabilir (assert_eq(dut.q, P {{ ... }}))"),
            ));
            return;
        }
    };
    if layout.width > MAX_WHOLE_STRUCT_BITS {
        diags.push(type_mismatch(
            port.span,
            lstr!(en: "struct '{}' is {} bits; a whole-struct test value holds at most {} bits — compare its fields (dut.{}.<field>)", layout.name, layout.width, MAX_WHOLE_STRUCT_BITS, port.text;
                  tr: "'{}' struct'ı {} bit; bütün-struct test değeri en çok {} bit taşır — alanlarını karşılaştırın (dut.{}.<alan>)", layout.name, layout.width, MAX_WHOLE_STRUCT_BITS, port.text),
        ));
        return;
    }
    match &other.kind {
        TestExprKind::StructLit { name, .. } if name.text != layout.name => {
            diags.push(type_mismatch(
                name.span,
                lstr!(en: "port '{}' is struct '{}', compared with a '{}' literal", port.text, layout.name, name.text;
                      tr: "'{}' portu '{}' struct'ı, bir '{}' literaliyle karşılaştırılıyor", port.text, layout.name, name.text),
            ));
        }
        TestExprKind::StructLit { .. } => check_literal(&layout, other, check_scalar, diags),
        TestExprKind::PortRead { .. } => {
            // İki struct portu: aynı struct olmalı.
            if let Some((l2, p2)) = layout_of(other) {
                if l2.name != layout.name {
                    diags.push(type_mismatch(
                        p2.span,
                        lstr!(en: "port '{}' is struct '{}', port '{}' is struct '{}'", port.text, layout.name, p2.text, l2.name;
                              tr: "'{}' portu '{}' struct'ı, '{}' portu '{}' struct'ı", port.text, layout.name, p2.text, l2.name),
                    ));
                }
            } else {
                diags.push(number_vs_struct(&layout, &port, other));
            }
        }
        _ => diags.push(number_vs_struct(&layout, &port, other)),
    }
}

fn number_vs_struct(layout: &StructLayout, port: &Name, other: &TestExpr) -> Diagnostic {
    type_mismatch(
        other.span,
        lstr!(en: "port '{}' is struct '{}'; compare it with a literal ({} {{ ... }}) or compare a field (dut.{}.<field>)", port.text, layout.name, layout.name, port.text;
              tr: "'{}' portu '{}' struct'ı; bir literalle ({} {{ ... }}) ya da bir alanını (dut.{}.<alan>) karşılaştırın", port.text, layout.name, layout.name, port.text),
    )
}

/// Literal: her alan bir kez, bilinmeyen alan yok, değerler sayı.
fn check_literal(
    layout: &StructLayout,
    lit: &TestExpr,
    check_scalar: &mut dyn FnMut(&TestExpr, &mut Vec<Diagnostic>),
    diags: &mut Vec<Diagnostic>,
) {
    check_literal_at(layout, lit, &[], check_scalar, diags);
}

fn check_literal_at(
    layout: &StructLayout,
    lit: &TestExpr,
    prefix: &[String],
    check_scalar: &mut dyn FnMut(&TestExpr, &mut Vec<Diagnostic>),
    diags: &mut Vec<Diagnostic>,
) {
    let TestExprKind::StructLit { name, fields } = &lit.kind else {
        return;
    };
    // Bu seviyedeki alan adları (düzen sırasıyla, tekil).
    let mut level: Vec<String> = Vec::new();
    for l in layout.leaves_under(prefix) {
        let n = l.path[prefix.len()].clone();
        if !level.contains(&n) {
            level.push(n);
        }
    }
    for (f, v) in fields {
        if !level.contains(&f.text) {
            diags.push(Diagnostic::error(
                ErrorCode::E8502,
                lstr!(en: "struct '{}' has no field '{}'", name.text, f.text;
                      tr: "'{}' struct'ında '{}' alanı yok", name.text, f.text),
                LabeledSpan::primary(f.span, lstr!(en: "unknown field"; tr: "bilinmeyen alan")),
                lstr!(en: "available fields: {}", level.join(", "); tr: "mevcut alanlar: {}", level.join(", ")),
            ));
            continue;
        }
        let mut path = prefix.to_vec();
        path.push(f.text.clone());
        let is_leaf = layout.leaves.iter().any(|l: &Leaf| l.path == path);
        if is_leaf {
            check_scalar(v, diags);
        } else {
            check_literal_at(layout, v, &path, check_scalar, diags);
        }
    }
    let missing: Vec<&String> = level
        .iter()
        .filter(|n| !fields.iter().any(|(f, _)| &f.text == *n))
        .collect();
    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|m| format!("'{m}'"))
            .collect::<Vec<_>>()
            .join(", ");
        diags.push(type_mismatch(
            lit.span,
            lstr!(en: "the '{}' literal is missing field(s) {list} — every field must be given", name.text;
                  tr: "'{}' literalinde {list} alan(lar)ı eksik — her alan verilmeli", name.text),
        ));
    }
}
