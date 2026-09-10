//! Pratt ifade parser'ı.
//!
//! Binding power tablosu: docs/spec/operator-precedence.md §3 (BAĞLAYICI).
//! W0010 parantez önerisi: operator-precedence.md §5.

use volt_ast::{
    ArrayLitKind, BinOp, Expr, ExprKind, FieldInit, Idx, IntSuffix, Name, NumBase, Path, UnOp,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::token::TokenKind;
use crate::token::TokenKind::*;

use super::{Parser, MAX_DEPTH};

/// (left_bp, right_bp) — operator-precedence.md §3 değerleri.
fn infix_binding_power(op: BinOp) -> (u8, u8) {
    match op {
        // SAĞ birleşmeli (r_bp < l_bp): a -> b -> c ≡ a -> (b -> c).
        // l_bp=1 Or ile eşit ama çakışmaz: sağ operand min_bp=0 ile
        // ayrıştırıldığından || implikasyondan sıkı bağlanır (ADR-0034).
        BinOp::Imp => (1, 0),
        BinOp::Or => (1, 2),
        BinOp::And => (3, 4),
        BinOp::Eq | BinOp::Ne => (5, 5), // BİRLEŞMEZ
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => (7, 7), // BİRLEŞMEZ
        BinOp::BitOr => (9, 10),
        BinOp::BitXor => (11, 12),
        BinOp::BitAnd => (13, 14),
        BinOp::Shl | BinOp::Shr => (15, 16),
        BinOp::Add | BinOp::Sub => (17, 18),
        BinOp::Mul | BinOp::Div | BinOp::Rem => (19, 20),
    }
}

const PREFIX_BP: u8 = 21;
const POSTFIX_BP: u8 = 23;

/// `bits<N>` argümanı ve blok atama LHS'i karşılaştırma tüketmesin diye
/// karşılaştırmanın hemen üstündeki taban güç.
pub(crate) const ABOVE_COMPARISON_BP: u8 = 8;

/// Generic argüman ifadeleri: `Fifo<Entry<8>>` içindeki `>>` kaydırma
/// operatörü DEĞİL, iki kapanıştır — kaydırmanın üstündeki taban güç.
/// Kaydırma gereken argümanlar parantezlenir: `bits<(1 << N)>`.
pub(crate) const ABOVE_SHIFT_BP: u8 = 16;

fn token_binop(kind: TokenKind) -> Option<BinOp> {
    Some(match kind {
        // İfade bağlamında '->' her zaman implikasyondur; fn dönüş
        // tipindeki '->' item parser'ında imza konumunda tüketilir,
        // bu döngüye hiç ulaşmaz (ADR-0034).
        Arrow => BinOp::Imp,
        PipePipe => BinOp::Or,
        AmpAmp => BinOp::And,
        EqEq => BinOp::Eq,
        NotEq => BinOp::Ne,
        Lt => BinOp::Lt,
        Gt => BinOp::Gt,
        Le => BinOp::Le,
        Ge => BinOp::Ge,
        Pipe => BinOp::BitOr,
        Caret => BinOp::BitXor,
        Amp => BinOp::BitAnd,
        Shl => BinOp::Shl,
        Shr => BinOp::Shr,
        Plus => BinOp::Add,
        Minus => BinOp::Sub,
        Star => BinOp::Mul,
        Slash => BinOp::Div,
        Percent => BinOp::Rem,
        _ => return None,
    })
}

/// W0010 çiftleri (operator-precedence.md §5): parantezsiz karışımda
/// okuyucu önceliği sorgular — uyarı ver.
fn w0010_pair(parent: BinOp, child: BinOp) -> Option<(&'static str, &'static str)> {
    match (parent, child) {
        (BinOp::BitOr, BinOp::BitAnd) | (BinOp::BitAnd, BinOp::BitOr) => Some(("&", "|")),
        (BinOp::Shl, BinOp::Add) | (BinOp::Add, BinOp::Shl) => Some(("<<", "+")),
        _ => None,
    }
}

impl Parser<'_> {
    pub(crate) fn parse_expr(&mut self) -> Idx<Expr> {
        self.parse_expr_bp(0)
    }

    pub(crate) fn parse_expr_bp(&mut self, min_bp: u8) -> Idx<Expr> {
        // Derinlik sınırı: patolojik iç içelikte yığın taşması yerine tanı
        if self.depth >= MAX_DEPTH {
            let span = self.bump(); // ilerleme garantisi
            self.push_error(Diagnostic::error(
                ErrorCode::E0001,
                lstr!(en: "expression is nested too deeply"; tr: "ifade çok derin iç içe"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "nesting depth limit exceeded"; tr: "derinlik sınırı aşıldı"),
                ),
                lstr!(en: "split the expression using intermediate let bindings"; tr: "ifadeyi ara let bağlamalarıyla bölün"),
            ));
            return self.alloc_error_expr(span);
        }
        self.depth += 1;
        let result = self.parse_expr_bp_inner(min_bp);
        self.depth -= 1;
        result
    }

    fn parse_expr_bp_inner(&mut self, min_bp: u8) -> Idx<Expr> {
        let start = self.pos;
        let mut lhs = self.parse_prefix();
        let mut lhs_is_comparison = false;

        while let Some(kind) = self.current() {
            // ─── Postfix: [] . () as — bp 23 ───
            if matches!(kind, LBracket | Dot | LParen | KwAs) {
                if POSTFIX_BP < min_bp {
                    break;
                }
                lhs = self.parse_postfix(lhs, start);
                lhs_is_comparison = false;
                continue;
            }

            // ─── İkili operatörler ───
            let Some(op) = token_binop(kind) else { break };
            let (l_bp, r_bp) = infix_binding_power(op);
            if l_bp < min_bp {
                break;
            }

            // E0010: karşılaştırma zinciri (operator-precedence.md §2.1)
            if op.is_comparison() && lhs_is_comparison {
                self.push_error(
                    Diagnostic::error(
                        ErrorCode::E0010,
                        lstr!(en: "comparison operators cannot be chained"; tr: "karşılaştırma operatörleri zincirlenemez"),
                        LabeledSpan::primary(
                            self.current_span(),
                            lstr!(en: "second comparison"; tr: "ikinci karşılaştırma"),
                        ),
                        lstr!(en: "write it as (a < b) && (b < c)"; tr: "(a < b) && (b < c) biçiminde yazın"),
                    )
                    .with_note(
                        NoteKind::Reason,
                        lstr!(en: "a < b < c is a mathematical trap: the 0/1 result of (a<b) is compared against c"; tr: "a < b < c matematiksel yanılgı üretir: (a<b) sonucu 0/1 olarak c ile karşılaştırılır"),
                    ),
                );
            }

            let op_span = self.current_span();
            self.bump_any(); // operatörü tüket

            // Birleşmez operatörlerde eşit bp'li zincir bu döngüye geri
            // dönmeli ki E0010 tespit edilebilsin → r_bp + 1
            let effective_r_bp = if op.is_comparison() { r_bp + 1 } else { r_bp };

            // Sağ operand eksikse Error üret ama DURMA (error-recovery.md §4.3)
            let rhs = if self.at_expr_start() {
                self.parse_expr_bp(effective_r_bp)
            } else {
                self.error_expected(
                    &lstr!(en: "expression after the '{}' operator", op.symbol(); tr: "'{}' operatöründen sonra ifade", op.symbol()),
                    &lstr!(en: "write an operand to the right of the operator"; tr: "operatörün sağına bir operand yazın"),
                );
                self.alloc_error_expr(self.current_span())
            };

            self.check_w0010(op, op_span, lhs, rhs);

            let span = self.span_from(start);
            lhs = self.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::Binary { op, lhs, rhs },
            });
            lhs_is_comparison = op.is_comparison();
        }

        lhs
    }

    /// W0010 (operator-precedence.md §5): '&' ile '|' veya '<<' ile '+'
    /// parantezsiz karışıyorsa mevcut yorumu gösteren uyarı üret.
    fn check_w0010(&mut self, op: BinOp, op_span: Span, lhs: Idx<Expr>, rhs: Idx<Expr>) {
        for child in [lhs, rhs] {
            if self.paren_exprs.contains(&child) {
                continue; // kullanıcı zaten parantezledi
            }
            let ExprKind::Binary { op: child_op, .. } = self.ast.exprs[child].kind else {
                continue;
            };
            let Some((a, b)) = w0010_pair(op, child_op) else {
                continue;
            };
            // Mevcut yorum: daha sıkı bağlanan (çocuk) ifade parantezlenir.
            let child_text = self.text_of(self.ast.exprs[child].span).to_string();
            let other = if child == lhs { rhs } else { lhs };
            let other_text = self.text_of(self.ast.exprs[other].span).to_string();
            let interpretation = if child == lhs {
                format!("({child_text}) {} {other_text}", op.symbol())
            } else {
                format!("{other_text} {} ({child_text})", op.symbol())
            };
            self.push_error(
                Diagnostic::warning(
                    ErrorCode::W0010,
                    lstr!(en: "'{a}' and '{b}' are mixed — parentheses recommended"; tr: "'{a}' ile '{b}' karışıyor — parantez önerilir"),
                    LabeledSpan::primary(
                        op_span,
                        lstr!(en: "clarify the precedence with parentheses"; tr: "önceliği parantezle netleştirin"),
                    ),
                    lstr!(en: "write {interpretation} (current interpretation)"; tr: "{interpretation} yazın (mevcut yorum)"),
                )
                .with_note(
                    NoteKind::Reason,
                    lstr!(en: "even when the precedence is correct, readers will doubt it"; tr: "öncelik doğru olsa bile okuyucu bundan şüphe eder"),
                ),
            );
            return; // düğüm başına tek uyarı yeter
        }
    }

    pub(crate) fn at_expr_start(&self) -> bool {
        matches!(
            self.current(),
            Some(
                IntLit
                    | KwTrue
                    | KwFalse
                    | Ident
                    | LParen
                    | LBracket
                    | KwIf
                    | KwMatch
                    | Bang
                    | Tilde
                    | Minus
                    | StringLit
                    | KwTodo
                    | InvalidNumber
                    | Reserved
            )
        )
    }

    fn parse_prefix(&mut self) -> Idx<Expr> {
        let start = self.pos;
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
            Some(Ident) => {
                let path = self.parse_path();
                // StructLit: `Ad { alan: değer }` — başlık bağlamlarında
                // ('if x {' gibi) allow_struct_lit=false ile bastırılır.
                if self.at(LBrace) && self.allow_struct_lit {
                    return self.parse_struct_lit(path, start);
                }
                let span = self.span_from(start);
                self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::Path(path),
                })
            }
            Some(Bang) | Some(Tilde) | Some(Minus) => {
                let op = match self.current() {
                    Some(Bang) => UnOp::Not,
                    Some(Tilde) => UnOp::BitNot,
                    _ => UnOp::Neg,
                };
                self.bump_any();
                let operand = if self.at_expr_start() {
                    self.parse_expr_bp(PREFIX_BP)
                } else {
                    self.error_expected(
                        &lstr!(en: "expression after the '{}' operator", op.symbol(); tr: "'{}' operatöründen sonra ifade", op.symbol()),
                        &lstr!(en: "write an operand to the right of the unary operator"; tr: "tekli operatörün sağına bir operand yazın"),
                    );
                    self.alloc_error_expr(self.current_span())
                };
                let span = self.span_from(start);
                self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::Unary { op, operand },
                })
            }
            Some(LParen) => self.parse_paren_or_tuple(start),
            Some(LBracket) => self.parse_array_lit(start),
            Some(KwIf) => self.parse_if_expr(),
            Some(KwMatch) => self.parse_match_expr(),
            Some(KwTodo) => self.parse_todo_expr(),
            Some(StringLit) => {
                let span = self.bump();
                let text = self.unescape_string(span);
                self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::StringLit(text),
                })
            }
            Some(InvalidNumber) | Some(Reserved) => {
                // Lexer bu tokenlar için zaten tanı üretti (E0005/E0003);
                // kaskad hata üretmeden Error düğümüyle devam et.
                let span = self.bump();
                self.alloc_error_expr(span)
            }
            _ => {
                self.error_expected(
                    &lstr!(en: "expression"; tr: "ifade"),
                    &lstr!(en: "expected a literal, name, '(' or unary operator"; tr: "literal, isim, '(' veya tekli operatör bekleniyor"),
                );
                self.alloc_error_expr(self.current_span())
            }
        }
    }

    /// `(ifade)` gruplama veya `(a, b)` tuple literali.
    fn parse_paren_or_tuple(&mut self, start: usize) -> Idx<Expr> {
        let open = self.bump(); // '('
        let prev = self.allow_struct_lit;
        self.allow_struct_lit = true; // parantez içinde yasak kalkar

        let inner = if self.at_expr_start() {
            self.parse_expr()
        } else {
            self.error_expected(
                &lstr!(en: "expression inside the parentheses"; tr: "parantez içinde ifade"),
                &lstr!(en: "empty parentheses are not valid"; tr: "boş parantez geçersizdir"),
            );
            let e = self.alloc_error_expr(self.current_span());
            self.allow_struct_lit = prev;
            self.expect(
                RParen,
                &lstr!(en: "closing ')'"; tr: "kapanış ')'"),
                &lstr!(en: "add the missing ')'"; tr: "eksik ')' ekleyin"),
            );
            return e;
        };

        if self.at(Comma) {
            // TupleLit: "(" Expr "," [ Expr { "," Expr } ] [ "," ] ")"
            let mut elems = vec![inner];
            while self.eat(Comma) {
                if self.at(RParen) {
                    break; // sondaki virgül
                }
                if self.at_expr_start() {
                    elems.push(self.parse_expr());
                } else {
                    self.error_expected(
                        &lstr!(en: "tuple element"; tr: "tuple elemanı"),
                        &lstr!(en: "write it as (a, b)"; tr: "(a, b) biçiminde yazın"),
                    );
                    break;
                }
            }
            self.allow_struct_lit = prev;
            self.expect_closing(RParen, ")", open);
            let span = self.span_from(start);
            return self.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::TupleLit(elems),
            });
        }

        self.allow_struct_lit = prev;
        self.expect(
            RParen,
            &lstr!(en: "closing ')'"; tr: "kapanış ')'"),
            &lstr!(en: "add the missing ')'"; tr: "eksik ')' ekleyin"),
        );
        // W0010 parantez önerisi bu ifadeyi atlasın diye işaretle.
        self.paren_exprs.insert(inner);
        inner
    }

    /// `[a, b, c]` liste veya `[değer; adet]` tekrar literali.
    fn parse_array_lit(&mut self, start: usize) -> Idx<Expr> {
        let open = self.bump(); // '['
        let prev = self.allow_struct_lit;
        self.allow_struct_lit = true;

        let kind = if self.at(RBracket) {
            ArrayLitKind::List(Vec::new())
        } else if self.at_expr_start() {
            let first = self.parse_expr();
            if self.eat(Semi) {
                let count = if self.at_expr_start() {
                    self.parse_expr()
                } else {
                    self.error_expected(
                        &lstr!(en: "repeat count"; tr: "tekrar sayısı"),
                        &lstr!(en: "write it as [0; 4]"; tr: "[0; 4] biçiminde yazın"),
                    );
                    self.alloc_error_expr(self.current_span())
                };
                ArrayLitKind::Repeat {
                    value: first,
                    count,
                }
            } else {
                let mut elems = vec![first];
                while self.eat(Comma) {
                    if self.at(RBracket) {
                        break; // sondaki virgül
                    }
                    if self.at_expr_start() {
                        elems.push(self.parse_expr());
                    } else {
                        self.error_expected(
                            &lstr!(en: "array element"; tr: "dizi elemanı"),
                            &lstr!(en: "write it as [a, b, c]"; tr: "[a, b, c] biçiminde yazın"),
                        );
                        break;
                    }
                }
                ArrayLitKind::List(elems)
            }
        } else {
            self.error_expected(
                &lstr!(en: "array literal"; tr: "dizi literali"),
                &lstr!(en: "write it as [a, b] or [value; count]"; tr: "[a, b] veya [değer; adet] biçiminde yazın"),
            );
            ArrayLitKind::List(Vec::new())
        };

        self.allow_struct_lit = prev;
        self.expect_closing(RBracket, "]", open);
        let span = self.span_from(start);
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::ArrayLit(kind),
        })
    }

    /// `Ad { alan: değer, kısayol }` — yapı literali. `let` konumunda
    /// [N3] kuralıyla InstanceDecl'e yeniden sınıflandırılabilir.
    fn parse_struct_lit(&mut self, path: Path, start: usize) -> Idx<Expr> {
        let open = self.bump(); // '{'
        let prev = self.allow_struct_lit;
        self.allow_struct_lit = true;

        let mut fields = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            if self.at(Ident) {
                let fstart = self.pos;
                let name = self.parse_name();
                let value = if self.eat(Colon) {
                    if self.at_expr_start() {
                        Some(self.parse_expr())
                    } else {
                        self.error_expected(
                            &lstr!(en: "field value"; tr: "alan değeri"),
                            &lstr!(en: "write it as field: expression"; tr: "alan: ifade biçiminde yazın"),
                        );
                        Some(self.alloc_error_expr(self.current_span()))
                    }
                } else {
                    None // `Foo { x }` kısayolu
                };
                fields.push(FieldInit {
                    span: self.span_from(fstart),
                    name,
                    value,
                });
            } else {
                self.error_expected(
                    &lstr!(en: "field name"; tr: "alan adı"),
                    &lstr!(en: "write it as Name {{ field: value }}"; tr: "Ad {{ alan: değer }} biçiminde yazın"),
                );
            }
            if !self.eat(Comma) && self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.allow_struct_lit = prev;
        self.expect_closing(RBrace, "}", open);
        let span = self.span_from(start);
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::StructLit { path, fields },
        })
    }

    /// `match x { desen => ifade, ... }` — ifade konumunda kollar yalnız
    /// ifade gövdesi alır (grammar §13 MatchArmExpr).
    fn parse_match_expr(&mut self) -> Idx<Expr> {
        let start = self.pos;
        self.bump_any(); // 'match'

        let scrutinee = if self.at_expr_start() {
            self.parse_expr_no_struct_lit()
        } else {
            self.error_expected(
                &lstr!(en: "match scrutinee"; tr: "match konusu"),
                &lstr!(en: "write it as match x {{ pattern => value }}"; tr: "match x {{ desen => değer }} biçiminde yazın"),
            );
            self.alloc_error_expr(self.current_span())
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the match body"; tr: "match gövdesi için '{{'"),
            &lstr!(en: "write it as match x {{ ... }}"; tr: "match x {{ ... }} biçiminde yazın"),
        );

        let mut arms = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            arms.push(self.parse_match_arm(None));
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.expect_closing(RBrace, "}", open);
        let span = self.span_from(start);
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::Match { scrutinee, arms },
        })
    }

    /// `todo!` veya `todo!("mesaj")` — tip kontrolünden geçer, sim'de durur.
    fn parse_todo_expr(&mut self) -> Idx<Expr> {
        let start = self.pos;
        self.bump_any(); // 'todo'
        self.expect(
            Bang,
            &lstr!(en: "'!' after 'todo'"; tr: "'todo' sonrası '!'"),
            &lstr!(en: "write it as todo!(\"message\")"; tr: "todo!(\"mesaj\") biçiminde yazın"),
        );

        let mut message = None;
        if self.at(LParen) {
            let open = self.bump();
            if self.at(StringLit) {
                let span = self.bump();
                message = Some(self.unescape_string(span));
            } else if !self.at(RParen) {
                self.error_expected(
                    &lstr!(en: "todo! message"; tr: "todo! mesajı"),
                    &lstr!(en: "todo!(\"message\") — it only accepts a string"; tr: "todo!(\"mesaj\") — yalnız string alır"),
                );
                // bozuk argümanı atla
                while !self.at(RParen) && !self.at_eof() && !self.at(RBrace) {
                    self.bump_any();
                }
            }
            self.expect_closing(RParen, ")", open);
        }

        let span = self.span_from(start);
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::Todo { message },
        })
    }

    /// String literalin tırnaklarını söker, escape dizilerini çözer.
    /// Geçersiz escape → E0012 (grammar §14: \" \\ \n \t \r \0).
    fn unescape_string(&mut self, span: Span) -> String {
        let raw = self.text_of(span);
        let body = &raw[1..raw.len().saturating_sub(1)];
        let mut out = String::with_capacity(body.len());
        let mut chars = body.char_indices();
        while let Some((i, c)) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some((_, 'r')) => out.push('\r'),
                Some((_, '0')) => out.push('\0'),
                Some((j, other)) => {
                    let esc_start = span.start + 1 + i as u32;
                    let esc_end = span.start + 1 + j as u32 + other.len_utf8() as u32;
                    self.push_error(Diagnostic::error(
                        ErrorCode::E0012,
                        lstr!(en: "invalid escape sequence: '\\{other}'"; tr: "geçersiz escape dizisi: '\\{other}'"),
                        LabeledSpan::primary(
                            Span::new(span.file, esc_start, esc_end),
                            lstr!(en: "unknown escape"; tr: "tanınmayan escape"),
                        ),
                        lstr!(en: "valid escape sequences: \\\" \\\\ \\n \\t \\r \\0"; tr: "geçerli escape dizileri: \\\" \\\\ \\n \\t \\r \\0"),
                    ));
                    out.push(other);
                }
                None => {}
            }
        }
        out
    }

    /// `[i]`, `[hi:lo]`, `.alan`, `(args)`, `as Tip` — bp 23.
    fn parse_postfix(&mut self, base: Idx<Expr>, start: usize) -> Idx<Expr> {
        match self.current() {
            Some(LBracket) => {
                let open = self.bump();
                let prev = self.allow_struct_lit;
                self.allow_struct_lit = true;
                let first = self.parse_expr();
                let kind = if self.eat(Colon) {
                    let lo = self.parse_expr();
                    ExprKind::Range {
                        base,
                        hi: first,
                        lo,
                    }
                } else if self.eat(PlusColon) {
                    let width = self.parse_expr();
                    ExprKind::PartSelect {
                        base,
                        start: first,
                        width,
                        ascending: true,
                    }
                } else if self.eat(MinusColon) {
                    let width = self.parse_expr();
                    ExprKind::PartSelect {
                        base,
                        start: first,
                        width,
                        ascending: false,
                    }
                } else {
                    ExprKind::Index { base, index: first }
                };
                self.allow_struct_lit = prev;
                self.expect_closing(RBracket, "]", open);
                let span = self.span_from(start);
                self.ast.exprs.alloc(Expr { span, kind })
            }
            Some(Dot) => {
                self.bump_any();
                let field = if self.at(Ident) {
                    self.parse_name()
                } else {
                    self.error_expected(
                        &lstr!(en: "field name after '.'"; tr: "'.' sonrasında alan adı"),
                        &lstr!(en: "write it as x.field"; tr: "x.alan biçiminde yazın"),
                    );
                    Name {
                        text: String::new(),
                        span: self.current_span(),
                    }
                };
                let span = self.span_from(start);
                self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::Field { base, field },
                })
            }
            Some(LParen) => {
                let open = self.bump();
                let prev = self.allow_struct_lit;
                self.allow_struct_lit = true;
                let mut args = Vec::new();
                while !self.at(RParen) && !self.at_eof() {
                    let before = self.pos;
                    args.push(self.parse_expr());
                    if !self.eat(Comma) {
                        break;
                    }
                    if self.pos == before {
                        self.bump_any(); // ilerleme garantisi
                    }
                }
                self.allow_struct_lit = prev;
                self.expect_closing(RParen, ")", open);
                let span = self.span_from(start);
                self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::Call { callee: base, args },
                })
            }
            _ => {
                // KwAs
                self.bump_any();
                let ty = self.parse_type_or_error();
                let span = self.span_from(start);
                self.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::Cast { expr: base, ty },
                })
            }
        }
    }

    /// `if cond { expr } else { expr }` — else ZORUNLU (E0008).
    fn parse_if_expr(&mut self) -> Idx<Expr> {
        let start = self.pos;
        self.bump_any(); // if
        let cond = self.parse_expr_no_struct_lit();
        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the 'if' body"; tr: "'if' gövdesi için '{{'"),
            &lstr!(en: "write it as if cond {{ value }}"; tr: "if koşul {{ deger }} biçiminde yazın"),
        );
        let then_expr = self.parse_expr();
        self.expect_closing(RBrace, "}", open);

        let else_expr = if self.eat(KwElse) {
            if self.at(KwIf) {
                self.parse_if_expr()
            } else {
                let open = self.current_span();
                self.expect(
                    LBrace,
                    &lstr!(en: "'{{' for the 'else' body"; tr: "'else' gövdesi için '{{'"),
                    &lstr!(en: "write it as else {{ value }}"; tr: "else {{ deger }} biçiminde yazın"),
                );
                let e = self.parse_expr();
                self.expect_closing(RBrace, "}", open);
                e
            }
        } else {
            // E0008 — eksik else latch riski (error-recovery.md §6.3)
            self.push_error(
                Diagnostic::error(
                    ErrorCode::E0008,
                    lstr!(en: "'if' expression requires an 'else' branch"; tr: "'if' ifadesinde 'else' dalı zorunlu"),
                    LabeledSpan::primary(
                        self.span_from(start),
                        lstr!(en: "missing else branch"; tr: "else dalı eksik"),
                    ),
                    lstr!(en: "add else {{ default_value }}"; tr: "else {{ varsayilan_deger }} ekleyin"),
                )
                .with_note(
                    NoteKind::Reason,
                    lstr!(en: "a missing branch produces a latch in hardware"; tr: "eksik dal donanımda latch üretir"),
                ),
            );
            self.alloc_error_expr(self.current_span())
        };

        let span = self.span_from(start);
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::If {
                cond,
                then_expr,
                else_expr,
            },
        })
    }

    pub(crate) fn parse_name(&mut self) -> Name {
        let span = self.bump();
        Name {
            text: self.text_of(span).to_string(),
            span,
        }
    }

    pub(crate) fn parse_path(&mut self) -> Path {
        let start = self.pos;
        let mut segments = vec![self.parse_name()];
        while self.at(ColonColon) && matches!(self.peek(1), Some(Ident)) {
            self.bump_any();
            segments.push(self.parse_name());
        }
        Path {
            span: self.span_from(start),
            segments,
        }
    }

    pub(crate) fn parse_int_lit(&mut self) -> Idx<Expr> {
        let span = self.bump();
        let text = self.text_of(span);
        let (value, suffix, base) = parse_int_text(text);
        match value {
            Some(value) => self.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::IntLit {
                    value,
                    suffix,
                    base,
                },
            }),
            None => {
                self.push_error(Diagnostic::error(
                    ErrorCode::E0005,
                    lstr!(en: "invalid numeric literal: '{text}'"; tr: "geçersiz sayısal literal: '{text}'"),
                    LabeledSpan::primary(
                        span,
                        lstr!(en: "value does not fit in 128 bits"; tr: "değer 128 bite sığmıyor"),
                    ),
                    lstr!(en: "use a smaller constant"; tr: "daha küçük bir sabit kullanın"),
                ));
                self.alloc_error_expr(span)
            }
        }
    }
}

/// Literal metnini (taban, sonek, değer) üçlüsüne çözer.
/// Lexer biçimi doğruladı; None yalnızca taşmada döner.
fn parse_int_text(text: &str) -> (Option<u128>, Option<IntSuffix>, NumBase) {
    const SUFFIXES: [(&str, IntSuffix); 8] = [
        ("u16", IntSuffix::U16),
        ("u32", IntSuffix::U32),
        ("u64", IntSuffix::U64),
        ("i16", IntSuffix::I16),
        ("i32", IntSuffix::I32),
        ("i64", IntSuffix::I64),
        ("u8", IntSuffix::U8),
        ("i8", IntSuffix::I8),
    ];

    let mut body = text;
    let mut suffix = None;
    for (s, sfx) in SUFFIXES {
        if let Some(stripped) = body.strip_suffix(s) {
            body = stripped;
            suffix = Some(sfx);
            break;
        }
    }

    let (digits, base, radix) = if let Some(rest) = body.strip_prefix("0x") {
        (rest, NumBase::Hex, 16)
    } else if let Some(rest) = body.strip_prefix("0b") {
        (rest, NumBase::Bin, 2)
    } else if let Some(rest) = body.strip_prefix("0o") {
        (rest, NumBase::Oct, 8)
    } else {
        (body, NumBase::Dec, 10)
    };

    let cleaned: String = digits.chars().filter(|&c| c != '_').collect();
    (u128::from_str_radix(&cleaned, radix).ok(), suffix, base)
}
