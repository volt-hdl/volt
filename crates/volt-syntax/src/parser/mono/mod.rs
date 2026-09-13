//! Generic argümanlı kullanıcı modülü örneklemesinin monomorfizasyonu
//! (ADR-0041 — `let f = FirFilter<8, 16> { ... }`).
//!
//! Pipeline desugar'ı (ADR-0038) gibi parser katmanında çalışır: isim
//! çözümleme, tip denetimi, alan çıkarımı ve SV üretimi AST düğümlerine
//! anahtarlıdır; her (argüman kombinasyonu) için AYRI bir somut modül
//! üretmek bu geçitlerin hiçbirinde değişiklik gerektirmez.
//!
//! Kurallar:
//! * `FirFilter<8, 16>` → `FirFilter_8_16`; aynı argümanlarla ikinci
//!   örnekleme aynı modülü kullanır.
//! * Yalnız const generic parametre; argüman tam sayı LİTERALİ olmalı
//!   (yerleşik primitiflerle aynı kural, E2008). Tip parametresine
//!   argüman E0003 (henüz yok).
//! * ≥1 kez örneklenen şablon `items` listesinden çıkar (arena'da
//!   kalır); hiç örneklenmemiş generic modül dokunulmadan kalır.
//! * İç içe generic örneklemeler (`A<8>` gövdesinde `B<TAPS>`) ikame
//!   sonrası sonraki turda yakalanır; tur sınırı [`MAX_ROUNDS`].

mod clone;
mod pattern;

use std::collections::HashMap;

use volt_ast::builtin::BuiltinPrim;
use volt_ast::{
    ExprKind, GenericArg, GenericParamKind, Idx, InstanceDecl, Item, ItemKind, Name, SourceFile,
    Stmt, StmtKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use clone::Cloner;

/// İç içe generic örnekleme derinliği sınırı.
const MAX_ROUNDS: usize = 64;

/// Dosyadaki generic örneklemeleri açar; tanıları döndürür.
pub fn monomorphize(ast: &mut SourceFile) -> Vec<Diagnostic> {
    let mut mono = Mono {
        modules: collect_modules(ast),
        produced: HashMap::new(),
        expansions: HashMap::new(),
        diagnostics: Vec::new(),
        next_ctx: 1,
        ast,
    };
    mono.run();
    mono.rebuild_items();
    mono.diagnostics
}

/// Şablon modülün generic parametreleri: (ad, const mu?).
struct ModuleInfo {
    item: Idx<Item>,
    params: Vec<(String, bool)>,
}

fn collect_modules(ast: &SourceFile) -> HashMap<String, ModuleInfo> {
    let mut map = HashMap::new();
    for &item in &ast.items {
        if let ItemKind::Module(m) = &ast.items_arena[item].kind {
            let params = m
                .generics
                .iter()
                .map(|g| match &g.kind {
                    GenericParamKind::Type { name, .. } => (name.text.clone(), false),
                    GenericParamKind::Const { name, .. } => (name.text.clone(), true),
                })
                .collect();
            map.insert(m.name.text.clone(), ModuleInfo { item, params });
        }
    }
    map
}

/// Bir örnekleme deyiminden okunan istek (arena ödünç alması bitmiş).
struct Request {
    stmt: Idx<Stmt>,
    target: String,
    target_span: Span,
    args: Vec<ArgInfo>,
}

enum ArgInfo {
    Lit(u128),
    Other(Span),
}

struct Mono<'a> {
    ast: &'a mut SourceFile,
    modules: HashMap<String, ModuleInfo>,
    /// Üretilmiş monomorf adı → öğe.
    produced: HashMap<String, Idx<Item>>,
    /// Şablon öğesi → monomorfları (üretim sırasıyla).
    expansions: HashMap<Idx<Item>, Vec<Idx<Item>>>,
    diagnostics: Vec<Diagnostic>,
    /// Sonraki monomorfun span bağlamı (1'den başlar; 0 = orijinal).
    next_ctx: u16,
}

impl Mono<'_> {
    fn run(&mut self) {
        let mut pending: Vec<Idx<Item>> = self
            .ast
            .items
            .iter()
            .copied()
            .filter(|&i| matches!(self.ast.items_arena[i].kind, ItemKind::Module(_)))
            .collect();
        for round in 0..=MAX_ROUNDS {
            let mut next = Vec::new();
            for item in pending {
                next.extend(self.process_module(item));
            }
            if next.is_empty() {
                return;
            }
            if round == MAX_ROUNDS {
                self.err_depth_exceeded(&next);
                return;
            }
            pending = next;
        }
    }

    /// Modül gövdesindeki generic örneklemeleri açar; yeni monomorfları döndürür.
    fn process_module(&mut self, item: Idx<Item>) -> Vec<Idx<Item>> {
        // Şablonun kendisi işlenmez: gövdesindeki `Inner<W>` ancak ikame
        // sonrası (klonda) literal argüman taşır.
        if self.is_generic_template(item) {
            return Vec::new();
        }
        let requests = self.collect_requests(item);
        let mut created = Vec::new();
        for req in requests {
            if let Some(new_item) = self.instantiate(&req) {
                created.push(new_item);
            }
        }
        created
    }

    fn is_generic_template(&self, item: Idx<Item>) -> bool {
        matches!(&self.ast.items_arena[item].kind, ItemKind::Module(m) if !m.generics.is_empty())
    }

    fn collect_requests(&self, item: Idx<Item>) -> Vec<Request> {
        let ItemKind::Module(m) = &self.ast.items_arena[item].kind else {
            return Vec::new();
        };
        m.body
            .iter()
            .filter_map(|&stmt| match &self.ast.stmts[stmt].kind {
                StmtKind::Instance(inst) => self.request_of(stmt, inst),
                _ => None,
            })
            .collect()
    }

    fn request_of(&self, stmt: Idx<Stmt>, inst: &InstanceDecl) -> Option<Request> {
        if inst.generic_args.is_empty() || inst.module_path.segments.len() != 1 {
            return None;
        }
        let head = &inst.module_path.segments[0];
        if BuiltinPrim::from_name(&head.text).is_some() {
            return None; // stdlib primitifi — typeck denetler
        }
        let args = inst.generic_args.iter().map(|a| self.arg_info(a)).collect();
        Some(Request {
            stmt,
            target: head.text.clone(),
            target_span: head.span,
            args,
        })
    }

    fn arg_info(&self, arg: &GenericArg) -> ArgInfo {
        match arg {
            GenericArg::Const(e) => match &self.ast.exprs[*e].kind {
                ExprKind::IntLit { value, .. } => ArgInfo::Lit(*value),
                _ => ArgInfo::Other(self.ast.exprs[*e].span),
            },
            GenericArg::Type(t) => ArgInfo::Other(self.ast.types[*t].span),
        }
    }

    /// İsteği doğrular, gerekirse klonlar, deyimi yeniden yazar.
    fn instantiate(&mut self, req: &Request) -> Option<Idx<Item>> {
        let Some(info) = self.modules.get(&req.target) else {
            return None; // bilinmeyen modül — E1001 çözümleyicide
        };
        let (template, params) = (info.item, info.params.clone());
        let Some(values) = self.check_args(req, &params) else {
            self.clear_generic_args(req.stmt); // kaskad bastırma
            return None;
        };
        let mangled = mangle(&req.target, &values);
        let (item, is_new) = match self.produced.get(&mangled) {
            Some(&existing) => (existing, false),
            None => {
                let subst = params
                    .iter()
                    .map(|(n, _)| n.clone())
                    .zip(values.iter().copied())
                    .collect();
                let item = self.clone_template(template, subst, mangled.clone());
                self.produced.insert(mangled.clone(), item);
                self.expansions.entry(template).or_default().push(item);
                (item, true)
            }
        };
        self.rewrite_instance(req.stmt, mangled);
        is_new.then_some(item)
    }

    /// Sayı/kind denetimi; başarıda argüman değerleri.
    fn check_args(&mut self, req: &Request, params: &[(String, bool)]) -> Option<Vec<u128>> {
        if params.is_empty() {
            self.err_not_generic(req);
            return None;
        }
        if req.args.len() != params.len() {
            self.err_arity(req, params.len());
            return None;
        }
        let mut values = Vec::with_capacity(params.len());
        for (arg, (pname, is_const)) in req.args.iter().zip(params) {
            match (arg, is_const) {
                (ArgInfo::Lit(v), true) => values.push(*v),
                (ArgInfo::Other(span), true) => {
                    self.err_not_literal(req, pname, *span);
                    return None;
                }
                (ArgInfo::Lit(_), false) | (ArgInfo::Other(_), false) => {
                    self.err_type_param(req, pname);
                    return None;
                }
            }
        }
        Some(values)
    }

    fn clone_template(
        &mut self,
        template: Idx<Item>,
        subst: HashMap<String, u128>,
        mangled: String,
    ) -> Idx<Item> {
        let (span, doc, visibility) = {
            let it = &self.ast.items_arena[template];
            (it.span, it.doc.clone(), it.visibility)
        };
        // Şablon arena'da kalır; klon yeni düğümlerle kurulur. Cloner
        // `&mut SourceFile` isterken şablona `&ModuleDecl` tutulamaz —
        // bildirim geçici olarak öğeden alınır, klon bitince geri konur.
        let template_decl = take_module_shallow(self.ast, template);
        let (attrs, kind) = {
            let attrs_src = template_attrs(self.ast, template);
            let ctx = self.next_ctx;
            self.next_ctx = self.next_ctx.saturating_add(1);
            let mut cloner = Cloner::new(self.ast, subst, ctx);
            let module = cloner.clone_module(&template_decl, mangled);
            let attrs = cloner.clone_attrs(&attrs_src);
            (attrs, ItemKind::Module(module))
        };
        restore_module_shallow(self.ast, template, template_decl);
        self.ast.items_arena.alloc(Item {
            span,
            attrs,
            doc,
            visibility,
            kind,
        })
    }

    fn rewrite_instance(&mut self, stmt: Idx<Stmt>, mangled: String) {
        if let StmtKind::Instance(inst) = &mut self.ast.stmts[stmt].kind {
            let span = inst.module_path.segments[0].span;
            inst.module_path.span = span;
            inst.module_path.segments = vec![Name {
                text: mangled,
                span,
            }];
            inst.generic_args.clear();
        }
    }

    fn clear_generic_args(&mut self, stmt: Idx<Stmt>) {
        if let StmtKind::Instance(inst) = &mut self.ast.stmts[stmt].kind {
            inst.generic_args.clear();
        }
    }

    /// Kullanılan şablonlar yerine monomorfları (şablon konumunda).
    fn rebuild_items(&mut self) {
        let old = std::mem::take(&mut self.ast.items);
        let mut items = Vec::with_capacity(old.len());
        for item in old {
            match self.expansions.get(&item) {
                Some(monos) => items.extend(monos.iter().copied()),
                None => items.push(item),
            }
        }
        self.ast.items = items;
    }

    // ═══ Tanılar (5 parça: kod, konum, açıklama, öneri, spec notu) ═══

    fn err_not_generic(&mut self, req: &Request) {
        let name = &req.target;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2003,
                lstr!(en: "'{name}' takes no generic arguments"; tr: "'{name}' generic argüman almaz"),
                LabeledSpan::primary(
                    req.target_span,
                    lstr!(en: "module is not generic"; tr: "modül generic değil"),
                ),
                lstr!(en: "write it as {name} {{ ... }}, or add <const N: u32> to the module";
                      tr: "{name} {{ ... }} biçiminde yazın ya da modüle <const N: u32> ekleyin"),
            )
            .with_note(NoteKind::Note, adr_note()),
        );
    }

    fn err_arity(&mut self, req: &Request, want: usize) {
        let (name, got) = (&req.target, req.args.len());
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2003,
                lstr!(en: "'{name}' expects {want} generic argument(s), got {got}";
                      tr: "'{name}' {want} generic argüman bekler, {got} verildi"),
                LabeledSpan::primary(
                    req.target_span,
                    lstr!(en: "wrong generic argument count"; tr: "yanlış generic argüman sayısı"),
                ),
                lstr!(en: "give exactly {want} argument(s), one per <const ...> parameter";
                      tr: "her <const ...> parametresi için bir tane, tam {want} argüman verin"),
            )
            .with_note(NoteKind::Note, adr_note()),
        );
    }

    fn err_not_literal(&mut self, req: &Request, param: &str, span: Span) {
        let name = &req.target;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2008,
                lstr!(en: "{name} {param} must be a compile-time integer literal";
                      tr: "{name} {param} derleme zamanı tam sayı literali olmalı"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "not an integer literal"; tr: "tam sayı literali değil"),
                ),
                lstr!(en: "write the value directly, e.g. {name}<8> {{ ... }}";
                      tr: "değeri doğrudan yazın, ör. {name}<8> {{ ... }}"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "the monomorphised module name is built from the literal values (ADR-0041)";
                      tr: "monomorf modül adı literal değerlerden kurulur (ADR-0041)"),
            ),
        );
    }

    fn err_type_param(&mut self, req: &Request, param: &str) {
        let name = &req.target;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E0003,
                lstr!(en: "type generic arguments on user modules are not supported yet ('{name}<{param}>')";
                      tr: "kullanıcı modüllerinde tip generic argümanı henüz desteklenmiyor ('{name}<{param}>')"),
                LabeledSpan::primary(
                    req.target_span,
                    lstr!(en: "only <const N: u32> parameters can be instantiated";
                          tr: "yalnız <const N: u32> parametreleri örneklenebilir"),
                ),
                lstr!(en: "use a const width parameter and sint<W>/uint<W> port types";
                      tr: "const genişlik parametresi ve sint<W>/uint<W> port tipleri kullanın"),
            )
            .with_note(NoteKind::Note, adr_note()),
        );
    }

    fn err_depth_exceeded(&mut self, items: &[Idx<Item>]) {
        let Some(&first) = items.first() else {
            return;
        };
        let span = self.ast.items_arena[first].span;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2003,
                lstr!(en: "generic instantiation depth exceeded ({MAX_ROUNDS} rounds)";
                      tr: "generic örnekleme derinliği aşıldı ({MAX_ROUNDS} tur)"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "instantiations keep producing new modules"; tr: "örneklemeler sürekli yeni modül üretiyor"),
                ),
                lstr!(en: "break the recursive generic instantiation chain";
                      tr: "özyinelemeli generic örnekleme zincirini kırın"),
            )
            .with_note(NoteKind::Note, adr_note()),
        );
    }
}

fn adr_note() -> String {
    lstr!(en: "generic instantiation is monomorphised per argument tuple (ADR-0041)";
          tr: "generic örnekleme argüman demeti başına monomorflanır (ADR-0041)")
}

/// `FirFilter<8, 16>` → `FirFilter_8_16`.
fn mangle(name: &str, values: &[u128]) -> String {
    let mut out = name.to_string();
    for v in values {
        out.push('_');
        out.push_str(&v.to_string());
    }
    out
}

/// Şablon modülü öğeden geçici olarak çıkarır (yerine boş `Error`).
/// Klon, arena'ya yeni düğümler eklerken `&mut SourceFile` gerekir;
/// şablona eş zamanlı `&ModuleDecl` tutmak ödünç kuralını bozar.
fn take_module_shallow(ast: &mut SourceFile, item: Idx<Item>) -> volt_ast::ModuleDecl {
    match std::mem::replace(&mut ast.items_arena[item].kind, ItemKind::Error) {
        ItemKind::Module(m) => m,
        _ => unreachable!("yalnız modüller monomorflanır"),
    }
}

fn restore_module_shallow(ast: &mut SourceFile, item: Idx<Item>, m: volt_ast::ModuleDecl) {
    ast.items_arena[item].kind = ItemKind::Module(m);
}

/// Öğe niteliklerinin sığ kopyası (Cloner derinini alır).
fn template_attrs(ast: &SourceFile, item: Idx<Item>) -> Vec<volt_ast::Attribute> {
    ast.items_arena[item]
        .attrs
        .iter()
        .map(|a| volt_ast::Attribute {
            span: a.span,
            name: a.name.clone(),
            args: a
                .args
                .iter()
                .map(|arg| match arg {
                    volt_ast::AttrArg::Named { name, value } => volt_ast::AttrArg::Named {
                        name: name.clone(),
                        value: *value,
                    },
                    volt_ast::AttrArg::Positional(e) => volt_ast::AttrArg::Positional(*e),
                })
                .collect(),
        })
        .collect()
}
