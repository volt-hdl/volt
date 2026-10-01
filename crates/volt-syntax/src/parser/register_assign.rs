//! Register'a `on` bloğu dışında `=` (E0019, ADR-0098).
//!
//! `reg r : u8 = 3` ve modül düzeyinde `r = r + 1` eskiden
//! `assign r = r + 8'd1;` üretiyordu: register'ın saati ve reset dalı yok
//! (başlangıç değeri kayıp), değer kendisini okuyunca kombinasyonel döngü.
//! Aynısı `comb` bloğunda ve modül düzeyi `for` gövdesinde de olur.
//! Register yalnız bir `on` bloğunda `<=` ile yazılır (sv-mapping.md §3-4);
//! E0007'nin aynası olarak burada, modül gövdesi ayrıştırıldıktan sonra
//! denetlenir. Blok `let`'i aynı adı gölgelerse o ad artık register değildir.

use std::collections::HashMap;

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, Idx, IfStmt, LValue, MatchArmBody, Port, Stmt, StmtKind,
    TypeRefKind,
};
use volt_diagnostics::{
    lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion,
};
use volt_span::Span;

use super::Parser;

/// Modülün register'ları ve `on` önerisinde yazılacak saatleri.
pub(super) struct RegClocks {
    /// Saat tipli portlar, bildirim sırasıyla.
    clocks: Vec<String>,
    /// reg adı → `reg(clk)` ile açıkça yazılan saat.
    regs: HashMap<String, Option<String>>,
}

impl RegClocks {
    /// Register'ın `on` önerisindeki saati: açık `reg(clk)`, yoksa tek saat
    /// portu; ikisi de yoksa `None` (öneride `<clock>` yazılır). Register
    /// değilse `None`.
    pub(super) fn clock_of(&self, name: &str) -> Option<Option<String>> {
        let explicit = self.regs.get(name)?;
        Some(match (explicit, self.clocks.as_slice()) {
            (Some(clk), _) => Some(clk.clone()),
            (None, [only]) => Some(only.clone()),
            (None, _) => None,
        })
    }
}

/// Atamanın bulunduğu yer: yalnız modül düzeyindeki deyim olduğu gibi
/// bir `on` bloğuyla değiştirilebilir.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Site {
    Module,
    Block,
}

impl Parser<'_> {
    pub(super) fn reg_clocks(&self, ports: &[Port], body: &[Idx<Stmt>]) -> RegClocks {
        let clocks = ports
            .iter()
            .filter(|p| matches!(self.ast.types[p.ty].kind, TypeRefKind::Clock))
            .map(|p| p.name.text.clone())
            .collect();
        let regs = body
            .iter()
            .filter_map(|&s| match &self.ast.stmts[s].kind {
                StmtKind::Reg(r) => Some((
                    r.name.text.clone(),
                    r.domain.as_ref().map(|d| d.text.clone()),
                )),
                _ => None,
            })
            .collect();
        RegClocks { clocks, regs }
    }

    pub(super) fn check_register_assigns(&mut self, regs: &RegClocks, body: &[Idx<Stmt>]) {
        if regs.regs.is_empty() {
            return;
        }
        let mut diags = Vec::new();
        for &s in body {
            match &self.ast.stmts[s].kind {
                StmtKind::Assign(a) => {
                    self.check_reg_target(regs, &[], &a.lhs, a.rhs, Site::Module, &mut diags)
                }
                StmtKind::Comb(b) => self.check_reg_block(regs, *b, &mut Vec::new(), &mut diags),
                StmtKind::For(f) => self.check_reg_block(regs, f.body, &mut Vec::new(), &mut diags),
                // `on` içinde register'a `=` ya da `<=` serbesttir; bildirimler
                // ve örnekler atama içermez; ifade deyimi E0007/E0001 alır.
                StmtKind::On(_)
                | StmtKind::Reg(_)
                | StmtKind::Let(_)
                | StmtKind::Wire(_)
                | StmtKind::Instance(_)
                | StmtKind::Expr(_)
                | StmtKind::Error => {}
            }
        }
        // Her atama kendi hatasıdır (kaskad değil): bastırma penceresi
        // uygulanmaz, gövde zaten ayrıştırıldı.
        self.diagnostics.extend(diags);
    }

    /// `comb` bloğu ya da modül düzeyi `for` gövdesi; `shadow` bu kapsamda
    /// `let` ile gölgelenen adlar.
    fn check_reg_block(
        &self,
        regs: &RegClocks,
        block: Idx<Block>,
        shadow: &mut Vec<String>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let outer = shadow.len();
        for stmt in &self.ast.blocks[block].stmts {
            match stmt {
                BlockStmt::BlockAssign { lhs, rhs, .. } => {
                    self.check_reg_target(regs, shadow, lhs, *rhs, Site::Block, diags)
                }
                // Blok dışında `<=` zaten E0007'dir.
                BlockStmt::NonBlockAssign { .. } => {}
                BlockStmt::If(if_stmt) => self.check_reg_if(regs, if_stmt, shadow, diags),
                BlockStmt::Match(m) => {
                    for arm in &m.arms {
                        match &arm.body {
                            MatchArmBody::Block(b) => self.check_reg_block(regs, *b, shadow, diags),
                            MatchArmBody::Expr(_) => {}
                        }
                    }
                }
                BlockStmt::For(f) => self.check_reg_block(regs, f.body, shadow, diags),
                BlockStmt::Let(l) => shadow.push(l.name.text.clone()),
                BlockStmt::Error => {}
            }
        }
        shadow.truncate(outer);
    }

    fn check_reg_if(
        &self,
        regs: &RegClocks,
        if_stmt: &IfStmt,
        shadow: &mut Vec<String>,
        diags: &mut Vec<Diagnostic>,
    ) {
        self.check_reg_block(regs, if_stmt.then_block, shadow, diags);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.check_reg_block(regs, *b, shadow, diags),
            Some(ElseBranch::If(elif)) => self.check_reg_if(regs, elif, shadow, diags),
            None => {}
        }
    }

    fn check_reg_target(
        &self,
        regs: &RegClocks,
        shadow: &[String],
        lhs: &LValue,
        rhs: Idx<Expr>,
        site: Site,
        diags: &mut Vec<Diagnostic>,
    ) {
        let name = &lhs.base.text;
        if shadow.contains(name) {
            return;
        }
        if let Some(clock) = regs.clock_of(name) {
            diags.push(self.register_assign_diag(lhs, rhs, clock, site));
        }
    }

    /// E0019, 5 parça: kod, konum, açıklama, öneri, explain/ADR-0098.
    fn register_assign_diag(
        &self,
        lhs: &LValue,
        rhs: Idx<Expr>,
        clock: Option<String>,
        site: Site,
    ) -> Diagnostic {
        let name = &lhs.base.text;
        let rhs_span = self.ast.exprs[rhs].span;
        let stmt = Span {
            start: lhs.span.start,
            end: rhs_span.end,
            ..lhs.span
        };
        let target = self.text_of(lhs.span);
        let value = self.text_of(rhs_span);
        let clk = clock.as_deref().unwrap_or("<clock>");
        let form = format!("on {clk} {{ {target} <= {value} }}");
        let diag = Diagnostic::error(
            ErrorCode::E0019,
            lstr!(
                en: "register '{name}' cannot be assigned with '=' outside an 'on' block";
                tr: "register '{name}' 'on' bloğu dışında '=' ile atanamaz"
            ),
            LabeledSpan::primary(
                stmt,
                lstr!(
                    en: "this drives the register combinationally";
                    tr: "bu, register'ı kombinasyonel olarak sürer"
                ),
            ),
            lstr!(
                en: "a register takes its next value in an 'on' block: {form}; \
                     if the signal is combinational, declare it with 'wire' instead of 'reg'";
                tr: "register sonraki değerini bir 'on' bloğunda alır: {form}; \
                     sinyal kombinasyonelse 'reg' yerine 'wire' ile bildirin"
            ),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "outside an 'on' block '=' is a continuous assignment: the register gets \
                     no clock edge and loses its reset value, and a value that reads '{name}' \
                     feeds back into it (a combinational loop)";
                tr: "'on' bloğu dışında '=' sürekli bir atamadır: register saat kenarı almaz, \
                     reset değerini kaybeder; değer '{name}' register'ını okuyorsa çıkış \
                     girişine geri beslenir (kombinasyonel döngü)"
            ),
        );
        // Yalnız modül düzeyindeki deyim tek başına `on` bloğuna dönüşür;
        // saat bilinmiyorsa yer tutuculu metin derlenmez.
        match (site, clock) {
            (Site::Module, Some(_)) => diag.with_suggestion(Suggestion {
                span: stmt,
                replacement: form,
                applicability: Applicability::MaybeIncorrect,
            }),
            _ => diag,
        }
    }
}
