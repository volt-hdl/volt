> UYARI: Bu belge geçersizdir.
> Güncel sürüm: docs/design/ altında.

# Volt HDL — F0–F1 Teknik Tasarım Dokümanı (Önyüz)

> **Kapsam:** Yol haritasının **F0 (Temel & Yürüyen İskelet, Ay 1)** ve **F1 (Önyüz
> & Parser, Ay 2–3)** fazları. Bu belge bir uygulayıcının doğrudan kodlamaya
> başlayabileceği somutlukta hedefler: workspace/crate ayrımı, crate sözleşmeleri,
> veri modelleri (CST/AST/HIR), lexer ve parser mimarisi, hata kurtarma stratejisi,
> isim çözünümü, ve F0 yürüyen-iskelet lowering'i.
>
> **Faz sonu çıktısı:** Hata-toleranslı bir önyüz — bir `.volt` dosyasını lexer →
> CST → tipli AST → isim-çözümlü HIR (`BoundAst`) hâline getiren; bozuk girdide bile
> konumlu tanılarla kısmi ağaç üreten; ve (F0'dan kalan) şablon lowering ile uçtan
> uca SystemVerilog üretebilen bir boru hattı.

---

## 1. Amaç ve Tasarım İlkeleri

F0–F1, derleyicinin "okuma ve anlama" katmanını kurar. Tip kontrolü (F2) ve CIRCT
lowering (F3) buraya **bağımlıdır**, dolayısıyla burada alınan veri-modeli kararları
tüm projeyi şekillendirir. İlkeler:

1. **Kayıpsızlık (lossless):** CST, kaynağın *her* baytını (boşluk, yorum, hatalı
   token dahil) korur. Bu, LSP'nin error-recovery'si, `volt fmt` ve doğru tanı
   konumları için zorunludur.
2. **Hata toleransı her yerde:** Parser asla panik atmaz; her girdide bir ağaç +
   tanı listesi üretir. "İlk hatada dur" yoktur.
3. **Konum her şeydir:** Her token ve düğüm bir `TextRange` taşır; tanılar bu
   aralıklara bağlanır. Konum kaybı = kötü hata mesajı.
4. **Katmanlı ve tek-yönlü bağımlılık:** `span → diagnostics → syntax → ast → hir`.
   Geri bağımlılık yok; her katman bağımsız test edilir.
5. **rust-analyzer mimarisinin kanıtlanmış desenleri:** olay-tabanlı parser,
   green/red ağaç, `Idx<T>` arena handle'ları, tipli AST view'ları. Tekerleği
   yeniden icat etmeyiz.

---

## 2. Mimari Genel Bakış

### 2.1 Veri Akışı

```
kaynak metin (&str)
   │  Lexer (volt-syntax)
   ▼
Token akışı  [(SyntaxKind, TextRange)]   + lexer tanıları
   │  Parser (olay-tabanlı, volt-syntax)
   ▼
Parser olayları [Start/Token/Finish/Error]
   │  TreeBuilder (cstree)
   ▼
GreenNode (kayıpsız CST)  ──► SyntaxNode (red, tembel, tipli erişim için)
   │  AST view (volt-ast)
   ▼
Tipli AST (SyntaxNode üzerinde ince sarmalayıcılar)
   │  Lowering + İsim Çözünümü (volt-hir):  UnboundAst → BoundAst
   ▼
HIR (arena-tabanlı, çözümlü referanslar, modül imza tablosu Σ)
   │  [F0 şablon lowering | F3 CIRCT lowering]
   ▼
SystemVerilog
```

### 2.2 Crate Grafiği

```
volt-cli ──────────────────────────────────┐
   │ depends on                              │
   ▼                                          ▼
volt-driver ──► volt-hir ──► volt-ast ──► volt-syntax ──► volt-diagnostics ──► volt-span
   │                                                                              ▲
   └──► volt-lower (F0 stub / F3) ───────────────────────────────────────────────┘
```

Ok yönü "bağımlıdır"ı gösterir. `volt-span` ve `volt-diagnostics` yapraktır; üst
katmanlar bunları paylaşır.

---

## 3. Repo ve Workspace Yapısı

```
volt/
├── Cargo.toml                 # [workspace] + ortak lint/profil ayarları
├── rust-toolchain.toml        # sabit toolchain sürümü
├── deny.toml                  # cargo-deny: lisans/güvenlik denetimi
├── .github/workflows/ci.yml   # build · test · clippy · fmt · fuzz-smoke
├── crates/
│   ├── volt-span/             # TextSize, TextRange, FileId, SourceMap
│   ├── volt-diagnostics/      # Diagnostic, Severity, error kodları, renderer
│   ├── volt-syntax/           # SyntaxKind, lexer, parser, CST (cstree)
│   ├── volt-ast/              # AstNode view'ları (SyntaxNode sarmalayıcı)
│   ├── volt-hir/              # arena, HIR düğümleri, isim çözünümü, Σ
│   ├── volt-lower/            # F0: şablon SV; F3: CIRCT köprüsü
│   ├── volt-driver/           # dosya yükleme, faz orkestrasyonu
│   ├── volt-cli/              # `volt` ikili (build/parse/fmt-dump)
│   └── volt-lsp/              # F5 — şimdilik boş iskelet
├── corpus/
│   ├── valid/                 # geçerli .volt + golden CST/AST/HIR/SV
│   ├── invalid/               # bozuk .volt + golden tanılar
│   └── fuzz-seeds/            # cargo-fuzz tohum korpusu
└── docs/adr/                  # mimari karar kayıtları (ADR-0001 ...)
```

**Workspace `Cargo.toml` ortak ayarları:** `[workspace.lints]` ile tüm crate'lere
`clippy::all`, `unsafe_code = "forbid"` (FFI'nin yaşadığı `volt-lower` hariç),
`missing_docs` (public API). Release profili `lto = "thin"`, `codegen-units = 1`.

---

## 4. Crate Sözleşmeleri (Public API Özetleri)

### 4.1 `volt-span`

```rust
pub type TextSize = u32;                  // bayt ofset (UTF-8)
pub struct TextRange { start: TextSize, end: TextSize }
pub struct FileId(pub u32);               // SourceMap içindeki dosya kimliği

pub struct SourceMap { /* FileId -> (yol, içerik, satır indeksi) */ }
impl SourceMap {
    pub fn add(&mut self, path: PathBuf, text: String) -> FileId;
    pub fn text(&self, f: FileId) -> &str;
    pub fn line_col(&self, f: FileId, off: TextSize) -> (u32 /*satır*/, u32 /*sütun*/);
}
```
Tüm konumlar **bayt ofsetidir** (UTF-8). Satır/sütun yalnızca tanı render'ında
hesaplanır. `TextRange` aritmetiği `text-size` crate'inden alınabilir.

### 4.2 `volt-diagnostics`

```rust
pub enum Severity { Error, Warning, Note, Help }

pub struct Diagnostic {
    pub code: DiagCode,           // örn. E0220
    pub severity: Severity,
    pub message: String,          // tek satır özet
    pub primary: Label,           // ^^^ ile işaretlenen ana aralık
    pub secondary: Vec<Label>,    // ilişkili aralıklar (---- ile)
    pub help: Vec<String>,        // "öneri:" satırları (düzeltme)
}
pub struct Label { pub file: FileId, pub range: TextRange, pub text: String }

pub struct DiagCode(pub u16);     // 0220 -> "E0220"
pub mod codes { /* sabitler + insan-okunur açıklamalar kataloğu */ }

pub trait Emitter { fn emit(&mut self, d: &Diagnostic, sm: &SourceMap); }
pub struct TtyEmitter;            // renkli, Elm/Rust tarzı (codespan-reporting)
pub struct JsonEmitter;           // --message-format=json (AI ajanları için)
```
**Render hedefi (örnek):**
```
hata[E0220]: 'count' sinyali tüm yollarda atanmamış (örtük latch)
  ┌─ alu.volt:14:9
14│     if cond { count = a }
  │              ^^^^^^^^^^^ 'else' dalında atama yok
  = öneri: 'else' dalında da 'count'a değer ver veya varsayılan tanımla.
```
Tanı kataloğu (`codes`) ile spesifikasyondaki `E02xx/E03xx/E04xx` **senkron** tutulur;
her kod için tek bir kaynak-of-truth açıklaması bulunur.

### 4.3 `volt-syntax`

```rust
pub enum SyntaxKind { /* token + node, tek enum (§7.1) */ }
pub fn lex(text: &str) -> (Vec<LexedToken>, Vec<Diagnostic>);
pub fn parse(text: &str) -> Parse;        // ana giriş
pub struct Parse { green: GreenNode, errors: Vec<Diagnostic> }
impl Parse {
    pub fn syntax(&self) -> SyntaxNode;   // red ağaç kökü
    pub fn errors(&self) -> &[Diagnostic];
}
// cstree tip takması:
pub type SyntaxNode = cstree::SyntaxNode<VoltLang>;
pub type SyntaxToken = cstree::SyntaxToken<VoltLang>;
```

### 4.4 `volt-ast`

```rust
pub trait AstNode { fn cast(node: SyntaxNode) -> Option<Self>; fn syntax(&self) -> &SyntaxNode; }
pub struct SourceFile(SyntaxNode);
pub struct ModuleDecl(SyntaxNode);
impl ModuleDecl {
    pub fn name(&self) -> Option<Name>;
    pub fn generics(&self) -> Option<GenericParams>;
    pub fn items(&self) -> impl Iterator<Item = ModuleItem>;
}
// ... her gramer üretimi için bir AstNode tipi
```

### 4.5 `volt-hir`

```rust
pub struct Hir { pub modules: Arena<ModuleData>, pub sigs: SignatureTable /* Σ */, ... }
pub fn lower_file(ast: &SourceFile, sm: &SourceMap) -> (Hir, Vec<Diagnostic>);

pub struct Idx<T>(u32, PhantomData<T>);   // la-arena tarzı handle
pub struct Arena<T> { /* Vec<T> + Idx erişimi */ }
```

---

## 5. `volt-span` — Kaynak Konum Modeli (F0)

- Tek kaynak doğruluğu: `SourceMap` tüm dosyaları tutar; `FileId` her yerde taşınır.
- `TextRange` tüm token ve düğümlerde; CST zaten her düğüm için aralık verir, ama
  HIR'de orijinale geri bağlamak için handle→range eşlemesi de saklanır.
- Satır indeksi tembel hesaplanır (`line_starts: Vec<TextSize>`), tanı render'ında
  ikili arama ile satır/sütun bulunur.

**Çıkış kriteri (F0):** `SourceMap` + `TextRange` aritmetiği + `line_col` testli.

---

## 6. `volt-diagnostics` — Tanı Altyapısı (F0)

**Tasarım kararları:**
- Tanı **veri**dir, string değil: kod, aralıklar, etiketler, öneriler ayrı alanlar.
  Render'dan tamamen ayrılır (TTY vs JSON aynı veriden).
- Her tanı **en az bir** somut öneri taşımaya teşvik edilir (zorunlu değil ama PR
  incelemesinde aranır).
- `codes` modülü, spesifikasyon hata kodlarının kanonik listesidir; yeni kural
  eklemek = burada yeni kod + açıklama + en az bir negatif korpus dosyası.

**F0'da uygulanan:** `Diagnostic`, `Label`, `Severity`, `DiagCode`, `TtyEmitter`
(codespan-reporting tabanlı), `JsonEmitter` iskeleti, `codes` ilk girişleri (lexer +
parser hataları). Snapshot testleriyle render kilitlenir (`insta`).

---

## 7. `volt-syntax` — Lexer (F1, Hafta 1–2)

### 7.1 `SyntaxKind` Tek Enum

Token + node tek enum'da (cstree gereği). Örnek (kısaltılmış):

```rust
#[repr(u16)]
pub enum SyntaxKind {
    // --- Trivia ---
    WHITESPACE, LINE_COMMENT, BLOCK_COMMENT, DOC_COMMENT,
    // --- Literaller ---
    DEC_INT, HEX_INT, BIN_INT, SIZED_INT, TIME_LIT, TRUE_KW, FALSE_KW,
    // --- Tanımlayıcı ---
    IDENT, TYPE_IDENT,
    // --- Anahtar kelimeler ---
    DOMAIN_KW, MODULE_KW, IN_KW, OUT_KW, INOUT_KW, REG_KW, WIRE_KW, ON_KW,
    IF_KW, ELSE_KW, MATCH_KW, FSM_KW, STATE_KW, INIT_KW, PIPELINE_KW, STAGE_KW,
    RULE_KW, LET_KW, CONST_KW, ENUM_KW, STRUCT_KW, BUNDLE_KW, NEWTYPE_KW,
    EXTERN_KW, PUB_KW, IMPORT_KW, AS_KW, TEST_KW, ASSERT_KW, INVARIANT_KW,
    COVER_KW, EXPECT_KW, TICK_KW, FORALL_KW, SYNC_KW, ASYNC_KW,
    POSEDGE_KW, NEGEDGE_KW, AFTER_KW, CYCLES_KW, EVENTUALLY_KW, WITHIN_KW,
    // --- Noktalama / operatör ---
    L_BRACE, R_BRACE, L_PAREN, R_PAREN, L_BRACK, R_BRACK,
    COMMA, COLON, COLON_COLON, SEMI, DOT, AT, ARROW_FAT, // =>
    EQ, LE_ASSIGN, /* <= as registered update */ STREAM, /* >> */ BIDIR, /* <> */
    PLUS, PLUS_PCT, /* +% */ PLUS_PIPE, /* +| */ MINUS, STAR, SLASH,
    AMP, PIPE, CARET, TILDE, BANG, AMP_AMP, PIPE_PIPE,
    EQ_EQ, NEQ, LT, GT, LTEQ, GTEQ, SHL, SHR,
    IMPL, /* |-> */ IMPL_NEXT, /* |=> */
    // --- Düğümler (nodes) ---
    SOURCE_FILE, MODULE_DECL, DOMAIN_DECL, ENUM_DECL, STRUCT_DECL, BUNDLE_DECL,
    NEWTYPE_DECL, PORT_DECL, REG_DECL, WIRE_DECL, ON_BLOCK, RULE_BLOCK,
    FSM_DECL, PIPELINE_DECL, STAGE_DECL, INST_DECL, CONNECT_STMT, REG_UPDATE,
    IF_STMT, MATCH_STMT, MATCH_ARM, BLOCK, LET_BIND, ASSERT_STMT, INVARIANT_STMT,
    COVER_STMT, TEST_DECL, TYPE_REF, GENERIC_PARAMS, GENERIC_PARAM,
    BIN_EXPR, UNARY_EXPR, CALL_EXPR, FIELD_EXPR, INDEX_EXPR, PAREN_EXPR,
    CAST_EXPR, NAME, NAME_REF, PATH, ARG_LIST,
    // --- Özel ---
    ERROR,         // hata kurtarma düğümü
    EOF,
}
```

`LE_ASSIGN` (`<=`) burada **registered update** operatörüdür; bağlamsal anlamı
parser/AST değil, F2 tip kuralları belirler. Lexer yalnızca token üretir.

### 7.2 Lexer Tasarımı

- **Araç:** `logos` (hızlı, türev-tabanlı) ya da elle yazım. Sized literal (`8'd42`)
  ve `42.cycles` gibi bağlamsal token'lar için elle düzeltme katmanı gerekebilir;
  `logos` + post-process önerilir.
- **Trivia korunur:** boşluk ve yorumlar atılmaz; token akışına dahil edilir (CST
  kayıpsızlığı için). Parser bunları "trivia" olarak atlar ama ağaca yapışık tutar.
- **Hatalı karakter:** bilinmeyen bayt → `ERROR` token + `E0001: unexpected character`
  tanısı; lexer **devam eder** (durmaz).
- **Doc comment** (`///`) ayrı kind: AST'de modül/öğe dokümantasyonuna bağlanır.

**Lexer çıktısı:**
```rust
pub struct LexedToken { pub kind: SyntaxKind, pub range: TextRange }
```

**Test (F1):** her token sınıfı için golden testler; sized/time literal kenar
durumları; bozuk girdide ERROR token + tanı; `insta` snapshot.

**Çıkış kriteri (Hafta 2):** Korpustaki tüm dosyalar token akışına dönüşüyor;
trivia korunuyor; fuzz-smoke (rastgele bayt) panik yapmıyor.

---

## 8. `volt-syntax` — CST Modeli (F1, Hafta 2–3)

### 8.1 Neden cstree

`cstree`, `rowan`'ın **dize tekilleştirmeli (interned)** varyantıdır:
- **GreenNode:** değişmez, paylaşımlı, konumdan bağımsız; yeşil ağaç bütün
  kaynakları kapsar. Interning sayesinde tekrar eden tanımlayıcılar tek kopya.
- **SyntaxNode (red):** yeşil ağaç üzerinde tembel, ebeveyn-konumlu görünüm; mutlak
  `TextRange` ve gezinme (parent/children/siblings) burada.
- **Error recovery dostu:** `ERROR` düğümleri ağaca dahil edilir; ağaç asla "yarım"
  kalmaz.

`VoltLang` dil tipi cstree'ye `SyntaxKind`'ı bağlar:
```rust
pub enum VoltLang {}
impl cstree::Language for VoltLang {
    type Kind = SyntaxKind;
    fn kind_from_raw(r: cstree::RawKind) -> SyntaxKind { /* transmute u16 */ }
    fn kind_to_raw(k: SyntaxKind) -> cstree::RawKind { k as u16 }
}
```

### 8.2 Ağaç Şekli (Örnek)

`module Counter { in enable: bool ... }` için:
```
SOURCE_FILE
  MODULE_DECL
    MODULE_KW "module"  WHITESPACE " "
    NAME ( TYPE_IDENT "Counter" )  WHITESPACE " "
    L_BRACE "{"  WHITESPACE "\n  "
    PORT_DECL
      IN_KW "in"  WHITESPACE " "
      NAME ( IDENT "enable" )  COLON ":"  WHITESPACE " "
      TYPE_REF ( IDENT "bool" )
    ...
    R_BRACE "}"
```
Her token ve düğüm `TextRange` taşır; boşluk/yorum ağaçta yaşar.

---

## 9. `volt-syntax` — Parser (F1, Hafta 3–6) — *F1'in kalbi*

### 9.1 Olay-Tabanlı Mimari (rust-analyzer deseni)

Parser doğrudan ağaç inşa etmez; düz bir **olay listesi** üretir, sonra
`TreeBuilder` bunu `GreenNode`'a çevirir. Bu, ileri-bakış (lookahead) ve geri-alma
(checkpoint) ile hata kurtarmayı temizler.

```rust
pub enum Event {
    Start { kind: SyntaxKind, forward_parent: Option<usize> },
    Token { kind: SyntaxKind },
    Finish,
    Error { diag: Diagnostic },
}

pub struct Parser<'t> {
    tokens: &'t [LexedToken],   // trivia hariç "anlamlı" akış + ayrı trivia eşlemesi
    pos: usize,
    events: Vec<Event>,
    fuel: Cell<u32>,            // sonsuz döngü koruması (her bump'ta resetlenir)
}
```

Çekirdek ilkeller:
```rust
impl Parser {
    fn nth(&self, n: usize) -> SyntaxKind;       // ileri bakış
    fn at(&self, k: SyntaxKind) -> bool;
    fn at_set(&self, set: &TokenSet) -> bool;
    fn bump(&mut self, k: SyntaxKind);           // token tüket
    fn eat(&mut self, k: SyntaxKind) -> bool;    // varsa tüket
    fn expect(&mut self, k: SyntaxKind);         // yoksa tanı + kurtarma
    fn start(&mut self) -> Marker;               // düğüm başlat (checkpoint)
    fn error_recover(&mut self, msg, recovery: &TokenSet);
}
```

### 9.2 Gramer Yapısı (recursive descent)

Spesifikasyon §3 birebir fonksiyonlara eşlenir:
```
source_file → { top_decl }
top_decl    → at DOMAIN_KW  ? domain_decl
            | at MODULE_KW  ? module_decl
            | at (ENUM_KW|STRUCT_KW|BUNDLE_KW|NEWTYPE_KW) ? type_decl
            | at TEST_KW    ? test_decl
            | else          ? error_recover(TOP_RECOVERY)
```
İfade ayrıştırma **Pratt parser** (öncelik tırmanma) ile §2.4 öncelik tablosunu
uygular; tekli/ikili/postfix (`.field`, `[i]`, `(args)`, `as T`, `.truncate()`)
tek motorla işlenir.

### 9.3 Hata Kurtarma Stratejisi

Üç mekanizma birlikte:

1. **Kurtarma kümeleri (recovery sets):** Her bağlamda "senkronizasyon" token
   kümeleri tanımlıdır. Örn. modül gövdesinde beklenmedik token görülünce, parser
   `MODULE_ITEM_RECOVERY = {IN_KW, OUT_KW, REG_KW, WIRE_KW, ON_KW, FSM_KW,
   PIPELINE_KW, R_BRACE, ...}` görene kadar `ERROR` düğümüne token yutar.
2. **`expect` ile yumuşak kurtarma:** Eksik `:` veya `}` için tanı verilir ama
   ağaç **o token yokmuş gibi** tamamlanır (silme kurtarması). Böylece tek eksik
   noktalama tüm dosyayı bozmaz.
3. **Çapa token'ları (anchors):** `;`, `}`, ve üst-seviye anahtar kelimeler güçlü
   çapadır; parser bunları asla yutmaz, hep onlara kadar kurtarır.

**Fuel mekanizması:** her döngü adımında `fuel` düşer, `bump` ile resetlenir; sıfıra
inerse parser "ilerleme yok" hatası verir ve token yutarak ilerler — sonsuz döngü
imkânsızdır.

### 9.4 TreeBuilder

Olaylar + orijinal trivia birleştirilerek `cstree::GreenNodeBuilder` ile yeşil ağaç
kurulur. Trivia, en yakın token'a "yapışık" (attached) kurala göre yerleştirilir
(satır içi → önceki token'a, satır başı → sonraki düğüme).

### 9.5 Parser Test Stratejisi

- **Golden CST snapshot:** her `corpus/valid/*.volt` için ağaç yapısı `insta` ile
  kilitlenir.
- **Tanı snapshot:** her `corpus/invalid/*.volt` için üretilen tanılar kilitlenir.
- **Round-trip:** `syntax().to_string() == kaynak` (kayıpsızlık garantisi) — her
  dosyada otomatik doğrulanır.
- **Fuzz (`cargo-fuzz`):** rastgele baytlar + korpus mutasyonu; tek invaryant:
  *panik yok* ve round-trip korunur.

**Çıkış kriteri (Hafta 6 / M2):** Korpustaki geçerli dosyalar doğru CST; bozuk
dosyalar makul kısmi CST + konumlu tanı; round-trip %100; fuzz temiz.

---

## 10. `volt-ast` — Tipli AST Katmanı (F1, Hafta 6–7)

CST gevşek tiplidir (her şey `SyntaxNode`). AST katmanı, gramer üretimleri için
**ince tipli görünümler** sunar — yeni veri kopyalamaz, CST üzerinde gezinir.

```rust
impl AstNode for ModuleDecl {
    fn cast(n: SyntaxNode) -> Option<Self> {
        (n.kind() == MODULE_DECL).then(|| ModuleDecl(n))
    }
    fn syntax(&self) -> &SyntaxNode { &self.0 }
}
impl ModuleDecl {
    pub fn name(&self) -> Option<Name> { child(&self.0) }
    pub fn items(&self) -> impl Iterator<Item = ModuleItem> { children(&self.0) }
}
```

- Erişimciler **Option** döner (CST eksik/bozuk olabilir) — AST katmanı hata
  toleransını korur, asla `unwrap` etmez.
- Çoğu boilerplate **kod üretimiyle** (ungrammar benzeri bir şema + build script)
  oluşturulur; elle yazım yalnızca özel erişimciler için.

**Çıkış kriteri:** Tüm gramer düğümleri için AST tipi + erişimci; AST üzerinden
korpus gezinme testleri.

---

## 11. `volt-hir` — İsim Çözünümü ve `UnboundAst → BoundAst` (F1, Hafta 7–8)

### 11.1 Neden Ayrı Bir HIR

AST sözdizimine bağlıdır (boşluk, parantez). Tip kontrolü (F2) ve lowering (F3)
için **desugared, çözümlü, arena-tabanlı** bir temsil gerekir. Bu, spesifikasyonun
"ağacı yerinde değiştirmek yerine `UnboundAst` tüketip yeni `BoundAst` üreten
fonksiyonel dönüşüm" kararının somut hâlidir.

### 11.2 Arena ve Handle Modeli

```rust
pub struct Idx<T>(u32, PhantomData<T>);            // u32 handle — Copy, ucuz
pub struct Arena<T> { data: Vec<T> }               // la-arena
impl<T> Index<Idx<T>> for Arena<T> { ... }

pub struct Hir {
    pub modules:   Arena<ModuleData>,
    pub exprs:     Arena<ExprData>,
    pub types:     Arena<TypeData>,
    pub stmts:     Arena<StmtData>,
    pub sigs:      SignatureTable,                  // Σ — modül imzaları
    pub src_map:   FxHashMap<HirId, TextRange>,     // HIR → kaynak (tanılar için)
}
```
`Idx<T>` ile döngüsel referanslar ve mutasyon kolaylaşır; borrow checker'la
güreşmeden grafik kurulabilir (rust-analyzer'ın çözümü).

### 11.3 İsim Çözünümü

İki geçiş:
1. **Toplama (collect):** Tüm üst-seviye bildirimler (modül, domain, enum/struct/
   bundle/newtype) bir **isim alanına** (scope) toplanır; modül imzaları `Σ`'ye
   yazılır. İleri-referans desteklenir (bildirim sırası önemsiz).
2. **Çözme (resolve):** Her `NAME_REF`/`PATH` ilgili bildirime bağlanır;
   `import ... as` takma adları uygulanır. Çözülemeyen isim → `E0100: unresolved
   name`; en yakın isim önerisi (Levenshtein) ile "şunu mu demek istediniz?".

Kapsam (scope) kuralları: modül gövdesi bir kapsam; `on`/`rule`/`block`/`match arm`
iç içe kapsamlar; `let` bağları ve port/reg/wire isimleri kapsamda görünür. Aynı
kapsamda çift tanım → `E0101: duplicate definition` (her iki konumu da işaretler).

### 11.4 Lowering (Desugaring)

AST → HIR sırasında:
- Parantezler düşürülür; `BIN_EXPR` operatör + iki `Idx<ExprData>`'ya iner.
- `if/match` kontrol akışı normalize edilir (F2'nin definite-assignment analizi için
  düzgün dallanma grafiği).
- `pipeline`/`fsm` yapıları **kendi HIR düğümlerini** korur (F2/F3'te özel işlenecek);
  bu aşamada düzleştirilmez, yalnızca isimleri çözülür.
- Tipler `TypeData`'ya çözülür (genişlik `const_expr`'leri değerlendirilir;
  değerlendirilemeyen sabit → `E0102`).

> **Not:** HIR'de henüz **tip yoktur** — yalnızca yapı ve çözümlü isimler. Tipleme
> F2'nin işidir; HIR onun girdisidir. Bu sınır kasıtlıdır (faz ayrımı temiz kalır).

**Çıkış kriteri (M2 / Hafta 8):** Korpustaki geçerli dosyalar HIR + `Σ` üretir;
çözülemeyen/çift isimler doğru tanılarla yakalanır; HIR→kaynak konum eşlemesi
tanılarda kullanılabilir.

---

## 12. F0 — Yürüyen İskelet: Şablon Lowering, CLI, CI (Ay 1)

F0, F1'den **önce** gelir ve amacı boru hattının uçtan uca *aktığını* kanıtlamaktır
— tam parser ve tip sistemi henüz yokken.

### 12.1 Şablon Lowering (`volt-lower`, geçici)

- Sabit, dar bir alt küme (yalnızca `Counter`/`SatCounter` gibi elle seçilmiş
  modüller) için **string-şablon** tabanlı SystemVerilog üretimi.
- Bu kod F3'te **tamamen atılacaktır** (CIRCT ile değiştirilir); amacı yalnızca
  CI'da "kaynak → SV → Verilator simülasyonu" döngüsünü ay 1'de kurmaktır.
- Golden SV dosyaları `corpus/valid/*.sv.golden` olarak saklanır; diferansiyel test.

### 12.2 CLI (`volt-cli`)

F0–F1 boyunca büyüyen alt komutlar:
```
volt parse  <file>            # CST'yi yazdır (debug)
volt ast    <file>            # AST ağacını yazdır
volt hir    <file>            # HIR + Σ dökümü
volt check  <file>            # tanıları çalıştır (F2'de tip kontrolü eklenir)
volt build  <file> [-o out]   # F0: şablon SV; F3: CIRCT SV
volt fmt    <file>            # F5; F1'de yalnızca round-trip doğrulama stub'ı
--message-format=json         # tüm komutlarda JSON tanı çıktısı
```

### 12.3 CI

`.github/workflows/ci.yml`:
- `cargo build --workspace` + `cargo nextest run`
- `cargo clippy -- -D warnings`, `cargo fmt --check`
- `cargo deny check` (lisans/güvenlik — IP stratejisiyle uyumlu)
- **fuzz-smoke:** `cargo fuzz run parser -- -max_total_time=60` (kısa, regresyon için)
- **golden diff:** `volt build` çıktısını `.sv.golden` ile karşılaştır + Verilator
  derleme kontrolü (Verilator CI imajında kurulu).
- `insta` snapshot doğrulaması (CST/AST/HIR/tanı).

**Çıkış kriteri (M1 / Hafta 4):** `volt build counter.volt` → golden SV → Verilator
simülasyonu CI'da yeşil; tüm lint/test geçer; ADR-0001..3 yazılı.

---

## 13. Test Stratejisi (Çapraz-Kesit)

| Katman | Yöntem | Araç |
|---|---|---|
| span/diagnostics | birim + render snapshot | `insta` |
| lexer | golden token akışı + kenar durumları | `insta` |
| parser | golden CST + round-trip + fuzz | `insta`, `cargo-fuzz` |
| tanılar | golden tanı (invalid korpus) | `insta` |
| ast | gezinme/erişimci birim testi | `nextest` |
| hir/resolve | golden HIR + Σ + çözünüm hataları | `insta` |
| uçtan uca (F0) | kaynak→SV golden + Verilator | `nextest` + Verilator |

**Korpus disiplini:** Her yeni gramer/kural özelliği, en az bir `valid/` ve bir
`invalid/` dosyasıyla birlikte gelir. Korpus M1'den itibaren regresyon kalkanıdır.

---

## 14. Haftalık Plan

**F0 (Hafta 1–4):**
- H1: workspace, CI iskeleti, `volt-span`, ADR süreci.
- H2: `volt-diagnostics` (TtyEmitter + ilk kodlar), SourceMap testleri.
- H3: minimal lexer + sabit-modül şablon lowering; `volt build` ilk uçtan uca.
- H4: Verilator CI entegrasyonu, golden SV, M1 kapanışı.

**F1 (Hafta 5–12):**
- H5–6: tam lexer (sized/time literal, trivia, ERROR token), lexer testleri.
- H7: `SyntaxKind` tamamlanması, cstree `VoltLang`, TreeBuilder.
- H8–9: parser çekirdeği (Pratt ifade + top_decl + module body), recovery sets.
- H10: `fsm`/`pipeline`/`on`/`match` üretimleri, hata kurtarma sertleştirme, fuzz.
- H11: `volt-ast` view'ları + (kısmi) kod üretimi, AST testleri.
- H12: `volt-hir` arena + isim çözünümü + lowering, Σ, M2 kapanışı.

---

## 15. Açık Kararlar ve ADR Konuları

Bu fazda karara bağlanıp ADR'ye yazılacaklar:

1. **ADR-0001 — Ağaç altyapısı:** `cstree` vs `rowan`. Öneri: `cstree` (interning).
2. **ADR-0002 — Lexer:** `logos` + post-process vs tam elle. Öneri: hibrit.
3. **ADR-0003 — Hata modeli:** tek `Diagnostic` tipi + ayrı render; `codespan-reporting`
   vs `ariadne`. Öneri: `codespan-reporting` (olgun, JSON'a uygun).
4. **ADR-0004 — HIR mı, AST'de tipleme mi?** Öneri: ayrı arena-tabanlı HIR (faz
   ayrımı için).
5. **ADR-0005 — Sorgu mimarisi (incremental):** `salsa` şimdi mi, sonra mı? Öneri:
   MVP'de **hayır** (LSP F5'te incremental ihtiyacı doğunca değerlendir); şimdilik
   düz fonksiyonlar, ama crate sınırları salsa'ya geçişe uygun tasarlanır.

---

## 16. Tamamlanma Tanımı (Definition of Done)

F0–F1 şu koşullar sağlandığında biter:

1. `volt parse/ast/hir/check/build` komutları tüm korpusta çalışıyor (panik yok).
2. CST round-trip kayıpsız (`%100`); fuzz temiz (60 sn smoke + gece uzun koşum).
3. Geçerli korpus → doğru CST/AST/HIR/Σ (golden snapshot kilitli).
4. Bozuk korpus → makul kısmi ağaç + konumlu, kodlu, önerili tanılar.
5. İsim çözünümü: çözülemeyen/çift tanım/öneri (did-you-mean) çalışıyor.
6. F0 uçtan uca: en az 3 örnek modül → SV → Verilator simülasyonu CI'da yeşil.
7. Tanılar hem TTY hem `--message-format=json` üretiyor.
8. ADR-0001..0005 yazılı; crate sınırları F2 (tip sistemi) girişine hazır
   (`Hir` + `Σ` + `SourceMap` + tanı kanalı).

> **F2'ye devir:** F1 sonunda F2 ekibi, tipsiz ama yapısal olarak çözümlü bir `Hir`
> ve `Σ` alır. F2'nin tek işi bu HIR üzerinde §4 tip kurallarını uygulayıp tanı
> üretmektir — önyüzün tamamı arkasında hazırdır.

---

*Bu belge F0–F1 uygulaması için yeterli somutluğu hedefler; crate API imzaları
referans uygulama sırasında ince ayarlanacak, ADR'lerle birlikte güncellenecektir.
Mimari, rust-analyzer'ın kanıtlanmış desenlerini bilinçli olarak takip eder —
risk azaltmanın en ucuz yolu denenmiş yolları izlemektir.*
