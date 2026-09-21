//! K4 — register domain'i: açık `reg(clk)` ya da 'on' bloğu
//! yazıcılarından çıkarım; iki saatten yazma E3011, hiç yazılmama W3001.

use std::collections::HashMap;

use volt_ast::{
    Block, BlockStmt, ElseBranch, Idx, IfStmt, MatchArmBody, ModuleDecl, OnTrigger, RegDecl,
    StmtKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::{DomainId, Inferencer};
use crate::resolve::{DefId, DefKind};

impl<'a> Inferencer<'a> {
    pub(super) fn assign_reg_domains(&mut self, m: &ModuleDecl) {
        let inferred = self.assign_annotated_regs(m);
        if inferred.is_empty() {
            return;
        }
        let writers = self.scan_writers(m);
        for (def, r) in inferred {
            let blocks = writers.get(&def).cloned().unwrap_or_default();
            let dom = self.reg_domain_from_writers(r, &blocks);
            self.signal_domains.insert(def, dom);
        }
    }

    /// Açık `reg(clk)` anotasyonları önce (K1); anotasyonsuzlar döner.
    fn assign_annotated_regs(&mut self, m: &ModuleDecl) -> Vec<(DefId, &'a RegDecl)> {
        let ast = self.ast;
        let mut inferred: Vec<(DefId, &RegDecl)> = Vec::new();
        for &stmt in &m.body {
            let StmtKind::Reg(r) = &ast.stmts[stmt].kind else {
                continue;
            };
            let Some(def) = self.decl_def(&r.name) else {
                continue;
            };
            match &r.domain {
                Some(ann) => {
                    let dom = self.signal_annotation_domain(&ann.clone());
                    self.signal_domains.insert(def, dom);
                }
                None => inferred.push((def, r)),
            }
        }
        inferred
    }

    /// Yazıcı taraması: her 'on' bloğunun domain'i + yazdığı tanımlar.
    fn scan_writers(&mut self, m: &ModuleDecl) -> HashMap<DefId, Vec<(DomainId, Span)>> {
        let mut writers: HashMap<DefId, Vec<(DomainId, Span)>> = HashMap::new();
        for &stmt in &m.body {
            let StmtKind::On(on) = &self.ast.stmts[stmt].kind else {
                continue;
            };
            let (dom, trigger_span) = self.on_block_domain(&on.trigger);
            let mut written = Vec::new();
            self.collect_writes(on.body, &mut written);
            for def in written {
                writers.entry(def).or_default().push((dom, trigger_span));
            }
        }
        writers
    }

    /// Anotasyonsuz register'ın alanı yazıcı bloklarından çıkar (K4).
    fn reg_domain_from_writers(&mut self, r: &RegDecl, blocks: &[(DomainId, Span)]) -> DomainId {
        if blocks.is_empty() {
            // Hiç yazılmıyor → sabit gibi (W3001).
            self.warn_never_written(r);
            return DomainId::Timeless;
        }
        let mut distinct: Vec<(u32, Span)> = Vec::new();
        for (dom, span) in blocks {
            if let DomainId::Explicit(id) = self.resolve_dom(*dom) {
                if !distinct.iter().any(|&(d, _)| d == id) {
                    distinct.push((id, *span));
                }
            }
        }
        match distinct.len() {
            0 => DomainId::Error, // yazıcılar hatalı — kaskad bastır
            1 => DomainId::Explicit(distinct[0].0),
            // ÇOK CİDDİ HATA: aynı register iki saatten yazılıyor.
            _ => {
                self.err_two_domains(r, &distinct);
                DomainId::Error
            }
        }
    }

    /// W3001 — register hiçbir 'on' bloğunda yazılmıyor.
    fn warn_never_written(&mut self, r: &RegDecl) {
        self.diagnostics.push(
            Diagnostic::warning(
                ErrorCode::W3001,
                lstr!(
                    en: "register is never written in any 'on' block: '{}'",
                        r.name.text;
                    tr: "register hiçbir 'on' bloğunda yazılmıyor: '{}'",
                        r.name.text
                ),
                LabeledSpan::primary(
                    r.name.span,
                    lstr!(
                        en: "no sequential assignment to this register";
                        tr: "bu register'a sıralı atama yok"
                    ),
                ),
                lstr!(
                    en: "write it with '<=' in an 'on <clock>' block \
                         or use 'let' if it is a constant";
                    tr: "bir 'on <saat>' bloğunda '<=' ile yazın \
                         ya da sabitse 'let' kullanın"
                ),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(
                    en: "a register that is never written produces a constant value";
                    tr: "yazılmayan register sabit bir değer üretir"
                ),
            ),
        );
    }

    /// K4 — E3011, 5 parça: kod, konum, neden, çözüm, spec referansı.
    fn err_two_domains(&mut self, r: &RegDecl, distinct: &[(u32, Span)]) {
        let names: Vec<String> = distinct
            .iter()
            .map(|(id, _)| format!("@{}", self.domain_name(*id)))
            .collect();
        let diag = Diagnostic::error(
            ErrorCode::E3011,
            lstr!(
                en: "register is written from more than one clock domain";
                tr: "register birden fazla saat alanından yazılıyor"
            ),
            LabeledSpan::primary(
                r.name.span,
                lstr!(
                    en: "'{}' is written from these domains: {}",
                        r.name.text, names.join(", ");
                    tr: "'{}' şu alanlardan yazılıyor: {}",
                        r.name.text, names.join(", ")
                ),
            ),
            lstr!(
                en: "each register must belong to a single clock domain — \
                     split the register or feed it from one domain with sync()";
                tr: "her register tek bir saat alanına ait olmalı — \
                     register'ı bölün ya da sync() ile tek alandan besleyin"
            ),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "a register written from two clocks cannot be synthesized \
                     in hardware; it is ambiguous which edge wins";
                tr: "iki saatten yazılan register donanımda sentezlenemez; \
                     hangi kenarın kazanacağı belirsizdir"
            ),
        );
        let diag = self.with_writer_labels(diag, distinct);
        self.diagnostics.push(diag);
    }

    /// E3011 ikincil etiketleri: önce yazıcı bloklar, sonra alan tanımları.
    fn with_writer_labels(&self, mut diag: Diagnostic, distinct: &[(u32, Span)]) -> Diagnostic {
        for (id, span) in distinct {
            diag = diag.with_secondary(
                *span,
                lstr!(
                    en: "@{} writes from here", self.domain_name(*id);
                    tr: "@{} buradan yazıyor", self.domain_name(*id)
                ),
            );
        }
        for (id, _) in distinct {
            diag = diag.with_secondary(self.domain_span(*id), self.defined_here(*id));
        }
        diag
    }

    /// 'on' bloğunun domain'i tetikleyici saatten gelir.
    pub(super) fn on_block_domain(&mut self, trigger: &OnTrigger) -> (DomainId, Span) {
        match trigger {
            OnTrigger::Clock(name) | OnTrigger::Reset(name) => {
                let dom = self
                    .use_def(name.span)
                    .and_then(|def| self.signal_domains.get(&def).copied())
                    .unwrap_or(DomainId::Error);
                (dom, name.span)
            }
            OnTrigger::Error => (
                DomainId::Error,
                Span::new(volt_span::FileId(u32::MAX), 0, 0),
            ),
        }
    }

    /// Blok içinde sıralı yazılan tanımları toplar (K4 yazıcı taraması).
    fn collect_writes(&self, block_idx: Idx<Block>, out: &mut Vec<DefId>) {
        let block = &self.ast.blocks[block_idx];
        for stmt in &block.stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, .. } | BlockStmt::BlockAssign { lhs, .. } => {
                    if let Some(def) = self.use_def(lhs.base.span) {
                        if self.res.def_kind(def) == DefKind::Register {
                            out.push(def);
                        }
                    }
                }
                BlockStmt::If(if_stmt) => self.collect_writes_if(if_stmt, out),
                BlockStmt::Match(mt) => {
                    for arm in &mt.arms {
                        if let MatchArmBody::Block(b) = &arm.body {
                            self.collect_writes(*b, out);
                        }
                    }
                }
                BlockStmt::For(f) => self.collect_writes(f.body, out),
                BlockStmt::Let(_) | BlockStmt::Error => {}
            }
        }
    }

    fn collect_writes_if(&self, if_stmt: &IfStmt, out: &mut Vec<DefId>) {
        self.collect_writes(if_stmt.then_block, out);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.collect_writes(*b, out),
            Some(ElseBranch::If(nested)) => self.collect_writes_if(nested, out),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};
    use super::super::DomainId;

    #[test]
    fn register_takes_the_domain_of_its_writer_block() {
        let r = inferred(&two_clock(
            "    in d : u8 @Slow\n    reg r : u8 = 0\n    on slow_clk {\n        r <= d\n    }",
        ));
        assert_eq!(r.domain_name_of("r"), Some("Slow"));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn writes_nested_in_if_and_match_are_found() {
        let r = inferred(&two_clock(
            "    in c : bool @Fast\n    in s : u2 @Fast\n    reg a : u8 = 0\n    reg b : u8 = 0\n    \
             on fast_clk {\n        if c {\n            a <= 1\n        } else {\n            \
             match s {\n                0 => { b <= 2 }\n                _ => { b <= 3 }\n            }\n        }\n    }",
        ));
        assert_eq!(r.domain_name_of("a"), Some("Fast"));
        assert_eq!(r.domain_name_of("b"), Some("Fast"));
    }

    #[test]
    fn register_written_from_two_clocks_is_e3011() {
        let r = inferred(&two_clock(
            "    reg r : u8 = 0\n    on fast_clk {\n        r <= 1\n    }\n    on slow_clk {\n        r <= 2\n    }",
        ));
        assert_eq!(r.count("E3011"), 1, "{:?}", r.codes());
        assert_eq!(r.domain_of("r"), DomainId::Error);
        let d = r
            .dom
            .diagnostics
            .iter()
            .find(|d| d.code.as_str() == "E3011")
            .unwrap();
        // Birincil + iki yazıcı + iki tanım satırı, bu sırayla.
        assert_eq!(d.spans.len(), 5);
        assert!(d.spans[1].label.contains("@Fast") && d.spans[2].label.contains("@Slow"));
        assert!(d.spans[3].label.contains("@Fast") && d.spans[4].label.contains("@Slow"));
    }

    #[test]
    fn never_written_register_is_timeless_with_w3001() {
        let r = inferred(
            "module M {\n    in clk : clock\n    out q : u8\n    reg r : u8 = 7\n    q = r\n}\n",
        );
        assert_eq!(r.codes(), ["W3001"]);
        assert_eq!(r.domain_of("r"), DomainId::Timeless);
    }

    #[test]
    fn two_blocks_of_the_same_clock_are_one_domain() {
        let r = inferred(
            "module M {\n    in clk : clock\n    reg r : u8 = 0\n    on clk {\n        r <= 1\n    }\n    \
             on clk {\n        r <= 2\n    }\n}\n",
        );
        assert_eq!(r.count("E3011"), 0, "{:?}", r.codes());
        assert_eq!(r.domain_name_of("r"), Some("clk"));
    }
}
