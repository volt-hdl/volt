//! Desen ayrıştırma (grammar-full.ebnf §12, ast-nodes.md §8).
//!
//! Kapsam: joker `_`, literal, bağlama, yol (enum varyantı), tuple ve
//! `|` alternatifleri. Exhaustiveness kontrolü F2'ye aittir — burada
//! yalnız ayrıştırılır.

use volt_ast::{Expr, ExprKind, FieldPattern, Idx, Pattern, PatternArgs, PatternKind, UnOp};

use crate::token::TokenKind::*;

use super::Parser;

impl Parser<'_> {
    /// `Pattern = Alternatif { "|" Alternatif }` — tek alternatif
    /// sarmalanmaz, birden çoğu `Or` düğümünde toplanır.
    pub(crate) fn parse_pattern(&mut self) -> Idx<Pattern> {
        let start = self.pos;
        let first = self.parse_pattern_atom();
        if !self.at(Pipe) {
            return first;
        }
        let mut alts = vec![first];
        while self.eat(Pipe) {
            alts.push(self.parse_pattern_atom());
        }
        let span = self.span_from(start);
        self.ast.patterns.alloc(Pattern {
            span,
            kind: PatternKind::Or(alts),
        })
    }

    fn parse_pattern_atom(&mut self) -> Idx<Pattern> {
        let start = self.pos;
        match self.current() {
            Some(Ident) if self.current_text() == "_" => {
                let span = self.bump();
                self.ast.patterns.alloc(Pattern {
                    span,
                    kind: PatternKind::Wildcard,
                })
            }
            Some(Ident) => {
                let path = self.parse_path();
                let args = match self.current() {
                    Some(LParen) => Some(self.parse_pattern_tuple_args()),
                    Some(LBrace) => Some(self.parse_pattern_struct_args()),
                    _ => None,
                };
                let span = self.span_from(start);
                // Tek parçalı, argümansız isim → bağlama; aksi yol deseni.
                let kind = if args.is_none() && path.segments.len() == 1 {
                    PatternKind::Binding(path.segments.into_iter().next().unwrap())
                } else {
                    PatternKind::Path { path, args }
                };
                self.ast.patterns.alloc(Pattern { span, kind })
            }
            Some(IntLit) | Some(KwTrue) | Some(KwFalse) => {
                let expr = self.parse_pattern_literal();
                let span = self.span_from(start);
                self.ast.patterns.alloc(Pattern {
                    span,
                    kind: PatternKind::Literal(expr),
                })
            }
            Some(Minus) => {
                // Negatif literal: `-1`
                self.bump_any();
                let operand = self.parse_pattern_literal();
                let span = self.span_from(start);
                let expr = self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::Unary {
                        op: UnOp::Neg,
                        operand,
                    },
                });
                self.ast.patterns.alloc(Pattern {
                    span,
                    kind: PatternKind::Literal(expr),
                })
            }
            Some(LParen) => {
                let open = self.bump();
                let mut elems = Vec::new();
                let mut saw_comma = false;
                while !self.at(RParen) && !self.at_eof() {
                    let before = self.pos;
                    elems.push(self.parse_pattern());
                    if self.eat(Comma) {
                        saw_comma = true;
                    } else if self.pos == before {
                        self.bump_any(); // ilerleme garantisi
                    } else {
                        break;
                    }
                }
                self.expect_closing(RParen, ")", open);
                let span = self.span_from(start);
                if elems.len() == 1 && !saw_comma {
                    // `(p)` gruplamadır, tek elemanlı tuple değil.
                    elems.into_iter().next().unwrap()
                } else {
                    self.ast.patterns.alloc(Pattern {
                        span,
                        kind: PatternKind::Tuple(elems),
                    })
                }
            }
            _ => {
                self.error_expected(
                    "desen",
                    "_, literal, isim, Yol::Varyant veya (desen, ...) bekleniyor",
                );
                self.alloc_error_pattern(self.current_span())
            }
        }
    }

    /// Desen içi literal: IntLit / true / false.
    fn parse_pattern_literal(&mut self) -> Idx<Expr> {
        match self.current() {
            Some(IntLit) => self.parse_int_lit(),
            Some(KwTrue) => {
                let span = self.bump();
                self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::BoolLit(true),
                })
            }
            Some(KwFalse) => {
                let span = self.bump();
                self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::BoolLit(false),
                })
            }
            _ => {
                self.error_expected("literal desen", "42, true veya false bekleniyor");
                self.alloc_error_expr(self.current_span())
            }
        }
    }

    /// `Some(x, _)` — parantezli desen argümanları.
    fn parse_pattern_tuple_args(&mut self) -> PatternArgs {
        let open = self.bump(); // '('
        let mut elems = Vec::new();
        while !self.at(RParen) && !self.at_eof() {
            let before = self.pos;
            elems.push(self.parse_pattern());
            if !self.eat(Comma) && self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        self.expect_closing(RParen, ")", open);
        PatternArgs::Tuple(elems)
    }

    /// `Point { x, y: 0 }` — alan desenleri.
    fn parse_pattern_struct_args(&mut self) -> PatternArgs {
        let open = self.bump(); // '{'
        let mut fields = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            if self.at(Ident) {
                let fstart = self.pos;
                let name = self.parse_name();
                let pattern = if self.eat(Colon) {
                    Some(self.parse_pattern())
                } else {
                    None
                };
                fields.push(FieldPattern {
                    span: self.span_from(fstart),
                    name,
                    pattern,
                });
            } else {
                self.error_expected("alan deseni", "İsim veya isim: desen bekleniyor");
            }
            if !self.eat(Comma) && self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        self.expect_closing(RBrace, "}", open);
        PatternArgs::Struct(fields)
    }
}
