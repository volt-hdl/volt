# Volt AST Düğüm Spesifikasyonu

> STATÜ: BAĞLAYICI
> Kaynak gramer: `docs/spec/grammar-full.ebnf`
> Her düğüm gramerdeki bir kurala karşılık gelir.

---

## 0. Tasarım İlkeleri

```
İ1. ARENA TABANLI
    Düğümler Vec<T> içinde tutulur, Idx<T> ile referans verilir.
    Gerekçe: döngüsel referans mümkün, cache dostu, Salsa uyumlu.

İ2. SPAN HER DÜĞÜMDE
    Her düğüm kaynak konumunu taşır.
    Gerekçe: hata mesajı "= çözüm:" satırı için konum şart.

İ3. AST EKSİK OLABİLİR
    Hata kurtarma sırasında Error düğümü üretilir.
    Gerekçe: tek sözdizimi hatası tüm analizi durdurmamalı.

İ4. AST ≠ CST
    CST (cstree) tüm token'ları tutar — formatter ve LSP için.
    AST anlamsal yapıdır — tip kontrolü ve lowering için.
    AST, CST'den türetilir.

İ5. TİP BİLGİSİ AST'DE YOK
    AST sözdizimsel yapıdır. Tip bilgisi HIR'da yaşar.
    Gerekçe: aşamaların ayrılması, yeniden kullanılabilirlik.
```

---

## 1. Arena ve Handle Altyapısı

```rust
// crates/volt-ast/src/arena.rs

use std::marker::PhantomData;

/// Tip-güvenli arena indeksi
pub struct Idx<T> {
    raw: u32,
    _marker: PhantomData<fn() -> T>,
}

// Manuel impl: T: Clone gerektirmemek için
impl<T> Clone for Idx<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for Idx<T> {}
impl<T> PartialEq for Idx<T> {
    fn eq(&self, o: &Self) -> bool { self.raw == o.raw }
}
impl<T> Eq for Idx<T> {}
impl<T> std::hash::Hash for Idx<T> {
    fn hash<H: std::hash::Hasher>(&self, s: &mut H) { self.raw.hash(s) }
}

pub struct Arena<T> {
    items: Vec<T>,
}

impl<T> Arena<T> {
    pub fn alloc(&mut self, value: T) -> Idx<T> {
        let idx = self.items.len() as u32;
        self.items.push(value);
        Idx { raw: idx, _marker: PhantomData }
    }
}

impl<T> std::ops::Index<Idx<T>> for Arena<T> {
    type Output = T;
    fn index(&self, idx: Idx<T>) -> &T {
        &self.items[idx.raw as usize]
    }
}
```

```rust
// Kaynak konumu — volt-span crate'i
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub start: u32,   // byte offset
    pub end: u32,
    pub file: FileId,
}
```

---

## 2. Kök Yapı

```rust
/// Bir dosyanın tam AST'si
pub struct SourceFile {
    pub package: Option<PackageDecl>,
    pub uses: Vec<UseDecl>,
    pub items: Vec<Idx<Item>>,

    // Tüm arenalar burada — düğümler Idx ile referans verir
    pub items_arena: Arena<Item>,
    pub exprs: Arena<Expr>,
    pub stmts: Arena<Stmt>,
    pub types: Arena<TypeRef>,
    pub patterns: Arena<Pattern>,
    pub blocks: Arena<Block>,

    /// Ayrıştırma sırasında toplanan hatalar
    pub errors: Vec<ParseError>,
}
```

---

## 3. Öğe Düğümleri (Items)

```rust
pub struct Item {
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub doc: Option<DocComment>,
    pub visibility: Visibility,
    pub kind: ItemKind,
}

pub enum Visibility { Private, Public }

pub enum ItemKind {
    Module(ModuleDecl),
    Domain(DomainDecl),
    Fn(FnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Const(ConstDecl),
    TypeAlias(TypeAlias),
    Extern(ExternDecl),
    /// Hata kurtarma: ayrıştırılamayan öğe
    Error,
}
```

### 3.1 Modül

```rust
pub struct ModuleDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub ports: Vec<Port>,
    pub contracts: Vec<Contract>,
    pub body: Vec<Idx<Stmt>>,
    /// `} module Counter` sonlandırıcısı varsa
    pub closing_name: Option<Name>,
}

pub struct Port {
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub doc: Option<DocComment>,
    pub direction: PortDir,
    pub name: Name,
    pub ty: Idx<TypeRef>,
    /// @DomainName anotasyonu
    pub domain: Option<Name>,
}

pub enum PortDir { In, Out, InOut }
```

### 3.2 Domain

```rust
pub struct DomainDecl {
    pub name: Name,
    pub fields: Vec<DomainField>,
}

pub struct DomainField {
    pub span: Span,
    pub key: DomainKey,
    pub value: DomainValue,
}

pub enum DomainKey {
    // Saat
    Clock, Frequency,
    // Sıfırlama
    Reset, ResetCycles, ResetSequence,
    // Güç [V1]
    Voltage, AlwaysOn, Retention, Isolation,
    // Güvenlik [V1]
    TrustLevel,
    /// Bilinmeyen anahtar — W0020 uyarısı
    Unknown(Name),
}

pub enum DomainValue {
    ClockEdge(ClockEdge),
    Reset(ResetSpec),
    Trust(TrustLevel),
    Isolation(IsolationKind),
    Literal(Idx<Expr>),
    Bool(bool),
    Error,
}

pub enum ClockEdge { Posedge, Negedge, None }

pub struct ResetSpec {
    pub sync: ResetSync,
    pub polarity: ResetPolarity,
}

pub enum ResetSync { Sync, Async, None }
pub enum ResetPolarity { ActiveHigh, ActiveLow }
pub enum TrustLevel { Secret, Confidential, Public }
pub enum IsolationKind { ClampLow, ClampHigh, Latch }
```

### 3.3 Fonksiyon ve Diğerleri

```rust
pub struct FnDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_ty: Option<Idx<TypeRef>>,
    pub contracts: Vec<Contract>,
    pub body: Idx<Block>,
}

pub struct Param {
    pub span: Span,
    pub name: Name,
    pub ty: Idx<TypeRef>,
}

pub struct StructDecl {
    pub name: Name,
    /// `struct port` — lineer tip [V1]
    pub is_port: bool,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<StructField>,
}

pub struct StructField {
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub doc: Option<DocComment>,
    pub name: Name,
    pub ty: Idx<TypeRef>,
}

pub struct EnumDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    /// `enum State : bits<2>` — temel tip
    pub repr: Option<Idx<TypeRef>>,
    pub variants: Vec<EnumVariant>,
}

pub struct EnumVariant {
    pub span: Span,
    pub doc: Option<DocComment>,
    pub name: Name,
    pub data: VariantData,
    /// `Idle = 0` açık değer
    pub discriminant: Option<Idx<Expr>>,
}

pub enum VariantData {
    Unit,
    Tuple(Vec<Idx<TypeRef>>),
    Struct(Vec<StructField>),
}

pub struct ConstDecl {
    pub name: Name,
    pub ty: Idx<TypeRef>,
    pub value: Idx<Expr>,
}

pub struct TypeAlias {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub target: Idx<TypeRef>,
}

pub struct ExternDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub ports: Vec<Port>,
}
```

---

## 4. Tip Referansları

```rust
/// AST'deki tip GÖSTERİMİ — çözümlenmemiş
/// Gerçek tip HIR'da hesaplanır
pub struct TypeRef {
    pub span: Span,
    pub kind: TypeRefKind,
}

pub enum TypeRefKind {
    Bool,
    Clock,
    Reset(Option<ResetSpec>),
    UInt(u8),        // 8, 16, 32, 64
    SInt(u8),
    Bits(Idx<Expr>), // bits<N> — N derleme zamanı ifadesi
    Trit,
    Array { elem: Idx<TypeRef>, len: Idx<Expr> },
    Tuple(Vec<Idx<TypeRef>>),
    /// &T veya &inv T [V1]
    Ref { inverted: bool, inner: Idx<TypeRef> },
    /// Kullanıcı tanımlı: Path + generic argümanlar
    Path { path: Path, args: Vec<GenericArg> },
    /// Hata kurtarma
    Error,
}

pub enum GenericArg {
    Type(Idx<TypeRef>),
    Const(Idx<Expr>),
}

pub struct GenericParam {
    pub span: Span,
    pub kind: GenericParamKind,
}

pub enum GenericParamKind {
    Type { name: Name, bounds: Vec<Path> },
    Const { name: Name, ty: Idx<TypeRef> },
}
```

---

## 5. Deyimler (Statements)

```rust
pub struct Stmt {
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub kind: StmtKind,
}

pub enum StmtKind {
    Reg(RegDecl),
    Let(LetDecl),
    Wire(WireDecl),
    Instance(InstanceDecl),
    On(OnBlock),
    Comb(Idx<Block>),
    Assign(AssignStmt),
    For(ForStmt),
    Expr(Idx<Expr>),
    Error,
}

pub struct RegDecl {
    pub name: Name,
    /// reg(clk) — açık domain; None ise çıkarım yapılır
    pub domain: Option<Name>,
    pub ty: Option<Idx<TypeRef>>,
    pub init: Idx<Expr>,
}

pub struct LetDecl {
    pub name: Name,
    pub ty: Option<Idx<TypeRef>>,
    pub value: Idx<Expr>,
}

pub struct WireDecl {
    pub name: Name,
    pub ty: Idx<TypeRef>,
}

pub struct InstanceDecl {
    pub name: Name,
    pub module_path: Path,
    pub generic_args: Vec<GenericArg>,
    pub bindings: Vec<PortBinding>,
}

pub struct PortBinding {
    pub span: Span,
    pub port_name: Name,
    /// `clk: clk` kısayolunda None → port_name kullanılır
    pub value: Option<Idx<Expr>>,
}

pub struct OnBlock {
    pub trigger: OnTrigger,
    pub body: Idx<Block>,
}

pub enum OnTrigger {
    /// on clk
    Clock(Name),
    /// on clk.reset
    Reset(Name),
    /// on spike(input) [V2]
    Spike(Name),
    Error,
}

pub struct AssignStmt {
    pub lhs: LValue,
    pub rhs: Idx<Expr>,
}

pub struct ForStmt {
    pub var: Name,
    pub start: Idx<Expr>,
    pub end: Idx<Expr>,
    pub body: Idx<Block>,
}
```

### 5.1 LValue

```rust
pub struct LValue {
    pub span: Span,
    pub base: Name,
    pub suffixes: Vec<LValueSuffix>,
}

pub enum LValueSuffix {
    /// x[3]
    Index(Idx<Expr>),
    /// x[7:4]
    Range { hi: Idx<Expr>, lo: Idx<Expr> },
    /// x.field
    Field(Name),
}
```

---

## 6. Bloklar

```rust
pub struct Block {
    pub span: Span,
    pub stmts: Vec<BlockStmt>,
    /// Fonksiyon gövdesinde son ifade → dönüş değeri
    pub tail: Option<Idx<Expr>>,
    /// Sıralı mı kombinasyonel mi — parser bağlamdan doldurur
    pub context: BlockContext,
}

pub enum BlockContext {
    /// on ... { } içinde — '<=' zorunlu
    Sequential,
    /// comb { } içinde — '=' zorunlu
    Combinational,
    /// fn gövdesi — atama yok
    Function,
}

pub enum BlockStmt {
    /// x <= expr
    NonBlockAssign { lhs: LValue, rhs: Idx<Expr>, span: Span },
    /// x = expr
    BlockAssign { lhs: LValue, rhs: Idx<Expr>, span: Span },
    If(IfStmt),
    Match(MatchStmt),
    Let(LetDecl),
    For(ForStmt),
    Error,
}

pub struct IfStmt {
    pub span: Span,
    pub cond: Idx<Expr>,
    pub then_block: Idx<Block>,
    pub else_branch: Option<ElseBranch>,
}

pub enum ElseBranch {
    Block(Idx<Block>),
    If(Box<IfStmt>),
}

pub struct MatchStmt {
    pub span: Span,
    pub scrutinee: Idx<Expr>,
    pub arms: Vec<MatchArm>,
}

pub struct MatchArm {
    pub span: Span,
    pub pattern: Idx<Pattern>,
    /// `Some(x) if x > 0 =>`
    pub guard: Option<Idx<Expr>>,
    pub body: MatchArmBody,
}

pub enum MatchArmBody {
    Block(Idx<Block>),
    Expr(Idx<Expr>),
}
```

---

## 7. İfadeler

```rust
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

pub enum ExprKind {
    // ─── Literaller ───
    IntLit { value: u128, suffix: Option<IntSuffix>, base: NumBase },
    BoolLit(bool),
    StringLit(String),
    UnitLit { value: u128, unit: Unit },

    // ─── Referanslar ───
    Path(Path),

    // ─── Operatörler ───
    Binary { op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr> },
    Unary { op: UnOp, operand: Idx<Expr> },

    // ─── Erişim ───
    Index { base: Idx<Expr>, index: Idx<Expr> },
    Range { base: Idx<Expr>, hi: Idx<Expr>, lo: Idx<Expr> },
    Field { base: Idx<Expr>, field: Name },
    Call { callee: Idx<Expr>, args: Vec<Idx<Expr>> },
    Cast { expr: Idx<Expr>, ty: Idx<TypeRef> },

    // ─── Kontrol ───
    If { cond: Idx<Expr>, then_expr: Idx<Expr>, else_expr: Idx<Expr> },
    Match { scrutinee: Idx<Expr>, arms: Vec<MatchArm> },

    // ─── Yapı literalleri ───
    StructLit { path: Path, fields: Vec<FieldInit> },
    ArrayLit(ArrayLitKind),
    TupleLit(Vec<Idx<Expr>>),

    // ─── Özel ───
    /// todo!("mesaj") — tip kontrolünden geçer, sim'de durur
    Todo { message: Option<String> },

    /// Hata kurtarma
    Error,
}

pub enum NumBase { Dec, Hex, Bin, Oct }

pub enum IntSuffix { U8, U16, U32, U64, I8, I16, I32, I64 }

pub enum Unit {
    Hz, Khz, Mhz, Ghz,
    Ps, Ns, Us, Ms, S,
    Fj, Pj, Nj, Uj,
    Uw, Mw, W,
    Um2, Mm2,
    V, Mv,
}

pub enum ArrayLitKind {
    /// [a, b, c]
    List(Vec<Idx<Expr>>),
    /// [value; count]
    Repeat { value: Idx<Expr>, count: Idx<Expr> },
}

pub struct FieldInit {
    pub span: Span,
    pub name: Name,
    /// `Foo { x }` kısayolunda None
    pub value: Option<Idx<Expr>>,
}
```

### 7.1 Operatörler

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinOp {
    // Aritmetik
    Add, Sub, Mul, Div, Rem,
    // Bit düzeyi
    BitAnd, BitOr, BitXor, Shl, Shr,
    // Karşılaştırma (BİRLEŞMEZ)
    Eq, Ne, Lt, Gt, Le, Ge,
    // Mantıksal
    And, Or,
}

impl BinOp {
    /// E0010 kontrolü için
    pub fn is_comparison(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne
                     | BinOp::Lt | BinOp::Gt
                     | BinOp::Le | BinOp::Ge)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnOp {
    /// !x — mantıksal değilleme
    Not,
    /// ~x — bit değilleme
    BitNot,
    /// -x — negatifleme
    Neg,
}
```

---

## 8. Desenler

```rust
pub struct Pattern {
    pub span: Span,
    pub kind: PatternKind,
}

pub enum PatternKind {
    /// _
    Wildcard,
    /// 42, true
    Literal(Idx<Expr>),
    /// x — bağlama
    Binding(Name),
    /// State::Idle veya Some(x)
    Path { path: Path, args: Option<PatternArgs> },
    /// (a, b)
    Tuple(Vec<Idx<Pattern>>),
    /// A | B
    Or(Vec<Idx<Pattern>>),
    Error,
}

pub enum PatternArgs {
    Tuple(Vec<Idx<Pattern>>),
    Struct(Vec<FieldPattern>),
}

pub struct FieldPattern {
    pub span: Span,
    pub name: Name,
    /// `Foo { x }` kısayolunda None
    pub pattern: Option<Idx<Pattern>>,
}
```

---

## 9. Kontrat ve Nitelik

```rust
pub struct Contract {
    pub span: Span,
    pub kind: ContractKind,
    pub expr: Idx<Expr>,
}

pub enum ContractKind {
    Requires, Ensures, Invariant, Cover, Assert, Assume,
}

pub struct Attribute {
    pub span: Span,
    pub name: Name,
    pub args: Vec<AttrArg>,
}

pub enum AttrArg {
    /// @budget(lut = 5000)
    Named { name: Name, value: Idx<Expr> },
    /// @synthesis_target(asic)
    Positional(Idx<Expr>),
}
```

---

## 10. Ortak Yapılar

```rust
/// İnterned string — bellek verimliliği
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Name(SymbolId);

pub struct Path {
    pub span: Span,
    pub segments: Vec<Name>,
}

pub struct DocComment {
    pub span: Span,
    pub text: String,
}

pub struct PackageDecl {
    pub span: Span,
    pub path: Path,
}

pub struct UseDecl {
    pub span: Span,
    pub path: Path,
    pub tree: Option<UseTree>,
}

pub enum UseTree {
    Glob,
    List(Vec<Path>),
    Alias { path: Path, alias: Name },
}
```

---

## 11. Düğüm Sayısı Özeti

```
KATEGORİ            DÜĞÜM SAYISI    AŞAMA
────────────────────────────────────────────
Item türleri        8 + Error       F0-F3
Tip referansları    11 + Error      F0-F3
Deyim türleri       9 + Error       F0-F3
Blok deyimleri      6 + Error       F0-F3
İfade türleri       17 + Error      F0-F3
Desen türleri       6 + Error       F3
────────────────────────────────────────────
TOPLAM              57 + 6 Error
```

**F0 için gereken minimum alt küme (12 düğüm):**

```
ItemKind::Module
Port, PortDir
TypeRefKind: Bool, Clock, UInt
StmtKind: Reg, On, Assign
BlockStmt: NonBlockAssign, If
ExprKind: IntLit, Path, Binary
BinOp: Add
```

---

## 12. Ziyaretçi (Visitor) Deseni

```rust
pub trait Visitor {
    fn visit_item(&mut self, item: &Item) { walk_item(self, item) }
    fn visit_expr(&mut self, expr: &Expr) { walk_expr(self, expr) }
    fn visit_stmt(&mut self, stmt: &Stmt) { walk_stmt(self, stmt) }
    fn visit_type(&mut self, ty: &TypeRef) { walk_type(self, ty) }
    fn visit_pattern(&mut self, pat: &Pattern) { walk_pattern(self, pat) }
}

// Varsayılan yürüyüş fonksiyonları — Visitor override edebilir
pub fn walk_expr<V: Visitor + ?Sized>(v: &mut V, expr: &Expr) { ... }
```

**Kullanım alanları:**
- İsim toplama (F1)
- Kullanılmayan sinyal tespiti (F2)
- Domain çıkarımı (F2)
- HIR'a dönüştürme (F2)

---

## 13. AST'de OLMAYAN Bilgi

Bu bilgiler **HIR'da** hesaplanır:

```
✗ Çözümlenmiş tipler        → HIR: TypeId
✗ İsim çözümleme sonucu     → HIR: DefId
✗ Domain ataması            → HIR: DomainId
✗ Bit genişlikleri          → HIR: hesaplanmış
✗ Sabit ifade değerleri     → HIR: const eval sonucu
✗ Kontrol akış grafiği      → MIR/lowering
```

Bu ayrım kritik: AST sözdizimseldir, anlamsal analiz sonrası gelmez.

---

## 14. Test Stratejisi

```rust
// tests/unit/ast_test.rs

#[test]
fn counter_ast_structure() {
    let src = include_str!("../fixtures/counter.volt");
    let ast = parse(src);

    assert_eq!(ast.errors.len(), 0);
    assert_eq!(ast.items.len(), 1);

    let module = ast.expect_module(0);
    assert_eq!(module.name.as_str(), "Counter");
    assert_eq!(module.ports.len(), 3);
    assert_eq!(module.body.len(), 3);  // reg, on, assign
}

#[test]
fn error_recovery_produces_partial_ast() {
    let src = "module Foo { in a : ; out b : u8  b = a }";
    //                            ^ tip eksik
    let ast = parse(src);

    assert_eq!(ast.errors.len(), 1);
    // Hataya rağmen ikinci port ayrıştırılmalı
    let module = ast.expect_module(0);
    assert_eq!(module.ports.len(), 2);
    assert!(matches!(
        ast.types[module.ports[0].ty].kind,
        TypeRefKind::Error
    ));
}
```
