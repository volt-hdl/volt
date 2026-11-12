//! Generic argümanlı kullanıcı modülü örneklemesinin monomorfizasyonu
//! (ADR-0041 — `let f = FirFilter<8, 16> { ... }`).
//!
//! Pipeline desugar'ı (ADR-0038) gibi parser katmanında çalışır: isim
//! çözümleme, tip denetimi, alan çıkarımı ve SV üretimi AST düğümlerine
//! anahtarlıdır; her (argüman kombinasyonu) için AYRI bir somut modül
//! üretmek bu geçitlerin hiçbirinde değişiklik gerektirmez.
//!
//! Kurallar:
//! * `FirFilter<8, 16>` → `FirFilter_8_16`; aynı şablonun aynı
//!   argümanlarla ikinci örneklemesi aynı modülü kullanır. Tablo
//!   (şablon, argüman değerleri) ile anahtarlıdır: iki FARKLI şablon aynı
//!   adı kurarsa (`M<1, 8>` ve `M_1<8>` → `M_1_8`, #205) E1003 — Volt
//!   üretilen adı kendiliğinden değiştirmez (ADR-0090).
//! * Yalnız const generic parametre; argüman tam sayı LİTERALİ olmalı
//!   (yerleşik primitiflerle aynı kural, E2008). Tip parametresine
//!   argüman E0003 (henüz yok).
//! * ≥1 kez örneklenen şablon `items` listesinden çıkar (arena'da
//!   kalır); hiç örneklenmemiş generic modül dokunulmadan kalır.
//! * İç içe generic örneklemeler (`A<8>` gövdesinde `B<TAPS>`) ikame
//!   sonrası sonraki turda yakalanır; tur sınırı [`MAX_ROUNDS`].

mod budget;
mod clone;
mod pattern;
pub(crate) mod scope;
pub(crate) mod unroll;

use std::collections::HashMap;

use volt_ast::builtin::BuiltinPrim;
use volt_ast::{
    ExprKind, GenericArg, GenericParamKind, Idx, InstanceDecl, Item, ItemKind, Name, SourceFile,
    Stmt, StmtKind, TypeRef, TypeRefKind,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use budget::ExpansionBudget;
pub(crate) use budget::MAX_EXPANSION_NODES;
use clone::Cloner;
pub use scope::shadowing_warning;

/// İç içe generic örnekleme derinliği sınırı.
const MAX_ROUNDS: usize = 64;

/// Dosyadaki generic örneklemeleri açar; tanıları döndürür.
pub fn monomorphize(ast: &mut SourceFile) -> Vec<Diagnostic> {
    let mut mono = Mono {
        modules: collect_modules(ast),
        produced: HashMap::new(),
        names: HashMap::new(),
        expansions: HashMap::new(),
        diagnostics: Vec::new(),
        next_ctx: 1,
        budget: ExpansionBudget::default(),
        ast,
    };
    mono.run();
    mono.rebuild_items();
    mono.check_case_collisions();
    mono.diagnostics
}

/// Şablon modülün generic parametreleri: (ad, const mu?) ve const
/// parametrenin tip aralığı (#262).
struct ModuleInfo {
    item: Idx<Item>,
    params: Vec<(String, bool)>,
    ranges: Vec<Option<ParamRange>>,
}

/// Const generic parametrenin tipinden okunan aralık: `uN`, `iN` ve
/// genişliği literal olan `uint<N>`/`sint<N>` (#262). Başka tip
/// (`bool`, genişliği ad olan `uint<W>`) denetlenmez.
#[derive(Clone, Copy)]
struct ParamRange {
    signed: bool,
    width: u32,
}

impl ParamRange {
    fn of(ast: &SourceFile, ty: Idx<TypeRef>) -> Option<Self> {
        let lit_width = |e: Idx<volt_ast::Expr>| {
            if let ExprKind::IntLit { value, .. } = ast.exprs[e].kind {
                u32::try_from(value).ok()
            } else {
                None
            }
        };
        let kind = &ast.types[ty].kind;
        let (signed, width) = if let TypeRefKind::UInt(w) = *kind {
            (false, u32::from(w))
        } else if let TypeRefKind::SInt(w) = *kind {
            (true, u32::from(w))
        } else if let TypeRefKind::UIntN(e) = *kind {
            (false, lit_width(e)?)
        } else if let TypeRefKind::SIntN(e) = *kind {
            (true, lit_width(e)?)
        } else {
            return None;
        };
        Some(Self { signed, width })
    }

    /// Argüman (negatif olmayan literal) aralığa sığıyor mu; en büyük değer.
    fn max(self) -> u128 {
        let bits = if self.signed {
            self.width.saturating_sub(1)
        } else {
            self.width
        };
        if bits >= 128 {
            u128::MAX
        } else {
            (1u128 << bits) - 1
        }
    }
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
            let ranges = m
                .generics
                .iter()
                .map(|g| match &g.kind {
                    GenericParamKind::Type { .. } => None,
                    GenericParamKind::Const { ty, .. } => ParamRange::of(ast, *ty),
                })
                .collect();
            map.insert(
                m.name.text.clone(),
                ModuleInfo {
                    item,
                    params,
                    ranges,
                },
            );
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

/// Bir monomorf adını ilk kuran örnekleme: çakışma tanısı iki şablonu ve
/// iki örnek sitesini gösterir.
#[derive(Clone)]
struct NameOwner {
    template: Idx<Item>,
    /// Kurulan monomorf öğesi.
    item: Idx<Item>,
    /// Kaynaktaki yazılışı: `M<1, 8>`.
    written: String,
    /// Örneklemedeki modül adının konumu.
    site: Span,
}

enum ArgInfo {
    Lit(u128),
    Other(Span),
}

/// Birimde kalan bir modül adı (#250, harf büyüklüğü denetimi).
#[derive(Clone)]
struct ModuleEntry {
    item: Idx<Item>,
    name: String,
    /// Bildirimin adı ya da (monomorfta) adı kuran örnekleme.
    site: Span,
    /// Monomorfta: örneklemenin yazılışı (`M<8>`) ve şablonu.
    generated: Option<(String, Idx<Item>)>,
}

struct Mono<'a> {
    ast: &'a mut SourceFile,
    modules: HashMap<String, ModuleInfo>,
    /// (şablon, argüman değerleri) → monomorf öğesi.
    produced: HashMap<(Idx<Item>, Vec<u128>), Idx<Item>>,
    /// Üretilmiş monomorf adı → onu ilk kuran örnekleme (#205).
    names: HashMap<String, NameOwner>,
    /// Şablon öğesi → monomorfları (üretim sırasıyla).
    expansions: HashMap<Idx<Item>, Vec<Idx<Item>>>,
    diagnostics: Vec<Diagnostic>,
    /// Sonraki monomorfun span bağlamı (1'den başlar; 0 = orijinal).
    next_ctx: u16,
    /// Açılım düğüm bütçesi (ADR-0068 §6): monomorf klonları ve `for`
    /// açılımı AYNI sayaçtan düşer — derleme birimi başına bir tane.
    budget: ExpansionBudget,
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
        // Modül seviyesi `for` açılımı (ADR-0056) önce: açılan gövdedeki
        // örneklemeler bu turun istekleri olur; klonlanmış generic
        // gövdede sınırlar ikame sonrası literaldir.
        unroll::unroll_module(
            self.ast,
            item,
            &mut self.next_ctx,
            &mut self.budget,
            &mut self.diagnostics,
        );
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

    /// Birimde bu adlı bir generic modül (şablon) var mı?
    fn is_generic_name(&self, name: &str) -> bool {
        self.modules.get(name).is_some_and(|m| !m.params.is_empty())
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
        if inst.module_path.segments.len() != 1 {
            return None;
        }
        let head = &inst.module_path.segments[0];
        if BuiltinPrim::from_name(&head.text).is_some() {
            return None; // stdlib primitifi — typeck denetler
        }
        // Argümansız örnek ancak generic modülü hedefliyorsa istektir:
        // generic parametrelerin varsayılan değeri yok (gramerde
        // `GenericParam` `= değer` almaz), sayı kuralı E2003 verir (#249).
        // Yoksa örnek, monomorfun adını taşıyan modüle sessizce bağlanırdı.
        if inst.generic_args.is_empty() && !self.is_generic_name(&head.text) {
            return None;
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
        let (template, params, ranges) = (info.item, info.params.clone(), info.ranges.clone());
        let Some(values) = self.check_args(req, &params, &ranges) else {
            self.clear_generic_args(req.stmt); // kaskad bastırma
            return None;
        };
        let mangled = mangle(&req.target, &values);
        let key = (template, values);
        let (item, is_new) = match self.produced.get(&key) {
            Some(&existing) => (existing, false),
            None => {
                // Ad başka bir (şablon, argümanlar) demetine ait: iki farklı
                // şablon aynı adı kuruyor (#205). Yeniden adlandırma yok.
                if let Some(owner) = self.names.get(&mangled).cloned() {
                    let written = written(&req.target, &key.1);
                    self.err_name_collision(&owner, template, req, &written, &mangled);
                    self.clear_generic_args(req.stmt); // kaskad bastırma
                    return None;
                }
                // Bütçe (ADR-0068 §6): klon sayısı × şablon boyu da
                // sınırlı — aşımda tek E2027, sonraki örneklemeler açılmaz.
                if self.budget.exhausted() {
                    if self.budget.take_report() {
                        self.err_node_budget(req);
                    }
                    self.clear_generic_args(req.stmt);
                    return None;
                }
                // Argüman literaldir, değeriyle ikame edilir (2^128 - 1'e
                // kadar; doyurma yok, #207).
                let item = self.clone_template(template, key.1.clone(), mangled.clone());
                let owner = NameOwner {
                    template,
                    item,
                    written: written(&req.target, &key.1),
                    site: req.target_span,
                };
                self.names.insert(mangled.clone(), owner);
                self.produced.insert(key, item);
                self.expansions.entry(template).or_default().push(item);
                (item, true)
            }
        };
        self.rewrite_instance(req.stmt, mangled);
        is_new.then_some(item)
    }

    /// Sayı/kind denetimi; başarıda argüman değerleri.
    fn check_args(
        &mut self,
        req: &Request,
        params: &[(String, bool)],
        ranges: &[Option<ParamRange>],
    ) -> Option<Vec<u128>> {
        if params.is_empty() {
            self.err_not_generic(req);
            return None;
        }
        if req.args.len() != params.len() {
            self.err_arity(req, params);
            return None;
        }
        let mut values = Vec::with_capacity(params.len());
        for ((arg, (pname, is_const)), range) in req.args.iter().zip(params).zip(ranges) {
            match (arg, is_const) {
                (ArgInfo::Lit(v), true) => {
                    // #262: argüman parametrenin tipine sığmalı.
                    if let Some(range) = range.filter(|r| *v > r.max()) {
                        self.err_arg_out_of_range(req, pname, *v, range);
                        return None;
                    }
                    values.push(*v);
                }
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

    /// Şablonu klonlar; `values` generic parametrelerin sırasıyla değeri.
    fn clone_template(
        &mut self,
        template: Idx<Item>,
        values: Vec<u128>,
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
            // Kapsam zinciri: modül kapsamının ilk bildirimleri const generic
            // parametreler (name-resolution.md §5); gövdede onlara bağlanan
            // yol literal olur, iç kapsamda yeniden bildirilen ad olmaz (#130).
            let params: Vec<(Name, u128)> = template_decl
                .generics
                .iter()
                .filter_map(|g| match &g.kind {
                    GenericParamKind::Const { name, .. } => Some(name.clone()),
                    GenericParamKind::Type { .. } => None,
                })
                .zip(values)
                .collect();
            let mut cloner = Cloner::new(self.ast, &mut self.budget, ctx).with_generics(&params);
            let module = cloner.clone_module(&template_decl, mangled);
            let attrs = cloner.clone_attrs(&attrs_src);
            // Çözücü parametreleri bu tablodan bildirir (gölgeleme, E1003).
            let names = params.iter().map(|(n, _)| cloner.tag_name(n)).collect();
            cloner
                .ast
                .write(|ast| ast.generate.mono_generics.insert(module.name.span, names));
            (attrs, ItemKind::Module(module))
        };
        restore_module_shallow(self.ast, template, template_decl);
        self.budget.charge(1);
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

    /// #250: birimde kalan modül adları (kullanıcı modülleri, hiç
    /// örneklenmeyen generic modüller ve monomorflar) yalnız harf
    /// büyüklüğüyle ayrılamaz. Her modül adını taşıyan dosyalara yazılır
    /// (`rtl/<Modül>.sv`, `constraints/<Modül>.sdc`): büyük/küçük harf
    /// duyarsız dosya sisteminde (Windows, macOS) `Foo.sv` ile `foo.sv` tek
    /// dosyadır; SVA dosyası ve denetleyicisi adın küçük harflisini kullanır
    /// (`formal/foo.sva`, `foo_sva`), orada her sistemde tek dosyadır. Volt
    /// adı değiştirmez (ADR-0090): E1003. Birebir aynı ad çözümleyicinin
    /// E1003'üdür (aynı kapsamda çift tanım); burada atlanır. Sıra kaynak
    /// sırasıdır: tanı her sonraki adda, kümenin ilk adına karşı.
    fn check_case_collisions(&mut self) {
        let mut entries: Vec<ModuleEntry> = self
            .ast
            .items
            .iter()
            .filter_map(|&item| match &self.ast.items_arena[item].kind {
                ItemKind::Module(m) => Some(self.module_entry(item, &m.name)),
                _ => None,
            })
            .collect();
        entries.sort_by_key(|e| span_order(e.site));
        let mut first: HashMap<String, usize> = HashMap::new();
        for i in 0..entries.len() {
            let folded = entries[i].name.to_lowercase();
            match first.get(&folded) {
                Some(&j) if entries[j].name != entries[i].name => {
                    let (a, b) = (entries[j].clone(), entries[i].clone());
                    if !self.reported_as_driver_collision(&a, &b) {
                        self.err_case_collision(&a, &b);
                    }
                }
                Some(_) => {}
                None => {
                    first.insert(folded, i);
                }
            }
        }
    }

    /// Öğe listesindeki modülün adı ve kökeni: monomorf ise onu kuran
    /// örnekleme, değilse bildirimin adı.
    fn module_entry(&self, item: Idx<Item>, name: &Name) -> ModuleEntry {
        match self.names.get(&name.text) {
            Some(owner) if owner.item == item => ModuleEntry {
                item,
                name: name.text.clone(),
                site: owner.site,
                generated: Some((owner.written.clone(), owner.template)),
            },
            _ => ModuleEntry {
                item,
                name: name.text.clone(),
                site: name.span,
                generated: None,
            },
        }
    }

    /// İki bildirilen `@mmio` modülünün sürücü dosya kökü aynıysa
    /// (`GpioRegs`/`GPIORegs` → `gpio_regs`) `@mmio` açılımı çifti E1014
    /// ile zaten raporladı (ADR-0079 §2): aynı kök nedene ikinci tanı yok.
    /// Kökleri ayrılan çift (`AbC` → `ab_c`, `abc`) E1014 almaz, E1003 alır.
    fn reported_as_driver_collision(&self, a: &ModuleEntry, b: &ModuleEntry) -> bool {
        let is_mmio = |e: &ModuleEntry| {
            e.generated.is_none()
                && self.ast.items_arena[e.item]
                    .attrs
                    .iter()
                    .any(|attr| attr.name.text == "mmio")
        };
        is_mmio(a)
            && is_mmio(b)
            && volt_ast::mmio_names::file_stem(&a.name) == volt_ast::mmio_names::file_stem(&b.name)
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

    /// #205: iki farklı şablon aynı monomorf adını kuruyor — E1003
    /// (ADR-0090: üretilen ad değiştirilmez). İleti iki örneklemeyi kaynak
    /// sırasıyla adlandırır; birincil etiket sonrakinde, böylece ileti
    /// örneklerin açılış sırasından bağımsızdır. İki şablon adı da
    /// etiketlenir; help hangi modüllerin adının değişebileceğini söyler.
    fn err_name_collision(
        &mut self,
        owner: &NameOwner,
        template: Idx<Item>,
        req: &Request,
        written: &str,
        name: &str,
    ) {
        let this = (req.target_span, written, template);
        let other = (owner.site, owner.written.as_str(), owner.template);
        let (first, second) = if span_order(other.0) <= span_order(this.0) {
            (other, this)
        } else {
            (this, other)
        };
        let (a, b) = (first.1, second.1);
        let (ta, ta_span) = self.module_name(first.2);
        let (tb, tb_span) = self.module_name(second.2);
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E1003,
                lstr!(en: "'{name}' is the generated module name of both {a} and {b}";
                      tr: "'{name}' hem {a} hem {b} için üretilen modül adı"),
                LabeledSpan::primary(
                    second.0,
                    lstr!(en: "becomes '{name}'"; tr: "'{name}' adını alıyor"),
                ),
                lstr!(en: "rename generic module '{ta}' or '{tb}' — Volt does not rename generated SystemVerilog module names";
                      tr: "'{ta}' ya da '{tb}' generic modülünü yeniden adlandırın — Volt üretilen SystemVerilog modül adlarını değiştirmez"),
            )
            .with_secondary(first.0, lstr!(en: "also '{name}'"; tr: "bu da '{name}'"))
            .with_secondary(ta_span, lstr!(en: "generic module '{ta}'"; tr: "generic modül '{ta}'"))
            .with_secondary(tb_span, lstr!(en: "generic module '{tb}'"; tr: "generic modül '{tb}'"))
            .with_note(
                NoteKind::Reason,
                lstr!(en: "a generic instance's module name is the module name followed by its argument values: {a} becomes '{name}'";
                      tr: "generic örneğin modül adı, modül adının ardına argüman değerleri eklenerek kurulur: {a} '{name}' olur"),
            ),
        );
    }

    /// #250: `a` (kaynakta önce) ve `b` yalnız harf büyüklüğüyle ayrılıyor —
    /// E1003, birincil etiket `b`'de. Monomorfun etiketi örnek sitesinde,
    /// şablonun adı da etiketlenir; help yeniden adlandırılabilecek iki
    /// modülü (monomorfta generic modülü) söyler.
    fn err_case_collision(&mut self, a: &ModuleEntry, b: &ModuleEntry) {
        let (an, bn) = (a.name.as_str(), b.name.as_str());
        let lower = bn.to_lowercase();
        let (ra, rb) = (self.rename_target(a), self.rename_target(b));
        let mut d = Diagnostic::error(
            ErrorCode::E1003,
            lstr!(en: "module names '{an}' and '{bn}' differ only in letter case";
                  tr: "'{an}' ve '{bn}' modül adları yalnız harf büyüklüğünde ayrılıyor"),
            LabeledSpan::primary(b.site, self.entry_label(b)),
            lstr!(en: "rename {ra} or {rb} — Volt does not rename SystemVerilog module names";
                  tr: "{ra} ya da {rb} yeniden adlandırın — Volt SystemVerilog modül adlarını değiştirmez"),
        )
        .with_secondary(a.site, self.entry_label(a));
        for e in [a, b] {
            if let Some((_, template)) = &e.generated {
                let (t, t_span) = self.module_name(*template);
                d = d.with_secondary(
                    t_span,
                    lstr!(en: "generic module '{t}'"; tr: "generic modül '{t}'"),
                );
            }
        }
        self.diagnostics.push(d.with_note(
            NoteKind::Reason,
            lstr!(en: "each module is written to files named after it: 'rtl/{an}.sv' and 'rtl/{bn}.sv' are one file where file names ignore letter case (Windows, macOS), and the contracts of both would go to 'formal/{lower}.sva' on every system";
                  tr: "her modül adını taşıyan dosyalara yazılır: 'rtl/{an}.sv' ile 'rtl/{bn}.sv' dosya adlarında harf büyüklüğünü ayırmayan sistemlerde (Windows, macOS) tek dosyadır; ikisinin kontratları da her sistemde 'formal/{lower}.sva' dosyasına giderdi"),
        ));
    }

    /// Ad etiketi: bildirilen modül ya da adı kuran örnekleme.
    fn entry_label(&self, e: &ModuleEntry) -> String {
        let name = &e.name;
        match &e.generated {
            Some((written, _)) => {
                lstr!(en: "{written} becomes '{name}'"; tr: "{written} '{name}' adını alıyor")
            }
            None => lstr!(en: "module '{name}'"; tr: "modül '{name}'"),
        }
    }

    /// Yeniden adlandırılabilecek modül: monomorfta generic modül.
    fn rename_target(&self, e: &ModuleEntry) -> String {
        match &e.generated {
            Some((_, template)) => {
                let t = self.module_name(*template).0;
                lstr!(en: "generic module '{t}'"; tr: "'{t}' generic modülünü")
            }
            None => {
                let name = &e.name;
                lstr!(en: "module '{name}'"; tr: "'{name}' modülünü")
            }
        }
    }

    /// Şablon öğesinin adı ve ad konumu.
    fn module_name(&self, item: Idx<Item>) -> (String, Span) {
        match &self.ast.items_arena[item].kind {
            ItemKind::Module(m) => (m.name.text.clone(), m.name.span),
            _ => unreachable!("yalnız modüller monomorflanır"),
        }
    }

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

    fn err_arity(&mut self, req: &Request, params: &[(String, bool)]) {
        let (name, got, want) = (&req.target, req.args.len(), params.len());
        let message = lstr!(en: "'{name}' expects {want} generic argument(s), got {got}";
                            tr: "'{name}' {want} generic argüman bekler, {got} verildi");
        if got > 0 {
            self.diagnostics.push(
                Diagnostic::error(
                    ErrorCode::E2003,
                    message,
                    LabeledSpan::primary(
                        req.target_span,
                        lstr!(en: "wrong generic argument count"; tr: "yanlış generic argüman sayısı"),
                    ),
                    lstr!(en: "give exactly {want} argument(s), one per <const ...> parameter";
                          tr: "her <const ...> parametresi için bir tane, tam {want} argüman verin"),
                )
                .with_note(NoteKind::Note, adr_note()),
            );
            return;
        }
        // #249: argümansız örnek — parametreleri adlarıyla ve bir yazılışla.
        let names: Vec<String> = params.iter().map(|(p, _)| format!("'{p}'")).collect();
        let names = names.join(", ");
        let example = vec!["8"; want].join(", ");
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2003,
                message,
                LabeledSpan::primary(
                    req.target_span,
                    lstr!(en: "generic module instantiated without generic arguments";
                          tr: "generic modül generic argümansız örneklenmiş"),
                ),
                lstr!(en: "give a value for each generic parameter ({names}), e.g. {name}<{example}> {{ ... }}";
                      tr: "her generic parametre ({names}) için bir değer verin, ör. {name}<{example}> {{ ... }}"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "generic parameters have no default values; each argument tuple builds its own module";
                      tr: "generic parametrelerin varsayılan değeri yoktur; her argüman demeti kendi modülünü kurar"),
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
                lstr!(en: "the monomorphised module name is built from the literal values";
                      tr: "monomorf modül adı literal değerlerden kurulur"),
            ),
        );
    }

    /// E2010 (#262): generic argüman parametrenin tipine sığmıyor.
    fn err_arg_out_of_range(&mut self, req: &Request, param: &str, value: u128, range: ParamRange) {
        let name = &req.target;
        let ty = if range.signed {
            format!("i{}", range.width)
        } else {
            format!("u{}", range.width)
        };
        let max = range.max();
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2010,
                lstr!(en: "generic argument {value} does not fit in the parameter {param} : {ty} of {name} (maximum {max})";
                      tr: "{value} generic argümanı {name} modülünün {param} : {ty} parametresine sığmıyor (en çok {max})"),
                LabeledSpan::primary(
                    req.target_span,
                    lstr!(en: "{param} = {value} is outside the parameter's type";
                          tr: "{param} = {value} parametrenin tipinin dışında"),
                ),
                lstr!(en: "use a value up to {max}, or declare {param} with a wider type";
                      tr: "en çok {max} olan bir değer kullanın ya da {param}'yi daha geniş bir tiple bildirin"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "the argument becomes a literal of the parameter's type inside the module; a value that does not fit would be cut";
                      tr: "argüman modülün içinde parametrenin tipinde bir literal olur; sığmayan değer kesilirdi"),
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

    /// Açılım düğüm bütçesi aşıldı (ADR-0068 §6) — birimde tek E2027.
    fn err_node_budget(&mut self, req: &Request) {
        let target = &req.target;
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E2027,
            lstr!(
                en: "instantiating generic module '{target}' exceeded the AST node budget ({MAX_EXPANSION_NODES} nodes per compilation unit)";
                tr: "'{target}' generic modülünü örneklemek AST düğüm bütçesini aştı (derleme birimi başına {MAX_EXPANSION_NODES} düğüm)"
            ),
            LabeledSpan::primary(
                req.target_span,
                lstr!(en: "this instantiation is not expanded"; tr: "bu örnekleme açılmadı"),
            ),
            lstr!(
                en: "use fewer distinct generic arguments, or shrink the generic module body";
                tr: "daha az farklı generic argüman kullanın ya da generic modül gövdesini küçültün"
            ),
        ));
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
    lstr!(en: "generic instantiation is monomorphised per argument tuple";
          tr: "generic örnekleme argüman demeti başına monomorflanır")
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

/// `M<1, 8>`: örneklemenin kaynaktaki yazılışı (değerler ondalık).
fn written(name: &str, values: &[u128]) -> String {
    let args: Vec<String> = values.iter().map(u128::to_string).collect();
    format!("{name}<{}>", args.join(", "))
}

/// Konumların kaynak sırası (dosya, bayt, açılım bağlamı): tanının
/// iletisi örneklerin açılış sırasından bağımsız olsun.
fn span_order(span: Span) -> (u32, u32, u16) {
    (span.file.0, span.start, span.ctx)
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
