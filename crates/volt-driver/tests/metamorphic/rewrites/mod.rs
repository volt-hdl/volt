//! The rewrite catalogue (#254, ADR-0117). Each rewrite turns one spelling
//! of a construct into another spelling that the specification says is the
//! same design, and names the sentence that says so. A rewrite without
//! such a sentence is not added: the gap is a finding (ADR-0117 §2).

pub mod annot_decl;
pub mod clock_alias;
pub mod if_match;
pub mod match_form;
pub mod shorthand;

use crate::metamorphic::edit::Edit;

pub struct Rewrite {
    /// Stable name, used by the known-failures list.
    pub id: &'static str,
    /// The construct whose two spellings it exchanges.
    pub construct: &'static str,
    /// The spec sentence or ADR that makes the two forms equivalent.
    pub source: &'static str,
    /// The silent-wrong issue this rewrite would have found.
    pub issue: u32,
    /// Codes the rewritten form may add, with the reason (a rewrite that
    /// adds a name may add W1001 for it). Empty: none.
    pub extra_codes: &'static [(&'static str, &'static str)],
    /// The edits at every site of `src`; empty when there is none.
    pub sites: fn(&str) -> Vec<Edit>,
    /// Complete designs with at least one site: the relations must hold on
    /// them (`rewrite_examples_keep_their_meaning`).
    pub examples: &'static [&'static str],
}

const SHORTHAND_EXAMPLE: &str = "struct S { a : u8, b : bool }
module Child {
    in  x : u8
    out y : u8
    y = x ^ 0x5A
}
module M {
    in  a : u8
    in  b : bool
    in  x : u8
    out o : S
    out y : u8
    o = S { a, b }
    let c = Child { x }
    y = c.y
}
";

const LONG_EXAMPLE: &str = "struct S { a : u8, b : bool }
module Child {
    in  x : u8
    out y : u8
    y = x ^ 0x5A
}
module M {
    in  a : u8
    in  b : bool
    in  x : u8
    out o : S
    out y : u8
    o = S { a: a, b: b }
    let c = Child { x: x }
    y = c.y
}
";

const MATCH_EXAMPLE: &str = "module M {
    in  clk : clock
    in  s : u2
    in  a : u8
    out y : u8
    out q : u8
    reg r : u8 = 0
    comb {
        match s {
            0 => { y = a }
            1 | 2 => { y = a ^ 0xFF }
            _ => { y = 0 }
        }
    }
    on clk {
        r <= match s { 0 => a, 1 => r, _ => r ^ a }
    }
    q = r
}
";

const IF_EXAMPLE: &str = "module M {
    in  clk : clock
    in  sel : bool
    in  en  : bool
    in  a : u8
    in  b : u8
    out y : u8
    out q : u8
    reg r : u8 = 0
    y = if sel { a } else { b ^ 0x0F }
    on clk {
        r <= if en { r ^ b } else if sel { a } else { r }
    }
    q = r
}
";

const BOOL_MATCH_EXAMPLE: &str = "module M {
    in  clk : clock
    in  sel : bool
    in  en  : bool
    in  a : u8
    in  b : u8
    out y : u8
    out q : u8
    reg r : u8 = 0
    y = match sel { true => a, _ => b ^ 0x0F }
    on clk {
        r <= match en { false => r, _ => r ^ a }
    }
    q = r
}
";

const ANNOT_EXAMPLE: &str = "domain Fast {
    clock = posedge
    reset = sync active_high
}
domain Slow {
    clock = posedge
    reset = sync active_high
}
module M {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  req  : bool @Fast
    in  d    : u8 @Fast
    out seen : bool @Slow
    out y    : u8 @Fast
    wire synced : bool @Slow
    synced = sync(req, slow_clk)
    let w : u8 @Fast = d ^ 0x3C
    seen = synced
    y = w
}
";

const CLOCK_EXAMPLE: &str = "module Child {
    in  clk : clock
    in  d : u8
    out q : u8
    reg r : u8 = 0
    on clk { r <= d ^ r }
    q = r
}
module M {
    in  clk : clock
    in  d : u8
    out q : u8
    let c = Child { clk: clk, d: d }
    q = c.q
}
";

const ALIAS_EXAMPLE: &str = "type Clk = clock
module M {
    in  clk : Clk
    in  d : u8
    out q : u8
    reg r : u8 = 0
    on clk { r <= d + r }
    q = r
}
";

pub const ALL: &[Rewrite] = &[
    Rewrite {
        id: "shorthand-to-long",
        construct: "field-init and port-binding shorthand",
        source: "grammar-full.ebnf, FieldInit and PortBind: the field `x` (and the binding `x`) \
                 means exactly `x: x`, and every check applies to both forms alike",
        issue: 204,
        extra_codes: &[],
        sites: shorthand::to_long,
        examples: &[SHORTHAND_EXAMPLE],
    },
    Rewrite {
        id: "long-to-shorthand",
        construct: "field-init and port-binding shorthand",
        source: "grammar-full.ebnf, FieldInit and PortBind: the field `x` (and the binding `x`) \
                 means exactly `x: x`, and every check applies to both forms alike",
        issue: 204,
        extra_codes: &[],
        sites: shorthand::to_short,
        examples: &[LONG_EXAMPLE],
    },
    Rewrite {
        id: "match-stmt-to-expr",
        construct: "statement match and expression match in on/comb",
        source: "sv-mapping.md §19.1 (ADR-0083): a match expression that is the whole right-hand \
                 side of an assignment inside on/comb is lowered to the statement form with the \
                 target repeated in every arm; grammar-full.ebnf MatchExpr: the expression match \
                 follows the same rules as the statement",
        issue: 216,
        extra_codes: &[],
        sites: match_form::stmt_to_expr,
        examples: &[MATCH_EXAMPLE],
    },
    Rewrite {
        id: "match-expr-to-stmt",
        construct: "statement match and expression match in on/comb",
        source: "sv-mapping.md §19.1 (ADR-0083): a match expression that is the whole right-hand \
                 side of an assignment inside on/comb is lowered to the statement form with the \
                 target repeated in every arm; grammar-full.ebnf MatchExpr: the expression match \
                 follows the same rules as the statement",
        issue: 216,
        extra_codes: &[],
        sites: match_form::expr_to_stmt,
        examples: &[MATCH_EXAMPLE],
    },
    Rewrite {
        id: "if-to-match",
        construct: "if/else on a bool and the two-arm match expression",
        source: "type-inference.md §3.9 (ADR-0083): a match expression is a multi-arm if expression                  (§3.7) whose width behaviour does not diverge; grammar-full.ebnf MatchExpr: literal                  patterns and `_`, the first matching arm wins; domain-inference.md K5: the                  condition and the subject join the result alike",
        issue: 215,
        extra_codes: &[],
        sites: if_match::if_to_match,
        examples: &[IF_EXAMPLE],
    },
    Rewrite {
        id: "match-to-if",
        construct: "if/else on a bool and the two-arm match expression",
        source: "type-inference.md §3.9 (ADR-0083): a match expression is a multi-arm if expression                  (§3.7) whose width behaviour does not diverge; grammar-full.ebnf MatchExpr: literal                  patterns and `_`, the first matching arm wins; domain-inference.md K5: the                  condition and the subject join the result alike",
        issue: 215,
        extra_codes: &[],
        sites: if_match::match_to_if,
        examples: &[BOOL_MATCH_EXAMPLE],
    },
    Rewrite {
        id: "wire-to-let",
        construct: "domain and trust annotation on a wire and on a let",
        source: "ADR-0088 and domain-inference.md K1: the annotation of a wire and of a module let                  fixes the signal's domain and is checked against its driver or value alike (E3001);                  K11 (ADR-0052): the signal's trust level is its annotated domain's;                  type-inference.md §3.9: a typed let and an assignment target are the same check                  context; sv-mapping.md §5.2, §16.3: both are a net with a continuous assignment",
        issue: 211,
        extra_codes: &[],
        sites: annot_decl::wire_to_let,
        examples: &[ANNOT_EXAMPLE],
    },
    Rewrite {
        id: "let-to-wire",
        construct: "domain and trust annotation on a wire and on a let",
        source: "ADR-0088 and domain-inference.md K1: the annotation of a wire and of a module let                  fixes the signal's domain and is checked against its driver or value alike (E3001);                  K11 (ADR-0052): the signal's trust level is its annotated domain's;                  type-inference.md §3.9: a typed let and an assignment target are the same check                  context; sv-mapping.md §5.2, §16.3: both are a net with a continuous assignment",
        issue: 211,
        extra_codes: &[],
        sites: annot_decl::let_to_wire,
        examples: &[ANNOT_EXAMPLE],
    },
    Rewrite {
        id: "clock-to-alias",
        construct: "a clock written as `clock` and through a type alias",
        source: "ADR-0070 §6: a non-generic alias resolves to its target; `type C = clock` works like                  the type itself in a port, a reg, a wire and a let",
        issue: 234,
        extra_codes: &[],
        sites: clock_alias::clock_to_alias,
        examples: &[CLOCK_EXAMPLE],
    },
    Rewrite {
        id: "alias-to-clock",
        construct: "a clock written as `clock` and through a type alias",
        source: "ADR-0070 §6: a non-generic alias resolves to its target; `type C = clock` works like                  the type itself in a port, a reg, a wire and a let",
        issue: 234,
        extra_codes: &[],
        sites: clock_alias::alias_to_clock,
        examples: &[ALIAS_EXAMPLE],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rewrite_names_its_source() {
        let mut ids = std::collections::BTreeSet::new();
        for r in ALL {
            assert!(ids.insert(r.id), "duplicate rewrite id {}", r.id);
            assert!(
                r.source.contains(".md") || r.source.contains(".ebnf") || r.source.contains("ADR-"),
                "{}: the source must name a spec file or an ADR",
                r.id
            );
            assert!(r.issue > 0);
            assert!(!r.examples.is_empty(), "{}: no example", r.id);
            for ex in r.examples {
                assert!(
                    !(r.sites)(ex).is_empty(),
                    "{}: an example without a site",
                    r.id
                );
            }
        }
    }
}
