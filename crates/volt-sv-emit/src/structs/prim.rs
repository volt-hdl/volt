//! Struct öğeli yerleşik primitifler (ADR-0087): `SyncFifo<Packet, 8>`.
//!
//! Primitif `T`'yi paketlenmiş vektör olarak saklar (bellek tek
//! `logic [W-1:0] mem [DEPTH]` — alan başına bellek değil, BRAM çıkarımı
//! korunur, ADR-0077 Karar 2 AoS). İndirgemeden ÖNCE:
//!
//! * veri GİRİŞİ bağlaması `wr_data: e` → `wr_data: e as uint<W>`
//!   (Karar 3 düzeni, `p as uN` kuralı paketler),
//! * veri ÇIKIŞI okuması `m.rd_data` → `(m.rd_data) as Packet` (yaprak
//!   başına dilim, `raw as P` kuralı açar).
//!
//! Açılan değeri yalnız derleyici yazdı: FIFO ailesi yazılmış değerleri
//! ya da `T`'nin varsayılan kodlamasıyla reset'lenen register'ı gösterir;
//! RAM ailesinde enum/`Trit` yapraklı `T` HIR'da E2009'dur. Böylece
//! ADR-0077'nin `bits → enum alanlı struct` yasağı iç işleyişte delinmez.

use std::collections::HashMap;

use volt_ast::builtin::{BuiltinPrim, PortKind};
use volt_ast::struct_layout;
use volt_ast::visit::walk_expr;
use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, ExprKind, GenericArg, Idx, IfStmt, ModuleDecl, Path,
    PortDir, SourceFile, StmtKind, TypeRef, TypeRefKind,
};

/// Modülün struct öğeli primitif örneklerini paketler/açar. Dönüş: en
/// az bir örnek yeniden yazıldı mı?
pub(super) fn pack_struct_prims(ast: &mut SourceFile, m: &ModuleDecl) -> bool {
    // örnek adı → (struct tip referansı, veri çıkış portları)
    let mut outputs: HashMap<String, (Idx<TypeRef>, Vec<&'static str>)> = HashMap::new();
    for &stmt in &m.body {
        let StmtKind::Instance(inst) = &ast.stmts[stmt].kind else {
            continue;
        };
        let [seg] = inst.module_path.segments.as_slice() else {
            continue;
        };
        let Some(prim) = BuiltinPrim::from_name(&seg.text) else {
            continue;
        };
        let Some(GenericArg::Type(t)) = inst.generic_args.first() else {
            continue;
        };
        let t = *t;
        let Some(decl) = struct_layout::struct_of_type(ast, t) else {
            continue;
        };
        let Ok(layout) = struct_layout::layout(ast, decl, &mut |e| super::const_int(ast, e)) else {
            continue; // düzen hatası HIR'da
        };
        let width = layout.width;
        let name = inst.name.text.clone();
        let data_outs: Vec<&'static str> = prim
            .ports()
            .iter()
            .filter(|p| p.kind == PortKind::Data && p.dir == PortDir::Out)
            .map(|p| p.name)
            .collect();
        pack_inputs(ast, stmt, prim, width);
        outputs.insert(name, (t, data_outs));
    }
    if outputs.is_empty() {
        return false;
    }
    let mut reads = Vec::new();
    for root in module_roots(ast, m) {
        walk_expr(ast, root, |e| {
            if let Some(t) = data_read(ast, e, &outputs) {
                reads.push((e, t));
            }
        });
    }
    for (e, t) in reads {
        let copy = ast.exprs[e].clone();
        let raw = ast.exprs.alloc(copy);
        ast.exprs[e].kind = ExprKind::Cast { expr: raw, ty: t };
    }
    true
}

/// `wr_data: e` → `wr_data: e as uint<W>`; `wr_data` kısayolu (değersiz
/// bağlama) aynı adlı sinyalin yolu olur.
fn pack_inputs(ast: &mut SourceFile, stmt: Idx<volt_ast::Stmt>, prim: BuiltinPrim, width: u32) {
    let StmtKind::Instance(inst) = &ast.stmts[stmt].kind else {
        return;
    };
    let mut bindings = inst.bindings.clone();
    for b in &mut bindings {
        let is_data_in = prim
            .port(&b.port_name.text)
            .is_some_and(|p| p.kind == PortKind::Data && p.dir == PortDir::In);
        if !is_data_in {
            continue;
        }
        let span = b.value.map_or(b.port_name.span, |v| ast.exprs[v].span);
        let value = b.value.unwrap_or_else(|| {
            ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::Path(Path {
                    span,
                    segments: vec![b.port_name.clone()],
                }),
            })
        });
        let w = ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::IntLit {
                value: u128::from(width),
                suffix: None,
                base: volt_ast::NumBase::Dec,
            },
        });
        let ty = ast.types.alloc(TypeRef {
            span,
            kind: TypeRefKind::UIntN(w),
        });
        b.value = Some(ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::Cast { expr: value, ty },
        }));
    }
    if let StmtKind::Instance(inst) = &mut ast.stmts[stmt].kind {
        inst.bindings = bindings;
    }
}

/// `m.rd_data` (struct öğeli örneğin veri çıkışı) → struct tip referansı.
fn data_read(
    ast: &SourceFile,
    e: Idx<Expr>,
    outputs: &HashMap<String, (Idx<TypeRef>, Vec<&'static str>)>,
) -> Option<Idx<TypeRef>> {
    let ExprKind::Field { base, field } = &ast.exprs[e].kind else {
        return None;
    };
    let ExprKind::Path(p) = &ast.exprs[*base].kind else {
        return None;
    };
    let [inst] = p.segments.as_slice() else {
        return None;
    };
    let (t, outs) = outputs.get(&inst.text)?;
    outs.contains(&field.text.as_str()).then_some(*t)
}

/// Modülün bütün kök ifadeleri (deyim, blok, bağlama, kontrat).
fn module_roots(ast: &SourceFile, m: &ModuleDecl) -> Vec<Idx<Expr>> {
    let mut out = Vec::new();
    for &stmt in &m.body {
        match &ast.stmts[stmt].kind {
            StmtKind::Reg(r) => out.push(r.init),
            StmtKind::Let(l) => out.push(l.value),
            StmtKind::Instance(i) => out.extend(i.bindings.iter().filter_map(|b| b.value)),
            StmtKind::On(on) => block_roots(ast, on.body, &mut out),
            StmtKind::Comb(b) => block_roots(ast, *b, &mut out),
            StmtKind::Assign(a) => out.push(a.rhs),
            StmtKind::Expr(e) => out.push(*e),
            StmtKind::For(f) => {
                out.extend([f.start, f.end]);
                block_roots(ast, f.body, &mut out);
            }
            StmtKind::Wire(_) | StmtKind::Error => {}
        }
    }
    out.extend(m.contracts.iter().map(|c| c.expr));
    out
}

fn block_roots(ast: &SourceFile, block: Idx<Block>, out: &mut Vec<Idx<Expr>>) {
    for s in &ast.blocks[block].stmts {
        match s {
            BlockStmt::NonBlockAssign { rhs, .. } | BlockStmt::BlockAssign { rhs, .. } => {
                out.push(*rhs);
            }
            BlockStmt::If(i) => if_roots(ast, i, out),
            BlockStmt::Match(m) => {
                out.push(m.scrutinee);
                for arm in &m.arms {
                    out.extend(arm.guard);
                    match arm.body {
                        volt_ast::MatchArmBody::Expr(e) => out.push(e),
                        volt_ast::MatchArmBody::Block(b) => block_roots(ast, b, out),
                    }
                }
            }
            BlockStmt::Let(l) => out.push(l.value),
            BlockStmt::For(f) => {
                out.extend([f.start, f.end]);
                block_roots(ast, f.body, out);
            }
            BlockStmt::Error => {}
        }
    }
    out.extend(ast.blocks[block].tail);
}

fn if_roots(ast: &SourceFile, i: &IfStmt, out: &mut Vec<Idx<Expr>>) {
    out.push(i.cond);
    block_roots(ast, i.then_block, out);
    match &i.else_branch {
        Some(ElseBranch::Block(b)) => block_roots(ast, *b, out),
        Some(ElseBranch::If(inner)) => if_roots(ast, inner, out),
        None => {}
    }
}
