//! A small tokenizer of Volt source for the rewrites (#254, ADR-0117).
//!
//! The metamorphic test must not ask the compiler where a construct is:
//! a rewrite that used the compiler's parser would inherit its mistakes
//! (#204 was a parser-level desugaring that the checkers did not see). The
//! tokenizer knows identifiers, numbers, strings, comments and
//! punctuation, and nothing of the grammar; each rewrite reads the token
//! shapes it needs and leaves every other text unchanged.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ident,
    Number,
    Str,
    Punct,
}

#[derive(Debug, Clone)]
pub struct Tok {
    pub kind: Kind,
    /// Byte range in the source.
    pub start: usize,
    pub end: usize,
    /// 1-based line of the first byte.
    pub line: usize,
    /// First token of its line.
    pub line_start: bool,
}

/// Multi-character punctuation, longest first.
const PUNCT: &[&str] = &[
    ">>>", "..=", "=>", "<=", ">=", "==", "!=", "&&", "||", "::", "..", "->", "+:", "-:", "<<",
    ">>",
];

/// Keywords: never the name of a struct, module or field.
pub const KEYWORDS: &[&str] = &[
    "module",
    "extern",
    "in",
    "out",
    "inout",
    "reg",
    "wire",
    "let",
    "const",
    "on",
    "comb",
    "if",
    "else",
    "match",
    "for",
    "struct",
    "enum",
    "bundle",
    "domain",
    "fn",
    "test",
    "type",
    "use",
    "pub",
    "package",
    "return",
    "true",
    "false",
    "as",
    "pipeline",
    "stage",
    "assert",
    "assume",
    "cover",
    "requires",
    "ensures",
    "invariant",
    "mmio",
    "trait",
    "impl",
    "self",
    "where",
];

pub fn is_keyword(s: &str) -> bool {
    KEYWORDS.contains(&s)
}

/// The tokens of `src`. Comments and white space are skipped; an
/// unterminated string or block comment ends at the end of the source.
pub fn lex(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut toks = Vec::new();
    let (mut i, mut line) = (0usize, 1usize);
    let mut line_start = true;
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            line_start = true;
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                if b[i] == b'\n' {
                    line += 1;
                    line_start = true;
                }
                i += 1;
            }
            i = (i + 2).min(b.len());
            continue;
        }
        let start = i;
        let tok_line = line;
        let kind = if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            Kind::Ident
        } else if c.is_ascii_digit() {
            // 0x1F, 0b1010, 1_000, 8'hFF: letters, digits, `_` and `'`.
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'\'') {
                i += 1;
            }
            Kind::Number
        } else if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                if i < b.len() && b[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            Kind::Str
        } else {
            let rest = &src[i..];
            let len = PUNCT.iter().find(|p| rest.starts_with(**p)).map_or_else(
                || rest.chars().next().map_or(1, char::len_utf8),
                |p| p.len(),
            );
            i += len;
            Kind::Punct
        };
        toks.push(Tok {
            kind,
            start,
            end: i,
            line: tok_line,
            line_start,
        });
        line_start = false;
    }
    toks
}

/// A token's text.
pub fn text<'a>(src: &'a str, t: &Tok) -> &'a str {
    &src[t.start..t.end]
}

/// Index of the token that closes the bracket opened at `open`
/// (`(`/`[`/`{`), counting all three kinds; `None` when unbalanced.
pub fn closing(src: &str, toks: &[Tok], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (k, t) in toks.iter().enumerate().skip(open) {
        match text(src, t) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

/// Indices of the tokens in `from..to` that are at bracket depth 0 of
/// that range.
pub fn top_level(src: &str, toks: &[Tok], from: usize, to: usize) -> Vec<usize> {
    let mut depth = 0i32;
    let mut out = Vec::new();
    for (k, t) in toks.iter().enumerate().take(to).skip(from) {
        let s = text(src, t);
        if matches!(s, ")" | "]" | "}") {
            depth -= 1;
        }
        if depth == 0 {
            out.push(k);
        }
        if matches!(s, "(" | "[" | "{") {
            depth += 1;
        }
    }
    out
}

/// The leading white space of the line that holds byte `pos`.
pub fn indent_of(src: &str, pos: usize) -> String {
    let line_start = src[..pos].rfind('\n').map_or(0, |n| n + 1);
    src[line_start..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<&str> {
        lex(src).iter().map(|t| text(src, t)).collect()
    }

    #[test]
    fn tokens_comments_and_lines() {
        let src = "let s = S { d, x: true } // S { y }\n/* a { b } */ q <= 8'hFF >> 2";
        assert_eq!(
            texts(src),
            [
                "let", "s", "=", "S", "{", "d", ",", "x", ":", "true", "}", "q", "<=", "8'hFF",
                ">>", "2"
            ]
        );
        let toks = lex(src);
        assert_eq!(toks[11].line, 2);
        assert!(toks[11].line_start);
        assert!(!toks[12].line_start);
    }

    #[test]
    fn strings_and_arrows() {
        let src = "test \"a { b }\" { x => y }";
        assert_eq!(
            texts(src),
            ["test", "\"a { b }\"", "{", "x", "=>", "y", "}"]
        );
    }

    #[test]
    fn brackets() {
        let src = "f(a, [b, {c}], d) e";
        let toks = lex(src);
        assert_eq!(closing(src, &toks, 1), Some(13));
        let top: Vec<&str> = top_level(src, &toks, 2, 13)
            .into_iter()
            .map(|k| text(src, &toks[k]))
            .collect();
        assert_eq!(top, ["a", ",", "[", "]", ",", "d"]);
    }
}
