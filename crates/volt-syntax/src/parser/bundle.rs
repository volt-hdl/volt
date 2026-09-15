//! Bundle (port grubu) düzleştirme (ADR-0039).
//!
//! `struct port` tipli bir modül portu (`in aw : AxiWriteAddr`) parser
//! sonunda düz portlara açılır: her alan `<port>_<alan>` adlı bağımsız
//! bir `Port` olur, yönü alanın bildirilen yönüdür ve port `in` ise
//! TERS çevrilir. Gövde ve kontratlardaki `aw.addr` erişimleri düz
//! isme (`aw_addr`) yeniden yazılır. Çıktı sıradan portlardır — isim
//! çözümleme, tip denetimi, domain çıkarımı ve SV üretimi bundle'ı
//! görmez (ADR-0038'in silme ilkesi); yalnız `Port::bundle` kaynağı
//! E4005/E3013 tanıları için taşınır.
//!
//! Yerleşik `Handshake<T>` bundle'ı (ADR-0050) aynı yoldan açılır;
//! ayrıntısı `handshake.rs`'de: sanal alanlar (`fired`/`stalled`)
//! ifade olarak yeniden yazılır, protokol kontratları otomatik üretilir.

use std::collections::HashMap;

use volt_ast::{
    BinOp, Block, BlockStmt, BundleOrigin, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind,
    LValue, LValueSuffix, Name, Path, Port, PortDir, Stmt, StmtKind, TypeRef, TypeRefKind, UnOp,
};
use volt_span::Span;

use super::desugar::{collect_arm_idxs, expr_children, lvalue_suffix_exprs};
use super::handshake::{has_attr, HandshakeInfo, HANDSHAKE, NO_PROTOCOL_CHECK};
use super::Parser;

/// İç içe bundle derinlik sınırı — kendine referanslı tanımda sonsuz
/// açılımı keser.
pub(super) const MAX_NESTING: usize = 8;

/// `struct port` alanı (düzleştirme girdisi).
struct FieldInfo {
    dir: PortDir,
    name: String,
    ty: Idx<TypeRef>,
    domain: Option<Name>,
    /// Alan tipi başka bir bundle ise adı.
    nested: Option<String>,
}

/// Modül başına yeniden yazma haritası: `aw.addr` → `aw_addr`.
#[derive(Default)]
pub(super) struct Flat {
    pub names: HashMap<String, String>,
    /// Sanal alanlar (ADR-0050): `tx.fired` → `tx_valid && tx_ready`.
    pub virtuals: HashMap<String, Virtual>,
}

/// Bir sanal alanın açılımı: `valid && ready` ya da `valid && !ready`.
pub(super) struct Virtual {
    pub valid: String,
    pub ready: String,
    pub negate_ready: bool,
}

impl Flat {
    fn lookup(&self, key: &str) -> Option<&str> {
        self.names.get(key).map(String::as_str)
    }

    fn is_empty(&self) -> bool {
        self.names.is_empty() && self.virtuals.is_empty()
    }
}

/// Bir portun / alanın tipi tek segmentli, argümansız bir yol ise adı.
fn simple_type_name(types: &volt_ast::Arena<TypeRef>, ty: Idx<TypeRef>) -> Option<&str> {
    match &types[ty].kind {
        TypeRefKind::Path { path, args } if args.is_empty() && path.segments.len() == 1 => {
            Some(path.segments[0].text.as_str())
        }
        _ => None,
    }
}

pub(super) fn flip(dir: PortDir) -> PortDir {
    match dir {
        PortDir::In => PortDir::Out,
        PortDir::Out => PortDir::In,
        PortDir::InOut => PortDir::InOut,
        PortDir::OpenDrain => PortDir::OpenDrain,
    }
}

/// Sentetik isimler için benzersiz span (decl_spans / use_spans
/// anahtarı). Önce port bildirimi içindeki tek karakterlik konumlar
/// (tanılar doğru satırı gösterir; bundle portunun kendi adı
/// bildirilmediğinden çakışmaz), tükenirse öğe sonundan geriye sayan
/// sıfır genişlikli konumlar (pipeline desugar'ı öğe BAŞINDAN ileri
/// sayar). `counter` modül başına tektir.
pub(super) fn fresh_name_span(port_span: Span, item_span: Span, counter: &mut u32) -> Span {
    *counter += 1;
    if port_span.start + *counter <= port_span.end {
        let p = port_span.start + *counter - 1;
        Span::new(port_span.file, p, p + 1)
    } else {
        let p = item_span.end.saturating_sub(*counter).max(item_span.start);
        Span::new(item_span.file, p, p)
    }
}

impl Parser<'_> {
    /// Dosyadaki tüm modül ve extern modüllerde bundle portlarını açar.
    pub(crate) fn flatten_bundles(&mut self) {
        let defs = self.collect_bundle_defs();
        // Kullanıcı `struct port Handshake` (generic olsun olmasın) yazdıysa
        // yerleşik devre dışı.
        let builtin_handshake = !self.user_defines_handshake() && self.uses_builtin_handshake();
        if defs.is_empty() && !builtin_handshake {
            return;
        }
        let plain = self.collect_plain_struct_defs();
        let items: Vec<_> = self.ast.items.clone();
        for item in items {
            let item_span = self.ast.items_arena[item].span;
            let item_no_check = has_attr(&self.ast.items_arena[item].attrs, NO_PROTOCOL_CHECK);
            let is_module = matches!(self.ast.items_arena[item].kind, ItemKind::Module(_));
            let ports = match &mut self.ast.items_arena[item].kind {
                ItemKind::Module(m) => std::mem::take(&mut m.ports),
                ItemKind::Extern(x) => std::mem::take(&mut x.ports),
                _ => continue,
            };
            let mut flat = Flat::default();
            let mut counter = 0u32;
            let mut out = Vec::with_capacity(ports.len());
            let mut handshakes: Vec<HandshakeInfo> = Vec::new();
            for port in ports {
                let bundle = simple_type_name(&self.ast.types, port.ty)
                    .filter(|n| defs.contains_key(*n))
                    .map(str::to_string);
                let payload = if builtin_handshake {
                    self.handshake_payload(port.ty)
                } else {
                    None
                };
                match (bundle, payload) {
                    (Some(bundle), _) => {
                        // `in` port yönleri tersler; `out`/`inout` bildirildiği gibi.
                        let flipped = port.direction == PortDir::In;
                        let prefix = port.name.text.clone();
                        expand(
                            &defs,
                            &port,
                            &bundle,
                            &prefix,
                            "",
                            flipped,
                            0,
                            &mut out,
                            &mut flat,
                            &mut counter,
                            item_span,
                        );
                    }
                    (None, Some(payload)) => {
                        let no_check = item_no_check || has_attr(&port.attrs, NO_PROTOCOL_CHECK);
                        let info = self.expand_handshake(
                            &port,
                            payload,
                            &plain,
                            &mut out,
                            &mut flat,
                            &mut counter,
                            item_span,
                        );
                        // extern modülün kontratı yoktur: sentezlenmez.
                        if !no_check && is_module {
                            handshakes.push(info);
                        }
                    }
                    (None, None) => out.push(port),
                }
            }
            let (body, contracts) = match &mut self.ast.items_arena[item].kind {
                ItemKind::Module(m) => {
                    m.ports = out;
                    (
                        m.body.clone(),
                        m.contracts.iter().map(|c| c.expr).collect::<Vec<_>>(),
                    )
                }
                ItemKind::Extern(x) => {
                    x.ports = out;
                    continue;
                }
                _ => unreachable!(),
            };
            if flat.is_empty() {
                continue;
            }
            for e in contracts {
                self.rw_bundle_expr(e, &flat);
            }
            for s in body {
                self.rw_bundle_stmt(s, &flat);
            }
            // Otomatik protokol kontratları düz isimlerle üretilir;
            // yeniden yazmaya girmez.
            let mut auto = Vec::new();
            for info in &handshakes {
                auto.extend(self.handshake_contracts(info, &mut counter, item_span));
            }
            if let ItemKind::Module(m) = &mut self.ast.items_arena[item].kind {
                m.contracts.extend(auto);
            }
        }
    }

    /// Kullanıcı `Handshake` adlı bir `struct port` tanımlamış mı (generic
    /// olsa bile — o zaman düzleştirilmez ama yerleşik de devreye girmez)?
    fn user_defines_handshake(&self) -> bool {
        self.ast.items.iter().any(|&i| {
            matches!(&self.ast.items_arena[i].kind,
                ItemKind::Struct(s) if s.is_port && s.name.text == HANDSHAKE)
        })
    }

    /// Herhangi bir modül/extern portu `Handshake<T>` tipli mi?
    fn uses_builtin_handshake(&self) -> bool {
        self.ast.items.iter().any(|&i| {
            let ports = match &self.ast.items_arena[i].kind {
                ItemKind::Module(m) => &m.ports,
                ItemKind::Extern(x) => &x.ports,
                _ => return false,
            };
            ports.iter().any(|p| self.handshake_payload(p.ty).is_some())
        })
    }

    /// Dosyadaki generic olmayan `struct port` bildirimleri.
    fn collect_bundle_defs(&self) -> HashMap<String, Vec<FieldInfo>> {
        let mut defs: HashMap<String, Vec<FieldInfo>> = HashMap::new();
        let port_structs: Vec<&volt_ast::StructDecl> = self
            .ast
            .items
            .iter()
            .filter_map(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Struct(s) if s.is_port && s.generics.is_empty() => Some(s),
                _ => None,
            })
            .collect();
        for s in &port_structs {
            let fields = s
                .fields
                .iter()
                .map(|f| FieldInfo {
                    // Yönsüz alan parser hatası almıştır; kurtarma: out.
                    dir: f.direction.unwrap_or(PortDir::Out),
                    name: f.name.text.clone(),
                    ty: f.ty,
                    domain: f.domain.clone(),
                    nested: simple_type_name(&self.ast.types, f.ty)
                        .filter(|n| port_structs.iter().any(|p| p.name.text == *n))
                        .map(str::to_string),
                })
                .collect();
            defs.insert(s.name.text.clone(), fields);
        }
        defs
    }

    fn rw_bundle_stmt(&mut self, si: Idx<Stmt>, flat: &Flat) {
        let mut exprs: Vec<Idx<Expr>> = Vec::new();
        let mut blocks: Vec<Idx<Block>> = Vec::new();
        match &mut self.ast.stmts[si].kind {
            StmtKind::Reg(r) => exprs.push(r.init),
            StmtKind::Let(l) => exprs.push(l.value),
            StmtKind::Assign(a) => {
                rw_lvalue(&mut a.lhs, flat);
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
            self.rw_bundle_expr(e, flat);
        }
        for b in blocks {
            self.rw_bundle_block(b, flat);
        }
    }

    fn rw_bundle_block(&mut self, bi: Idx<Block>, flat: &Flat) {
        let mut stmts = std::mem::take(&mut self.ast.blocks[bi].stmts);
        for bs in &mut stmts {
            self.rw_bundle_block_stmt(bs, flat);
        }
        self.ast.blocks[bi].stmts = stmts;
        if let Some(tail) = self.ast.blocks[bi].tail {
            self.rw_bundle_expr(tail, flat);
        }
    }

    fn rw_bundle_block_stmt(&mut self, bs: &mut BlockStmt, flat: &Flat) {
        match bs {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                rw_lvalue(lhs, flat);
                self.rw_bundle_expr(*rhs, flat);
                let idxs: Vec<Idx<Expr>> =
                    lhs.suffixes.iter().flat_map(lvalue_suffix_exprs).collect();
                for e in idxs {
                    self.rw_bundle_expr(e, flat);
                }
            }
            BlockStmt::If(ifstmt) => self.rw_bundle_if(ifstmt, flat),
            BlockStmt::Match(m) => {
                self.rw_bundle_expr(m.scrutinee, flat);
                let (mut exprs, mut blocks) = (Vec::new(), Vec::new());
                collect_arm_idxs(&m.arms, &mut exprs, &mut blocks);
                for e in exprs {
                    self.rw_bundle_expr(e, flat);
                }
                for b in blocks {
                    self.rw_bundle_block(b, flat);
                }
            }
            BlockStmt::Let(l) => self.rw_bundle_expr(l.value, flat),
            BlockStmt::For(f) => {
                self.rw_bundle_expr(f.start, flat);
                self.rw_bundle_expr(f.end, flat);
                self.rw_bundle_block(f.body, flat);
            }
            BlockStmt::Error => {}
        }
    }

    fn rw_bundle_if(&mut self, ifstmt: &IfStmt, flat: &Flat) {
        self.rw_bundle_expr(ifstmt.cond, flat);
        self.rw_bundle_block(ifstmt.then_block, flat);
        match &ifstmt.else_branch {
            Some(ElseBranch::Block(b)) => self.rw_bundle_block(*b, flat),
            Some(ElseBranch::If(inner)) => self.rw_bundle_if(inner, flat),
            None => {}
        }
    }

    /// `aw.addr` (ve iç içe `aw.sub.addr`) alan zincirini düz porta
    /// yeniden yazar; eşleşmeyen düğümlerde çocuklara iner.
    fn rw_bundle_expr(&mut self, e: Idx<Expr>, flat: &Flat) {
        if let Some(key) = self.field_chain(e) {
            if let Some(name) = flat.lookup(&key) {
                let span = self.ast.exprs[e].span;
                self.ast.exprs[e].kind = ExprKind::Path(Path {
                    span,
                    segments: vec![Name {
                        text: name.to_string(),
                        span,
                    }],
                });
                return;
            }
            if let Some(v) = flat.virtuals.get(&key) {
                self.rw_virtual(e, v);
                return;
            }
        }
        let (exprs, blocks) = expr_children(&self.ast.exprs[e].kind);
        for c in exprs {
            self.rw_bundle_expr(c, flat);
        }
        for b in blocks {
            self.rw_bundle_block(b, flat);
        }
    }

    /// `tx.fired` → `tx_valid && tx_ready`, `tx.stalled` → `tx_valid &&
    /// !tx_ready` (ADR-0050). İki isim farklı span alır (`use_spans`
    /// anahtarı): ifadenin ilk ve son karakteri.
    fn rw_virtual(&mut self, e: Idx<Expr>, v: &Virtual) {
        let span = self.ast.exprs[e].span;
        let first = Span::new(span.file, span.start, (span.start + 1).min(span.end));
        let last = Span::new(
            span.file,
            span.end.saturating_sub(1).max(span.start),
            span.end,
        );
        let path = |me: &mut Self, text: &str, nspan: Span| {
            me.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::Path(Path {
                    span,
                    segments: vec![Name {
                        text: text.to_string(),
                        span: nspan,
                    }],
                }),
            })
        };
        let valid = path(self, &v.valid, first);
        let ready = path(self, &v.ready, last);
        let rhs = if v.negate_ready {
            self.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::Unary {
                    op: UnOp::Not,
                    operand: ready,
                },
            })
        } else {
            ready
        };
        self.ast.exprs[e].kind = ExprKind::Binary {
            op: BinOp::And,
            lhs: valid,
            rhs,
        };
    }

    /// `a.b.c` biçimindeki zinciri `"a.b.c"` anahtarına çevirir; taban
    /// tek segmentli bir yol değilse None.
    fn field_chain(&self, e: Idx<Expr>) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        let mut cur = e;
        loop {
            match &self.ast.exprs[cur].kind {
                ExprKind::Field { base, field } => {
                    parts.push(field.text.as_str());
                    cur = *base;
                }
                ExprKind::Path(p) if p.segments.len() == 1 && !parts.is_empty() => {
                    parts.push(p.segments[0].text.as_str());
                    parts.reverse();
                    return Some(parts.join("."));
                }
                _ => return None,
            }
        }
    }
}

/// Sol taraf: `aw.ready = x` → `aw_ready = x` (öndeki alan sonekleri
/// düşer, indeks/aralık sonekleri kalır).
fn rw_lvalue(lv: &mut LValue, flat: &Flat) {
    let mut key = lv.base.text.clone();
    let mut best: Option<(usize, String)> = None;
    for (i, s) in lv.suffixes.iter().enumerate() {
        let LValueSuffix::Field(f) = s else { break };
        key.push('.');
        key.push_str(&f.text);
        if let Some(name) = flat.lookup(&key) {
            best = Some((i + 1, name.to_string()));
        }
    }
    if let Some((n, name)) = best {
        lv.base.text = name;
        lv.suffixes.drain(..n);
    }
}

/// Bir bundle portunu düz portlara açar (iç içe bundle'larda özyineli).
#[allow(clippy::too_many_arguments)]
fn expand(
    defs: &HashMap<String, Vec<FieldInfo>>,
    port: &Port,
    bundle: &str,
    prefix: &str,
    path: &str,
    flipped: bool,
    depth: usize,
    out: &mut Vec<Port>,
    flat: &mut Flat,
    counter: &mut u32,
    item_span: Span,
) {
    if depth > MAX_NESTING {
        return;
    }
    let Some(fields) = defs.get(bundle) else {
        return;
    };
    for f in fields {
        let flat_name = format!("{prefix}_{}", f.name);
        let field_path = if path.is_empty() {
            f.name.clone()
        } else {
            format!("{path}.{}", f.name)
        };
        if let Some(inner) = &f.nested {
            expand(
                defs,
                port,
                inner,
                &flat_name,
                &field_path,
                flipped ^ (f.dir == PortDir::In),
                depth + 1,
                out,
                flat,
                counter,
                item_span,
            );
            continue;
        }
        // Bildirim span'i benzersiz olmalı (decl_spans anahtarı).
        let name_span = fresh_name_span(port.span, item_span, counter);
        flat.names.insert(
            format!("{}.{field_path}", port.name.text),
            flat_name.clone(),
        );
        out.push(Port {
            span: port.span,
            attrs: Vec::new(),
            doc: None,
            direction: if flipped { flip(f.dir) } else { f.dir },
            name: Name {
                text: flat_name,
                span: name_span,
            },
            ty: f.ty,
            // Anotasyon span'i de düzleştirilmiş port başına benzersiz
            // (use_spans anahtarı): aynı struct'ı iki extern kullanınca
            // sembolik alan tanımları birbirini ezmesin (ADR-0047).
            domain: f
                .domain
                .clone()
                .or_else(|| port.domain.clone())
                .map(|d| Name {
                    text: d.text,
                    span: name_span,
                }),
            bundle: Some(BundleOrigin {
                port: port.name.clone(),
                bundle: bundle.to_string(),
                path: field_path,
                declared: f.dir,
                flipped,
            }),
        });
    }
}
