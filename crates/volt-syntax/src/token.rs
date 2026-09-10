//! Volt token türleri.
//!
//! Referans: docs/spec/grammar-full.ebnf §14-§17.

use logos::Logos;

use crate::span::Span;

/// Span taşıyan tek bir token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Sözcüksel token türleri. Metin, span üzerinden kaynaktan alınır.
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // ─── Trivia (varsayılan akışta atlanır, span'i korunur) ───
    #[regex(r"[ \t\r\n]+")]
    Whitespace,

    #[regex(r"//([^/!\n][^\n]*)?")]
    LineComment,

    /// `/* ... */` — iç içe desteklenir; alan: kapanış bulundu mu?
    #[token("/*", lex_block_comment)]
    BlockComment(bool),

    // ─── Doc yorumları (gramer öğesi, akışta kalır) ───
    #[regex(r"///[^\n]*")]
    DocComment,

    #[regex(r"//![^\n]*")]
    InnerDocComment,

    // ─── Literaller ───
    #[regex(
        r"(0x[0-9a-fA-F][0-9a-fA-F_]*|0b[01][01_]*|0o[0-7][0-7_]*|[0-9][0-9_]*)(u8|u16|u32|u64|i8|i16|i32|i64)?",
        priority = 6
    )]
    IntLit,

    /// Radix öneki var ama gövdesi geçersiz: `0x`, `0b12`, `0o9` → E0005
    #[regex(r"0[xbo][0-9a-zA-Z_]*", priority = 3)]
    InvalidNumber,

    #[regex(r#""([^"\\\n]|\\[^\n])*""#)]
    StringLit,

    // ─── Kimlik ───
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", priority = 2)]
    Ident,

    // ─── Aktif anahtar kelimeler (§17) ───
    #[token("module")]
    KwModule,
    #[token("in")]
    KwIn,
    #[token("out")]
    KwOut,
    #[token("inout")]
    KwInout,
    #[token("reg")]
    KwReg,
    #[token("let")]
    KwLet,
    #[token("wire")]
    KwWire,
    #[token("on")]
    KwOn,
    #[token("comb")]
    KwComb,
    #[token("domain")]
    KwDomain,
    #[token("if")]
    KwIf,
    #[token("else")]
    KwElse,
    #[token("match")]
    KwMatch,
    #[token("for")]
    KwFor,
    #[token("as")]
    KwAs,
    #[token("fn")]
    KwFn,
    #[token("const")]
    KwConst,
    #[token("type")]
    KwType,
    #[token("struct")]
    KwStruct,
    #[token("enum")]
    KwEnum,
    #[token("extern")]
    KwExtern,
    #[token("package")]
    KwPackage,
    #[token("use")]
    KwUse,
    #[token("pub")]
    KwPub,
    #[token("bool")]
    KwBool,
    #[token("clock")]
    KwClock,
    #[token("reset")]
    KwReset,
    #[token("bits")]
    KwBits,
    #[token("Trit")]
    KwTrit,
    /// `u1`..`uN` — keyfi genişlikli işaretsiz tip ailesi (ADR-0031).
    /// Genişlik sınırı (1..=64) parser'da denetlenir; `u8x` gibi daha
    /// uzun Ident eşleşmeleri logos'un en-uzun-eşleşme kuralıyla kazanır.
    #[regex(r"u[1-9][0-9]*", priority = 3)]
    UIntType,
    /// `i1`..`iN` — keyfi genişlikli işaretli tip ailesi (ADR-0031).
    #[regex(r"i[1-9][0-9]*", priority = 3)]
    SIntType,
    #[token("true")]
    KwTrue,
    #[token("false")]
    KwFalse,
    #[token("todo")]
    KwTodo,
    #[token("requires")]
    KwRequires,
    #[token("ensures")]
    KwEnsures,
    #[token("invariant")]
    KwInvariant,
    #[token("cover")]
    KwCover,
    #[token("assert")]
    KwAssert,
    #[token("assume")]
    KwAssume,
    #[token("posedge")]
    KwPosedge,
    #[token("negedge")]
    KwNegedge,
    #[token("none")]
    KwNone,
    // 'sync'/'async' bağlamsal anahtar kelimedir (ADR-0023):
    // lexer Ident üretir, parser yalnız "reset =" konumunda yorumlar.
    #[token("active_high")]
    KwActiveHigh,
    #[token("active_low")]
    KwActiveLow,

    // ─── Ayrılmış anahtar kelimeler (§17, kullanımı E0003) ───
    // ADR-0028: fifo/ram/regfile/arbiter stdlib bileşeni oldu, listeden
    // çıkarıldı — normal Ident olarak tanınırlar.
    #[token("pipeline")]
    #[token("fsm")]
    #[token("stage")]
    #[token("stall")]
    #[token("flush")]
    #[token("hook")]
    #[token("impl")]
    #[token("trait")]
    #[token("where")]
    #[token("Spike")]
    #[token("PTrit")]
    #[token("Option")]
    #[token("Some")]
    #[token("None")]
    #[token("Result")]
    #[token("Ok")]
    #[token("Err")]
    #[token("spike")]
    #[token("self")]
    #[token("Self")]
    #[token("mut")]
    #[token("ref")]
    #[token("move")]
    #[token("secret")]
    #[token("confidential")]
    #[token("public")]
    #[token("clamp_low")]
    #[token("clamp_high")]
    #[token("latch")]
    #[token("retention")]
    #[token("isolation")]
    #[token("always_on")]
    #[token("voltage")]
    Reserved,

    // ─── Operatörler ───
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("~")]
    Tilde,
    #[token("!")]
    Bang,
    #[token("&&")]
    AmpAmp,
    #[token("||")]
    PipePipe,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    /// Hem `a <= b` karşılaştırması hem nonblocking atama; parser ayırır (§19 N6 benzeri).
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("<<")]
    Shl,
    #[token(">>")]
    Shr,
    #[token("=")]
    Eq,
    #[token("@")]
    At,
    #[token("::")]
    ColonColon,
    #[token(":")]
    Colon,
    #[token(";")]
    Semi,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    /// `for i in 0..N` aralığı (§10 ForStmt).
    #[token("..")]
    DotDot,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    /// `data[i +: W]` artan parça seçimi (ADR-0035).
    #[token("+:")]
    PlusColon,
    /// `data[i -: W]` azalan parça seçimi (ADR-0035).
    #[token("-:")]
    MinusColon,

    /// Hiçbir kurala uymayan girdi (E0001).
    Error,
}

impl TokenKind {
    /// Parser'ın atladığı önemsiz tokenlar (doc yorumları HARİÇ).
    pub fn is_trivia(&self) -> bool {
        matches!(
            self,
            TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment(_)
        )
    }
}

/// İç içe blok yorumu tüketir. Dönüş: kapanış `*/` bulundu mu?
fn lex_block_comment(lex: &mut logos::Lexer<TokenKind>) -> bool {
    let bytes = lex.remainder().as_bytes();
    let mut depth: usize = 1;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                lex.bump(i);
                return true;
            }
        } else {
            i += 1;
        }
    }
    lex.bump(bytes.len());
    false
}
