//! `test "ad" { ... }` bloğu ayrıştırma (ADR-0033).
//!
//! `test` bağlamsal anahtar kelimedir (ADR-0023 deseni): lexer Ident
//! üretir, yalnız öğe konumunda `test <StringLit>` dizilimi test bloğu
//! başlatır. Gövde donanım değil doğrusal betiktir; deyimler `;` ile
//! biter ve modül deyim arenalarını KULLANMAZ (grammar-full.ebnf
//! TestStmt/TestExpr). ADR-0058: yerel değişken, dizi, `for` döngüsü;
//! ifade ayrıştırması `test_expr.rs`'tedir.

use volt_ast::{ItemKind, TestDecl, TestStmt};
use volt_diagnostics::lstr;

use crate::token::TokenKind::*;

use super::{Parser, MAX_DEPTH};

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
        let stmts = self.parse_test_block();

        ItemKind::Test(TestDecl {
            name,
            name_span,
            stmts,
        })
    }

    /// `{ { TestStmt } }` — `{` üzerindeyken çağrılır (test ve `for` gövdesi).
    fn parse_test_block(&mut self) -> Vec<TestStmt> {
        let open = self.bump(); // '{'
        let mut stmts = Vec::new();
        // İç içe `for` sınırı: yığın taşması yerine tanı; gövde atlanır.
        if self.depth >= MAX_DEPTH {
            self.error_expected(
                &lstr!(en: "a shallower loop nest"; tr: "daha sığ bir döngü yuvası"),
                &lstr!(en: "loops are nested too deeply"; tr: "döngüler çok derin iç içe"),
            );
            while !self.at_eof() {
                self.bump_any();
            }
            return stmts;
        }
        self.depth += 1;
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            if let Some(stmt) = self.parse_test_stmt() {
                stmts.push(stmt);
            }
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        self.depth -= 1;
        self.expect_closing(RBrace, "}", open);
        stmts
    }

    /// Tek test deyimi; sözdizimi bozuksa `None` döner ve `;`/`}`'ye
    /// kadar sessizce atlanır (kaskad önlemi).
    fn parse_test_stmt(&mut self) -> Option<TestStmt> {
        match (self.current(), self.peek(1)) {
            (Some(KwLet), _) => self.parse_test_let(),
            (Some(KwFor), _) => self.parse_test_for(),
            // 'reset' anahtar kelimedir (tip konumunda reset tipi); test
            // gövdesinde çağrı olarak da tanınır.
            (Some(KwReset), Some(LParen)) => self.parse_test_reset(),
            (Some(Ident), Some(Dot)) => self.parse_test_set_port(),
            (Some(Ident), Some(LParen)) => self.parse_test_call(),
            (Some(Ident), _) => self.test_stmt_error(
                &lstr!(en: "'.' or '(' after the name"; tr: "isimden sonra '.' veya '('"),
            ),
            _ => self.test_stmt_error(
                &lstr!(en: "a test statement (let / for / dut.port = / a call)"; tr: "test deyimi (let / for / dut.port = / çağrı)"),
            ),
        }
    }

    /// `let dut = Counter { };` ya da `let ad = <ifade>;` (ADR-0058).
    fn parse_test_let(&mut self) -> Option<TestStmt> {
        let start = self.pos;
        self.bump_any(); // 'let'
        if !self.at(Ident) {
            return self.test_stmt_error(
                &lstr!(en: "instance name after 'let'"; tr: "'let' sonrası örnek adı"),
            );
        }
        let name = self.parse_name();
        if !self.eat(Eq) {
            return self.test_stmt_error(
                &lstr!(en: "'=' after the instance name"; tr: "örnek adından sonra '='"),
            );
        }
        // `Ad {` → DUT örnekleme; diğer her şey yerel değişken.
        if !(self.at(Ident) && matches!(self.peek(1), Some(LBrace))) {
            let value = self.parse_test_expr()?;
            self.expect_test_semi();
            return Some(TestStmt::LetVar {
                span: self.span_from(start),
                name,
                value,
            });
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

    /// `for i in 0..16 { ... }` — çalışma zamanı döngüsü (ADR-0058).
    fn parse_test_for(&mut self) -> Option<TestStmt> {
        let start = self.pos;
        self.bump_any(); // 'for'
        if !self.at(Ident) {
            return self.test_for_error(
                &lstr!(en: "loop variable after 'for'"; tr: "'for' sonrası döngü değişkeni"),
            );
        }
        let var = self.parse_name();
        if !self.eat(KwIn) {
            return self.test_for_error(
                &lstr!(en: "'in' after the loop variable"; tr: "döngü değişkeninden sonra 'in'"),
            );
        }
        let Some(range_start) = self.parse_test_expr() else {
            return self.skip_test_for_body();
        };
        if !self.eat(DotDot) {
            return self
                .test_for_error(&lstr!(en: "'..' in the loop range"; tr: "döngü aralığında '..'"));
        }
        let Some(range_end) = self.parse_test_expr() else {
            return self.skip_test_for_body();
        };
        if !self.at(LBrace) {
            return self.test_for_error(
                &lstr!(en: "'{{' after the loop range"; tr: "döngü aralığından sonra '{{'"),
            );
        }
        let body = self.parse_test_block();
        Some(TestStmt::For {
            span: self.span_from(start),
            var,
            start: range_start,
            end: range_end,
            body,
        })
    }

    /// `reset();`
    fn parse_test_reset(&mut self) -> Option<TestStmt> {
        let start = self.pos;
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

    /// `dut.port = v;`
    fn parse_test_set_port(&mut self) -> Option<TestStmt> {
        let start = self.pos;
        let dut = self.parse_name();
        self.bump_any(); // '.'
        if !self.at(Ident) {
            return self
                .test_stmt_error(&lstr!(en: "port name after '.'"; tr: "'.' sonrası port adı"));
        }
        let port = self.parse_name();
        if !self.eat(Eq) {
            return self.test_stmt_error(&lstr!(en: "'=' after the port"; tr: "porttan sonra '='"));
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

    /// `step(1);`, `assert_eq(a, b);`, `load(dut.mem, data);`
    fn parse_test_call(&mut self) -> Option<TestStmt> {
        let start = self.pos;
        let func = self.parse_name();
        let open = self.bump(); // '('
        let args = self.parse_test_expr_list(RParen);
        self.expect_closing(RParen, ")", open);
        self.expect_test_semi();
        Some(TestStmt::Call {
            span: self.span_from(start),
            func,
            args,
        })
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

    /// `for` başlığı bozuk: hata bildir, gövde `{ ... }` varsa onu da
    /// tüket — yoksa gövdenin `}`'si test bloğunu erken kapatırdı.
    fn test_for_error(&mut self, what: &str) -> Option<TestStmt> {
        self.error_expected(
            what,
            &lstr!(
                en: "write the loop as: for i in 0..16 {{ ... }}";
                tr: "döngüyü şöyle yazın: for i in 0..16 {{ ... }}"
            ),
        );
        self.skip_test_for_body()
    }

    /// Bozuk `for`'un kalanını atlar (tanı zaten üretildi): gövde varsa
    /// ayrıştırılıp atılır, yoksa `;`'e kadar gidilir.
    fn skip_test_for_body(&mut self) -> Option<TestStmt> {
        while !self.at_eof() && !self.at(LBrace) && !self.at(Semi) && !self.at(RBrace) {
            self.bump_any();
        }
        if self.at(LBrace) {
            self.parse_test_block();
        } else {
            self.eat(Semi);
        }
        None
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
pub(super) fn parse_int_text(text: &str) -> Option<u64> {
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
