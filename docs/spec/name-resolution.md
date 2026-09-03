# Volt İsim Çözümleme Spesifikasyonu

> STATÜ: BAĞLAYICI
> İlgili: `ast-nodes.md`, `type-inference.md`, `domain-inference.md`
> Aşama: F1 sonu — F2 için ÖNKOŞUL

---

## 0. Konum ve Sorumluluk

```
Parser  → AST üretir (isimler sadece metin)
   ↓
İSİM ÇÖZÜMLEME  ← bu belge
   ↓
Tip çıkarımı → DefId'lerin tiplerini hesaplar
   ↓
Domain çıkarımı → DefId'lerin domain'lerini hesaplar
```

**Sorumluluk sınırı:** Bu aşama "bu isim neyi gösteriyor?"
sorusunu yanıtlar. "Tipi ne?" sorusuna DEĞİL.

---

## 1. Tanım Kimliği (DefId)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DefId(u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefKind {
    // ── Öğe seviyesi ──
    Module,
    Domain,
    Function,
    Struct,
    Enum,
    EnumVariant { parent: DefId },
    Const,
    TypeAlias,
    ExternModule,

    // ── Modül içi ──
    Port { dir: PortDir },
    Register,
    Wire,          // let veya wire bildirimi
    Instance,      // let u = Uart { ... }

    // ── Yerel ──
    LocalBinding,  // blok içi let
    LoopVar,       // for i in 0..N
    PatternBinding,// match Some(x) => ...
    GenericParam,

    // ── Yerleşik ──
    Builtin(BuiltinKind),

    /// Hata kurtarma — her kullanımla uyumlu
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinKind {
    Sync,      // sync(data, clk)
    Sync3,     // sync3(data, clk)
    Zext,      // zext(x)
    Sext,      // sext(x)
    Trunc,     // trunc(x)
    Concat,    // concat(a, b)
    Replicate, // replicate(x, n)
    PopCount,  // popcount(x)
    Clog2,     // clog2(n) — derleme zamanı
}

pub struct DefData {
    pub kind: DefKind,
    pub name: Name,
    /// Bildirim konumu — hata mesajlarında "burada tanımlı"
    pub span: Span,
    /// Hangi kapsamda tanımlı
    pub scope: ScopeId,
    /// AST düğümüne geri referans
    pub ast_node: AstNodeId,
}
```

---

## 2. Kapsam Hiyerarşisi

```
Root (paket seviyesi)
 ├── Prelude (yerleşikler — daima görünür)
 ├── Imports (use ile getirilenler)
 └── Item scope (modüller, fonksiyonlar, sabitler)
      │
      └── Module scope
           ├── Generic parametreler
           ├── Portlar
           ├── Register/wire/instance bildirimleri
           └── Block scope (on / comb / fn gövdesi)
                ├── Yerel let bağlamaları
                ├── For döngü değişkeni
                └── Nested block scope (if/else/match içi)
```

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(u32);

pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    /// İsim → tanım eşlemesi
    pub bindings: FxHashMap<Name, DefId>,
    /// Sıralı bildirimler (gölgeleme ve "sonra tanımlı" için)
    pub order: Vec<(Name, DefId, Span)>,
}

pub enum ScopeKind {
    Root,
    Prelude,
    Module(DefId),
    Function(DefId),
    Block,
    Loop,
    MatchArm,
}
```

---

## 3. Çözümleme Algoritması

### 3.1 Ana Rutin

```rust
fn resolve(&mut self, path: &Path, scope: ScopeId) -> DefId {
    match path.segments.len() {
        // Tek segment: local → module → item → prelude
        1 => self.resolve_simple(path.segments[0], scope, path.span),
        // Çok segment: Foo::Bar veya soc::uart::Uart
        _ => self.resolve_qualified(path, scope),
    }
}

fn resolve_simple(&mut self, name: Name, scope: ScopeId,
                  span: Span) -> DefId {
    // Kapsam zincirini yukarı doğru tara
    let mut current = Some(scope);
    while let Some(s) = current {
        if let Some(def) = self.scopes[s].bindings.get(&name) {
            self.record_use(*def, span);
            return *def;
        }
        current = self.scopes[s].parent;
    }

    // Bulunamadı — yardımcı hata üret
    self.error_unresolved(name, span, scope);
    self.error_def()
}
```

### 3.2 Nitelikli Yol

```rust
fn resolve_qualified(&mut self, path: &Path, scope: ScopeId) -> DefId {
    let first = self.resolve_simple(path.segments[0], scope, path.span);
    let mut current = first;

    for (i, seg) in path.segments[1..].iter().enumerate() {
        current = match self.def_kind(current) {
            // State::Idle
            DefKind::Enum => self.lookup_variant(current, *seg, path.span),
            // uart.busy  (nokta ile de yazılabilir)
            DefKind::Instance => self.lookup_port(current, *seg, path.span),
            // pkt.header
            DefKind::Struct => self.lookup_field(current, *seg, path.span),
            DefKind::Error => return self.error_def(),
            other => {
                self.error(E1005, path.span,
                    format!("'{}' bir ad alanı değil", self.name_of(current)),
                    format!("{:?} içinde '::' kullanılamaz", other));
                return self.error_def();
            }
        };
    }
    current
}
```

---

## 4. Kritik Kural: İleri Referans

Bu Volt'un en önemli çözümleme kararı.

```
ÖĞE SEVİYESİ (module, fn, struct, const, domain):
  İLERİ REFERANS SERBEST
  → İki geçişli toplama gerekiyor

MODÜL İÇİ (port, reg, wire, let):
  BİLDİRİM ÖNCE OLMALI
  → Tek geçişli, sıralı çözümleme
```

### Neden Bu Ayrım

```volt
// ✓ GEÇERLİ — modüller birbirini önden görebilir
module Top {
    let u = Uart { clk: clk }   // Uart aşağıda tanımlı
}

module Uart { ... }

// ✗ GEÇERSİZ — sinyal kullanılmadan önce tanımlanmalı
module Bad {
    out y : u8
    y = temp            // E1002: 'temp' henüz tanımlı değil
    let temp = 42
}
```

**Gerekçeler:**

```
Öğe seviyesinde ileri referans:
  Dosya sırası mimari kararı olmamalı
  Karşılıklı bağımlılık doğal (A→B, B→A modülleri)
  Rust, C++, SystemVerilog hepsi böyle

Modül içi sıralı:
  Donanımda "önce tanımla" veri akışını netleştiriyor
  Döngüsel kombinasyonel bağımlılık zaten yasak
  Okunabilirlik: yukarıdan aşağı akış
```

### Uygulama: İki Geçiş

```rust
pub fn resolve_file(&mut self, ast: &SourceFile) -> ResolveResult {
    // ── GEÇİŞ 1: Öğe toplama ──
    // Tüm modül/fn/struct/const/domain isimlerini kaydet
    let root = self.new_scope(ScopeKind::Root, None);
    for item in &ast.items {
        self.collect_item(*item, root);
    }

    // ── GEÇİŞ 2: Gövde çözümleme ──
    // Her öğenin içini sıralı olarak çöz
    for item in &ast.items {
        self.resolve_item_body(*item, root);
    }

    self.finish()
}
```

---

## 5. Modül İçi Sıralı Çözümleme

```rust
fn resolve_module_body(&mut self, m: &ModuleDecl, parent: ScopeId) {
    let scope = self.new_scope(ScopeKind::Module(m.def_id), Some(parent));

    // 1. Generic parametreler (en önce)
    for g in &m.generics {
        self.declare(g.name, DefKind::GenericParam, scope, g.span);
    }

    // 2. Tüm portlar — birbirini görebilir
    //    (port sırası bağlantıyı etkilemez)
    for p in &m.ports {
        self.declare_checked(p.name, DefKind::Port { dir: p.direction },
                             scope, p.span);
    }

    // 3. Gövde — SIRALI
    for stmt in &m.body {
        self.resolve_stmt(*stmt, scope);
        // resolve_stmt içinde declare() çağrılıyor
        // → sonraki deyimler bu ismi görebiliyor
    }
}
```

**Portlar neden sıralı değil:** Port listesi bir arayüz
tanımıdır, sıralama anlamsal bilgi taşımaz.

---

## 6. Gölgeleme (Shadowing)

```
KURAL: Aynı kapsamda YASAK, iç kapsamda İZİNLİ + uyarı
```

```volt
module Example {
    in  data : u8
    let data = 5        // ✗ E1003: aynı kapsamda çift tanım

    on clk {
        let temp = 1
        if cond {
            let temp = 2   // ⚠ W1002: dış 'temp' gölgeleniyor
        }
    }
}
```

```rust
fn declare_checked(&mut self, name: Name, kind: DefKind,
                   scope: ScopeId, span: Span) -> DefId {
    // Aynı kapsamda var mı?
    if let Some(prev) = self.scopes[scope].bindings.get(&name) {
        let prev_span = self.defs[*prev].span;
        self.error(E1003, span,
            format!("'{}' bu kapsamda zaten tanımlı", name),
            "farklı bir isim kullanın")
            .with_secondary(prev_span, "önceki tanım burada");
        return self.error_def();
    }

    // Dış kapsamda var mı? → gölgeleme uyarısı
    if let Some(outer) = self.lookup_in_parents(name, scope) {
        // Yerleşikleri gölgelemek daha ciddi
        let severity = if self.is_builtin(outer) {
            Severity::Warning  // W1003
        } else {
            Severity::Warning  // W1002
        };
        self.warn_shadowing(name, span, self.defs[outer].span, severity);
    }

    self.declare(name, kind, scope, span)
}
```

**Neden aynı kapsamda yasak:** Donanımda iki farklı sinyal
aynı isme sahip olamaz — SV çıktısında çakışır.

---

## 7. Yerleşikler (Prelude)

```rust
fn init_prelude(&mut self) -> ScopeId {
    let s = self.new_scope(ScopeKind::Prelude, None);

    // CDC köprüleri
    self.declare_builtin("sync",      BuiltinKind::Sync,      s);
    self.declare_builtin("sync3",     BuiltinKind::Sync3,     s);

    // Genişletme/daraltma
    self.declare_builtin("zext",      BuiltinKind::Zext,      s);
    self.declare_builtin("sext",      BuiltinKind::Sext,      s);
    self.declare_builtin("trunc",     BuiltinKind::Trunc,     s);

    // Bit işlemleri
    self.declare_builtin("concat",    BuiltinKind::Concat,    s);
    self.declare_builtin("replicate", BuiltinKind::Replicate, s);
    self.declare_builtin("popcount",  BuiltinKind::PopCount,  s);

    // Derleme zamanı
    self.declare_builtin("clog2",     BuiltinKind::Clog2,     s);

    s
}
```

**Trit prelude'de DEĞİL** — UX Anayasası gereği `import
volt::ternary::Trit` ile açık getirilir.

---

## 8. Yardımcı Hata Mesajları

### E1001 — Tanımsız İsim + Benzer Öneri

```rust
fn error_unresolved(&mut self, name: Name, span: Span, scope: ScopeId) {
    let candidates = self.visible_names(scope);
    let suggestion = self.closest_match(name, &candidates);

    let mut err = Diagnostic::new(E1001)
        .with_span(span)
        .with_message(format!("tanımsız isim: '{}'", name));

    match suggestion {
        Some(s) => {
            err = err.with_help(format!("'{}' mi demek istediniz?", s))
                     .with_suggestion(Suggestion::replace(span, s));
        }
        None => {
            err = err.with_help("bu isim hiçbir kapsamda tanımlı değil");
        }
    }
    self.push(err);
}

/// Levenshtein mesafesi — eşik: isim uzunluğunun üçte biri
fn closest_match(&self, name: Name, cands: &[Name]) -> Option<Name> {
    let target = name.as_str();
    let threshold = (target.len() / 3).max(1);
    cands.iter()
        .map(|c| (c, levenshtein(target, c.as_str())))
        .filter(|(_, d)| *d <= threshold)
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| *c)
}
```

Örnek çıktı:

```
error[E1001]: tanımsız isim: 'enabel'
  ┌─ design.volt:9:12
  │
9 │         if enabel {
  │            ^^^^^^
  │
  = çözüm: 'enable' mi demek istediniz?
  = daha fazla: volt explain E1001
```

### E1002 — Bildirimden Önce Kullanım

```
error[E1002]: 'temp' bu noktada henüz tanımlı değil
  ┌─ design.volt:6:9
  │
6 │     y = temp
  │         ^^^^ burada kullanılıyor
7 │     let temp = 42
  │         ---- ama burada tanımlanıyor
  │
  = neden: modül içi bildirimler kullanımdan önce gelmeli
  = çözüm: 'let temp = 42' satırını yukarı taşıyın
  = daha fazla: volt explain E1002
```

---

## 9. Kullanım Takibi

Kullanılmayan tanımları raporlamak için:

```rust
pub struct UseTracker {
    uses: FxHashMap<DefId, Vec<Span>>,
}

fn report_unused(&mut self) {
    for (def_id, data) in self.defs.iter() {
        // '_' önekli isimler muaf (UX Anayasası)
        if data.name.as_str().starts_with('_') { continue; }
        // Public öğeler muaf (dışarıdan kullanılabilir)
        if self.is_public(def_id) { continue; }

        if !self.uses.contains_key(&def_id) {
            let (code, msg) = match data.kind {
                DefKind::Port { dir: PortDir::In } =>
                    (W1001, "kullanılmayan giriş portu"),
                DefKind::Register =>
                    (W1004, "kullanılmayan register"),
                DefKind::Wire | DefKind::LocalBinding =>
                    (W1001, "kullanılmayan bağlama"),
                DefKind::Domain =>
                    (W3004, "kullanılmayan domain tanımı"),
                _ => continue,
            };
            self.warn(code, data.span, msg,
                format!("'_' öneki ekleyerek susturabilirsiniz: _{}",
                        data.name));
        }
    }
}
```

---

## 10. Hata ve Uyarı Kodları

```
E1001  Tanımsız isim
E1002  Bildirimden önce kullanım
E1003  Aynı kapsamda çift tanım
E1004  Özel (pub olmayan) öğeye erişim
E1005  Ad alanı olmayan öğede '::' kullanımı
E1006  Döngüsel modül bağımlılığı
E1007  Enum varyantı bulunamadı
E1008  Struct alanı bulunamadı
E1009  Modül portu bulunamadı
E1010  Ambiguous import (iki 'use' aynı ismi getiriyor)

W1001  Kullanılmayan sinyal / bağlama
W1002  Gölgeleme (iç kapsamda aynı isim)
W1003  Yerleşik ismin gölgelenmesi
W1004  Yazılıp okunmayan register
W1005  Kullanılmayan import
```

---

## 11. Uygulama Sırası

```
F1 sonu (1 hafta):
  ScopeId, DefId, Scope yapıları
  İki geçişli toplama
  resolve_simple (tek segment)
  E1001, E1003

F2 başı (1 hafta):
  Nitelikli yol (Enum::Variant, instance.port)
  Prelude yerleşikleri
  E1002 sıralı kontrol
  Benzer isim önerisi

F2 sonu (3 gün):
  Kullanım takibi
  W1001-W1005 uyarıları
```

---

## 12. Test Vektörleri

```
DURUM                                   BEKLENEN
──────────────────────────────────────────────────────────
Port referansı                          ✓ çözülür
Aşağıda tanımlı modüle referans         ✓ çözülür (ileri ref)
İç blokta gölgeleme                     ✓ + W1002
'_' önekli kullanılmayan sinyal         ✓ uyarı yok
Enum varyantı: State::Idle              ✓ çözülür
Instance port: uart.busy                ✓ çözülür
Yerleşik: sync(a, clk)                  ✓ çözülür
──────────────────────────────────────────────────────────
Yazım hatası: 'enabel'                  ✗ E1001 + öneri
Bildirimden önce kullanım               ✗ E1002
Aynı kapsamda çift tanım                ✗ E1003
Tanımsız enum varyantı                  ✗ E1007
Tanımsız port: uart.nonexistent         ✗ E1009
Kullanılmayan register                  ⚠ W1004
```
