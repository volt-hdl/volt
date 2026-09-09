//! `test "ad" { ... }` bloğu ayrıştırma (ADR-0033).
//!
//! `test` bağlamsal anahtar kelimedir (ADR-0023 deseni): lexer Ident
//! üretir, yalnız öğe konumunda `test <StringLit>` dizilimi test bloğu
//! başlatır. Gövde donanım değil doğrusal betiktir; deyimler `;` ile
//! biter ve modül deyim arenalarını KULLANMAZ (grammar-full.ebnf
//! TestStmt/TestExpr).

use volt_ast::{ItemKind, TestDecl, TestExpr, TestExprKind, TestStmt};
use volt_diagnostics::lstr;

use crate::token::TokenKind::*;

use super::Parser;

impl Parser<'_> {
    /// Öğe konumunda bağlamsal `test` başlangıcı mı?
    pub(crate) fn at_test_decl(&self) -> bool {
        self.at(Ident) && self.current_text() == "test" && matches!(self.peek(1), Some(StringLit))
    }

    /// `test "ad" { { TestStmt } }` — at_test_decl() doğruyken çağrılır.
    pub(crate) fn parse_test(&mut self) -> ItemKind {
        self.bump_any(); // 'test'
        let name_span = self.bump(); // StringLit
        let raw = self.text_of(name_span);
        // Tırnakları at; kaçış dizisi test adında anlamsız, aynen taşınır.
        let name = raw
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(raw)
            .to_string();

        if !self.at(LBrace) {
            self.error_expected(
                &lstr!(en: "'{{' after the test name"; tr: "test adından sonra '{{'"),
                &lstr!(en: "write it as test \"name\" {{ ... }}"; tr: "test \"ad\" {{ ... }} biçiminde yazın"),
            );
            return ItemKind::Test(TestDecl {
                name,
                name_span,
                stmts: Vec::new(),
            });
        }
        let open = self.bump(); // '{'

        let mut stmts = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            if let Some(stmt) = self.parse_test_stmt() {
                stmts.push(stmt);
            }
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        self.expect_closing(RBrace, "}", open);

        ItemKind::Test(TestDecl {
            name,
            name_span,
            stmts,
        })
    }

    /// Tek test deyimi; sözdizimi bozuksa `None` döner ve `;`/`}`'ye
    /// kadar sessizce atlanır (kaskad önlemi).
    fn parse_test_stmt(&mut self) -> Option<TestStmt> {
        let start = self.pos;
        match self.current() {
            // `let dut = Counter { };`
            Some(KwLet) => {
                self.bump_any();
                if !self.at(Ident) {
                    return self.test_stmt_error(
                        &lstr!(en: "instance name after 'let'"; tr: "'let' sonrası örnek adı"),
                    );
                }
                let name = self.parse_name();
                if !self.eat(Eq) {
                    return self.test_stmt_error(&lstr!(en: "'=' after the instance name"; tr: "örnek adından sonra '='"));
                }
                if !self.at(Ident) {
                    return self.test_stmt_error(&lstr!(en: "module name after '='"; tr: "'=' sonrası modül adı"));
                }
                let module = self.parse_name();
                let brace_ok = self.eat(LBrace) && self.eat(RBrace);
                if !brace_ok {
                    return self.test_stmt_error(
                        &lstr!(en: "'{{ }}' after the module name"; tr: "modül adından sonra '{{ }}'"),
                    );
                }
                self.expect_test_semi();
                Some(TestStmt::LetDut {
                    span: self.span_from(start),
                    name,
                    module,
                })
            }
            // `reset();` — 'reset' anahtar kelimedir (tip konumunda reset
            // tipi); test gövdesinde çağrı olarak da tanınır.
            Some(KwReset) if matches!(self.peek(1), Some(LParen)) => {
                let span = self.bump(); // 'reset'
                let func = volt_ast::Name {
                    text: "reset".to_string(),
                    span,
                };
                let open = self.bump(); // '('
                self.expect_closing(RParen, ")", open);
                self.expect_test_semi();
                Some(TestStmt::Call {
                    span: self.span_from(start),
                    func,
                    args: Vec::new(),
                })
            }
            // `dut.port = v;` ya da `step(1);`
            Some(Ident) => match self.peek(1) {
                Some(Dot) => {
                    let dut = self.parse_name();
                    self.bump_any(); // '.'
                    if !self.at(Ident) {
                        return self
                            .test_stmt_error(&lstr!(en: "port name after '.'"; tr: "'.' sonrası port adı"));
                    }
                    let port = self.parse_name();
                    if !self.eat(Eq) {
                        return self
                            .test_stmt_error(&lstr!(en: "'=' after the port"; tr: "porttan sonra '='"));
                    }
                    let value = self.parse_test_expr()?;
                    self.expect_test_semi();
                    Some(TestStmt::SetPort {
                        span: self.span_from(start),
                        dut,
                        port,
                        value,
                    })
                }
                Some(LParen) => {
                    let func = self.parse_name();
                    let open = self.bump(); // '('
                    let mut args = Vec::new();
                    while !self.at(RParen) && !self.at_eof() {
                        let before = self.pos;
                        if let Some(arg) = self.parse_test_expr() {
                            args.push(arg);
                        }
                        if !self.eat(Comma) && self.pos == before {
                            break;
                        }
                    }
                    self.expect_closing(RParen, ")", open);
                    self.expect_test_semi();
                    Some(TestStmt::Call {
                        span: self.span_from(start),
                        func,
                        args,
                    })
                }
                _ => self.test_stmt_error(
                    &lstr!(en: "'.' or '(' after the name"; tr: "isimden sonra '.' veya '('"),
                ),
            },
            _ => self.test_stmt_error(
                &lstr!(en: "a test statement (let / dut.port = / a call)"; tr: "test deyimi (let / dut.port = / çağrı)"),
            ),
        }
    }

    /// `IntLit | true | false | Ident "." Ident`
    fn parse_test_expr(&mut self) -> Option<TestExpr> {
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
            Some(Ident) if matches!(self.peek(1), Some(Dot)) => {
                let dut = self.parse_name();
                self.bump_any(); // '.'
                if !self.at(Ident) {
                    self.error_expected(
                        &lstr!(en: "port name after '.'"; tr: "'.' sonrası port adı"),
                        &lstr!(en: "write it as dut.port"; tr: "dut.port biçiminde yazın"),
                    );
                    return None;
                }
                let port = self.parse_name();
                Some(TestExpr {
                    span: self.span_from(start),
                    kind: TestExprKind::PortRead { dut, port },
                })
            }
            _ => {
                self.error_expected(
                    &lstr!(en: "a test expression (literal or dut.port)"; tr: "test ifadesi (literal veya dut.port)"),
                    &lstr!(en: "allowed forms: 4, 0xA5, true, false, dut.port"; tr: "izinli biçimler: 4, 0xA5, true, false, dut.port"),
                );
                None
            }
        }
    }

    /// Deyim sonu `;` — eksikse tanı üretir ama ilerlemeyi bozmaz.
    fn expect_test_semi(&mut self) {
        if !self.eat(Semi) {
            self.error_expected(
                &lstr!(en: "';' at the end of the test statement"; tr: "test deyiminin sonunda ';'"),
                &lstr!(en: "test statements end with ';'"; tr: "test deyimleri ';' ile biter"),
            );
        }
    }

    /// Hata bildir + `;`/`}` sınırına kadar sessizce atla; `;` yenir.
    fn test_stmt_error(&mut self, what: &str) -> Option<TestStmt> {
        self.error_expected(
            what,
            &lstr!(
                en: "see 'volt explain simulation-setup' for the test syntax";
                tr: "test sözdizimi için 'volt explain simulation-setup' konusuna bakın"
            ),
        );
        while !self.at_eof() && !self.at(Semi) && !self.at(RBrace) {
            self.bump_any();
        }
        self.eat(Semi);
        None
    }
}

/// Tamsayı literal metnini değere çevirir: `_` ayırıcıları, `0x/0b/0o`
/// tabanları ve `u8`..`i64` sonekleri desteklenir.
fn parse_int_text(text: &str) -> Option<u64> {
    let mut t = text.replace('_', "");
    for suffix in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        if t.len() > suffix.len() && t.ends_with(suffix) {
            let cut = t.len() - suffix.len();
            // `0x1u8` gibi: sonek öncesi en az bir rakam kalmalı.
            if t[..cut]
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_hexdigit())
            {
                t.truncate(cut);
            }
            break;
        }
    }
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if let Some(bin) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
        u64::from_str_radix(bin, 2).ok()
    } else if let Some(oct) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
        u64::from_str_radix(oct, 8).ok()
    } else {
        t.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_int_text;

    #[test]
    fn int_text_plain_hex_bin_and_suffix() {
        assert_eq!(parse_int_text("42"), Some(42));
        assert_eq!(parse_int_text("0xA5"), Some(0xA5));
        assert_eq!(parse_int_text("0b1010"), Some(0b1010));
        assert_eq!(parse_int_text("0o17"), Some(0o17));
        assert_eq!(parse_int_text("1_000"), Some(1000));
        assert_eq!(parse_int_text("7u8"), Some(7));
        assert_eq!(parse_int_text("0xfeu8"), Some(0xfe));
    }

    #[test]
    fn int_text_rejects_garbage() {
        assert_eq!(parse_int_text("0x"), None);
        assert_eq!(parse_int_text(""), None);
    }
}
