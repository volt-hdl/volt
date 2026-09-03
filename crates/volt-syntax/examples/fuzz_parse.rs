//! Yerleşik parser fuzzer'ı — libFuzzer erişilemeyen ortamlar için.
//!
//! Üç strateji: (1) rastgele bayt çorbası, (2) Volt token çorbası,
//! (3) tohum korpusunun mutasyonu. Deterministik xorshift PRNG kullanır;
//! panik olursa süreç çöker ve komut başarısız olur.
//!
//! Kullanım: cargo run --release -p volt-syntax --example fuzz_parse -- <saniye>

use std::time::{Duration, Instant};

use volt_span::FileId;
use volt_syntax::parser::parse;

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const VOCAB: &[&str] = &[
    "module",
    "domain",
    "in",
    "out",
    "inout",
    "reg",
    "let",
    "wire",
    "on",
    "comb",
    "if",
    "else",
    "match",
    "for",
    "as",
    "fn",
    "struct",
    "enum",
    "use",
    "pub",
    "bool",
    "clock",
    "reset",
    "bits",
    "Trit",
    "u8",
    "u16",
    "i8",
    "i64",
    "true",
    "false",
    "todo",
    "requires",
    "posedge",
    "negedge",
    "sync",
    "async",
    "active_high",
    "active_low",
    "pipeline",
    "fsm",
    "self",
    "+",
    "-",
    "*",
    "/",
    "%",
    "&",
    "|",
    "^",
    "~",
    "!",
    "&&",
    "||",
    "==",
    "!=",
    "<",
    ">",
    "<=",
    ">=",
    "<<",
    ">>",
    "=",
    "@",
    "::",
    ":",
    ";",
    ",",
    ".",
    "..",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "->",
    "=>",
    "0",
    "1",
    "42",
    "0xFF",
    "0b1010",
    "1_000u8",
    "-5i16",
    "0x",
    "0b12",
    "clk",
    "a",
    "b",
    "sayaç",
    "değer_r",
    "\"str\"",
    "// yorum\n",
    "/* blok */",
    "/* /* iç */ */",
    "/*açık",
    "///doc\n",
    "//!iç\n",
    "\n",
    " ",
    "\t",
    "#",
    "$",
    "€",
    "\u{0}",
];

const SEEDS: &[&str] = &[
    include_str!("../../../tests/fixtures/counter.volt"),
    "module M { in a : bits<8> on clk { if a { x <= 1 } else { x <= 0 } } }",
    "domain Fast { clock = posedge, reset = sync active_high }",
    "module Broken { in a : \n} module Valid { out c : u8 c = 1 }",
    "let x = a < b < c; let y = -a as i16 + b[7:4];",
];

fn random_bytes(rng: &mut XorShift) -> String {
    let len = rng.below(300);
    (0..len)
        .map(|_| {
            let c = rng.next() as u32 % 0x250; // ASCII + Latin ekleri + biraz ötesi
            char::from_u32(c).unwrap_or('?')
        })
        .collect()
}

fn token_soup(rng: &mut XorShift) -> String {
    let len = rng.below(200);
    let mut s = String::new();
    for _ in 0..len {
        s.push_str(VOCAB[rng.below(VOCAB.len())]);
        if rng.below(3) != 0 {
            s.push(' ');
        }
    }
    s
}

/// `i`'yi geriye doğru en yakın UTF-8 karakter sınırına çeker.
fn floor_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn mutate_seed(rng: &mut XorShift) -> String {
    let mut s = SEEDS[rng.below(SEEDS.len())].to_string();
    for _ in 0..rng.below(8) {
        match rng.below(4) {
            // kes
            0 if !s.is_empty() => {
                let cut = floor_boundary(&s, rng.below(s.len()));
                s.truncate(cut);
            }
            // araya token sok
            1 => {
                let pos = floor_boundary(&s, rng.below(s.len() + 1));
                s.insert_str(pos, VOCAB[rng.below(VOCAB.len())]);
            }
            // parçayı kopyala
            2 if s.len() > 4 => {
                let a = floor_boundary(&s, rng.below(s.len()));
                let b = floor_boundary(&s, a + rng.below(s.len() - a + 1));
                let chunk = s[a..b].to_string();
                s.push_str(&chunk);
            }
            // bir karakteri boz
            _ => {
                let count = s.chars().count();
                if count > 0 {
                    if let Some((pos, c)) = s.char_indices().nth(rng.below(count)) {
                        let ch = char::from_u32(33 + (rng.next() as u32 % 90)).unwrap_or('!');
                        s.replace_range(pos..pos + c.len_utf8(), &ch.to_string());
                    }
                }
            }
        }
    }
    s
}

fn main() {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(60);
    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut rng = XorShift(0x5eed_cafe_f00d_1234);
    let mut iterations: u64 = 0;

    while Instant::now() < deadline {
        for _ in 0..256 {
            let input = match rng.below(3) {
                0 => random_bytes(&mut rng),
                1 => token_soup(&mut rng),
                _ => mutate_seed(&mut rng),
            };
            let _ = parse(FileId(0), &input); // panik → süreç çöker
            iterations += 1;
        }
    }

    println!("panik yok: {iterations} girdi ayrıştırıldı ({secs}s)");
}
