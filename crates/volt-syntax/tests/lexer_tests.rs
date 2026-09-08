//! Volt lexer birim testleri.
//!
//! Referans: docs/spec/grammar-full.ebnf §14-§17

use volt_syntax::lexer::{tokenize, tokenize_with_trivia};
use volt_syntax::span::FileId;
use volt_syntax::token::TokenKind::{self, *};

fn kinds(src: &str) -> Vec<TokenKind> {
    tokenize(FileId(0), src)
        .tokens
        .iter()
        .map(|t| t.kind)
        .collect()
}

fn error_codes(src: &str) -> Vec<&'static str> {
    tokenize(FileId(0), src)
        .errors
        .iter()
        .map(|e| e.code.as_str())
        .collect()
}

/// Hatasız tokenize edilmesini bekler, token türlerini döndürür.
fn assert_clean(src: &str) -> Vec<TokenKind> {
    let out = tokenize(FileId(0), src);
    assert!(
        out.errors.is_empty(),
        "beklenmeyen hatalar: {:?}",
        out.errors
    );
    out.tokens.iter().map(|t| t.kind).collect()
}

// ═══ Anahtar kelimeler (aktif liste) ══════════════════════════════

#[test]
fn kw_module_and_ident() {
    assert_eq!(assert_clean("module Counter"), vec![KwModule, Ident]);
}

#[test]
fn kw_port_directions() {
    assert_eq!(assert_clean("in out inout"), vec![KwIn, KwOut, KwInout]);
}

#[test]
fn kw_storage() {
    assert_eq!(assert_clean("reg let wire"), vec![KwReg, KwLet, KwWire]);
}

#[test]
fn kw_blocks() {
    assert_eq!(assert_clean("on comb domain"), vec![KwOn, KwComb, KwDomain]);
}

#[test]
fn kw_control_flow() {
    assert_eq!(
        assert_clean("if else match for as"),
        vec![KwIf, KwElse, KwMatch, KwFor, KwAs]
    );
}

#[test]
fn kw_items() {
    assert_eq!(
        assert_clean("fn const type struct enum extern package use pub"),
        vec![KwFn, KwConst, KwType, KwStruct, KwEnum, KwExtern, KwPackage, KwUse, KwPub]
    );
}

#[test]
fn kw_primitive_types() {
    assert_eq!(
        assert_clean("bool clock reset bits Trit"),
        vec![KwBool, KwClock, KwReset, KwBits, KwTrit]
    );
}

#[test]
fn kw_unsigned_int_types() {
    assert_eq!(
        assert_clean("u8 u16 u32 u64"),
        vec![KwU8, KwU16, KwU32, KwU64]
    );
}

#[test]
fn kw_signed_int_types() {
    assert_eq!(
        assert_clean("i8 i16 i32 i64"),
        vec![KwI8, KwI16, KwI32, KwI64]
    );
}

#[test]
fn kw_bool_literals() {
    assert_eq!(assert_clean("true false"), vec![KwTrue, KwFalse]);
}

#[test]
fn kw_todo() {
    assert_eq!(assert_clean("todo"), vec![KwTodo]);
}

#[test]
fn kw_contracts() {
    assert_eq!(
        assert_clean("requires ensures invariant cover assert assume"),
        vec![
            KwRequires,
            KwEnsures,
            KwInvariant,
            KwCover,
            KwAssert,
            KwAssume
        ]
    );
}

#[test]
fn kw_clock_edges() {
    assert_eq!(
        assert_clean("posedge negedge none"),
        vec![KwPosedge, KwNegedge, KwNone]
    );
}

#[test]
fn kw_reset_spec() {
    // ADR-0023: sync/async bağlamsal — lexer düz Ident üretir,
    // yorumu parser "reset =" konumunda yapar.
    assert_eq!(
        assert_clean("sync async active_high active_low"),
        vec![Ident, Ident, KwActiveHigh, KwActiveLow]
    );
}

// ═══ Ayrılmış anahtar kelimeler (E0003) ═══════════════════════════

#[test]
fn reserved_structural() {
    let ks = kinds("pipeline fsm");
    assert_eq!(ks, vec![Reserved; 2]);
}

#[test]
fn released_stdlib_words_are_plain_idents() {
    // ADR-0028: stdlib bileşeni oldular, rezervasyon kalktı.
    assert_eq!(assert_clean("fifo ram regfile arbiter"), vec![Ident; 4]);
}

#[test]
fn reserved_pipeline_control() {
    assert_eq!(kinds("stage stall flush hook"), vec![Reserved; 4]);
}

#[test]
fn reserved_rust_like() {
    assert_eq!(
        kinds("impl trait where mut ref move self Self"),
        vec![Reserved; 8]
    );
}

#[test]
fn reserved_type_names() {
    assert_eq!(
        kinds("Spike PTrit Option Some None Result Ok Err spike"),
        vec![Reserved; 9]
    );
}

#[test]
fn reserved_v1_domain_words() {
    // grammar-full.ebnf §17'deki tam liste — V1 domain/güvenlik kelimeleri
    assert_eq!(
        kinds("secret confidential public clamp_low clamp_high latch retention isolation always_on voltage"),
        vec![Reserved; 10]
    );
}

#[test]
fn reserved_emits_e0003() {
    assert_eq!(error_codes("pipeline"), vec!["E0003"]);
}

#[test]
fn reserved_each_use_emits_own_error() {
    assert_eq!(error_codes("fsm fsm"), vec!["E0003", "E0003"]);
}

// ═══ Operatörler ══════════════════════════════════════════════════

#[test]
fn op_arithmetic() {
    assert_eq!(
        assert_clean("+ - * / %"),
        vec![Plus, Minus, Star, Slash, Percent]
    );
}

#[test]
fn op_bitwise() {
    assert_eq!(assert_clean("& | ^ ~"), vec![Amp, Pipe, Caret, Tilde]);
}

#[test]
fn op_logical() {
    assert_eq!(assert_clean("! && ||"), vec![Bang, AmpAmp, PipePipe]);
}

#[test]
fn op_comparison() {
    assert_eq!(
        assert_clean("== != < > <= >="),
        vec![EqEq, NotEq, Lt, Gt, Le, Ge]
    );
}

#[test]
fn op_shift() {
    assert_eq!(assert_clean("<< >>"), vec![Shl, Shr]);
}

#[test]
fn op_assign_and_at() {
    assert_eq!(assert_clean("= @"), vec![Eq, At]);
}

#[test]
fn op_punctuation() {
    assert_eq!(
        assert_clean(":: : ; , . .."),
        vec![ColonColon, Colon, Semi, Comma, Dot, DotDot]
    );
}

#[test]
fn op_delimiters() {
    assert_eq!(
        assert_clean("( ) [ ] { }"),
        vec![LParen, RParen, LBracket, RBracket, LBrace, RBrace]
    );
}

#[test]
fn op_arrows() {
    assert_eq!(assert_clean("-> =>"), vec![Arrow, FatArrow]);
}

#[test]
fn op_longest_match_le() {
    // '<=' tek token — hem karşılaştırma hem nonblocking atama (parser ayırır)
    assert_eq!(assert_clean("a<=b"), vec![Ident, Le, Ident]);
}

#[test]
fn op_longest_match_shl_vs_lt() {
    assert_eq!(assert_clean("a<<b<c"), vec![Ident, Shl, Ident, Lt, Ident]);
}

#[test]
fn op_longest_match_coloncolon() {
    assert_eq!(
        assert_clean("a::b:c"),
        vec![Ident, ColonColon, Ident, Colon, Ident]
    );
}

#[test]
fn op_fat_arrow_vs_eq() {
    assert_eq!(assert_clean("= => =="), vec![Eq, FatArrow, EqEq]);
}

// ═══ Literaller ═══════════════════════════════════════════════════

#[test]
fn lit_decimal() {
    assert_eq!(assert_clean("42"), vec![IntLit]);
}

#[test]
fn lit_decimal_underscore_is_single_token() {
    let out = tokenize(FileId(0), "1_000");
    assert!(out.errors.is_empty());
    assert_eq!(out.tokens.len(), 1);
    assert_eq!(out.tokens[0].kind, IntLit);
    assert_eq!(out.tokens[0].span.end - out.tokens[0].span.start, 5);
}

#[test]
fn lit_hex() {
    assert_eq!(assert_clean("0xFF"), vec![IntLit]);
}

#[test]
fn lit_hex_mixed_case_underscore() {
    assert_eq!(assert_clean("0xDe_aD"), vec![IntLit]);
}

#[test]
fn lit_binary() {
    assert_eq!(assert_clean("0b1010"), vec![IntLit]);
}

#[test]
fn lit_octal() {
    assert_eq!(assert_clean("0o755"), vec![IntLit]);
}

#[test]
fn lit_suffix_u8_is_single_token() {
    let out = tokenize(FileId(0), "42u8");
    assert!(out.errors.is_empty());
    assert_eq!(out.tokens.len(), 1);
    assert_eq!(out.tokens[0].kind, IntLit);
    assert_eq!(out.tokens[0].span.end, 4);
}

#[test]
fn lit_negative_is_minus_then_int() {
    // Spec §19 [N6]: lexer tek '-' üretir, parser tekli/ikili ayırır
    assert_eq!(assert_clean("-5i16"), vec![Minus, IntLit]);
}

#[test]
fn lit_suffix_on_hex() {
    let out = tokenize(FileId(0), "0xFFu8");
    assert!(out.errors.is_empty());
    assert_eq!(out.tokens.len(), 1);
    assert_eq!(out.tokens[0].kind, IntLit);
}

#[test]
fn lit_invalid_hex_without_digits_e0005() {
    assert_eq!(error_codes("0x"), vec!["E0005"]);
}

#[test]
fn lit_invalid_binary_digit_e0005() {
    assert_eq!(error_codes("0b12"), vec!["E0005"]);
}

#[test]
fn lit_string_simple() {
    assert_eq!(assert_clean("\"merhaba\""), vec![StringLit]);
}

#[test]
fn lit_string_with_escapes() {
    assert_eq!(assert_clean(r#""a\n\t\"b\\""#), vec![StringLit]);
}

// ═══ Kimlikler ════════════════════════════════════════════════════

#[test]
fn ident_variants() {
    assert_eq!(assert_clean("_x x1 _ abc_def"), vec![Ident; 4]);
}

#[test]
fn ident_with_keyword_prefix_is_ident() {
    // "input" 'in' değildir, "iffy" 'if' değildir
    assert_eq!(assert_clean("input module_x iffy"), vec![Ident; 3]);
}

// ═══ Yorumlar ═════════════════════════════════════════════════════

#[test]
fn comment_line_skipped() {
    assert_eq!(assert_clean("a // yorum\nb"), vec![Ident, Ident]);
}

#[test]
fn comment_empty_line_comment_skipped() {
    assert_eq!(assert_clean("a //\nb"), vec![Ident, Ident]);
}

#[test]
fn comment_doc_is_token() {
    assert_eq!(assert_clean("/// doc\nmodule"), vec![DocComment, KwModule]);
}

#[test]
fn comment_inner_doc_is_token() {
    assert_eq!(assert_clean("//! iç doc\n"), vec![InnerDocComment]);
}

#[test]
fn comment_block_skipped() {
    assert_eq!(assert_clean("a /* yorum */ b"), vec![Ident, Ident]);
}

#[test]
fn comment_nested_block_supported() {
    // İç içe blok yorum DESTEKLENİR (spec §16)
    assert_eq!(
        assert_clean("a /* dış /* iç */ devam */ b"),
        vec![Ident, Ident]
    );
}

#[test]
fn comment_unterminated_block_e0013() {
    assert_eq!(error_codes("/* açık kaldı"), vec!["E0013"]);
}

// ═══ Hatalı girdi ═════════════════════════════════════════════════

#[test]
fn unexpected_char_e0001() {
    assert_eq!(error_codes("#"), vec!["E0001"]);
}

// ═══ Span doğruluğu ═══════════════════════════════════════════════

#[test]
fn span_start_end() {
    let out = tokenize(FileId(0), "let x");
    assert_eq!((out.tokens[0].span.start, out.tokens[0].span.end), (0, 3));
    assert_eq!((out.tokens[1].span.start, out.tokens[1].span.end), (4, 5));
}

#[test]
fn span_carries_file_id() {
    let out = tokenize(FileId(7), "module");
    assert_eq!(out.tokens[0].span.file, FileId(7));
}

#[test]
fn span_absolute_after_skipped_whitespace() {
    // Boşluk atlanır ama konumlar mutlak kalır
    let out = tokenize(FileId(0), "  a");
    assert_eq!((out.tokens[0].span.start, out.tokens[0].span.end), (2, 3));
}

#[test]
fn trivia_whitespace_recorded_with_span() {
    let out = tokenize_with_trivia(FileId(0), "a b");
    assert_eq!(out.tokens[1].kind, Whitespace);
    assert_eq!((out.tokens[1].span.start, out.tokens[1].span.end), (1, 2));
}

#[test]
fn spans_cover_source_contiguously_with_trivia() {
    let src = "let x = 0xFF // yorum\n";
    let out = tokenize_with_trivia(FileId(0), src);
    let mut pos = 0u32;
    for t in &out.tokens {
        assert_eq!(t.span.start, pos, "boşluksuz span dizisi: {:?}", t);
        pos = t.span.end;
    }
    assert_eq!(pos as usize, src.len());
}

// ═══ Fixture ══════════════════════════════════════════════════════

#[test]
fn fixture_counter_volt_tokenizes_cleanly() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/counter.volt"
    );
    let src = std::fs::read_to_string(path).expect("counter.volt okunamalı");
    let out = tokenize(FileId(1), &src);
    assert!(out.errors.is_empty(), "hatalar: {:?}", out.errors);
    assert!(
        out.tokens.len() > 30,
        "beklenenden az token: {}",
        out.tokens.len()
    );
    // İlk iki token doc comment olmalı
    assert_eq!(out.tokens[0].kind, DocComment);
    assert_eq!(out.tokens[1].kind, DocComment);
    // Her token doğru dosya kimliğini taşımalı
    assert!(out.tokens.iter().all(|t| t.span.file == FileId(1)));
    // Span'ler kaynak sınırları içinde ve sıralı olmalı
    let mut prev_end = 0u32;
    for t in &out.tokens {
        assert!(t.span.start >= prev_end);
        assert!((t.span.end as usize) <= src.len());
        prev_end = t.span.end;
    }
}
