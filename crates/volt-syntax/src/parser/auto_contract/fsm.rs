//! FSM tanıma ve kontratları (ADR-0066 F2/F3).
//!
//! Durum register'ı: `uN` register'ı; her yazması sabit bir değer
//! (`s <= 2`, `s <= IDLE`), en az iki farklı değer alır ve ilk saatin
//! `on` bloğunda `match s { literal => ..., _ => ... }` deyimi vardır.
//!
//! * F3 geçiş cover'ı: kol `a` içinde `s <= b` (b ≠ a) →
//!   `cover: prev(s) == a && s == b`; joker kolda kaynak "adı geçen
//!   hiçbir literal değil" olur. Sayı `MAX_TRANSITIONS`'ı aşarsa F3
//!   üretilmez (kombinasyonel patlama, ADR-0066 §2).
//! * F2 durum cover'ı: bir geçiş cover'ının hedefi OLMAYAN her yazılan
//!   değer için `cover: s == v` (geçiş cover'ı hedefe ulaşmayı zaten
//!   kapsar). Reset değeri kapsam dışı: 0. döngüde kendiliğinden tutar.
//! * F1 (geçerli durum) üretilmez: E0014 her match'te `_` kolunu zorunlu
//!   kılar ve tüm yazmalar sabit olduğundan değişmez tanım gereği tutar.

use std::collections::HashMap;

use volt_ast::{AutoRule, BinOp, ContractKind, Expr, Idx, SourceFile};

use super::super::mono::unroll::eval_const;
use super::gen::{Spec, G};
use super::scan::{ArmPat, RegInfo, RegMatch, Scan};

/// FSM başına en çok geçiş cover'ı; aşılırsa F3 yerine F2.
pub(super) const MAX_TRANSITIONS: usize = 16;
/// FSM başına en çok durum cover'ı; aşılırsa F2 üretilmez.
pub(super) const MAX_STATES: usize = 16;

const DEPTH0: u32 = 0;

pub(super) fn specs(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    scan: &Scan,
    reg: &RegInfo,
) -> Vec<Spec> {
    let Some(fsm) = recognize(ast, consts, scan, reg) else {
        return Vec::new();
    };
    let name = &reg.name;
    let first = &scan.matches[fsm.first_match];
    let subject = format!("match on {name}");
    let mut out = Vec::new();
    let mut targets = Vec::new();
    if fsm.transitions.len() <= MAX_TRANSITIONS {
        for t in &fsm.transitions {
            let m = &scan.matches[t.match_id];
            let Some((src, src_text)) = source_pred(m, t.arm, name) else {
                continue;
            };
            let dst = G::bin(BinOp::Eq, G::Name(name.clone()), G::Copy(t.target_expr));
            out.push(Spec {
                kind: ContractKind::Cover,
                rule: AutoRule::FsmTransition,
                expr: G::bin(BinOp::And, src, dst),
                subject: format!("{subject}, transition {src_text} -> {}", t.target),
                from: t.span,
            });
            targets.push(t.target);
        }
    }
    let states: Vec<&(i128, Idx<Expr>)> = fsm
        .values
        .iter()
        .filter(|(v, _)| *v != fsm.init && !targets.contains(v))
        .collect();
    if states.len() <= MAX_STATES {
        for &&(_, e) in &states {
            out.push(Spec {
                kind: ContractKind::Cover,
                rule: AutoRule::FsmState,
                expr: G::bin(BinOp::Eq, G::Name(name.clone()), G::Copy(e)),
                subject: subject.clone(),
                from: first.span,
            });
        }
    }
    out
}

struct Transition {
    match_id: usize,
    arm: usize,
    target: i128,
    target_expr: Idx<Expr>,
    span: volt_span::Span,
}

struct Fsm {
    init: i128,
    /// Yazılan değerler (kaynak sırası) ve temsilci ifadesi.
    values: Vec<(i128, Idx<Expr>)>,
    transitions: Vec<Transition>,
    first_match: usize,
}

fn recognize(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    scan: &Scan,
    reg: &RegInfo,
) -> Option<Fsm> {
    let name = &reg.name;
    let writes = scan.writes.get(name)?;
    let max = super::max_value(reg.width);
    let in_range = |v: i128| (0..=max).contains(&v);
    let init = eval_const(ast, consts, reg.init, DEPTH0).filter(|&v| in_range(v))?;
    let first_match = scan
        .matches
        .iter()
        .position(|m| &m.scrutinee == name && m.arms.is_some())?;
    let mut values: Vec<(i128, Idx<Expr>)> = Vec::new();
    let mut transitions: Vec<Transition> = Vec::new();
    for w in writes {
        if !super::is_const_expr(ast, &scan.locals, w.rhs) {
            return None;
        }
        let v = eval_const(ast, consts, w.rhs, DEPTH0).filter(|&v| in_range(v))?;
        if !values.iter().any(|(x, _)| *x == v) {
            values.push((v, w.rhs));
        }
        let Some(ctx) = w
            .arms
            .iter()
            .rev()
            .find(|a| a.scrutinee.as_deref() == Some(name.as_str()))
        else {
            continue;
        };
        let m = &scan.matches[ctx.match_id];
        let Some(arms) = &m.arms else { continue };
        let is_self = match &arms[ctx.arm] {
            ArmPat::Values(vs) => vs.iter().any(|(x, _)| *x == v),
            // Joker kolun kaynağı adı geçmeyen değerlerdir; hedef de adı
            // geçmeyen bir değerse bu joker içi bir döngü olabilir.
            ArmPat::Wildcard => !m.literal_values().iter().any(|(x, _)| *x == v),
        };
        let dup = transitions
            .iter()
            .any(|t| t.match_id == ctx.match_id && t.arm == ctx.arm && t.target == v);
        if !is_self && !dup {
            transitions.push(Transition {
                match_id: ctx.match_id,
                arm: ctx.arm,
                target: v,
                target_expr: w.rhs,
                span: w.span,
            });
        }
    }
    let distinct = values.iter().filter(|(v, _)| *v != init).count() + 1;
    (distinct >= 2).then_some(Fsm {
        init,
        values,
        transitions,
        first_match,
    })
}

/// Geçişin kaynak koşulu ve kısa yazımı (`1`, `1 | 2`, `_`).
fn source_pred(m: &RegMatch, arm: usize, name: &str) -> Option<(G, String)> {
    let prev_eq = |e: Idx<Expr>| G::bin(BinOp::Eq, G::Prev(name.to_string()), G::Copy(e));
    let prev_ne = |e: Idx<Expr>| G::bin(BinOp::Ne, G::Prev(name.to_string()), G::Copy(e));
    match &m.arms.as_ref()?[arm] {
        ArmPat::Values(vs) => {
            let text = vs
                .iter()
                .map(|(v, _)| v.to_string())
                .collect::<Vec<_>>()
                .join(" | ");
            Some((fold(BinOp::Or, vs.iter().map(|&(_, e)| prev_eq(e)))?, text))
        }
        ArmPat::Wildcard => {
            let lits = m.literal_values();
            Some((
                fold(BinOp::And, lits.iter().map(|&(_, e)| prev_ne(e)))?,
                "_".to_string(),
            ))
        }
    }
}

/// Soldan birleşen zincir; boş girdide None (yalnız jokerli match).
fn fold(op: BinOp, items: impl Iterator<Item = G>) -> Option<G> {
    let mut acc: Option<G> = None;
    for g in items {
        acc = Some(match acc {
            None => g,
            Some(a) => G::bin(op, a, g),
        });
    }
    acc
}
