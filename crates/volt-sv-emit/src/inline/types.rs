//! Açılımın tip karşılaştırması: bir argüman parametrenin tipindeyse
//! yerine doğrudan yazılabilir; değilse örtük genişleme ya tel ya boyut
//! dönüşümüyle korunur (ADR-0081 Karar 12, ADR-0041).
//!
//! Emitter tip denetleyicisini görmez; karşılaştırma bildirimlerin
//! `TypeRef`'leri üzerinden yapılır. Bilinmeyen (tipsiz `let`, blok
//! yereli, döngü değişkeni) her zaman "farklı" sayılır — güvenli yön.

use std::collections::HashMap;

use volt_ast::{
    Expr, ExprKind, Idx, ItemKind, ModuleDecl, SourceFile, StmtKind, TypeRef, TypeRefKind,
};

/// Kanonik tip: yazım biçiminden bağımsız (`u9` = `uint<9>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Canon {
    Bool,
    UInt(i128),
    SInt(i128),
    Bits(i128),
    Trit,
    Named(String),
    Array(Box<Canon>, i128),
}

impl Canon {
    /// Genişliği bağlama duyarlı tam sayı ailesi (boyut dönüşümü anlamlı).
    pub(super) fn is_int_like(&self) -> bool {
        matches!(self, Canon::UInt(_) | Canon::SInt(_) | Canon::Bits(_))
    }
}

pub(super) fn canon(ast: &SourceFile, ty: Idx<TypeRef>) -> Option<Canon> {
    canon_depth(ast, ty, 0)
}

fn canon_depth(ast: &SourceFile, ty: Idx<TypeRef>, depth: u32) -> Option<Canon> {
    if depth > volt_ast::MAX_DEPTH {
        return None;
    }
    let ty = crate::alias::resolve(ast, ty);
    let width = |e: Idx<Expr>| crate::structs::const_int(ast, e);
    Some(match &ast.types[ty].kind {
        TypeRefKind::Bool => Canon::Bool,
        TypeRefKind::UInt(w) => Canon::UInt(i128::from(*w)),
        TypeRefKind::SInt(w) => Canon::SInt(i128::from(*w)),
        TypeRefKind::UIntN(e) => Canon::UInt(width(*e)?),
        TypeRefKind::SIntN(e) => Canon::SInt(width(*e)?),
        TypeRefKind::Bits(e) => Canon::Bits(width(*e)?),
        TypeRefKind::Trit => Canon::Trit,
        TypeRefKind::Array { elem, len } => {
            Canon::Array(Box::new(canon_depth(ast, *elem, depth + 1)?), width(*len)?)
        }
        TypeRefKind::Path { path, .. } if path.segments.len() == 1 => {
            let name = &path.segments[0].text;
            match widened(name) {
                Some(c) => c,
                None => Canon::Named(name.clone()),
            }
        }
        _ => return None,
    })
}

/// `u9` / `i33` biçimli genişletilmiş tam sayı adları.
fn widened(name: &str) -> Option<Canon> {
    let (head, digits) = name.split_at(1);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let w: i128 = digits.parse().ok()?;
    match head {
        "u" => Some(Canon::UInt(w)),
        "i" => Some(Canon::SInt(w)),
        _ => None,
    }
}

/// Çağıran modülün adı → bildirilen tip tablosu (port, reg, wire, tipli
/// `let`, örnek adı → modül adı) ve birimin const'ları.
#[derive(Default)]
pub(super) struct DeclTypes {
    pub(super) types: HashMap<String, Idx<TypeRef>>,
    /// Örnek adı → örneklenen modül adı (`u.q` alan yolu için).
    pub(super) instances: HashMap<String, String>,
}

impl DeclTypes {
    pub(super) fn of_module(ast: &SourceFile, m: &ModuleDecl) -> Self {
        let mut d = DeclTypes::default();
        for &item in &ast.items {
            if let ItemKind::Const(c) = &ast.items_arena[item].kind {
                d.types.insert(c.name.text.clone(), c.ty);
            }
        }
        for p in &m.ports {
            d.types.insert(p.name.text.clone(), p.ty);
        }
        for &s in &m.body {
            match &ast.stmts[s].kind {
                StmtKind::Reg(r) => {
                    if let Some(t) = r.ty {
                        d.types.insert(r.name.text.clone(), t);
                    }
                }
                StmtKind::Wire(w) => {
                    d.types.insert(w.name.text.clone(), w.ty);
                }
                StmtKind::Let(l) => {
                    if let Some(t) = l.ty {
                        d.types.insert(l.name.text.clone(), t);
                    }
                }
                StmtKind::Instance(i) if i.module_path.segments.len() == 1 => {
                    d.instances
                        .insert(i.name.text.clone(), i.module_path.segments[0].text.clone());
                }
                _ => {}
            }
        }
        d
    }

    /// Yalın ad, struct alan yolu, dizi elemanı ya da kullanıcı modülü
    /// çıkışı olan ifadenin bildirilen tipi. Blok yerelleri (`locals`) bilinmez.
    pub(super) fn type_of(
        &self,
        ast: &SourceFile,
        e: Idx<Expr>,
        locals: &[String],
    ) -> Option<Idx<TypeRef>> {
        match &ast.exprs[e].kind {
            ExprKind::Path(p) if p.segments.len() == 1 => {
                let name = &p.segments[0].text;
                if locals.contains(name) {
                    return None;
                }
                self.types.get(name).copied()
            }
            ExprKind::Field { base, field } => {
                if let ExprKind::Path(p) = &ast.exprs[*base].kind {
                    if p.segments.len() == 1 && !locals.contains(&p.segments[0].text) {
                        if let Some(module) = self.instances.get(&p.segments[0].text) {
                            return module_port(ast, module, &field.text);
                        }
                    }
                }
                let base_ty = self.type_of(ast, *base, locals)?;
                struct_field(ast, base_ty, &field.text)
            }
            // Dizi elemanı (`arr[i]`): eleman tipi.
            ExprKind::Index { base, .. } => {
                let base_ty = crate::alias::resolve(ast, self.type_of(ast, *base, locals)?);
                match &ast.types[base_ty].kind {
                    TypeRefKind::Array { elem, .. } => Some(*elem),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

fn module_port(ast: &SourceFile, module: &str, port: &str) -> Option<Idx<TypeRef>> {
    ast.items
        .iter()
        .find_map(|&item| match &ast.items_arena[item].kind {
            ItemKind::Module(m) if m.name.text == module => {
                m.ports.iter().find(|p| p.name.text == port).map(|p| p.ty)
            }
            _ => None,
        })
}

fn struct_field(ast: &SourceFile, ty: Idx<TypeRef>, field: &str) -> Option<Idx<TypeRef>> {
    let ty = crate::alias::resolve(ast, ty);
    let TypeRefKind::Path { path, .. } = &ast.types[ty].kind else {
        return None;
    };
    let name = &path.segments.last()?.text;
    ast.items
        .iter()
        .find_map(|&item| match &ast.items_arena[item].kind {
            ItemKind::Struct(s) if !s.is_port && &s.name.text == name => {
                s.fields.iter().find(|f| f.name.text == field).map(|f| f.ty)
            }
            _ => None,
        })
}
