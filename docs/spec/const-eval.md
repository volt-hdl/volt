# Volt Derleme Zamanı Değerlendirme (Const Eval)

> STATÜ: BAĞLAYICI
> İlgili: `type-inference.md`, `name-resolution.md`
> Aşama: F2 — tip kontrolüyle iç içe

---

## 0. Neden Gerekli

Volt'ta bazı ifadeler **derleme zamanında** hesaplanmak zorunda:

```volt
const WIDTH : u32 = 8;

in  data  : bits<WIDTH * 2>      // 16 olmalı — tip için şart
in  mem   : [u8; 1 << 4]         // 16 eleman
out slice : bits<WIDTH>          // 8

for i in 0..WIDTH { ... }        // döngü sınırı

let x = data[WIDTH - 1]          // bit indeksi
```

Bu değerler bilinmezse tip hesaplanamaz, dizi boyutu
belirlenemez, döngü açılamaz.

---

## 1. Değer Gösterimi

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConstValue {
    /// Tam sayı — geniş tutulur, sığdırma sonra kontrol edilir
    Int(i128),
    Bool(bool),
    /// Sabit uzunluklu dizi
    Array(Vec<ConstValue>),
    /// Demet
    Tuple(Vec<ConstValue>),
    /// Enum varyantı — ayrıştırıcı değeri
    EnumVariant { def: DefId, discriminant: i128 },
    /// Hesaplanamadı (hata zaten raporlandı)
    Error,
}
```

**`i128` seçimi:** Ara hesaplarda taşmayı yakalayabilmek için
en geniş tip. Hedef tipe sığdırma ayrı kontrol edilir.

---

## 2. Değerlendirilebilir İfadeler

```
DEĞERLENDİRİLEBİLİR:
  ✓ Sayısal literaller           42, 0xFF, 0b1010
  ✓ Bool literaller              true, false
  ✓ const bildirimleri           const W : u32 = 8
  ✓ Generic const parametreler   <const N: u32>
  ✓ Aritmetik                    +, -, *, /, %
  ✓ Bit işlemleri                &, |, ^, ~, <<, >>
  ✓ Karşılaştırma                ==, !=, <, >, <=, >=
  ✓ Mantıksal                    &&, ||, !
  ✓ Koşullu                      if c { a } else { b }
  ✓ Dizi literali                [1, 2, 3]
  ✓ Dizi indeksi (sabit)         ARR[2]
  ✓ Enum varyantı                State::Idle
  ✓ clog2(N)                     yerleşik
  ✓ Cast (sabit değer)           (8 as u16)

DEĞERLENDİRİLEMEZ:
  ✗ Port referansı               in data — çalışma zamanı
  ✗ Register referansı           reg r — çalışma zamanı
  ✗ Wire/let bağlaması           let x = a + b
  ✗ Modül örneği alanı           uart.busy
  ✗ Bit/aralık seçimi (sinyalden) data[3]
  ✗ sync(), popcount() vb.       donanım fonksiyonları
  ✗ todo!                        tanımsız
```

---

## 3. Değerlendirme Algoritması

```rust
pub fn const_eval(&mut self, expr: Idx<Expr>) -> Result<ConstValue, EvalError> {
    // Sonuç önbelleği — aynı ifade tekrar hesaplanmasın
    if let Some(cached) = self.const_cache.get(&expr) {
        return cached.clone();
    }

    let result = self.const_eval_uncached(expr);
    self.const_cache.insert(expr, result.clone());
    result
}

fn const_eval_uncached(&mut self, expr: Idx<Expr>)
    -> Result<ConstValue, EvalError>
{
    let e = &self.ast.exprs[expr];
    match &e.kind {
        ExprKind::IntLit { value, .. } => Ok(ConstValue::Int(*value as i128)),
        ExprKind::BoolLit(b) => Ok(ConstValue::Bool(*b)),

        ExprKind::Path(path) => {
            let def = self.resolve(path, self.current_scope);
            match self.def_kind(def) {
                DefKind::Const => self.eval_const_def(def),
                DefKind::GenericParam => self.eval_generic_arg(def, e.span),
                DefKind::EnumVariant { parent } =>
                    Ok(self.variant_value(def, parent)),
                other => Err(EvalError::NotConstant {
                    span: e.span,
                    what: self.describe_def_kind(other),
                }),
            }
        }

        ExprKind::Binary { op, lhs, rhs } => {
            let l = self.const_eval(*lhs)?;
            let r = self.const_eval(*rhs)?;
            self.eval_binop(*op, l, r, e.span)
        }

        ExprKind::Unary { op, operand } => {
            let v = self.const_eval(*operand)?;
            self.eval_unop(*op, v, e.span)
        }

        ExprKind::If { cond, then_expr, else_expr } => {
            match self.const_eval(*cond)? {
                ConstValue::Bool(true)  => self.const_eval(*then_expr),
                ConstValue::Bool(false) => self.const_eval(*else_expr),
                _ => Err(EvalError::TypeMismatch {
                    span: e.span,
                    expected: "bool",
                }),
            }
        }

        ExprKind::Cast { expr: inner, ty } => {
            let v = self.const_eval(*inner)?;
            let target = self.resolve_type(*ty);
            self.eval_cast(v, target, e.span)
        }

        ExprKind::Call { callee, args } => self.eval_builtin_call(
            *callee, args, e.span),

        ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
            let vals: Result<Vec<_>, _> =
                items.iter().map(|i| self.const_eval(*i)).collect();
            Ok(ConstValue::Array(vals?))
        }

        ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
            let v = self.const_eval(*value)?;
            let n = self.const_eval_usize(*count)?;
            self.check_array_size(n, e.span)?;
            Ok(ConstValue::Array(vec![v; n]))
        }

        ExprKind::Index { base, index } => {
            let arr = self.const_eval(*base)?;
            let idx = self.const_eval_usize(*index)?;
            match arr {
                ConstValue::Array(items) => items.get(idx).cloned()
                    .ok_or(EvalError::IndexOutOfBounds {
                        span: e.span, index: idx, len: items.len() }),
                _ => Err(EvalError::NotIndexable { span: e.span }),
            }
        }

        ExprKind::Error => Ok(ConstValue::Error),

        _ => Err(EvalError::NotConstant {
            span: e.span,
            what: "bu ifade",
        }),
    }
}
```

---

## 4. Taşma Kuralları

**Kritik karar: derleme zamanı taşma HATA, sarma değil.**

```rust
fn eval_binop(&mut self, op: BinOp, l: ConstValue, r: ConstValue,
              span: Span) -> Result<ConstValue, EvalError> {
    use ConstValue::*;
    match (l, r) {
        (Error, _) | (_, Error) => Ok(Error),

        (Int(a), Int(b)) => {
            let v = match op {
                BinOp::Add => a.checked_add(b),
                BinOp::Sub => a.checked_sub(b),
                BinOp::Mul => a.checked_mul(b),

                BinOp::Div => {
                    if b == 0 {
                        return Err(EvalError::DivisionByZero { span });
                    }
                    a.checked_div(b)
                }
                BinOp::Rem => {
                    if b == 0 {
                        return Err(EvalError::DivisionByZero { span });
                    }
                    a.checked_rem(b)
                }

                BinOp::Shl => {
                    if !(0..128).contains(&b) {
                        return Err(EvalError::ShiftOverflow { span, amount: b });
                    }
                    a.checked_shl(b as u32)
                }
                BinOp::Shr => {
                    if !(0..128).contains(&b) {
                        return Err(EvalError::ShiftOverflow { span, amount: b });
                    }
                    a.checked_shr(b as u32)
                }

                BinOp::BitAnd => Some(a & b),
                BinOp::BitOr  => Some(a | b),
                BinOp::BitXor => Some(a ^ b),

                BinOp::Eq => return Ok(Bool(a == b)),
                BinOp::Ne => return Ok(Bool(a != b)),
                BinOp::Lt => return Ok(Bool(a <  b)),
                BinOp::Gt => return Ok(Bool(a >  b)),
                BinOp::Le => return Ok(Bool(a <= b)),
                BinOp::Ge => return Ok(Bool(a >= b)),

                BinOp::And | BinOp::Or =>
                    return Err(EvalError::TypeMismatch {
                        span, expected: "bool" }),
            };

            v.map(Int).ok_or(EvalError::Overflow { span, op })
        }

        (Bool(a), Bool(b)) => match op {
            BinOp::And => Ok(Bool(a && b)),
            BinOp::Or  => Ok(Bool(a || b)),
            BinOp::Eq  => Ok(Bool(a == b)),
            BinOp::Ne  => Ok(Bool(a != b)),
            _ => Err(EvalError::TypeMismatch { span, expected: "sayısal" }),
        },

        _ => Err(EvalError::TypeMismatch { span, expected: "aynı tip" }),
    }
}
```

**Gerekçe:**

```
Çalışma zamanı: u8 + u8 → u9 (genişleme, taşma yok)
Derleme zamanı: sarma sessiz hata üretir

const W : u32 = 200 * 200;   // 40000, u32'ye sığar ✓
const X : u8  = 200 + 100;   // 300, u8'e sığmaz ✗ E2010
```

---

## 5. Döngüsel Bağımlılık Tespiti

```volt
const A : u32 = B + 1;
const B : u32 = A + 1;      // ✗ sonsuz döngü
```

```rust
pub struct ConstEvaluator {
    /// Şu an değerlendirilmekte olan sabitler
    in_progress: Vec<DefId>,
    cache: FxHashMap<DefId, Result<ConstValue, EvalError>>,
}

fn eval_const_def(&mut self, def: DefId) -> Result<ConstValue, EvalError> {
    if let Some(cached) = self.cache.get(&def) {
        return cached.clone();
    }

    // Döngü kontrolü
    if let Some(pos) = self.in_progress.iter().position(|d| *d == def) {
        let cycle: Vec<Name> = self.in_progress[pos..].iter()
            .map(|d| self.name_of(*d))
            .collect();
        return Err(EvalError::CyclicDependency {
            span: self.def_span(def),
            cycle,
        });
    }

    self.in_progress.push(def);
    let init = self.const_init_expr(def);
    let result = self.const_eval(init);
    self.in_progress.pop();

    self.cache.insert(def, result.clone());
    result
}
```

Hata mesajı:

```
error[E2020]: döngüsel sabit bağımlılığı
  ┌─ design.volt:1:1
  │
1 │ const A : u32 = B + 1;
  │ ^^^^^^^^^^^^^^^^^^^^^ 'A' hesaplanırken
2 │ const B : u32 = A + 1;
  │ --------------------- 'B' gerekiyor, o da 'A'ya bağlı
  │
  = döngü: A → B → A
  = çözüm: bağımlılıklardan birini kaldırın
```

---

## 6. Yerleşik Fonksiyonlar

```rust
fn eval_builtin_call(&mut self, callee: Idx<Expr>, args: &[Idx<Expr>],
                     span: Span) -> Result<ConstValue, EvalError> {
    let builtin = self.resolve_builtin(callee)?;

    match builtin {
        // clog2(n) — n'i temsil etmek için gereken bit sayısı
        BuiltinKind::Clog2 => {
            self.expect_arity(args, 1, span)?;
            let n = self.const_eval_i128(args[0])?;
            if n <= 0 {
                return Err(EvalError::InvalidArgument {
                    span,
                    message: "clog2 pozitif değer bekler".into(),
                });
            }
            // clog2(1)=0, clog2(2)=1, clog2(3)=2, clog2(8)=3
            let bits = 128 - (n as u128 - 1).leading_zeros();
            Ok(ConstValue::Int(if n == 1 { 0 } else { bits as i128 }))
        }

        // Donanım fonksiyonları derleme zamanı değerlendirilemez
        BuiltinKind::Sync | BuiltinKind::Sync3
        | BuiltinKind::PopCount | BuiltinKind::Concat
        | BuiltinKind::Replicate => Err(EvalError::NotConstant {
            span,
            what: "donanım fonksiyonu",
        }),

        // Cast benzeri yerleşikler değerlendirilebilir
        BuiltinKind::Zext | BuiltinKind::Sext | BuiltinKind::Trunc => {
            self.expect_arity(args, 1, span)?;
            self.const_eval(args[0])   // değer değişmiyor, sadece tip
        }
    }
}
```

**`clog2` neden önemli:** Adres genişliği hesabında standart.

```volt
const DEPTH : u32 = 64;
in addr : bits<clog2(DEPTH)>    // bits<6>
```

---

## 7. Tip Bağlamında Kullanım

```rust
/// bits<N>, [T; N] gibi tip pozisyonlarında
fn eval_type_arg(&mut self, expr: Idx<Expr>) -> Result<u32, EvalError> {
    let v = self.const_eval(expr)?;
    match v {
        ConstValue::Int(n) if n > 0 && n <= MAX_WIDTH as i128 => Ok(n as u32),
        ConstValue::Int(n) if n <= 0 => Err(EvalError::InvalidWidth {
            span: self.span(expr),
            value: n,
            reason: "genişlik pozitif olmalı",
        }),
        ConstValue::Int(n) => Err(EvalError::InvalidWidth {
            span: self.span(expr),
            value: n,
            reason: "genişlik çok büyük",
        }),
        ConstValue::Error => Ok(1),   // hata kurtarma
        _ => Err(EvalError::TypeMismatch {
            span: self.span(expr),
            expected: "sayısal sabit",
        }),
    }
}

pub const MAX_WIDTH: u32 = 65_536;
pub const MAX_ARRAY_LEN: usize = 1_048_576;   // 1M eleman
```

**Sınırlar neden var:** `bits<1000000>` sentezlenemez, derleyici
belleği tüketir. Erken hata daha iyi.

---

## 8. Döngü Açma (Loop Unrolling)

```volt
for i in 0..4 {
    out[i] = in[i] & mask
}
```

`for` derleme zamanı açılıyor — donanımda döngü yok:

```rust
fn unroll_for(&mut self, f: &ForStmt) -> Result<Vec<Stmt>, EvalError> {
    let start = self.const_eval_i128(f.start)?;
    let end   = self.const_eval_i128(f.end)?;

    if end < start {
        return Err(EvalError::InvalidRange {
            span: f.span, start, end,
        });
    }

    let count = (end - start) as usize;
    if count > MAX_UNROLL {
        return Err(EvalError::UnrollTooLarge {
            span: f.span, count, max: MAX_UNROLL,
        });
    }

    let mut result = Vec::with_capacity(count);
    for i in start..end {
        // Döngü değişkenini sabit olarak bağla
        self.bind_loop_var(f.var, ConstValue::Int(i));
        result.extend(self.clone_body_with_substitution(f.body)?);
        self.unbind_loop_var(f.var);
    }
    Ok(result)
}

pub const MAX_UNROLL: usize = 65_536;
```

---

## 9. Hata Kodları

```
E2020  Döngüsel sabit bağımlılığı
E2021  Sabit ifade bekleniyor (çalışma zamanı değeri kullanıldı)
E2022  Derleme zamanı taşması
E2023  Sıfıra bölme
E2024  Geçersiz kaydırma miktarı
E2025  Geçersiz genişlik (0, negatif veya çok büyük)
E2026  Dizi boyutu sınır aşımı
E2027  Döngü açma sınırı aşıldı
E2028  Geçersiz aralık (end < start)
E2029  Sabit dizi indeksi sınır dışı

W2020  Sabit koşul — dal her zaman aynı sonuç veriyor
W2021  Kullanılmayan const bildirimi
```

---

## 10. Hata Mesajı Örnekleri

```
error[E2021]: sabit ifade bekleniyor
  ┌─ design.volt:7:18
  │
7 │     in data : bits<width_signal>
  │                    ^^^^^^^^^^^^ bu bir port (çalışma zamanı değeri)
  │
  = neden: tip genişlikleri derleme zamanında bilinmeli
  = çözüm: const WIDTH : u32 = 8; kullanın veya
           generic parametre ekleyin: module Foo<const W: u32>
  = daha fazla: volt explain E2021
```

```
error[E2022]: derleme zamanı taşması
  ┌─ design.volt:3:25
  │
3 │ const HUGE : u32 = 1 << 200;
  │                    ^^^^^^^^ 200 bit kaydırma
  │
  = neden: sonuç i128 aralığını aşıyor
  = çözüm: daha küçük bir kaydırma miktarı kullanın
```

---

## 11. Uygulama Sırası

```
F2a (3 gün):
  ConstValue, temel literal değerlendirme
  Aritmetik + bit işlemleri
  Taşma kontrolü (checked_*)

F2b (2 gün):
  const bildirimi çözümleme
  Döngüsel bağımlılık tespiti
  Önbellekleme

F2c (2 gün):
  Tip bağlamında kullanım (bits<N>, [T; N])
  clog2 ve diğer yerleşikler
  Sınır kontrolleri

F3 (2 gün):
  for döngüsü açma
  Generic const parametre çözümleme
```

---

## 12. Test Vektörleri

```
İFADE                          SONUÇ         NOT
─────────────────────────────────────────────────────────
42                             42
2 + 3 * 4                      14            öncelik
1 << 8                         256
clog2(64)                      6             adres genişliği
clog2(1)                       0             özel durum
clog2(3)                       2             yukarı yuvarlama
if true { 1 } else { 2 }       1             sabit koşul
[1, 2, 3][1]                   2             dizi indeksi
[0; 4]                         [0,0,0,0]     tekrar
(8 as u16)                     8             cast
WIDTH * 2  (WIDTH=8)           16            const referansı
State::Idle                    0             enum varyantı
─────────────────────────────────────────────────────────
port_signal                    E2021         çalışma zamanı
1 / 0                          E2023         sıfıra bölme
1 << 200                       E2024         kaydırma sınırı
bits<0>                        E2025         geçersiz genişlik
A=B+1, B=A+1                   E2020         döngüsel
for i in 5..2                  E2028         ters aralık
for i in 0..100000000          E2027         açma sınırı
```
