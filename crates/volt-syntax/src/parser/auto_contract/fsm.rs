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
//! * F3 joker kaynağı erişilebilir olmalı: sayısal FSM'de adı geçen
//!   literaller yazılan her değeri (ve reset değerini) kapsıyorsa joker
//!   kol erişilemezdir ve geçiş cover'ı ÜRETİLMEZ (cover kipinde yanlış
//!   E5001 — ADR-0074 yan bulgu 2). Enum FSM'de joker kaynağı adı
//!   geçmeyen varyantların kümesidir; boşsa üretilmez (ADR-0074 Karar 6).
//! * F1 (geçerli durum, ADR-0074): durum register'ı enum tipliyse ve her
//!   kod bir varyant değilse (`n < 2^W`) `invariant: s == A || s == B ...`.
//!   Tümevarımsaldır: bütün yazmalar varyant sabiti, `uN as Enum` yasak.
//!   Sayısal FSM'de üretilmez — geçerli değer kümesi dilde yok.

use std::collections::HashMap;

use volt_ast::{AutoRule, BinOp, ContractKind, Expr, Idx, SourceFile};

use super::gen::{Spec, G};
use super::scan::{ArmPat, Lit, RegInfo, RegMatch, Scan};
use super::value_of;

/// FSM başına en çok geçiş cover'ı; aşılırsa F3 yerine F2.
pub(super) const MAX_TRANSITIONS: usize = 16;
/// FSM başına en çok durum cover'ı; aşılırsa F2 üretilmez.
pub(super) const MAX_STATES: usize = 16;

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
            let Some((src, src_text)) = source_pred(m, t.arm, reg, &fsm) else {
                continue;
            };
            let dst = G::bin(BinOp::Eq, G::Name(name.clone()), G::Copy(t.target_expr));
            out.push(Spec {
                kind: ContractKind::Cover,
                rule: AutoRule::FsmTransition,
                expr: G::bin(BinOp::And, src, dst),
                subject: format!(
                    "{subject}, transition {src_text} -> {}",
                    show(reg, t.target)
                ),
                from: t.span,
            });
            targets.push(t.target);
        }
    }
    if let Some(e) = reg.enum_info.as_ref().filter(|e| !e.dense) {
        let valid = e.variants.iter().map(|(v, _)| {
            G::bin(
                BinOp::Eq,
                G::Name(name.clone()),
                G::Path(vec![e.name.clone(), v.clone()]),
            )
        });
        if let Some(expr) = fold(BinOp::Or, valid) {
            out.push(Spec {
                kind: ContractKind::Invariant,
                rule: AutoRule::FsmValid,
                expr,
                subject: format!("enum {}", e.name),
                from: first.span,
            });
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
    let init = value_of(ast, consts, reg.init).filter(|&v| in_range(v))?;
    let first_match = scan
        .matches
        .iter()
        .position(|m| &m.scrutinee == name && m.arms.is_some())?;
    // Desenler register'ın tipine uymalı (enum'da yalnız kendi varyantları,
    // sayıda yalnız literaller); uymayan tip hatasıdır (E2003) ve
    // kontrat ikinci bir tanı üretmemeli.
    let fits = |lit: &Lit| match (lit, &reg.enum_info) {
        (Lit::Expr(_), None) => true,
        (Lit::Path(p), Some(e)) => p.first() == Some(&e.name),
        _ => false,
    };
    let consistent = scan
        .matches
        .iter()
        .filter(|m| &m.scrutinee == name)
        .flat_map(|m| m.arms.iter().flatten())
        .all(|arm| match arm {
            ArmPat::Values(vs) => vs.iter().all(|(_, l)| fits(l)),
            ArmPat::Wildcard => true,
        });
    if !consistent {
        return None;
    }
    let mut values: Vec<(i128, Idx<Expr>)> = Vec::new();
    let mut transitions: Vec<Transition> = Vec::new();
    for w in writes {
        if !super::is_const_expr(ast, &scan.locals, w.rhs) {
            return None;
        }
        let v = value_of(ast, consts, w.rhs).filter(|&v| in_range(v))?;
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

/// Değerin rapor yazımı: enum'da varyant yolu, sayıda sayı.
fn show(reg: &RegInfo, v: i128) -> String {
    reg.enum_info
        .as_ref()
        .and_then(|e| e.variants.iter().find(|(_, c)| *c == v))
        .map_or_else(
            || v.to_string(),
            |(n, _)| {
                format!(
                    "{}::{n}",
                    reg.enum_info.as_ref().map_or("", |e| e.name.as_str())
                )
            },
        )
}

/// Geçişin kaynak koşulu ve kısa yazımı (`1`, `1 | 2`, `_`). Joker
/// kolun kaynağı erişilemezse `None` (cover üretilmez).
fn source_pred(m: &RegMatch, arm: usize, reg: &RegInfo, fsm: &Fsm) -> Option<(G, String)> {
    let name = &reg.name;
    let prev_eq = |g: G| G::bin(BinOp::Eq, G::Prev(name.to_string()), g);
    let prev_ne = |g: G| G::bin(BinOp::Ne, G::Prev(name.to_string()), g);
    match &m.arms.as_ref()?[arm] {
        ArmPat::Values(vs) => {
            let text = vs
                .iter()
                .map(|(v, _)| show(reg, *v))
                .collect::<Vec<_>>()
                .join(" | ");
            Some((
                fold(BinOp::Or, vs.iter().map(|(_, l)| prev_eq(l.g())))?,
                text,
            ))
        }
        ArmPat::Wildcard => {
            let lits = m.literal_values();
            let named = |v: i128| lits.iter().any(|(x, _)| *x == v);
            if let Some(e) = &reg.enum_info {
                // Enum: joker kaynağı adı geçmeyen varyantlar (Karar 6).
                let unnamed = e
                    .variants
                    .iter()
                    .filter(|(_, c)| !named(*c))
                    .map(|(v, _)| prev_eq(G::Path(vec![e.name.clone(), v.clone()])));
                return Some((fold(BinOp::Or, unnamed)?, "_".to_string()));
            }
            // Sayısal: yazılan değerlerin ve reset değerinin hepsi adlıysa
            // joker kol erişilemez (ADR-0074 yan bulgu 2).
            let reachable_unnamed = !named(fsm.init) || fsm.values.iter().any(|(v, _)| !named(*v));
            if !reachable_unnamed {
                return None;
            }
            Some((
                fold(BinOp::And, lits.iter().map(|(_, l)| prev_ne(l.g())))?,
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
