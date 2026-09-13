//! `prev()` — ardışık kontratlar (ADR-0040).
//!
//! `prev(x)` bir önceki döngüdeki, `prev(x, N)` N döngü önceki değerdir
//! ve yalnız kontratlarda geçer (E5017 isim çözümlemede). İki üretim
//! biçimi:
//!
//! - `SvaMode::Inline`/`Separate` (`--emit=sva`, ticari araçlar):
//!   `$past(x)` / `$past(x, N)`.
//! - `SvaMode::Immediate` (`volt verify`, Yosys/SymbiYosys): Yosys'in
//!   Verilog ön ucu `$past`'i bilmez; her farklı argüman için
//!   `past_<x>_1 .. past_<x>_N` yardımcı register zinciri üretilir ve
//!   assertion'da zincirin halkası kullanılır. Reset değeri 0 — reset
//!   sonrası ilk döngüde `prev(x) == 0` kabul edilir.

use volt_ast::{ClockEdge, Expr, ExprKind, Idx, ModuleDecl};

use crate::expr::Sig;
use crate::{ClockPort, Emitter, SvaMode};

/// Yardımcı reg zinciri: (taban ad, kaynak ifade metni, genişlik, azami derinlik).
struct PastChain {
    base: String,
    src: String,
    width: u32,
    max_depth: u32,
}

impl<'a> Emitter<'a> {
    /// Çağrının hedefi `prev` yerleşiği mi? (Emitter isim çözümlemesine
    /// erişmez; `prev` prelude adıdır, gölgelenmesi E1xxx ile yakalanır.)
    pub(crate) fn is_prev_call(&self, callee: Idx<Expr>) -> bool {
        matches!(&self.ast.exprs[callee].kind,
            ExprKind::Path(p) if p.segments.len() == 1 && p.segments[0].text == "prev")
    }

    /// `prev(x[, N])` → `$past(x[, N])`; Immediate modda yardımcı reg adı.
    pub(crate) fn emit_prev(
        &mut self,
        call: Idx<Expr>,
        args: &[Idx<Expr>],
        ctx: Option<Sig>,
    ) -> String {
        let Some(&x) = args.first() else {
            return "1'b0".to_string();
        };
        if self.sva_mode == SvaMode::Immediate {
            if let Some(name) = self.past_regs.get(&call) {
                return name.clone();
            }
        }
        let depth = self.prev_depth(args);
        let inner = self.emit_expr(x, ctx);
        if depth == 1 {
            format!("$past({inner})")
        } else {
            format!("$past({inner}, {depth})")
        }
    }

    /// İkinci argüman (döngü sayısı); yoksa ya da sabit değilse 1.
    fn prev_depth(&self, args: &[Idx<Expr>]) -> u32 {
        args.get(1)
            .and_then(|&n| self.eval_const(n))
            .map(|v| v.clamp(1, u32::MAX as u128) as u32)
            .unwrap_or(1)
    }

    /// Immediate mod: modül kontratlarındaki her `prev()` için yardımcı
    /// register zinciri bildirimi + `always_ff` blokları. `past_regs`
    /// haritasını doldurur; kontrat ifadeleri bundan SONRA üretilmeli.
    pub(crate) fn past_reg_block(
        &mut self,
        module: &'a ModuleDecl,
        clock: &ClockPort,
        indent: usize,
    ) -> Option<String> {
        self.past_regs.clear();
        let mut calls = Vec::new();
        for c in &module.contracts {
            collect_prev_calls(self.ast, c.expr, &mut calls);
        }
        if calls.is_empty() {
            return None;
        }

        let mut chains: Vec<PastChain> = Vec::new();
        let mut anon = 0u32;
        for call in calls {
            let ExprKind::Call { args, .. } = &self.ast.exprs[call].kind else {
                continue;
            };
            let args = args.clone();
            let Some(&x) = args.first() else {
                continue;
            };
            let depth = self.prev_depth(&args);
            let src = self.emit_expr(x, None);
            let width = self.width_of(x).map(|s| s.width).unwrap_or(1);
            let idx = match chains.iter().position(|c| c.src == src) {
                Some(i) => i,
                None => {
                    let base = match &self.ast.exprs[x].kind {
                        ExprKind::Path(p) if p.segments.len() == 1 => format!("past_{src}"),
                        _ => {
                            anon += 1;
                            format!("past_e{anon}")
                        }
                    };
                    chains.push(PastChain {
                        base,
                        src,
                        width,
                        max_depth: 0,
                    });
                    chains.len() - 1
                }
            };
            chains[idx].max_depth = chains[idx].max_depth.max(depth);
            let name = format!("{}_{depth}", chains[idx].base);
            self.past_regs.insert(call, name);
        }

        let ind = " ".repeat(indent);
        let edge = match clock.info.edge {
            ClockEdge::Negedge => "negedge",
            _ => "posedge",
        };
        let reset = (!clock.info.reset.is_none()).then(|| clock.info.reset.condition());
        let mut out = vec![format!(
            "{ind}// prev() helper registers (ADR-0040): value N cycles ago, 0 after reset"
        )];
        for chain in &chains {
            let ty = if chain.width == 1 {
                "logic".to_string()
            } else {
                format!("logic [{}:0]", chain.width - 1)
            };
            for k in 1..=chain.max_depth {
                out.push(format!("{ind}{ty} {}_{k};", chain.base));
            }
            let links: Vec<(String, String)> = (1..=chain.max_depth)
                .map(|k| {
                    let from = if k == 1 {
                        chain.src.clone()
                    } else {
                        format!("{}_{}", chain.base, k - 1)
                    };
                    (format!("{}_{k}", chain.base), from)
                })
                .collect();
            let shift = links
                .iter()
                .map(|(to, from)| format!("{ind}        {to} <= {from};"))
                .collect::<Vec<_>>()
                .join("\n");
            match &reset {
                None => out.push(format!(
                    "{ind}always_ff @({edge} {}) begin\n{shift}\n{ind}end",
                    clock.name
                )),
                Some(cond) => {
                    let clear = links
                        .iter()
                        .map(|(to, _)| format!("{ind}        {to} <= '0;"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.push(format!(
                        "{ind}always_ff @({edge} {}) begin\n\
                         {ind}    if ({cond}) begin\n{clear}\n\
                         {ind}    end else begin\n{shift}\n\
                         {ind}    end\n\
                         {ind}end",
                        clock.name
                    ));
                }
            }
        }
        Some(out.join("\n"))
    }
}

/// Kontrat ifadesindeki `prev(...)` çağrıları, iç çağrılar önce
/// (iç içe `prev(prev(x))` dıştaki üretilirken içtekinin adı hazır olsun).
fn collect_prev_calls(ast: &volt_ast::SourceFile, e: Idx<Expr>, out: &mut Vec<Idx<Expr>>) {
    let children: Vec<Idx<Expr>> = match &ast.exprs[e].kind {
        ExprKind::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ExprKind::Unary { operand, .. } => vec![*operand],
        ExprKind::Cast { expr, .. } => vec![*expr],
        ExprKind::Index { base, index } => vec![*base, *index],
        ExprKind::Range { base, hi, lo } => vec![*base, *hi, *lo],
        ExprKind::PartSelect {
            base, start, width, ..
        } => vec![*base, *start, *width],
        ExprKind::Field { base, .. } => vec![*base],
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => vec![*cond, *then_expr, *else_expr],
        ExprKind::Call { callee, args } => {
            let mut v = vec![*callee];
            v.extend(args.iter().copied());
            v
        }
        _ => Vec::new(),
    };
    for c in children {
        collect_prev_calls(ast, c, out);
    }
    if let ExprKind::Call { callee, .. } = &ast.exprs[e].kind {
        if matches!(&ast.exprs[*callee].kind,
            ExprKind::Path(p) if p.segments.len() == 1 && p.segments[0].text == "prev")
        {
            out.push(e);
        }
    }
}
