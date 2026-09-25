//! Lexer ve parser: kayıpsız somut sözdizimi ağacı (CST) üretimi.
//!
//! Şu an yalnızca lexer mevcuttur. Referans: docs/spec/grammar-full.ebnf.

pub mod lexer;
pub mod parser;
pub mod span;
pub mod stack;
pub mod token;

pub use lexer::{tokenize, tokenize_with_trivia, LexOutput};
pub use parser::{
    monomorphize, parse, parse_expr, parse_unit, GeneratedSource, ParseResult, MAX_DEPTH,
};
pub use stack::{with_compiler_stack, COMPILER_STACK_SIZE};
pub use token::{Token, TokenKind};
// Geriye uyumluluk re-export'u: Span tipleri volt-span'den gelir.
pub use volt_span::{FileId, Span};
