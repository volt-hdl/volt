//! Otomatik FSM ve sayaç kontratları — ADR-0066.
//!
//! Parser desugar'ı tanınan durum makinelerine geçiş/durum cover'ı,
//! açık sınırlı sayaçlara sınır invariant'ı ve sarma cover'ı ekler; her
//! kontrat kökenini (`AutoOrigin`) taşır. Tanıma muhafazakârdır: tanınan
//! biçimin dışındaki tek bir yazma bile register'ı aday dışı bırakır.

use std::collections::HashSet;

use volt_ast::{AutoRule, ContractKind, ExprKind, ItemKind, ModuleDecl};
use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};

fn p(src: &str) -> ParseResult {
    let res = parse(FileId(0), src);
    assert!(res.diagnostics.is_empty(), "{:?}", res.error_codes());
    res
}

fn module(res: &ParseResult, idx: usize) -> &ModuleDecl {
    let ItemKind::Module(m) = &res.ast.items_arena[res.ast.items[idx]].kind else {
        panic!("öğe {idx} modül olmalı");
    };
    m
}

/// Otomatik kontratlar: (kural, tür, metin).
fn autos(res: &ParseResult, idx: usize) -> Vec<(AutoRule, ContractKind, String)> {
    module(res, idx)
        .contracts
        .iter()
        .filter_map(|c| c.auto.as_ref().map(|a| (a.rule, c.kind, a.text.clone())))
        .collect()
}

fn texts(res: &ParseResult, idx: usize, rule: AutoRule) -> Vec<String> {
    autos(res, idx)
        .into_iter()
        .filter(|(r, _, _)| *r == rule)
        .map(|(_, _, t)| t)
        .collect()
}

/// Üç durumlu FSM: 0 (boşta) → 1 → 2 → joker → 0.
const FSM: &str = "module F {
    in clk : clock
    in go : bool
    out busy : bool
    reg s : u2 = 0
    on clk {
        match s {
            0 => { if go { s <= 1 } }
            1 => { s <= 2 }
            _ => { s <= 0 }
        }
    }
    busy = s != 0
}";

/// Sayaç gövdesi: `{guard}` artışı korur.
fn counter(decl: &str, body: &str) -> String {
    format!(
        "const LIMIT : u8 = 10\nmodule C {{\n    in clk : clock\n    in en : bool\n    in lim : u8\n    \
         out q : u8\n    {decl}\n    on clk {{\n{body}\n    }}\n    q = r as u8\n}}"
    )
}

// ═══ FSM ═══════════════════════════════════════════════════════════

#[test]
fn fsm_literal_arms_get_one_transition_cover_each() {
    let res = p(FSM);
    let t = texts(&res, 0, AutoRule::FsmTransition);
    assert_eq!(
        t,
        [
            "prev(s) == 0 && s == 1",
            "prev(s) == 1 && s == 2",
            "prev(s) != 0 && prev(s) != 1 && s == 0",
        ]
    );
}

#[test]
fn fsm_transition_covers_are_cover_kind() {
    let res = p(FSM);
    assert!(autos(&res, 0)
        .iter()
        .all(|(_, kind, _)| *kind == ContractKind::Cover));
}

#[test]
fn fsm_states_reached_by_a_transition_get_no_separate_state_cover() {
    // 1 ve 2 geçiş cover'larının hedefi, 0 reset değeri → F2 yok.
    let res = p(FSM);
    assert!(texts(&res, 0, AutoRule::FsmState).is_empty());
}

#[test]
fn fsm_valid_state_invariant_is_never_generated() {
    // F1: E0014 `_` kolunu zorunlu kılar; invariant üretilmez.
    let res = p(FSM);
    assert!(autos(&res, 0)
        .iter()
        .all(|(_, kind, _)| *kind != ContractKind::Invariant));
}

#[test]
fn fsm_or_pattern_arm_source_is_a_disjunction() {
    let res = p(
        "module F { in clk : clock\n reg s : u2 = 0\n on clk { match s {
        0 => { s <= 1 }
        1 | 2 => { s <= 3 }
        _ => { s <= 0 } } } }",
    );
    let t = texts(&res, 0, AutoRule::FsmTransition);
    assert!(
        t.contains(&"(prev(s) == 1 || prev(s) == 2) && s == 3".to_string()),
        "{t:?}"
    );
}

#[test]
fn fsm_self_loop_is_not_a_transition() {
    let res = p(
        "module F { in clk : clock\n in x : bool\n reg s : u2 = 0\n on clk { match s {
        0 => { s <= 1 }
        1 => { if x { s <= 1 } else { s <= 0 } }
        _ => { s <= 0 } } } }",
    );
    let t = texts(&res, 0, AutoRule::FsmTransition);
    // `1 => s <= 1` öz-döngüdür, cover almaz; joker kolun `_ -> 0`'ı alır.
    assert_eq!(
        t,
        [
            "prev(s) == 0 && s == 1",
            "prev(s) == 1 && s == 0",
            "prev(s) != 0 && prev(s) != 1 && s == 0",
        ]
    );
}

#[test]
fn fsm_wildcard_writing_a_wildcard_value_is_not_a_transition() {
    // 2 ve 3 `_` kolunda yaşar: `_ -> 3` kaynağı "0 ya da 1 değil" olur ve
    // 3'te beklemekle de tutar — anlamsız cover. 3 yerine F2 alır.
    let res = p(
        "module F { in clk : clock\n in x : bool\n reg s : u2 = 0\n on clk { match s {
        0 => { s <= 1 }
        1 => { s <= 2 }
        _ => { if x { s <= 3 } else { s <= 0 } } } } }",
    );
    let t = texts(&res, 0, AutoRule::FsmTransition);
    assert!(t.iter().all(|c| !c.ends_with("s == 3")), "{t:?}");
    assert_eq!(texts(&res, 0, AutoRule::FsmState), ["s == 3"]);
}

#[test]
fn register_with_an_unrecognised_write_gets_nothing() {
    // Sonekli yazma (`r[0] <=`) register'ı aday dışı bırakır — tanınan
    // yazmalar tek başına sayaç gibi görünse bile.
    let res = p(&counter(
        "reg r : u8 = 0",
        "if r == 9 { r <= 0 } else { r <= r + 1 }\n if en { r[0] <= false }",
    ));
    assert!(autos(&res, 1).is_empty());
}

#[test]
fn fsm_state_entered_outside_the_match_gets_a_state_cover() {
    let res = p(
        "module F { in clk : clock\n in abort : bool\n reg s : u2 = 0\n on clk {
        match s { 0 => { s <= 1 }\n _ => { s <= 0 } }
        if abort { s <= 3 } } }",
    );
    assert_eq!(texts(&res, 0, AutoRule::FsmState), ["s == 3"]);
}

#[test]
fn fsm_named_constant_state_is_rendered_by_name() {
    let res = p(
        "const BUSY : u2 = 2\nmodule F { in clk : clock\n reg s : u2 = 0\n on clk {
        match s { 0 => { s <= BUSY }\n _ => { s <= 0 } } } }",
    );
    assert!(
        texts(&res, 1, AutoRule::FsmTransition).contains(&"prev(s) == 0 && s == BUSY".to_string())
    );
}

#[test]
fn fsm_non_constant_write_disables_the_register() {
    let res = p(
        "module F { in clk : clock\n in nx : u2\n reg s : u2 = 0\n on clk {
        match s { 0 => { s <= 1 }\n _ => { s <= nx } } } }",
    );
    assert!(autos(&res, 0).is_empty());
}

#[test]
fn fsm_register_without_a_match_is_not_a_state_machine() {
    let res = p(
        "module F { in clk : clock\n in x : bool\n reg s : u2 = 0\n on clk {
        if x { s <= 1 } else { s <= 2 } } }",
    );
    assert!(autos(&res, 0).is_empty());
}

#[test]
fn fsm_over_the_transition_limit_falls_back_to_state_covers() {
    // 5 durumlu tam ağ: 5 × 4 = 20 geçiş > 16 → F3 yerine reset dışı
    // her durum için F2 (4 cover).
    let arms: String = (0..5)
        .map(|i| {
            let targets: Vec<String> = (0..5)
                .filter(|&j| j != i)
                .enumerate()
                .map(|(k, j)| format!("if sel == {k} {{ s <= {j} }}"))
                .collect();
            format!("            {i} => {{ {} }}\n", targets.join("\n "))
        })
        .collect();
    let src = format!(
        "module F {{ in clk : clock\n in sel : u2\n reg s : u3 = 0\n on clk {{ match s {{\n{arms} _ => {{ s <= 0 }} }} }} }}"
    );
    let res = p(&src);
    assert!(texts(&res, 0, AutoRule::FsmTransition).is_empty());
    assert_eq!(
        texts(&res, 0, AutoRule::FsmState),
        ["s == 1", "s == 2", "s == 3", "s == 4"]
    );
}

#[test]
fn fsm_over_both_limits_gets_no_covers() {
    // 20 durumlu halka: 20 geçiş > 16 ve 19 durum > 16 → hiç cover yok;
    // büyük FSM'in kapsamı elle yazılır (ADR-0066 §2).
    let arms: String = (0..20)
        .map(|i| format!("            {i} => {{ s <= {} }}\n", (i + 1) % 20))
        .collect();
    let src = format!(
        "module F {{ in clk : clock\n reg s : u5 = 0\n on clk {{ match s {{\n{arms} _ => {{ s <= 0 }} }} }} }}"
    );
    let res = p(&src);
    assert!(autos(&res, 0).is_empty());
}

#[test]
fn fsm_user_written_state_cover_is_not_duplicated() {
    let res = p(
        "module F { in clk : clock\n in abort : bool\n cover: s == 3\n reg s : u2 = 0\n on clk {
        match s { 0 => { s <= 1 }\n _ => { s <= 0 } }
        if abort { s <= 3 } } }",
    );
    assert!(texts(&res, 0, AutoRule::FsmState).is_empty());
}

#[test]
fn fsm_on_a_second_clock_is_ignored() {
    // Kontratlar ilk saatte örneklenir; başka saatteki FSM aday değil.
    let res = p(
        "module F { in clk : clock\n in clk2 : clock\n reg s : u2 = 0\n on clk2 {
        match s { 0 => { s <= 1 }\n _ => { s <= 0 } } } }",
    );
    assert!(autos(&res, 0).is_empty());
}

// ═══ Sayaç ═════════════════════════════════════════════════════════

#[test]
fn counter_else_of_equality_gives_bound_and_wrap() {
    let res = p(&counter(
        "reg r : u8 = 0",
        "if r == 9 { r <= 0 } else { r <= r + 1 }",
    ));
    assert_eq!(texts(&res, 1, AutoRule::CounterBound), ["r <= 9"]);
    assert_eq!(texts(&res, 1, AutoRule::CounterWrap), ["r == 9"]);
    let kinds: Vec<ContractKind> = autos(&res, 1).iter().map(|(_, k, _)| *k).collect();
    assert_eq!(kinds, [ContractKind::Invariant, ContractKind::Cover]);
}

#[test]
fn counter_then_of_less_than_gives_bound() {
    let res = p(&counter("reg r : u8 = 0", "if r < 9 { r <= r + 1 }"));
    assert_eq!(texts(&res, 1, AutoRule::CounterBound), ["r <= 9"]);
}

#[test]
fn counter_else_of_greater_equal_gives_bound() {
    let res = p(&counter(
        "reg r : u4 = 0",
        "if r >= 7 { r <= 0 } else { r <= 1 + r }",
    ));
    assert_eq!(texts(&res, 1, AutoRule::CounterBound), ["r <= 7"]);
}

#[test]
fn counter_guard_inside_a_conjunction_counts() {
    let res = p(&counter("reg r : u8 = 0", "if en && r != 9 { r <= r + 1 }"));
    assert_eq!(texts(&res, 1, AutoRule::CounterBound), ["r <= 9"]);
}

#[test]
fn counter_constant_bound_expression_is_kept_verbatim() {
    let res = p(&counter(
        "reg r : u8 = 0",
        "if r == LIMIT - 1 { r <= 0 } else { r <= r + 1 }",
    ));
    assert_eq!(texts(&res, 1, AutoRule::CounterBound), ["r <= LIMIT - 1"]);
}

#[test]
fn counter_unguarded_increment_is_free_running() {
    let res = p(&counter("reg r : u8 = 0", "if en { r <= r + 1 }"));
    assert!(autos(&res, 1).is_empty());
}

#[test]
fn counter_bound_from_a_port_is_not_constant() {
    let res = p(&counter(
        "reg r : u8 = 0",
        "if r == lim { r <= 0 } else { r <= r + 1 }",
    ));
    assert!(autos(&res, 1).is_empty());
}

#[test]
fn counter_bound_at_type_maximum_gets_only_the_wrap_cover() {
    let res = p(&counter(
        "reg r : u4 = 0",
        "if r == 15 { r <= 0 } else { r <= r + 1 }",
    ));
    assert!(texts(&res, 1, AutoRule::CounterBound).is_empty());
    assert_eq!(texts(&res, 1, AutoRule::CounterWrap), ["r == 15"]);
}

#[test]
fn counter_with_two_different_bounds_is_rejected() {
    let res = p(&counter(
        "reg r : u8 = 0",
        "if en { if r == 9 { r <= 0 } else { r <= r + 1 } } else { if r == 5 { r <= 0 } else { r <= r + 1 } }",
    ));
    assert!(autos(&res, 1).is_empty());
}

#[test]
fn counter_reset_value_above_the_bound_is_rejected() {
    let res = p(&counter(
        "reg r : u8 = 20",
        "if r == 9 { r <= 0 } else { r <= r + 1 }",
    ));
    assert!(autos(&res, 1).is_empty());
}

#[test]
fn counter_other_update_form_is_rejected() {
    let res = p(&counter(
        "reg r : u8 = 0",
        "if r == 9 { r <= 0 } else { r <= r + 2 }",
    ));
    assert!(autos(&res, 1).is_empty());
}

#[test]
fn counter_user_written_bound_is_not_duplicated() {
    let src = counter("reg r : u8 = 0", "if r == 9 { r <= 0 } else { r <= r + 1 }")
        .replace("out q : u8", "out q : u8\n    invariant: r <= 9");
    let res = p(&src);
    assert!(texts(&res, 1, AutoRule::CounterBound).is_empty());
    assert_eq!(texts(&res, 1, AutoRule::CounterWrap), ["r == 9"]);
}

// ═══ Kapatma ve köken ══════════════════════════════════════════════

#[test]
fn no_auto_contracts_on_the_module_suppresses_everything() {
    let res = p(&format!("@no_auto_contracts\n{FSM}"));
    assert!(autos(&res, 0).is_empty());
}

#[test]
fn no_auto_contracts_on_a_register_suppresses_only_that_register() {
    let src = counter(
        "@no_auto_contracts reg r : u8 = 0\n    reg k : u8 = 0",
        "if r == 9 { r <= 0 } else { r <= r + 1 }\n if k == 3 { k <= 0 } else { k <= k + 1 }",
    );
    let res = p(&src);
    assert_eq!(texts(&res, 1, AutoRule::CounterBound), ["k <= 3"]);
}

#[test]
fn origin_points_at_the_guarding_comparison() {
    let src = counter("reg r : u8 = 0", "if r == 9 { r <= 0 } else { r <= r + 1 }");
    let res = p(&src);
    let c = &module(&res, 1).contracts[0];
    let a = c.auto.as_ref().expect("otomatik");
    assert_eq!(&src[a.from.start as usize..a.from.end as usize], "r == 9");
    assert_eq!(a.subject, "wrap check on r");
    assert_eq!(c.span, a.from);
}

#[test]
fn transition_origin_points_at_the_assignment() {
    let res = p(FSM);
    let c = &module(&res, 0).contracts[1];
    let a = c.auto.as_ref().expect("otomatik");
    assert_eq!(&FSM[a.from.start as usize..a.from.end as usize], "s <= 2");
    assert_eq!(a.subject, "match on s, transition 1 -> 2");
}

#[test]
fn generated_names_have_distinct_zero_length_spans() {
    // İsim çözümleme kullanımları span ile anahtarlar: her ad benzersiz.
    let res = p(FSM);
    let mut seen = HashSet::new();
    for c in &module(&res, 0).contracts {
        let mut stack = vec![c.expr];
        while let Some(e) = stack.pop() {
            match &res.ast.exprs[e].kind {
                ExprKind::Path(path) => {
                    for n in &path.segments {
                        assert_eq!(n.span.start, n.span.end, "{}", n.text);
                        assert!(seen.insert(n.span), "tekrarlanan span: {}", n.text);
                    }
                }
                ExprKind::Binary { lhs, rhs, .. } => stack.extend([*lhs, *rhs]),
                ExprKind::Call { callee, args } => {
                    stack.push(*callee);
                    stack.extend(args.iter().copied());
                }
                _ => {}
            }
        }
    }
    assert!(seen.len() >= 10);
}

#[test]
fn handshake_contracts_carry_their_origin() {
    let res =
        p("module M { in clk : clock\n out tx : Handshake<u8>\n tx.valid = true\n tx.data = 0 }");
    let rules: Vec<(AutoRule, String)> = module(&res, 0)
        .contracts
        .iter()
        .filter_map(|c| c.auto.as_ref().map(|a| (a.rule, a.subject.clone())))
        .collect();
    assert_eq!(rules.len(), 2);
    assert!(rules
        .iter()
        .all(|(r, s)| *r == AutoRule::Handshake && s == "Handshake port tx"));
    assert_eq!(
        texts(&res, 0, AutoRule::Handshake)[0],
        "prev(tx_valid) && !prev(tx_ready) -> tx_valid"
    );
}
