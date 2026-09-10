//! Pipeline desugar (ADR-0038): `PipelineDecl` → `ModuleDecl`.
//!
//! Aşamalar arası akan her değer için sınır register'ları
//! (`<aşama>_<isim>_r`), stall/flush muhafız sinyalleri
//! (`stall_<aşama>`, `flush_<aşama>`) ve tek `always_ff` gövdesi
//! üretilir; `stage(X).y` referansları düz isimlere yeniden yazılır.
//! Çıktı sıradan bir modüldür — çözümleme, tip denetimi, L1 zamanlama
//! ve SV üretimi pipeline'ı hiç görmez.

use std::collections::{HashMap, HashSet};

use volt_ast::{
    Block, BlockContext, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind, LValue,
    LetDecl, ModuleDecl, Name, NumBase, OnBlock, OnTrigger, Path, PipelineDecl, RegDecl, Stmt,
    StmtKind, TypeRef, TypeRefKind,
};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use super::pipeline::{pipeline_err, StageRefData, StageRefMap, StageRefTarget};
use super::Parser;

/// Sınır register'ı: (reg adı, kaynak ifadesi, eleman tipi).
type BoundaryReg = (String, Idx<Expr>, Idx<TypeRef>);

/// Aşama başına koşul listeleri (stall kapsamı / önek sonları).
type StageConds = Vec<Vec<Idx<Expr>>>;

/// Aşama-yerel bir değerin desugar kaydı.
struct ValueInfo {
    stage: usize,
    ty: Option<Idx<TypeRef>>,
    name_span: Span,
    /// Gerekli en yüksek sınır register'ının aşama indeksi.
    max_boundary: Option<usize>,
    /// İlk sınır geçen kullanım — E5014 tanısında gösterilir.
    first_cross: Option<Span>,
}

/// `stage(...)` içeren ifadelerin sabitleme bilgisi.
#[derive(Default)]
struct RefFlags {
    has_ref: bool,
    /// (hedef aşamanın gecikmesi, referans span'i).
    first: Option<(usize, Span)>,
}

struct Ctx {
    nstages: usize,
    /// Küçük harfli aşama adları (register/sinyal önekleri).
    lower: Vec<String>,
    stage_index: HashMap<String, usize>,
    values: HashMap<String, ValueInfo>,
    value_order: Vec<String>,
    refs: StageRefMap,
    /// Sentetik bildirim span'leri: pipeline metni içinde artan,
    /// sıfır genişlikli benzersiz konumlar (decl_spans/use_spans
    /// anahtar çakışmasını önler).
    span_file: volt_span::FileId,
    span_base: u32,
    span_limit: u32,
    span_next: u32,
}

impl Ctx {
    fn fresh_span(&mut self) -> Span {
        let p = (self.span_base + self.span_next).min(self.span_limit);
        self.span_next += 1;
        Span::new(self.span_file, p, p)
    }

    fn reg_name(&self, boundary: usize, value: &str) -> String {
        format!("{}_{value}_r", self.lower[boundary])
    }

    fn need_boundary(&mut self, value: &str, boundary: usize, use_span: Span) {
        if let Some(v) = self.values.get_mut(value) {
            v.max_boundary = Some(v.max_boundary.map_or(boundary, |m| m.max(boundary)));
            v.first_cross.get_or_insert(use_span);
        }
    }
}

impl Parser<'_> {
    /// Ayrıştırılmış pipeline'ı modüle indirger. Yapısal hatalar tanı
    /// üretir ama mümkün olduğunca ilerlenir (error-recovery ilkesi).
    pub(crate) fn desugar_pipeline(&mut self, p: PipelineDecl, full_span: Span) -> ItemKind {
        let refs = std::mem::take(&mut self.stage_refs);
        let name_span = p.name.span;

        if p.stages.is_empty() {
            self.push_error(pipeline_err(
                ErrorCode::E5011,
                name_span,
                lstr!(en: "pipeline '{}' has no stages", p.name.text; tr: "'{}' pipeline'ında aşama yok", p.name.text),
                lstr!(en: "declare the stages: stage Fetch {{ ... }}"; tr: "aşamaları bildirin: stage Fetch {{ ... }}"),
            ));
            return ItemKind::Module(ModuleDecl {
                name: p.name,
                generics: Vec::new(),
                ports: p.ports,
                contracts: p.contracts,
                body: p.body,
                closing_name: None,
            });
        }
        if p.stages.len() as u32 != p.depth {
            self.push_error(pipeline_err(
                ErrorCode::E5011,
                p.depth_span,
                lstr!(en: "pipeline declares {} stages but defines {}", p.depth, p.stages.len(); tr: "pipeline {} aşama bildiriyor ama {} aşama tanımlıyor", p.depth, p.stages.len()),
                lstr!(en: "make pipeline(N) match the number of 'stage' blocks"; tr: "pipeline(N) ile 'stage' bloklarının sayısını eşleyin"),
            ));
        }

        let mut ctx = self.build_ctx(&p, refs, full_span);

        // Aşama gövdelerini yeniden yaz; let'leri kaldır (hoist).
        // hoisted: (isim, aşama, LetDecl); remainder: aşamada kalan
        // ardışık deyimler (stall muhafızıyla on-clk'ye girecek).
        let mut hoisted: Vec<(String, usize, LetDecl)> = Vec::new();
        let mut remainders: Vec<Vec<BlockStmt>> = Vec::new();
        for (i, st) in p.stages.iter().enumerate() {
            let stmts = std::mem::take(&mut self.ast.blocks[st.body].stmts);
            let mut rest = Vec::new();
            for bs in stmts {
                match bs {
                    BlockStmt::Let(l) => {
                        let mut fl = RefFlags::default();
                        self.rw_expr(l.value, Some(i), &mut ctx, &mut fl);
                        if fl.has_ref {
                            let src = fl.first.map_or(l.name.span, |(_, s)| s);
                            self.ast.timing.pinned.insert(l.name.span, (i as u32, src));
                        }
                        hoisted.push((l.name.text.clone(), i, l));
                    }
                    other => {
                        self.rw_block_stmt(&other, Some(i), &mut ctx);
                        rest.push(other);
                    }
                }
            }
            remainders.push(rest);
        }

        // Stall/flush koşulları, modül gövdesi ve kontratlar.
        for sd in &p.stalls {
            let mut fl = RefFlags::default();
            self.rw_expr(sd.cond, sd.in_stage, &mut ctx, &mut fl);
        }
        for fd in &p.flushes {
            let mut fl = RefFlags::default();
            self.rw_expr(fd.cond, fd.in_stage, &mut ctx, &mut fl);
        }
        for &si in &p.body {
            self.rw_stmt(si, &mut ctx);
        }
        for c in &p.contracts {
            let mut fl = RefFlags::default();
            self.rw_expr(c.expr, None, &mut ctx, &mut fl);
        }

        // Çözülmemiş stage referansları (bozuk hedefler) Error kalır;
        // tanıları resolve_stage_ref üretti. Kalanları temizle.
        ctx.refs.clear();

        // Stall/flush kümeleri.
        let (stall_conds, ends_at) = self.stall_sets(&p, &ctx);
        let flush_conds = self.flush_sets(&p, &ctx);

        // Sınır register'ları: değer → aşama..=max_boundary zinciri.
        // boundary_regs[b] = (reg adı, kaynak ifadesi, tip).
        let mut boundary_regs: Vec<Vec<BoundaryReg>> = vec![Vec::new(); ctx.nstages];
        let mut reg_stmts: Vec<Idx<Stmt>> = Vec::new();
        let order = ctx.value_order.clone();
        for vname in &order {
            let (stage, ty, name_span, maxb, cross) = {
                let v = &ctx.values[vname];
                (v.stage, v.ty, v.name_span, v.max_boundary, v.first_cross)
            };
            let Some(maxb) = maxb else { continue };
            let Some(ty) = ty.and_then(|t| self.copy_scalar_ty_check(t)) else {
                let mut d = pipeline_err(
                    ErrorCode::E5014,
                    name_span,
                    lstr!(en: "pipelined value '{vname}' needs an explicit scalar type"; tr: "boru hattında taşınan '{vname}' değerine açık skaler tip gerekli"),
                    lstr!(en: "annotate it: let {vname} : u32 = ... (bool, uN, iN or bits<K>)"; tr: "anotasyon ekleyin: let {vname} : u32 = ... (bool, uN, iN veya bits<K>)"),
                );
                if let Some(cs) = cross {
                    d = d.with_secondary(
                        cs,
                        lstr!(en: "crosses a stage boundary here"; tr: "aşama sınırını burada geçiyor"),
                    );
                }
                self.push_error(d);
                continue;
            };
            for (b, slot) in boundary_regs
                .iter_mut()
                .enumerate()
                .take(maxb + 1)
                .skip(stage)
            {
                let reg = ctx.reg_name(b, vname);
                let src = if b == stage {
                    self.path_expr(vname, name_span)
                } else {
                    self.path_expr(&ctx.reg_name(b - 1, vname), name_span)
                };
                let rty = self.copy_ty(ty, name_span);
                let init = self.zero_expr(rty, name_span);
                let rspan = ctx.fresh_span();
                let stmt = self.ast.stmts.alloc(Stmt {
                    span: name_span,
                    attrs: Vec::new(),
                    kind: StmtKind::Reg(RegDecl {
                        name: Name {
                            text: reg.clone(),
                            span: rspan,
                        },
                        domain: None,
                        ty: Some(rty),
                        init,
                    }),
                });
                reg_stmts.push(stmt);
                slot.push((reg, src, ty));
            }
        }

        // stall_<s> / flush_<s> sinyal adları.
        let mut stall_name: HashMap<usize, String> = HashMap::new();
        let mut flush_name: HashMap<usize, String> = HashMap::new();
        for s in 0..ctx.nstages {
            if !stall_conds[s].is_empty() {
                stall_name.insert(s, format!("stall_{}", ctx.lower[s]));
            }
            if !flush_conds[s].is_empty() {
                flush_name.insert(s, format!("flush_{}", ctx.lower[s]));
            }
        }

        // on-clk gövdesi — hangi muhafız sinyallerinin kullanıldığını izler.
        let mut used_guards: HashSet<String> = HashSet::new();
        let clock = self.single_clock(&p, name_span);
        let on_stmt = clock.map(|clk| {
            self.build_on_block(
                &clk,
                &remainders,
                &boundary_regs,
                &stall_name,
                &flush_name,
                &stall_conds,
                &ends_at,
                &mut used_guards,
                &mut ctx,
                name_span,
            )
        });

        // Kullanıcı kodundaki muhafız referansları (debug çıkışları gibi).
        let guard_pool: HashSet<String> = stall_name
            .values()
            .chain(flush_name.values())
            .cloned()
            .collect();
        for (_, _, l) in &hoisted {
            self.collect_names(l.value, &guard_pool, &mut used_guards);
        }
        for &si in &p.body {
            self.collect_stmt_names(si, &guard_pool, &mut used_guards);
        }
        for c in &p.contracts {
            self.collect_names(c.expr, &guard_pool, &mut used_guards);
        }

        // Yalnız kullanılan muhafızlar üretilir (W1001 gürültüsü olmasın).
        let mut guard_lets: Vec<(String, LetDecl)> = Vec::new();
        for s in 0..ctx.nstages {
            if let Some(nm) = stall_name.get(&s) {
                if used_guards.contains(nm) {
                    let decl = self.guard_let(nm, &stall_conds[s], s, &mut ctx);
                    guard_lets.push((nm.clone(), decl));
                }
            }
            if let Some(nm) = flush_name.get(&s) {
                if used_guards.contains(nm) {
                    let decl = self.guard_let(nm, &flush_conds[s], s, &mut ctx);
                    guard_lets.push((nm.clone(), decl));
                }
            }
        }

        // Taşınan let'ler + muhafız let'leri: bağımlılık sırası.
        let sorted_lets = self.order_lets(hoisted, guard_lets, &ctx, name_span);
        let let_stmts: Vec<Idx<Stmt>> = sorted_lets
            .into_iter()
            .map(|l| {
                let span = l.name.span;
                self.ast.stmts.alloc(Stmt {
                    span,
                    attrs: Vec::new(),
                    kind: StmtKind::Let(l),
                })
            })
            .collect();

        // Gövde: kullanıcı bildirimleri → sınır reg'leri → let'ler →
        // kalan kullanıcı deyimleri (çıkış atamaları) → always_ff.
        let (user_decls, user_rest): (Vec<Idx<Stmt>>, Vec<Idx<Stmt>>) =
            p.body.iter().copied().partition(|&si| {
                matches!(
                    self.ast.stmts[si].kind,
                    StmtKind::Reg(_) | StmtKind::Wire(_) | StmtKind::Instance(_)
                )
            });
        let mut body: Vec<Idx<Stmt>> = Vec::new();
        body.extend(user_decls);
        body.extend(reg_stmts);
        body.extend(let_stmts);
        body.extend(user_rest);
        body.extend(on_stmt);

        ItemKind::Module(ModuleDecl {
            name: p.name,
            generics: Vec::new(),
            ports: p.ports,
            contracts: p.contracts,
            body,
            closing_name: None,
        })
    }

    fn build_ctx(&mut self, p: &PipelineDecl, refs: StageRefMap, pl_span: Span) -> Ctx {
        let mut stage_index = HashMap::new();
        let mut lower = Vec::new();
        for (i, st) in p.stages.iter().enumerate() {
            if stage_index.insert(st.name.text.clone(), i).is_some() {
                self.push_error(pipeline_err(
                    ErrorCode::E5011,
                    st.name.span,
                    lstr!(en: "duplicate stage name '{}'", st.name.text; tr: "yinelenen aşama adı '{}'", st.name.text),
                    lstr!(en: "every stage needs a distinct name"; tr: "her aşamanın adı farklı olmalı"),
                ));
            }
            lower.push(st.name.text.to_lowercase());
        }

        let mut values = HashMap::new();
        let mut value_order = Vec::new();
        for (i, st) in p.stages.iter().enumerate() {
            // Yalnız üst seviye let'ler aşama değeri olur; iç bloklardaki
            // let'ler yereldir ve sınır geçemez.
            let names: Vec<(String, Option<Idx<TypeRef>>, Span)> = self.ast.blocks[st.body]
                .stmts
                .iter()
                .filter_map(|bs| match bs {
                    BlockStmt::Let(l) => Some((l.name.text.clone(), l.ty, l.name.span)),
                    _ => None,
                })
                .collect();
            for (nm, ty, nspan) in names {
                if values.contains_key(&nm) {
                    self.push_error(pipeline_err(
                        ErrorCode::E5016,
                        nspan,
                        lstr!(en: "stage-local value '{nm}' is defined in more than one stage"; tr: "aşama-yerel '{nm}' değeri birden çok aşamada tanımlı"),
                        lstr!(en: "pipeline-wide names must be unique; rename one of them"; tr: "pipeline genelinde adlar benzersiz olmalı; birini yeniden adlandırın"),
                    ));
                    continue;
                }
                values.insert(
                    nm.clone(),
                    ValueInfo {
                        stage: i,
                        ty,
                        name_span: nspan,
                        max_boundary: None,
                        first_cross: None,
                    },
                );
                value_order.push(nm);
            }
        }

        Ctx {
            nstages: p.stages.len(),
            lower,
            stage_index,
            values,
            value_order,
            refs,
            span_file: pl_span.file,
            span_base: pl_span.start,
            span_limit: pl_span.end.max(pl_span.start),
            span_next: 1,
        }
    }

    // ═══ Yeniden yazma ════════════════════════════════════════════

    fn rw_stmt(&mut self, si: Idx<Stmt>, ctx: &mut Ctx) {
        // Önce indeksler çıkarılır (arena ödünç alması biter), sonra
        // yeniden yazma koşulur.
        let mut exprs: Vec<Idx<Expr>> = Vec::new();
        let mut blocks: Vec<Idx<Block>> = Vec::new();
        let mut pin_let: Option<Span> = None;
        match &self.ast.stmts[si].kind {
            StmtKind::Reg(r) => exprs.push(r.init),
            StmtKind::Let(l) => {
                exprs.push(l.value);
                pin_let = Some(l.name.span);
            }
            StmtKind::Assign(a) => {
                exprs.push(a.rhs);
                exprs.extend(a.lhs.suffixes.iter().flat_map(lvalue_suffix_exprs));
            }
            StmtKind::On(on) => blocks.push(on.body),
            StmtKind::Comb(b) => blocks.push(*b),
            StmtKind::For(f) => {
                exprs.extend([f.start, f.end]);
                blocks.push(f.body);
            }
            StmtKind::Expr(e) => exprs.push(*e),
            StmtKind::Instance(inst) => {
                exprs.extend(inst.bindings.iter().filter_map(|b| b.value));
            }
            StmtKind::Wire(_) | StmtKind::Error => {}
        }
        let mut fl = RefFlags::default();
        for e in exprs {
            self.rw_expr(e, None, ctx, &mut fl);
        }
        for b in blocks {
            self.rw_block(b, None, ctx);
        }
        // Modül seviyesi let'te stage(...) varsa ilk hedefin gecikmesine
        // sabitlenir (bilinçli karışım noktası).
        if let (Some(nspan), Some((age, src))) = (pin_let, fl.first) {
            self.ast.timing.pinned.insert(nspan, (age as u32, src));
        }
    }

    fn rw_block(&mut self, bi: Idx<Block>, cs: Option<usize>, ctx: &mut Ctx) {
        let stmts = std::mem::take(&mut self.ast.blocks[bi].stmts);
        for bs in &stmts {
            self.rw_block_stmt(bs, cs, ctx);
        }
        self.ast.blocks[bi].stmts = stmts;
        if let Some(tail) = self.ast.blocks[bi].tail {
            let mut fl = RefFlags::default();
            self.rw_expr(tail, cs, ctx, &mut fl);
        }
    }

    fn rw_block_stmt(&mut self, bs: &BlockStmt, cs: Option<usize>, ctx: &mut Ctx) {
        let mut fl = RefFlags::default();
        match bs {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                self.rw_expr(*rhs, cs, ctx, &mut fl);
                let idxs: Vec<Idx<Expr>> =
                    lhs.suffixes.iter().flat_map(lvalue_suffix_exprs).collect();
                for e in idxs {
                    self.rw_expr(e, cs, ctx, &mut fl);
                }
            }
            BlockStmt::If(ifstmt) => self.rw_if(ifstmt, cs, ctx),
            BlockStmt::Match(m) => {
                self.rw_expr(m.scrutinee, cs, ctx, &mut fl);
                let (mut exprs, mut blocks) = (Vec::new(), Vec::new());
                collect_arm_idxs(&m.arms, &mut exprs, &mut blocks);
                for e in exprs {
                    self.rw_expr(e, cs, ctx, &mut fl);
                }
                for b in blocks {
                    self.rw_block(b, cs, ctx);
                }
            }
            BlockStmt::Let(l) => self.rw_expr(l.value, cs, ctx, &mut fl),
            BlockStmt::For(f) => {
                self.rw_expr(f.start, cs, ctx, &mut fl);
                self.rw_expr(f.end, cs, ctx, &mut fl);
                self.rw_block(f.body, cs, ctx);
            }
            BlockStmt::Error => {}
        }
    }

    fn rw_if(&mut self, ifstmt: &IfStmt, cs: Option<usize>, ctx: &mut Ctx) {
        let mut fl = RefFlags::default();
        self.rw_expr(ifstmt.cond, cs, ctx, &mut fl);
        self.rw_block(ifstmt.then_block, cs, ctx);
        match &ifstmt.else_branch {
            Some(ElseBranch::Block(b)) => self.rw_block(*b, cs, ctx),
            Some(ElseBranch::If(inner)) => self.rw_if(inner, cs, ctx),
            None => {}
        }
    }

    /// İfade ağacını gezer: aşama-yerel isimleri sınır register'larına,
    /// `stage(X).y` yer tutucularını hedeflerine yeniden yazar.
    fn rw_expr(&mut self, e: Idx<Expr>, cs: Option<usize>, ctx: &mut Ctx, fl: &mut RefFlags) {
        if let Some(sr) = ctx.refs.remove(&e) {
            self.resolve_stage_ref(e, sr, ctx, fl);
            return;
        }

        // Düz isim: aşama-yerel değer mi?
        if let ExprKind::Path(path) = &self.ast.exprs[e].kind {
            if path.segments.len() == 1 {
                let nm = path.segments[0].text.clone();
                let span = self.ast.exprs[e].span;
                if let Some(vstage) = ctx.values.get(&nm).map(|v| v.stage) {
                    match cs {
                        Some(s) if vstage == s => {}
                        Some(s) if vstage < s => {
                            let reg = ctx.reg_name(s - 1, &nm);
                            ctx.need_boundary(&nm, s - 1, span);
                            self.rewrite_path(e, &reg, span, ctx);
                        }
                        Some(_) => {
                            self.push_error(pipeline_err(
                                ErrorCode::E5012,
                                span,
                                lstr!(en: "'{nm}' is defined in a later stage — it does not exist here yet"; tr: "'{nm}' sonraki bir aşamada tanımlı — burada henüz yok"),
                                lstr!(en: "read the live value of a later stage explicitly: stage(X).{nm}"; tr: "sonraki aşamanın canlı değerini açıkça okuyun: stage(X).{nm}"),
                            ));
                        }
                        None => {
                            self.push_error(pipeline_err(
                                ErrorCode::E5012,
                                span,
                                lstr!(en: "stage-local value '{nm}' cannot be referenced by bare name at module level"; tr: "aşama-yerel '{nm}' değerine modül seviyesinde düz adla erişilemez"),
                                lstr!(en: "name the stage whose view you want: stage(X).{nm}"; tr: "hangi aşamanın görünümünü istediğinizi yazın: stage(X).{nm}"),
                            ));
                        }
                    }
                }
                return;
            }
        }

        let (exprs, blocks) = expr_children(&self.ast.exprs[e].kind);
        for c in exprs {
            self.rw_expr(c, cs, ctx, fl);
        }
        for b in blocks {
            self.rw_block(b, cs, ctx);
        }
    }

    fn resolve_stage_ref(
        &mut self,
        e: Idx<Expr>,
        sr: StageRefData,
        ctx: &mut Ctx,
        fl: &mut RefFlags,
    ) {
        let last = ctx.nstages - 1;
        let target = match &sr.target {
            StageRefTarget::Named(n) => match ctx.stage_index.get(&n.text) {
                Some(&i) => i,
                None => {
                    self.push_error(pipeline_err(
                        ErrorCode::E5012,
                        n.span,
                        lstr!(en: "unknown stage '{}'", n.text; tr: "bilinmeyen aşama '{}'", n.text),
                        lstr!(en: "use one of the declared stage names"; tr: "bildirilen aşama adlarından birini kullanın"),
                    ));
                    return;
                }
            },
            StageRefTarget::Relative(k) => {
                let Some(cur) = sr.stage else {
                    self.push_error(pipeline_err(
                        ErrorCode::E5012,
                        sr.span,
                        lstr!(en: "relative stage reference outside a stage body"; tr: "aşama gövdesi dışında göreli aşama referansı"),
                        lstr!(en: "at module level, name the stage: stage(Execute).{}", sr.field.text; tr: "modül seviyesinde aşamayı adlandırın: stage(Execute).{}", sr.field.text),
                    ));
                    return;
                };
                let t = cur as i64 + k;
                if t < 0 || t > last as i64 {
                    self.push_error(pipeline_err(
                        ErrorCode::E5012,
                        sr.span,
                        lstr!(en: "stage({k:+}) from stage {cur} lands on {t}, outside 0..={last}"; tr: "aşama {cur} içinden stage({k:+}) {t}'ye düşer, 0..={last} dışı"),
                        lstr!(en: "the pipeline has {} stages; adjust the offset", ctx.nstages; tr: "pipeline {} aşamalı; uzaklığı düzeltin", ctx.nstages),
                    ));
                    return;
                }
                t as usize
            }
        };

        let field = &sr.field.text;
        let Some(def_stage) = ctx.values.get(field).map(|v| v.stage) else {
            self.push_error(pipeline_err(
                ErrorCode::E5012,
                sr.field.span,
                lstr!(en: "unknown stage-local value '{field}'"; tr: "bilinmeyen aşama-yerel değer '{field}'"),
                lstr!(en: "stage(...).y refers to a 'let' defined in a stage body"; tr: "stage(...).y bir aşama gövdesindeki 'let'e işaret eder"),
            ));
            return;
        };
        if target < def_stage {
            self.push_error(pipeline_err(
                ErrorCode::E5012,
                sr.span,
                lstr!(en: "'{field}' is defined in stage {def_stage}; it does not exist yet in stage {target}"; tr: "'{field}' aşama {def_stage}'de tanımlı; aşama {target}'de henüz yok"),
                lstr!(en: "reference a stage at or after the defining one"; tr: "tanım aşaması veya sonrasındaki bir aşamayı referans edin"),
            ));
            return;
        }

        fl.has_ref = true;
        fl.first.get_or_insert((target, sr.span));
        if target == def_stage {
            let field = field.clone();
            self.rewrite_path(e, &field, sr.span, ctx);
        } else {
            let reg = ctx.reg_name(target - 1, field);
            let field = field.clone();
            ctx.need_boundary(&field, target - 1, sr.span);
            self.rewrite_path(e, &reg, sr.span, ctx);
        }
    }

    fn rewrite_path(&mut self, e: Idx<Expr>, name: &str, span: Span, ctx: &mut Ctx) {
        let nspan = ctx.fresh_span();
        self.ast.exprs[e] = Expr {
            span,
            kind: ExprKind::Path(Path {
                span,
                segments: vec![Name {
                    text: name.to_string(),
                    span: nspan,
                }],
            }),
        };
    }

    // ═══ Stall / flush kümeleri ═══════════════════════════════════

    /// Dönüş: (aşama başına stall koşulları, sınır başına önek-sonu
    /// koşulları — bubble kaynağı).
    fn stall_sets(&mut self, p: &PipelineDecl, ctx: &Ctx) -> (StageConds, StageConds) {
        let mut per_stage = vec![Vec::new(); ctx.nstages];
        let mut ends_at = vec![Vec::new(); ctx.nstages];
        for sd in &p.stalls {
            let end = if sd.stages.is_empty() {
                match sd.in_stage {
                    Some(s) => s,
                    None => {
                        self.push_error(pipeline_err(
                            ErrorCode::E5013,
                            sd.span,
                            lstr!(en: "module-level 'stall when' needs a stage list"; tr: "modül seviyesinde 'stall when' aşama listesi ister"),
                            lstr!(en: "write 'stall Fetch, Decode when cond', or move the statement into a stage body"; tr: "'stall Fetch, Decode when koşul' yazın ya da deyimi bir aşama gövdesine taşıyın"),
                        ));
                        continue;
                    }
                }
            } else {
                let Some(end) = self.resolve_prefix(&sd.stages, sd.span, ctx) else {
                    continue;
                };
                end
            };
            for stage_conds in per_stage.iter_mut().take(end + 1) {
                stage_conds.push(sd.cond);
            }
            ends_at[end].push(sd.cond);
        }
        (per_stage, ends_at)
    }

    /// Stall listesi `0..=k` bitişik öneki olmalı (ADR-0038 §4).
    fn resolve_prefix(&mut self, names: &[Name], span: Span, ctx: &Ctx) -> Option<usize> {
        let mut idxs = Vec::new();
        for n in names {
            match ctx.stage_index.get(&n.text) {
                Some(&i) => idxs.push(i),
                None => {
                    self.push_error(pipeline_err(
                        ErrorCode::E5013,
                        n.span,
                        lstr!(en: "unknown stage '{}' in the stall list", n.text; tr: "stall listesinde bilinmeyen aşama '{}'", n.text),
                        lstr!(en: "use the declared stage names"; tr: "bildirilen aşama adlarını kullanın"),
                    ));
                    return None;
                }
            }
        }
        idxs.sort_unstable();
        idxs.dedup();
        let is_prefix = idxs.first() == Some(&0) && *idxs.last().unwrap() == idxs.len() - 1;
        if !is_prefix {
            self.push_error(pipeline_err(
                ErrorCode::E5013,
                span,
                lstr!(en: "stalled stages must form a contiguous prefix starting at the first stage"; tr: "durdurulan aşamalar ilk aşamadan başlayan bitişik bir önek olmalı"),
                lstr!(en: "a held stage would be overwritten by the stage still advancing behind it — stall the earlier stages too"; tr: "tutulan aşama, arkasında ilerlemeyi sürdüren aşama tarafından ezilir — önceki aşamaları da durdurun"),
            ));
            return None;
        }
        Some(idxs.len() - 1)
    }

    fn flush_sets(&mut self, p: &PipelineDecl, ctx: &Ctx) -> Vec<Vec<Idx<Expr>>> {
        let mut per_stage = vec![Vec::new(); ctx.nstages];
        for fd in &p.flushes {
            if fd.stages.is_empty() {
                self.push_error(pipeline_err(
                    ErrorCode::E5013,
                    fd.span,
                    lstr!(en: "'flush when' needs a stage list"; tr: "'flush when' aşama listesi ister"),
                    lstr!(en: "write 'flush Fetch, Decode when cond'"; tr: "'flush Fetch, Decode when koşul' yazın"),
                ));
                continue;
            }
            for n in &fd.stages {
                match ctx.stage_index.get(&n.text) {
                    Some(&i) => per_stage[i].push(fd.cond),
                    None => {
                        self.push_error(pipeline_err(
                            ErrorCode::E5013,
                            n.span,
                            lstr!(en: "unknown stage '{}' in the flush list", n.text; tr: "flush listesinde bilinmeyen aşama '{}'", n.text),
                            lstr!(en: "use the declared stage names"; tr: "bildirilen aşama adlarını kullanın"),
                        ));
                    }
                }
            }
        }
        per_stage
    }

    /// `let stall_<s> : bool = c1 || c2 ...` — kendi aşamasının
    /// gecikmesine sabitlenir (koşul karışımı kontrol sinyalidir).
    fn guard_let(
        &mut self,
        name: &str,
        conds: &[Idx<Expr>],
        stage: usize,
        ctx: &mut Ctx,
    ) -> LetDecl {
        let value = self.or_chain(conds);
        let nspan = ctx.fresh_span();
        let cond_span = self.ast.exprs[conds[0]].span;
        self.ast
            .timing
            .pinned
            .insert(nspan, (stage as u32, cond_span));
        let ty = self.ast.types.alloc(TypeRef {
            span: cond_span,
            kind: TypeRefKind::Bool,
        });
        LetDecl {
            name: Name {
                text: name.to_string(),
                span: nspan,
            },
            ty: Some(ty),
            value,
        }
    }

    fn or_chain(&mut self, conds: &[Idx<Expr>]) -> Idx<Expr> {
        let mut it = conds.iter().copied();
        let first = it.next().expect("guard_let yalnız dolu listeyle çağrılır");
        it.fold(first, |acc, c| {
            let span = self.ast.exprs[acc].span;
            self.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::Binary {
                    op: volt_ast::BinOp::Or,
                    lhs: acc,
                    rhs: c,
                },
            })
        })
    }

    // ═══ Sıralama ═════════════════════════════════════════════════

    /// Taşınan let'leri + muhafız let'lerini bağımlılık sırasına dizer
    /// (çözümleme sıralıdır); çevrim E5015. Kararlı sıra: kaynak sırası.
    fn order_lets(
        &mut self,
        hoisted: Vec<(String, usize, LetDecl)>,
        guards: Vec<(String, LetDecl)>,
        _ctx: &Ctx,
        pl_span: Span,
    ) -> Vec<LetDecl> {
        let mut pool: Vec<(String, LetDecl)> = hoisted
            .into_iter()
            .map(|(n, _, l)| (n, l))
            .chain(guards)
            .collect();
        let names: HashSet<String> = pool.iter().map(|(n, _)| n.clone()).collect();

        // Bağımlılıklar: let değerindeki tek parçalı yol adları ∩ havuz.
        let deps: Vec<HashSet<String>> = pool
            .iter()
            .map(|(_, l)| {
                let mut found = HashSet::new();
                self.collect_names(l.value, &names, &mut found);
                found
            })
            .collect();

        let n = pool.len();
        let mut emitted: HashSet<String> = HashSet::new();
        let mut done = vec![false; n];
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let mut progressed = false;
            for i in 0..n {
                if done[i] {
                    continue;
                }
                if deps[i]
                    .iter()
                    .all(|d| emitted.contains(d) || d == &pool[i].0)
                {
                    emitted.insert(pool[i].0.clone());
                    done[i] = true;
                    let entry = std::mem::replace(
                        &mut pool[i],
                        (
                            String::new(),
                            LetDecl {
                                name: Name {
                                    text: String::new(),
                                    span: pl_span,
                                },
                                ty: None,
                                value: self.alloc_error_expr(pl_span),
                            },
                        ),
                    );
                    out.push(entry.1);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        if out.len() < n {
            let cyclic: Vec<String> = (0..n)
                .filter(|&i| !done[i])
                .map(|i| pool[i].0.clone())
                .collect();
            self.push_error(pipeline_err(
                ErrorCode::E5015,
                pl_span,
                lstr!(en: "combinational cycle through stage references: {}", cyclic.join(" → "); tr: "aşama referansları üzerinden kombinasyonel çevrim: {}", cyclic.join(" → ")),
                lstr!(en: "route one direction through a pipeline register (read the value from a later stage)"; tr: "bir yönü boru hattı register'ından geçirin (değeri sonraki aşamadan okuyun)"),
            ));
            for i in 0..n {
                if !done[i] {
                    let entry = std::mem::replace(
                        &mut pool[i],
                        (
                            String::new(),
                            LetDecl {
                                name: Name {
                                    text: String::new(),
                                    span: pl_span,
                                },
                                ty: None,
                                value: self.alloc_error_expr(pl_span),
                            },
                        ),
                    );
                    out.push(entry.1);
                }
            }
        }
        out
    }

    fn collect_names(&self, e: Idx<Expr>, pool: &HashSet<String>, out: &mut HashSet<String>) {
        if let ExprKind::Path(path) = &self.ast.exprs[e].kind {
            if path.segments.len() == 1 && pool.contains(&path.segments[0].text) {
                out.insert(path.segments[0].text.clone());
            }
            return;
        }
        let (exprs, blocks) = expr_children(&self.ast.exprs[e].kind);
        for c in exprs {
            self.collect_names(c, pool, out);
        }
        for b in blocks {
            let block = &self.ast.blocks[b];
            for bs in &block.stmts {
                self.collect_block_names(bs, pool, out);
            }
        }
    }

    /// Modül seviyesi deyimdeki tek parçalı yol adları ∩ havuz.
    fn collect_stmt_names(&self, si: Idx<Stmt>, pool: &HashSet<String>, out: &mut HashSet<String>) {
        let mut exprs: Vec<Idx<Expr>> = Vec::new();
        let mut blocks: Vec<Idx<Block>> = Vec::new();
        match &self.ast.stmts[si].kind {
            StmtKind::Reg(r) => exprs.push(r.init),
            StmtKind::Let(l) => exprs.push(l.value),
            StmtKind::Assign(a) => {
                exprs.push(a.rhs);
                exprs.extend(a.lhs.suffixes.iter().flat_map(lvalue_suffix_exprs));
            }
            StmtKind::On(on) => blocks.push(on.body),
            StmtKind::Comb(b) => blocks.push(*b),
            StmtKind::For(f) => {
                exprs.extend([f.start, f.end]);
                blocks.push(f.body);
            }
            StmtKind::Expr(e) => exprs.push(*e),
            StmtKind::Instance(inst) => {
                exprs.extend(inst.bindings.iter().filter_map(|b| b.value));
            }
            StmtKind::Wire(_) | StmtKind::Error => {}
        }
        for e in exprs {
            self.collect_names(e, pool, out);
        }
        for b in blocks {
            for bs in &self.ast.blocks[b].stmts {
                self.collect_block_names(bs, pool, out);
            }
        }
    }

    fn collect_block_names(
        &self,
        bs: &BlockStmt,
        pool: &HashSet<String>,
        out: &mut HashSet<String>,
    ) {
        match bs {
            BlockStmt::NonBlockAssign { rhs, .. } | BlockStmt::BlockAssign { rhs, .. } => {
                self.collect_names(*rhs, pool, out)
            }
            BlockStmt::If(i) => {
                self.collect_names(i.cond, pool, out);
                for bs in &self.ast.blocks[i.then_block].stmts {
                    self.collect_block_names(bs, pool, out);
                }
            }
            BlockStmt::Match(m) => self.collect_names(m.scrutinee, pool, out),
            BlockStmt::Let(l) => self.collect_names(l.value, pool, out),
            BlockStmt::For(_) | BlockStmt::Error => {}
        }
    }

    // ═══ on-clk üretimi ═══════════════════════════════════════════

    fn single_clock(&mut self, p: &PipelineDecl, name_span: Span) -> Option<String> {
        let clocks: Vec<String> = p
            .ports
            .iter()
            .filter(|port| matches!(self.ast.types[port.ty].kind, TypeRefKind::Clock))
            .map(|port| port.name.text.clone())
            .collect();
        if clocks.len() == 1 {
            return Some(clocks.into_iter().next().unwrap());
        }
        self.push_error(pipeline_err(
            ErrorCode::E5011,
            name_span,
            lstr!(en: "a pipeline needs exactly one clock port (found {})", clocks.len(); tr: "pipeline tam bir saat portu ister ({} bulundu)", clocks.len()),
            lstr!(en: "declare a single 'in clk : clock' port"; tr: "tek bir 'in clk : clock' portu bildirin"),
        ));
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn build_on_block(
        &mut self,
        clk: &str,
        remainders: &[Vec<BlockStmt>],
        boundary_regs: &[Vec<BoundaryReg>],
        stall_name: &HashMap<usize, String>,
        flush_name: &HashMap<usize, String>,
        stall_conds: &[Vec<Idx<Expr>>],
        ends_at: &[Vec<Idx<Expr>>],
        used: &mut HashSet<String>,
        ctx: &mut Ctx,
        name_span: Span,
    ) -> Idx<Stmt> {
        let mut stmts: Vec<BlockStmt> = Vec::new();

        // Kullanıcının aşama içi ardışık deyimleri, stall muhafızıyla.
        for (s, rest) in remainders.iter().enumerate() {
            if rest.is_empty() {
                continue;
            }
            let body: Vec<BlockStmt> = rest.iter().map(clone_block_stmt_shallow).collect();
            match stall_name.get(&s) {
                Some(nm) => {
                    used.insert(nm.clone());
                    let cond = self.not_of(nm, name_span, ctx);
                    let block = self.seq_block(body, name_span);
                    stmts.push(BlockStmt::If(IfStmt {
                        span: name_span,
                        cond,
                        then_block: block,
                        else_branch: None,
                    }));
                }
                None => stmts.extend(body),
            }
        }

        // Sınır register'ları (ADR-0038 §5 şablonu).
        for (b, regs) in boundary_regs.iter().enumerate() {
            if regs.is_empty() {
                continue;
            }
            let normal: Vec<BlockStmt> = regs
                .iter()
                .map(|(nm, src, _)| self.assign_to(nm, *src, name_span, ctx))
                .collect();
            let zeros = |me: &mut Self, ctx: &mut Ctx| -> Vec<BlockStmt> {
                regs.iter()
                    .map(|(nm, _, ty)| {
                        let z = me.zero_expr(*ty, name_span);
                        me.assign_to(nm, z, name_span, ctx)
                    })
                    .collect()
            };

            let stalled_next = (b + 1 < ctx.nstages)
                .then(|| stall_name.get(&(b + 1)))
                .flatten();
            let has_bubble = !ends_at[b].is_empty();

            // İç kısım: hold/bubble/normal.
            let rest: Vec<BlockStmt> = match (stalled_next, has_bubble) {
                (Some(next), true) => {
                    used.insert(next.clone());
                    let z = zeros(self, ctx);
                    let bc =
                        self.bubble_cond(b, ends_at, stall_conds, stall_name, used, ctx, name_span);
                    let inner = self.if_else(bc, z, normal, name_span);
                    let cond = self.not_of(next, name_span, ctx);
                    let block = self.seq_block(vec![inner], name_span);
                    vec![BlockStmt::If(IfStmt {
                        span: name_span,
                        cond,
                        then_block: block,
                        else_branch: None,
                    })]
                }
                (Some(next), false) => {
                    used.insert(next.clone());
                    let cond = self.not_of(next, name_span, ctx);
                    let block = self.seq_block(normal, name_span);
                    vec![BlockStmt::If(IfStmt {
                        span: name_span,
                        cond,
                        then_block: block,
                        else_branch: None,
                    })]
                }
                (None, true) => {
                    let z = zeros(self, ctx);
                    let bc =
                        self.bubble_cond(b, ends_at, stall_conds, stall_name, used, ctx, name_span);
                    vec![self.if_else(bc, z, normal, name_span)]
                }
                (None, false) => normal,
            };

            match flush_name.get(&b) {
                Some(fnm) => {
                    used.insert(fnm.clone());
                    let z = zeros(self, ctx);
                    let cond = self.path_expr_fresh(fnm, name_span, ctx);
                    stmts.push(self.if_else(cond, z, rest, name_span));
                }
                None => stmts.extend(rest),
            }
        }

        let block = self.seq_block(stmts, name_span);
        let clk_span = ctx.fresh_span();
        self.ast.stmts.alloc(Stmt {
            span: name_span,
            attrs: Vec::new(),
            kind: StmtKind::On(OnBlock {
                trigger: OnTrigger::Clock(Name {
                    text: clk.to_string(),
                    span: clk_span,
                }),
                body: block,
            }),
        })
    }

    /// Bubble koşulu: önek tam bu sınırda bitiyorsa okunabilir
    /// `stall_<b>` sinyali, aksi halde ilgili koşulların OR'u.
    #[allow(clippy::too_many_arguments)]
    fn bubble_cond(
        &mut self,
        b: usize,
        ends_at: &[Vec<Idx<Expr>>],
        stall_conds: &[Vec<Idx<Expr>>],
        stall_name: &HashMap<usize, String>,
        used: &mut HashSet<String>,
        ctx: &mut Ctx,
        span: Span,
    ) -> Idx<Expr> {
        if ends_at[b] == stall_conds[b] {
            let nm = &stall_name[&b];
            used.insert(nm.clone());
            self.path_expr_fresh(nm, span, ctx)
        } else {
            self.or_chain(&ends_at[b])
        }
    }

    fn if_else(
        &mut self,
        cond: Idx<Expr>,
        then_stmts: Vec<BlockStmt>,
        mut else_stmts: Vec<BlockStmt>,
        span: Span,
    ) -> BlockStmt {
        let then_block = self.seq_block(then_stmts, span);
        // Tek if'lik else, `else if` zincirine düzleşir — üretilen SV
        // elle yazılmış gibi okunur (ADR-0012).
        let else_branch = match else_stmts.len() {
            0 => None,
            1 if matches!(else_stmts[0], BlockStmt::If(_)) => {
                let BlockStmt::If(inner) = else_stmts.pop().unwrap() else {
                    unreachable!()
                };
                Some(ElseBranch::If(Box::new(inner)))
            }
            _ => Some(ElseBranch::Block(self.seq_block(else_stmts, span))),
        };
        BlockStmt::If(IfStmt {
            span,
            cond,
            then_block,
            else_branch,
        })
    }

    fn seq_block(&mut self, stmts: Vec<BlockStmt>, span: Span) -> Idx<Block> {
        self.ast.blocks.alloc(Block {
            span,
            stmts,
            tail: None,
            context: BlockContext::Sequential,
        })
    }

    fn assign_to(&mut self, name: &str, rhs: Idx<Expr>, span: Span, ctx: &mut Ctx) -> BlockStmt {
        let nspan = ctx.fresh_span();
        BlockStmt::NonBlockAssign {
            lhs: LValue {
                span,
                base: Name {
                    text: name.to_string(),
                    span: nspan,
                },
                suffixes: Vec::new(),
            },
            rhs,
            span,
        }
    }

    fn not_of(&mut self, name: &str, span: Span, ctx: &mut Ctx) -> Idx<Expr> {
        let p = self.path_expr_fresh(name, span, ctx);
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::Unary {
                op: volt_ast::UnOp::Not,
                operand: p,
            },
        })
    }

    fn path_expr(&mut self, name: &str, span: Span) -> Idx<Expr> {
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::Path(Path {
                span,
                segments: vec![Name {
                    text: name.to_string(),
                    span,
                }],
            }),
        })
    }

    fn path_expr_fresh(&mut self, name: &str, span: Span, ctx: &mut Ctx) -> Idx<Expr> {
        let nspan = ctx.fresh_span();
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::Path(Path {
                span,
                segments: vec![Name {
                    text: name.to_string(),
                    span: nspan,
                }],
            }),
        })
    }

    // ═══ Tipler ve sıfırlar ═══════════════════════════════════════

    /// Skaler tipse Some (kaynak tip düğümü), değilse None (E5014).
    fn copy_scalar_ty_check(&self, ty: Idx<TypeRef>) -> Option<Idx<TypeRef>> {
        match self.ast.types[ty].kind {
            TypeRefKind::Bool
            | TypeRefKind::UInt(_)
            | TypeRefKind::SInt(_)
            | TypeRefKind::Bits(_) => Some(ty),
            _ => None,
        }
    }

    fn copy_ty(&mut self, ty: Idx<TypeRef>, span: Span) -> Idx<TypeRef> {
        let kind = match &self.ast.types[ty].kind {
            TypeRefKind::Bool => TypeRefKind::Bool,
            TypeRefKind::UInt(w) => TypeRefKind::UInt(*w),
            TypeRefKind::SInt(w) => TypeRefKind::SInt(*w),
            // Genişlik ifadesi paylaşılır: sabit ifade, tek kez çözülür.
            TypeRefKind::Bits(e) => TypeRefKind::Bits(*e),
            _ => TypeRefKind::Error,
        };
        self.ast.types.alloc(TypeRef { span, kind })
    }

    /// Tipin sıfırı: bubble/init değeri (bool → false, sayısal → 0).
    fn zero_expr(&mut self, ty: Idx<TypeRef>, span: Span) -> Idx<Expr> {
        let kind = match self.ast.types[ty].kind {
            TypeRefKind::Bool => ExprKind::BoolLit(false),
            _ => ExprKind::IntLit {
                value: 0,
                suffix: None,
                base: NumBase::Dec,
            },
        };
        self.ast.exprs.alloc(Expr { span, kind })
    }
}

/// LValue son eklerindeki ifade indeksleri.
fn lvalue_suffix_exprs(s: &volt_ast::LValueSuffix) -> Vec<Idx<Expr>> {
    use volt_ast::LValueSuffix::*;
    match s {
        Index(e) => vec![*e],
        Range { hi, lo } => vec![*hi, *lo],
        PartSelect { start, width, .. } => vec![*start, *width],
        Field(_) => Vec::new(),
    }
}

/// Match kollarındaki ifade/blok indeksleri.
fn collect_arm_idxs(
    arms: &[volt_ast::MatchArm],
    exprs: &mut Vec<Idx<Expr>>,
    blocks: &mut Vec<Idx<Block>>,
) {
    for a in arms {
        if let Some(g) = a.guard {
            exprs.push(g);
        }
        match &a.body {
            volt_ast::MatchArmBody::Expr(e) => exprs.push(*e),
            volt_ast::MatchArmBody::Block(b) => blocks.push(*b),
        }
    }
}

/// Bir ifadenin çocuk ifade ve blok indeksleri (yeniden yazma gezgini).
fn expr_children(kind: &ExprKind) -> (Vec<Idx<Expr>>, Vec<Idx<Block>>) {
    let mut es = Vec::new();
    let mut bs = Vec::new();
    match kind {
        ExprKind::IntLit { .. }
        | ExprKind::BoolLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::Path(_)
        | ExprKind::Todo { .. }
        | ExprKind::Error => {}
        ExprKind::Binary { lhs, rhs, .. } => es.extend([*lhs, *rhs]),
        ExprKind::Unary { operand, .. } => es.push(*operand),
        ExprKind::Index { base, index } => es.extend([*base, *index]),
        ExprKind::Range { base, hi, lo } => es.extend([*base, *hi, *lo]),
        ExprKind::PartSelect {
            base, start, width, ..
        } => es.extend([*base, *start, *width]),
        ExprKind::Field { base, .. } => es.push(*base),
        ExprKind::Call { callee, args } => {
            es.push(*callee);
            es.extend(args.iter().copied());
        }
        ExprKind::Cast { expr, .. } => es.push(*expr),
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => es.extend([*cond, *then_expr, *else_expr]),
        ExprKind::Match { scrutinee, arms } => {
            es.push(*scrutinee);
            collect_arm_idxs(arms, &mut es, &mut bs);
        }
        ExprKind::StructLit { fields, .. } => es.extend(fields.iter().filter_map(|f| f.value)),
        ExprKind::ArrayLit(volt_ast::ArrayLitKind::List(items)) => es.extend(items.iter().copied()),
        ExprKind::ArrayLit(volt_ast::ArrayLitKind::Repeat { value, count }) => {
            es.extend([*value, *count])
        }
        ExprKind::TupleLit(items) => es.extend(items.iter().copied()),
    }
    (es, bs)
}

/// Kullanıcının aşama içi deyimleri on-clk gövdesine taşınırken sığ
/// kopyalanır — Idx alanları Copy olduğundan yapı yeniden kurulur.
fn clone_block_stmt_shallow(bs: &BlockStmt) -> BlockStmt {
    match bs {
        BlockStmt::NonBlockAssign { lhs, rhs, span } => BlockStmt::NonBlockAssign {
            lhs: clone_lvalue(lhs),
            rhs: *rhs,
            span: *span,
        },
        BlockStmt::BlockAssign { lhs, rhs, span } => BlockStmt::BlockAssign {
            lhs: clone_lvalue(lhs),
            rhs: *rhs,
            span: *span,
        },
        BlockStmt::If(i) => BlockStmt::If(clone_if(i)),
        BlockStmt::Match(m) => BlockStmt::Match(volt_ast::MatchStmt {
            span: m.span,
            scrutinee: m.scrutinee,
            arms: m.arms.iter().map(clone_arm).collect(),
        }),
        BlockStmt::Let(l) => BlockStmt::Let(LetDecl {
            name: l.name.clone(),
            ty: l.ty,
            value: l.value,
        }),
        BlockStmt::For(f) => BlockStmt::For(volt_ast::ForStmt {
            var: f.var.clone(),
            start: f.start,
            end: f.end,
            body: f.body,
        }),
        BlockStmt::Error => BlockStmt::Error,
    }
}

fn clone_lvalue(l: &LValue) -> LValue {
    use volt_ast::LValueSuffix::*;
    LValue {
        span: l.span,
        base: l.base.clone(),
        suffixes: l
            .suffixes
            .iter()
            .map(|s| match s {
                Index(e) => Index(*e),
                Range { hi, lo } => Range { hi: *hi, lo: *lo },
                PartSelect {
                    start,
                    width,
                    ascending,
                } => PartSelect {
                    start: *start,
                    width: *width,
                    ascending: *ascending,
                },
                Field(n) => Field(n.clone()),
            })
            .collect(),
    }
}

fn clone_if(i: &IfStmt) -> IfStmt {
    IfStmt {
        span: i.span,
        cond: i.cond,
        then_block: i.then_block,
        else_branch: match &i.else_branch {
            None => None,
            Some(ElseBranch::Block(b)) => Some(ElseBranch::Block(*b)),
            Some(ElseBranch::If(inner)) => Some(ElseBranch::If(Box::new(clone_if(inner)))),
        },
    }
}

fn clone_arm(a: &volt_ast::MatchArm) -> volt_ast::MatchArm {
    volt_ast::MatchArm {
        span: a.span,
        pattern: a.pattern,
        guard: a.guard,
        body: match &a.body {
            volt_ast::MatchArmBody::Expr(e) => volt_ast::MatchArmBody::Expr(*e),
            volt_ast::MatchArmBody::Block(b) => volt_ast::MatchArmBody::Block(*b),
        },
    }
}
