//! Soyut sözdizimi ağacı: docs/spec/ast-nodes.md'deki düğüm tanımları.
//!
//! F1 kapsamı: tüm öğe türleri (module, domain, fn, struct, enum, const,
//! type alias, extern module), package/use, generics, kontratlar,
//! nitelikler, match + desenler, for, comb, wire, modül örnekleme ve tam
//! ifade kümesi. Her enum hata kurtarma için `Error` varyantı taşır
//! (ast-nodes.md İ3).

pub mod arena;
pub mod builtin;

pub use arena::{Arena, Idx};
use volt_span::Span;

/// Kaynak isim. Spec interning öngörür (Name(SymbolId)); F0/F1'de
/// basitlik için metin taşınır — HIR aşamasında intern edilecek.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub span: Span,
    pub segments: Vec<Name>,
}

/// Bir dosyanın tam AST'si (ast-nodes.md §2).
#[derive(Debug, Default)]
pub struct SourceFile {
    pub package: Option<PackageDecl>,
    pub uses: Vec<UseDecl>,
    pub items: Vec<Idx<Item>>,
    pub items_arena: Arena<Item>,
    pub exprs: Arena<Expr>,
    pub stmts: Arena<Stmt>,
    pub types: Arena<TypeRef>,
    pub patterns: Arena<Pattern>,
    pub blocks: Arena<Block>,
}

impl SourceFile {
    /// Test yardımcıları: i'inci öğe modülse döndür.
    pub fn module(&self, i: usize) -> Option<&ModuleDecl> {
        match &self.items_arena[*self.items.get(i)?].kind {
            ItemKind::Module(m) => Some(m),
            _ => None,
        }
    }

    pub fn domain(&self, i: usize) -> Option<&DomainDecl> {
        match &self.items_arena[*self.items.get(i)?].kind {
            ItemKind::Domain(d) => Some(d),
            _ => None,
        }
    }
}

// ═══ Program başı bildirimleri ════════════════════════════════════

#[derive(Debug)]
pub struct PackageDecl {
    pub span: Span,
    pub path: Path,
}

#[derive(Debug)]
pub struct UseDecl {
    pub span: Span,
    pub path: Path,
    pub tree: Option<UseTree>,
}

#[derive(Debug)]
pub enum UseTree {
    /// `use foo::*`
    Glob,
    /// `use foo::{a, b::c}`
    List(Vec<Path>),
    /// `use foo::bar as baz` — tam yol `path` alanında, takma ad burada.
    Alias(Name),
}

// ═══ Öğeler ═══════════════════════════════════════════════════════

#[derive(Debug)]
pub struct Item {
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub doc: Option<String>,
    pub visibility: Visibility,
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Public,
}

#[derive(Debug)]
pub enum ItemKind {
    Module(ModuleDecl),
    Domain(DomainDecl),
    Fn(FnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Const(ConstDecl),
    TypeAlias(TypeAlias),
    Extern(ExternDecl),
    /// `test "ad" { ... }` — simülasyon testi (ADR-0033).
    Test(TestDecl),
    /// Hata kurtarma: ayrıştırılamayan öğe.
    Error,
}

#[derive(Debug)]
pub struct ModuleDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub ports: Vec<Port>,
    pub contracts: Vec<Contract>,
    pub body: Vec<Idx<Stmt>>,
    /// `} module Counter` sonlandırıcısı varsa.
    pub closing_name: Option<Name>,
}

#[derive(Debug)]
pub struct Port {
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub doc: Option<String>,
    pub direction: PortDir,
    pub name: Name,
    pub ty: Idx<TypeRef>,
    /// `@DomainName` anotasyonu.
    pub domain: Option<Name>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDir {
    In,
    Out,
    InOut,
}

#[derive(Debug)]
pub struct FnDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_ty: Option<Idx<TypeRef>>,
    pub contracts: Vec<Contract>,
    pub body: Idx<Block>,
}

#[derive(Debug)]
pub struct Param {
    pub span: Span,
    pub name: Name,
    pub ty: Idx<TypeRef>,
}

#[derive(Debug)]
pub struct StructDecl {
    pub name: Name,
    /// `struct port` — lineer tip [V1]; şimdilik yalnız ayrıştırılır.
    pub is_port: bool,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<StructField>,
}

#[derive(Debug)]
pub struct StructField {
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub doc: Option<String>,
    pub name: Name,
    pub ty: Idx<TypeRef>,
}

#[derive(Debug)]
pub struct EnumDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    /// `enum State : bits<2>` — temel tip.
    pub repr: Option<Idx<TypeRef>>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug)]
pub struct EnumVariant {
    pub span: Span,
    pub doc: Option<String>,
    pub name: Name,
    pub data: VariantData,
    /// `Idle = 0` açık değer.
    pub discriminant: Option<Idx<Expr>>,
}

#[derive(Debug)]
pub enum VariantData {
    Unit,
    Tuple(Vec<Idx<TypeRef>>),
    Struct(Vec<StructField>),
}

#[derive(Debug)]
pub struct ConstDecl {
    pub name: Name,
    pub ty: Idx<TypeRef>,
    pub value: Idx<Expr>,
}

#[derive(Debug)]
pub struct TypeAlias {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub target: Idx<TypeRef>,
}

#[derive(Debug)]
pub struct ExternDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub ports: Vec<Port>,
}

/// `test "ad" { ... }` bloğu (ADR-0033). Gövde donanım değil doğrusal
/// betik olduğundan modül deyim arenalarını kullanmaz; kendi küçük
/// deyim/ifade türlerini taşır.
#[derive(Debug)]
pub struct TestDecl {
    /// String literal içeriği (tırnaklar hariç), ör. `counter increments`.
    pub name: String,
    pub name_span: Span,
    pub stmts: Vec<TestStmt>,
}

impl TestDecl {
    /// Cargo biçimli raporda görünen ad: boşluklar `_` olur.
    pub fn display_name(&self) -> String {
        self.name.replace(' ', "_")
    }
}

/// Test gövdesi deyimi (grammar-full.ebnf TestStmt).
#[derive(Debug)]
pub enum TestStmt {
    /// `let dut = Counter { };`
    LetDut {
        span: Span,
        name: Name,
        module: Name,
    },
    /// `dut.port = <ifade>;`
    SetPort {
        span: Span,
        dut: Name,
        port: Name,
        value: TestExpr,
    },
    /// `step(1);`, `reset();`, `assert_eq(a, b);` ...
    Call {
        span: Span,
        func: Name,
        args: Vec<TestExpr>,
    },
}

/// Test gövdesi ifadesi (grammar-full.ebnf TestExpr).
#[derive(Debug)]
pub struct TestExpr {
    pub span: Span,
    pub kind: TestExprKind,
}

#[derive(Debug)]
pub enum TestExprKind {
    Int(u64),
    Bool(bool),
    /// `dut.port` okuması.
    PortRead {
        dut: Name,
        port: Name,
    },
}

#[derive(Debug)]
pub struct DomainDecl {
    pub name: Name,
    pub fields: Vec<DomainField>,
}

#[derive(Debug)]
pub struct DomainField {
    pub span: Span,
    pub key: DomainKey,
    pub value: DomainValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainKey {
    Clock,
    Frequency,
    Reset,
    ResetCycles,
    ResetSequence,
    /// Bilinmeyen anahtar — W0020 uyarısı.
    Unknown(Name),
}

#[derive(Debug)]
pub enum DomainValue {
    ClockEdge(ClockEdge),
    Reset(ResetSpec),
    Literal(Idx<Expr>),
    Bool(bool),
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockEdge {
    Posedge,
    Negedge,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetSpec {
    pub sync: ResetSync,
    pub polarity: ResetPolarity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetSync {
    Sync,
    Async,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetPolarity {
    ActiveHigh,
    ActiveLow,
}

// ═══ Kontrat ve nitelik ═══════════════════════════════════════════

#[derive(Debug)]
pub struct Contract {
    pub span: Span,
    pub kind: ContractKind,
    pub expr: Idx<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractKind {
    Requires,
    Ensures,
    Invariant,
    Cover,
    Assert,
    Assume,
}

#[derive(Debug)]
pub struct Attribute {
    pub span: Span,
    pub name: Name,
    pub args: Vec<AttrArg>,
}

#[derive(Debug)]
pub enum AttrArg {
    /// `@budget(lut = 5000)`
    Named { name: Name, value: Idx<Expr> },
    /// `@synthesis_target(asic)`
    Positional(Idx<Expr>),
}

// ═══ Generics ═════════════════════════════════════════════════════

#[derive(Debug)]
pub struct GenericParam {
    pub span: Span,
    pub kind: GenericParamKind,
}

#[derive(Debug)]
pub enum GenericParamKind {
    /// `<T: Bound + Other>`
    Type { name: Name, bounds: Vec<Path> },
    /// `<const N: u32>`
    Const { name: Name, ty: Idx<TypeRef> },
}

#[derive(Debug)]
pub enum GenericArg {
    Type(Idx<TypeRef>),
    Const(Idx<Expr>),
}

// ═══ Tip referansları ═════════════════════════════════════════════

#[derive(Debug)]
pub struct TypeRef {
    pub span: Span,
    pub kind: TypeRefKind,
}

#[derive(Debug)]
pub enum TypeRefKind {
    Bool,
    Clock,
    /// `reset` veya `reset(sync, active_high)`.
    Reset(Option<ResetSpec>),
    UInt(u8),
    SInt(u8),
    /// `bits<N>` — N derleme zamanı ifadesi.
    Bits(Idx<Expr>),
    Trit,
    /// `[T; N]`
    Array {
        elem: Idx<TypeRef>,
        len: Idx<Expr>,
    },
    /// `(T, U)`
    Tuple(Vec<Idx<TypeRef>>),
    /// Kullanıcı tanımlı: Path + generic argümanlar.
    Path {
        path: Path,
        args: Vec<GenericArg>,
    },
    /// Hata kurtarma.
    Error,
}

// ═══ Deyimler ═════════════════════════════════════════════════════

#[derive(Debug)]
pub struct Stmt {
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub kind: StmtKind,
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct RegDecl {
    pub name: Name,
    /// `reg(clk)` — açık domain; None ise çıkarım yapılır.
    pub domain: Option<Name>,
    pub ty: Option<Idx<TypeRef>>,
    pub init: Idx<Expr>,
}

#[derive(Debug)]
pub struct LetDecl {
    pub name: Name,
    pub ty: Option<Idx<TypeRef>>,
    pub value: Idx<Expr>,
}

#[derive(Debug)]
pub struct WireDecl {
    pub name: Name,
    pub ty: Idx<TypeRef>,
}

/// `let u = Uart { clk: clk }` — grammar-full.ebnf §19 [N3]:
/// geri izleme değil, '=' sonrası yapı literali görülünce
/// yeniden sınıflandırma.
#[derive(Debug)]
pub struct InstanceDecl {
    pub name: Name,
    pub module_path: Path,
    pub generic_args: Vec<GenericArg>,
    pub bindings: Vec<PortBinding>,
}

#[derive(Debug)]
pub struct PortBinding {
    pub span: Span,
    pub port_name: Name,
    /// `clk: clk` kısayolunda None → port_name kullanılır.
    pub value: Option<Idx<Expr>>,
}

#[derive(Debug)]
pub struct OnBlock {
    pub trigger: OnTrigger,
    pub body: Idx<Block>,
}

#[derive(Debug)]
pub enum OnTrigger {
    /// `on clk`
    Clock(Name),
    /// `on clk.reset`
    Reset(Name),
    Error,
}

#[derive(Debug)]
pub struct AssignStmt {
    pub lhs: LValue,
    pub rhs: Idx<Expr>,
}

/// `for i in 0..N { ... }` — yalnız derleme zamanı (generate) döngüsü;
/// açma F2'de yapılır, burada yalnız ayrıştırılır.
#[derive(Debug)]
pub struct ForStmt {
    pub var: Name,
    pub start: Idx<Expr>,
    pub end: Idx<Expr>,
    pub body: Idx<Block>,
}

#[derive(Debug)]
pub struct LValue {
    pub span: Span,
    pub base: Name,
    pub suffixes: Vec<LValueSuffix>,
}

#[derive(Debug)]
pub enum LValueSuffix {
    /// `x[3]`
    Index(Idx<Expr>),
    /// `x[7:4]`
    Range { hi: Idx<Expr>, lo: Idx<Expr> },
    /// `x.field`
    Field(Name),
}

// ═══ Bloklar ══════════════════════════════════════════════════════

#[derive(Debug)]
pub struct Block {
    pub span: Span,
    pub stmts: Vec<BlockStmt>,
    /// Fonksiyon gövdesinde son ifade → dönüş değeri.
    pub tail: Option<Idx<Expr>>,
    /// Sıralı mı kombinasyonel mi — parser bağlamdan doldurur.
    pub context: BlockContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockContext {
    /// `on ... { }` içinde — '<=' zorunlu.
    Sequential,
    /// `comb { }` / `for { }` içinde — '=' zorunlu.
    Combinational,
    /// `fn` gövdesi — donanım ataması yok.
    Function,
}

#[derive(Debug)]
pub enum BlockStmt {
    /// `x <= expr`
    NonBlockAssign {
        lhs: LValue,
        rhs: Idx<Expr>,
        span: Span,
    },
    /// `x = expr`
    BlockAssign {
        lhs: LValue,
        rhs: Idx<Expr>,
        span: Span,
    },
    If(IfStmt),
    Match(MatchStmt),
    Let(LetDecl),
    For(ForStmt),
    Error,
}

#[derive(Debug)]
pub struct IfStmt {
    pub span: Span,
    pub cond: Idx<Expr>,
    pub then_block: Idx<Block>,
    pub else_branch: Option<ElseBranch>,
}

#[derive(Debug)]
pub enum ElseBranch {
    Block(Idx<Block>),
    If(Box<IfStmt>),
}

#[derive(Debug)]
pub struct MatchStmt {
    pub span: Span,
    pub scrutinee: Idx<Expr>,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug)]
pub struct MatchArm {
    pub span: Span,
    pub pattern: Idx<Pattern>,
    /// `Some(x) if x > 0 =>`
    pub guard: Option<Idx<Expr>>,
    pub body: MatchArmBody,
}

#[derive(Debug)]
pub enum MatchArmBody {
    Block(Idx<Block>),
    Expr(Idx<Expr>),
}

// ═══ Desenler ═════════════════════════════════════════════════════

#[derive(Debug)]
pub struct Pattern {
    pub span: Span,
    pub kind: PatternKind,
}

#[derive(Debug)]
pub enum PatternKind {
    /// `_`
    Wildcard,
    /// `42`, `true`
    Literal(Idx<Expr>),
    /// `x` — bağlama
    Binding(Name),
    /// `State::Idle` veya `Some(x)`
    Path {
        path: Path,
        args: Option<PatternArgs>,
    },
    /// `(a, b)`
    Tuple(Vec<Idx<Pattern>>),
    /// `A | B`
    Or(Vec<Idx<Pattern>>),
    Error,
}

#[derive(Debug)]
pub enum PatternArgs {
    Tuple(Vec<Idx<Pattern>>),
    Struct(Vec<FieldPattern>),
}

#[derive(Debug)]
pub struct FieldPattern {
    pub span: Span,
    pub name: Name,
    /// `Foo { x }` kısayolunda None.
    pub pattern: Option<Idx<Pattern>>,
}

// ═══ İfadeler ═════════════════════════════════════════════════════

#[derive(Debug)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Debug)]
pub enum ExprKind {
    IntLit {
        value: u128,
        suffix: Option<IntSuffix>,
        base: NumBase,
    },
    BoolLit(bool),
    StringLit(String),
    Path(Path),
    Binary {
        op: BinOp,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Idx<Expr>,
    },
    Index {
        base: Idx<Expr>,
        index: Idx<Expr>,
    },
    Range {
        base: Idx<Expr>,
        hi: Idx<Expr>,
        lo: Idx<Expr>,
    },
    Field {
        base: Idx<Expr>,
        field: Name,
    },
    Call {
        callee: Idx<Expr>,
        args: Vec<Idx<Expr>>,
    },
    Cast {
        expr: Idx<Expr>,
        ty: Idx<TypeRef>,
    },
    If {
        cond: Idx<Expr>,
        then_expr: Idx<Expr>,
        else_expr: Idx<Expr>,
    },
    Match {
        scrutinee: Idx<Expr>,
        arms: Vec<MatchArm>,
    },
    StructLit {
        path: Path,
        fields: Vec<FieldInit>,
    },
    ArrayLit(ArrayLitKind),
    TupleLit(Vec<Idx<Expr>>),
    /// `todo!("mesaj")` — tip kontrolünden geçer, sim'de durur.
    Todo {
        message: Option<String>,
    },
    /// Hata kurtarma.
    Error,
}

#[derive(Debug)]
pub enum ArrayLitKind {
    /// `[a, b, c]`
    List(Vec<Idx<Expr>>),
    /// `[değer; adet]`
    Repeat { value: Idx<Expr>, count: Idx<Expr> },
}

#[derive(Debug)]
pub struct FieldInit {
    pub span: Span,
    pub name: Name,
    /// `Foo { x }` kısayolunda None.
    pub value: Option<Idx<Expr>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumBase {
    Dec,
    Hex,
    Bin,
    Oct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntSuffix {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

impl BinOp {
    /// E0010 kontrolü için.
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
        )
    }

    /// S-ifade dökümü ve hata mesajları için.
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `!x` — mantıksal değilleme
    Not,
    /// `~x` — bit değilleme
    BitNot,
    /// `-x` — negatifleme
    Neg,
}

impl UnOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnOp::Not => "!",
            UnOp::BitNot => "~",
            UnOp::Neg => "-",
        }
    }
}
