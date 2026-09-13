//! Üretim (generate) yapıları — ADR-0041: `for` açma, `wire`, `comb`.
//!
//! `for i in a..b { ... }` derleme zamanı döngüsüdür (const-eval.md §8):
//! sınırlar sabit olmalı; gövde her iterasyon için AÇILIR, döngü
//! değişkeni `loop_vars` üzerinden sabit olarak ikame edilir (SV
//! `generate` bloğu üretilmez — çıktı düz ve araç-bağımsızdır).
//! `comb { }` → `always_comb`, `wire x : T` → `logic` bildirimi.

use volt_ast::{Block, BlockStmt, ForStmt, Idx};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use crate::Emitter;

/// Açılabilecek en fazla iterasyon — sonsuz/devasa açılıma karşı.
const MAX_UNROLL: i128 = 4096;

impl<'a> Emitter<'a> {
    /// Döngü sınırları `[start, end)`; sabit değilse ya da çok genişse
    /// E2005 ile None.
    fn for_bounds(&mut self, f: &'a ForStmt, span: Span) -> Option<(i128, i128)> {
        let (start, end) = (self.eval_const(f.start), self.eval_const(f.end));
        let (Some(s), Some(e)) = (start, end) else {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "the bounds of 'for {}' must be compile-time constants", f.var.text;
                    tr: "'for {}' sınırları derleme zamanı sabiti olmalı", f.var.text
                ),
                span,
                &lstr!(
                    en: "use literals, const items or generic parameters: for i in 0..N";
                    tr: "literal, const ya da generic parametre kullanın: for i in 0..N"
                ),
            );
            return None;
        };
        if e - s > MAX_UNROLL {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "'for {}' unrolls {} iterations, the limit is {MAX_UNROLL}", f.var.text, e - s;
                    tr: "'for {}' {} iterasyona açılıyor, sınır {MAX_UNROLL}", f.var.text, e - s
                ),
                span,
                &lstr!(en: "narrow the range"; tr: "aralığı daraltın"),
            );
            return None;
        }
        Some((s, e))
    }

    /// Blok içi `for` (on/comb/stage gövdesi): gövde satırları iterasyon
    /// başına tekrar üretilir.
    pub(crate) fn emit_for_in_block(
        &mut self,
        f: &'a ForStmt,
        indent: usize,
        lines: &mut Vec<String>,
    ) {
        let span = self.ast.blocks[f.body].span;
        let Some((s, e)) = self.for_bounds(f, span) else {
            return;
        };
        for v in s..e {
            self.loop_vars.push((f.var.text.clone(), v));
            lines.extend(self.emit_block(f.body, indent));
            self.loop_vars.pop();
        }
    }

    /// Modül seviyesi `for`: gövdesi kombinasyonel; her iterasyondaki
    /// `x = y` ataması `assign` satırı olur. Boş aralık → None.
    pub(crate) fn emit_module_for(&mut self, f: &'a ForStmt, span: Span) -> Option<String> {
        let (s, e) = self.for_bounds(f, span)?;
        let mut lines = Vec::new();
        for v in s..e {
            self.loop_vars.push((f.var.text.clone(), v));
            self.emit_for_body_assigns(f.body, &mut lines);
            self.loop_vars.pop();
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    fn emit_for_body_assigns(&mut self, body: Idx<Block>, lines: &mut Vec<String>) {
        let ast = self.ast;
        for stmt in &ast.blocks[body].stmts {
            match stmt {
                BlockStmt::BlockAssign { lhs, rhs, .. } => {
                    let sig = self.lvalue_sig(lhs);
                    let l = self.emit_lvalue(lhs);
                    let r = self.emit_assigned(*rhs, sig);
                    lines.push(format!("    assign {l} = {r};"));
                }
                BlockStmt::For(inner) => {
                    let span = ast.blocks[inner.body].span;
                    if let Some(chunk) = self.emit_module_for(inner, span) {
                        lines.push(chunk);
                    }
                }
                BlockStmt::Error => {}
                BlockStmt::NonBlockAssign { span, .. } => self.future(
                    *span,
                    &lstr!(
                        en: "non-blocking assignment in a module-level 'for'";
                        tr: "modül seviyesi 'for' içinde ardışık atama"
                    ),
                ),
                BlockStmt::If(_) | BlockStmt::Match(_) | BlockStmt::Let(_) => self.future(
                    ast.blocks[body].span,
                    &lstr!(
                        en: "if/match/let inside a module-level 'for' (move them into a 'comb' block)";
                        tr: "modül seviyesi 'for' içinde if/match/let (bir 'comb' bloğuna taşıyın)"
                    ),
                ),
            }
        }
    }

    /// `comb { ... }` → `always_comb begin ... end`.
    pub(crate) fn emit_comb(&mut self, block: Idx<Block>) -> String {
        let mut out = String::from("    always_comb begin\n");
        for line in self.emit_block(block, 8) {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("    end");
        out
    }
}
