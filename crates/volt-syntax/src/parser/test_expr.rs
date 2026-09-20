//! Test gövdesi ifadeleri (ADR-0033, ADR-0058).
//!
//! Donanım ifade ayrıştırıcısından bağımsız küçük bir öncelik
//! tırmanışıdır: test ifadeleri 64 bit işaretsiz betik değerleridir,
//! modül ifade arenasına girmez. Öncelik ve birleşim donanım
//! ifadeleriyle aynıdır (operator-precedence.md): `* / %`, `+ -`,
//! `<< >>`, `&`, `^`, `|`, `< > <= >=`, `== !=`, `&&`, `||`;
//! karşılaştırma ve eşitlik zincirlenemez (E0010 kuralı).

use volt_ast::{TestBinOp, TestExpr, TestExprKind, TestUnOp};
use volt_diagnostics::lstr;

use crate::token::TokenKind::{self, *};

use super::test::parse_int_text;
use super::{Parser, MAX_DEPTH};

/// Zincirlenemeyen düzeyler: `a < b < c` ve `a == b == c` hatadır.
const COMPARISON_POWER: u8 = 4;
const EQUALITY_POWER: u8 = 3;

/// Operatör → (ikili işlem, bağlama gücü). Büyük sayı sıkı bağlar.
fn binary_op(kind: TokenKind) -> Option<(TestBinOp, u8)> {
    Some(match kind {
        Star => (TestBinOp::Mul, 10),
        Slash => (TestBinOp::Div, 10),
        Percent => (TestBinOp::Rem, 10),
        Plus => (TestBinOp::Add, 9),
        Minus => (TestBinOp::Sub, 9),
        Shl => (TestBinOp::Shl, 8),
        Shr => (TestBinOp::Shr, 8),
        Amp => (TestBinOp::And, 7),
        Caret => (TestBinOp::Xor, 6),
        Pipe => (TestBinOp::Or, 5),
        Lt => (TestBinOp::Lt, COMPARISON_POWER),
        Le => (TestBinOp::Le, COMPARISON_POWER),
        Gt => (TestBinOp::Gt, COMPARISON_POWER),
        Ge => (TestBinOp::Ge, COMPARISON_POWER),
        EqEq => (TestBinOp::Eq, EQUALITY_POWER),
        NotEq => (TestBinOp::Ne, EQUALITY_POWER),
        AmpAmp => (TestBinOp::LogAnd, 2),
        PipePipe => (TestBinOp::LogOr, 1),
        _ => return None,
    })
}

impl Parser<'_> {
    /// Tam test ifadesi.
    pub(super) fn parse_test_expr(&mut self) -> Option<TestExpr> {
        self.parse_test_binary(0)
    }

    /// Derinlik sınırı aşıldıysa tanı üretir. Ağaç derinliği hem iç
    /// içelikle hem de sol-derin operatör zinciriyle büyür; ikisi de aynı
    /// sayaca yazılır, böylece sonraki özyineli yürüyüşler (denetim,
    /// indirgeme, C++ üretimi) yığını taşıramaz.
    fn test_depth_exceeded(&mut self) -> bool {
        if self.depth < MAX_DEPTH {
            return false;
        }
        self.error_expected(
            &lstr!(en: "a shallower test expression"; tr: "daha sığ bir test ifadesi"),
            &lstr!(en: "split the expression using intermediate 'let' bindings";
                   tr: "ifadeyi ara 'let' bağlamalarıyla bölün"),
        );
        true
    }

    /// Sol-birleşimli öncelik tırmanışı.
    fn parse_test_binary(&mut self, min_power: u8) -> Option<TestExpr> {
        let entry_depth = self.depth;
        let result = self.parse_test_binary_inner(min_power);
        self.depth = entry_depth;
        result
    }

    fn parse_test_binary_inner(&mut self, min_power: u8) -> Option<TestExpr> {
        let start = self.pos;
        let mut lhs = self.parse_test_unary()?;
        while let Some((op, power)) = self.current().and_then(binary_op) {
            if power < min_power {
                break;
            }
            if self.test_depth_exceeded() {
                return None;
            }
            self.depth += 1; // zincirin her halkası ağacı bir kat derinleştirir
            self.bump_any();
            let rhs = self.parse_test_binary(power + 1)?;
            let non_assoc = power == COMPARISON_POWER || power == EQUALITY_POWER;
            if non_assoc
                && self
                    .current()
                    .and_then(binary_op)
                    .is_some_and(|(_, p)| p == power)
            {
                self.error_expected(
                    &lstr!(en: "a single comparison"; tr: "tek bir karşılaştırma"),
                    &lstr!(en: "comparisons cannot be chained; use parentheses or '&&'";
                           tr: "karşılaştırmalar zincirlenemez; parantez ya da '&&' kullanın"),
                );
                return None;
            }
            lhs = TestExpr {
                span: self.span_from(start),
                kind: TestExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            };
        }
        Some(lhs)
    }

    fn parse_test_unary(&mut self) -> Option<TestExpr> {
        if self.test_depth_exceeded() {
            return None;
        }
        self.depth += 1;
        let result = self.parse_test_unary_inner();
        self.depth -= 1;
        result
    }

    fn parse_test_unary_inner(&mut self) -> Option<TestExpr> {
        if !self.at(Bang) {
            return self.parse_test_primary();
        }
        let start = self.pos;
        self.bump_any();
        let operand = self.parse_test_unary()?;
        Some(TestExpr {
            span: self.span_from(start),
            kind: TestExprKind::Unary {
                op: TestUnOp::Not,
                operand: Box::new(operand),
            },
        })
    }

    fn parse_test_primary(&mut self) -> Option<TestExpr> {
        let start = self.pos;
        match self.current() {
            Some(IntLit) => {
                let span = self.bump();
                let text = self.text_of(span);
                let Some(value) = parse_int_text(text) else {
                    self.error_expected(
                        &lstr!(en: "an integer literal"; tr: "tamsayı literali"),
                        &lstr!(en: "use a plain value such as 4 or 0xA5"; tr: "4 ya da 0xA5 gibi düz bir değer kullanın"),
                    );
                    return None;
                };
                Some(TestExpr {
                    span,
                    kind: TestExprKind::Int(value),
                })
            }
            Some(KwTrue) => Some(TestExpr {
                span: self.bump(),
                kind: TestExprKind::Bool(true),
            }),
            Some(KwFalse) => Some(TestExpr {
                span: self.bump(),
                kind: TestExprKind::Bool(false),
            }),
            Some(StringLit) => {
                let span = self.bump();
                let kind = TestExprKind::Str(unquote(self.text_of(span)));
                Some(TestExpr { span, kind })
            }
            Some(LParen) => {
                let open = self.bump();
                let inner = self.parse_test_expr()?;
                self.expect_closing(RParen, ")", open);
                Some(TestExpr {
                    span: self.span_from(start),
                    kind: inner.kind,
                })
            }
            Some(LBracket) => {
                let open = self.bump();
                let items = self.parse_test_expr_list(RBracket);
                self.expect_closing(RBracket, "]", open);
                Some(TestExpr {
                    span: self.span_from(start),
                    kind: TestExprKind::Array(items),
                })
            }
            Some(Ident) => self.parse_test_name_expr(),
            _ => {
                self.error_expected(
                    &lstr!(en: "a test expression"; tr: "test ifadesi"),
                    &lstr!(en: "allowed forms: 4, 0xA5, true, dut.port, a name, name[i], [1, 2], len(x), read_hex(\"f.hex\")";
                           tr: "izinli biçimler: 4, 0xA5, true, dut.port, bir ad, ad[i], [1, 2], len(x), read_hex(\"f.hex\")"),
                );
                None
            }
        }
    }

    /// İsimle başlayan biçimler: `dut.port`, `dut.cpu.imem`, `f(...)`,
    /// `ad[i]`, `ad`.
    fn parse_test_name_expr(&mut self) -> Option<TestExpr> {
        let start = self.pos;
        let name = self.parse_name();
        let kind = match self.current() {
            Some(Dot) => {
                let mut path = Vec::new();
                while self.eat(Dot) {
                    if !self.at(Ident) {
                        self.error_expected(
                            &lstr!(en: "port name after '.'"; tr: "'.' sonrası port adı"),
                            &lstr!(en: "write it as dut.port"; tr: "dut.port biçiminde yazın"),
                        );
                        return None;
                    }
                    path.push(self.parse_name());
                }
                if path.len() == 1 {
                    let port = path.remove(0);
                    TestExprKind::PortRead { dut: name, port }
                } else {
                    TestExprKind::MemberPath { dut: name, path }
                }
            }
            Some(LParen) => {
                let open = self.bump();
                let args = self.parse_test_expr_list(RParen);
                self.expect_closing(RParen, ")", open);
                TestExprKind::Call { func: name, args }
            }
            Some(LBracket) => {
                let open = self.bump();
                let index = self.parse_test_expr()?;
                self.expect_closing(RBracket, "]", open);
                TestExprKind::Index {
                    base: name,
                    index: Box::new(index),
                }
            }
            _ => TestExprKind::Var(name),
        };
        Some(TestExpr {
            span: self.span_from(start),
            kind,
        })
    }

    /// Virgülle ayrılmış ifadeler; `close` görülünce durur (yemez).
    /// Sondaki virgül serbesttir.
    pub(super) fn parse_test_expr_list(&mut self, close: TokenKind) -> Vec<TestExpr> {
        let mut items = Vec::new();
        while !self.at(close) && !self.at_eof() {
            let before = self.pos;
            if let Some(item) = self.parse_test_expr() {
                items.push(item);
            }
            if !self.eat(Comma) && self.pos == before {
                break;
            }
        }
        items
    }
}

/// String literal metninden tırnakları atar; `\\` ve `\"` kaçışlarını
/// çözer (Windows yolları için), diğer kaçışlar aynen kalır.
fn unquote(raw: &str) -> String {
    let inner = raw
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(raw);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('\\', Some('\\')) | ('\\', Some('"')) => {
                out.extend(chars.next());
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::unquote;

    #[test]
    fn unquote_strips_quotes_and_backslash_escapes() {
        assert_eq!(unquote("\"hello.hex\""), "hello.hex");
        assert_eq!(unquote("\"..\\\\x.hex\""), "..\\x.hex");
        assert_eq!(unquote("\"a\\\"b\""), "a\"b");
    }
}
