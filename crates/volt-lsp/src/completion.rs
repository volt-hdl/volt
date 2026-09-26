//! Bağlama duyarlı otomatik tamamlama (error-recovery.md §9):
//! parser hata kurtarma yaptığı için yarım kodda da AST vardır —
//! tamamlama analiz hatalarından bağımsız çalışır.

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation};
use volt_ast::ItemKind;
use volt_hir::{DefId, DefKind, Ty};

use crate::analysis::Analysis;
use crate::docs;

/// İmleç konumundan türetilen tamamlama bağlamı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// `:` sonrası — tip konumu.
    Type,
    /// `@` sonrası — domain anotasyonu.
    Domain,
    /// `on ` sonrası — clock portu.
    OnClock,
    /// `.` sonrası — taban ismin üyeleri.
    Member(String),
    /// `::` sonrası — yolun ön eki (enum varyantları, ADR-0074).
    Path(String),
    /// Satır başı — deyim anahtar kelimeleri.
    StmtStart,
    /// Diğer her yer — ifade bağlamı.
    Expr,
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Bayt offsetinden bağlam çıkarır (yalnız metne bakar; AST gerekmez).
pub fn context_at(text: &str, offset: usize) -> Context {
    let bytes = text.as_bytes();
    let mut p = offset.min(bytes.len());
    // Yazılmakta olan öneki atla.
    while p > 0 && is_ident_byte(bytes[p - 1]) {
        p -= 1;
    }
    if p > 0 && bytes[p - 1] == b'@' {
        return Context::Domain;
    }
    if p >= 2 && &bytes[p - 2..p] == b"::" {
        let mut b = p - 2;
        while b > 0 && is_ident_byte(bytes[b - 1]) {
            b -= 1;
        }
        if b < p - 2 {
            return Context::Path(text[b..p - 2].to_string());
        }
        return Context::Expr;
    }
    if p > 0 && bytes[p - 1] == b'.' && !(p >= 2 && bytes[p - 2] == b'.') {
        // '.' öncesindeki taban ismi oku; iç içe alan zinciri (`p.i.`,
        // ADR-0077) noktalı taban olarak döner.
        let mut b = p - 1;
        while b > 0 && is_ident_byte(bytes[b - 1]) {
            b -= 1;
        }
        if b == p - 1 {
            return Context::Expr;
        }
        while b >= 2 && bytes[b - 1] == b'.' && is_ident_byte(bytes[b - 2]) {
            let mut c = b - 1;
            while c > 0 && is_ident_byte(bytes[c - 1]) {
                c -= 1;
            }
            if c > 0 && bytes[c - 1] == b'.' && c >= 2 && bytes[c - 2] == b'.' {
                break; // `0..n` aralığı
            }
            b = c;
        }
        return Context::Member(text[b..p - 1].to_string());
    }
    // Boşlukları atlayıp ':' ve 'on ' desenlerine bak.
    let mut q = p;
    while q > 0 && (bytes[q - 1] == b' ' || bytes[q - 1] == b'\t') {
        q -= 1;
    }
    if q > 0 && bytes[q - 1] == b':' && !(q >= 2 && bytes[q - 2] == b':') {
        return Context::Type;
    }
    if q >= 2 && &bytes[q - 2..q] == b"on" && (q == 2 || !is_ident_byte(bytes[q - 3])) && p > q {
        return Context::OnClock;
    }
    // Satır başı (yalnız girinti) veya blok açılışı → deyim başlangıcı.
    if q == 0 || bytes[q - 1] == b'\n' || bytes[q - 1] == b'{' {
        return Context::StmtStart;
    }
    Context::Expr
}

const TYPES: &[&str] = &[
    "bool", "clock", "reset", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "Trit",
];

const STMT_KEYWORDS: &[&str] = &["reg", "let", "wire", "on", "comb", "if", "match", "for"];
const TOP_KEYWORDS: &[&str] = &[
    "module", "domain", "const", "struct", "enum", "fn", "extern", "use", "pub",
];

fn item(label: impl Into<String>, kind: CompletionItemKind) -> CompletionItem {
    CompletionItem {
        label: label.into(),
        kind: Some(kind),
        ..CompletionItem::default()
    }
}

fn stdlib_items() -> Vec<CompletionItem> {
    docs::STDLIB
        .iter()
        .map(|d| CompletionItem {
            label: d.name.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(d.signature.to_string()),
            documentation: Some(Documentation::String(d.doc.to_string())),
            ..CompletionItem::default()
        })
        .collect()
}

/// Offsetteki bağlama göre tamamlama listesi.
pub fn completions(analysis: &Analysis, offset: u32) -> Vec<CompletionItem> {
    let ctx = context_at(analysis.source(), offset as usize);
    match ctx {
        Context::Type => {
            let mut items: Vec<CompletionItem> = TYPES
                .iter()
                .map(|t| item(*t, CompletionItemKind::KEYWORD))
                .collect();
            items.push(CompletionItem {
                label: "bits<N>".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                insert_text: Some("bits<".to_string()),
                ..CompletionItem::default()
            });
            // Kullanıcı tipleri: struct / enum / type alias.
            for &idx in &analysis.ast.items {
                match &analysis.ast.items_arena[idx].kind {
                    ItemKind::Struct(s) => {
                        items.push(item(&s.name.text, CompletionItemKind::STRUCT))
                    }
                    ItemKind::Enum(e) => items.push(item(&e.name.text, CompletionItemKind::ENUM)),
                    ItemKind::TypeAlias(t) => {
                        items.push(item(&t.name.text, CompletionItemKind::REFERENCE))
                    }
                    _ => {}
                }
            }
            items
        }
        Context::Domain => analysis
            .domain_names()
            .into_iter()
            .map(|n| item(n, CompletionItemKind::MODULE))
            .collect(),
        Context::OnClock => match analysis.module_at(offset) {
            Some(m) => analysis
                .clock_ports(m)
                .into_iter()
                .map(|n| CompletionItem {
                    label: n.to_string(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some("clock".to_string()),
                    ..CompletionItem::default()
                })
                .collect(),
            None => Vec::new(),
        },
        Context::Member(base) => member_completions(analysis, &base),
        Context::Path(base) => path_completions(analysis, &base),
        Context::StmtStart => {
            let keywords = if analysis.module_at(offset).is_some() {
                STMT_KEYWORDS
            } else {
                TOP_KEYWORDS
            };
            keywords
                .iter()
                .map(|k| item(*k, CompletionItemKind::KEYWORD))
                .collect()
        }
        Context::Expr => expr_completions(analysis),
    }
}

/// `Enum::` sonrası: varyantlar, bildirim sırasıyla (ADR-0074).
fn path_completions(analysis: &Analysis, base: &str) -> Vec<CompletionItem> {
    analysis
        .ast
        .items
        .iter()
        .find_map(|&idx| match &analysis.ast.items_arena[idx].kind {
            ItemKind::Enum(e) if e.name.text == base => Some(e),
            _ => None,
        })
        .map(|e| {
            e.variants
                .iter()
                .map(|v| CompletionItem {
                    label: v.name.text.clone(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    detail: Some(format!("{base}::{}", v.name.text)),
                    ..CompletionItem::default()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `.` sonrası: modül örneği portları veya struct alanları; noktalı
/// taban (`p.i`, `s0.q`) iç içe struct alanlarına iner (ADR-0077).
fn member_completions(analysis: &Analysis, base: &str) -> Vec<CompletionItem> {
    let Some(res) = &analysis.resolve else {
        return Vec::new();
    };
    if let Some((root, rest)) = base.split_once('.') {
        return nested_field_completions(analysis, root, rest);
    }
    let Some(def) = res
        .defs
        .iter()
        .position(|d| d.name == base && !matches!(d.kind, DefKind::Error))
        .map(|i| DefId(i as u32))
    else {
        return Vec::new();
    };

    // Yerleşik primitif örneği → statik port tablosu.
    if let Some(prim) = res.instance_builtin.get(&def) {
        return prim
            .ports()
            .iter()
            .map(|p| CompletionItem {
                label: p.name.to_string(),
                kind: Some(CompletionItemKind::FIELD),
                detail: Some(format!("{:?} port", p.dir).to_lowercase()),
                ..CompletionItem::default()
            })
            .collect();
    }

    // Kullanıcı modülü örneği → hedef modülün portları.
    if let Some(target) = res.instance_module.get(&def) {
        if let Some(item_idx) = res.item_of_def.get(target) {
            if let ItemKind::Module(m) = &analysis.ast.items_arena[*item_idx].kind {
                return m
                    .ports
                    .iter()
                    .map(|p| CompletionItem {
                        label: p.name.text.clone(),
                        kind: Some(CompletionItemKind::FIELD),
                        detail: Some(format!("{:?}", p.direction).to_lowercase()),
                        ..CompletionItem::default()
                    })
                    .collect();
            }
        }
    }

    // Struct tipli sinyal → alanları.
    if let Some(typeck) = &analysis.typeck {
        if let Some(ty_id) = typeck.def_types.get(&def) {
            if let Ty::Struct(sid) = typeck.types.ty(*ty_id) {
                let struct_def = DefId(sid.0);
                if let Some(item_idx) = res.item_of_def.get(&struct_def) {
                    if let ItemKind::Struct(s) = &analysis.ast.items_arena[*item_idx].kind {
                        return s
                            .fields
                            .iter()
                            .map(|f| item(&f.name.text, CompletionItemKind::FIELD))
                            .collect();
                    }
                }
            }
        }
    }
    Vec::new()
}

/// `p.i.` / `s0.q.`: kökün struct tipinden alan zinciri izlenir.
fn nested_field_completions(analysis: &Analysis, root: &str, rest: &str) -> Vec<CompletionItem> {
    let ast = &analysis.ast;
    let Some(res) = &analysis.resolve else {
        return Vec::new();
    };
    let Some(def) = res
        .defs
        .iter()
        .position(|d| d.name == root && !matches!(d.kind, DefKind::Error))
        .map(|i| DefId(i as u32))
    else {
        return Vec::new();
    };
    let mut parts = rest.split('.');
    // Kökün struct bildirimi: struct tipli sinyal ya da örneğin struct portu.
    let mut decl: Option<&volt_ast::StructDecl> = None;
    if let Some(target) = res.instance_module.get(&def) {
        let port = parts.next().unwrap_or("");
        if let Some(item_idx) = res.item_of_def.get(target) {
            if let ItemKind::Module(m) = &ast.items_arena[*item_idx].kind {
                decl = m
                    .ports
                    .iter()
                    .find(|p| p.name.text == port)
                    .and_then(|p| volt_ast::struct_layout::struct_of_type(ast, p.ty));
            }
        }
    } else if let Some(typeck) = &analysis.typeck {
        if let Some(Ty::Struct(sid)) = typeck.def_types.get(&def).map(|t| typeck.types.ty(*t)) {
            if let Some(item_idx) = res.item_of_def.get(&DefId(sid.0)) {
                if let ItemKind::Struct(s) = &ast.items_arena[*item_idx].kind {
                    decl = Some(s);
                }
            }
        }
    }
    for field in parts {
        decl = decl
            .and_then(|d| d.fields.iter().find(|f| f.name.text == field))
            .and_then(|f| volt_ast::struct_layout::struct_of_type(ast, f.ty));
    }
    decl.map(|d| {
        d.fields
            .iter()
            .map(|f| item(&f.name.text, CompletionItemKind::FIELD))
            .collect()
    })
    .unwrap_or_default()
}

/// İfade bağlamı: kapsamdaki isimler + stdlib modülleri.
fn expr_completions(analysis: &Analysis) -> Vec<CompletionItem> {
    let mut items = stdlib_items();
    let Some(res) = &analysis.resolve else {
        // Parse hatası varken resolve yok — en azından stdlib + deyim
        // anahtar kelimeleri öner (yarım kod çökmemeli).
        items.extend(
            STMT_KEYWORDS
                .iter()
                .map(|k| item(*k, CompletionItemKind::KEYWORD)),
        );
        return items;
    };
    for (i, data) in res.defs.iter().enumerate() {
        let kind = match data.kind {
            DefKind::Port { .. } | DefKind::Register | DefKind::Wire => {
                CompletionItemKind::VARIABLE
            }
            DefKind::LocalBinding | DefKind::LoopVar | DefKind::PatternBinding => {
                CompletionItemKind::VARIABLE
            }
            DefKind::Instance => CompletionItemKind::VALUE,
            DefKind::Const | DefKind::GenericParam => CompletionItemKind::CONSTANT,
            DefKind::Function | DefKind::Builtin(_) => CompletionItemKind::FUNCTION,
            DefKind::Module | DefKind::ExternModule => CompletionItemKind::MODULE,
            DefKind::Enum => CompletionItemKind::ENUM,
            DefKind::Struct => CompletionItemKind::STRUCT,
            _ => continue,
        };
        let mut it = item(&data.name, kind);
        // ADR-0081 Karar 13: fn'in ayrıntısı imzasıdır.
        if data.kind == DefKind::Function {
            it.detail = analysis.fn_signature(volt_hir::DefId(i as u32));
        }
        items.push(it);
    }
    // Tip detaylarını ekle (def sırası defs ile aynı DEĞİL; ada göre).
    if let Some(typeck) = &analysis.typeck {
        for entry in &mut items {
            if let Some((def, _)) = res.def_by_name(&entry.label) {
                if let Some(ty) = typeck.def_types.get(&def) {
                    entry.detail.get_or_insert(typeck.types.display(*ty));
                }
            }
        }
    }
    items
}
