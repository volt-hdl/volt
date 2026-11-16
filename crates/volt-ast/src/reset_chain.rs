//! Ham reset zincirinin tüketilip tüketilmediği (ADR-0065 §2, ADR-0072).
//!
//! Ham portla beslenen her saat için sv-emit bir bırakma senkronizörü,
//! volt-hir `constraints` de onun SDC kısıtını üretir. Yalnız ham portu
//! bir çocuğa geçiren ara seviyede (`examples/hybrid_accel` HybridTb)
//! zincirin çıkışını hiçbir şey okumaz: ölü mantık, Verilator
//! `UNUSEDSIGNAL`, var olmayan hücrelere SDC kısıtı. İki taraf aynı
//! kararı buradan alır. Kural volt-hir `rdc::converge::chain_used` ile
//! aynıdır; ek olarak kontratlı modül tutucu biçimde "kullanıyor" sayılır
//! (SVA `disable iff` / `initial assume` alanın reset'ine bakar).

use crate::builtin::BuiltinPrim;
use crate::{ExprKind, Idx, ModuleDecl, OnBlock, OnTrigger, RegDecl, SourceFile, Stmt, StmtKind};

/// `clk` saatinin zinciri modülde bir şeyi sıfırlıyor mu: `on clk` /
/// `on clk.reset`, `reg(clk)`, `sync(_, clk)`, flop'larını reset'leyen
/// yerleşik bağlaması, modül kontratları ya da bir çocuğun OTOMATİK
/// reset'li saatine bağlama (`child_auto_reset(çocuk, port)`; zincir
/// çıkışı o porta gider). Extern ve kullanıcı modülü bağlamaları kendi
/// başına saymaz; reset'siz yerleşik bağlaması (`AsyncDualPortRam.wr_clk`)
/// da saymaz.
pub fn chain_consumed(
    ast: &SourceFile,
    module: &ModuleDecl,
    clk: &str,
    child_auto_reset: impl Fn(&ModuleDecl, &str) -> bool,
) -> bool {
    if !module.contracts.is_empty() {
        return true;
    }
    module
        .body
        .iter()
        .any(|&s| stmt_consumes(ast, s, clk, &child_auto_reset))
        || syncs_to(ast, module, clk)
}

fn stmt_consumes(
    ast: &SourceFile,
    s: Idx<Stmt>,
    clk: &str,
    child_auto_reset: &impl Fn(&ModuleDecl, &str) -> bool,
) -> bool {
    match &ast.stmts[s].kind {
        StmtKind::On(OnBlock {
            trigger: OnTrigger::Clock(n) | OnTrigger::Reset(n),
            ..
        }) => n.text == clk,
        StmtKind::Reg(RegDecl {
            domain: Some(n), ..
        }) => n.text == clk,
        StmtKind::Instance(inst) => {
            let [seg] = inst.module_path.segments.as_slice() else {
                return false;
            };
            let prim = BuiltinPrim::from_name(&seg.text);
            let child = module_named(ast, &seg.text);
            inst.bindings.iter().any(|b| {
                let value = match b.value {
                    Some(e) => path_single(ast, e),
                    None => Some(b.port_name.text.as_str()),
                };
                if value != Some(clk) {
                    return false;
                }
                match (prim, child) {
                    (Some(p), _) => p.clock_resets_flops(&b.port_name.text),
                    (None, Some(c)) => child_auto_reset(c, &b.port_name.text),
                    (None, None) => false,
                }
            })
        }
        _ => false,
    }
}

/// Dosyadaki `module <name>` bildirimi.
fn module_named<'a>(ast: &'a SourceFile, name: &str) -> Option<&'a ModuleDecl> {
    ast.items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            crate::ItemKind::Module(m) if m.name.text == name => Some(m),
            _ => None,
        })
}

/// Modülün gövde aralığında `sync(_, clk)` / `sync3(_, clk)` çağrısı var
/// mı. Aralık deyim konumlarından: aynı dosyada, açılım bağlamı (`ctx`)
/// gözetmeden — monomorf kardeş kopyaları da sayılır (tutucu yön).
pub fn syncs_to(ast: &SourceFile, module: &ModuleDecl, clk: &str) -> bool {
    let mut spans = module.body.iter().map(|&s| ast.stmts[s].span);
    let Some(first) = spans.next() else {
        return false;
    };
    let (start, end) = spans.fold((first.start, first.end), |(a, b), s| {
        (a.min(s.start), b.max(s.end))
    });
    ast.exprs.iter().any(|e| {
        let ExprKind::Call { callee, args } = &e.kind else {
            return false;
        };
        e.span.file == first.file
            && start <= e.span.start
            && e.span.end <= end
            && matches!(path_single(ast, *callee), Some("sync" | "sync3"))
            && args.get(1).and_then(|&a| path_single(ast, a)) == Some(clk)
    })
}

/// Tek segmentli yol ifadesinin adı.
fn path_single(ast: &SourceFile, e: Idx<crate::Expr>) -> Option<&str> {
    match &ast.exprs[e].kind {
        ExprKind::Path(p) => match p.segments.as_slice() {
            [seg] => Some(seg.text.as_str()),
            _ => None,
        },
        _ => None,
    }
}
