# Volt Tip Çıkarım Algoritması

> STATÜ: BAĞLAYICI
> İlgili: `grammar-full.ebnf`, `ast-nodes.md`, `domain-inference.md`
> Aşama: F2

---

## 0. Tasarım Kararı: Neden Hindley-Milner Değil

```
Hindley-Milner (Rust, Haskell, ML):
  Tip değişkenleri + unification + genelleştirme
  Amaç: "let id = |x| x" gibi polimorfik fonksiyonlar

Volt'un ihtiyacı FARKLI:
  Bit genişliği hesabı (u8 + u8 → u9)
  Taşma genişlemesi kuralları
  Donanım kaynak tüketimi belirlenmeli
  Polimorfizm sınırlı (sadece const generics)

KARAR: Çift yönlü (bidirectional) tip kontrolü
  ↓ checking mode:   "bu ifadenin tipi T olmalı"
  ↑ synthesis mode:  "bu ifadenin tipi nedir?"

Gerekçe:
  Basit (unification tablosu yok)
  Hata mesajları iyi (beklenen/bulunan net)
  Bit genişliği hesabı doğal oturuyor
  Donanımda tip değişkeni anlamsız (genişlik somut olmalı)
```

---

## 1. Tip Gösterimi

```rust
// crates/volt-hir/src/ty.rs

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(u32);

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// Tek bit mantıksal
    Bool,
    /// İşaretsiz tam sayı — width bit
    UInt { width: u16 },
    /// İşaretli tam sayı — width bit (işaret dahil)
    SInt { width: u16 },
    /// Ham bit vektörü — aritmetik YOK
    Bits { width: u16 },
    /// Dengeli üçlü {-1, 0, +1} — depolama 2 bit
    Trit,
    /// Saat sinyali
    Clock,
    /// Sıfırlama sinyali
    Reset { spec: ResetSpec },
    /// Sabit uzunluklu dizi
    Array { elem: TypeId, len: u32 },
    /// Demet
    Tuple(Vec<TypeId>),
    /// Kullanıcı tanımlı struct
    Struct(StructId),
    /// Kullanıcı tanımlı enum
    Enum(EnumId),
    /// Modül örneği (port erişimi için)
    Instance(ModuleId),
    /// Boyutlandırılmamış tamsayı literali — bağlamdan belirlenir
    IntLit,
    /// Hata kurtarma — her tiple uyumlu
    Error,
}
```

**Kritik: `Ty::Error`** — kaskad hata bastırma için. Herhangi bir
kontrolde `Error` görülürse sessizce yayılır, yeni hata üretilmez.

**Kritik: `Ty::IntLit`** — `let x = 5;` yazıldığında 5'in tipi
henüz belli değil. Bağlamdan çözülür (§4).

---

## 2. Çift Yönlü Algoritma

```rust
/// ↑ Sentez: "bu ifadenin tipi ne?"
fn synth(&mut self, expr: Idx<Expr>) -> TypeId;

/// ↓ Kontrol: "bu ifade T tipinde mi?"
fn check(&mut self, expr: Idx<Expr>, expected: TypeId) -> Result<()>;
```

Hangisinin kullanılacağı bağlama bağlı:

```
BAĞLAM                              MOD
──────────────────────────────────────────────────
let x: u8 = expr                    check(expr, u8)
let x = expr                        synth(expr)
port_out = expr                     check(expr, port_type)
reg r : u8 = expr                   check(expr, u8)
r <= expr                           check(expr, reg_type)
if cond { }                         check(cond, Bool)
a + b                               synth(a), synth(b)
f(arg)                              check(arg, param_type)
```

**Kural:** Beklenen tip biliniyorsa `check`, bilinmiyorsa `synth`.

---

## 3. Sentez Kuralları

### 3.1 Literaller

```rust
fn synth(&mut self, expr: Idx<Expr>) -> TypeId {
    match &self.ast.exprs[expr].kind {
        ExprKind::IntLit { value, suffix, .. } => {
            match suffix {
                // Açık sonek: 42u8
                Some(s) => {
                    let ty = self.suffix_to_ty(*s);
                    self.check_literal_fits(*value, ty, expr);
                    ty
                }
                // Soneksiz: 42 → IntLit, bağlamdan çözülecek
                None => self.mk_ty(Ty::IntLit),
            }
        }
        ExprKind::BoolLit(_) => self.mk_ty(Ty::Bool),
        // ...
    }
}
```

### 3.2 Değişken Referansı

```rust
ExprKind::Path(path) => {
    match self.resolve(path) {
        Some(def) => self.def_type(def),
        None => {
            self.error(E1001, path.span,
                format!("tanımsız isim: '{}'", path),
                self.suggest_similar_name(path));
            self.mk_ty(Ty::Error)
        }
    }
}
```

### 3.3 İkili Operatörler — Kritik Bölüm

```rust
ExprKind::Binary { op, lhs, rhs } => {
    let lt = self.synth(*lhs);
    let rt = self.synth(*rhs);
    self.binary_result_ty(*op, lt, rt, expr)
}
```

#### Aritmetik: Taşma Genişlemesi

```rust
fn arith_result(&mut self, op: BinOp, lt: TypeId, rt: TypeId,
                span: Span) -> TypeId {
    use Ty::*;
    match (self.ty(lt), self.ty(rt)) {
        // Hata yayılımı
        (Error, _) | (_, Error) => self.mk_ty(Error),

        // Literal + Literal → hâlâ literal
        (IntLit, IntLit) => self.mk_ty(IntLit),

        // Literal + somut → somut tipe uyarla
        (IntLit, _) => self.arith_result(op, rt, rt, span),
        (_, IntLit) => self.arith_result(op, lt, lt, span),

        // ── İŞARETSİZ ──
        (UInt { width: a }, UInt { width: b }) => {
            if a != b {
                self.error_width_mismatch(a, b, span);
                return self.mk_ty(Error);
            }
            let w = match op {
                // Toplama/çıkarma: 1 bit taşma
                BinOp::Add | BinOp::Sub => a + 1,
                // Çarpma: genişlikler toplanır
                BinOp::Mul => a * 2,
                // Bölme/mod: genişlemez
                BinOp::Div | BinOp::Rem => a,
                _ => unreachable!(),
            };
            self.mk_ty(UInt { width: w.min(MAX_WIDTH) })
        }

        // ── İŞARETLİ ──
        (SInt { width: a }, SInt { width: b }) => {
            if a != b {
                self.error_width_mismatch(a, b, span);
                return self.mk_ty(Error);
            }
            let w = match op {
                BinOp::Add | BinOp::Sub => a + 1,
                BinOp::Mul => a * 2,
                BinOp::Div | BinOp::Rem => a,
                _ => unreachable!(),
            };
            self.mk_ty(SInt { width: w.min(MAX_WIDTH) })
        }

        // ── İŞARET UYUMSUZLUĞU ──
        (UInt { .. }, SInt { .. }) | (SInt { .. }, UInt { .. }) => {
            self.error(E2002, span,
                "işaretli ve işaretsiz karıştırılamaz",
                "as ile açık dönüşüm yapın");
            self.mk_ty(Error)
        }

        // ── TRIT ──
        (Trit, Trit) => match op {
            // {-1,0,1} × {-1,0,1} ⊆ {-1,0,1} → kapalı
            BinOp::Mul => self.mk_ty(Trit),
            // +1 + +1 = +2 → taşma, i3'e genişle
            BinOp::Add | BinOp::Sub => self.mk_ty(SInt { width: 3 }),
            _ => { self.error_bad_op(op, span); self.mk_ty(Error) }
        },
        // Trit × sayı → sayının tipi (ternary MAC deseni)
        (Trit, SInt { width }) | (SInt { width }, Trit)
            if op == BinOp::Mul => self.mk_ty(SInt { width }),

        // ── BITS: aritmetik yasak ──
        (Bits { .. }, _) | (_, Bits { .. }) => {
            self.error(E2004, span,
                "bits<N> tipinde aritmetik yapılamaz",
                "u8/i8 gibi sayısal tipe dönüştürün");
            self.mk_ty(Error)
        }

        _ => { self.error_incompatible(lt, rt, span); self.mk_ty(Error) }
    }
}
```

#### Bit Düzeyi: Genişlemez

```rust
fn bitwise_result(&mut self, lt: TypeId, rt: TypeId,
                  span: Span) -> TypeId {
    match (self.ty(lt), self.ty(rt)) {
        (Error, _) | (_, Error) => self.mk_ty(Ty::Error),
        (Bool, Bool) => self.mk_ty(Ty::Bool),
        (UInt { width: a }, UInt { width: b }) if a == b
            => self.mk_ty(Ty::UInt { width: a }),
        (SInt { width: a }, SInt { width: b }) if a == b
            => self.mk_ty(Ty::SInt { width: a }),
        (Bits { width: a }, Bits { width: b }) if a == b
            => self.mk_ty(Ty::Bits { width: a }),
        _ => { self.error_width_mismatch_generic(lt, rt, span);
               self.mk_ty(Ty::Error) }
    }
}
```

#### Kaydırma: Sol Operandın Tipi

```rust
BinOp::Shl | BinOp::Shr => {
    // Sağ operand herhangi bir sayısal tip olabilir
    if !self.is_numeric(rt) {
        self.error(E2003, rhs_span, "kaydırma miktarı sayısal olmalı", "");
    }
    lt   // sonuç: sol operandın tipi (genişlemez)
}
```

#### Karşılaştırma: Her Zaman Bool

```rust
BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt
| BinOp::Le | BinOp::Ge => {
    self.unify_for_comparison(lt, rt, span);   // aynı tip olmalı
    self.mk_ty(Ty::Bool)
}
```

#### Mantıksal: Sadece Bool

```rust
BinOp::And | BinOp::Or => {
    self.check_is_bool(lt, lhs_span);
    self.check_is_bool(rt, rhs_span);
    self.mk_ty(Ty::Bool)
}
```

### 3.4 Tekli Operatörler

```rust
ExprKind::Unary { op, operand } => {
    let ot = self.synth(*operand);
    match op {
        // !x — sadece bool
        UnOp::Not => { self.check_is_bool(ot, span); self.mk_ty(Ty::Bool) }
        // ~x — bit tersleme, genişlik korunur
        UnOp::BitNot => ot,
        // -x — işaretli olmalı, 1 bit genişler
        UnOp::Neg => match self.ty(ot) {
            Ty::SInt { width } => self.mk_ty(Ty::SInt { width: width + 1 }),
            Ty::Trit => self.mk_ty(Ty::Trit),   // -Trit → Trit
            Ty::IntLit => self.mk_ty(Ty::IntLit),
            Ty::UInt { .. } => {
                self.error(E2002, span,
                    "işaretsiz değer negatiflenemez",
                    "önce i8/i16 gibi işaretli tipe dönüştürün");
                self.mk_ty(Ty::Error)
            }
            _ => self.mk_ty(Ty::Error),
        },
    }
}
```

### 3.5 Bit ve Aralık Seçimi

```rust
// x[3] → bool
ExprKind::Index { base, index } => {
    let bt = self.synth(*base);
    let width = self.width_of(bt)?;
    // Sabit indeks ise sınır kontrolü
    if let Some(i) = self.const_eval(*index) {
        if i >= width as u128 {
            self.error(E2006, span,
                format!("indeks {} sınır dışı (genişlik {})", i, width),
                format!("geçerli aralık: 0..{}", width - 1));
            return self.mk_ty(Ty::Error);
        }
    }
    self.mk_ty(Ty::Bool)
}

// x[7:4] → bits<4>
ExprKind::Range { base, hi, lo } => {
    let bt = self.synth(*base);
    let width = self.width_of(bt)?;
    match (self.const_eval(*hi), self.const_eval(*lo)) {
        (Some(h), Some(l)) => {
            if h < l {
                self.error(E2007, span, "aralık ters (hi < lo)",
                           format!("[{}:{}] yazın", l, h));
                return self.mk_ty(Ty::Error);
            }
            if h >= width as u128 {
                self.error(E2006, span, "aralık sınır dışı", "");
                return self.mk_ty(Ty::Error);
            }
            self.mk_ty(Ty::Bits { width: (h - l + 1) as u16 })
        }
        // Değişken aralık → sabit genişlik gerekli
        _ => {
            self.error(E2008, span,
                "aralık sınırları derleme zamanı sabiti olmalı",
                "değişken indeks için x[i +: WIDTH] kullanın");
            self.mk_ty(Ty::Error)
        }
    }
}

// ADR-0035: dizi tabanında indeks ELEMANI seçer; idx değişken olabilir.
// arr : [T; N], idx sayısal → arr[idx] : T
// Sabit idx'te derleme zamanı sınır denetimi (E2006); değişken idx'te
// denetim YOK — idx < N garantisi kontratla sağlanır (invariant: idx < N).

// ADR-0035: indexed part-select — x[i +: W] / x[i -: W] → bits<W>
// W derleme zamanı sabiti olmalı (değilse E2008) ve 1..=genişlik
// aralığında olmalı (değilse E2006). i değişken olabilir; i sabitse
// tüm seçim aralığı derleme zamanında denetlenir (E2006).
ExprKind::PartSelect { base, start, width, ascending } => {
    let bt = self.synth(*base);
    let base_width = self.width_of(bt)?;
    let Some(w) = self.const_eval(*width) else {
        self.error(E2008, span,
            "parça seçimi genişliği derleme zamanı sabiti olmalı",
            "WIDTH'i literal veya const yapın");
        return self.mk_ty(Ty::Error);
    };
    self.mk_ty(Ty::Bits { width: w as u16 })
}
```

### 3.6 Tip Dönüşümü (`as`)

```rust
ExprKind::Cast { expr, ty } => {
    let src = self.synth(*expr);
    let dst = self.resolve_type(*ty);
    self.check_cast_legal(src, dst, span);
    dst
}

fn check_cast_legal(&mut self, src: TypeId, dst: TypeId, span: Span) {
    use Ty::*;
    let legal = match (self.ty(src), self.ty(dst)) {
        (Error, _) | (_, Error) => true,
        // Genişletme — her zaman güvenli
        (UInt { width: a }, UInt { width: b }) if b >= a => true,
        (SInt { width: a }, SInt { width: b }) if b >= a => true,
        // Daraltma — izinli ama uyarı
        (UInt { width: a }, UInt { width: b }) if b < a => {
            self.warn(W2010, span,
                format!("{} bit → {} bit daraltma, üst bitler kesilir", a, b));
            true
        }
        // İşaret değişimi — açık cast ile OK
        (UInt { .. }, SInt { .. }) | (SInt { .. }, UInt { .. }) => true,
        // bits<N> ↔ sayısal, aynı genişlikte
        (Bits { width: a }, UInt { width: b }) if a == b => true,
        (UInt { width: a }, Bits { width: b }) if a == b => true,
        (Bits { width: a }, SInt { width: b }) if a == b => true,
        // Trit → sayısal (genişleme)
        (Trit, SInt { width }) if width >= 2 => true,
        // Sayısal → Trit YASAK (bilgi kaybı, sessiz kırpma)
        (_, Trit) => false,
        // Bool ↔ 1-bit
        (Bool, UInt { width: 1 }) | (UInt { width: 1 }, Bool) => true,
        _ => false,
    };
    if !legal {
        self.error(E2009, span,
            format!("'{}' → '{}' dönüşümü geçersiz",
                    self.fmt(src), self.fmt(dst)),
            "ara dönüşüm gerekebilir");
    }
}
```

### 3.7 Koşullu İfade

```rust
ExprKind::If { cond, then_expr, else_expr } => {
    self.check(*cond, self.bool_ty());
    let tt = self.synth(*then_expr);
    // else dalı then'in tipinde olmalı
    self.check(*else_expr, tt);
    tt
}
```

---

## 4. Kontrol Modu ve Literal Çözümleme

`check` modunun asıl işi: soneksiz literalleri bağlamdan çözmek.

```rust
fn check(&mut self, expr: Idx<Expr>, expected: TypeId) -> Result<()> {
    // Error her tiple uyumlu — sessiz yayılım
    if self.is_error(expected) { return Ok(()); }

    match &self.ast.exprs[expr].kind {
        // Literal: beklenen tipe uyarla
        ExprKind::IntLit { value, suffix: None, .. } => {
            match self.ty(expected) {
                Ty::UInt { width } => {
                    if *value >= (1u128 << width) {
                        return self.err_literal_overflow(*value, width, expr);
                    }
                    self.record_ty(expr, expected);
                    Ok(())
                }
                Ty::SInt { width } => {
                    let max = 1i128 << (width - 1);
                    if (*value as i128) >= max {
                        return self.err_literal_overflow(*value, width, expr);
                    }
                    self.record_ty(expr, expected);
                    Ok(())
                }
                Ty::Trit => {
                    if !matches!(*value, 0 | 1) {
                        return self.err(E2011, expr,
                            "Trit literali {-1, 0, +1} olmalı", "");
                    }
                    self.record_ty(expr, expected);
                    Ok(())
                }
                Ty::Bool => self.err(E2003, expr,
                    "sayısal literal bool bağlamında",
                    "true veya false yazın"),
                _ => self.err_type_mismatch(expected, expr),
            }
        }

        // İkili operatör: her iki tarafa da beklenen tipi it
        ExprKind::Binary { op, lhs, rhs } if op.is_arithmetic() => {
            // Genişleme kuralı nedeniyle doğrudan check edilemez
            // Sentez yapıp sonucu karşılaştır
            let actual = self.synth(expr);
            self.expect_assignable(actual, expected, expr)
        }

        // Koşullu: her iki dala da it
        ExprKind::If { cond, then_expr, else_expr } => {
            self.check(*cond, self.bool_ty())?;
            self.check(*then_expr, expected)?;
            self.check(*else_expr, expected)
        }

        // Diğerleri: sentezle ve karşılaştır
        _ => {
            let actual = self.synth(expr);
            self.expect_assignable(actual, expected, expr)
        }
    }
}
```

---

## 5. Atanabilirlik Kuralı

```rust
/// actual tipindeki değer expected tipine atanabilir mi?
fn expect_assignable(&mut self, actual: TypeId, expected: TypeId,
                     expr: Idx<Expr>) -> Result<()> {
    if actual == expected { return Ok(()); }
    if self.is_error(actual) || self.is_error(expected) { return Ok(()); }

    use Ty::*;
    match (self.ty(actual), self.ty(expected)) {
        // Literal her sayısal tipe uyar (sınır kontrolü yapıldı)
        (IntLit, UInt { .. } | SInt { .. } | Trit) => Ok(()),

        // DARALTMA — örtük YASAK
        (UInt { width: a }, UInt { width: b }) if a > b => {
            self.error(E2001, self.span(expr),
                format!("{} bit değer {} bit hedefe sığmaz", a, b),
                format!("açık kesme için: (ifade) as u{}", b))
        }

        // GENİŞLEME — örtük YASAK (açıklık ilkesi)
        (UInt { width: a }, UInt { width: b }) if a < b => {
            self.error(E2001, self.span(expr),
                format!("{} bit değer {} bit hedefe örtük genişlemez", a, b),
                format!("açık genişletme: (ifade) as u{}", b))
        }

        // İŞARET UYUMSUZLUĞU
        (UInt { .. }, SInt { .. }) | (SInt { .. }, UInt { .. }) => {
            self.error(E2002, self.span(expr),
                "işaret uyumsuzluğu",
                "as ile açık dönüşüm yapın")
        }

        _ => self.err_type_mismatch(expected, expr),
    }
}
```

**Tasarım kararı: örtük genişleme bile yasak.**

```
Gerekçe: donanımda genişleme BEDAVA DEĞİL
  u8 → u16 zero-extend: 8 tel daha
  i8 → i16 sign-extend: 8 tel + fanout

Kullanıcı bunu görmeli. Rust aynı kararı verdi.
```

---

## 6. Deyim Seviyesi Kontrol

```rust
fn check_stmt(&mut self, stmt: Idx<Stmt>) {
    match &self.ast.stmts[stmt].kind {
        StmtKind::Reg(r) => {
            let ty = match &r.ty {
                // Açık tip: reg count : u8 = 0
                Some(t) => self.resolve_type(*t),
                // Çıkarım: reg count = 0  → hata (belirsiz)
                None => {
                    let inferred = self.synth(r.init);
                    if self.is_int_lit(inferred) {
                        self.error(E2012, r.span,
                            "register tipi belirlenemiyor",
                            "reg count : u8 = 0 şeklinde tip yazın");
                        self.mk_ty(Ty::Error)
                    } else { inferred }
                }
            };
            self.check(r.init, ty);
            self.declare(r.name, ty, DefKind::Register);
        }

        StmtKind::Let(l) => {
            let ty = match &l.ty {
                Some(t) => { let ty = self.resolve_type(*t);
                             self.check(l.value, ty); ty }
                None => {
                    let ty = self.synth(l.value);
                    // Boyutsuz literal varsayılan: i32
                    if self.is_int_lit(ty) {
                        self.warn(W2012, l.span,
                            "tip belirtilmedi, i32 varsayıldı");
                        self.mk_ty(Ty::SInt { width: 32 })
                    } else { ty }
                }
            };
            self.declare(l.name, ty, DefKind::Wire);
        }

        StmtKind::Assign(a) => {
            let lhs_ty = self.lvalue_type(&a.lhs);
            self.check(a.rhs, lhs_ty);
            self.record_driver(&a.lhs, a.span);   // E4001 için
        }

        // ...
    }
}
```

---

## 7. Hata Kodları

```
E2001  Bit genişliği uyumsuzluğu
E2002  İşaret uyumsuzluğu
E2003  Tip uyumsuzluğu (genel)
E2004  bits<N> tipinde aritmetik
E2005  Literal genişliği belirlenemiyor
E2006  İndeks/aralık sınır dışı
E2007  Ters aralık (hi < lo)
E2008  Değişken aralık sınırı
E2009  Geçersiz tip dönüşümü
E2010  Literal hedef tipe sığmıyor
E2011  Geçersiz Trit literali
E2012  Register tipi belirlenemiyor

W2010  Daraltıcı dönüşüm (bilgi kaybı)
W2011  Kullanılmayan tip parametresi
W2012  Tip belirtilmedi, varsayılan kullanıldı
```

---

## 8. Hata Mesajı Şablonu

UX Anayasası'nın 5 parçalı formatı:

```
error[E2001]: bit genişliği uyumsuzluğu
  ┌─ design.volt:12:15
  │
12│     sum = small + large
  │           ^^^^^   ^^^^^ 'large' → u16
  │           │
  │           'small' → u8
  │
  = neden: farklı genişlikte değerler örtük olarak
           birleştirilemez; donanımda genişletme
           ek tel ve mantık gerektirir
  = çözüm: (small as u16) + large
  = daha fazla: volt explain E2001
```

---

## 9. Uygulama Sırası

```
F2a (2 hafta):
  Ty enum, TypeId arena
  Literal + değişken sentezi
  Bool ve UInt temel

F2b (2 hafta):
  Aritmetik genişleme kuralları
  Bit düzeyi operatörler
  check/synth çift yönlü akış

F2c (1 hafta):
  Cast kuralları
  Bit/aralık seçimi
  Sınır kontrolü

F2d (1 hafta):
  Trit tipi
  bits<N> kısıtları
  Hata mesajı cilası
```

---

## 10. Test Vektörleri

```
İFADE                       BEKLENEN TİP     NOT
────────────────────────────────────────────────────────
42                          IntLit           bağlam bekliyor
42u8                        u8               sonek
a + b   (a,b: u8)           u9               taşma genişlemesi
a * b   (a,b: u8)           u16              çarpım genişlemesi
a & b   (a,b: u8)           u8               genişlemez
a == b  (a,b: u8)           bool             karşılaştırma
!flag   (flag: bool)        bool
~a      (a: u8)             u8               genişlemez
-x      (x: i8)             i9               negasyon genişler
a[3]    (a: u8)             bool             bit seçimi
a[7:4]  (a: u8)             bits<4>          aralık
a as u16 (a: u8)            u16              açık genişletme
t * t   (t: Trit)           Trit             kapalı
t + t   (t: Trit)           i3               taşma
t * x   (t:Trit, x:i8)      i8               ternary MAC
────────────────────────────────────────────────────────
a + b   (a:u8, b:u16)       HATA E2001
a + b   (a:u8, b:i8)        HATA E2002
a + b   (a,b: bits<8>)      HATA E2004
a[9]    (a: u8)             HATA E2006
x as Trit                   HATA E2009
```

---

## 11. Sürücü Analizi (E4xxx)

Tip kontrolüyle aynı geçişte yapılır — her sinyalin kaç kez
sürüldüğü takip edilir.

### 11.1 Sürücü Tablosu

```rust
pub struct DriverTable {
    /// sinyal → onu süren atamaların konumları
    drivers: HashMap<DefId, Vec<Span>>,
}

impl DriverTable {
    fn record(&mut self, target: DefId, span: Span) {
        self.drivers.entry(target).or_default().push(span);
    }
}
```

### 11.2 E4001 — Çift Sürücü

```rust
fn check_multiple_drivers(&mut self) {
    for (def, spans) in &self.drivers.drivers {
        if spans.len() > 1 {
            let name = self.name_of(*def);
            self.error(E4001, spans[1],
                format!("'{}' zaten sürülüyor", name),
                "tek bir atama kullanın veya koşullu ifade yazın")
                .with_secondary(spans[0], "ilk atama burada");
        }
    }
}
```

**İstisna:** Aynı `on` bloğu içindeki koşullu atamalar tek
sürücü sayılır:

```volt
on clk {
    if a      { r <= 1 }   // ✓ aynı blok
    else if b { r <= 2 }   // ✓ tek sürücü
    else      { r <= 3 }
}
```

### 11.3 E4002 — Sürücüsüz Çıkış

```rust
fn check_undriven_outputs(&mut self, module: &ModuleDecl) {
    for port in module.ports.iter().filter(|p| p.direction == PortDir::Out) {
        let def = self.def_of_port(port);
        if !self.drivers.drivers.contains_key(&def) {
            self.error(E4002, port.span,
                format!("'{}' çıkış portu sürülmüyor", port.name),
                format!("{} = ... şeklinde bir atama ekleyin", port.name));
        }
    }
}
```

**Neden hata, uyarı değil:** Sürücüsüz çıkış SystemVerilog'da
yüksek empedans (Z) üretir. Sentezde tanımsız davranış.

### 11.4 E4003/E4004 — Lineer Tipler [V1]

```rust
// &inv T tipindeki portlar tam bir kez tüketilmeli
E4003  Lineer port çift tüketim
E4004  Lineer port tüketilmedi
```

### 11.5 Hata Kodları

```
E4001  Çift sürücü
E4002  Sürücüsüz çıkış portu
E4003  Lineer port çift tüketim [V1]
E4004  Lineer port tüketilmedi [V1]

W4001  Kullanılmayan sinyal (_ öneki ile susturulur)
W4002  Yazılıp hiç okunmayan register
```

### 11.6 Analiz Sırası

```
1. Tip kontrolü geçişi
   → Aynı anda sürücü kaydı tutulur

2. Modül sonu
   → check_multiple_drivers()  (E4001)
   → check_undriven_outputs()  (E4002)

3. Kullanım analizi
   → W4001, W4002
```
