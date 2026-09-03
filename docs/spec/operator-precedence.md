# Volt Operator Precedence and Associativity

> STATUS: BINDING SPECIFICATION
> This table is the reference for the parser implementation.
> Changes require an ADR.
>
> Turkish translation: `docs/spec/tr/operator-precedence.md`

---

## 1. Precedence Table

Lower number = lower precedence (binds last).
Used as `binding power` in the Pratt parser.

| Prec | Operator | Meaning | Assoc | Example |
|------|----------|---------|-------|---------|
| 1 | `\|\|` | logical or | left | `a \|\| b \|\| c` → `(a \|\| b) \|\| c` |
| 2 | `&&` | logical and | left | `a && b && c` → `(a && b) && c` |
| 3 | `==` `!=` | equality | **non-assoc** | `a == b == c` → ERROR E0010 |
| 4 | `<` `>` `<=` `>=` | comparison | **non-assoc** | `a < b < c` → ERROR E0010 |
| 5 | `\|` | bitwise or | left | `a \| b \| c` |
| 6 | `^` | bitwise xor | left | `a ^ b ^ c` |
| 7 | `&` | bitwise and | left | `a & b & c` |
| 8 | `<<` `>>` | shift | left | `a << 1 << 2` |
| 9 | `+` `-` | add/subtract | left | `a - b - c` → `(a - b) - c` |
| 10 | `*` `/` `%` | multiply/divide/mod | left | `a / b / c` → `(a / b) / c` |
| 11 | `!` `~` `-` (unary) | negation | right | `!!a` → `!(!a)` |
| 12 | `.` `[]` `()` `as` | access/call/cast | left | `a.b[c]()` |

---

## 2. Key Design Decisions

### 2.1 Comparison Operators Do Not Chain

```volt
// ERROR — prevents a mathematical fallacy
let bad = a < b < c;
// error[E0010]: comparison operators cannot be chained
//   = help: write (a < b) && (b < c)
```

**Rationale:** In C, `a < b < c` silently becomes `(a<b) < c`,
evaluating `0 < c` or `1 < c`. This is almost always a bug.
Rust made this decision; Volt follows.

### 2.2 Bitwise Operators Bind Tighter Than Comparison

```volt
// Volt (correct):
if a & MASK == 0 { }
// → if (a & MASK) == 0   ← expected

// C (trap):
if (a & MASK == 0)
// → if a & (MASK == 0)   ← unexpected!
```

**Rationale:** The `a & MASK == 0` pattern is very common in
hardware code. C's ordering is a historical mistake. Volt fixes it.

### 2.3 Shift Binds Tighter Than Addition

```volt
let x = a << 2 + 1;
// Volt: a << (2 + 1) = a << 3
```

**Rationale:** Same as C. Changing it would surprise more people.
Use parentheses when ambiguous.

### 2.4 Unary Minus vs Binary Minus

```
The lexer emits the same token: MINUS
The parser distinguishes by context:

  At expression start or after an operator → unary (prec 11)
  After an operand                          → binary (prec 9)

Examples:
  -a + b     → (-a) + b
  a - -b     → a - (-b)
  a * -b     → a * (-b)
```

### 2.5 The `as` Operator (Type Cast)

```volt
let x = a as u16 + b;
// → (a as u16) + b    ← as binds first
```

At precedence 12, tighter than even unary operators:

```volt
let y = -a as i16;
// → -(a as i16)   ← as binds first
// If ambiguous, write: (-a) as i16
```

---

## 3. Binding Power for the Pratt Parser

Ready-to-use values for the Rust implementation:

```rust
/// (left_bp, right_bp) — for left-assoc, right_bp = left_bp + 1
fn infix_binding_power(op: BinOp) -> Option<(u8, u8)> {
    let bp = match op {
        BinOp::Or        => (1, 2),      // left
        BinOp::And       => (3, 4),      // left
        BinOp::Eq | BinOp::Ne
                         => (5, 5),      // NON-ASSOC (equal bp)
        BinOp::Lt | BinOp::Gt
        | BinOp::Le | BinOp::Ge
                         => (7, 7),      // NON-ASSOC
        BinOp::BitOr     => (9, 10),
        BinOp::BitXor    => (11, 12),
        BinOp::BitAnd    => (13, 14),
        BinOp::Shl | BinOp::Shr
                         => (15, 16),
        BinOp::Add | BinOp::Sub
                         => (17, 18),
        BinOp::Mul | BinOp::Div | BinOp::Rem
                         => (19, 20),
    };
    Some(bp)
}

/// Prefix operators — right bp only
fn prefix_binding_power(op: UnOp) -> ((), u8) {
    match op {
        UnOp::Not | UnOp::BitNot | UnOp::Neg => ((), 21),
    }
}

/// Postfix — left bp only
fn postfix_binding_power(op: PostfixOp) -> Option<(u8, ())> {
    match op {
        PostfixOp::Index | PostfixOp::Field
        | PostfixOp::Call | PostfixOp::Cast => Some((23, ())),
    }
}
```

**Non-associativity check:**

```rust
// Equal binding power → two operators at the same level
// The parser must detect this and emit E0010
if lhs_was_comparison && op.is_comparison() {
    return Err(Diagnostic::new(ErrorCode::E0010)
        .with_message("comparison operators cannot be chained")
        .with_help("write (a < b) && (b < c)"));
}
```

---

## 4. Verification Test Vectors

The parser must produce these trees:

```
INPUT                        EXPECTED TREE
──────────────────────────────────────────────────────────
a + b * c                    (+ a (* b c))
a * b + c                    (+ (* a b) c)
a - b - c                    (- (- a b) c)
a && b || c                  (|| (&& a b) c)
a || b && c                  (|| a (&& b c))
a & MASK == 0                (== (& a MASK) 0)
a << 2 + 1                   (<< a (+ 2 1))
!a && b                      (&& (! a) b)
-a + b                       (+ (- a) b)
a as u16 + b                 (+ (as a u16) b)
!a[0]                        (! (index a 0))
a.b.c                        (field (field a b) c)
(a + b) * c                  (* (+ a b) c)
──────────────────────────────────────────────────────────
a < b < c                    ERROR E0010
a == b == c                  ERROR E0010
```

This table is the content of `tests/unit/precedence_test.rs`.

---

## 5. Common Pitfalls (Warnings Emitted)

```volt
// W0010: ambiguous precedence, parentheses recommended
let x = a & b | c;
// warning[W0010]: mixing '&' and '|' — parentheses recommended
//   = help: write (a & b) | c

let y = a << 1 + 2;
// warning[W0010]: mixing '<<' and '+' — parentheses recommended
//   = help: write a << (1 + 2) (current interpretation)
```

**Rationale:** Even when precedence is correct, readers doubt it.
Same approach as Rust's `clippy::precedence` lint.
