//! Üretim (generate) yapıları — ADR-0041: blok içi `for` açma, `comb`.
//!
//! `for i in a..b { ... }` derleme zamanı döngüsüdür (const-eval.md §8):
//! sınırlar sabit olmalı; gövde her iterasyon için AÇILIR, döngü
//! değişkeni `loop_vars` üzerinden sabit olarak ikame edilir (SV
//! `generate` bloğu üretilmez — çıktı düz ve araç-bağımsızdır).
//! Yalnız `on`/`comb`/`stage` gövdesindeki `for` buraya gelir; modül
//! seviyesi `for` parser'da açılır (ADR-0056, volt-syntax
//! parser/mono/unroll.rs). `comb { }` → `always_comb`.

use volt_ast::{Block, ForStmt, Idx};
use volt_diagnostics::{lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, Suggestion};
use volt_span::Span;

use crate::Emitter;

/// Açılabilecek en fazla iterasyon — sonsuz/devasa açılıma karşı.
const MAX_UNROLL: i128 = 4096;

impl<'a> Emitter<'a> {
    /// Döngü sınırları `[start, end)`; modül seviyesi açılımla aynı
    /// kodlar: sabit değilse E2021, ters aralık E2028, sınır üstü E2027.
    fn for_bounds(&mut self, f: &'a ForStmt, span: Span) -> Option<(i128, i128)> {
        let (start, end) = (self.eval_const(f.start), self.eval_const(f.end));
        let (Some(s), Some(e)) = (start, end) else {
            self.error(
                ErrorCode::E2021,
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
        if e < s {
            let diag = Diagnostic::error(
                ErrorCode::E2028,
                lstr!(
                    en: "'for {}' range is reversed: {s}..{e}", f.var.text;
                    tr: "'for {}' aralığı ters: {s}..{e}", f.var.text
                ),
                LabeledSpan::primary(span, ""),
                lstr!(
                    en: "write the smaller bound first: for {} in {e}..{s}", f.var.text;
                    tr: "küçük sınırı önce yazın: for {} in {e}..{s}", f.var.text
                ),
            )
            // suggestion: e2028_block_for
            .with_suggestion(
                Suggestion::replace(
                    self.ast.exprs[f.start].span,
                    e.to_string(),
                    Applicability::MaybeIncorrect,
                )
                .and_replace(self.ast.exprs[f.end].span, s.to_string()),
            );
            if !self.diagnostics.contains(&diag) {
                self.diagnostics.push(diag);
            }
            return None;
        }
        if e - s > MAX_UNROLL {
            self.error(
                ErrorCode::E2027,
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

    /// `comb { ... }` → `always_comb begin ... end`; blok `let`'i varsa
    /// adlı blok + süreç yerelleri (ADR-0083 Karar 11).
    pub(crate) fn emit_comb(&mut self, block: Idx<Block>, index: usize) -> String {
        let outer = self.begin_process();
        let body = self.emit_block(block, 8);
        let mut out = match self.end_process(outer, "comb", index, 8) {
            Some((label, decls)) => {
                let mut s = format!("    always_comb begin : {label}\n");
                for d in decls {
                    s.push_str(&d);
                    s.push('\n');
                }
                s
            }
            None => String::from("    always_comb begin\n"),
        };
        for line in body {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("    end");
        out
    }
}
