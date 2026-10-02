//! Tele `on` bloğunda `<=` (E0020, ADR-0098 eki).
//!
//! `wire w : u8` ve `on clk { w <= d }` eskiden reset dalı boş bir flop
//! üretiyordu (`if (rst) begin end else w <= d`): sinyal saat kenarında
//! değer tutar ama başlangıç değeri yoktur, reset sonrası önceki değerini
//! korur. Tel yalnız `comb` bloğunda ya da `assign` ile sürülür
//! (sv-mapping.md §16.3); değer tutan sinyal reset değeriyle bir `reg`'dir.
//! E0019 ile aynı yerde, modül gövdesi ayrıştırıldıktan sonra denetlenir:
//! öneri bildirimin kaynak metnini yeniden yazar.

use std::collections::HashMap;

use volt_ast::{
    Block, BlockStmt, ElseBranch, Expr, Idx, IfStmt, LValue, MatchArmBody, Stmt, StmtKind,
    TypeRefKind, WireDecl,
};
use volt_diagnostics::{
    lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion,
};
use volt_span::Span;

use super::Parser;

/// Modül gövdesindeki bir `wire` bildirimi.
struct WireInfo<'w> {
    decl: &'w WireDecl,
    span: Span,
}

impl Parser<'_> {
    pub(super) fn check_wire_nonblocking(&mut self, body: &[Idx<Stmt>]) {
        let wires: HashMap<String, WireInfo<'_>> = body
            .iter()
            .filter_map(|&s| {
                let stmt = &self.ast.stmts[s];
                match &stmt.kind {
                    StmtKind::Wire(w) => Some((
                        w.name.text.clone(),
                        WireInfo {
                            decl: w,
                            span: stmt.span,
                        },
                    )),
                    _ => None,
                }
            })
            .collect();
        if wires.is_empty() {
            return;
        }
        let mut diags = Vec::new();
        for &s in body {
            if let StmtKind::On(on) = &self.ast.stmts[s].kind {
                self.check_wire_block(&wires, on.body, &mut Vec::new(), &mut diags);
            }
        }
        // Her atama kendi hatasıdır (kaskad değil): bastırma penceresi
        // uygulanmaz, gövde zaten ayrıştırıldı.
        self.diagnostics.extend(diags);
    }

    /// `on` gövdesi ve iç blokları; `shadow` blok `let`'iyle gölgelenen adlar.
    fn check_wire_block(
        &self,
        wires: &HashMap<String, WireInfo<'_>>,
        block: Idx<Block>,
        shadow: &mut Vec<String>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let outer = shadow.len();
        for stmt in &self.ast.blocks[block].stmts {
            match stmt {
                BlockStmt::NonBlockAssign { lhs, rhs, .. } => {
                    let name = &lhs.base.text;
                    if shadow.contains(name) {
                        continue;
                    }
                    if let Some(wire) = wires.get(name) {
                        diags.push(self.wire_nonblocking_diag(wire, lhs, *rhs));
                    }
                }
                // `on` içinde `=` zaten E0006'dır.
                BlockStmt::BlockAssign { .. } => {}
                BlockStmt::If(if_stmt) => self.check_wire_if(wires, if_stmt, shadow, diags),
                BlockStmt::Match(m) => {
                    for arm in &m.arms {
                        match &arm.body {
                            MatchArmBody::Block(b) => {
                                self.check_wire_block(wires, *b, shadow, diags)
                            }
                            MatchArmBody::Expr(_) => {}
                        }
                    }
                }
                BlockStmt::For(f) => self.check_wire_block(wires, f.body, shadow, diags),
                BlockStmt::Let(l) => shadow.push(l.name.text.clone()),
                BlockStmt::Error => {}
            }
        }
        shadow.truncate(outer);
    }

    fn check_wire_if(
        &self,
        wires: &HashMap<String, WireInfo<'_>>,
        if_stmt: &IfStmt,
        shadow: &mut Vec<String>,
        diags: &mut Vec<Diagnostic>,
    ) {
        self.check_wire_block(wires, if_stmt.then_block, shadow, diags);
        match &if_stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.check_wire_block(wires, *b, shadow, diags),
            Some(ElseBranch::If(elif)) => self.check_wire_if(wires, elif, shadow, diags),
            None => {}
        }
    }

    /// Reset değeri tek sözcükle yazılabilen tipler; diğerlerinde `None`.
    fn reset_literal(&self, wire: &WireDecl) -> Option<&'static str> {
        match &self.ast.types[wire.ty].kind {
            TypeRefKind::Bool => Some("false"),
            TypeRefKind::UInt(_)
            | TypeRefKind::SInt(_)
            | TypeRefKind::Bits(_)
            | TypeRefKind::UIntN(_)
            | TypeRefKind::SIntN(_) => Some("0"),
            TypeRefKind::Clock
            | TypeRefKind::Reset(_)
            | TypeRefKind::Trit
            | TypeRefKind::Array { .. }
            | TypeRefKind::Tuple(_)
            | TypeRefKind::Path { .. }
            | TypeRefKind::Error => None,
        }
    }

    /// E0020, 5 parça: kod, konum, açıklama, öneri, explain/ADR-0098.
    fn wire_nonblocking_diag(
        &self,
        wire: &WireInfo<'_>,
        lhs: &LValue,
        rhs: Idx<Expr>,
    ) -> Diagnostic {
        let name = &wire.decl.name.text;
        let stmt = Span {
            start: lhs.span.start,
            end: self.ast.exprs[rhs].span.end,
            ..lhs.span
        };
        let ty = self.text_of(self.ast.types[wire.decl.ty].span);
        let domain = wire
            .decl
            .domain
            .as_ref()
            .map(|d| format!(" @{}", d.text))
            .unwrap_or_default();
        let literal = self.reset_literal(wire.decl);
        let value = literal.unwrap_or("<reset value>");
        let form = format!("reg {name} : {ty}{domain} = {value}");
        let diag = Diagnostic::error(
            ErrorCode::E0020,
            lstr!(
                en: "wire '{name}' cannot be assigned with '<=' in an 'on' block";
                tr: "tel '{name}' bir 'on' bloğunda '<=' ile atanamaz"
            ),
            LabeledSpan::primary(
                stmt,
                lstr!(
                    en: "this would make '{name}' a flip-flop without a reset value";
                    tr: "bu, '{name}' telini reset değeri olmayan bir flop yapardı"
                ),
            ),
            lstr!(
                en: "a signal that holds its value between clock edges is a register: \
                     declare it as '{form}' (its value after reset); if the signal is \
                     combinational, assign it with '=' in a 'comb' block or at module level";
                tr: "saat kenarları arasında değerini tutan sinyal bir register'dır: \
                     '{form}' olarak bildirin (reset sonrası değeri); sinyal kombinasyonelse \
                     bir 'comb' bloğunda ya da modül düzeyinde '=' ile atayın"
            ),
        )
        .with_secondary(
            wire.span,
            lstr!(
                en: "'{name}' is declared as a wire here";
                tr: "'{name}' burada tel olarak bildirildi"
            ),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "a wire has no reset value: written in an 'on' block it keeps, after \
                     reset, whatever value it had before (unknown in hardware)";
                tr: "telin reset değeri yoktur: 'on' bloğunda yazılınca reset sonrasında \
                     önceki değerini korur (donanımda bilinmez)"
            ),
        );
        // Tipin sıfırı tek sözcük değilse yer tutuculu metin derlenmez.
        match literal {
            Some(_) => diag.with_suggestion(Suggestion {
                span: wire.span,
                replacement: form,
                applicability: Applicability::MaybeIncorrect,
            }),
            None => diag,
        }
    }
}
