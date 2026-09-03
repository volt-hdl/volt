# Volt Operatör Önceliği ve Birleşme Kuralları

> STATÜ: BAĞLAYICI SPESİFİKASYON
> Bu tablo parser implementasyonunun referansıdır.
> Değişiklik ADR gerektirir.

---

## 1. Öncelik Tablosu

Düşük sayı = düşük öncelik (en son bağlanır).
Pratt parser'da `binding power` olarak kullanılır.

| Öncelik | Operatör | Anlam | Birleşme | Örnek |
|---------|----------|-------|----------|-------|
| 1 | `\|\|` | mantıksal veya | sol | `a \|\| b \|\| c` → `(a \|\| b) \|\| c` |
| 2 | `&&` | mantıksal ve | sol | `a && b && c` → `(a && b) && c` |
| 3 | `==` `!=` | eşitlik | **birleşmez** | `a == b == c` → HATA E0010 |
| 4 | `<` `>` `<=` `>=` | karşılaştırma | **birleşmez** | `a < b < c` → HATA E0010 |
| 5 | `\|` | bit veya | sol | `a \| b \| c` |
| 6 | `^` | bit xor | sol | `a ^ b ^ c` |
| 7 | `&` | bit ve | sol | `a & b & c` |
| 8 | `<<` `>>` | kaydırma | sol | `a << 1 << 2` |
| 9 | `+` `-` | toplama/çıkarma | sol | `a - b - c` → `(a - b) - c` |
| 10 | `*` `/` `%` | çarpma/bölme/mod | sol | `a / b / c` → `(a / b) / c` |
| 11 | `!` `~` `-` (tekli) | değilleme | sağ | `!!a` → `!(!a)` |
| 12 | `.` `[]` `()` `as` | erişim/çağrı/cast | sol | `a.b[c]()` |

---

## 2. Kritik Tasarım Kararları

### 2.1 Karşılaştırma Operatörleri Birleşmez

```volt
// HATA — matematiksel yanılgıyı önler
let bad = a < b < c;
// error[E0010]: karşılaştırma operatörleri zincirlenemez
//   help: (a < b) && (b < c) yazın
```

**Gerekçe:** C'de `a < b < c` sessizce `(a<b) < c` olur ve
`0 < c` veya `1 < c` hesaplanır. Bu neredeyse her zaman hata.
Rust bu kararı verdi, Volt de veriyor.

### 2.2 Bit Operatörleri Karşılaştırmadan Yüksek

```volt
// Volt (doğru):
if a & MASK == 0 { }
// → if (a & MASK) == 0   ← beklenen

// C (tuzak):
if (a & MASK == 0)
// → if a & (MASK == 0)   ← beklenmedik!
```

**Gerekçe:** Donanım kodunda `a & MASK == 0` deseni çok yaygın.
C'nin sırası tarihsel bir hatadır. Volt düzeltiyor.

### 2.3 Kaydırma Toplamadan Yüksek

```volt
let x = a << 2 + 1;
// Volt: a << (2 + 1) = a << 3
```

**Gerekçe:** C ile aynı. Değiştirmek daha fazla şaşırtır.
Belirsizlik varsa parantez kullanılmalı.

### 2.4 Tekli Eksi vs İkili Eksi

```
Lexer aynı token üretir: MINUS
Parser bağlama göre ayırır:

  İfade başında veya operatörden sonra → tekli (öncelik 11)
  Operanddan sonra                     → ikili (öncelik 9)

Örnek:
  -a + b     → (-a) + b
  a - -b     → a - (-b)
  a * -b     → a * (-b)
```

### 2.5 `as` Operatörü (Tip Dönüşümü)

```volt
let x = a as u16 + b;
// → (a as u16) + b    ← as en yüksek öncelikte
```

Öncelik 12'de, tekli operatörlerden bile yüksek:

```volt
let y = -a as i16;
// → -(a as i16)   ← as önce bağlanır
// Belirsizse: (-a) as i16 yazılmalı
```

---

## 3. Pratt Parser İçin Binding Power

Rust implementasyonu için hazır değerler:

```rust
/// (sol_bp, sağ_bp) — sol birleşme için sağ_bp = sol_bp + 1
fn infix_binding_power(op: BinOp) -> Option<(u8, u8)> {
    let bp = match op {
        BinOp::Or        => (1, 2),      // sol
        BinOp::And       => (3, 4),      // sol
        BinOp::Eq | BinOp::Ne
                         => (5, 5),      // BİRLEŞMEZ (eşit bp)
        BinOp::Lt | BinOp::Gt
        | BinOp::Le | BinOp::Ge
                         => (7, 7),      // BİRLEŞMEZ
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

/// Tekli operatörler — sadece sağ bp
fn prefix_binding_power(op: UnOp) -> ((), u8) {
    match op {
        UnOp::Not | UnOp::BitNot | UnOp::Neg => ((), 21),
    }
}

/// Postfix — sadece sol bp
fn postfix_binding_power(op: PostfixOp) -> Option<(u8, ())> {
    match op {
        PostfixOp::Index | PostfixOp::Field
        | PostfixOp::Call | PostfixOp::Cast => Some((23, ())),
    }
}
```

**Birleşmezlik kontrolü:**

```rust
// Eşit binding power → aynı seviyede iki operatör
// Parser bunu tespit edip E0010 üretmeli
if lhs_was_comparison && op.is_comparison() {
    return Err(Diagnostic::new(ErrorCode::E0010)
        .with_message("karşılaştırma operatörleri zincirlenemez")
        .with_help("(a < b) && (b < c) yazın"));
}
```

---

## 4. Doğrulama Test Vektörleri

Parser bu ifadeleri şu ağaçlara ayrıştırmalı:

```
GİRDİ                        BEKLENEN AĞAÇ
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
a < b < c                    HATA E0010
a == b == c                  HATA E0010
```

Bu tablo `tests/unit/precedence_test.rs` dosyasının içeriğidir.

---

## 5. Sık Yapılan Hatalar (Uyarı Üretilecek)

```volt
// W0010: belirsiz öncelik, parantez önerilir
let x = a & b | c;
// warning[W0010]: '&' ve '|' karışımında parantez önerilir
//   help: (a & b) | c yazın

let y = a << 1 + 2;
// warning[W0010]: '<<' ve '+' karışımında parantez önerilir
//   help: a << (1 + 2) yazın (mevcut yorum)
```

**Gerekçe:** Öncelik doğru olsa bile okuyan kişi şüphe duyuyor.
Rust'ın `clippy::precedence` lint'i ile aynı yaklaşım.
