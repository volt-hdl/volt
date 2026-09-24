//! Ham reset portu ve bırakma senkronizörü (ADR-0065 §1-§2).
//!
//! `in ext_rst_n : reset(async, active_low)` beslediği her saat portu
//! için iki aşamalı bir zincir üretir (`rst_sync_<saat>_stage<i>`): ham
//! portla asenkron etkinleşir, saat kenarıyla senkron bırakılır. Alanın
//! register'ları zincirin son aşamasıyla sıfırlanır
//! (`ResetCfg::synced`); o alan otomatik `rst`/`rst_n` portu almaz.
//! Bağlama kuralı volt-hir `domain/rdc/facts.rs::feeds_of` ile aynıdır.

use volt_ast::builtin::BuiltinPrim;
use volt_ast::{
    ClockEdge, ExprKind, ModuleDecl, OnBlock, OnTrigger, Port, PortDir, RegDecl, ResetPolarity,
    SourceFile, StmtKind, TypeRefKind,
};

use volt_span::Span;

use crate::{ClockPort, DomainInfo};

/// Zincir uzunluğu (ADR-0065 §2; `@reset_stages(N)` gelecek iş).
pub(crate) const RESET_SYNC_STAGES: usize = 2;

/// Bir saat alanını besleyen ham reset portu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawReset {
    pub(crate) name: String,
    /// Etkinleşme polaritesi: port türünden, yazılmamışsa alandan.
    pub(crate) polarity: ResetPolarity,
}

/// Zincir aşamasının adı; son aşama alanın reset sinyalidir.
pub(crate) fn stage_name(clk: &str, stage: usize) -> String {
    format!("rst_sync_{clk}_stage{stage}")
}

/// Ham reset portu: giriş yönlü `reset` tipli port.
pub(crate) fn is_raw_reset(ast: &SourceFile, port: &Port) -> bool {
    port.direction == PortDir::In
        && matches!(
            ast.types[crate::alias::resolve(ast, port.ty)].kind,
            TypeRefKind::Reset(_)
        )
}

/// ADR-0065 §1: tek anotasyonsuz ham port reset'li bütün alanları
/// besler; aksi hâlde `@D` anotasyonu saat portunun alanıyla eşleşen.
pub(crate) fn feeding_raw(
    ast: &SourceFile,
    module: &ModuleDecl,
    clock_domain: Option<&str>,
    info: &DomainInfo,
) -> Option<RawReset> {
    if info.reset.is_none() {
        return None;
    }
    let raws: Vec<&Port> = module
        .ports
        .iter()
        .filter(|p| is_raw_reset(ast, p))
        .collect();
    let port = match raws.as_slice() {
        [only] if only.domain.is_none() => *only,
        raws => *raws.iter().find(|r| {
            r.domain
                .as_ref()
                .is_some_and(|d| Some(d.text.as_str()) == clock_domain)
        })?,
    };
    let polarity = match ast.types[crate::alias::resolve(ast, port.ty)].kind {
        TypeRefKind::Reset(Some(spec)) => spec.polarity,
        _ => info.reset.polarity,
    };
    Some(RawReset {
        name: port.name.text.clone(),
        polarity,
    })
}

/// Saat modülde yalnız reset'siz yerleşik saat bağlamasında mı
/// kullanılıyor (ADR-0065 R5', `BuiltinPrim::clock_resets_flops`)?
/// Tutucu: `on clk`, `reg(clk)`, başka bir örnek bağlaması ya da
/// modül gövdesinde `sync(_, clk)` saati reset örnekleyen
/// sayar. volt-hir `rdc::facts::ClockFact::used` ile aynı kural.
pub(crate) fn reset_free_clock(ast: &SourceFile, module: &ModuleDecl, clk: &str) -> bool {
    let mut reset_free_use = false;
    for &s in &module.body {
        match &ast.stmts[s].kind {
            StmtKind::On(OnBlock {
                trigger: OnTrigger::Clock(n),
                ..
            }) if n.text == clk => return false,
            StmtKind::Reg(RegDecl {
                domain: Some(n), ..
            }) if n.text == clk => return false,
            StmtKind::Instance(inst) => {
                let prim = match inst.module_path.segments.as_slice() {
                    [seg] => BuiltinPrim::from_name(&seg.text),
                    _ => None,
                };
                for b in &inst.bindings {
                    let value = match b.value {
                        Some(e) => crate::path_single(ast, e),
                        None => Some(b.port_name.text.as_str()),
                    };
                    if value != Some(clk) {
                        continue;
                    }
                    match prim {
                        Some(p) if !p.clock_resets_flops(&b.port_name.text) => {
                            reset_free_use = true;
                        }
                        _ => return false,
                    }
                }
            }
            _ => {}
        }
    }
    reset_free_use && !synced_to(ast, module, clk)
}

/// Modülün gövde aralığında `sync(_, clk)` çağrısı var mı. Aralık
/// deyim konumlarından: aynı dosyada, açılım bağlamı (`ctx`) gözetmeden
/// — monomorf kardeş kopyaları da sayılır (tutucu yön).
fn synced_to(ast: &SourceFile, module: &ModuleDecl, clk: &str) -> bool {
    let spans: Vec<Span> = module.body.iter().map(|&s| ast.stmts[s].span).collect();
    let Some(first) = spans.first() else {
        return false;
    };
    let file = first.file;
    let start = spans.iter().map(|s| s.start).min().unwrap_or(0);
    let end = spans.iter().map(|s| s.end).max().unwrap_or(0);
    ast.exprs.iter().any(|e| {
        let ExprKind::Call { callee, args } = &e.kind else {
            return false;
        };
        e.span.file == file
            && start <= e.span.start
            && e.span.end <= end
            && crate::path_single(ast, *callee) == Some("sync")
            && args.get(1).and_then(|&a| crate::path_single(ast, a)) == Some(clk)
    })
}

/// Saat portunun bırakma senkronizörü (ADR-0065 §2). Zincir ham portun
/// polaritesiyle etkinleşir; aşamaların değeri alanın polaritesidir
/// (farklıysa ilk aşamada evrilir).
pub(crate) fn synchronizer_block(clock: &ClockPort) -> Option<String> {
    let raw = clock.raw_reset.as_ref()?;
    let clk = &clock.name;
    let edge = match clock.info.edge {
        ClockEdge::Negedge => "negedge",
        _ => "posedge",
    };
    let (sens, cond) = match raw.polarity {
        ResetPolarity::ActiveHigh => (format!("posedge {}", raw.name), raw.name.clone()),
        ResetPolarity::ActiveLow => (format!("negedge {}", raw.name), format!("!{}", raw.name)),
    };
    let (asserted, released) = match clock.info.reset.polarity {
        ResetPolarity::ActiveHigh => ("1'b1", "1'b0"),
        ResetPolarity::ActiveLow => ("1'b0", "1'b1"),
    };
    let mut out = format!(
        "    // reset synchronizer: {} -> {clk} (async assert, sync release)\n",
        raw.name
    );
    for i in 0..RESET_SYNC_STAGES {
        out.push_str(&format!("    logic {};\n", stage_name(clk, i)));
    }
    out.push_str(&format!("    always_ff @({edge} {clk} or {sens}) begin\n"));
    out.push_str(&format!("        if ({cond}) begin\n"));
    for i in 0..RESET_SYNC_STAGES {
        out.push_str(&format!(
            "            {} <= {asserted};\n",
            stage_name(clk, i)
        ));
    }
    out.push_str("        end else begin\n");
    for i in 0..RESET_SYNC_STAGES {
        let prev = match i {
            0 => released.to_string(),
            _ => stage_name(clk, i - 1),
        };
        out.push_str(&format!("            {} <= {prev};\n", stage_name(clk, i)));
    }
    out.push_str("        end\n    end");
    Some(out)
}
