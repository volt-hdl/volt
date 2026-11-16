//! The rewrite catalogue (#254, ADR-0117). Each rewrite turns one spelling
//! of a construct into another spelling that the specification says is the
//! same design, and names the sentence that says so. A rewrite without
//! such a sentence is not added: the gap is a finding (ADR-0117 §2).

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
