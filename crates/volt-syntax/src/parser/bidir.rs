//! Çift yönlü portlar (ADR-0051): `inout` ve `opendrain`.
//!
//! ```volt
//! inout     dq  : bits<8>     // sürücü etkinken değer, değilse yüksek empedans
//! opendrain sda : bool        // yalnız aşağı çekilir; pull-up harici
//!
//! on clk {
//!     dq.drive(value)          // dq_oe <= true; dq_out <= value
//!     dq.release()             // dq_oe <= false
//!     sda.drive_low()          // sda_drive_low <= true
//!     sda.release()            // sda_drive_low <= false
//! }
//! let level = sda.read()       // hattın gerçek seviyesi (= sda)
//! invariant: !busy -> sda.released      // = !sda_drive_low
//! ```
//!
//! Sürücü niyeti modülün kendi register'larıdır: parser her sürülen
//! çift yönlü port için `<p>_oe` (+ `<p>_out`, yalnız `inout`) ya da
//! `<p>_drive_low` register'ını sentezler, yöntem çağrılarını sıradan
//! `<=` atamalarına, sanal alanları (`released`/`driving`) o
//! register'a, `read()` çağrısını portun kendisine yeniden yazar. İsim
//! çözümleme, tip denetimi ve domain çıkarımı yalnız sıradan port +
//! register görür (ADR-0038 silme ilkesi); SV üretimi port yönüne
//! bakarak `assign p = enable ? data : 'z` üretir (sv-mapping.md §17).
//!
//! Yöntem çağrıları yalnız `on` bloğunda geçerlidir (sürücü durumu
//! register'dır, sıfırlamada serbest); porta doğrudan atama, bilinmeyen
//! üye ve yanlış bağlam E4008'dir.

use std::collections::{HashMap, HashSet};

use volt_ast::{
    BidirRegs, Block, BlockContext, BlockStmt, ElseBranch, Expr, ExprKind, Idx, IfStmt, ItemKind,
    LValue, LetDecl, Name, NumBase, Path, Port, PortDir, RegDecl, Stmt, StmtKind, TypeRef,
    TypeRefKind, UnOp, BIDIR_DRIVE, BIDIR_DRIVE_LOW, BIDIR_DRIVING, BIDIR_READ, BIDIR_RELEASE,
    BIDIR_RELEASED,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::desugar::{collect_arm_idxs, expr_children, lvalue_suffix_exprs};
use super::Parser;

/// `opendrain` bağlamsal anahtar kelimesi (ADR-0023 tarzı: yalnız port
/// konumunda, `opendrain ad : tip` deseninde tanınır).
pub(crate) const OPENDRAIN: &str = "opendrain";

/// Ayrıştırma sırasındaki çift yönlü port durumu (öğe başına sıfırlanır).
#[derive(Default)]
pub(crate) struct BidirState {
    /// Bu öğenin çift yönlü portları → yönü.
    pub ports: HashMap<String, PortDir>,
    /// `p.drive(v)` iki deyim üretir; ikincisi bir sonraki deyim
    /// olarak buradan alınır (`parse_block` / pipeline aşama gövdesi).
    pub pending: Vec<BlockStmt>,
}

/// Port bildiriminden türeyen, modül gövdesine eklenecek sentetik
/// bildirim: sürülen port register alır, yalnız gözlenen port sabit
/// `let` (SV'de sabit tel; sürücü yok, `assign p = 'z`).
struct Synth {
    port_span: Span,
    regs: BidirRegs,
    ty: Idx<TypeRef>,
    driven: bool,
}

impl Parser<'_> {
    /// Port konumunda `opendrain ad : tip` mi?
    pub(crate) fn at_opendrain_port(&self) -> bool {
        self.at(crate::token::TokenKind::Ident)
            && self.current_text() == OPENDRAIN
            && matches!(self.peek(1), Some(crate::token::TokenKind::Ident))
            && matches!(self.peek(2), Some(crate::token::TokenKind::Colon))
    }

    /// Blok deyimi olarak `p.yöntem(args)` (ADR-0051). Sol taraf zaten
    /// ifade olarak ayrıştırıldı; çift yönlü bir portun yöntem çağrısıysa
    /// `<=` atamalarına çevrilir ve `Some` döner (`;` tüketilir). Değilse
    /// `None`: sıradan atama yolu sürer.
    pub(crate) fn try_bidir_call(
        &mut self,
        lhs: Idx<Expr>,
        ctx: BlockContext,
    ) -> Option<BlockStmt> {
        let span = self.ast.exprs[lhs].span;
        let ExprKind::Call { callee, args } = &self.ast.exprs[lhs].kind else {
            return None;
        };
        let args = args.clone();
        let ExprKind::Field { base, field } = &self.ast.exprs[*callee].kind else {
            return None;
        };
        let method = field.clone();
        let ExprKind::Path(p) = &self.ast.exprs[*base].kind else {
            return None;
        };
        let [port] = p.segments.as_slice() else {
            return None;
        };
        let port = port.clone();
        let dir = *self.bidir.ports.get(&port.text)?;
        let regs = dir.bidir_regs(&port.text)?;
        self.eat(crate::token::TokenKind::Semi);

        if ctx != BlockContext::Sequential {
            self.push_error(e4008(
                span,
                lstr!(en: "'{}.{}()' is only allowed inside an 'on' block", port.text, method.text;
                      tr: "'{}.{}()' yalnız 'on' bloğu içinde kullanılabilir", port.text, method.text),
                lstr!(en: "the drive state of a bidirectional port is a register: move the call into on clk {{ ... }}";
                      tr: "çift yönlü portun sürücü durumu bir register'dır: çağrıyı on clk {{ ... }} içine taşıyın"),
            ));
            return Some(BlockStmt::Error);
        }

        let arity_ok = |n: usize| args.len() == n;
        match (method.text.as_str(), dir) {
            (BIDIR_RELEASE, _) if arity_ok(0) => {
                let rhs = self.bool_lit(false, span);
                Some(self.nb_assign(regs.enable, port.span, rhs, span))
            }
            (BIDIR_DRIVE_LOW, PortDir::OpenDrain) if arity_ok(0) => {
                let rhs = self.bool_lit(true, span);
                Some(self.nb_assign(regs.enable, port.span, rhs, span))
            }
            (BIDIR_DRIVE, PortDir::InOut) if arity_ok(1) => {
                let data = regs.data.expect("inout portunun veri register'ı");
                let value = args[0];
                let second = self.nb_assign(data, method.span, value, span);
                self.bidir.pending.push(second);
                let rhs = self.bool_lit(true, span);
                Some(self.nb_assign(regs.enable, port.span, rhs, span))
            }
            (BIDIR_RELEASE, _)
            | (BIDIR_DRIVE_LOW, PortDir::OpenDrain)
            | (BIDIR_DRIVE, PortDir::InOut) => {
                let expected = if method.text == BIDIR_DRIVE { 1 } else { 0 };
                self.push_error(e4008(
                    span,
                    lstr!(en: "'{}.{}()' takes {expected} argument(s), {} given", port.text, method.text, args.len();
                          tr: "'{}.{}()' {expected} argüman alır, {} verildi", port.text, method.text, args.len()),
                    lstr!(en: "drive(value) carries the value to drive; drive_low() and release() take none";
                          tr: "drive(değer) sürülecek değeri alır; drive_low() ve release() argüman almaz"),
                ));
                Some(BlockStmt::Error)
            }
            (BIDIR_READ, _) => {
                self.push_error(e4008(
                    span,
                    lstr!(en: "'{}.read()' is an expression, not a statement", port.text;
                          tr: "'{}.read()' bir ifadedir, deyim değil", port.text),
                    lstr!(en: "use it on the right-hand side: let level = {}.read()", port.text;
                          tr: "sağ tarafta kullanın: let level = {}.read()", port.text),
                ));
                Some(BlockStmt::Error)
            }
            _ => {
                self.push_error(unknown_member(span, &port, &method.text, dir));
                Some(BlockStmt::Error)
            }
        }
    }

    fn bool_lit(&mut self, value: bool, span: Span) -> Idx<Expr> {
        self.ast.exprs.alloc(Expr {
            span,
            kind: ExprKind::BoolLit(value),
        })
    }

    fn nb_assign(
        &mut self,
        target: String,
        name_span: Span,
        rhs: Idx<Expr>,
        span: Span,
    ) -> BlockStmt {
        BlockStmt::NonBlockAssign {
            lhs: LValue {
                span,
                base: Name {
                    text: target,
                    span: name_span,
                },
                suffixes: Vec::new(),
            },
            rhs,
            span,
        }
    }

    /// Parser sonu geçidi: her modülde çift yönlü portların tip kuralı,
    /// `read()` / `released` / `driving` yeniden yazımı ve sürücü
    /// register'larının sentezi. Bundle düzleştirmesinden SONRA koşar
    /// (bundle alanı `inout` olabilir).
    pub(crate) fn expand_bidir_ports(&mut self) {
        let items: Vec<_> = self.ast.items.clone();
        for item in items {
            let (ports, is_module): (BidirPorts, bool) = match &self.ast.items_arena[item].kind {
                ItemKind::Module(m) => (bidir_ports(&m.ports), true),
                ItemKind::Extern(x) => (bidir_ports(&x.ports), false),
                _ => continue,
            };
            if ports.is_empty() {
                continue;
            }
            let mut synth: Vec<(String, Synth)> = Vec::new();
            for (name, dir, ty, port_span) in &ports {
                if !self.check_bidir_type(*dir, *ty, *port_span) {
                    continue;
                }
                let regs = dir
                    .bidir_regs(name)
                    .expect("çift yönlü port sürücü register'ı");
                synth.push((
                    name.clone(),
                    Synth {
                        port_span: *port_span,
                        regs,
                        ty: *ty,
                        driven: false,
                    },
                ));
            }
            if !is_module {
                continue;
            }
            let ItemKind::Module(m) = &self.ast.items_arena[item].kind else {
                unreachable!()
            };
            let body = m.body.clone();
            let contracts: Vec<Idx<Expr>> = m.contracts.iter().map(|c| c.expr).collect();
            let table: HashMap<String, (PortDir, BidirRegs)> = synth
                .iter()
                .map(|(n, s)| (n.clone(), (self.port_dir(&ports, n), s.regs.clone())))
                .collect();
            let mut referenced: HashSet<String> = HashSet::new();
            for e in contracts {
                self.rw_bidir_expr(e, &table, &mut referenced);
            }
            for &s in &body {
                self.rw_bidir_stmt(s, &table, &mut referenced);
            }
            // Sürülen portlar: gövdede `<enable> <= ...` ataması olanlar
            // (parse aşamasında yöntem çağrılarından üretildi).
            let mut written: HashSet<String> = HashSet::new();
            for &s in &body {
                if let StmtKind::On(on) = &self.ast.stmts[s].kind {
                    collect_written_names(&self.ast, on.body, &mut written);
                }
            }
            let mut decls: Vec<Idx<Stmt>> = Vec::new();
            for (name, s) in synth.iter_mut() {
                s.driven = written.contains(&s.regs.enable);
                if s.driven {
                    decls.extend(self.synth_regs(s));
                } else if referenced.contains(name) {
                    decls.push(self.synth_const_enable(s));
                }
            }
            if decls.is_empty() {
                continue;
            }
            if let ItemKind::Module(m) = &mut self.ast.items_arena[item].kind {
                decls.extend(std::mem::take(&mut m.body));
                m.body = decls;
            }
        }
    }

    fn port_dir(&self, ports: &BidirPorts, name: &str) -> PortDir {
        ports
            .iter()
            .find(|(n, ..)| n == name)
            .map(|(_, d, ..)| *d)
            .expect("çift yönlü port")
    }

    /// `opendrain` yalnız `bool`; `inout` bool / tam sayı / bits<N>
    /// (sürülen değer register'ının sıfır değeri yazılabilmeli).
    fn check_bidir_type(&mut self, dir: PortDir, ty: Idx<TypeRef>, port_span: Span) -> bool {
        let kind = &self.ast.types[ty].kind;
        let ok = match dir {
            PortDir::OpenDrain => matches!(kind, TypeRefKind::Bool | TypeRefKind::Error),
            _ => matches!(
                kind,
                TypeRefKind::Bool
                    | TypeRefKind::UInt(_)
                    | TypeRefKind::SInt(_)
                    | TypeRefKind::Bits(_)
                    | TypeRefKind::UIntN(_)
                    | TypeRefKind::SIntN(_)
                    | TypeRefKind::Error
            ),
        };
        if ok {
            return true;
        }
        let (msg, help) = if dir == PortDir::OpenDrain {
            (
                lstr!(en: "an 'opendrain' port must be 'bool'"; tr: "'opendrain' portu 'bool' olmalı"),
                lstr!(en: "an open-drain line carries one wired-AND bit; for a bus declare one port per line or use 'inout'";
                      tr: "açık drenaj hattı tek bir kablolu-VE bitidir; veri yolu için hat başına port bildirin ya da 'inout' kullanın"),
            )
        } else {
            (
                lstr!(en: "an 'inout' port must be 'bool', 'uN', 'iN' or 'bits<N>'"; tr: "'inout' portu 'bool', 'uN', 'iN' ya da 'bits<N>' olmalı"),
                lstr!(en: "the driven value is kept in a register with a zero reset value; arrays, tuples and structs have none";
                      tr: "sürülen değer sıfır reset değerli bir register'da tutulur; dizi, tuple ve struct için bu tanımsız"),
            )
        };
        self.diagnostics.push(e4008(port_span, msg, help));
        false
    }

    /// `reg <enable> : bool = false` (+ `reg <data> : T = 0`).
    fn synth_regs(&mut self, s: &Synth) -> Vec<Idx<Stmt>> {
        let mut out = Vec::new();
        let span = s.port_span;
        let mut counter = 0u32;
        let bool_ty = self.ast.types.alloc(TypeRef {
            span,
            kind: TypeRefKind::Bool,
        });
        let init = self.bool_lit(false, span);
        let name = Name {
            text: s.regs.enable.clone(),
            span: synthetic_span(span, &mut counter),
        };
        out.push(self.ast.stmts.alloc(Stmt {
            span,
            attrs: Vec::new(),
            kind: StmtKind::Reg(RegDecl {
                name,
                domain: None,
                ty: Some(bool_ty),
                init,
            }),
        }));
        if let Some(data) = &s.regs.data {
            let zero = |me: &mut Self| {
                me.ast.exprs.alloc(Expr {
                    span,
                    kind: ExprKind::IntLit {
                        value: 0,
                        suffix: None,
                        base: NumBase::Dec,
                    },
                })
            };
            let init = match self.ast.types[s.ty].kind {
                TypeRefKind::Bool => self.bool_lit(false, span),
                // bits<N> literal almaz: `0 as bits<N>` (type-inference.md §5).
                TypeRefKind::Bits(_) => {
                    let expr = zero(self);
                    self.ast.exprs.alloc(Expr {
                        span,
                        kind: ExprKind::Cast { expr, ty: s.ty },
                    })
                }
                _ => zero(self),
            };
            let name = Name {
                text: data.clone(),
                span: synthetic_span(span, &mut counter),
            };
            out.push(self.ast.stmts.alloc(Stmt {
                span,
                attrs: Vec::new(),
                kind: StmtKind::Reg(RegDecl {
                    name,
                    domain: None,
                    ty: Some(s.ty),
                    init,
                }),
            }));
        }
        out
    }

    /// Hiç sürülmeyen ama `released`/`driving` ile gözlenen port:
    /// `let <enable> : bool = false` (sabit tel).
    fn synth_const_enable(&mut self, s: &Synth) -> Idx<Stmt> {
        let span = s.port_span;
        let mut counter = 0u32;
        let bool_ty = self.ast.types.alloc(TypeRef {
            span,
            kind: TypeRefKind::Bool,
        });
        let value = self.bool_lit(false, span);
        self.ast.stmts.alloc(Stmt {
            span,
            attrs: Vec::new(),
            kind: StmtKind::Let(LetDecl {
                name: Name {
                    text: s.regs.enable.clone(),
                    span: synthetic_span(span, &mut counter),
                },
                ty: Some(bool_ty),
                value,
            }),
        })
    }

    // ─── Yeniden yazma gezgini (bundle.rs ile aynı iskelet) ───

    fn rw_bidir_stmt(
        &mut self,
        si: Idx<Stmt>,
        table: &HashMap<String, (PortDir, BidirRegs)>,
        referenced: &mut HashSet<String>,
    ) {
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
            self.rw_bidir_expr(e, table, referenced);
        }
        for b in blocks {
            self.rw_bidir_block(b, table, referenced);
        }
    }

    fn rw_bidir_block(
        &mut self,
        bi: Idx<Block>,
        table: &HashMap<String, (PortDir, BidirRegs)>,
        referenced: &mut HashSet<String>,
    ) {
        let stmts = std::mem::take(&mut self.ast.blocks[bi].stmts);
        for bs in &stmts {
            self.rw_bidir_block_stmt(bs, table, referenced);
        }
        self.ast.blocks[bi].stmts = stmts;
        if let Some(tail) = self.ast.blocks[bi].tail {
            self.rw_bidir_expr(tail, table, referenced);
        }
    }

    fn rw_bidir_block_stmt(
        &mut self,
        bs: &BlockStmt,
        table: &HashMap<String, (PortDir, BidirRegs)>,
        referenced: &mut HashSet<String>,
    ) {
        match bs {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                self.rw_bidir_expr(*rhs, table, referenced);
                let idxs: Vec<Idx<Expr>> =
                    lhs.suffixes.iter().flat_map(lvalue_suffix_exprs).collect();
                for e in idxs {
                    self.rw_bidir_expr(e, table, referenced);
                }
            }
            BlockStmt::If(ifstmt) => self.rw_bidir_if(ifstmt, table, referenced),
            BlockStmt::Match(m) => {
                self.rw_bidir_expr(m.scrutinee, table, referenced);
                let (mut exprs, mut blocks) = (Vec::new(), Vec::new());
                collect_arm_idxs(&m.arms, &mut exprs, &mut blocks);
                for e in exprs {
                    self.rw_bidir_expr(e, table, referenced);
                }
                for b in blocks {
                    self.rw_bidir_block(b, table, referenced);
                }
            }
            BlockStmt::Let(l) => self.rw_bidir_expr(l.value, table, referenced),
            BlockStmt::For(f) => {
                self.rw_bidir_expr(f.start, table, referenced);
                self.rw_bidir_expr(f.end, table, referenced);
                self.rw_bidir_block(f.body, table, referenced);
            }
            BlockStmt::Error => {}
        }
    }

    fn rw_bidir_if(
        &mut self,
        ifstmt: &IfStmt,
        table: &HashMap<String, (PortDir, BidirRegs)>,
        referenced: &mut HashSet<String>,
    ) {
        self.rw_bidir_expr(ifstmt.cond, table, referenced);
        self.rw_bidir_block(ifstmt.then_block, table, referenced);
        match &ifstmt.else_branch {
            Some(ElseBranch::Block(b)) => self.rw_bidir_block(*b, table, referenced),
            Some(ElseBranch::If(inner)) => self.rw_bidir_if(inner, table, referenced),
            None => {}
        }
    }

    /// `p.read()` → `p`; `p.released` → `!<enable>`; `p.driving` →
    /// `<enable>`; başka üye E4008.
    fn rw_bidir_expr(
        &mut self,
        e: Idx<Expr>,
        table: &HashMap<String, (PortDir, BidirRegs)>,
        referenced: &mut HashSet<String>,
    ) {
        let span = self.ast.exprs[e].span;
        // `p.read()`
        if let ExprKind::Call { callee, args } = &self.ast.exprs[e].kind {
            let (callee, nargs) = (*callee, args.len());
            if let Some((port, method)) = self.bidir_member(callee, table) {
                let dir = table[&port.text].0;
                if method.text == BIDIR_READ && nargs == 0 {
                    self.ast.exprs[e].kind = ExprKind::Path(Path {
                        span,
                        segments: vec![port],
                    });
                } else if method.text == BIDIR_READ {
                    self.diagnostics.push(e4008(
                        span,
                        lstr!(en: "'{}.read()' takes no argument", port.text;
                              tr: "'{}.read()' argüman almaz", port.text),
                        lstr!(en: "write it as {}.read()", port.text;
                              tr: "{}.read() biçiminde yazın", port.text),
                    ));
                } else if matches!(
                    method.text.as_str(),
                    BIDIR_DRIVE | BIDIR_DRIVE_LOW | BIDIR_RELEASE
                ) {
                    self.diagnostics.push(e4008(
                        span,
                        lstr!(en: "'{}.{}()' is a statement, not a value", port.text, method.text;
                              tr: "'{}.{}()' bir deyimdir, değer değil", port.text, method.text),
                        lstr!(en: "call it on its own line inside on clk {{ ... }}; read the line with {}.read()", port.text;
                              tr: "on clk {{ ... }} içinde kendi satırında çağırın; hattı {}.read() ile okuyun", port.text),
                    ));
                } else {
                    self.diagnostics
                        .push(unknown_member(span, &port, &method.text, dir));
                }
                return;
            }
        }
        // `p.released` / `p.driving`
        if let Some((port, member)) = self.bidir_member(e, table) {
            let (dir, regs) = table[&port.text].clone();
            match member.text.as_str() {
                BIDIR_RELEASED | BIDIR_DRIVING => {
                    referenced.insert(port.text.clone());
                    let enable_path = ExprKind::Path(Path {
                        span,
                        segments: vec![Name {
                            text: regs.enable,
                            span: member.span,
                        }],
                    });
                    self.ast.exprs[e].kind = if member.text == BIDIR_RELEASED {
                        let enable = self.ast.exprs.alloc(Expr {
                            span,
                            kind: enable_path,
                        });
                        ExprKind::Unary {
                            op: UnOp::Not,
                            operand: enable,
                        }
                    } else {
                        enable_path
                    };
                }
                BIDIR_READ => {
                    self.diagnostics.push(e4008(
                        span,
                        lstr!(en: "'{}.read' is a method: write {}.read()", port.text, port.text;
                              tr: "'{}.read' bir yöntemdir: {}.read() yazın", port.text, port.text),
                        lstr!(en: "add the parentheses"; tr: "parantezleri ekleyin"),
                    ));
                }
                _ => self
                    .diagnostics
                    .push(unknown_member(span, &port, &member.text, dir)),
            }
            return;
        }
        let (exprs, blocks) = expr_children(&self.ast.exprs[e].kind);
        for c in exprs {
            self.rw_bidir_expr(c, table, referenced);
        }
        for b in blocks {
            self.rw_bidir_block(b, table, referenced);
        }
    }

    /// `e` = `p.üye` ve `p` çift yönlü port ise (port adı, üye adı).
    fn bidir_member(
        &self,
        e: Idx<Expr>,
        table: &HashMap<String, (PortDir, BidirRegs)>,
    ) -> Option<(Name, Name)> {
        let ExprKind::Field { base, field } = &self.ast.exprs[e].kind else {
            return None;
        };
        let ExprKind::Path(p) = &self.ast.exprs[*base].kind else {
            return None;
        };
        let [port] = p.segments.as_slice() else {
            return None;
        };
        table
            .contains_key(&port.text)
            .then(|| (port.clone(), field.clone()))
    }
}

/// Bir öğenin çift yönlü portları: (ad, yön, tip, port span).
type BidirPorts = Vec<(String, PortDir, Idx<TypeRef>, Span)>;

fn bidir_ports(ports: &[Port]) -> BidirPorts {
    ports
        .iter()
        .filter(|p| p.direction.is_bidirectional())
        .map(|p| (p.name.text.clone(), p.direction, p.ty, p.span))
        .collect()
}

/// Sentetik bildirim adı için benzersiz span (`decl_spans` anahtarı):
/// port bildiriminin SONUNDAN geriye tek karakterlik konumlar (tip
/// metni); bundle düzleştirmesi baştan ileri sayar, port adı baştadır —
/// çakışmaz.
fn synthetic_span(port_span: Span, counter: &mut u32) -> Span {
    *counter += 1;
    let end = port_span
        .end
        .saturating_sub(*counter - 1)
        .max(port_span.start);
    let start = end.saturating_sub(1).max(port_span.start);
    Span::new(port_span.file, start, end)
}

/// `on` gövdesinde (iç içe bloklar dâhil) sol tarafı düz isim olan
/// atamaların hedef adları.
fn collect_written_names(ast: &volt_ast::SourceFile, block: Idx<Block>, out: &mut HashSet<String>) {
    for stmt in &ast.blocks[block].stmts {
        match stmt {
            BlockStmt::NonBlockAssign { lhs, .. } | BlockStmt::BlockAssign { lhs, .. } => {
                if lhs.suffixes.is_empty() {
                    out.insert(lhs.base.text.clone());
                }
            }
            BlockStmt::If(i) => collect_written_if(ast, i, out),
            BlockStmt::Match(m) => {
                for arm in &m.arms {
                    if let volt_ast::MatchArmBody::Block(b) = &arm.body {
                        collect_written_names(ast, *b, out);
                    }
                }
            }
            BlockStmt::For(f) => collect_written_names(ast, f.body, out),
            BlockStmt::Let(_) | BlockStmt::Error => {}
        }
    }
}

fn collect_written_if(ast: &volt_ast::SourceFile, i: &IfStmt, out: &mut HashSet<String>) {
    collect_written_names(ast, i.then_block, out);
    match &i.else_branch {
        Some(ElseBranch::Block(b)) => collect_written_names(ast, *b, out),
        Some(ElseBranch::If(inner)) => collect_written_if(ast, inner, out),
        None => {}
    }
}

/// E4008 — beş parça: kod, konum, açıklama, öneri, ADR referansı (not).
fn e4008(span: Span, message: String, help: String) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E4008,
        message,
        LabeledSpan::primary(span, lstr!(en: "here"; tr: "burada")),
        help,
    )
    .with_note(
        NoteKind::Reason,
        lstr!(en: "a bidirectional port is driven only through drive()/drive_low()/release() inside an 'on' block and read with read(); the compiler owns the tri-state buffer (ADR-0051)";
              tr: "çift yönlü port yalnız 'on' bloğunda drive()/drive_low()/release() ile sürülür ve read() ile okunur; üç durumlu tamponu derleyici üretir (ADR-0051)"),
    )
}

fn unknown_member(span: Span, port: &Name, member: &str, dir: PortDir) -> Diagnostic {
    let members = match dir {
        PortDir::OpenDrain => "drive_low(), release(), read(), released, driving",
        _ => "drive(value), release(), read(), released, driving",
    };
    e4008(
        span,
        lstr!(en: "'{}' has no member '{member}'", port.text;
              tr: "'{}' portunun '{member}' üyesi yok", port.text),
        lstr!(en: "{} '{}' offers: {members}", dir.keyword(), port.text;
              tr: "{} '{}' şunları sunar: {members}", dir.keyword(), port.text),
    )
}
