# Volt Hata Kurtarma Stratejisi

> STATÜ: BAĞLAYICI
> İlgili: `grammar-full.ebnf`, `ast-nodes.md`, UX Anayasası

---

## 0. Neden Kritik

```
Kurtarma OLMADAN:
  Satır 5'te sözdizimi hatası → parser durur
  Satır 47'deki gerçek sorun görünmez
  Kullanıcı: düzelt → derle → yeni hata → düzelt → derle
  → 10 hata için 10 tur

Kurtarma İLE:
  Tüm hatalar tek seferde gösterilir
  Kullanıcı hepsini bir turda düzeltir
  → 10 hata için 1 tur

Ve LSP için ZORUNLU:
  Kullanıcı yazarken kod her an geçersiz
  "module Cou" → henüz tamamlanmamış
  Parser çökerse otomatik tamamlama çalışmaz
```

---

## 1. Temel İlkeler

```
İ1. ASLA PANİK ETME
    Parser her girdide bir AST üretir — belki hatalı ama üretir.
    panic!/unwrap() parser kodunda YASAK.

İ2. HATA DÜĞÜMÜ ÜRET
    Ayrıştırılamayan yapı → Error varyantı
    AST yapısı korunur, aşağı akış çökmez.

İ3. TEK HATA, TEK MESAJ
    Bir sözdizimi hatası bir tanı üretir.
    Kaskad hatalar bastırılır (bkz. §5).

İ4. İLERLEME GARANTİSİ
    Her kurtarma en az bir token tüketir.
    Sonsuz döngü imkânsız.

İ5. EN YAKIN SINIRA KURTAR
    Hata noktasından en yakın güvenli sınıra atla.
    Mümkün olduğunca az kod atlanır.
```

---

## 2. Senkronizasyon Kümeleri

Kurtarma noktaları hiyerarşiktir. Her seviye kendi kümesine sahip:

```rust
// crates/volt-syntax/src/recovery.rs

/// Öğe seviyesi — en dış sınır
const ITEM_START: TokenSet = token_set![
    MODULE_KW, DOMAIN_KW, FN_KW, STRUCT_KW, ENUM_KW,
    CONST_KW, TYPE_KW, EXTERN_KW, PUB_KW, USE_KW,
    PACKAGE_KW, AT, DOC_COMMENT
];

/// Port seviyesi
const PORT_START: TokenSet = token_set![
    IN_KW, OUT_KW, INOUT_KW, AT, DOC_COMMENT
];

/// Deyim seviyesi
const STMT_START: TokenSet = token_set![
    REG_KW, LET_KW, WIRE_KW, ON_KW, COMB_KW,
    FOR_KW, IF_KW, MATCH_KW, AT
];

/// Blok içi deyim
const BLOCK_STMT_START: TokenSet = token_set![
    IF_KW, MATCH_KW, LET_KW, FOR_KW, IDENT
];

/// Kontrat
const CONTRACT_START: TokenSet = token_set![
    REQUIRES_KW, ENSURES_KW, INVARIANT_KW,
    COVER_KW, ASSERT_KW, ASSUME_KW
];

/// Evrensel durak noktaları — her seviyede geçerli
const UNIVERSAL_STOP: TokenSet = token_set![
    R_BRACE, SEMICOLON, EOF
];
```

---

## 3. Kurtarma Algoritması

```rust
impl Parser {
    /// Ana kurtarma rutini
    fn recover(&mut self, sync: TokenSet, err: Diagnostic) -> RecoveryResult {
        self.push_error(err);

        // Zaten senkronizasyon noktasındaysak
        if self.at_any(sync) || self.at_any(UNIVERSAL_STOP) {
            return RecoveryResult::AtSyncPoint;
        }

        // İlerleme garantisi: en az bir token tüket
        let start_pos = self.pos;
        self.bump_any();

        // Senkronizasyon noktası bulana kadar atla
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.current() {
                // Parantez derinliği takibi
                L_BRACE | L_PAREN | L_BRACKET => depth += 1,
                R_BRACE | R_PAREN | R_BRACKET => {
                    if depth == 0 {
                        // Dış kapanış — burada dur
                        return RecoveryResult::Recovered {
                            skipped: self.pos - start_pos
                        };
                    }
                    depth -= 1;
                }
                t if depth == 0 && sync.contains(t) => {
                    return RecoveryResult::Recovered {
                        skipped: self.pos - start_pos
                    };
                }
                _ => {}
            }
            self.bump_any();
        }

        RecoveryResult::ReachedEof
    }
}

pub enum RecoveryResult {
    AtSyncPoint,
    Recovered { skipped: usize },
    ReachedEof,
}
```

**Kritik detay — parantez derinliği:** İç bloktaki `}` dış
sınır sanılmamalı. Derinlik sayacı bunu engelliyor.

---

## 4. Bağlama Özgü Kurtarma

### 4.1 Öğe Seviyesi

```volt
module Foo {
    in a : u8
    out b : u8
    b = a
}

@@@ bozuk içerik @@@         ← ayrıştırılamıyor

module Bar {                  ← BURADAN devam edilmeli
    in c : u8
}
```

```rust
fn parse_item(&mut self) -> Idx<Item> {
    let start = self.pos;
    match self.current() {
        MODULE_KW => self.parse_module(),
        DOMAIN_KW => self.parse_domain(),
        FN_KW     => self.parse_fn(),
        // ...
        _ => {
            let err = Diagnostic::new(E0001)
                .with_span(self.current_span())
                .with_message(format!(
                    "beklenmeyen '{}', öğe bekleniyor", self.current_text()
                ))
                .with_help("module, domain, fn veya struct bekleniyor");

            self.recover(ITEM_START, err);
            self.alloc_item(ItemKind::Error, start)
        }
    }
}
```

**Sonuç:** `Bar` modülü normal ayrıştırılır, sadece bozuk kısım
Error düğümü olur.

### 4.2 Port Listesi

```volt
module Foo {
    in  a : u8
    in  b :              ← tip eksik
    out c : u16          ← BU AYRIŞTIRILMALI
}
```

```rust
fn parse_port(&mut self) -> Option<Port> {
    let start = self.pos;
    let dir = self.parse_port_dir()?;
    let name = self.expect_ident()?;

    if !self.eat(COLON) {
        let err = Diagnostic::new(E0001)
            .with_span(self.current_span())
            .with_message("port tanımında ':' bekleniyor")
            .with_help(format!("in {} : u8 şeklinde yazın", name));

        self.recover(PORT_START, err);
        return Some(Port {
            span: self.span_from(start),
            direction: dir,
            name,
            ty: self.alloc_error_type(),   // ← Error tipi
            domain: None,
            attrs: vec![], doc: None,
        });
    }

    let ty = self.parse_type_or_error();
    // ...
}
```

**Kural:** Port kısmen ayrıştırılabiliyorsa AST'ye eklenir.
İsim biliniyorsa isim çözümleme çalışmaya devam eder.

### 4.3 İfade Seviyesi

```volt
let x = a + ;      ← sağ operand eksik
let y = b * c;     ← BU AYRIŞTIRILMALI
```

```rust
fn parse_expr_bp(&mut self, min_bp: u8) -> Idx<Expr> {
    let mut lhs = self.parse_prefix();

    loop {
        let op = match self.current_binop() {
            Some(op) => op,
            None => break,
        };

        let (l_bp, r_bp) = infix_binding_power(op);
        if l_bp < min_bp { break; }

        // E0010: karşılaştırma zinciri kontrolü
        if op.is_comparison() && self.last_was_comparison {
            self.push_error(Diagnostic::new(E0010)
                .with_span(self.current_span())
                .with_message("karşılaştırma operatörleri zincirlenemez")
                .with_help("(a < b) && (b < c) yazın"));
        }

        self.bump_any();

        // Sağ operand eksikse Error üret, ama DURMA
        let rhs = if self.at_expr_start() {
            self.parse_expr_bp(r_bp)
        } else {
            self.push_error(Diagnostic::new(E0001)
                .with_span(self.current_span())
                .with_message(format!("'{}' operatöründen sonra ifade bekleniyor", op)));
            self.alloc_error_expr()
        };

        lhs = self.alloc_binary(op, lhs, rhs);
    }
    lhs
}
```

### 4.4 Blok İçi Deyimler

```volt
on clk {
    a <= b + ;      ← hatalı
    c <= d;         ← BU AYRIŞTIRILMALI
    e <= f;         ← BU DA
}
```

Blok parser'ı her deyimden sonra kendini toparlar:

```rust
fn parse_block(&mut self, ctx: BlockContext) -> Idx<Block> {
    self.expect(L_BRACE);
    let mut stmts = Vec::new();

    while !self.at(R_BRACE) && !self.at_eof() {
        let before = self.pos;

        match self.parse_block_stmt(ctx) {
            Some(s) => stmts.push(s),
            None => {
                self.recover(BLOCK_STMT_START, /* err zaten eklendi */);
            }
        }

        // İlerleme garantisi — sonsuz döngü koruması
        if self.pos == before {
            self.bump_any();
        }
    }

    self.expect_closing(R_BRACE);
    self.alloc_block(stmts, ctx)
}
```

---

## 5. Kaskad Hata Bastırma

Bir hata sonrası aynı bölgede yeni hata üretilmemeli:

```rust
pub struct Parser {
    /// Son hata konumu
    last_error_pos: Option<usize>,
    /// Bastırma penceresi (token sayısı)
    suppress_window: usize,   // varsayılan: 2
}

impl Parser {
    fn push_error(&mut self, err: Diagnostic) {
        // Aynı pozisyonda ikinci hata → bastır
        if let Some(last) = self.last_error_pos {
            if self.pos.saturating_sub(last) < self.suppress_window {
                return;
            }
        }
        self.last_error_pos = Some(self.pos);
        self.errors.push(err);
    }
}
```

**Ayrıca:** Error düğümü içeren ifadeler tip kontrolüne
girmez — ikinci kez hata üretilmez.

```rust
// HIR aşamasında
fn check_expr(&mut self, expr: Idx<Expr>) -> TypeId {
    if matches!(self.ast.exprs[expr].kind, ExprKind::Error) {
        return TypeId::ERROR;   // sessizce yayılır
    }
    // ...
}
```

`TypeId::ERROR` her tiple uyumludur — kaskad tip hatası olmaz.

---

## 6. Yaygın Hatalar İçin Özel Kurtarma

Genel kurtarma yerine, sık yapılan hataları tanıyıp **daha iyi
mesaj** üretme:

### 6.1 Yanlış Atama Operatörü

```volt
on clk {
    r = r + 1        ← '<=' olmalı
}
```

```rust
// Kurtarma yerine düzeltme önerisi
if ctx == BlockContext::Sequential && self.at(EQ) {
    self.push_error(Diagnostic::new(E0006)
        .with_span(self.current_span())
        .with_message("sıralı blokta '=' kullanılamaz")
        .with_note("'on' bloğu içindeki atamalar saat kenarında olur")
        .with_help("'<=' kullanın")
        .with_suggestion(Suggestion::replace(self.current_span(), "<=")));

    self.bump_any();   // '=' tüket
    // AMA ayrıştırmaya DEVAM et — sanki '<=' yazılmış gibi
    let rhs = self.parse_expr();
    return Some(BlockStmt::NonBlockAssign { lhs, rhs, span });
}
```

**Sonuç:** Kullanıcı bir hata görür ama geri kalan analiz
(tip kontrolü, CDC) normal çalışır.

### 6.2 Verilog Alışkanlıkları

```volt
always @(posedge clk) begin    ← Verilog sözdizimi
```

```rust
if self.at_ident("always") {
    let span = self.current_span();
    // Verilog kalıbını tanı ve atla
    self.skip_verilog_always_block();

    self.push_error(Diagnostic::new(E0003)
        .with_span(span)
        .with_message("'always' Volt'ta geçersiz")
        .with_note("Volt'ta saat blokları 'on' ile yazılır")
        .with_help("on clk { ... } şeklinde yazın")
        .with_note("Verilog'dan geçiş: docs/verilog-to-volt.md"));
}
```

### 6.3 Eksik `else`

```volt
y = if cond { a }     ← else eksik → latch riski
```

```rust
if !self.eat(ELSE_KW) {
    self.push_error(Diagnostic::new(E0008)
        .with_span(self.span_from(if_start))
        .with_message("'if' ifadesinde 'else' dalı zorunlu")
        .with_note("eksik dal donanımda latch üretir")
        .with_help("else { varsayilan_deger } ekleyin"));

    // Error ifadesi ile devam et
    let else_expr = self.alloc_error_expr();
    return self.alloc_if(cond, then_expr, else_expr);
}
```

### 6.4 Eksik Kapanış

```volt
module Foo {
    in a : u8
    // '}' unutuldu
```

```rust
fn expect_closing(&mut self, tok: SyntaxKind) {
    if self.eat(tok) { return; }

    // Açılış konumunu hatırla — daha iyi mesaj için
    let open = self.open_brace_stack.last().copied();

    let mut err = Diagnostic::new(E0002)
        .with_span(self.current_span())
        .with_message(format!("eksik kapanış: '{}'", tok));

    if let Some(open_span) = open {
        err = err.with_secondary_span(open_span, "açılış burada");
    }

    err = err.with_help(format!("'{}' ekleyin", tok));
    self.push_error(err);
}
```

Hata mesajı örneği:

```
error[E0002]: eksik kapanış: '}'
  ┌─ design.volt:8:1
  │
3 │ module Foo {
  │            - açılış burada
  ⋮
8 │
  │ ^ dosya sonu, '}' bekleniyordu
  │
  = çözüm: '}' ekleyin
```

---

## 7. Girinti Tabanlı İpuçları

Kapanış eksikse, girinti bize nerede olması gerektiğini söyler:

```rust
/// Girintiden kapanış konumu tahmin et
fn guess_closing_position(&self, open_indent: usize) -> Option<usize> {
    for (i, tok) in self.tokens[self.pos..].iter().enumerate() {
        if tok.is_line_start() && tok.indent() <= open_indent {
            return Some(self.pos + i);
        }
    }
    None
}
```

```volt
module Foo {
    in a : u8
    out b : u8
        b = a          ← girinti fazla, ama sorun değil

module Bar {           ← girinti 0 → Foo burada bitmeliydi
```

Bu ipucu ile hata mesajı:
```
  = not: 'module Bar' satırındaki girinti, Foo'nun burada
         bitmesi gerektiğini gösteriyor
```

---

## 8. Test Stratejisi

### 8.1 Kurtarma Testleri

```
tests/ui/recover/
├── 01_missing_type.volt
├── 02_missing_brace.volt
├── 03_bad_operator.volt
├── 04_verilog_always.volt
└── 05_cascading.volt
```

Format — beklenen hata sayısı ve devam eden ayrıştırma:

```volt
//~ RECOVER errors=1 items=2
// Bozuk modülden sonra ikinci modül ayrıştırılmalı

module Broken {
    in a :
    //~^ ERROR port tanımında tip bekleniyor
}

module Valid {
    in b : u8
    out c : u8
    c = b
}
```

Doğrulama:
```rust
#[test]
fn recovery_continues_after_error() {
    let ast = parse(src);
    assert_eq!(ast.errors.len(), 1);      // TEK hata
    assert_eq!(ast.items.len(), 2);       // İKİ modül
    assert!(ast.expect_module(1).name == "Valid");
}
```

### 8.2 Fuzzing (Zorunlu)

```rust
// fuzz/fuzz_targets/parse_never_panics.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // ASLA panik etmemeli
        let _ = volt_syntax::parse(s);
    }
});
```

```bash
cargo fuzz run parse_never_panics -- -max_total_time=300
```

**CI kuralı:** Her PR'da 5 dakika fuzzing çalışır.

### 8.3 İlerleme Garantisi Testi

```rust
#[test]
fn parser_always_terminates() {
    // Patolojik girdiler
    let cases = [
        "", "{", "}", "((((", "module", "module {",
        "@@@@", "\0\0\0", "module module module",
        &"{".repeat(10_000),
    ];

    for src in cases {
        let start = std::time::Instant::now();
        let _ = parse(src);
        assert!(start.elapsed().as_secs() < 1, "takıldı: {src:?}");
    }
}
```

---

## 9. LSP Entegrasyonu

Hata kurtarma LSP'nin temelidir:

```rust
// Kullanıcı yazarken:
//   "module Cou"          → eksik ama ayrıştırılıyor
//   "module Counter {"    → eksik ama ayrıştırılıyor
//   "module Counter { in" → eksik ama ayrıştırılıyor

// Her durumda AST var → otomatik tamamlama çalışıyor
fn completions(&self, pos: Position) -> Vec<CompletionItem> {
    let ast = self.parse_cached();   // hatalı olabilir, önemli değil
    let node = ast.node_at(pos);

    match node {
        // Port bağlamında → tip önerileri
        Some(Node::Port { ty: None, .. }) => TYPE_COMPLETIONS.to_vec(),
        // Blok içinde → deyim önerileri
        Some(Node::Block { .. }) => STMT_COMPLETIONS.to_vec(),
        _ => vec![],
    }
}
```

---

## 10. Kalite Ölçütleri

```
ÖLÇÜT                          HEDEF        NASIL ÖLÇÜLÜR
──────────────────────────────────────────────────────────────
Panik oranı                    0            fuzzing (5 dk)
Kaskad hata oranı              < %10        recover testleri
Kurtarma sonrası devam         > %90        items ayrıştırıldı
Hata başına atlanan token      < 10         ortalama
Ayrıştırma süresi (10K satır)  < 100 ms     benchmark

Ölçüm:
  just recovery-report
  → tests/ui/recover/ üzerinde istatistik
```

---

## 11. Uygulama Sırası

```
F0:  Sadece ITEM_START kurtarması
     "bozuk öğeyi atla, sonrakine geç"
     → 50 satır kod

F1:  Port, deyim, ifade kurtarması
     Kaskad bastırma
     → 200 satır kod

F1:  Fuzzing kurulumu (CI'da)

F2:  Özel hata kalıpları (E0006, E0008, Verilog)
     Girinti ipuçları
     → 150 satır kod

F5:  LSP entegrasyonu, artımlı yeniden ayrıştırma
```

---

## 12. Karşı Örnekler (Yapılmayacaklar)

```rust
// ✗ YANLIŞ: panik
fn parse_type(&mut self) -> Type {
    self.expect(COLON).unwrap();   // ← ASLA
}

// ✗ YANLIŞ: hatada durma
fn parse_module(&mut self) -> Result<Module, Error> {
    let port = self.parse_port()?;   // ← ilk hatada çıkıyor
}

// ✗ YANLIŞ: sessiz atlama
if !self.eat(COLON) {
    return None;   // ← hata mesajı yok, kullanıcı bilmiyor
}

// ✗ YANLIŞ: ilerleme garantisi yok
while !self.at(R_BRACE) {
    self.parse_stmt();   // ← hiç token tüketmezse sonsuz döngü
}

// ✓ DOĞRU
fn parse_port(&mut self) -> Option<Port> {
    let dir = self.parse_dir()?;
    let name = self.expect_ident_or_recover(PORT_START)?;
    if !self.eat(COLON) {
        self.push_error(/* açıklayıcı hata */);
        self.recover(PORT_START, ...);
        return Some(Port::with_error_type(dir, name));
    }
    // ...
}
```
