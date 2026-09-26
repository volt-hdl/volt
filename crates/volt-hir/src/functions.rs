//! Fonksiyon denetimleri (ADR-0081): saflık, desteklenmeyen yapılar,
//! özyineleme ve açılım bütçesi.
//!
//! Tip denetimi (`typeck::func`) gövdeyi imzaya karşı denetler; bu geçit
//! fn'in donanıma inebilmesi için gereken yapısal kuralları uygular:
//!
//! * **Saflık (Karar 1):** gövdede `sync()`/`sync3()` ya da örnek
//!   (`M { … }`) → E2016. `reg`/`on`/`comb`/atama parser'da E2016 aldı.
//! * **`declassify` (Karar 10):** gövdede → E3015.
//! * **Bu turda eşlemesi yok (Karar 3, 7, 8):** gövdede `for`, generic fn,
//!   fn kontratı → E0003.
//! * **Özyineleme (Karar 6):** çağrı çizgesi — düğüm fn tanımı, kenar
//!   gövdedeki (ve kontratlardaki) çağrı; çözüm tabanlı (fn adı bir `let`
//!   tarafından gölgelenebilir). Döngüdeki her fn E4013 (E4009 biçimi).
//! * **Bütçe (Karar 11):** döngüsüz çizgede her fn'in açılmış düğüm
//!   sayısı bir kez hesaplanır (`let` başvurusu değerinin boyutu kadar);
//!   birimdeki çağrıların toplamı `MAX_EXPANSION_NODES`'u aşarsa E2027
//!   çağrı yerinde (birimde bir kez).
//!
//! Tanının yeri ilkesi: fn'in kendisiyle ilgili hata tanımda ve bir kez;
//! kullanımla ilgili hata (bütçe) çağrı yerinde.

use std::collections::{HashMap, HashSet};

use volt_ast::visit::walk_expr;
use volt_ast::{
    graph, BlockStmt, Expr, ExprKind, FnDecl, Idx, ItemKind, SourceFile, MAX_EXPANSION_NODES,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::resolve::{BuiltinKind, DefId, DefKind, ResolveResult};

/// Döngü yolu notunda tam yolun yazıldığı en büyük bileşen (E4009 ile
/// aynı sınır — büyük bileşende yol yerine sayı).
const MAX_PATH_COMPONENT: usize = 16;

/// Çizge düğümü: bir fn tanımı.
struct FnNode<'a> {
    decl: &'a FnDecl,
    /// Gövde ve kontratlardaki kullanıcı fn çağrıları (kaynak sırası):
    /// (çağrı ifadesi, hedef düğüm).
    calls: Vec<(Idx<Expr>, usize)>,
    /// Çağrı başına hedef listesi (`graph` üye biçimi).
    targets: Vec<Vec<usize>>,
}

/// Birimdeki fn'leri denetler.
pub fn check_functions(ast: &SourceFile, res: &ResolveResult) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut nodes: Vec<FnNode> = Vec::new();
    let mut index: HashMap<DefId, usize> = HashMap::new();
    for &item in &ast.items {
        let ItemKind::Fn(f) = &ast.items_arena[item].kind else {
            continue;
        };
        let Some(&def) = res.decl_spans.get(&f.name.span) else {
            continue;
        };
        index.insert(def, nodes.len());
        nodes.push(FnNode {
            decl: f,
            calls: Vec::new(),
            targets: Vec::new(),
        });
    }
    let mut body_exprs: HashSet<Idx<Expr>> = HashSet::new();
    for node in &mut nodes {
        check_unsupported(ast, node.decl, &mut diags);
        for root in fn_roots(ast, node.decl) {
            walk_expr(ast, root, |e| {
                body_exprs.insert(e);
                inspect(ast, res, e, &index, &mut node.calls, &mut diags);
            });
        }
        node.targets = node.calls.iter().map(|&(_, t)| vec![t]).collect();
    }

    let n = nodes.len();
    let comp = graph::components(n, |v| &nodes[v].targets);
    let cycles = graph::cycle_edges(n, |v| &nodes[v].targets, &comp);
    let cyclic: HashSet<usize> = cycles.iter().map(|e| e.node).collect();
    for e in &cycles {
        let path = if e.size <= MAX_PATH_COMPONENT {
            let mut steps = vec![(e.node, e.member)];
            steps.extend(graph::path_within(
                |v| &nodes[v].targets,
                &comp,
                e.target,
                e.node,
            ));
            cycle_path(&nodes, &steps)
        } else {
            lstr!(en: "{} functions", e.size; tr: "{} fonksiyon", e.size)
        };
        diags.push(err_recursive_fn(ast, &nodes, e.node, e.member, &path));
    }

    let sizes = expanded_sizes(ast, res, &nodes, &comp, &cyclic);
    check_budget(ast, res, &index, &sizes, &body_exprs, &mut diags);
    diags
}

/// fn'in ifade kökleri: `let` değerleri, son ifade, kontratlar.
fn fn_roots(ast: &SourceFile, f: &FnDecl) -> Vec<Idx<Expr>> {
    let block = &ast.blocks[f.body];
    let mut roots: Vec<Idx<Expr>> = block
        .stmts
        .iter()
        .filter_map(|s| match s {
            BlockStmt::Let(l) => Some(l.value),
            _ => None,
        })
        .collect();
    roots.extend(block.tail);
    roots.extend(f.contracts.iter().map(|c| c.expr));
    roots
}

/// Tek ifade: çağrı kenarı, `sync`/örnek (E2016), `declassify` (E3015).
fn inspect(
    ast: &SourceFile,
    res: &ResolveResult,
    e: Idx<Expr>,
    index: &HashMap<DefId, usize>,
    calls: &mut Vec<(Idx<Expr>, usize)>,
    diags: &mut Vec<Diagnostic>,
) {
    let span = ast.exprs[e].span;
    if let Some(site) = ast.trust.declassify.get(&e) {
        diags.push(err_declassify_in_fn(site.span));
    }
    match &ast.exprs[e].kind {
        ExprKind::Call { callee, .. } => {
            let Some(&def) = res.resolutions.get(callee) else {
                return;
            };
            match res.def_kind(def) {
                DefKind::Function => {
                    if let Some(&t) = index.get(&def) {
                        calls.push((e, t));
                    }
                }
                DefKind::Builtin(b @ (BuiltinKind::Sync | BuiltinKind::Sync3)) => {
                    let name = if b == BuiltinKind::Sync3 {
                        "sync3()"
                    } else {
                        "sync()"
                    };
                    let what =
                        lstr!(en: "'{name}' (a register chain)"; tr: "'{name}' (register zinciri)");
                    diags.push(err_not_combinational(span, &what));
                }
                _ => {}
            }
        }
        ExprKind::StructLit { .. } => {
            if let Some(&def) = res.resolutions.get(&e) {
                if matches!(res.def_kind(def), DefKind::Module | DefKind::ExternModule) {
                    let what = lstr!(en: "a module instance"; tr: "modül örneği");
                    diags.push(err_not_combinational(span, &what));
                }
            }
        }
        _ => {}
    }
}

/// Bu turda SV eşlemesi olmayan fn yapıları (E0003): generic fn, fn
/// kontratı, gövdede `for`.
fn check_unsupported(ast: &SourceFile, f: &FnDecl, diags: &mut Vec<Diagnostic>) {
    if !f.generics.is_empty() {
        diags.push(err_unsupported(
            f.name.span,
            &lstr!(en: "generic functions"; tr: "generic fonksiyonlar"),
            &lstr!(en: "write one function per width; a function's generic parameters are not inferred from the call yet (ADR-0081)";
                   tr: "her genişlik için bir fonksiyon yazın; fonksiyonun generic parametreleri henüz çağrıdan çıkarılmıyor (ADR-0081)"),
        ));
    }
    for c in &f.contracts {
        diags.push(err_unsupported(
            c.span,
            &lstr!(en: "contracts on functions (requires/ensures)"; tr: "fonksiyon kontratları (requires/ensures)"),
            &lstr!(en: "state the property in the calling module's contracts, where the call's arguments are signals (ADR-0081)";
                   tr: "özelliği çağıran modülün kontratlarında, argümanların sinyal olduğu yerde yazın (ADR-0081)"),
        ));
    }
    for s in &ast.blocks[f.body].stmts {
        if let BlockStmt::For(fs) = s {
            let body = ast.blocks[fs.body].span;
            let span = Span::new(fs.var.span.file, fs.var.span.start, body.end).with_ctx(body.ctx);
            diags.push(err_unsupported(
                span,
                &lstr!(en: "'for' loops in function bodies"; tr: "fonksiyon gövdesinde 'for' döngüsü"),
                &lstr!(en: "use builtins (popcount, concat, replicate) or an if chain; a folding 'for' in functions is future work (ADR-0081)";
                       tr: "yerleşikleri (popcount, concat, replicate) ya da if zincirini kullanın; fonksiyonda katlayan 'for' ileride (ADR-0081)"),
            ));
        }
    }
}

/// Karar 11: fn başına açılmış düğüm sayısı. Tarjan bileşenleri ters
/// topolojik sırada numaralanır (önce çağrılanlar); döngüdeki fn 0
/// sayılır (E4013 zaten raporlandı, açılmaz).
fn expanded_sizes(
    ast: &SourceFile,
    res: &ResolveResult,
    nodes: &[FnNode],
    comp: &[usize],
    cyclic: &HashSet<usize>,
) -> HashMap<DefId, usize> {
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&v| comp[v]);
    let mut sizes: HashMap<DefId, usize> = HashMap::new();
    for v in order {
        let f = nodes[v].decl;
        let Some(&def) = res.decl_spans.get(&f.name.span) else {
            continue;
        };
        if cyclic.contains(&v) {
            sizes.insert(def, 0);
            continue;
        }
        let mut lets: HashMap<DefId, usize> = HashMap::new();
        let block = &ast.blocks[f.body];
        for s in &block.stmts {
            if let BlockStmt::Let(l) = s {
                let n = expr_size(ast, res, l.value, &lets, &sizes);
                if let Some(&ld) = res.decl_spans.get(&l.name.span) {
                    lets.insert(ld, n);
                }
            }
        }
        let n = block
            .tail
            .map_or(0, |t| expr_size(ast, res, t, &lets, &sizes));
        sizes.insert(def, n);
    }
    sizes
}

/// İkame boyutu: her düğüm 1; `let` başvurusu değerinin boyutu; iç
/// çağrı çağrılanın açılmış boyutu.
fn expr_size(
    ast: &SourceFile,
    res: &ResolveResult,
    root: Idx<Expr>,
    lets: &HashMap<DefId, usize>,
    fns: &HashMap<DefId, usize>,
) -> usize {
    let mut total = 0usize;
    walk_expr(ast, root, |e| {
        let add = match &ast.exprs[e].kind {
            ExprKind::Path(_) => res
                .resolutions
                .get(&e)
                .and_then(|d| lets.get(d))
                .copied()
                .unwrap_or(1),
            ExprKind::Call { callee, .. } => {
                1 + res
                    .resolutions
                    .get(callee)
                    .and_then(|d| fns.get(d))
                    .copied()
                    .unwrap_or(0)
            }
            _ => 1,
        };
        total = total.saturating_add(add);
    });
    total
}

/// Birimdeki fn dışı çağrı yerleri (modül gövdeleri ve kontratları)
/// bütçeden düşer; aşan ilk çağrı E2027 alır.
fn check_budget(
    ast: &SourceFile,
    res: &ResolveResult,
    index: &HashMap<DefId, usize>,
    sizes: &HashMap<DefId, usize>,
    body_exprs: &HashSet<Idx<Expr>>,
    diags: &mut Vec<Diagnostic>,
) {
    let mut used = 0usize;
    for (e, expr) in ast.exprs.iter_idx() {
        let ExprKind::Call { callee, .. } = &expr.kind else {
            continue;
        };
        if body_exprs.contains(&e) {
            continue;
        }
        let Some(&def) = res.resolutions.get(callee) else {
            continue;
        };
        if !index.contains_key(&def) {
            continue;
        }
        let size = sizes.get(&def).copied().unwrap_or(0);
        used = used.saturating_add(size.saturating_add(1));
        if used > MAX_EXPANSION_NODES {
            let name = res.defs[def.0 as usize].name.clone();
            diags.push(err_budget(expr.span, &name, size));
            return;
        }
    }
}

// ═══ Tanılar ══════════════════════════════════════════════════════

/// E2016 — fn gövdesinde durum kuran yapı (parser'ınkiyle aynı metin).
fn err_not_combinational(span: Span, what: &str) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E2016,
        lstr!(en: "a function must be combinational: {what} is not allowed in a function body";
              tr: "fonksiyon kombinasyonel olmalı: fonksiyon gövdesinde {what} kullanılamaz"),
        LabeledSpan::primary(
            span,
            lstr!(en: "state or a driver inside a function"; tr: "fonksiyonda durum ya da sürücü"),
        ),
        lstr!(en: "compute the value with let bindings and a final expression; if you need state, write a module";
              tr: "değeri let bağlamaları ve son ifadeyle hesaplayın; durum gerekiyorsa modül yazın"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "a function call is inlined as combinational logic at every call site (ADR-0081)";
              tr: "fonksiyon çağrısı her çağrı yerinde kombinasyonel mantık olarak açılır (ADR-0081)"),
    )
}

/// E3015 — fn gövdesinde `declassify`.
fn err_declassify_in_fn(span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3015,
        lstr!(en: "declassify cannot be used inside a function body"; tr: "fonksiyon gövdesinde declassify kullanılamaz"),
        LabeledSpan::primary(
            span,
            lstr!(en: "hidden trust downgrade at every call site"; tr: "her çağrı yerinde görünmez güven düşürme"),
        ),
        lstr!(en: "return the value and declassify the call's result in the calling module: declassify(f(x), \"reason\")";
              tr: "değeri döndürün, çağrının sonucunu çağıran modülde declassify edin: declassify(f(x), \"gerekçe\")"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "a declassify must be visible, with its reason, where the downgrade happens (ADR-0052, ADR-0081)";
              tr: "declassify, gerekçesiyle, düşürmenin yapıldığı yerde görünmelidir (ADR-0052, ADR-0081)"),
    )
}

/// E0003 — bu turda eşlemesi olmayan fn yapısı.
fn err_unsupported(span: Span, what: &str, help: &str) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E0003,
        lstr!(en: "not supported yet: {what}"; tr: "henüz desteklenmiyor: {what}"),
        LabeledSpan::primary(
            span,
            lstr!(en: "no hardware mapping yet"; tr: "henüz donanım eşlemesi yok"),
        ),
        help.to_string(),
    )
}

/// E4013 — özyineli fn (E4009 biçimi).
fn err_recursive_fn(
    ast: &SourceFile,
    nodes: &[FnNode],
    i: usize,
    m: usize,
    path: &str,
) -> Diagnostic {
    let f = nodes[i].decl;
    let n = f.name.text.as_str();
    let (call, _) = nodes[i].calls[m];
    Diagnostic::error(
        ErrorCode::E4013,
        lstr!(en: "function '{n}' calls itself (a call leads back to '{n}')";
              tr: "'{n}' fonksiyonu kendini çağırıyor (bir çağrı '{n}' fonksiyonuna geri dönüyor)"),
        LabeledSpan::primary(
            f.name.span,
            lstr!(en: "recursive function"; tr: "özyineli fonksiyon"),
        ),
        lstr!(en: "break the cycle: a function is inlined at every call, so it cannot call itself; unroll the levels into separate functions";
              tr: "döngüyü kırın: fonksiyon her çağrıda açılır, kendini çağıramaz; seviyeleri ayrı fonksiyonlara açın"),
    )
    .with_secondary(
        ast.exprs[call].span,
        lstr!(en: "this call closes the cycle"; tr: "döngüyü bu çağrı kapatıyor"),
    )
    .with_note(NoteKind::Note, lstr!(en: "cycle: {path}"; tr: "döngü: {path}"))
}

/// `f → g → f`.
fn cycle_path(nodes: &[FnNode], steps: &[(usize, usize)]) -> String {
    let mut s = String::new();
    for &(v, _) in steps {
        s.push_str(&nodes[v].decl.name.text);
        s.push_str(" → ");
    }
    if let Some(&(v, _)) = steps.first() {
        s.push_str(&nodes[v].decl.name.text);
    }
    s
}

/// E2027 — fn açılımı birim bütçesini aştı.
fn err_budget(span: Span, name: &str, size: usize) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E2027,
        lstr!(en: "inlining the call to '{name}' exceeded the AST node budget ({MAX_EXPANSION_NODES} nodes per compilation unit)";
              tr: "'{name}' çağrısını açmak AST düğüm bütçesini aştı (derleme birimi başına {MAX_EXPANSION_NODES} düğüm)"),
        LabeledSpan::primary(
            span,
            lstr!(en: "expansion budget exceeded here"; tr: "açılım bütçesi burada aşıldı"),
        ),
        lstr!(en: "a function that calls another function twice doubles at every level; share the result in a let, or restructure the computation";
              tr: "başka bir fonksiyonu iki kez çağıran fonksiyon her seviyede ikiye katlanır; sonucu bir let'te paylaşın ya da hesabı yeniden yapılandırın"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "one call to '{name}' expands to {size} nodes (ADR-0068, ADR-0081)";
              tr: "'{name}' çağrısı başına {size} düğüm açılır (ADR-0068, ADR-0081)"),
    )
}
